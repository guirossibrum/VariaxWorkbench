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
//!
//! Only the requests defined by the `line6-devices` families can be sent, and
//! only those this file's policy allows. Every packet in and out is printed
//! and saved, with each reply, to a new folder under `--out` (default
//! `analysis/captures`). Nothing is overwritten.

#![forbid(unsafe_code)]

mod backup;

use backup::Capture;
use line6_devices::family::{Family, podxt, variax};
use line6_devices::gate::{GatePolicy, Operation};
use line6_devices::identity::Identity;
use line6_devices::midi::ControllerTally;
use line6_devices::session::{Log, Path, Session};
use line6_devices::transport::{rawmidi, usb};
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The owner's direct permissions for this tool (2026-11-22). Nothing else.
struct OwnerGrants;

impl GatePolicy for OwnerGrants {
    fn allows(&self, operation: Operation) -> bool {
        match operation {
            // G2: Variax 600 identity through the PODxt Live.
            Operation::VariaxIdentity => true,
            // G3: Variax current-preset metadata read (session on/off, two blocks).
            Operation::VariaxRead => true,
            // "test my podxtlive same way": PODxt identity and current-tone read.
            Operation::PodxtIdentity | Operation::PodxtRead => true,
            // G5 for the PODxt: read every stored tone/setup by location (no switching).
            Operation::PodxtLibraryRead => true,
            // G5 + preset switching for the Variax 600: program change through
            // each preset for the library read, then back to the original.
            Operation::VariaxPresetSwitch => true,
            // Owner's direct permission 2026-11-22: Line 6 Edit's read-only
            // installed model-pack query (`03 0E 00`).
            Operation::PodxtPackQuery => true,
        }
    }
}

const USAGE: &str = "Usage: variax-probe variax identity|read|listen [SECONDS]|library [--out DIR]
       variax-probe podxt read|listen [SECONDS]|library [--out DIR]
       variax-probe compare FIRST_BACKUP SECOND_BACKUP
Read-only. Run it yourself; it contacts the connected PODxt Live and Variax.
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
    let mut restore_note: Option<String> = None;
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
                    let line = preset_line(stored.preset, &stored.metadata);
                    let _ = writeln!(std::io::stdout(), "{line}");
                    progress_lines.push(line);
                };
                variax::read_library(&mut session, progress).map(|library| {
                    summary.push(describe_identity(&library.identified.reply, Some(library.identified.requests)));
                    summary.push(format!("Original preset: {}", library.original.name));
                    summary.push(format!("Presets read: {} of 60", library.presets.len()));
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
                    let mut index = String::new();
                    let [first, second] = library.original_replies;
                    saved.extend([("identity.syx".into(), library.identified.reply), ("original-1.syx".into(), first), ("original-2.syx".into(), second)]);
                    for stored in library.presets {
                        let stem = format!("presets/{:02}-{}-{}", stored.preset.bank(), stored.preset.bank_name().replace(' ', "_"), stored.preset.slot() + 1);
                        index.push_str(&preset_line(stored.preset, &stored.metadata));
                        index.push('\n');
                        let [first, second] = stored.replies;
                        saved.extend([(format!("{stem}-1.syx"), first), (format!("{stem}-2.syx"), second)]);
                    }
                    saved.push(("index.txt".into(), index.into_bytes()));
                    restore_note = Some(VARIAX_RESTORE.to_owned());
                })
            }
            Command::PodxtRead
            | Command::PodxtListen(_)
            | Command::PodxtLibrary
            | Command::Compare(..)
            | Command::PodxtPacks => unreachable!(),
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
    if let Some(note) = restore_note {
        capture.set_restore_note(note);
    }
    for line in summary {
        capture.line(line);
    }
    for (name, bytes) in &saved {
        capture.file(name, bytes)?;
    }
    result.and(cleanup)
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
    capture.file("identity.syx", &identity_reply)
}

const VARIAX_RESTORE: &str = "Variax 600 library backup (read-only copy, made by variax-probe).

presets/BB-Bank-S-1.syx and -2.syx are the exact 200-byte SysEx replies the guitar
sent for metadata blocks 1 (address 851) and 2 (address 883) of bank BB, slot S,
after a program change to that preset. index.txt lists bank, slot, Workbench
program number, preset name and body model. original-1/2.syx is the preset that
was active before the read. Decode with line6_devices::family::variax::
PresetMetadata::from_replies (or variax-workbench decode FIRST SECOND).

Check integrity:   sha256sum -c MANIFEST.sha256
Verified backup:   run `variax-probe variax library` again and
                   `variax-probe compare THIS_FOLDER OTHER_FOLDER`.

These blocks are current-preset metadata as Workbench reads it, not a DSP or
firmware image. Restoring them to the guitar needs a write gate (G6/G7), the
owner's direct permission and an approved restore procedure. Nothing in
variax-probe writes to a device.
";

const PODXT_RESTORE: &str = "PODxt Live library backup (read-only copy, made by variax-probe).

tones/NNN-LABEL.syx, amp-setups/NNN-N.syx and fx-setups/NNN-N.syx are the exact
168-byte SysEx dumps (F0 00 01 0C 03 74 .. F7) the PODxt sent when asked for each
stored location by address (Line 6 Edit 3.06 RequestPatch layout: name at bytes
7..22, controller n at byte 39+n). index.txt lists location, name and amp.
current-before.syx and current-after.syx are the active tone before and after.
Decode with line6_devices::family::podxt::EditBuffer.

Check integrity:   sha256sum -c MANIFEST.sha256
Verified backup:   run `variax-probe podxt library` again and
                   `variax-probe compare THIS_FOLDER OTHER_FOLDER`.

Restoring to the PODxt needs a write gate (G6/G7), the owner's direct
permission and an approved restore procedure. Nothing in variax-probe writes
to a device.
";

fn preset_line(preset: variax::Preset, metadata: &variax::PresetMetadata) -> String {
    let body = match metadata.parameters.get("body_model") {
        Some(variax::ParameterValue::Integer(id)) => format!("{id:#010x}"),
        _ => "?".into(),
    };
    format!(
        "{:>8} {} (program {:3}): {:<18} body {body}",
        preset.bank_name(),
        preset.slot() + 1,
        preset.program_number(),
        metadata.name
    )
}

fn tone_line(stored: &podxt::StoredTone) -> String {
    let tone = &stored.tone;
    format!(
        "{:<10} {:>4}: {:<16} amp {}",
        stored.location.kind().name(),
        stored.location.label(),
        tone.name(),
        tone.amp().name
    )
}

/// Controllers whose stored value differs between two edit-buffer reads.
fn edit_buffer_changes(before: &podxt::EditBuffer, after: &podxt::EditBuffer) -> Vec<String> {
    (0u8..=127)
        .filter(|&cc| {
            39 + usize::from(cc) < podxt::EDIT_BUFFER_LEN - 1 && before.cc(cc) != after.cc(cc)
        })
        .map(|cc| {
            let name = podxt::controls::name(cc).unwrap_or("unnamed");
            format!("CC {cc} {name}: {} -> {}", before.cc(cc), after.cc(cc))
        })
        .collect()
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
            let line = tone_line(stored);
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
            let changes = edit_buffer_changes(&library.before, after);
            if after.name() == library.before.name() && changes.is_empty() {
                capture.line("Current tone after: unchanged (all 168 bytes identical)".into());
            } else {
                capture.line(format!("Current tone after: {} — {} stored values differ (knob readings drift on their own):", after.name(), changes.len()));
                for change in changes {
                    capture.line(format!("  {change}"));
                }
            }
            capture.file("current-after.syx", &after.raw)?;
        }
        Err(error) => capture.line(format!("Current tone after: not read ({error})")),
    }
    capture.file("identity.syx", &library.identity_reply)?;
    capture.file("current-before.syx", &library.before.raw)?;
    let mut index = String::new();
    for stored in &library.entries {
        index.push_str(&tone_line(stored));
        index.push('\n');
        let name = format!(
            "{}/{:03}-{}.syx",
            stored.location.kind().slug(),
            stored.location.index() + 1,
            stored.location.label().replace('#', "")
        );
        capture.file(&name, &stored.tone.raw)?;
    }
    capture.file("index.txt", index.as_bytes())?;
    capture.set_restore_note(PODXT_RESTORE.to_owned());
    match &library.stopped {
        Some(why) => Err(format!("library read stopped early at {why}")),
        None => Ok(()),
    }
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

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (command, out) = match parse(&args) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    if let Command::Compare(first, second) = &command {
        match backup::compare(first, second) {
            Ok((lines, verified)) => {
                for line in lines {
                    println!("{line}");
                }
                std::process::exit(if verified { 0 } else { 1 });
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
        Ok(capture) => capture,
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
        _ => run_variax(&command, &mut capture),
    };
    let saved = capture.finish(&result).map(|_| ());
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
