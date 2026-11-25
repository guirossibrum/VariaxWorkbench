//! Attachment-scoped adapter for the shared Workbench procedures.
use super::live_edit::Previous;
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

#[cfg(all(target_os = "linux", not(test)))]
mod linux {
    use super::*;
    use crate::editor::variax_panel::{
        PresetFile, live_attachment::Attachment, live_edit::EditBufferDevice,
    };
    use line6_devices::{
        device_lock::DeviceLock,
        gate::{GatePolicy, Operation},
        session::Quiet,
        transport::usb::UsbInterfaceLink,
    };
    use sha2::{Digest, Sha256};
    use std::{
        io::Read,
        path::{Path, PathBuf},
    };
    use variax_workbench::{
        PresetMetadata,
        hardware_gates::{Gate, is_locked, run_if_unlocked},
        host::{HostEngine, HostOptions, HostParameters, ModelLibrary},
        variax_library::LibraryPreferences,
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

    struct EditPolicy {
        writing: bool,
    }

    impl GatePolicy for EditPolicy {
        fn allows(&self, operation: Operation) -> bool {
            let gate = match operation {
                Operation::VariaxIdentity => Gate::IdentityRequest,
                Operation::VariaxRead => Gate::CurrentSettingsRead,
                Operation::VariaxEditBufferWrite if self.writing => Gate::EditBufferWrite,
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
        writing: bool,
        execute: impl FnOnce(&mut Session<'_, UsbInterfaceLink<&DeviceLock>>) -> Result<T, String>,
    ) -> Result<T, String> {
        for gate in [
            Gate::OsDiscovery,
            Gate::IdentityRequest,
            Gate::CurrentSettingsRead,
        ] {
            run_if_unlocked(gate, || Ok(()))?;
        }
        if writing {
            run_if_unlocked(Gate::EditBufferWrite, || Ok(()))?;
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
        let policy = EditPolicy { writing };
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

    fn require_reference(reference: &str, image: &Image) -> Result<(), String> {
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

    impl EditBufferDevice for EditorDevice {
        fn send(
            &self,
            attachment: &Attachment,
            model: &PresetFile,
            previous: Option<&Previous>,
            cancel: &AtomicBool,
        ) -> Result<Image, String> {
            check_cancel(cancel)?;
            if variax_workbench::math::PROFILE != "host-std-v1" {
                return Err(
                    "Variax live sends require the verified host-std-v1 math profile.".into(),
                );
            }
            let preferences = self
                .preferences
                .as_deref()
                .ok_or("Variax live sends require a local app data directory.")?;
            if LibraryPreferences::new(preferences)
                .latest_variax_library()
                .is_none()
            {
                return Err("Variax live sends require a verified library backup.".into());
            }
            let directory = preferences.with_file_name("variax-live-reference");
            let reference = open_reference(&directory.join("workbench.sha256"))?;
            let models = ModelLibrary::load_manifest(&directory.join("models.tsv"))?;
            let image = render(model, &models, false)?;
            require_reference(&reference, &image)?;
            let transition = if previous.is_some_and(|previous| previous.tuning_transition) {
                let muted = render(model, &models, true)?;
                require_reference(&reference, &muted)?;
                Some(muted)
            } else {
                None
            };
            session(attachment, cancel, true, |session| {
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
            session(attachment, cancel, false, |session| {
                let words = procedures::read_buffer(
                    session,
                    || cancel.load(Ordering::Relaxed),
                    |_| Ok(()),
                )?;
                let image = procedures::pack(&words);
                let metadata = PresetMetadata::from_bytes(
                    image[procedures::METADATA]
                        .try_into()
                        .expect("bounded metadata"),
                )?;
                let model = PresetFile::from_metadata(&metadata, 0x0007_0004, 0x0310_0000)?;
                Ok((model, image))
            })
        }
    }
}

#[cfg(all(target_os = "linux", not(test)))]
pub(super) use linux::EditorDevice;

#[cfg(test)]
mod tests;
