//! Device-check library backups for the Variax and the PODxt Live.
//!
//! The Variax backup sends a program change for each stored preset, reads it,
//! then tries to switch back to the preset that was active. The PODxt Live
//! backup reads stored tones and setups by memory location and does not change
//! the current tone. Neither backup stores anything onto a device. A test
//! build returns before either link is opened.

#![cfg(feature = "usb")]

use std::path::Path;

use crate::hardware_gates::{self, Gate};

#[cfg(all(not(test), target_os = "linux"))]
use line6_devices::session::Log;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    Variax,
    Podxt,
}

/// Linux builds with the backup gate open. Tests may see this and must not
/// answer Yes against a real device.
pub fn offered() -> bool {
    cfg!(target_os = "linux") && !hardware_gates::is_locked(Gate::DeviceCheckBackup)
}

pub fn run(which: Which, root: &Path, progress: impl FnMut(&str)) -> Result<String, String> {
    hardware_gates::run_if_unlocked(Gate::DeviceCheckBackup, || {
        hardware_gates::run_if_unlocked(Gate::OsDiscovery, || {
            run_if_supported(which, root, progress)
        })
    })
}

fn run_if_supported(
    which: Which,
    root: &Path,
    progress: impl FnMut(&str),
) -> Result<String, String> {
    #[cfg(any(test, not(target_os = "linux")))]
    {
        let _ = (which, root, progress);
        Err("this build does not open a device for a backup; nothing was sent".into())
    }
    #[cfg(all(not(test), target_os = "linux"))]
    {
        let _rig =
            line6_devices::device_lock::DeviceLock::acquire().map_err(|error| error.to_string())?;
        run_live(which, root, progress)
    }
}

#[cfg(all(not(test), target_os = "linux"))]
fn stamp() -> (String, String) {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    line6_devices::backup::utc(seconds)
}

#[cfg(all(not(test), target_os = "linux"))]
fn run_live(which: Which, root: &Path, progress: impl FnMut(&str)) -> Result<String, String> {
    match which {
        Which::Variax => backup_variax(root, progress),
        Which::Podxt => backup_podxt(root, progress),
    }
}

#[cfg(all(not(test), target_os = "linux"))]
struct BackupPolicy;

#[cfg(all(not(test), target_os = "linux"))]
impl line6_devices::gate::GatePolicy for BackupPolicy {
    fn allows(&self, operation: line6_devices::gate::Operation) -> bool {
        if hardware_gates::is_locked(Gate::DeviceCheckBackup) {
            return false;
        }
        match operation {
            line6_devices::gate::Operation::VariaxIdentity => {
                !hardware_gates::is_locked(Gate::IdentityRequest)
            }
            line6_devices::gate::Operation::VariaxRead
            | line6_devices::gate::Operation::VariaxBankNameRead
            | line6_devices::gate::Operation::VariaxLibraryBridge
            | line6_devices::gate::Operation::VariaxPresetSwitch
            | line6_devices::gate::Operation::PodxtIdentity
            | line6_devices::gate::Operation::PodxtRead
            | line6_devices::gate::Operation::PodxtLibraryRead
            | line6_devices::gate::Operation::PodxtPackQuery => true,
            // Edits, channel and Variax control, globals and the tuner are not
            // part of a library backup; they stay closed in the app.
            line6_devices::gate::Operation::PodxtEditBufferWrite
            | line6_devices::gate::Operation::PodxtAdditionalEditBufferWrite
            | line6_devices::gate::Operation::PodxtKeepEditBufferChanges
            | line6_devices::gate::Operation::PodxtChannelSwitch
            | line6_devices::gate::Operation::PodxtVariaxControlRelease
            | line6_devices::gate::Operation::PodxtLinkedVariaxModelEdit
            | line6_devices::gate::Operation::PodxtGlobalRead
            | line6_devices::gate::Operation::PodxtTunerMode
            | line6_devices::gate::Operation::VariaxEditBufferWrite
            | line6_devices::gate::Operation::VariaxSlotRead
            | line6_devices::gate::Operation::VariaxStoreRestore => false,
        }
    }
}

/// Silence after the identity question, or a lost first packet that a second
/// question on that same open still did not complete. Preset reads are not
/// included: those already changed the guitar.
fn identity_miss(error: &str) -> bool {
    error.contains("nothing arrived from the guitar")
        || error.contains("No complete identity reply after the second request")
}

#[cfg(all(not(test), target_os = "linux"))]
fn close_folder(
    folder: line6_devices::backup::BackupFolder,
    complete: bool,
    summary: Result<String, String>,
) -> Result<String, String> {
    let path = folder.finish(complete).map_err(|error| match &summary {
        Ok(text) | Err(text) => format!("{text} The folder could not be closed: {error}"),
    })?;
    let text = match summary {
        Ok(text) | Err(text) => format!("{text} Saved in {}.", path.display()),
    };
    if complete { Ok(text) } else { Err(text) }
}

#[cfg(all(not(test), target_os = "linux"))]
fn backup_variax(root: &Path, mut progress: impl FnMut(&str)) -> Result<String, String> {
    use line6_devices::backup::{self, BackupFolder};
    use line6_devices::family::variax::transport::VARIAX_VIA_PODXT_LIVE;
    use line6_devices::transport::usb::UsbInterfaceLink;

    let (stamp, when) = stamp();
    let mut folder = BackupFolder::create(root, &format!("variax-library-{stamp}"))?;
    folder.line(format!(
        "Variax library backup at {when}. The guitar changes preset while this runs, then this tries to switch back. Nothing is stored."
    ));
    progress("Starting the Variax backup. The guitar will change preset.");
    let mut link = match UsbInterfaceLink::open(VARIAX_VIA_PODXT_LIVE) {
        Ok(link) => link,
        Err(error) => {
            folder.line(format!("STOPPED: {error}"));
            return close_folder(folder, false, Err(error));
        }
    };
    let mut lines = Vec::new();
    let policy = BackupPolicy;
    let (mut library, mut cleanup) =
        read_variax_on(&mut link, &policy, &mut folder, &mut lines, &mut progress);
    // Release this USB handle before any second open. finish() already gave
    // interface 1 back; drop closes the handle itself.
    drop(link);
    // A later fresh open is what completed this read when the first question
    // got silence. One more connection only, after the driver has been given back.
    if cleanup.is_ok()
        && let Err(why) = &library
        && identity_miss(why)
    {
        folder.line(format!(
            "The guitar did not answer ({why}). Giving the driver back and trying the connection once more."
        ));
        progress("The guitar did not answer. Trying the backup once more.");
        std::thread::sleep(std::time::Duration::from_secs(1));
        match UsbInterfaceLink::open(VARIAX_VIA_PODXT_LIVE) {
            Ok(mut again) => {
                let (library_again, cleanup_again) =
                    read_variax_on(&mut again, &policy, &mut folder, &mut lines, &mut progress);
                library = library_again;
                cleanup = cleanup_again;
            }
            Err(error) => {
                folder.line(format!("STOPPED: {error}"));
                library = Err(error);
            }
        }
    }
    for line in &lines {
        folder.record(line.clone());
    }
    let sweep_complete = library
        .as_ref()
        .is_ok_and(|library| library.stopped.is_none() && library.release_error.is_none());
    let summary = match library {
        Ok(library) => {
            let text = variax_summary(&library);
            match backup::save_variax_library(&mut folder, &library) {
                Ok(()) => Ok(text),
                Err(error) => Err(format!("{text} Saving the backup failed: {error}")),
            }
        }
        Err(error) => Err(format!("Variax backup stopped: {error}")),
    };
    let sweep_complete = sweep_complete && summary.is_ok();
    match &summary {
        Ok(text) => folder.line(text.clone()),
        Err(error) => folder.line(format!("STOPPED: {error}")),
    }
    let cleanup_ok = cleanup.is_ok();
    let summary = match cleanup {
        Ok(()) => summary,
        Err(error) => Err(match summary {
            Ok(text) | Err(text) => format!("{text} The guitar driver was not given back: {error}"),
        }),
    };
    close_folder(folder, sweep_complete && cleanup_ok, summary)
}

#[cfg(all(not(test), target_os = "linux"))]
fn variax_summary(library: &line6_devices::family::variax::Library) -> String {
    use line6_devices::family::variax;
    let mut text = format!(
        "Variax backup. Original preset {}. Presets read: {} of 60. Bank names read: {} of 12.",
        library.original.name,
        library.presets.len(),
        library.bank_names.len()
    );
    if let Some(why) = &library.stopped {
        text.push_str(&format!(" Stopped early at {why}."));
    }
    text.push(' ');
    text.push_str(&match &library.restore {
        variax::Restore::Switched { to, verified: true } => format!(
            "Switched back to {} {}. A read afterwards matched the original.",
            to.bank_name(),
            to.slot() + 1
        ),
        variax::Restore::Switched { to, verified: false } => format!(
            "Switched back to {} {}, but the check read did not match. Move the pickup selector or model knob to reselect your preset.",
            to.bank_name(),
            to.slot() + 1
        ),
        variax::Restore::NoUniqueMatch { matches } => format!(
            "Not switched back: {matches} stored presets match the original. Move the pickup selector or model knob to reselect your preset."
        ),
        variax::Restore::Failed(error) => format!(
            "Switch back failed: {error}. Move the pickup selector or model knob to reselect your preset."
        ),
    });
    if let Some(error) = &library.release_error {
        text.push_str(&format!(" Cleanup failed: {error}."));
    }
    text.push_str(" Nothing was stored.");
    text
}

#[cfg(all(not(test), target_os = "linux"))]
fn remember_preset(
    count: &mut u16,
    lines: &mut Vec<String>,
    progress: &mut impl FnMut(&str),
    stored: &line6_devices::family::variax::StoredPreset,
) {
    use line6_devices::backup;
    *count += 1;
    let line = format!(
        "Variax {count} of 60. {}",
        backup::variax_index_line(stored.preset, &stored.metadata)
    );
    lines.push(line.clone());
    progress(&line);
}

/// One open of interface 1. When the first identity question gets silence,
/// ask once more before giving the driver back. The shared identity check
/// still does not resend silence by itself.
#[cfg(all(not(test), target_os = "linux"))]
fn read_variax_on(
    link: &mut line6_devices::transport::usb::UsbInterfaceLink,
    policy: &BackupPolicy,
    folder: &mut line6_devices::backup::BackupFolder,
    lines: &mut Vec<String>,
    progress: &mut impl FnMut(&str),
) -> (
    Result<line6_devices::family::variax::Library, String>,
    Result<(), String>,
) {
    use line6_devices::family::variax;
    use line6_devices::session::Session;

    let mut session = Session::new(link, policy, folder);
    let (detached, claimed) = session.link.state();
    session.note(format!(
        "interface 1: kernel driver detached={detached}, claimed={claimed}, altsetting 1"
    ));
    session.log_packets = false;
    let mut count = 0u16;
    let first = variax::read_library(&mut session, |stored| {
        remember_preset(&mut count, lines, progress, stored);
    });
    let library = if first
        .as_ref()
        .is_err_and(|error| error.contains("nothing arrived from the guitar"))
    {
        session.note(
            "The guitar sent nothing on the first question. Asking once more on this connection.",
        );
        progress("The guitar did not answer. Asking again.");
        variax::read_library(&mut session, |stored| {
            remember_preset(&mut count, lines, progress, stored);
        })
    } else {
        first
    };
    let cleanup = session.link.finish();
    session.note(match &cleanup {
        Ok(()) => "interface 1 released and given back to the kernel driver".to_owned(),
        Err(error) => format!("CLEANUP FAILED: {error}"),
    });
    (library, cleanup)
}

#[cfg(all(not(test), target_os = "linux"))]
fn backup_podxt(root: &Path, mut progress: impl FnMut(&str)) -> Result<String, String> {
    use line6_devices::backup::{self, BackupFolder};
    use line6_devices::family::podxt;
    use line6_devices::session::Session;
    use line6_devices::{
        family::podxt::transport::PODXT_LIVE_MIDI, transport::rawmidi::RawMidiLink,
    };

    let (stamp, when) = stamp();
    let mut folder = BackupFolder::create(root, &format!("podxt-library-{stamp}"))?;
    folder.line(format!(
        "PODxt Live library backup at {when}. Stored tones and setups are read by location. The current tone is not changed on purpose. Nothing is stored."
    ));
    progress("Starting the PODxt Live backup. The current tone stays put.");
    let mut link = match RawMidiLink::open_spec(PODXT_LIVE_MIDI) {
        Ok(link) => link,
        Err(error) => {
            folder.line(format!("STOPPED: {error}"));
            return close_folder(folder, false, Err(error));
        }
    };
    let policy = BackupPolicy;
    let mut lines = Vec::new();
    let packs_note = {
        let mut session = Session::new(&mut link, &policy, &mut folder);
        match podxt::read_packs(&mut session) {
            Ok((_, packs)) => {
                let names = packs.names();
                let note = if names.is_empty() {
                    format!("Installed model packs: none (mask {:#04x}).", packs.0)
                } else {
                    format!(
                        "Installed model packs: {} (mask {:#04x}).",
                        names.join(", "),
                        packs.0
                    )
                };
                progress(&note);
                note
            }
            Err(error) => {
                let note = format!("Installed model packs were not read: {error}");
                progress(&note);
                note
            }
        }
    };
    folder.line(packs_note.clone());
    let library = {
        let mut session = Session::new(&mut link, &policy, &mut folder);
        session.log_packets = false;
        let total: u16 = podxt::BankKind::ALL.iter().map(|kind| kind.size()).sum();
        let mut count = 0u16;
        podxt::read_library(&mut session, &podxt::BankKind::ALL, |stored| {
            count += 1;
            let line = format!(
                "PODxt Live {count} of {total}. {}",
                backup::podxt_index_line(stored)
            );
            lines.push(line.clone());
            progress(&line);
        })
    };
    for line in &lines {
        folder.record(line.clone());
    }
    let summary = match library {
        Ok(library) => {
            if let Err(error) = backup::save_podxt_library(&mut folder, &library) {
                Err(error)
            } else {
                Ok(podxt_summary(&library, &packs_note))
            }
        }
        Err(error) => Err(format!("PODxt Live backup stopped: {error}")),
    };
    if let Err(error) = &summary {
        folder.line(format!("STOPPED: {error}"));
    }
    let complete = summary
        .as_ref()
        .is_ok_and(|text| !text.contains("Stopped early"));
    close_folder(folder, complete, summary)
}

#[cfg(all(not(test), target_os = "linux"))]
fn podxt_summary(library: &line6_devices::family::podxt::Library, packs_note: &str) -> String {
    use line6_devices::family::podxt;
    let total: u16 = podxt::BankKind::ALL.iter().map(|kind| kind.size()).sum();
    let mut text = format!(
        "PODxt Live backup. {packs_note} Current tone before: {}. Stored entries read: {} of {total}.",
        library.before.name(),
        library.entries.len()
    );
    if let Some(why) = &library.stopped {
        text.push_str(&format!(" Stopped early at {why}."));
    }
    text.push(' ');
    text.push_str(&match &library.after {
        Ok(after) => {
            let changes = library.before.differences(after);
            if after.name() == library.before.name() && changes.is_empty() {
                "Current tone after: unchanged.".to_owned()
            } else {
                format!(
                    "Current tone after: {}. {} values differ. Knob readings can drift on their own.",
                    after.name(),
                    changes.len()
                )
            }
        }
        Err(error) => format!("Current tone after: not read ({error})."),
    });
    text.push_str(" Nothing was stored.");
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_test_build_does_not_open_either_device() {
        let root = std::path::PathBuf::from("/tmp/variax-workbench-backup-must-not-be-created");
        for which in [Which::Variax, Which::Podxt] {
            let error = run(which, &root, |_| {}).unwrap_err();
            assert!(error.contains("nothing was sent"), "{error}");
            assert!(!root.exists());
        }
    }

    #[test]
    fn the_old_backup_lock_stays_on() {
        assert!(!hardware_gates::is_locked(Gate::FullBackupRead));
        assert!(!hardware_gates::is_locked(Gate::DeviceCheckBackup));
        assert!(!hardware_gates::is_locked(Gate::EditBufferWrite));
        assert!(!hardware_gates::is_locked(Gate::StoreRestore));
    }

    #[test]
    fn only_a_missed_identity_asks_again() {
        assert!(identity_miss(
            "No reply to the identity request: nothing arrived from the guitar."
        ));
        assert!(identity_miss(
            "No complete identity reply after the second request."
        ));
        assert!(!identity_miss(
            "The reply is not a Variax identity. Nothing else was sent."
        ));
        assert!(!identity_miss("No 200-byte metadata reply to Metadata1."));
        assert!(!identity_miss("USB write: timeout"));
    }
}
