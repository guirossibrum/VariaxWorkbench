//! Bounded reads of the 1,092-word active buffer on Variax firmware 3.10.

use super::{HEADER, decode_word, is_reply, with_bridged_read_session};
use crate::{
    gate::Operation,
    midi::Event,
    session::{Link, POLL, Path, Request, Session},
};
use std::time::Duration;

pub const BUFFER_WORDS: u16 = 1092;
// Monkey enables the same PODxt Live large-buffer relay, then waits 1 s (#232).
const BRIDGE_SETTLE: Duration = Duration::from_secs(1);
const REPLY_WINDOW: Duration = Duration::from_secs(10);

/// An in-bounds, nonempty range in the active preset buffer, in words.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PresetSpan {
    start: u16,
    count: u16,
}

impl PresetSpan {
    pub const fn new(start: u16, count: u16) -> Option<Self> {
        if start < BUFFER_WORDS && count != 0 && count <= BUFFER_WORDS - start {
            Some(Self { start, count })
        } else {
            None
        }
    }

    pub const fn start(self) -> u16 {
        self.start
    }

    pub const fn count(self) -> u16 {
        self.count
    }

    pub const fn reply_bytes(self) -> usize {
        8 + 6 * self.count as usize
    }

    /// Decode complete SysEx into the requested number of 24-bit words.
    /// A 0x3C reply carries neither the start address nor a sequence number.
    pub fn decode_reply(self, reply: &[u8]) -> Result<Vec<u32>, String> {
        if !is_reply(reply, 0x3c) {
            return Err("Invalid Variax 0x3C reply envelope or missing F7.".into());
        }
        if reply.len() != self.reply_bytes() {
            return Err(format!(
                "Variax 0x3C reply has {} bytes, expected {}.",
                reply.len(),
                self.reply_bytes()
            ));
        }
        reply[7..reply.len() - 1]
            .as_chunks::<6>()
            .0
            .iter()
            .map(|nibbles| decode_word(nibbles))
            .collect()
    }
}

impl Request for PresetSpan {
    fn bytes(self) -> Vec<u8> {
        let nibbles = |word: u16| (0..6).map(move |i| ((u32::from(word) >> (4 * i)) & 15) as u8);
        HEADER
            .into_iter()
            .chain([0, 0x3c])
            .chain(nibbles(self.count))
            .chain(nibbles(self.start))
            .chain([0xf7])
            .collect()
    }

    fn path(self) -> Path {
        Path::VariaxViaPodxt
    }

    fn operation(self) -> Operation {
        Operation::VariaxRead
    }
}

#[derive(Debug)]
pub struct SpanReply {
    pub span: PresetSpan,
    pub raw: Vec<u8>,
    pub words: Vec<u32>,
}

/// Read spans in order in one G3 session. Hand off each complete reply before
/// sending the next request. Cancellation takes effect between spans so the
/// host keeps draining an in-flight reply until it ends or the deadline expires.
pub fn read_spans<L: Link>(
    session: &mut Session<L>,
    spans: &[PresetSpan],
    mut stopping: impl FnMut() -> bool,
    mut received: impl FnMut(SpanReply) -> Result<(), String>,
) -> Result<(), String> {
    if session.link.path() != Path::VariaxViaPodxt || !session.allows(Operation::VariaxRead) {
        return Err("Buffer read requires the PODxt Live bridge and VariaxRead.".into());
    }
    if spans.is_empty() {
        return Ok(());
    }
    if stopping() {
        return Err("Variax buffer read cancelled or log unavailable.".into());
    }
    with_bridged_read_session(session, BRIDGE_SETTLE, |session| {
        for &span in spans {
            if stopping() {
                return Err("Variax buffer read cancelled or log unavailable.".into());
            }
            let reply = read_span(session, span).map_err(|error| {
                format!("Start {}, count {}: {error}", span.start(), span.count())
            })?;
            received(reply)?;
        }
        Ok(())
    })
}

fn read_span<L: Link>(session: &mut Session<L>, span: PresetSpan) -> Result<SpanReply, String> {
    session.send(span)?;
    let deadline = session.link.elapsed() + REPLY_WINDOW;
    let orphans_before = session.assembler.orphans;
    let mut in_sysex = false;
    let mut reply = None;
    let timeout = || {
        format!(
            "Timed out waiting for a complete {}-byte 0x3C reply ending in F7.",
            span.reply_bytes()
        )
    };
    loop {
        let remaining = deadline.saturating_sub(session.link.elapsed());
        if remaining.is_zero() {
            return Err(timeout());
        }
        let events = session.poll_with(remaining.min(POLL), |packet| {
            for &byte in packet {
                match byte {
                    0xf0 if in_sysex => return Err("New SysEx before the previous F7.".into()),
                    0xf0 => in_sysex = true,
                    0xf7 => in_sysex = false,
                    0xf1..=0xf6 => {
                        return Err("Unexpected system-common byte during 0x3C read.".into());
                    }
                    _ => {}
                }
            }
            Ok(())
        })?;
        if session.link.elapsed() > deadline {
            return Err(timeout());
        }
        if session.assembler.orphans != orphans_before {
            return Err("Unexpected orphan MIDI bytes during 0x3C read.".into());
        }
        let Some(events) = events else { continue };
        for event in events {
            match event {
                Event::Sysex(message)
                    if [0x28, 0x4d, 0x74].iter().any(|&op| is_reply(&message, op)) =>
                {
                    session.note(format!("Skipped Variax notice 0x{:02X}.", message[6]));
                }
                Event::Sysex(raw) if reply.is_none() => {
                    let words = span.decode_reply(&raw)?;
                    reply = Some(SpanReply { span, raw, words });
                }
                Event::Short(message) if message[0] & 0xf0 == 0xb0 => {
                    // The enabled MIDI session can emit unsolicited controls.
                }
                _ => return Err("Unexpected MIDI message during Variax 0x3C read.".into()),
            }
        }
        if !in_sysex && let Some(reply) = reply {
            return Ok(reply);
        }
    }
}

#[cfg(test)]
mod tests;
