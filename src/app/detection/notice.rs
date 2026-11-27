//! Passive PODxt Live input shared by Variax presence and channel Follow.
#[cfg(any(test, all(feature = "usb", target_os = "linux")))]
use line6_devices::midi::{Assembler, Event};
use std::time::Instant;
#[cfg(any(test, all(feature = "usb", target_os = "linux")))]
use variax_workbench::midi::SysexDecoder;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::editor) struct PodxtProgram {
    pub channel: u8,
    pub observed_at: Instant,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(
    not(any(test, all(feature = "usb", target_os = "linux"))),
    allow(dead_code)
)]
pub(in crate::editor) enum Notice {
    Connected,
    Disconnected,
    PodxtProgram(PodxtProgram),
}

#[cfg(any(test, all(feature = "usb", target_os = "linux")))]
const LINE6_HEADER: [u8; 4] = [0xf0, 0x00, 0x01, 0x0c];

#[cfg(any(test, all(feature = "usb", target_os = "linux")))]
fn known_prefix(bytes: &[u8]) -> bool {
    bytes.starts_with(&LINE6_HEADER) && matches!(bytes.get(4..6), Some([0x03, 0x11] | [0x7f, 0x03]))
}

#[cfg(any(test, all(feature = "usb", target_os = "linux")))]
fn decode(frame: &[u8]) -> Option<Result<Notice, &'static str>> {
    if !frame.starts_with(&LINE6_HEADER) {
        return None;
    }
    match frame.get(4..6) {
        Some([0x03, 0x11]) if frame.len() == 11 => Some(Ok(if frame[9] == 0 {
            Notice::Disconnected
        } else {
            Notice::Connected
        })),
        Some([0x7f, 0x03]) if frame.len() == 12 => Some(Ok(Notice::Disconnected)),
        Some([0x03, 0x11] | [0x7f, 0x03]) => Some(Err("malformed notice length")),
        Some([0x03, _]) if frame.len() == 11 => Some(Err("unknown 03 notice opcode")),
        Some([0x7f, _]) if frame.len() == 12 => Some(Err("unknown 7F notice opcode")),
        _ => None,
    }
}

#[cfg(any(test, all(feature = "usb", target_os = "linux")))]
#[derive(Default)]
pub(super) struct NoticeStream {
    sysex: SysexDecoder,
    midi: Assembler,
}

#[cfg(any(test, all(feature = "usb", target_os = "linux")))]
impl NoticeStream {
    pub fn receive(&mut self, packet: &[u8], notices: &mut Vec<Notice>, mut log: impl FnMut(&str)) {
        // Keep the Variax decoder's existing malformed-packet handling.
        // Assemble short MIDI in parallel from the same passive input.
        if let Ok(events) = self.midi.feed(packet) {
            let observed_at = Instant::now();
            notices.extend(events.into_iter().filter_map(|event| {
                let Event::Short(message) = event else {
                    return None;
                };
                match message.as_slice() {
                    [status, channel] if status & 0xf0 == 0xc0 => {
                        Some(Notice::PodxtProgram(PodxtProgram {
                            channel: *channel,
                            observed_at,
                        }))
                    }
                    _ => None,
                }
            }));
        }
        match self.sysex.feed_with_discard(packet, |bytes| {
            if known_prefix(bytes) {
                log("interrupted notice");
            }
        }) {
            Ok(frames) => {
                for frame in frames {
                    match decode(&frame) {
                        Some(Ok(notice)) => {
                            let last_presence = notices.iter().rev().find(|notice| {
                                matches!(notice, Notice::Connected | Notice::Disconnected)
                            });
                            if last_presence != Some(&notice) {
                                notices.push(notice);
                            }
                        }
                        Some(Err(error)) => log(error),
                        None => {}
                    }
                }
            }
            Err(error) => log(&error),
        }
    }

    pub fn finish(self, mut log: impl FnMut(&str)) {
        if known_prefix(self.sysex.unfinished()) {
            log("unfinished notice");
        }
    }
}

/// Read the next batch until a usable notice arrives or active work takes over.
#[cfg(any(test, all(feature = "usb", target_os = "linux")))]
pub(super) fn read_notices(
    mut read_input: impl FnMut(std::time::Duration) -> Result<Option<Vec<u8>>, String>,
    mut stopping: impl FnMut() -> bool,
    mut log: impl FnMut(&str),
) -> Vec<Notice> {
    let mut stream = NoticeStream::default();
    let mut notices = Vec::new();
    while notices.is_empty() && !stopping() {
        match read_input(std::time::Duration::from_millis(20)) {
            Ok(Some(packet)) => stream.receive(&packet, &mut notices, &mut log),
            Ok(None) => {}
            Err(_) => break,
        }
    }
    stream.finish(log);
    notices
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passive_input_reads_only_until_a_notice_or_yield() {
        let mut packets = std::collections::VecDeque::from([
            vec![0xf0, 0, 1, 12, 3, 0x12, 0, 0, 0, 1, 0xf7],
            vec![0xf0, 0, 1, 12, 3, 0x11],
            vec![0, 0, 0, 1, 0xf7],
        ]);
        let mut logged = Vec::new();
        let mut read = |wait: std::time::Duration| {
            assert_eq!(wait.as_millis(), 20);
            packets
                .pop_front()
                .map(Some)
                .ok_or_else(|| "end of fake input".into())
        };
        assert!(read_notices(&mut read, || true, |_| panic!("yield before reading")).is_empty());
        assert_eq!(
            read_notices(read, || false, |line| logged.push(line.to_owned())),
            [Notice::Connected]
        );
        assert_eq!(logged, ["unknown 03 notice opcode"]);
    }

    #[test]
    fn shared_input_assembles_program_changes_and_preserves_variax_notices() {
        let mut packets = std::collections::VecDeque::from([
            vec![0xb0, 0, 4, 0xc0],
            vec![3, 127, 0xf0, 0, 1, 12, 3, 0x11, 0, 0, 0, 7, 0xf7],
        ]);
        let result = read_notices(
            |_| Ok(packets.pop_front()),
            || false,
            |error| panic!("{error}"),
        );
        let [
            Notice::PodxtProgram(first),
            Notice::PodxtProgram(last),
            Notice::Connected,
        ] = result.as_slice()
        else {
            panic!("unexpected notices {result:?}");
        };
        assert_eq!([first.channel, last.channel], [3, 127]);
        assert_eq!(first.observed_at, last.observed_at);
        assert!(packets.is_empty());
    }

    #[test]
    fn fragmented_notices_keep_the_latest_state_and_ignore_malformed_or_unknown_frames() {
        let connected = [0xf0, 0, 1, 12, 3, 0x11, 0, 0, 0, 7, 0xf7];
        let mut input = NoticeStream::default();
        let mut notices = Vec::new();
        let mut messages = Vec::new();
        input.receive(&connected[..6], &mut notices, |error| {
            messages.push(error.to_owned())
        });
        assert!(notices.is_empty());
        input.receive(&connected[6..], &mut notices, |error| {
            messages.push(error.to_owned())
        });
        assert_eq!(notices.last(), Some(&Notice::Connected));

        let mut wrong = connected;
        wrong[5] = 0x12;
        input.receive(&wrong, &mut notices, |error| {
            messages.push(error.to_owned())
        });
        input.receive(&[0xf0, 0, 1, 12, 3, 0x11, 0xf7], &mut notices, |error| {
            messages.push(error.to_owned())
        });
        assert_eq!(notices.last(), Some(&Notice::Connected));
        assert_eq!(
            messages,
            ["unknown 03 notice opcode", "malformed notice length"]
        );

        let mut disconnected = connected;
        disconnected[9] = 0;
        input.receive(&disconnected, &mut notices, |_| panic!("valid notice"));
        assert_eq!(notices.last(), Some(&Notice::Disconnected));
        let disconnect = [0xf0, 0, 1, 12, 0x7f, 3, 0, 0, 0, 0, 0, 0xf7];
        input.receive(&connected, &mut notices, |_| panic!("valid notice"));
        input.receive(&disconnect, &mut notices, |_| panic!("valid notice"));
        assert_eq!(notices.last(), Some(&Notice::Disconnected));
        input.receive(&[0xf0, 0, 1, 12, 3, 0x11, 0x90], &mut notices, |error| {
            assert_eq!(error, "interrupted notice")
        });
        assert_eq!(notices.last(), Some(&Notice::Disconnected));
        input.receive(&connected[..10], &mut notices, |_| panic!("split fragment"));
        input.finish(|error| assert_eq!(error, "unfinished notice"));
    }
}
