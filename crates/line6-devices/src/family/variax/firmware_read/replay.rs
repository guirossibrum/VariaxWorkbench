//! Run the compatibility decoder on in-memory replies, with no device link.

use super::{CurrentProgram, Operation, SpanReply, read_on};
use crate::{
    gate::GatePolicy,
    identity::REQUEST,
    session::{Link, Log, Path, Session},
};
use std::{collections::VecDeque, time::Duration};

/// Replay synthetic or captured replies in request order: identity, two
/// settings replies, then program chunks. This API owns its in-memory link.
pub fn replay_current_program(
    replies: impl IntoIterator<Item = Vec<u8>>,
    log: &mut dyn Log,
    stopping: impl FnMut() -> bool,
    received: impl FnMut(SpanReply) -> Result<(), String>,
) -> Result<CurrentProgram, String> {
    let link = Replay {
        replies: replies.into_iter().collect(),
        packets: VecDeque::new(),
        elapsed: Duration::ZERO,
    };
    read_on(&mut Session::new(link, &ReadOnly, log), stopping, received)
}

struct ReadOnly;

impl GatePolicy for ReadOnly {
    fn allows(&self, operation: Operation) -> bool {
        matches!(operation, Operation::VariaxIdentity | Operation::VariaxRead)
    }
}

struct Replay {
    replies: VecDeque<Vec<u8>>,
    packets: VecDeque<Vec<u8>>,
    elapsed: Duration,
}

impl Link for Replay {
    fn path(&self) -> Path {
        Path::VariaxViaPodxt
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes == REQUEST || bytes.starts_with(&[0xf0, 0, 1, 0x0c, 7, 0, 0x3c]) {
            if let Some(reply) = self.replies.pop_front() {
                self.packets.extend(reply.chunks(16).map(<[u8]>::to_vec));
            }
        } else if !matches!(
            bytes,
            [0xf0, 0, 1, 0x0c, 3, 0x23, 0 | 1, 0xf7] | [0xf0, 0, 1, 0x0c, 7, 0, 0x2a, 0 | 1, 0xf7]
        ) {
            return Err("Unexpected request during Variax firmware replay.".into());
        }
        Ok(())
    }

    fn read(&mut self, wait: Duration) -> Result<Option<Vec<u8>>, String> {
        let packet = self.packets.pop_front();
        self.elapsed += if packet.is_some() {
            Duration::from_millis(4)
        } else {
            wait
        };
        Ok(packet)
    }

    fn elapsed(&self) -> Duration {
        self.elapsed
    }

    fn pause(&mut self, duration: Duration) {
        self.elapsed += duration;
    }
}
