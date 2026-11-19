#![forbid(unsafe_code)]
//! One in-memory rig session.
//!
//! `initial_rig` describes the Variax 600 and PODxt Live roles. `extension`
//! names an unverified future profile. Neither constructor opens a device.

use std::path::Path;

use super::backend;
use super::capability::{self, Capability, CapabilityState, Operation};
use super::model::{self, Component, ProductProfile, RoleKind, VersionKnowledge};
use super::registry::{INITIAL_HOST, PhysicalHost};
use crate::pod_preset::PodModelEnable;

/// No function constructs a live transport. This stays `None`.
pub const LIVE_BACKEND_INITIALIZER: Option<fn() -> Session> = None;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LiveBackend {
    Absent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SessionKind {
    InitialRig,
    Extension(ProductProfile),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OfflineRecordingGate {
    pub route: crate::pod_audio::RecordRoute,
    pub stream_opened: bool,
    pub outbound_len: usize,
    pub capture_stays_locked: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Session {
    kind: SessionKind,
    live: LiveBackend,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuitarPresetSummary {
    pub name: String,
    pub device_id: u32,
    pub models: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PodToneSummary {
    pub name: String,
    pub device_id: u32,
    pub device_version: u32,
    pub container: &'static str,
    pub amp_model_id: Option<u32>,
    pub cab_model_id: Option<u32>,
    pub amp_name: Option<&'static str>,
    pub cab_name: Option<&'static str>,
    pub undecoded_slots: usize,
    pub recorded_parameter_chunks: usize,
    /// PARM chunks that are not the 12-byte record.
    pub undecoded_parameter_chunks: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GuitarBankSummary {
    pub banks: usize,
    pub presets: usize,
    pub fixed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FirmwareSummary {
    pub product_id: u32,
    pub version: u32,
    pub applicability: &'static str,
    pub integrity_is_not_authentication: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rejection {
    Locked {
        operation: Operation,
    },
    Planned {
        operation: Operation,
        issue: u16,
    },
    Unverified {
        profile: ProductProfile,
        operation: Operation,
    },
    Unsupported {
        profile: ProductProfile,
        operation: Operation,
    },
    CrossFamily {
        from: super::model::DeviceFamily,
        to: super::model::DeviceFamily,
        operation: Operation,
    },
    DifferentModel {
        profile: ProductProfile,
        device_id: u32,
    },
    WrongProduct {
        declared: u32,
        expected: u32,
    },
    WrongVersion {
        declared: u32,
        expected: u32,
    },
    Format {
        detail: String,
    },
    NotPhysical {
        operation: Operation,
        state: CapabilityState,
    },
    ProfileIsPartOfInitialRig {
        profile: ProductProfile,
    },
}

impl Rejection {
    pub(crate) fn format(detail: impl std::fmt::Display) -> Self {
        Self::Format {
            detail: detail.to_string(),
        }
    }
}

impl Session {
    /// The Variax 600 through PODxt Live rig. This does not scan USB.
    pub const fn initial_rig() -> Self {
        Self {
            kind: SessionKind::InitialRig,
            live: LiveBackend::Absent,
        }
    }

    /// An unverified future profile. Initial-rig profiles stay on [`Self::initial_rig`].
    pub const fn extension(profile: ProductProfile) -> Result<Self, Rejection> {
        if profile.is_extension() {
            Ok(Self {
                kind: SessionKind::Extension(profile),
                live: LiveBackend::Absent,
            })
        } else {
            Err(Rejection::ProfileIsPartOfInitialRig { profile })
        }
    }

    pub const fn live_backend_is_absent(self) -> bool {
        matches!(self.live, LiveBackend::Absent)
    }

    pub const fn host(self) -> Option<&'static PhysicalHost> {
        match self.kind {
            SessionKind::InitialRig => Some(&INITIAL_HOST),
            SessionKind::Extension(_) => None,
        }
    }

    pub fn capability(self, role: RoleKind, operation: Operation) -> Result<Capability, Rejection> {
        let profile = self.initial_profile(role, operation)?;
        Ok(capability::capability(profile, operation))
    }

    pub fn version(
        self,
        role: RoleKind,
        component: Component,
    ) -> Result<VersionKnowledge, Rejection> {
        let profile = self.initial_profile(role, Operation::InspectFirmware)?;
        Ok(model::version(profile, component))
    }

    /// Driver-text identity for the POD HD500X extension only.
    ///
    /// Workbench HD, JTV-69, and the initial rig do not inherit these ids.
    pub fn pod_hd500x(
        self,
    ) -> Result<&'static crate::pod_hd500x::Record, crate::pod_hd500x::ProfileMiss> {
        match self.kind {
            SessionKind::Extension(ProductProfile::PodHd500x) => Ok(crate::pod_hd500x::record()),
            SessionKind::Extension(_) => Err(crate::pod_hd500x::ProfileMiss::DifferentExtension),
            SessionKind::InitialRig => Err(crate::pod_hd500x::ProfileMiss::InitialRig),
        }
    }

    pub fn extension_version(self, component: Component) -> Result<VersionKnowledge, Rejection> {
        match self.kind {
            SessionKind::Extension(profile) => Ok(model::version(profile, component)),
            SessionKind::InitialRig => Err(Rejection::ProfileIsPartOfInitialRig {
                profile: ProductProfile::Variax600,
            }),
        }
    }

    pub fn open_preset(
        self,
        role: RoleKind,
        bytes: &[u8],
    ) -> Result<GuitarPresetSummary, Rejection> {
        self.require(role, Operation::OpenGuitarPreset)?;
        backend::open_guitar_preset(bytes)
    }

    pub fn open_guitar_bank(self, bytes: &[u8]) -> Result<GuitarBankSummary, Rejection> {
        self.require(RoleKind::Guitar, Operation::OpenGuitarBank)?;
        backend::open_guitar_bank(bytes)
    }

    pub fn save_guitar_copy(self, path: &Path, bytes: &[u8]) -> Result<(), Rejection> {
        self.require(RoleKind::Guitar, Operation::SaveGuitarCopy)?;
        backend::save_guitar_copy(path, bytes)
    }

    /// Opens one PODxt Live tone file in memory. The summary carries amp and
    /// cab ids, asset names when the PODxt Live table has them, and the
    /// parameter-record counts. Parameter ranges are not decoded.
    pub fn open_pod_preset(
        self,
        role: RoleKind,
        bytes: &[u8],
    ) -> Result<PodToneSummary, Rejection> {
        self.require(role, Operation::OpenPodPreset)?;
        backend::open_pod_preset(bytes)
    }

    /// Renames the PINF name and returns the updated file bytes. Nothing is sent.
    pub fn rename_pod_preset(
        self,
        role: RoleKind,
        bytes: &[u8],
        name: &str,
    ) -> Result<Vec<u8>, Rejection> {
        self.require(role, Operation::EditPodPreset)?;
        backend::rename_pod_preset(bytes, name)
    }

    /// Changes one source-backed stored PODxt model-enable flag in memory.
    /// This is an offline file edit; it never sends the value to a device.
    pub fn set_pod_model_enabled(
        self,
        role: RoleKind,
        bytes: &[u8],
        control: PodModelEnable,
        enabled: bool,
    ) -> Result<Vec<u8>, Rejection> {
        self.require(role, Operation::EditPodPreset)?;
        backend::set_pod_model_enabled(bytes, control, enabled)
    }

    /// Applies one admitted AMP model-id choice to an existing FORM/L6PA tone.
    /// This explicit offline edit changes only its MINF model-id word.
    pub fn set_pod_amp_model_choice(
        self,
        role: RoleKind,
        bytes: &[u8],
        choice: crate::pod_preset::PodAmpModelChoice,
    ) -> Result<Vec<u8>, Rejection> {
        self.require(role, Operation::EditPodPreset)?;
        backend::set_pod_amp_model_choice(bytes, choice)
    }

    pub fn set_pod_cab_model_choice(
        self,
        role: RoleKind,
        bytes: &[u8],
        choice: crate::pod_preset::PodCabModelChoice,
    ) -> Result<Vec<u8>, Rejection> {
        self.require(role, Operation::EditPodPreset)?;
        backend::set_pod_cab_model_choice(bytes, choice)
    }

    pub fn set_pod_mic_choice(
        self,
        role: RoleKind,
        bytes: &[u8],
        choice: crate::pod_preset::PodMicChoice,
    ) -> Result<Vec<u8>, Rejection> {
        self.require(role, Operation::EditPodPreset)?;
        backend::set_pod_mic_choice(bytes, choice)
    }

    /// Applies one typed, source-backed Stomp harmony choice offline.
    /// The returned tone bytes retain all unrelated data; nothing is sent.
    pub fn set_pod_stomp_harmony_selection(
        self,
        role: RoleKind,
        bytes: &[u8],
        selection: crate::pod_preset::StompHarmonySelection,
    ) -> Result<Vec<u8>, Rejection> {
        self.require(role, Operation::EditPodPreset)?;
        backend::set_pod_stomp_harmony_selection(bytes, selection)
    }

    /// Applies one typed Delay NOTE choice offline. Positive choices derive
    /// TIME from stored Tempo; Off changes NOTE only. Nothing is sent.
    pub fn set_pod_delay_note_choice(
        self,
        role: RoleKind,
        bytes: &[u8],
        choice: crate::pod_preset::DelayNoteChoice,
    ) -> Result<Vec<u8>, Rejection> {
        self.require(role, Operation::EditPodPreset)?;
        backend::set_pod_delay_note_choice(bytes, choice)
    }

    pub fn set_pod_mod_note_choice(
        self,
        role: RoleKind,
        bytes: &[u8],
        choice: crate::pod_preset::ModNoteChoice,
    ) -> Result<Vec<u8>, Rejection> {
        self.require(role, Operation::EditPodPreset)?;
        backend::set_pod_mod_note_choice(bytes, choice)
    }

    pub fn set_pod_mod_speed_hz(
        self,
        role: RoleKind,
        bytes: &[u8],
        hz: f32,
    ) -> Result<Vec<u8>, Rejection> {
        self.require(role, Operation::EditPodPreset)?;
        backend::set_pod_mod_speed_hz(bytes, hz)
    }

    /// Applies a displayed Delay TIME value in milliseconds offline and clears
    /// a positive Delay NOTE. This isolated edit does not read Tempo or Mod.
    pub fn set_pod_delay_time_ms(
        self,
        role: RoleKind,
        bytes: &[u8],
        milliseconds: f32,
    ) -> Result<Vec<u8>, Rejection> {
        self.require(role, Operation::EditPodPreset)?;
        backend::set_pod_delay_time_ms(bytes, milliseconds)
    }

    /// Writes a new copy through the non-overwriting save. It does not upload.
    pub fn save_pod_copy(self, role: RoleKind, path: &Path, bytes: &[u8]) -> Result<(), Rejection> {
        self.require(role, Operation::EditPodPreset)?;
        backend::save_pod_copy(path, bytes)
    }

    pub fn inspect_firmware(
        self,
        role: RoleKind,
        bytes: &[u8],
    ) -> Result<FirmwareSummary, Rejection> {
        let profile = self.require(role, Operation::InspectFirmware)?;
        backend::inspect_firmware(profile, bytes)
    }

    pub fn inspect_extension_firmware(self, _bytes: &[u8]) -> Result<FirmwareSummary, Rejection> {
        let profile = self.extension_profile(Operation::InspectFirmware)?;
        self.require_profile(profile, Operation::InspectFirmware)?;
        Err(Rejection::Unverified {
            profile,
            operation: Operation::InspectFirmware,
        })
    }

    /// Plans one documented PODxt Live record route. The returned plan keeps
    /// `stream_opened` false and carries no outbound bytes.
    pub fn select_record_route(
        self,
        route: crate::pod_audio::RecordRoute,
        period_bytes: u32,
        periods: u32,
    ) -> Result<crate::pod_audio::RoutePlan, Rejection> {
        self.require(RoleKind::Pod, Operation::SelectRecordRoute)?;
        crate::pod_audio::plan(route, period_bytes, periods, || {}).map_err(Rejection::format)
    }

    /// Arms the in-memory record-route check used by an offline test take.
    ///
    /// `attempt_capture` is the physical capture closure. It is not called.
    /// The returned gate does not open a stream and carries no outbound bytes.
    pub fn gate_offline_recording<F>(
        self,
        route: crate::pod_audio::RecordRoute,
        period_bytes: u32,
        periods: u32,
        attempt_capture: F,
    ) -> Result<OfflineRecordingGate, Rejection>
    where
        F: FnOnce() -> Result<(), Rejection>,
    {
        self.require(RoleKind::Pod, Operation::SelectRecordRoute)?;
        match self.reject_physical(
            RoleKind::Pod,
            Operation::StartAudioCapture,
            Some(CapabilityState::Offline),
            attempt_capture,
        ) {
            Err(Rejection::Locked {
                operation: Operation::StartAudioCapture,
            }) => {}
            Err(rejection) => return Err(rejection),
            Ok(()) => {
                return Err(Rejection::format(
                    "capture attempt returned, so the offline gate refuses to continue",
                ));
            }
        }
        let plan = crate::pod_audio::plan(route, period_bytes, periods, || {})
            .map_err(Rejection::format)?;
        Ok(OfflineRecordingGate {
            route: plan.route,
            stream_opened: plan.stream_opened,
            outbound_len: plan.outbound.len(),
            capture_stays_locked: true,
        })
    }

    pub fn accept_family_math(self, role: RoleKind, device_id: u32) -> Result<(), Rejection> {
        let profile = self.require(role, Operation::ApplyFamilyMath)?;
        backend::accept_family_math(profile, device_id)
    }

    pub fn extension_math(self, device_id: u32) -> Result<(), Rejection> {
        let profile = self.extension_profile(Operation::ApplyFamilyMath)?;
        let _ = device_id;
        backend::accept_family_math(profile, device_id)
    }

    /// Physical operations return [`Rejection::Locked`] without calling `attempt`.
    pub fn reject_physical<F>(
        self,
        role: RoleKind,
        operation: Operation,
        claim: Option<CapabilityState>,
        _attempt: F,
    ) -> Result<(), Rejection>
    where
        F: FnOnce() -> Result<(), Rejection>,
    {
        let profile = self.initial_profile(role, operation)?;
        self.reject_physical_for(profile, operation, claim)
    }

    pub fn reject_extension_physical<F>(
        self,
        operation: Operation,
        claim: Option<CapabilityState>,
        _attempt: F,
    ) -> Result<(), Rejection>
    where
        F: FnOnce() -> Result<(), Rejection>,
    {
        let profile = self.extension_profile(operation)?;
        self.reject_physical_for(profile, operation, claim)
    }

    fn reject_physical_for(
        self,
        profile: ProductProfile,
        operation: Operation,
        claim: Option<CapabilityState>,
    ) -> Result<(), Rejection> {
        match capability::resolve(profile, operation, claim) {
            CapabilityState::Locked => Err(Rejection::Locked { operation }),
            state => Err(Rejection::NotPhysical { operation, state }),
        }
    }

    fn require(self, role: RoleKind, operation: Operation) -> Result<ProductProfile, Rejection> {
        let profile = self.initial_profile(role, operation)?;
        self.require_profile(profile, operation)?;
        Ok(profile)
    }

    fn require_profile(
        self,
        profile: ProductProfile,
        operation: Operation,
    ) -> Result<(), Rejection> {
        match capability::resolve(profile, operation, Some(CapabilityState::Offline)) {
            CapabilityState::Offline => Ok(()),
            CapabilityState::Locked => Err(Rejection::Locked { operation }),
            CapabilityState::Planned { issue } => Err(Rejection::Planned { operation, issue }),
            CapabilityState::Unverified => Err(Rejection::Unverified { profile, operation }),
            CapabilityState::Unsupported => Err(Rejection::Unsupported { profile, operation }),
        }
    }

    fn initial_profile(
        self,
        role: RoleKind,
        operation: Operation,
    ) -> Result<ProductProfile, Rejection> {
        match self.kind {
            SessionKind::InitialRig => Ok(role.profile()),
            SessionKind::Extension(profile) => Err(Rejection::Unverified { profile, operation }),
        }
    }

    fn extension_profile(self, operation: Operation) -> Result<ProductProfile, Rejection> {
        match self.kind {
            SessionKind::Extension(profile) => Ok(profile),
            SessionKind::InitialRig => Err(Rejection::Unsupported {
                profile: ProductProfile::Variax600,
                operation,
            }),
        }
    }
}
