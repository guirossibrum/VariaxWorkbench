//! What each step does once the person has chosen Run.

use super::catalogue::StepId;
use super::computer::{self, UsbView};
use super::evidence;
use super::practice::{Program, preset_label};
use super::procedures::{
    self, GUARD, PODXT_IDENTITY, PODXT_LIBRARY, PODXT_READ, VARIAX_IDENTITY, VARIAX_SELECT,
    program_bytes, same_program, variax_full_read, variax_read_preset, with_podxt, with_variax,
};
use super::record::Mode;
use super::runner::Ctx;
use line6_devices::family::podxt::{self, BankKind, EditBuffer, Location};
use line6_devices::family::variax::{self, Preset, procedures as shared_variax};
use line6_devices::identity::Identity;
use line6_devices::midi::hex;
use std::path::PathBuf;
use std::time::Duration;

/// Results that later steps build on.
#[derive(Default)]
pub struct RunData {
    pub computer: Vec<(String, String)>,
    pub devices: Vec<(String, String)>,
    /// Optional folder for backups and captures (never inside the report).
    pub evidence_dir: Option<PathBuf>,
    pub podxt_identity: Option<Identity>,
    pub podxt_verified: Option<bool>,
    pub variax_identity: Option<Identity>,
    pub variax_verified: Option<bool>,
    pub start_tone: Option<EditBuffer>,
    pub start_channel: Option<Location>,
    pub podxt_library: Vec<(Location, EditBuffer)>,
    /// Reads from steps 2.3 and 2.4, compared with the backup in 2.5.
    pub podxt_reads: Vec<(Location, Result<EditBuffer, String>)>,
    pub start_program: Option<Program>,
    /// The 192 settings bytes from step 3.2.
    pub start_settings: Option<[u8; 192]>,
    pub start_preset: Option<Preset>,
    pub variax_library: Vec<(Preset, Program)>,
    pub touched_podxt: bool,
    pub touched_variax: bool,
    pub store_state_unknown: bool,
    pub read_only: bool,
    /// The USB view from step 0.2.
    pub usb: Option<UsbView>,
    /// The 2.5 backup folder in the shared library's format (used by 5.2).
    pub podxt_backup_dir: Option<PathBuf>,
    /// Practice mode only: a temporary folder for the 2.5 backup when no
    /// evidence folder was chosen. Removed when the run ends.
    pub practice_scratch: Option<tempfile::TempDir>,
    /// A private folder of reference models for step 0.3 (optional).
    pub engine_folder: Option<PathBuf>,
    /// Step 0.3 results, for linking recording differences (#266).
    pub engine_summary: Vec<String>,
    /// Test models for step 4.3: built here and matching their reference
    /// in step 0.3 (name, program, optional A/B question).
    pub test_models: Vec<(String, Program, Option<String>)>,
}

impl RunData {
    /// Why a live or save step must not run on these devices, if so.
    pub fn unverified_reason(&self, step: StepId) -> Option<String> {
        let podxt_step = matches!(
            (step.0, step.1),
            (2, 6) | (4, 1 | 6..=8) | (5, 2) | (7, 3..=5)
        );
        if podxt_step {
            return (self.podxt_verified != Some(true)).then(|| {
                "Live and save steps run only on a verified PODxt Live with firmware 3.01. This device is flagged unverified: read-only steps only.".into()
            });
        }
        (self.variax_verified != Some(true)).then(|| {
            "Live and save steps run only on a verified Variax 300/600 with firmware 3.10. This device is flagged unverified: read-only steps only.".into()
        })
    }
}

fn touches(id: StepId) -> (bool, bool) {
    match (id.0, id.1) {
        (2, _) | (4, 1 | 6..=8) | (5, 2) | (7, 3..=5) => (true, false),
        (1, 2) | (3, _) | (4, _) | (5, _) | (6, 1) | (7, _) => (false, true),
        _ => (false, false),
    }
}

pub fn execute(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let id = ctx.step();
    let (podxt, variax) = touches(id);
    ctx.data.touched_podxt |= podxt;
    ctx.data.touched_variax |= variax;
    match (id.0, id.1) {
        (0, 1) => computer_system(ctx),
        (0, 2) => computer_usb(ctx),
        (0, 3) => engine_check(ctx),
        (1, 1) => super::connection::see_podxt(ctx),
        (1, 2) => super::connection::open_and_release(ctx),
        (2, 1) => podxt_identity(ctx),
        (2, 2) => podxt_current_tone(ctx),
        (2, 3) => super::podxt_reads::one_channel(ctx),
        (2, 4) => super::podxt_reads::one_bank(ctx),
        (2, 5) => podxt_library(ctx),
        (2, 6) => super::live::podxt_editor_get(ctx),
        (3, 1) => variax_identity(ctx),
        (3, 2) => super::variax_reads::current_settings(ctx),
        (3, 3) => variax_current_program(ctx),
        (3, 4) => variax_all_presets(ctx),
        (3, 5) => super::variax_reads::one_preset(ctx),
        (3, 6) => super::variax_reads::one_bank(ctx),
        (3, 7) => super::variax_evidence::read(ctx),
        (4, 1) => super::live::podxt_stomp(ctx),
        (4, 2) => super::variax_live::variax_own_bytes(ctx),
        (4, 3) => super::variax_live::variax_test_model(ctx),
        (4, 4) => super::variax_live::trim(ctx),
        (4, 5) => super::variax_live::tuning(ctx),
        (4, 6) => super::live::podxt_whole_tone(ctx),
        (4, 7) => super::live::podxt_recall(ctx),
        (4, 8) => super::live::podxt_tune(ctx),
        (5, 1) => super::variax_save::save_own_bytes(ctx),
        (5, 2) => super::podxt_save::save_own_bytes(ctx),
        (5, 3) => super::variax_save::edit_and_restore(ctx),
        (5, 4) => super::variax_save::sync_own_bytes(ctx),
        (6, 1) => super::recover::vdi_replug(ctx),
        (6, 2) => rig_guard(ctx),
        (7, 1) => super::guided::selector(ctx),
        (7, 2) => super::guided::knobs(ctx),
        (7, 3) => super::guided::footswitch(ctx),
        (7, 4) => super::guided::pedal(ctx),
        (7, 5) => super::guided::podxt_knob(ctx),
        (8, 1) => super::audio::detect(ctx),
        (8, _) => super::audio::record_or_play(ctx),
        _ => placeholder(ctx),
    }
}

fn placeholder(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let issue = match (ctx.step().0, ctx.step().1) {
        (0, 2) => "#250",
        (0, 3) => "#252",
        (1, _) => "#253",
        (2, _) => "#254",
        (3, _) => "#255",
        (4, 1) => "#256",
        (4, _) => "#257",
        (5, 1) => "#258",
        (5, _) => "#259",
        (6, _) => "#261",
        (7, _) => "#260",
        _ => "#264",
    };
    if ctx.mode == Mode::Real {
        return Err(format!(
            "This step's procedure is not in this build yet ({issue})."
        ));
    }
    ctx.rec.note(format!(
        "Practice placeholder: the procedure arrives with {issue}."
    ));
    Ok(())
}

fn computer_system(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let lines = if cfg!(target_os = "linux") || ctx.mode == Mode::Practice {
        let system = ctx.port.system();
        computer::linux::system(&*system)
    } else if cfg!(target_os = "macos") {
        computer::other::mac_system(&computer::other::SystemCommands)
    } else {
        computer::other::this_os(&computer::other::SystemCommands)
            .ok_or("No computer collector for this operating system.")?
            .into_iter()
            .filter(|(name, _)| !is_device_line(name))
            .collect()
    };
    if ctx.mode == Mode::Practice {
        ctx.rec
            .note("Practice mode: this is a simulated computer, not this one.");
    }
    for (name, value) in lines {
        ctx.rec.show(format!("{name}: {value}"));
        ctx.data.computer.push((name, value));
    }
    Ok(())
}

fn engine_check(ctx: &mut Ctx<'_>) -> Result<(), String> {
    use super::engine_check as engine;
    let builtin = engine::builtin();
    let mut results: Vec<engine::ModelResult> = builtin.to_vec();
    if let Some(folder) = ctx.data.engine_folder.clone() {
        match engine::local_folder(&folder) {
            Ok(local) => results.extend(local),
            Err(error) => ctx
                .rec
                .note(format!("Your reference folder could not be read: {error}")),
        }
    }
    let cases: Vec<&engine::ModelResult> = results
        .iter()
        .filter(|r| r.name.starts_with("Synthetic engine case"))
        .collect();
    let identical_cases = cases.iter().filter(|r| r.identical()).count();
    let mut summary = Vec::new();
    for result in results
        .iter()
        .filter(|r| !r.name.starts_with("Synthetic engine case") || !r.identical())
    {
        summary.push(result.summary());
    }
    summary.push(format!(
        "Synthetic engine cases 01-{:02}: {identical_cases} of {} identical",
        cases.len(),
        cases.len()
    ));
    for line in &summary {
        ctx.rec.show(line.clone());
    }
    for result in &results {
        if let Some(hash) = &result.actual_sha256 {
            ctx.rec
                .checksums
                .push((format!("{} (built here)", result.name), hash.clone()));
        }
        if !result.diffs.is_empty() {
            ctx.rec.note(format!("{} — word detail:", result.name));
            for line in result.details() {
                ctx.rec.note(line);
            }
        }
    }
    ctx.data.engine_summary = summary;
    ctx.data.test_models = results
        .iter()
        .filter(|r| r.identical())
        .filter_map(|r| {
            let bytes = r.program.as_ref()?;
            let words: Vec<u32> = bytes
                .chunks(3)
                .map(|c| u32::from_be_bytes([0, c[0], c[1], c[2]]))
                .collect();
            Some((r.name.clone(), words.try_into().ok()?, r.question.clone()))
        })
        .collect();
    let different = results.iter().filter(|r| !r.identical()).count();
    if different == 0 {
        Ok(())
    } else {
        Err(format!(
            "{different} reference program(s) differ on this computer. The word, bit and sound-part detail is in the report."
        ))
    }
}

/// Windows/macOS lines that belong to step 0.2 rather than 0.1.
fn is_device_line(name: &str) -> bool {
    ["USB", "driver", "MIDI"]
        .iter()
        .any(|word| name.contains(word))
}

fn computer_usb(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let view =
        crate::hardware_gates::run_if_unlocked(crate::hardware_gates::Gate::OsDiscovery, || {
            if cfg!(target_os = "linux") || ctx.mode == Mode::Practice {
                let system = ctx.port.system();
                Ok(computer::linux::usb(&*system, &ctx.port.libusb()))
            } else {
                let commands = &computer::other::SystemCommands;
                let lines = if cfg!(target_os = "macos") {
                    computer::other::mac_devices(commands)
                } else {
                    computer::other::this_os(commands)
                        .ok_or("No computer collector for this operating system.")?
                        .into_iter()
                        .filter(|(name, _)| is_device_line(name) || name == "Collector")
                        .collect()
                };
                Ok(UsbView {
                    devices: Vec::new(),
                    lines,
                })
            }
        })?;
    for (name, value) in &view.lines {
        ctx.rec.note(format!("{name}: {value}"));
        ctx.data.computer.push((name.clone(), value.clone()));
    }
    if cfg!(target_os = "linux") || ctx.mode == Mode::Practice {
        ctx.rec
            .result
            .push(format!("USB devices: {}", view.devices.len()));
        for device in &view.devices {
            ctx.rec.result.push(format!(
                "Port {}: {}",
                device.port,
                device.name.as_deref().unwrap_or("Unnamed USB device")
            ));
        }
    } else {
        ctx.rec.result.extend(
            view.lines
                .iter()
                .map(|(name, value)| format!("{name}: {value}")),
        );
    }
    ctx.data.usb = Some(view);
    Ok(())
}

pub fn variax_verified(identity: &Identity) -> bool {
    shared_variax::check_identity(identity).is_ok()
}

fn podxt_identity(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let (reply, _) = with_podxt(ctx, PODXT_IDENTITY, |session| {
        podxt::listen(session, Duration::ZERO)
    })?;
    let identity = Identity::parse(&reply).ok_or("The PODxt identity reply did not parse.")?;
    let verified = podxt::check_live_identity(&identity).is_ok();
    let model = podxt::member_name(identity.member);
    ctx.rec.note(format!("Identity reply: {}", hex(&reply)));
    ctx.rec.show(format!(
        "{model}, firmware {}{}",
        identity.version_text(),
        if verified {
            " (verified device)"
        } else {
            " (unverified device: read-only steps only)"
        }
    ));
    ctx.data.devices.push((
        "PODxt".into(),
        format!(
            "{model}, firmware {}, {}",
            identity.version_text(),
            if verified { "verified" } else { "unverified" }
        ),
    ));
    ctx.data.podxt_identity = Some(identity);
    ctx.data.podxt_verified = Some(verified);
    Ok(())
}

fn podxt_current_tone(ctx: &mut Ctx<'_>) -> Result<(), String> {
    // A closure, not the function path: the session's link lifetime is generic.
    #[allow(clippy::redundant_closure)]
    let current = with_podxt(ctx, PODXT_READ, |session| podxt::read_current(session))?;
    let tone = current.tone;
    ctx.rec.show(format!("Current tone: {}", tone.name()));
    for line in tone.describe() {
        ctx.rec.note(format!("  {line}"));
    }
    let hash = ctx
        .rec
        .checksum("PODxt current tone (168 bytes)", &tone.raw);
    ctx.rec
        .note(format!("Dump SHA-256 {hash}: {}", hex(&tone.raw)));
    ctx.data.start_tone = Some(tone);
    Ok(())
}

fn podxt_library(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let library = with_podxt(ctx, PODXT_LIBRARY, |session| {
        session.log_packets = false;
        podxt::read_library(session, &BankKind::ALL, |_| {})
    })?;
    ctx.rec
        .show(format!("Read {} of 299 entries.", library.entries.len()));
    if let Some(reason) = &library.stopped {
        return Err(format!(
            "The library read stopped after {} of 299 entries: {reason}",
            library.entries.len()
        ));
    }
    let mut all = Vec::new();
    for entry in &library.entries {
        all.extend_from_slice(&entry.tone.raw);
    }
    let hash = ctx.rec.checksum("PODxt library (299 dumps)", &all);
    ctx.rec.note(format!("Library SHA-256 {hash}"));
    match &library.after {
        Ok(after) if after.raw == library.before.raw => ctx
            .rec
            .show("The playing tone is unchanged after the library read."),
        Ok(_) => return Err("The playing tone changed during the library read.".into()),
        Err(error) => return Err(format!("The closing current-tone read failed: {error}")),
    }
    // The backup in the shared library's own format, so step 5.2 can verify
    // it with the same code as the editor. Practice mode without an evidence
    // folder uses a private temporary folder.
    let root = match ctx.data.evidence_dir.clone() {
        Some(root) => Some(root),
        None if ctx.mode == Mode::Practice => {
            let scratch = tempfile::tempdir().map_err(|e| e.to_string())?;
            let path = scratch.path().to_owned();
            ctx.data.practice_scratch = Some(scratch);
            Some(path)
        }
        None => None,
    };
    if let Some(root) = root {
        use line6_devices::backup::BackupFolder;
        let mut folder = BackupFolder::create(&root, "podxt-library")?;
        folder.record(format!(
            "PODxt Live library backup at {} (Rig Check step 2.5)",
            super::record::utc_now()
        ));
        podxt::backup::save_podxt_library(&mut folder, &library)?;
        let path = folder.finish(true)?;
        let manifest = std::fs::read(path.join("MANIFEST.sha256")).map_err(|e| e.to_string())?;
        ctx.rec.note(format!(
            "Saved the 299-entry backup to {} (manifest SHA-256 {}).",
            if ctx.data.evidence_dir.is_some() {
                "the evidence folder podxt-library"
            } else {
                "a temporary practice folder"
            },
            &super::record::sha256_hex(&manifest)[..16]
        ));
        ctx.data.podxt_backup_dir = Some(path);
    }
    ctx.data.podxt_library = library
        .entries
        .iter()
        .map(|entry| (entry.location, entry.tone.clone()))
        .collect();
    let content = super::podxt_reads::compare_with_backup(ctx);
    if content > 0 {
        return Err(format!(
            "{content} earlier channel read(s) differ in content from the backup."
        ));
    }
    if let Some(start) = &ctx.data.start_tone {
        let matches: Vec<Location> = ctx
            .data
            .podxt_library
            .iter()
            .filter(|(location, tone)| location.kind() == BankKind::Tones && tone.same_tone(start))
            .map(|(location, _)| *location)
            .collect();
        match matches[..] {
            [channel] => {
                ctx.rec
                    .show(format!("Starting channel: {}", channel.label()));
                ctx.data.start_channel = Some(channel);
            }
            _ => ctx.rec.note(format!(
                "The playing tone matches {} stored channels; the starting channel is unknown.",
                matches.len()
            )),
        }
    }
    Ok(())
}

fn variax_identity(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let identified = with_variax(ctx, VARIAX_IDENTITY, |session, _| {
        shared_variax::identify_reported(session)
    })?;
    let identity = identified.identity;
    if !identity.is_line6() || identity.family != variax::IDENTITY_FAMILY {
        return Err("The reply is not a Variax identity.".into());
    }
    let verified = variax_verified(&identity);
    let model = match identity.member {
        0 => "Variax 500/700 (the identity does not tell them apart)",
        4 => "Variax 300/600 (the identity does not tell them apart)",
        _ => "Variax (unknown member)",
    };
    ctx.rec
        .note(format!("Identity reply: {}", hex(&identified.reply)));
    ctx.rec.note(format!(
        "Identity requests sent: {} (a second one means the first reply packet was lost or the first try was silent).",
        identified.requests
    ));
    ctx.rec.show(format!(
        "{model}, firmware {}{}",
        identity.version_text(),
        if verified {
            " (verified device)"
        } else if super::variax_evidence::applicable(Some(&identity)) {
            " (unverified firmware: evidence mode available)"
        } else {
            " (unverified device: read-only steps only)"
        }
    ));
    ctx.rec
        .show("The VDI link answered through the PODxt Live.");
    ctx.data.devices.push((
        "Variax".into(),
        format!(
            "{model}, firmware {}, {}",
            identity.version_text(),
            if verified { "verified" } else { "unverified" }
        ),
    ));
    ctx.data.variax_identity = Some(identity);
    ctx.data.variax_verified = Some(verified);
    Ok(())
}

fn variax_current_program(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let read = with_variax(ctx, VARIAX_SELECT, |session, stopping| {
        variax_full_read(session, stopping)
    })?;
    let image = shared_variax::pack(&read.words);
    let mut checked = image;
    shared_variax::set_firmware_checksum(&mut checked);
    if image != checked {
        return Err("The Variax current program failed the firmware checksum check.".into());
    }
    ctx.rec.show(format!(
        "Progress {} of 35 chunks, 1,092 words, checksum OK.",
        read.raw_replies.len()
    ));
    ctx.rec.note(format!(
        "Read {} chunks, 1,092 words. Retries: {}.",
        read.raw_replies.len(),
        ctx.rec.retries
    ));
    let hash = ctx.rec.checksum(
        "Variax current program (1,092 words)",
        &program_bytes(&read.words),
    );
    ctx.rec.note(format!(
        "Program SHA-256 {hash}, name {:?}, checksum word 0x{:06X}",
        super::practice::program_name(&read.words),
        read.words[1091]
    ));
    super::variax_reads::after_full_read(ctx, &read.words);
    ctx.data.start_program = Some(read.words);
    Ok(())
}

fn variax_all_presets(ctx: &mut Ctx<'_>) -> Result<(), String> {
    super::variax_reads::flag_unverified(ctx);
    let start = ctx.data.start_program.ok_or("No starting program (3.3).")?;
    use line6_devices::gate::Operation;
    const BACKUP: &[Operation] = &[
        Operation::VariaxIdentity,
        Operation::VariaxRead,
        Operation::VariaxPresetSwitch,
        Operation::VariaxLibraryBridge,
        Operation::VariaxBankNameRead,
    ];
    let read = with_variax(ctx, BACKUP, |session, stopping| {
        session.log_packets = false;
        variax::backup::read_full_library_in_session(session, stopping, |_| {})
    })?;
    let library: Vec<(Preset, Program)> = read
        .verified_programs()
        .map(|(preset, bytes)| {
            (
                preset,
                std::array::from_fn(|index| {
                    let [a, b, c] = bytes.as_chunks::<3>().0[index];
                    u32::from_be_bytes([0, a, b, c])
                }),
            )
        })
        .collect();
    for (bank, name) in &read.settings.bank_names {
        ctx.rec.note(format!("Bank {}: {:?}", bank + 1, name.name));
    }
    let failure = read
        .settings
        .stopped
        .clone()
        .or(read.settings.release_error.clone());
    if !read.complete() || read.settings.bank_names.len() != 12 {
        ctx.rec.note(format!(
            "Partial backup: {} programs and {} bank names.",
            library.len(),
            read.settings.bank_names.len()
        ));
    }
    ctx.rec
        .show(format!("Read {} of 60 presets.", library.len()));
    for (preset, words) in &library {
        let hash = ctx.rec.checksum(
            format!("Variax {} (1,092 words)", preset_label(*preset)),
            &program_bytes(words),
        );
        ctx.rec.note(format!(
            "{}: SHA-256 {}",
            preset_label(*preset),
            &hash[..16]
        ));
    }
    let matches: Vec<Preset> = library
        .iter()
        .filter(|(_, words)| same_program(words, &start))
        .map(|(preset, _)| *preset)
        .collect();
    let back = match matches[..] {
        [preset] => {
            ctx.rec
                .show(format!("Starting preset: {}", preset_label(preset)));
            ctx.data.start_preset = Some(preset);
            let read = with_variax(ctx, VARIAX_SELECT, |session, stopping| {
                variax_read_preset(session, preset, stopping)
            })?;
            if same_program(&read.words, &start) {
                ctx.rec.show(format!(
                    "Back on {}: all 1,092 words match the start.",
                    preset_label(preset)
                ));
                Ok(())
            } else {
                Err(format!(
                    "Switched back to {}, but the program differs from the start.",
                    preset_label(preset)
                ))
            }
        }
        _ => Err(format!(
            "The starting program matches {} stored presets; Rig Check cannot switch back. Select your preset with the guitar's selector.",
            matches.len()
        )),
    };
    if let Some(root) = ctx.data.evidence_dir.clone() {
        let files: Vec<(String, Vec<u8>)> = library
            .iter()
            .map(|(preset, words)| {
                (
                    format!(
                        "{:02}-{}.bin",
                        super::practice::flash_index(*preset),
                        evidence::file_name(&preset_label(*preset))
                    ),
                    program_bytes(words),
                )
            })
            .collect();
        let manifest = evidence::save_folder(&root, "variax-library", &files)?;
        ctx.rec.note(format!(
            "Saved {} programs to the evidence folder variax-library (manifest SHA-256 {manifest}).",
            files.len()
        ));
    }
    ctx.data.variax_library = library;
    if let Some(failure) = failure {
        return Err(match back {
            Ok(()) => format!("{failure} The guitar is back on its starting preset."),
            Err(error) => format!("{failure} {error}"),
        });
    }
    back
}

fn rig_guard(ctx: &mut Ctx<'_>) -> Result<(), String> {
    if !ctx.data.touched_podxt && !ctx.data.touched_variax && !ctx.port.variax_claimed() {
        ctx.rec
            .show("Rig: no device step ran; the devices were not touched.");
        return Ok(());
    }
    let mut problems = Vec::new();
    if let Some(start) = ctx.data.start_tone.clone() {
        let now = with_podxt(ctx, GUARD, |session| {
            podxt::read_current(session).map(|current| current.tone)
        })?;
        if now.raw == start.raw {
            ctx.rec
                .show("Rig: the PODxt Live tone is the starting tone (all 168 bytes).");
        } else if now.same_tone(&start) {
            ctx.rec.show(format!(
                "Rig: the PODxt Live tone is the starting tone; only live pedal/knob values differ {:?}.",
                start.differences(&now)
            ));
        } else if let Some(channel) = ctx.data.start_channel.filter(|_| !ctx.data.read_only) {
            let switch = with_podxt(ctx, GUARD, |session| {
                podxt::switch_channel(session, channel, None, false)
            })?;
            if switch.switched() && switch.target_stored.same_tone(&start) {
                ctx.rec.show(format!(
                    "Rig: the PODxt Live was switched back to channel {} and verified.",
                    channel.label()
                ));
            } else {
                problems.push(format!(
                    "The PODxt Live could not be verified on channel {}.",
                    channel.label()
                ));
            }
        } else {
            problems.push(
                "The PODxt Live tone differs from the start and the starting channel is unknown."
                    .to_string(),
            );
        }
    } else if ctx.data.touched_podxt {
        ctx.rec
            .show("Rig: the PODxt Live starting tone was not recorded (2.2 did not pass).");
    }
    if let Some(start) = ctx.data.start_program {
        let now = with_variax(ctx, VARIAX_SELECT, |session, stopping| {
            // Let anything the guitar sent meanwhile (a selector turn after
            // Stop) arrive and be logged before the read starts.
            procedures::drain(session)?;
            variax_full_read(session, stopping)
        })?;
        if same_program(&now.words, &start) {
            ctx.rec
                .show("Rig: the Variax plays the starting program (1,092 words).");
        } else if let Some(preset) = ctx.data.start_preset.filter(|_| !ctx.data.read_only) {
            let back = with_variax(ctx, VARIAX_SELECT, |session, stopping| {
                variax_read_preset(session, preset, stopping)
            })?;
            if same_program(&back.words, &start) {
                ctx.rec.show(format!(
                    "Rig: the Variax was switched back to {} and verified.",
                    preset_label(preset)
                ));
            } else {
                problems.push(format!(
                    "The Variax could not be verified on {}.",
                    preset_label(preset)
                ));
            }
        } else {
            problems.push(
                "The Variax program differs from the start and the starting preset is unknown."
                    .into(),
            );
        }
    } else if ctx.data.touched_variax {
        ctx.rec
            .show("Rig: the Variax starting program was not recorded (3.3 did not pass).");
    }
    if ctx.port.variax_claimed() {
        match ctx.port.release_variax() {
            Ok(lines) => {
                for line in lines {
                    ctx.rec.show(format!("Rig: {line}"));
                }
            }
            Err(error) => problems.push(error),
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        for problem in &problems {
            ctx.rec.note(format!("Rig: {problem}"));
        }
        Err(problems.join(" "))
    }
}
