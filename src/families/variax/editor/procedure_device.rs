//! Attachment-scoped adapter for the shared Workbench procedures.
use super::{
    PresetFile,
    device_sync::{Report, Target},
    live_edit::Previous,
};
use line6_devices::{
    family::variax::{
        procedures::{self, BufferEdit, Image},
        write::Program,
    },
    session::{Link, Session},
};
use std::sync::atomic::{AtomicBool, Ordering};

fn send_on<L: Link>(
    session: &mut Session<L>,
    name: &str,
    image: &Image,
    transition: Option<&Image>,
    previous: Option<&Previous>,
    cancel: &AtomicBool,
) -> Result<Image, String> {
    let stopping = || cancel.load(Ordering::Relaxed);
    let next = Program::new(name, image)?;
    let muted = transition
        .map(|bytes| Program::new(name, bytes))
        .transpose()?;
    if let Some(previous) = previous {
        // Other rig users may have changed the buffer between lock holders.
        let actual = procedures::read_buffer(session, stopping, |_| Ok(()))?;
        procedures::compare_image(&actual, &previous.image, "Prior edit-buffer verification")?;
    }
    let edit = match (previous, muted.as_ref()) {
        (Some(previous), Some(transition)) if previous.tuning_transition => {
            BufferEdit::AlternateTuning {
                previous: &previous.image,
                transition,
                full: &next,
            }
        }
        (Some(previous), None) if !previous.tuning_transition => BufferEdit::Difference {
            previous: &previous.image,
            next: &next,
        },
        (None, None) => BufferEdit::Full(&next),
        _ => return Err("The alternate-tuning transition is incomplete.".into()),
    };
    procedures::send_buffer_verified(
        session,
        edit,
        "Editor working-buffer verification",
        stopping,
        || Ok(|_| Ok(())),
    )
}

fn decode_model(image: &Image) -> Result<PresetFile, String> {
    let metadata = variax_workbench::PresetMetadata::from_bytes(
        image[procedures::METADATA]
            .try_into()
            .expect("bounded metadata"),
    )?;
    PresetFile::from_metadata(&metadata, 0x0007_0004, 0x0310_0000)
}

struct StoreImages {
    candidate: Image,
    backup: Image,
}

fn sync_on<L: Link>(
    session: &mut Session<L>,
    targets: &[Target],
    stores: Option<&[StoreImages]>,
    cancel: &AtomicBool,
    mut notice: impl FnMut(String),
) -> Report {
    use procedures::VerifiedStoreError;
    let mut report = Report::default();
    let stopping = || cancel.load(Ordering::Relaxed);
    for (index, target) in targets.iter().enumerate() {
        if stopping() {
            report.error = Some("Cancelled remaining guitar library work.".into());
            break;
        }
        let slot = target.position.slot() + 1;
        let label = format!("{} {slot}", target.position.bank_name());
        notice(format!("Reading saved {label}…"));
        let result = procedures::read_unmodified(session, target.position, stopping, |_| Ok(()))
            .and_then(|words| {
                let Some(images) = stores.map(|images| &images[index]) else {
                    return decode_model(&procedures::pack(&words));
                };
                procedures::compare_image(
                    &words,
                    &images.backup,
                    "Saved position differs from the verified restore backup",
                )?;
                let program = Program::new(target.model.name(), &images.candidate)?;
                notice(format!("Storing and verifying {label}…"));
                match procedures::store_and_reload(
                    session,
                    target.position,
                    &program,
                    stopping,
                    |message| notice(message.to_owned()),
                    |_| Ok(()),
                ) {
                    Ok(()) => Ok(target.model.clone()),
                    Err(error) => {
                        report.store_unknown =
                            !matches!(error, VerifiedStoreError::BeforeCommit(_));
                        let warning = if report.store_unknown {
                            format!(" {}", procedures::spare_warning(target.position))
                        } else {
                            String::new()
                        };
                        Err(format!("{error}{warning}"))
                    }
                }
            });
        match result {
            Ok(model) => report.verified.push((
                usize::from(target.position.bank()),
                usize::from(slot - 1),
                model,
            )),
            Err(error) => {
                report.error = Some(format!("{label}: {error}"));
                break;
            }
        }
    }
    report
}

#[cfg(any(target_os = "linux", test))]
mod rendering {
    use line6_devices::family::variax::procedures::Image;
    use sha2::{Digest, Sha256};
    use std::{borrow::Cow, io::Read, path::Path};
    use variax_workbench::host::ModelLibrary;

    pub(super) struct Rendering {
        pub(super) models: Cow<'static, ModelLibrary>,
        reference: Option<String>,
    }

    impl Rendering {
        pub(super) fn load(preferences: &Path) -> Result<Self, String> {
            let directory = preferences.with_file_name("variax-live-reference");
            match std::fs::symlink_metadata(&directory) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self {
                    models: Cow::Borrowed(ModelLibrary::built_in()?),
                    reference: None,
                }),
                Err(error) => Err(format!("{}: {error}", directory.display())),
                Ok(_) => Ok(Self {
                    models: Cow::Owned(ModelLibrary::load_manifest(&directory.join("models.tsv"))?),
                    reference: Some(open_reference(&directory.join("workbench.sha256"))?),
                }),
            }
        }

        pub(super) fn verify(&self, image: &Image) -> Result<(), String> {
            let Some(reference) = &self.reference else {
                return Ok(());
            };
            let digest = format!("{:x}", Sha256::digest(image));
            if reference
                .lines()
                .filter_map(|line| line.split_whitespace().next())
                .any(|expected| expected.eq_ignore_ascii_case(&digest))
            {
                Ok(())
            } else {
                Err("The rendered Variax program has no matching Mac Workbench reference.".into())
            }
        }
    }

    fn open_reference(path: &Path) -> Result<String, String> {
        const LIMIT: u64 = 4 * 1024 * 1024;
        let facts = variax_workbench::path_facts(path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        if !facts.regular_file || facts.len > LIMIT {
            return Err("Expected a bounded Workbench program reference file.".into());
        }
        let mut text = String::new();
        variax_workbench::open_regular_file(path)
            .and_then(|file| file.take(LIMIT + 1).read_to_string(&mut text))
            .map_err(|error| format!("{}: {error}", path.display()))?;
        if text.len() as u64 > LIMIT {
            return Err("The Workbench program reference file is too large.".into());
        }
        Ok(text)
    }

    #[cfg(test)]
    #[test]
    fn bundled_default_and_strict_local_reference() {
        let tmp = tempfile::tempdir().unwrap();
        let preferences = tmp.path().join("preferences.toml");
        let source = Rendering::load(&preferences).unwrap();
        assert_eq!(source.models.bodies.len(), 28);
        source
            .verify(&[0; line6_devices::family::variax::procedures::PROGRAM_BYTES])
            .unwrap();

        let directory = tmp.path().join("variax-live-reference");
        std::fs::create_dir(&directory).unwrap();
        assert!(Rendering::load(&preferences).is_err());
        std::fs::write(directory.join("models.tsv"), "").unwrap();
        std::fs::write(directory.join("workbench.sha256"), "invalid").unwrap();
        let source = Rendering::load(&preferences).unwrap();
        assert!(source.models.bodies.is_empty());
        assert!(
            source
                .verify(&[0; line6_devices::family::variax::procedures::PROGRAM_BYTES])
                .is_err()
        );
    }
}

#[cfg(all(target_os = "linux", not(test)))]
mod linux {
    use super::{rendering::Rendering, *};
    use crate::editor::variax_panel::{
        PresetFile, device_sync::Action, live_attachment::Attachment, live_edit::EditBufferDevice,
    };
    use line6_devices::{
        device_lock::DeviceLock,
        gate::{GatePolicy, Operation},
        session::Quiet,
        transport::usb::UsbInterfaceLink,
    };
    use std::{
        io::Read,
        path::{Path, PathBuf},
        sync::mpsc::Sender,
    };
    use variax_workbench::{
        hardware_gates::{Gate, is_locked, run_if_unlocked},
        host::{HostEngine, HostOptions, HostParameters, ModelLibrary},
        variax_library::{CompletedVariaxBackup, LibraryPreferences},
    };

    pub(in crate::editor::variax_panel) struct EditorDevice {
        preferences: Option<PathBuf>,
    }

    impl EditorDevice {
        pub(in crate::editor::variax_panel) fn new(preferences: Option<&Path>) -> Self {
            Self {
                preferences: preferences.map(Path::to_owned),
            }
        }
    }

    #[derive(Clone, Copy)]
    enum Mode {
        BufferRead,
        BufferWrite,
        LibraryRead,
        LibraryWrite,
    }

    impl GatePolicy for Mode {
        fn allows(&self, operation: Operation) -> bool {
            let gate = match operation {
                Operation::VariaxIdentity => Gate::IdentityRequest,
                Operation::VariaxRead => Gate::CurrentSettingsRead,
                Operation::VariaxEditBufferWrite if matches!(self, Self::BufferWrite) => {
                    Gate::EditBufferWrite
                }
                Operation::VariaxPresetSwitch
                    if matches!(self, Self::LibraryRead | Self::LibraryWrite) =>
                {
                    Gate::FullBackupRead
                }
                Operation::VariaxStoreRestore if matches!(self, Self::LibraryWrite) => {
                    Gate::StoreRestore
                }
                _ => return false,
            };
            !is_locked(gate)
        }
    }

    fn check_cancel(cancel: &AtomicBool) -> Result<(), String> {
        if cancel.load(Ordering::Relaxed) {
            return Err("Variax edit-buffer operation cancelled.".into());
        }
        Ok(())
    }

    fn session<T>(
        attachment: &Attachment,
        cancel: &AtomicBool,
        policy: Mode,
        execute: impl FnOnce(&mut Session<'_, UsbInterfaceLink<&DeviceLock>>) -> Result<T, String>,
    ) -> Result<T, String> {
        for gate in [
            Gate::OsDiscovery,
            Gate::IdentityRequest,
            Gate::CurrentSettingsRead,
        ] {
            run_if_unlocked(gate, || Ok(()))?;
        }
        match policy {
            Mode::BufferWrite => run_if_unlocked(Gate::EditBufferWrite, || Ok(()))?,
            Mode::LibraryRead => run_if_unlocked(Gate::FullBackupRead, || Ok(()))?,
            Mode::LibraryWrite => {
                run_if_unlocked(Gate::FullBackupRead, || Ok(()))?;
                run_if_unlocked(Gate::StoreRestore, || Ok(()))?;
            }
            Mode::BufferRead => {}
        }
        check_cancel(cancel)?;
        // Variax tab admission already verified PODxt Live 3.01 for this USB
        // attachment. The scoped opener also checks the physical USB address.
        let rig = DeviceLock::acquire().map_err(|error| error.to_string())?;
        check_cancel(cancel)?;
        let link = variax_workbench::usb::open_variax_bridge_borrowed(
            &rig,
            (&attachment.path, attachment.address),
        )?;
        let mut log = Quiet;
        let mut session = Session::new(link, &policy, &mut log);
        session.log_packets = false;
        let result = check_cancel(cancel)
            .and_then(|()| procedures::identify(&mut session))
            .and_then(|_| check_cancel(cancel))
            .and_then(|()| execute(&mut session));
        match (result, session.link.finish()) {
            (Ok(value), Ok(())) => Ok(value),
            (Ok(_), Err(error)) | (Err(error), Ok(())) => Err(error),
            (Err(error), Err(cleanup)) => Err(format!("{error}. {cleanup}")),
        }
    }

    fn render(preset: &PresetFile, models: &ModelLibrary, muted: bool) -> Result<Image, String> {
        let parameters = HostParameters::from_preset(preset)?;
        let settings = parameters.map(HostOptions {
            muted,
            ..Default::default()
        })?;
        let generated = HostEngine::new(models).generate(&settings)?;
        Ok(parameters.finish(preset.name(), &settings, generated.packed()))
    }

    fn require_backup(preferences: &Path) -> Result<CompletedVariaxBackup, String> {
        LibraryPreferences::new(preferences)
            .latest_variax_library()
            .ok_or("Variax writes require a verified library backup.".into())
    }

    fn rendering(preferences: &Path) -> Result<Rendering, String> {
        if variax_workbench::math::PROFILE != "host-std-v1" {
            return Err("Variax writes require the verified host-std-v1 math profile.".into());
        }
        Rendering::load(preferences)
    }

    fn prepare_stores(
        preferences: Option<&Path>,
        imported_backup: Option<&Path>,
        targets: &[Target],
        cancel: &AtomicBool,
    ) -> Result<Vec<StoreImages>, String> {
        let preferences = preferences.ok_or("Variax stores require a local app data directory.")?;
        let backup = match imported_backup {
            Some(path) => CompletedVariaxBackup::import(path)?,
            None => require_backup(preferences)?,
        };
        if backup.identity.product_id != 0x0007_0004 || backup.identity.firmware != "3.10" {
            return Err(
                "Variax stores require a verified 3.10 backup for the supported guitar.".into(),
            );
        }
        let entries = line6_devices::backup::verified_manifest_entries(backup.folder())?;
        let source = rendering(preferences)?;
        targets
            .iter()
            .map(|target| {
                check_cancel(cancel)?;
                let p = target.position;
                let index = (p.bank() + 1) * 5 - 1 - p.slot();
                let name = format!(
                    "programs/{index:02}-{}-{}.bin",
                    p.bank_name().replace(' ', "_"),
                    p.slot() + 1
                );
                if !entries.contains(&name) {
                    return Err(format!(
                        "The verified backup must include the byte-level restore image {name}."
                    ));
                }
                let path = backup.folder().join(name);
                let mut bytes = Vec::new();
                variax_workbench::open_regular_file(&path)
                    .and_then(|file| {
                        file.take((procedures::PROGRAM_BYTES + 1) as u64)
                            .read_to_end(&mut bytes)
                    })
                    .map_err(|error| format!("{}: {error}", path.display()))?;
                let backup = Image::try_from(bytes)
                    .map_err(|_| format!("{} has an invalid program length.", path.display()))?;
                let candidate = render(&target.model, &source.models, false)?;
                source.verify(&candidate)?;
                Program::new(target.model.name(), &candidate)?;
                Ok(StoreImages { candidate, backup })
            })
            .collect()
    }

    pub(in crate::editor::variax_panel) fn sync(
        action: Action,
        attachment: &Attachment,
        (preferences, imported_backup): (Option<&Path>, Option<&Path>),
        targets: &[Target],
        cancel: &AtomicBool,
        status: &Sender<String>,
    ) -> Report {
        let mut report = Report::default();
        let result = (|| {
            if !action.permitted() {
                return Err(
                    "This Variax store path is closed pending its first owner test.".into(),
                );
            }
            let (mode, images) = if action.writes() {
                (
                    Mode::LibraryWrite,
                    Some(prepare_stores(
                        preferences,
                        imported_backup,
                        targets,
                        cancel,
                    )?),
                )
            } else {
                (Mode::LibraryRead, None)
            };
            session(attachment, cancel, mode, |session| {
                report = sync_on(session, targets, images.as_deref(), cancel, |message| {
                    let _ = status.send(message);
                });
                Ok(())
            })
        })();
        if let Err(error) = result {
            report.error = Some(match report.error.take() {
                Some(prior) => format!("{prior} {error}"),
                None => error,
            });
        }
        report
    }

    impl EditBufferDevice for EditorDevice {
        fn send(
            &self,
            attachment: &Attachment,
            model: &PresetFile,
            previous: Option<&Previous>,
            cancel: &AtomicBool,
        ) -> Result<Image, String> {
            check_cancel(cancel)?;
            let preferences = self
                .preferences
                .as_deref()
                .ok_or("Variax live sends require a local app data directory.")?;
            require_backup(preferences)?;
            let source = rendering(preferences)?;
            let image = render(model, &source.models, false)?;
            source.verify(&image)?;
            let transition = if previous.is_some_and(|previous| previous.tuning_transition) {
                let muted = render(model, &source.models, true)?;
                source.verify(&muted)?;
                Some(muted)
            } else {
                None
            };
            session(attachment, cancel, Mode::BufferWrite, |session| {
                send_on(
                    session,
                    model.name(),
                    &image,
                    transition.as_ref(),
                    previous,
                    cancel,
                )
            })
        }

        fn get(
            &self,
            attachment: &Attachment,
            cancel: &AtomicBool,
        ) -> Result<(PresetFile, Image), String> {
            session(attachment, cancel, Mode::BufferRead, |session| {
                let words = procedures::read_buffer(
                    session,
                    || cancel.load(Ordering::Relaxed),
                    |_| Ok(()),
                )?;
                let image = procedures::pack(&words);
                Ok((decode_model(&image)?, image))
            })
        }
    }
}

#[cfg(all(target_os = "linux", not(test)))]
pub(super) use linux::{EditorDevice, sync};

#[cfg(test)]
mod tests;
