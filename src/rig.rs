#![forbid(unsafe_code)]
//! Shared rig sessions, model identities, and the static capability matrix.
//!
//! The initial rig is a Variax 600 reached through one PODxt Live USB device.
//! That physical device has two logical roles. Family codecs, DSP gates,
//! firmware formats, and transports stay separate. Record-route selection for
//! the POD role calls the offline planner in `crate::pod_audio`. Nothing in
//! this module enumerates, opens, or queries USB, MIDI, or audio, and there
//! is no live backend initializer.

pub mod audio;
mod backend;
mod capability;
pub mod hd500x_audio;
mod model;
mod registry;
mod session;
pub mod windows_descriptor;

pub use capability::{Capability, CapabilityState, Operation};
pub use model::{Component, DeviceFamily, ProductProfile, RoleKind, VersionKnowledge};
pub use registry::{INITIAL_HOST, LogicalRole, PhysicalHost, WORKBENCH_USB_IS_A_SEPARATE_MATCH};
pub use session::{
    FirmwareStructureSummary, FirmwareSummary, GuitarBankSummary, GuitarPresetSummary,
    HdEditorFileSummary, HdEditorLibrary, LIVE_BACKEND_INITIALIZER, OfflineFileFamily,
    OfflineRecordingGate, PhysicalTargetApplicability, PodToneSummary, Rejection, Session,
};

#[cfg(test)]
mod tests;
