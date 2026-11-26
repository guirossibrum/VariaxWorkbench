//! Attachment-scoped Variax cache capture. A native adapter must establish
//! guitar as well as relay continuity and observe a clean selected slot.

use super::{
    FullLibrary, ProgramCaptures, SnapshotCapture, SnapshotCoverage, save_variax_library,
    validate_snapshot,
};
use crate::{
    backup::BackupFolder,
    device_lock::DeviceLock,
    family::{
        podxt::check_live_identity,
        variax::{
            BANKS, KnownStart, Library, Preset, PresetMetadata, Restore, StoredPreset,
            procedures::check_identity, same_preset, sweep_library_from,
        },
    },
    gate::{GatePolicy, Operation},
    identity::Identity,
    session::{Link, Log, Path, Session},
};
use std::{
    path::{Component, Path as FilePath, PathBuf},
    time::{Duration, SystemTime},
};

mod sealed {
    pub trait Sealed {}
}

/// An attachment observation includes the generation of the guitar/relay
/// connection and both verified MIDI identities.
#[derive(PartialEq, Eq)]
pub struct AttachmentIdentity<G> {
    pub generation: G,
    pub guitar: Identity,
    pub relay: Identity,
}

/// A fresh observation of the actual selected slot and its stored metadata.
/// An adapter must establish that the entire playing buffer is a clean
/// recall of this slot on the current attachment.
pub struct CleanPosition {
    pub preset: Preset,
    pub stored_metadata: PresetMetadata,
}

/// Trusted attachment opener. There are currently no production implementations.
/// On error it must check cleanup of any partially opened handle under the lock.
///
/// ```compile_fail,E0277
/// use line6_devices::{
///     device_lock::DeviceLock,
///     family::variax::backup::{AttachmentIdentity, CaptureAttachment, CaptureLink},
/// };
/// struct CallerOpener<L>(L);
/// impl<L: CaptureLink<Generation = u64>> CaptureAttachment for CallerOpener<L> {
///     type Link = L;
///     fn open(self, _: &DeviceLock, _: &AttachmentIdentity<u64>) -> Result<L, String> {
///         Ok(self.0)
///     }
/// }
/// ```
pub trait CaptureAttachment: sealed::Sealed {
    type Link: CaptureLink;

    fn open(
        self,
        rig: &DeviceLock,
        selected: &AttachmentIdentity<<Self::Link as CaptureLink>::Generation>,
    ) -> Result<Self::Link, String>;
}

/// An exclusively held link that can check guitar/relay continuity and close
/// the very handle used for the sweep. Checks must fail closed when the
/// selected unit cannot be distinguished from a replacement. Adapters must
/// also prevent an I/O operation from being redirected after a check.
pub trait CaptureLink: Link + sealed::Sealed {
    type Generation: Eq;

    fn observe_attachment(&mut self) -> Result<AttachmentIdentity<Self::Generation>, String>;
    fn observe_clean_position(&mut self) -> Result<CleanPosition, String>;

    /// Consume the exact link, check continuity through release, and report
    /// close or driver-return failures. After replacement, release local
    /// resources without addressing the replacement and return an error.
    fn finish_capture(self, selected: &AttachmentIdentity<Self::Generation>) -> Result<(), String>;
}

/// Reserved by a trusted adapter when it verifies the selected attachment
/// under the owned rig lock. The profile is an opaque local choice.
pub struct SelectedCapture<A: CaptureAttachment> {
    opener: A,
    rig: DeviceLock,
    attachment: AttachmentIdentity<<A::Link as CaptureLink>::Generation>,
    profile: String,
}

enum CapturedLibrary {
    Metadata(Library),
    Full(FullLibrary),
}

impl CapturedLibrary {
    fn settings(&self) -> &Library {
        match self {
            Self::Metadata(library) => library,
            Self::Full(library) => &library.settings,
        }
    }

    fn check_complete(&self, start: &CleanPosition) -> Result<(), String> {
        let settings = self.settings();
        if let Some(error) = &settings.release_error {
            return Err(error.clone());
        }
        if let Some(error) = &settings.stopped {
            return Err(format!(
                "Incomplete Variax capture: {error} Return: {:?}.",
                settings.restore
            ));
        }
        if !settings
            .presets
            .iter()
            .map(|entry| entry.preset)
            .eq(Preset::all())
            || !settings
                .bank_names
                .iter()
                .map(|(bank, _)| *bank)
                .eq(0..BANKS.len() as u8)
            || matches!(self, Self::Full(full) if !full.verified_programs().map(|(preset, _)| preset).eq(Preset::all()))
        {
            return Err("The Variax capture has incomplete or invalid coverage.".into());
        }
        if let Self::Full(full) = self {
            // The two settings requests read words 851 through 914 of the
            // same active image. A valid checksum alone does not tie a full
            // program to the metadata pair read for that position.
            const METADATA_OFFSET: usize = 851 * 3;
            for (entry, (_, program)) in settings.presets.iter().zip(full.verified_programs()) {
                let raw = program[METADATA_OFFSET..][..192]
                    .try_into()
                    .expect("metadata range");
                let embedded = PresetMetadata::from_bytes(raw)?;
                if !same_preset(&entry.metadata, &embedded) {
                    return Err(format!(
                        "The full Variax program differs from the metadata at program {}.",
                        entry.preset.program_number()
                    ));
                }
            }
        }
        let mut matches = settings
            .presets
            .iter()
            .filter(|entry| same_preset(&entry.metadata, &settings.original));
        if matches.next().map(|entry| entry.preset) != Some(start.preset)
            || matches.next().is_some()
        {
            return Err("The starting Variax preset does not uniquely match the sweep.".into());
        }
        if settings.restore
            != (Restore::Switched {
                to: start.preset,
                verified: true,
            })
        {
            return Err(format!(
                "Variax return could not be verified: {:?}.",
                settings.restore
            ));
        }
        Ok(())
    }
}

/// The private sweep result travels with the verified tuple and capture
/// context. Callers can save it in v1 format and bind a validated snapshot.
pub struct FinalizedCapture {
    library: CapturedLibrary,
    guitar: Identity,
    relay: Identity,
    profile: String,
    coverage: SnapshotCoverage,
    captured_at: SystemTime,
}

impl FinalizedCapture {
    /// Reserve a new generation beneath an existing private root and write
    /// this sweep in v1 format. The token binds only the directory it created.
    pub fn stage(self, root: &FilePath, name: &str) -> Result<StagedCapture, String> {
        let mut components = FilePath::new(name).components();
        if !matches!(components.next(), Some(Component::Normal(part)) if part == name)
            || components.next().is_some()
        {
            return Err("Expected a single Variax snapshot directory name.".into());
        }
        let root = std::path::absolute(root).map_err(|error| error.to_string())?;
        let generation = root.join(name);
        std::fs::create_dir(&generation)
            .map_err(|error| format!("Could not reserve {}: {error}", generation.display()))?;
        let mut folder = BackupFolder::create(&generation, "library")?;
        match &self.library {
            CapturedLibrary::Metadata(library) => save_variax_library(&mut folder, library)?,
            CapturedLibrary::Full(library) => library.save(&mut folder)?,
        }
        let source = folder.finish(true)?;
        let report = validate_snapshot(&source, &self.guitar, self.coverage)?;
        Ok(StagedCapture {
            source,
            manifest_sha256: report.manifest_sha256().to_owned(),
            capture: self,
        })
    }
}

/// The source created from this capture, awaiting strict snapshot validation.
pub struct StagedCapture {
    source: PathBuf,
    manifest_sha256: String,
    capture: FinalizedCapture,
}

impl StagedCapture {
    /// Consume this stage and bind its exact source, wire replies, and programs
    /// to the manifest. The publisher must independently revalidate the source
    /// before copying and publishing the generation.
    pub fn bind(self) -> Result<CaptureReceipt, String> {
        let Self {
            source,
            manifest_sha256,
            capture,
        } = self;
        let report = validate_snapshot(&source, &capture.guitar, capture.coverage)?;
        report.check_capture_matches(match &capture.library {
            CapturedLibrary::Metadata(library) => SnapshotCapture::Metadata(library),
            CapturedLibrary::Full(library) => SnapshotCapture::Full(library),
        })?;
        if report.manifest_sha256() != manifest_sha256 {
            return Err("The Variax snapshot manifest changed after staging.".into());
        }
        Ok(CaptureReceipt {
            source,
            guitar: capture.guitar,
            relay: capture.relay,
            profile: capture.profile,
            coverage: capture.coverage,
            captured_at: capture.captured_at,
            manifest_sha256,
        })
    }
}

/// Evidence of one completed sweep. It confers no live-device permission.
///
/// ```compile_fail,E0599
/// use line6_devices::family::variax::{Library, backup::FinalizedCapture};
/// fn from_compatibility_backup(library: Library) -> FinalizedCapture {
///     FinalizedCapture::from(library)
/// }
/// ```
pub struct CaptureReceipt {
    source: PathBuf,
    guitar: Identity,
    relay: Identity,
    profile: String,
    coverage: SnapshotCoverage,
    captured_at: SystemTime,
    manifest_sha256: String,
}

impl CaptureReceipt {
    pub fn source(&self) -> &FilePath {
        &self.source
    }
    pub fn guitar(&self) -> &Identity {
        &self.guitar
    }
    pub fn relay(&self) -> &Identity {
        &self.relay
    }
    pub fn profile(&self) -> &str {
        &self.profile
    }
    pub fn coverage(&self) -> SnapshotCoverage {
        self.coverage
    }
    pub fn captured_at(&self) -> SystemTime {
        self.captured_at
    }
    pub fn manifest_sha256(&self) -> &str {
        &self.manifest_sha256
    }
}

/// Capture and return on the selected link under one owned rig lock. A
/// cancellation request stops the sweep while leaving return and cleanup
/// available on the original link.
pub fn capture_library<A: CaptureAttachment>(
    selected: SelectedCapture<A>,
    coverage: SnapshotCoverage,
    policy: &dyn GatePolicy,
    log: &mut dyn Log,
    cancelled: impl Fn() -> bool,
    progress: impl FnMut(&StoredPreset),
) -> Result<FinalizedCapture, String> {
    const CANCELLED: &str = "Variax library capture cancelled.";
    let SelectedCapture {
        opener,
        rig,
        attachment,
        profile,
    } = selected;
    check_identity(&attachment.guitar)?;
    check_live_identity(&attachment.relay)?;
    for operation in [
        Operation::VariaxIdentity,
        Operation::VariaxLibraryBridge,
        Operation::VariaxRead,
        Operation::VariaxPresetSwitch,
        Operation::VariaxBankNameRead,
    ] {
        if !policy.allows(operation) {
            return Err(format!("Variax capture requires {operation:?}."));
        }
    }
    if cancelled() {
        return Err(CANCELLED.into());
    }

    let captured_at = SystemTime::now();
    let mut link = CheckedLink {
        link: opener.open(&rig, &attachment)?,
        selected: &attachment,
        continuity_error: None,
    };
    let read = (|| {
        if link.path() != Path::VariaxViaPodxt {
            return Err("Variax capture requires the PODxt Live bridge.".into());
        }
        if cancelled() {
            return Err(CANCELLED.into());
        }
        let start = link.clean_position()?;
        let mut programs = ProgramCaptures::default();
        let settings = sweep_library_from(
            &mut Session::new(&mut link, policy, log),
            &cancelled,
            progress,
            (coverage == SnapshotCoverage::FullPrograms).then_some(&mut programs),
            Some(KnownStart {
                guitar: &attachment.guitar,
                preset: start.preset,
                stored_metadata: &start.stored_metadata,
            }),
        )?;
        let library = match coverage {
            SnapshotCoverage::Metadata => CapturedLibrary::Metadata(settings),
            SnapshotCoverage::FullPrograms => {
                CapturedLibrary::Full(FullLibrary { settings, programs })
            }
        };
        library.check_complete(&start)?;
        let returned = link.clean_position()?;
        if returned.preset != start.preset
            || !same_preset(&returned.stored_metadata, &start.stored_metadata)
        {
            return Err("The returned Variax position differs from the verified start.".into());
        }
        Ok(library)
    })();
    let library = match (read, link.finish()) {
        (Ok(library), Ok(())) => library,
        (Err(error), Ok(())) => return Err(error),
        (Ok(_), Err(error)) => return Err(format!("Variax capture link cleanup failed: {error}")),
        (Err(read), Err(finish)) => {
            return Err(format!(
                "{read} Variax capture link cleanup also failed: {finish}"
            ));
        }
    };
    drop(rig);
    if cancelled() {
        return Err(CANCELLED.into());
    }
    Ok(FinalizedCapture {
        library,
        profile,
        coverage,
        captured_at,
        guitar: attachment.guitar,
        relay: attachment.relay,
    })
}

struct CheckedLink<'a, L: CaptureLink> {
    link: L,
    selected: &'a AttachmentIdentity<L::Generation>,
    continuity_error: Option<String>,
}

impl<L: CaptureLink> CheckedLink<'_, L> {
    fn check(&mut self) -> Result<(), String> {
        if let Some(error) = &self.continuity_error {
            return Err(error.clone());
        }
        let result = self.link.observe_attachment().and_then(|actual| {
            if actual == *self.selected {
                Ok(())
            } else {
                Err("The selected Variax/PODxt attachment changed.".into())
            }
        });
        self.continuity_error = result.as_ref().err().cloned();
        result
    }

    fn clean_position(&mut self) -> Result<CleanPosition, String> {
        self.check()?;
        let position = self.link.observe_clean_position()?;
        self.check()?;
        Ok(position)
    }

    fn finish(mut self) -> Result<(), String> {
        let continuity = self.check();
        let release = self.link.finish_capture(self.selected);
        match (continuity, release) {
            (Err(error), Err(release)) => Err(format!("{error} {release}")),
            (continuity, release) => continuity.and(release),
        }
    }
}

impl<L: CaptureLink> Link for CheckedLink<'_, L> {
    fn path(&self) -> Path {
        self.link.path()
    }
    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.check()?;
        self.link.write(bytes)
    }
    fn read(&mut self, wait: Duration) -> Result<Option<Vec<u8>>, String> {
        self.check()?;
        let reply = self.link.read(wait)?;
        self.check()?;
        Ok(reply)
    }
    fn elapsed(&self) -> Duration {
        self.link.elapsed()
    }
    fn pause(&mut self, duration: Duration) {
        self.link.pause(duration)
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests;
