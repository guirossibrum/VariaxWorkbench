#![forbid(unsafe_code)]
//! Memory-only protocol/state models. There is no physical transport or unlock.
//!
//! These models follow the bounded source map in `docs/update-recovery.md`.
//! An acknowledgment is a modeled protocol event, never a flash-durability or
//! hardware-safety claim. Unknown ROM/peripheral/recovery behavior fails closed.

mod control;
mod receiver;
mod wire;

pub use control::{ControlFault, VirtualControl, VirtualSessionState};
pub use receiver::{Fault, Integrity, OperatingMode, Phase, Receiver, ReceiverReport, Stage};

use crate::firmware;

pub const SCOPE: &str = "Memory-only synthetic protocol model. Not device firmware execution, audio emulation, a physical flash model, or proof of recovery or hardware safety.";
pub const PHYSICAL_OPERATIONS_PERMITTED: bool = false;

/// Run a synthetic Variax transfer in memory. Buffers never leave this module.
/// Corruption/fault experiments can feed [`Receiver`] directly and inspect its
/// report after an error, including any already-recorded programming intent.
pub fn simulate_synthetic(
    profile: Profile,
    stage: Stage,
    payload: &[u8],
    fragment_bytes: usize,
) -> Result<ReceiverReport, ModelError> {
    if !(1..=receiver::MAX_FEED_BYTES).contains(&fragment_bytes) {
        return Err(ModelError::Limit);
    }
    if stage == Stage::MainCode && (payload.is_empty() || !payload.len().is_multiple_of(3)) {
        return Err(ModelError::UnsupportedCount);
    }
    if stage == Stage::BootRegion && payload.len() != 16_384 {
        return Err(ModelError::UnsupportedCount);
    }
    let mut receiver = Receiver::new(profile, stage)?;
    for frame in wire::variax(stage, payload)? {
        for fragment in frame.chunks(fragment_bytes) {
            receiver.feed(fragment)?;
        }
    }
    receiver.finish()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PodSegmentShape {
    source_bytes: usize,
    send_calls: usize,
    encoded_sha256: [u8; 32],
}

impl PodSegmentShape {
    pub const fn source_bytes(&self) -> usize {
        self.source_bytes
    }
    pub const fn send_calls(&self) -> usize {
        self.send_calls
    }
    pub const fn encoded_sha256(&self) -> &[u8; 32] {
        &self.encoded_sha256
    }
    pub const fn receiver_modeled(&self) -> bool {
        false
    }
    pub const fn acknowledgment_modeled(&self) -> bool {
        false
    }
    pub const fn durability_known(&self) -> bool {
        false
    }
}

/// Host packing is established; a POD DSP receiver/flash model is not.
/// The output is only a fingerprint/count summary, never sendable buffers.
pub fn describe_synthetic_pod_segment(payload: &[u8]) -> Result<PodSegmentShape, ModelError> {
    use sha2::{Digest, Sha256};
    let frames = wire::pod(payload)?;
    let mut hash = Sha256::new();
    for frame in &frames {
        hash.update(frame);
    }
    Ok(PodSegmentShape {
        source_bytes: payload.len(),
        send_calls: frames.len(),
        encoded_sha256: hash.finalize().into(),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    Variax600,
    PodxtLive,
}

impl Profile {
    pub const fn target(self) -> (u32, u32) {
        match self {
            Self::Variax600 => (
                firmware::VARIAX_300_600_PRODUCT_ID,
                firmware::VARIAX_300_600_VERSION,
            ),
            Self::PodxtLive => (
                firmware::PODXT_LIVE_PRODUCT_ID,
                firmware::PODXT_LIVE_VERSION,
            ),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelError {
    WrongFamily,
    Candidate(String),
    InvalidHeader,
    WrongStage,
    NonCanonicalNibble,
    UnsupportedCount,
    Limit,
    Truncated,
    ExtraData,
    Checksum,
    Interrupted,
    UnsupportedRecovery,
    UnsupportedOperation,
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "offline model: {self:?}")
    }
}

impl std::error::Error for ModelError {}

/// A checked container declaration; even exact archived bytes are not signed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    profile: Profile,
    source_sha256: [u8; 32],
    image_sha256: [u8; 32],
    image_bytes: usize,
}

impl Candidate {
    pub fn inspect(profile: Profile, bytes: &[u8]) -> Result<Self, ModelError> {
        let inspection =
            firmware::inspect(bytes).map_err(|error| ModelError::Candidate(error.to_string()))?;
        let (product, version) = profile.target();
        inspection
            .require_target(product, version)
            .map_err(|error| ModelError::Candidate(error.to_string()))?;
        Ok(Self {
            profile,
            source_sha256: inspection.report.source_sha256,
            image_sha256: inspection.report.image_sha256,
            image_bytes: inspection.image().len(),
        })
    }

    pub const fn profile(&self) -> Profile {
        self.profile
    }

    pub const fn source_sha256(&self) -> &[u8; 32] {
        &self.source_sha256
    }

    pub const fn image_sha256(&self) -> &[u8; 32] {
        &self.image_sha256
    }

    pub const fn image_bytes(&self) -> usize {
        self.image_bytes
    }

    pub const fn authenticated(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests;
