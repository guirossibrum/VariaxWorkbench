//! Read the active program on a Variax 300/600 that may report unverified firmware.

use super::{
    CurrentPreset, IDENTITY_FAMILY,
    procedures::{self, FullBuffer},
    span_read::SpanReply,
};
use crate::{
    gate::Operation,
    session::{Link, Session},
};

mod replay;
pub use replay::replay_current_program;

/// Released for the first owner-run Rig Check evidence read (2026-11-25, #271).
pub const APPROVED_FIRMWARE_READ: bool = true;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FirmwareStatus {
    Verified310,
    /// The identity reports another version. The captured bytes are diagnostic.
    Unverified,
}

pub struct CurrentProgram {
    pub firmware: FirmwareStatus,
    pub settings: CurrentPreset,
    pub words: FullBuffer,
}

/// Read the identity, two settings blocks and 35 spans of the current buffer.
/// Requires the caller's existing rig lock, identity and read gates, and the
/// separate compile-time approval above. The observer receives complete
/// program chunks. Only the active buffer is read.
pub fn read_current_program<L: Link>(
    session: &mut Session<L>,
    stopping: impl FnMut() -> bool,
    received: impl FnMut(SpanReply) -> Result<(), String>,
) -> Result<CurrentProgram, String> {
    if !APPROVED_FIRMWARE_READ {
        return Err("The Variax firmware compatibility read has no owner approval.".into());
    }
    read_on(session, stopping, received)
}

fn read_on<L: Link>(
    session: &mut Session<L>,
    mut stopping: impl FnMut() -> bool,
    received: impl FnMut(SpanReply) -> Result<(), String>,
) -> Result<CurrentProgram, String> {
    if !session.allows(Operation::VariaxIdentity) || !session.allows(Operation::VariaxRead) {
        return Err(
            "The Variax firmware compatibility read requires identity and read permission.".into(),
        );
    }
    if stopping() {
        return Err("Variax firmware compatibility read cancelled before identity.".into());
    }
    let settings = procedures::read_settings_checked(session, |identity| {
        if !identity.is_line6() || identity.family != IDENTITY_FAMILY || identity.member != 4 {
            return Err(
                "The firmware compatibility read expects a Variax 300/600 identity.".into(),
            );
        }
        if !identity.version.iter().all(u8::is_ascii_digit) {
            return Err("The Variax identity reply has a malformed firmware version.".into());
        }
        if stopping() {
            return Err("Variax firmware compatibility read cancelled after identity.".into());
        }
        Ok(())
    })?;
    let firmware = if procedures::check_identity(&settings.identified.identity).is_ok() {
        FirmwareStatus::Verified310
    } else {
        FirmwareStatus::Unverified
    };
    let words = procedures::read_buffer(session, stopping, received)?;
    Ok(CurrentProgram {
        firmware,
        settings,
        words,
    })
}

#[cfg(test)]
mod tests;
