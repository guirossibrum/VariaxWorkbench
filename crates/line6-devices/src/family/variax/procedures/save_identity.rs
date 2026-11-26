//! Strict identity exchanges for the 5.1 save preflight.

use super::{IDENTITY_RETRY_DELAY, Identity, Link, Session, check_identity};
use crate::family::variax::{VariaxRequest, WINDOW};
use crate::session::POLL;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveIdentity {
    Verified,
    /// No packet on request one, then exactly one orphan F7 on request two.
    SilenceThenOrphan,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Reply {
    Identity,
    Silent,
    Orphan,
}

/// This exchange permits at most two requests on the original link. Only the
/// observed silence-then-F7 sequence can authorize a new read-only exchange
/// after the caller has verified USB cleanup.
pub fn identify_for_save_preflight<L: Link>(
    session: &mut Session<L>,
    stopping: &dyn Fn() -> bool,
) -> Result<SaveIdentity, String> {
    let first = exchange(session, stopping, None)?;
    let retry = match first {
        Reply::Identity => return Ok(SaveIdentity::Verified),
        Reply::Silent => {
            session.note(format!(
                "No Variax identity traffic. Waiting {} ms before another request.",
                IDENTITY_RETRY_DELAY.as_millis()
            ));
            session.link.pause(IDENTITY_RETRY_DELAY);
            "Retrying the Variax identity request after silence."
        }
        Reply::Orphan => "Retrying the Variax identity request after an orphan F7.",
    };
    match exchange(session, stopping, Some(retry))? {
        Reply::Identity => Ok(SaveIdentity::Verified),
        Reply::Orphan if first == Reply::Silent => Ok(SaveIdentity::SilenceThenOrphan),
        _ => Err("No complete Variax identity reply after two requests.".into()),
    }
}

/// Exactly one request after a verified release and reclaim. No resend.
pub fn identify_after_save_preflight_reopen<L: Link>(
    session: &mut Session<L>,
    stopping: &dyn Fn() -> bool,
) -> Result<(), String> {
    match exchange(
        session,
        stopping,
        Some("Retrying the Variax identity request once after verified USB release."),
    )? {
        Reply::Identity => Ok(()),
        _ => Err("Variax save preflight identity recovery exhausted after reopening USB.".into()),
    }
}

fn exchange<L: Link>(
    session: &mut Session<L>,
    stopping: &dyn Fn() -> bool,
    retry: Option<&str>,
) -> Result<Reply, String> {
    const PREFIX: [u8; 5] = [0xf0, 0x7e, 0x7f, 0x06, 0x02];
    const REPLY_LEN: usize = 17;
    const UNEXPECTED: &str = "Unrelated or malformed MIDI reply during Variax save preflight.";
    let check_stop = || -> Result<(), String> {
        if stopping() {
            Err("Stopped before verifying the Variax save identity.".into())
        } else {
            Ok(())
        }
    };
    check_stop()?;
    if let Some(retry) = retry {
        session.note(retry);
    }
    session.send(VariaxRequest::Identity)?;
    let end = session.link.elapsed() + WINDOW;
    let mut frame = Vec::with_capacity(REPLY_LEN);
    let mut orphan = false;
    while let Some(left) = end
        .checked_sub(session.link.elapsed())
        .filter(|d| !d.is_zero())
    {
        check_stop()?;
        session.poll_with(left.min(POLL), |packet| {
            if frame.is_empty() && !orphan && packet == [0xf7] {
                orphan = true;
                return Ok(());
            }
            if orphan || packet.is_empty() || packet.len() > REPLY_LEN - frame.len() {
                return Err(UNEXPECTED.into());
            }
            for &byte in packet {
                let index = frame.len();
                let good = match index {
                    0..=4 => byte == PREFIX[index],
                    5..=15 => byte < 0x80,
                    _ => byte == 0xf7,
                };
                if !good {
                    return Err(UNEXPECTED.into());
                }
                frame.push(byte);
            }
            Ok(())
        })?;
        if frame.len() == REPLY_LEN {
            check_identity(&Identity::parse(&frame).ok_or(UNEXPECTED)?)?;
            return Ok(Reply::Identity);
        }
    }
    if orphan {
        // An F7 only qualifies if nothing else arrived in the reply window.
        session.assembler = Default::default();
        Ok(Reply::Orphan)
    } else if frame.is_empty() {
        Ok(Reply::Silent)
    } else {
        Err("Incomplete Variax identity reply during save preflight.".into())
    }
}
