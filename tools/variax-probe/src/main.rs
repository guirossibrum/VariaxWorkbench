//! variax-probe: lean, owner-run, read-only probe built on `line6-devices`.
//!
//!   variax identity        identity request on PODxt interface 1
//!   variax read            identity, session on, two metadata reads, session off
//!   variax listen [SECS]   identity, session on, listen only, session off
//!   podxt read             PODxt identity and current-tone (edit buffer) read
//!   podxt listen [SECS]    PODxt identity, then listen only (pedal, knobs)
//!   podxt library          every stored tone/setup by memory location (G5, PODxt)
//!   variax library         every stored preset via program changes, then restore (G5)
//!   compare DIR_A DIR_B    offline: are two library backups identical?
//!   podxt packs            installed model packs (read-only query)
//!   podxt live-test [NAME=VALUE ...]  G6: live edits in the active tone, read, undo, read
//!                          (default: stomp/mod/delay/reverb toggled, treble +20, mid -20;
//!                          `all`: every live control, switches toggled, knobs +20)
//!   podxt stomp-test       G6: the same with only the stomp toggled
//!   podxt status           read-only global values (current channel, edited flag, ...)
//!   podxt tuner [SECS]     tuner on, note and cents every 250 ms, tuner off
//!   podxt channel 3C [--from 7B]       select a channel (program change)
//!   podxt channel-test 3C [--from 7B]  select a channel, check, switch back, check
//!
//! Only the requests defined by the `line6-devices` families can be sent, and
//! only those this file's policy allows. Every packet in and out is printed
//! and saved, with each reply, to a new folder under `--out` (default
//! `analysis/captures`). Nothing is overwritten.

#![forbid(unsafe_code)]

mod linked_variax;

use line6_devices::backup::{self, BackupFolder as Capture};
use line6_devices::family::{Family, podxt, variax};
use line6_devices::gate::{GatePolicy, Operation};
use line6_devices::identity::Identity;
use line6_devices::midi::ControllerTally;
use line6_devices::session::{Log, Path, Session};
use line6_devices::transport::{rawmidi, usb};
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The owner's direct permissions for this worktree-only tool.
struct OwnerGrants;

impl GatePolicy for OwnerGrants {
    fn allows(&self, operation: Operation) -> bool {
        match operation {
            // G2: Variax 600 identity through the PODxt Live.
            Operation::VariaxIdentity => true,
            // G3: Variax current-preset metadata read (session on/off, two blocks).
            Operation::VariaxRead => true,
            Operation::VariaxEditBufferWrite | Operation::VariaxStoreRestore => false,
            // "test my podxtlive same way": PODxt identity and current-tone read.
            Operation::PodxtIdentity | Operation::PodxtRead => true,
            // G5 for the PODxt: read every stored tone/setup by location (no switching).
            Operation::PodxtLibraryRead => true,
            // G5 + preset switching for the Variax 600: program change through
            // each preset for the library read, then back to the original.
            Operation::VariaxPresetSwitch
            | Operation::VariaxBankNameRead
            | Operation::VariaxLibraryBridge => true,
            // Owner's direct permission 2026-11-22: Line 6 Edit's read-only
            // installed model-pack query (`03 0E 00`).
            Operation::PodxtPackQuery => true,
            // G6, owner-approved single-controller edit-buffer tests and undo.
            Operation::PodxtEditBufferWrite => true,
            // G6, direct owner approval for item 2 in this worktree tool only.
            Operation::PodxtAdditionalEditBufferWrite => true,
            // The owner's probe permission covers edits with automatic undo.
            Operation::PodxtKeepEditBufferChanges => false,
            // Owner's request 2026-11-23: select the playing channel by program
            // change (like the footswitch). Refused on unsaved edits; no store.
            Operation::PodxtChannelSwitch => true,
            // Owner's direct permission for the standalone display check.
            Operation::PodxtVariaxControlRelease => true,
            // Owner's direct permission for the F-T-Model 1 → F-Spank 1 → return test.
            Operation::PodxtLinkedVariaxModelEdit => true,
            // Owner's request 2026-11-23 ("tap/tuner"): tuner on/off (CC 69) and the
            // read-only global-value query it needs (also used by `podxt status`).
            Operation::PodxtGlobalRead | Operation::PodxtTunerMode => true,
        }
    }
}

const USAGE: &str = "Usage: variax-probe variax identity|read|listen [SECONDS]|library [--out DIR]
       variax-probe podxt read|listen [SECONDS]|library [--out DIR]
       variax-probe podxt channel|channel-test CHANNEL [--from CHANNEL] [--out DIR]
       variax-probe podxt live-test [all|NAME=VALUE ...] --out DIR
       variax-probe podxt release-variax-control|linked-variax-model-test --out DIR
       variax-probe podxt status|tuner [SECONDS] [--out DIR]
       variax-probe compare FIRST_BACKUP SECOND_BACKUP
Device commands are owner-run. Live tests change the edit buffer and attempt undo.
--out is required for device commands (library backups: the repo's backups/,
everything else: analysis/captures/), so captures never land in the current folder.";

#[derive(Debug, PartialEq)]
enum Command {
    VariaxIdentity,
    VariaxRead,
    VariaxListen(u64),
    PodxtRead,
    PodxtListen(u64),
    VariaxLibrary,
    PodxtLibrary,
    Compare(PathBuf, PathBuf),
    PodxtPacks,
    PodxtReleaseVariaxControl,
    PodxtLinkedVariaxModelTest,
    PodxtLiveTest(Vec<podxt::LiveTarget>),
    PodxtStatus,
    PodxtTuner(u64),
    PodxtChannel {
        target: podxt::Location,
        from: Option<podxt::Location>,
        back: bool,
    },
}

impl Command {
    fn slug(&self) -> &'static str {
        match self {
            Command::VariaxIdentity => "variax-identity",
            Command::VariaxRead => "variax-read",
            Command::VariaxListen(_) => "variax-listen",
            Command::PodxtRead => "podxt-read",
            Command::PodxtListen(_) => "podxt-listen",
            Command::VariaxLibrary => "variax-library",
            Command::PodxtLibrary => "podxt-library",
            Command::Compare(..) => "compare",
            Command::PodxtPacks => "podxt-packs",
            Command::PodxtReleaseVariaxControl => "podxt-release-variax-control",
            Command::PodxtLinkedVariaxModelTest => "podxt-linked-variax-model-test",
            Command::PodxtLiveTest(_) => "podxt-live-test",
            Command::PodxtStatus => "podxt-status",
            Command::PodxtTuner(_) => "podxt-tuner",
            Command::PodxtChannel { back: false, .. } => "podxt-channel",
            Command::PodxtChannel { back: true, .. } => "podxt-channel-test",
        }
    }

    fn plan(&self) -> &'static str {
        match self {
            Command::VariaxIdentity => {
                "identity (sent once more only if the reply's start is lost)"
            }
            Command::VariaxRead => {
                "identity, Variax session on, metadata blocks 1 and 2, session off"
            }
            Command::VariaxListen(_) => "identity, Variax session on, listen only, session off",
            Command::PodxtRead => {
                "PODxt identity, then the PODxt edit-buffer (current tone) request"
            }
            Command::PodxtListen(_) => "PODxt identity, then listen only",
            Command::VariaxLibrary => {
                "identity, session on, current preset, then for each of 60 presets a program change and two metadata reads, switch back to the original, session off"
            }
            Command::PodxtLibrary => {
                "PODxt identity, current tone, every stored tone/amp setup/FX setup by memory location, current tone again"
            }
            Command::Compare(..) => "nothing (offline comparison)",
            Command::PodxtPacks => "PODxt identity, then the installed model-pack query",
            Command::PodxtReleaseVariaxControl => {
                "PODxt Live identity, then the Workbench Variax-control release once"
            }
            Command::PodxtLinkedVariaxModelTest => {
                "PODxt and Variax readback, F-Spank 1 edit, readback, F-T-Model 1 undo, readback"
            }
            Command::PodxtStatus => "PODxt identity, then read-only global value queries",
            Command::PodxtTuner(_) => {
                "PODxt identity, tuner on (CC 69), tuner note/deviation queries every 250 ms, tuner off"
            }
            Command::PodxtChannel { back: false, .. } => {
                "PODxt identity, current tone, stored tones (to check for unsaved edits), program change to the channel, current tone"
            }
            Command::PodxtChannel { back: true, .. } => {
                "PODxt identity, current tone, stored tones, program change to the channel, current tone, program change back, current tone"
            }
            Command::PodxtLiveTest(_) => {
                "PODxt identity, current tone, pack query for model changes, live edits, read, undo, read"
            }
        }
    }
}

fn parse(args: &[String]) -> Result<(Command, Option<PathBuf>), String> {
    let mut out = None;
    let mut words = Vec::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--out" {
            out = Some(PathBuf::from(iter.next().ok_or("--out needs a folder")?));
        } else {
            words.push(arg.as_str());
        }
    }
    let max = variax::LISTEN_MAX
        .as_secs()
        .min(podxt::LISTEN_MAX.as_secs());
    let seconds = |text: &str| match text.parse::<u64>() {
        Ok(seconds) if (1..=max).contains(&seconds) => Ok(seconds),
        _ => Err(format!("listen SECONDS must be a whole number 1..={max}")),
    };
    let command = match words.as_slice() {
        ["variax", "identity"] => Command::VariaxIdentity,
        ["variax", "read"] => Command::VariaxRead,
        ["variax", "listen"] => Command::VariaxListen(10),
        ["variax", "listen", text] => Command::VariaxListen(seconds(text)?),
        ["podxt", "read"] => Command::PodxtRead,
        ["podxt", "listen"] => Command::PodxtListen(10),
        ["podxt", "listen", text] => Command::PodxtListen(seconds(text)?),
        ["variax", "library"] => Command::VariaxLibrary,
        ["podxt", "library"] => Command::PodxtLibrary,
        ["podxt", "packs"] => Command::PodxtPacks,
        ["podxt", "release-variax-control"] => Command::PodxtReleaseVariaxControl,
        ["podxt", "linked-variax-model-test"] => Command::PodxtLinkedVariaxModelTest,
        ["podxt", "stomp-test"] => Command::PodxtLiveTest(vec![podxt::LiveTarget::Basic(
            podxt::Control::StompEnable,
            podxt::Target::Toggle,
        )]),
        ["podxt", "live-test", rest @ ..] => Command::PodxtLiveTest(live_targets(rest)?),
        ["podxt", "status"] => Command::PodxtStatus,
        ["podxt", "tuner"] => Command::PodxtTuner(15),
        ["podxt", "tuner", text] => match text.parse::<u64>() {
            Ok(n) if (1..=podxt::TUNER_MAX.as_secs()).contains(&n) => Command::PodxtTuner(n),
            _ => {
                return Err(format!(
                    "tuner SECONDS must be 1..={}",
                    podxt::TUNER_MAX.as_secs()
                ));
            }
        },
        ["podxt", mode @ ("channel" | "channel-test"), rest @ ..] => {
            let tone = |label: &str| {
                podxt::Location::tone_from_label(label)
                    .ok_or(format!("{label}: not a channel (1A..32D)"))
            };
            let (target, from) = match rest {
                [target] => (tone(target)?, None),
                [target, "--from", from] => (tone(target)?, Some(tone(from)?)),
                _ => return Err(USAGE.into()),
            };
            Command::PodxtChannel {
                target,
                from,
                back: *mode == "channel-test",
            }
        }
        ["compare", first, second] => Command::Compare(PathBuf::from(first), PathBuf::from(second)),
        _ => return Err(USAGE.into()),
    };
    Ok((command, out))
}

fn describe_identity(reply: &[u8], requests: Option<u8>) -> String {
    let Some(identity) = Identity::parse(reply) else {
        return "Identity: (unparsed)".into();
    };
    let device = Family::from_identity(&identity).map_or("unknown Line 6 device".to_owned(), |f| {
        f.member_name(identity.member).to_owned()
    });
    let requests = requests.map_or(String::new(), |n| format!(", identity requests sent: {n}"));
    format!(
        "Identity: {device} (family {:#06x}, member {:#06x}), version {}{requests}",
        identity.family,
        identity.member,
        identity.version_text()
    )
}

fn variax_lines(metadata: &variax::PresetMetadata) -> Vec<String> {
    let mut lines = vec![format!("Preset name : {}", metadata.name)];
    for (key, value) in &metadata.parameters {
        let shown = match value {
            variax::ParameterValue::Integer(v) if key.ends_with("_model") => format!("{v:#010x}"),
            variax::ParameterValue::Integer(v) => v.to_string(),
            variax::ParameterValue::Boolean(v) => v.to_string(),
            variax::ParameterValue::Float(v) => v.to_string(),
        };
        lines.push(format!("  {key} = {shown}"));
    }
    lines
}

/// One line per controller heard: name, count, first/last/min/max and position.
fn tally_lines(tally: &ControllerTally, name: impl Fn(u8) -> Option<String>) -> Vec<String> {
    let mut lines = vec![format!(
        "Controllers heard: {} (other messages: {})",
        tally.controllers.len(),
        tally.other_messages
    )];
    if tally.controllers.is_empty() {
        lines.push("  none: nothing was moved, or this device does not send it".into());
    }
    for ((channel, controller), t) in &tally.controllers {
        let label = name(*controller).unwrap_or_else(|| "unnamed".into());
        lines.push(format!(
            "  CC {controller:3} ch {channel} {label}: {} messages, first {} last {} (min {} max {}), last = {:.1}%",
            t.count,
            t.first,
            t.last,
            t.min,
            t.max,
            f32::from(t.last) / 127.0 * 100.0
        ));
    }
    lines
}

fn variax_control_name(controller: u8) -> Option<String> {
    variax::control(controller).map(|c| {
        let source = match (c.workbench, c.verified_on_device) {
            (true, true) => "Workbench, verified on device",
            (true, false) => "Workbench, not yet seen on device",
            (false, true) => "verified on device",
            (false, false) => "unverified",
        };
        format!("{} [{source}]", c.name)
    })
}

fn podxt_control_name(controller: u8) -> Option<String> {
    podxt::controls::name(controller).map(|n| format!("{n} [Line 6 Edit]"))
}

fn run_variax(command: &Command, capture: &mut Capture) -> Result<(), String> {
    let mut link = usb::UsbInterfaceLink::open(usb::VARIAX_VIA_PODXT_LIVE)?;
    let (detached, claimed) = link.state();
    let mut saved: Vec<(String, Vec<u8>)> = Vec::new();
    let mut progress_lines: Vec<String> = Vec::new();
    let mut variax_library: Option<variax::Library> = None;
    let mut summary = Vec::new();
    let (result, cleanup) = {
        let mut session = Session::new(&mut link, &OwnerGrants, capture);
        session.note(format!(
            "interface 1: kernel driver detached={detached}, claimed={claimed}, altsetting 1"
        ));
        let result = match command {
            Command::VariaxIdentity => variax::identify(&mut session).map(|id| {
                summary.push(describe_identity(&id.reply, Some(id.requests)));
                saved.push(("identity.syx".into(), id.reply));
            }),
            Command::VariaxRead => variax::read_current(&mut session).map(|preset| {
                summary.push(describe_identity(
                    &preset.identified.reply,
                    Some(preset.identified.requests),
                ));
                summary.extend(variax_lines(&preset.metadata));
                let [first, second] = preset.replies;
                saved.extend([
                    ("identity.syx".into(), preset.identified.reply),
                    ("metadata-1.syx".into(), first),
                    ("metadata-2.syx".into(), second),
                ]);
            }),
            Command::VariaxListen(seconds) => {
                variax::listen(&mut session, Duration::from_secs(*seconds)).map(|(id, tally)| {
                    summary.push(describe_identity(&id.reply, Some(id.requests)));
                    summary.extend(tally_lines(&tally, variax_control_name));
                    saved.push(("identity.syx".into(), id.reply));
                })
            }
            Command::VariaxLibrary => {
                session.log_packets = false;
                let progress = |stored: &variax::StoredPreset| {
                    let line = backup::variax_index_line(stored.preset, &stored.metadata);
                    let _ = writeln!(std::io::stdout(), "{line}");
                    progress_lines.push(line);
                };
                variax::read_library(&mut session, progress).map(|library| {
                    summary.push(describe_identity(&library.identified.reply, Some(library.identified.requests)));
                    summary.push(format!("Original preset: {}", library.original.name));
                    summary.push(format!("Presets read: {} of 60", library.presets.len()));
                    summary.push(format!("Bank names read: {} of 12", library.bank_names.len()));
                    if let Some(why) = &library.stopped {
                        summary.push(format!("Stopped early at {why}"));
                    }
                    summary.push(match &library.restore {
                        variax::Restore::Switched { to, verified: true } => format!(
                            "Switched back to {} {} (program {}); a read afterwards matched the original.",
                            to.bank_name(), to.slot() + 1, to.program_number()
                        ),
                        variax::Restore::Switched { to, verified: false } => format!(
                            "Switched back to {} {} (program {}), but the check read did not match. Move the pickup selector or model knob to reselect your preset.",
                            to.bank_name(), to.slot() + 1, to.program_number()
                        ),
                        variax::Restore::NoUniqueMatch { matches } => format!(
                            "Not switched back: {matches} stored presets match the original. Move the pickup selector or model knob to reselect your preset."
                        ),
                        variax::Restore::Failed(error) => format!(
                            "Switch back failed: {error}. Move the pickup selector or model knob to reselect your preset."
                        ),
                    });
                    if let Some(error) = &library.release_error {
                        summary.push(format!("CLEANUP FAILED: {error}"));
                    }
                    variax_library = Some(library);
                })
            }
            Command::PodxtRead
            | Command::PodxtListen(_)
            | Command::PodxtLibrary
            | Command::Compare(..)
            | Command::PodxtPacks
            | Command::PodxtReleaseVariaxControl
            | Command::PodxtLinkedVariaxModelTest
            | Command::PodxtLiveTest(_)
            | Command::PodxtChannel { .. }
            | Command::PodxtStatus
            | Command::PodxtTuner(_) => unreachable!(),
        };
        let cleanup = session.link.finish();
        session.note(match &cleanup {
            Ok(()) => "interface 1 released and given back to the kernel driver".to_owned(),
            Err(error) => format!("CLEANUP FAILED: {error}"),
        });
        (result, cleanup)
    };
    for line in progress_lines {
        capture.record(line);
    }
    for line in summary {
        capture.line(line);
    }
    for (name, bytes) in &saved {
        capture.file(name, bytes)?;
    }
    let sweep = if let Some(library) = &variax_library {
        backup::save_variax_library(capture, library)?;
        let problems: Vec<_> = [
            library.stopped.as_deref(),
            library.release_error.as_deref(),
            cleanup.as_ref().err().map(String::as_str),
        ]
        .into_iter()
        .flatten()
        .collect();
        if problems.is_empty() {
            Ok(())
        } else {
            Err(problems.join("\n"))
        }
    } else {
        cleanup
    };
    result.and(sweep)
}

fn run_podxt_listen(seconds: u64, capture: &mut Capture) -> Result<(), String> {
    let mut link = rawmidi::RawMidiLink::open(rawmidi::PODXT_LIVE_CARD, Path::PodxtMidi)?;
    let device = link.device.display().to_string();
    let result = {
        let mut session = Session::new(&mut link, &OwnerGrants, capture);
        session.note(format!("opened {device}"));
        podxt::listen(&mut session, Duration::from_secs(seconds))
    };
    let (identity_reply, tally) = result?;
    capture.line(describe_identity(&identity_reply, None));
    for line in tally_lines(&tally, podxt_control_name) {
        capture.line(line);
    }
    if !tally.programs.is_empty() {
        let channels: Vec<String> = tally
            .programs
            .iter()
            .map(|(_, program)| {
                podxt::Location::new(podxt::BankKind::Tones, u16::from(*program))
                    .map_or(format!("program {program}"), |l| l.label())
            })
            .collect();
        capture.line(format!(
            "Channel changes reported by the PODxt: {}",
            channels.join(", ")
        ));
    }
    capture.file("identity.syx", &identity_reply)
}

fn run_podxt_library(capture: &mut Capture) -> Result<(), String> {
    let mut link = rawmidi::RawMidiLink::open(rawmidi::PODXT_LIVE_CARD, Path::PodxtMidi)?;
    let device = link.device.display().to_string();
    let mut progress_lines = Vec::new();
    let result = {
        let mut session = Session::new(&mut link, &OwnerGrants, capture);
        session.note(format!("opened {device}"));
        session.log_packets = false;
        let progress = |stored: &podxt::StoredTone| {
            let line = backup::podxt_index_line(stored);
            let _ = writeln!(std::io::stdout(), "{line}");
            progress_lines.push(line);
        };
        podxt::read_library(&mut session, &podxt::BankKind::ALL, progress)
    };
    for line in progress_lines {
        capture.record(line);
    }
    let library = result?;
    capture.line(describe_identity(&library.identity_reply, None));
    let total: u16 = podxt::BankKind::ALL.iter().map(|k| k.size()).sum();
    capture.line(format!(
        "Stored entries read: {} of {total}",
        library.entries.len()
    ));
    if let Some(why) = &library.stopped {
        capture.line(format!("Stopped early at {why}"));
    }
    capture.line(format!("Current tone before: {}", library.before.name()));
    match &library.after {
        Ok(after) => {
            let changes = library.before.differences(after);
            if after.name() == library.before.name() && changes.is_empty() {
                capture.line("Current tone after: unchanged (all 168 bytes identical)".into());
            } else {
                capture.line(format!(
                    "Current tone after: {} — {} stored values differ (knob readings drift on their own):",
                    after.name(),
                    changes.len()
                ));
                for (cc, before, now) in changes {
                    let name = podxt::controls::name(cc).unwrap_or("unnamed");
                    capture.line(format!("  CC {cc} {name}: {before} -> {now}"));
                }
            }
        }
        Err(error) => capture.line(format!("Current tone after: not read ({error})")),
    }
    backup::save_podxt_library(capture, &library)?;
    match &library.stopped {
        Some(why) => Err(format!("library read stopped early at {why}")),
        None => Ok(()),
    }
}

/// Default live-edit set: four switches toggled and two knobs nudged.
const DEFAULT_LIVE: [&str; 6] = [
    "stomp=toggle",
    "mod=toggle",
    "delay=toggle",
    "reverb=toggle",
    "treble=+20",
    "mid=-20",
];

fn live_targets(words: &[&str]) -> Result<Vec<podxt::LiveTarget>, String> {
    if words == ["all"] {
        // Preserve the existing test set. Models and selectors need explicit values.
        return Ok(podxt::Control::ALL
            .iter()
            .map(|&control| {
                let target = if control.is_switch() {
                    podxt::Target::Toggle
                } else {
                    podxt::Target::Offset(20)
                };
                podxt::LiveTarget::Basic(control, target)
            })
            .collect());
    }
    let words: Vec<&str> = if words.is_empty() {
        DEFAULT_LIVE.to_vec()
    } else {
        words.to_vec()
    };
    let names: Vec<&str> = podxt::Control::ALL
        .iter()
        .map(|c| c.name())
        .chain(podxt::Setting::ALL.iter().map(|s| s.name()))
        .collect();
    words
        .iter()
        .map(|word| {
            let (name, value) = word
                .split_once('=')
                .ok_or(format!("{word}: use NAME=VALUE"))?;
            if let Some(setting) = podxt::Setting::from_name(name) {
                return setting
                    .parse(value)
                    .map(|value| podxt::LiveTarget::Additional(setting, value));
            }
            let control = podxt::Control::from_name(name).ok_or(format!(
                "{name}: not a live control (allowed: {})",
                names.join(", ")
            ))?;
            let target = match (control.is_switch(), value) {
                (_, "toggle") => podxt::Target::Toggle,
                (true, "pre") if control.is_routing() => podxt::Target::Off,
                (true, "post") if control.is_routing() => podxt::Target::On,
                (true, "on") => podxt::Target::On,
                (true, "off") => podxt::Target::Off,
                (false, v) if v.starts_with('+') || v.starts_with('-') => podxt::Target::Offset(
                    v.parse::<i8>()
                        .ok()
                        .filter(|n| n.unsigned_abs() <= 127)
                        .ok_or(format!("{word}: offset must be -127..=+127"))?,
                ),
                (false, v) => podxt::Target::Value(
                    v.parse::<u8>()
                        .ok()
                        .and_then(podxt::Value::new)
                        .ok_or(format!("{word}: value must be 0..=127"))?,
                ),
                (true, _) if control.is_routing() => {
                    return Err(format!("{word}: routing takes pre, post or toggle"));
                }
                (true, _) => return Err(format!("{word}: switches take on, off or toggle")),
            };
            Ok(podxt::LiveTarget::Basic(control, target))
        })
        .collect()
}

fn run_podxt_live_test(targets: &[podxt::LiveTarget], capture: &mut Capture) -> Result<(), String> {
    let mut link = rawmidi::RawMidiLink::open(rawmidi::PODXT_LIVE_CARD, Path::PodxtMidi)?;
    let device = link.device.display().to_string();
    let result = {
        let mut session = Session::new(&mut link, &OwnerGrants, capture);
        session.note(format!("opened {device}"));
        podxt::live_settings_test(&mut session, targets)
    };
    let test = result?;
    let cc_list = |items: &[(u8, u8, u8)]| {
        items
            .iter()
            .map(|(cc, a, b)| {
                format!(
                    "CC {cc} {}: {a} -> {b}",
                    podxt::controls::name(*cc).unwrap_or("unnamed")
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    capture.line(describe_identity(&test.identity_reply, None));
    capture.line(format!("Tone: {}", test.before.name()));
    capture.line(format!(
        "Changes sent: {} of {}",
        test.sent,
        test.changes.len()
    ));
    for (change, ok) in test.applied() {
        let now = test
            .after
            .as_ref()
            .map(|after| change.control.describe(change.control.read(after)))
            .unwrap_or_else(|_| "not read".into());
        let cc = change.control.cc();
        let controllers = match change.control.low_cc() {
            Some(low) => format!("CC {cc}+{low}"),
            None => format!("CC {cc}"),
        };
        capture.line(format!(
            "  {:<8} {controllers}: {} -> asked {} -> read {}  {}",
            change.control.name(),
            change.control.describe(change.original),
            change.control.describe(change.target),
            now,
            match (ok, change.control.device_may_override()) {
                (true, _) => "OK",
                (false, true) => "NOT APPLIED (the PODxt can override this from its own controls)",
                (false, false) => "NOT APPLIED",
            }
        ));
    }
    let unexpected = test.unexpected();
    capture.line(match &test.after {
        Err(error) => format!("After edits: {error}"),
        Ok(_) if unexpected.is_empty() => "Other settings after the changes: none moved".to_owned(),
        Ok(_) => format!(
            "Other settings that moved without being asked (device side effect or knob drift): {}",
            cc_list(&unexpected)
        ),
    });
    capture.line(match &test.undo_sent {
        Ok(()) => "Undo sent: original values for all attempted changes".to_owned(),
        Err(error) => format!("UNDO NOT FULLY SENT: {error}"),
    });
    let device_list = |items: &[(u8, u8)]| {
        items
            .iter()
            .map(|(cc, v)| {
                format!(
                    "CC {cc} {} = {v}",
                    podxt::controls::name(*cc).unwrap_or("unnamed")
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    if !test.device_sent_after_changes.is_empty() {
        capture.line(format!(
            "Sent by the PODxt on its own after the changes: {}",
            device_list(&test.device_sent_after_changes)
        ));
    }
    if !test.device_sent_after_undo.is_empty() {
        capture.line(format!(
            "Sent by the PODxt on its own after the undo: {}",
            device_list(&test.device_sent_after_undo)
        ));
    }
    let not_restored = test.not_restored();
    capture.line(match &test.restored {
        Ok(restored) if test.before.raw == restored.raw => {
            "After undo: all 168 bytes identical to before".to_owned()
        }
        Ok(_) if not_restored.is_empty() => {
            "After undo: controller bytes match, but other tone bytes changed".to_owned()
        }
        Ok(_) => format!(
            "After undo, still different from before: {}",
            cc_list(&not_restored)
        ),
        Err(error) => format!("After undo: not read ({error})"),
    });
    let additional = targets
        .iter()
        .any(|target| matches!(target, podxt::LiveTarget::Additional(..)));
    let routing_only = targets.iter().all(
        |target| matches!(target, podxt::LiveTarget::Basic(control, _) if control.is_routing()),
    );
    let room_only = matches!(targets, [podxt::LiveTarget::Basic(podxt::Control::Room, _)]);
    let strict = additional || routing_only || room_only;
    let undone = if strict {
        test.exact_undo()
    } else {
        test.undone()
    };
    capture.line(format!(
        "Result: changes {}; undo {}.",
        if test.stuck() {
            "ALL APPLIED"
        } else {
            "NOT all applied"
        },
        if undone {
            "confirmed"
        } else {
            "NOT confirmed (re-select the channel on the PODxt if you started from a saved tone)"
        }
    ));
    capture.file("identity.syx", &test.identity_reply)?;
    capture.file("before.syx", &test.before.raw)?;
    if let Ok(after) = &test.after {
        capture.file("after-changes.syx", &after.raw)?;
    }
    if let Ok(restored) = &test.restored {
        capture.file("after-undo.syx", &restored.raw)?;
    }
    if !undone {
        Err("undo not confirmed".into())
    } else if strict && !test.stuck() {
        Err("one or more requested changes were not confirmed".into())
    } else {
        Ok(())
    }
}

fn global_text(global: podxt::Global, raw: Option<u16>) -> String {
    let Some(raw) = raw else {
        return "no answer".into();
    };
    let decoded = match global {
        podxt::Global::CurrentChannel => podxt::Location::new(podxt::BankKind::Tones, raw)
            .map(|l| format!("channel {}", l.label())),
        podxt::Global::TunerNote => Some(format!("note {}", podxt::tuner_note_name(raw))),
        podxt::Global::TunerDeviation => Some(format!("{:+.0} cents", podxt::tuner_cents(raw))),
        podxt::Global::Version => Some(format!("{}.{:02}", raw >> 8, raw & 0xff)),
        podxt::Global::TunerMode => Some((if raw != 0 { "on" } else { "off" }).to_owned()),
        podxt::Global::Edited => Some((if raw != 0 { "yes" } else { "no" }).to_owned()),
        podxt::Global::TunerReference => Some(format!("A = {raw} Hz")),
        podxt::Global::MidiChannel => Some(format!("channel {}", raw + 1)),
        _ => None,
    };
    match decoded {
        Some(text) => format!("{text} (raw {raw})"),
        None => format!("raw {raw} ({})", podxt::signed(raw)),
    }
}

fn run_podxt_status(capture: &mut Capture) -> Result<(), String> {
    let mut link = rawmidi::RawMidiLink::open(rawmidi::PODXT_LIVE_CARD, Path::PodxtMidi)?;
    let device = link.device.display().to_string();
    let result = {
        let mut session = Session::new(&mut link, &OwnerGrants, capture);
        session.note(format!("opened {device}"));
        podxt::read_status(&mut session)
    };
    let (identity_reply, values) = result?;
    capture.line(describe_identity(&identity_reply, None));
    for (global, raw) in values {
        capture.line(format!(
            "  {:<32} {}",
            global.name(),
            global_text(global, raw)
        ));
    }
    capture.file("identity.syx", &identity_reply)
}

fn run_podxt_tuner(seconds: u64, capture: &mut Capture) -> Result<(), String> {
    let mut link = rawmidi::RawMidiLink::open(rawmidi::PODXT_LIVE_CARD, Path::PodxtMidi)?;
    let device = link.device.display().to_string();
    let mut lines = Vec::new();
    let result = {
        let mut session = Session::new(&mut link, &OwnerGrants, capture);
        session.note(format!("opened {device}"));
        session.note(format!("tuner for {seconds} s: play single notes"));
        session.log_packets = false;
        podxt::tuner(&mut session, Duration::from_secs(seconds), |reading| {
            let line = format!(
                "  {:6.2}s  {:<3} {}",
                reading.at.as_secs_f64(),
                reading.note.map_or("?", podxt::tuner_note_name),
                reading.deviation.map_or("?".into(), |d| format!(
                    "{:+4.0} cents",
                    podxt::tuner_cents(d)
                ))
            );
            let _ = writeln!(std::io::stdout(), "{line}");
            lines.push(line);
        })
    };
    for line in lines {
        capture.record(line);
    }
    let run = result?;
    capture.line(describe_identity(&run.identity_reply, None));
    capture.line(format!("Tuner readings: {}", run.readings.len()));
    capture.line(match &run.off_sent {
        Ok(()) => "Tuner switched off.".to_owned(),
        Err(error) => format!("TUNER OFF NOT SENT: {error} — press the tuner switch on the PODxt"),
    });
    capture.file("identity.syx", &run.identity_reply)?;
    run.off_sent
}

fn run_podxt_channel(
    target: podxt::Location,
    from: Option<podxt::Location>,
    back: bool,
    capture: &mut Capture,
) -> Result<(), String> {
    let mut link = rawmidi::RawMidiLink::open(rawmidi::PODXT_LIVE_CARD, Path::PodxtMidi)?;
    let device = link.device.display().to_string();
    let result = {
        let mut session = Session::new(&mut link, &OwnerGrants, capture);
        session.note(format!("opened {device}"));
        if from.is_none() {
            session.note("finding the current channel: reading the 128 stored tones (about 20 s)");
            session.log_packets = false;
        }
        podxt::switch_channel(&mut session, target, from, back)
    };
    let test = result?;
    let check = |ok: bool| if ok { "OK" } else { "DOES NOT MATCH" };
    capture.line(describe_identity(&test.identity_reply, None));
    capture.line(format!(
        "Playing before: {} (channel {})",
        test.before.name(),
        test.original.map_or("?".into(), |l| l.label())
    ));
    capture.line(format!(
        "Target: channel {} = stored \"{}\"",
        test.target.label(),
        test.target_stored.name()
    ));
    capture.line(match &test.after {
        Ok(after) => format!(
            "After the switch: playing \"{}\" — {}",
            after.name(),
            check(test.switched())
        ),
        Err(error) => format!("After the switch: not read ({error})"),
    });
    if let Some(back) = &test.back {
        capture.line(match back {
            Ok(tone) => format!(
                "After switching back: playing \"{}\" — {}",
                tone.name(),
                check(test.returned())
            ),
            Err(error) => format!(
                "After switching back: not read ({error}) — select your channel on the PODxt"
            ),
        });
    }
    capture.file("identity.syx", &test.identity_reply)?;
    capture.file("before.syx", &test.before.raw)?;
    capture.file("target-stored.syx", &test.target_stored.raw)?;
    if let Ok(after) = &test.after {
        capture.file("after-switch.syx", &after.raw)?;
    }
    if let Some(Ok(tone)) = &test.back {
        capture.file("after-back.syx", &tone.raw)?;
    }
    let ok = test.switched() && (!back || test.returned());
    if ok {
        Ok(())
    } else {
        Err("channel switch not confirmed".into())
    }
}

fn run_podxt_release_variax_control(capture: &mut Capture) -> Result<(), String> {
    let mut link = rawmidi::RawMidiLink::open(rawmidi::PODXT_LIVE_CARD, Path::PodxtMidi)?;
    let device = link.device.display().to_string();
    let reply = {
        let mut session = Session::new(&mut link, &OwnerGrants, capture);
        session.note(format!("opened {device}"));
        podxt::release_variax_control(&mut session)?
    };
    capture.line(describe_identity(&reply, None));
    capture.file("identity.syx", &reply)?;
    capture.line("Release sent. Check whether the last EDIT page now shows MODEL.".into());
    Ok(())
}

fn run_podxt_packs(capture: &mut Capture) -> Result<(), String> {
    let mut link = rawmidi::RawMidiLink::open(rawmidi::PODXT_LIVE_CARD, Path::PodxtMidi)?;
    let device = link.device.display().to_string();
    let result = {
        let mut session = Session::new(&mut link, &OwnerGrants, capture);
        session.note(format!("opened {device}"));
        podxt::read_packs(&mut session)
    };
    let (identity_reply, packs) = result?;
    capture.line(describe_identity(&identity_reply, None));
    let names = packs.names();
    capture.line(format!(
        "Installed model packs (mask {:#04x}): {}",
        packs.0,
        if names.is_empty() {
            "none".to_owned()
        } else {
            names.join(", ")
        }
    ));
    capture.file("identity.syx", &identity_reply)
}

fn run_podxt(capture: &mut Capture) -> Result<(), String> {
    let mut link = rawmidi::RawMidiLink::open(rawmidi::PODXT_LIVE_CARD, Path::PodxtMidi)?;
    let device = link.device.display().to_string();
    let result = {
        let mut session = Session::new(&mut link, &OwnerGrants, capture);
        session.note(format!("opened {device}"));
        podxt::read_current(&mut session)
    };
    let tone = result?;
    capture.line(describe_identity(&tone.identity_reply, None));
    for line in tone.tone.describe() {
        capture.line(line);
    }
    capture.file("identity.syx", &tone.identity_reply)?;
    capture.file("editbuffer.syx", &tone.tone.raw)
}

const LINKED_RECALL_LOCK: &str = "VARIAX_WORKBENCH_LINKED_RECALL_LOCK";

fn acquire_rig() -> Result<Option<line6_devices::device_lock::DeviceLock>, String> {
    use line6_devices::device_lock::DeviceLock;
    #[cfg(target_os = "linux")]
    if std::env::var_os(LINKED_RECALL_LOCK).is_some() {
        DeviceLock::verify_inherited_stdin().map_err(|error| error.to_string())?;
        return Ok(None);
    }
    DeviceLock::acquire()
        .map(Some)
        .map_err(|error| error.to_string())
}

#[cfg(target_os = "linux")]
fn run_locked_recall(script: &str) -> Result<i32, String> {
    let rig =
        line6_devices::device_lock::DeviceLock::acquire().map_err(|error| error.to_string())?;
    let stdin = rig.child_stdin().map_err(|error| error.to_string())?;
    let status = std::process::Command::new("bash")
        .arg(script)
        .stdin(stdin)
        .env(LINKED_RECALL_LOCK, "1")
        .status()
        .map_err(|error| error.to_string())?;
    Ok(status.code().unwrap_or(1))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    #[cfg(target_os = "linux")]
    if let [flag, script] = args.as_slice()
        && flag == "--lock-linked-recall"
    {
        let result = run_locked_recall(script);
        match result {
            Ok(status) => std::process::exit(status),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
    }
    let (command, out) = match parse(&args) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    if let Command::Compare(first, second) = &command {
        match backup::compare(first, second) {
            Ok(comparison) => {
                println!(
                    "{} stored entries identical, {} problems: {}",
                    comparison.identical,
                    comparison.problems.len(),
                    if comparison.verified() {
                        "VERIFIED (two independent reads match)"
                    } else {
                        "NOT verified"
                    }
                );
                for problem in &comparison.problems {
                    println!("  {problem}");
                }
                std::process::exit(if comparison.verified() { 0 } else { 1 });
            }
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
    }
    let library = matches!(command, Command::VariaxLibrary | Command::PodxtLibrary);
    let Some(out) = out else {
        eprintln!("--out DIR is required for device commands.\n{USAGE}");
        std::process::exit(2);
    };
    let _rig = match acquire_rig() {
        Ok(rig) => rig,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let (stamp, when) = backup::utc(seconds);
    let name = if library {
        format!("{}-{stamp}", command.slug())
    } else {
        format!("probe-{}-{stamp}", command.slug())
    };
    let mut capture = match Capture::create(&out, &name) {
        Ok(capture) => capture.with_echo(|text| {
            let _ = writeln!(std::io::stdout(), "{text}");
        }),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    capture.line(format!(
        "variax-probe {} at {when} — will send: {}",
        command.slug(),
        command.plan()
    ));
    let result = match command {
        Command::PodxtRead => run_podxt(&mut capture),
        Command::PodxtListen(seconds) => run_podxt_listen(seconds, &mut capture),
        Command::PodxtLibrary => run_podxt_library(&mut capture),
        Command::PodxtPacks => run_podxt_packs(&mut capture),
        Command::PodxtReleaseVariaxControl => run_podxt_release_variax_control(&mut capture),
        Command::PodxtLinkedVariaxModelTest => linked_variax::run(&mut capture),
        Command::PodxtLiveTest(targets) => run_podxt_live_test(&targets, &mut capture),
        Command::PodxtChannel { target, from, back } => {
            run_podxt_channel(target, from, back, &mut capture)
        }
        Command::PodxtStatus => run_podxt_status(&mut capture),
        Command::PodxtTuner(seconds) => run_podxt_tuner(seconds, &mut capture),
        _ => run_variax(&command, &mut capture),
    };
    if let Err(error) = &result {
        capture.line(format!("STOPPED: {error}"));
    }
    let saved = capture.finish(result.is_ok()).map(|dir| {
        let _ = writeln!(std::io::stdout(), "Capture: {}", dir.display());
    });
    if let Err(error) = result.and(saved) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(text: &str) -> Vec<String> {
        text.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn commands_parse_and_unknown_ones_do_not() {
        assert_eq!(
            parse(&args("podxt release-variax-control")).unwrap().0,
            Command::PodxtReleaseVariaxControl
        );
        assert!(parse(&args("podxt release-variax-control on")).is_err());
        assert_eq!(parse(&args("variax read")).unwrap().0, Command::VariaxRead);
        assert_eq!(
            parse(&args("variax listen 5")).unwrap().0,
            Command::VariaxListen(5)
        );
        assert_eq!(
            parse(&args("podxt read --out /x")).unwrap(),
            (Command::PodxtRead, Some(PathBuf::from("/x")))
        );
        assert_eq!(
            parse(&args("podxt listen 7")).unwrap().0,
            Command::PodxtListen(7)
        );
        assert_eq!(
            parse(&args("podxt listen")).unwrap().0,
            Command::PodxtListen(10)
        );
        assert_eq!(
            parse(&args("podxt library")).unwrap().0,
            Command::PodxtLibrary
        );
        assert_eq!(
            parse(&args("podxt stomp-test")).unwrap().0,
            Command::PodxtLiveTest(vec![podxt::LiveTarget::Basic(
                podxt::Control::StompEnable,
                podxt::Target::Toggle
            )])
        );
        match parse(&args("podxt live-test all")).unwrap().0 {
            Command::PodxtLiveTest(targets) => assert_eq!(targets.len(), podxt::Control::ALL.len()),
            other => panic!("{other:?}"),
        }
        match parse(&args("podxt live-test")).unwrap().0 {
            Command::PodxtLiveTest(targets) => assert_eq!(targets.len(), 6),
            other => panic!("{other:?}"),
        }
        match parse(&args("podxt live-test drive=100 mid=-20 delay=on"))
            .unwrap()
            .0
        {
            Command::PodxtLiveTest(targets) => assert_eq!(
                targets,
                vec![
                    podxt::LiveTarget::Basic(
                        podxt::Control::Drive,
                        podxt::Target::Value(podxt::Value::new(100).unwrap())
                    ),
                    podxt::LiveTarget::Basic(podxt::Control::Mid, podxt::Target::Offset(-20)),
                    podxt::LiveTarget::Basic(podxt::Control::DelayEnable, podxt::Target::On),
                ]
            ),
            other => panic!("{other:?}"),
        }
        for bad in [
            "podxt live-test amp=1",
            "podxt live-test drive=200",
            "podxt live-test delay=64",
            "podxt live-test mid=on",
            "podxt live-test drive",
        ] {
            assert!(parse(&args(bad)).is_err(), "{bad}");
        }
        assert_eq!(
            parse(&args("variax library")).unwrap().0,
            Command::VariaxLibrary
        );
        assert_eq!(
            parse(&args("compare /a /b")).unwrap().0,
            Command::Compare(PathBuf::from("/a"), PathBuf::from("/b"))
        );
        for bad in [
            "",
            "variax write",
            "podxt store",
            "variax listen 0",
            "variax listen 31",
            "podxt identity",
        ] {
            assert!(parse(&args(bad)).is_err(), "{bad}");
        }
    }

    /// Guitar-side encoding of 96 metadata bytes into one 200-byte reply.
    fn encode_reply(bytes: &[u8]) -> Vec<u8> {
        let mut reply = vec![0xf0, 0x00, 0x01, 0x0c, 0x07, 0x04, 0x3c];
        for word in bytes.chunks(3) {
            let value = u32::from_be_bytes([0, word[0], word[1], word[2]]);
            reply.extend((0..6).map(|i| ((value >> (4 * i)) & 15) as u8));
        }
        reply.push(0xf7);
        reply
    }

    #[test]
    fn live_test_accepts_item_two_values_and_pre_post_routing() {
        use podxt::{Control, LiveTarget, Setting, Target};

        assert_eq!(
            live_targets(&[
                "amp-model=0x00020020",
                "stomp-model=15",
                "gate-threshold=-48",
                "mic=2",
                "mod-note=4",
                "delay-note=8",
                "tweak-param=9",
                "pedal-assign=1",
                "mod-speed=4097",
                "delay-time=8193",
                "tempo=113.5",
                "vol-pedal-prepost=pre",
                "mod-prepost=post",
                "delay-prepost=pre",
            ])
            .unwrap(),
            [
                LiveTarget::Additional(Setting::AmpModel, 22),
                LiveTarget::Additional(Setting::StompModel, 15),
                LiveTarget::Additional(Setting::GateThreshold, 48),
                LiveTarget::Additional(Setting::Mic, 2),
                LiveTarget::Additional(Setting::ModNote, 4),
                LiveTarget::Additional(Setting::DelayNote, 8),
                LiveTarget::Additional(Setting::TweakParam, 9),
                LiveTarget::Additional(Setting::PedalAssign, 64),
                LiveTarget::Additional(Setting::ModSpeed, 4097),
                LiveTarget::Additional(Setting::DelayTime, 8193),
                LiveTarget::Additional(Setting::Tempo, 1135),
                LiveTarget::Basic(Control::VolumePedalPrePost, Target::Off),
                LiveTarget::Basic(Control::ModPrePost, Target::On),
                LiveTarget::Basic(Control::DelayPrePost, Target::Off),
            ]
        );
        for invalid in [
            "amp-model=107",
            "cab-model=0x00020020",
            "gate-threshold=1",
            "gate-threshold=-97",
            "mic=4",
            "mod-note=14",
            "delay-note=-1",
            "tweak-param=21",
            "pedal-assign=3",
            "mod-speed=16384",
            "delay-time=-1",
            "tempo=29.9",
            "tempo=240.1",
            "tempo=120.25",
            "delay=pre",
        ] {
            assert!(live_targets(&[invalid]).is_err(), "{invalid}");
        }
    }

    #[test]
    fn library_variax_decoder_matches_the_main_crate() {
        for seed in 0u32..64 {
            let mut raw = [0u8; 192];
            raw[..6].copy_from_slice(b"Seed  ");
            let mut state = seed.wrapping_mul(2_654_435_761).wrapping_add(1);
            for byte in &mut raw[18..] {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                *byte = state as u8;
            }
            let (first, second) = (encode_reply(&raw[..96]), encode_reply(&raw[96..]));
            let ours = variax::PresetMetadata::from_replies(&first, &second).unwrap();
            let main = variax_workbench::PresetMetadata::from_replies(&first, &second).unwrap();
            assert_eq!(ours.name, main.name);
            assert_eq!(ours.raw, main.raw);
            assert_eq!(
                format!("{:?}", ours.parameters),
                format!("{:?}", main.parameters),
                "seed {seed}"
            );
        }
    }
}
