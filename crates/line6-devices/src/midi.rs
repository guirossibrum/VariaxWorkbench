//! MIDI byte-stream reassembly. Pure code, no device access.

use std::collections::BTreeMap;

const SYSEX_LIMIT: usize = 8192;

/// Values seen for one controller during a listen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tally {
    pub count: usize,
    pub first: u8,
    pub last: u8,
    pub min: u8,
    pub max: u8,
}

/// Controller messages seen during a listen, keyed by (channel 1..=16, controller).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ControllerTally {
    pub controllers: BTreeMap<(u8, u8), Tally>,
    pub other_messages: usize,
}

impl ControllerTally {
    pub fn add(&mut self, event: &Event) {
        match event {
            Event::Short(message) if message.len() == 3 && message[0] & 0xf0 == 0xb0 => {
                let key = ((message[0] & 0x0f) + 1, message[1]);
                let value = message[2];
                self.controllers
                    .entry(key)
                    .and_modify(|t| {
                        t.count += 1;
                        t.last = value;
                        t.min = t.min.min(value);
                        t.max = t.max.max(value);
                    })
                    .or_insert(Tally {
                        count: 1,
                        first: value,
                        last: value,
                        min: value,
                        max: value,
                    });
            }
            _ => self.other_messages += 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// A complete `F0 .. F7` message.
    Sysex(Vec<u8>),
    /// A complete channel message (note, controller, program change, ...).
    Short(Vec<u8>),
}

/// Reassembles SysEx and channel messages from packets of any size.
///
/// `orphans` counts bytes that arrive with nothing to belong to, such as an
/// `F7` or data bytes whose `F0` never arrived. On PODxt Live interface 1 this
/// is how a lost first reply packet shows up.
#[derive(Default)]
pub struct Assembler {
    sysex: Option<Vec<u8>>,
    status: Option<u8>,
    short: Vec<u8>,
    pub orphans: usize,
}

fn short_len(status: u8) -> usize {
    match status & 0xf0 {
        0xc0 | 0xd0 => 2,
        _ => 3,
    }
}

impl Assembler {
    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<Event>, String> {
        let mut events = Vec::new();
        for &byte in bytes {
            match byte {
                0xf8..=0xff => {} // realtime may appear anywhere, even inside SysEx
                0xf0 => {
                    self.sysex = Some(vec![0xf0]);
                    self.status = None;
                    self.short.clear();
                }
                0xf7 => match self.sysex.take() {
                    Some(mut message) => {
                        message.push(0xf7);
                        events.push(Event::Sysex(message));
                    }
                    None => self.orphans += 1,
                },
                0x80..=0xef => {
                    if self.sysex.take().is_some() {
                        self.orphans += 1; // a status byte cut a SysEx short
                    }
                    self.status = Some(byte);
                    self.short = vec![byte];
                }
                0xf1..=0xf6 => {
                    self.sysex = None;
                    self.status = None;
                    self.short.clear();
                }
                _ => {
                    if let Some(message) = self.sysex.as_mut() {
                        message.push(byte);
                        if message.len() > SYSEX_LIMIT {
                            return Err("unexpectedly large SysEx message".into());
                        }
                    } else if let Some(status) = self.status {
                        if self.short.is_empty() {
                            self.short.push(status); // running status
                        }
                        self.short.push(byte);
                        if self.short.len() == short_len(status) {
                            events.push(Event::Short(std::mem::take(&mut self.short)));
                        }
                    } else {
                        self.orphans += 1;
                    }
                }
            }
        }
        Ok(events)
    }
}

pub fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn describe_short(message: &[u8]) -> String {
    let channel = (message[0] & 0x0f) + 1;
    match (message[0] & 0xf0, message) {
        (0xb0, [_, cc, value]) => format!("controller {cc} = {value} (channel {channel})"),
        (0xc0, [_, program]) => format!("program change {program} (channel {channel})"),
        _ => format!("channel message {}", hex(message)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_identity_reply_reassembles() {
        let mut assembler = Assembler::default();
        let first = [
            0xf0, 0x7e, 0x7f, 0x06, 0x02, 0x00, 0x01, 0x0c, 0x07, 0x00, 0x04, 0x00, 0x30, 0x33,
            0x31, 0x30,
        ];
        assert!(assembler.feed(&first).unwrap().is_empty());
        assert_eq!(assembler.feed(&[0xf7]).unwrap().len(), 1);
        assert_eq!(assembler.orphans, 0);
    }

    #[test]
    fn a_lone_tail_is_counted_as_an_orphan() {
        let mut assembler = Assembler::default();
        assert!(assembler.feed(&[0xf7]).unwrap().is_empty());
        assert_eq!(assembler.orphans, 1);
    }

    #[test]
    fn realtime_inside_sysex_is_ignored() {
        let mut assembler = Assembler::default();
        let events = assembler
            .feed(&[0xf0, 0x01, 0xfe, 0x02, 0xf8, 0xf7])
            .unwrap();
        assert_eq!(events, vec![Event::Sysex(vec![0xf0, 0x01, 0x02, 0xf7])]);
    }

    #[test]
    fn controllers_with_running_status() {
        let mut assembler = Assembler::default();
        let events = assembler.feed(&[0xb0, 0x07, 0x0e, 0x07, 0x0f]).unwrap();
        assert_eq!(
            events,
            vec![
                Event::Short(vec![0xb0, 7, 14]),
                Event::Short(vec![0xb0, 7, 15])
            ]
        );
        assert_eq!(
            describe_short(&[0xb0, 7, 14]),
            "controller 7 = 14 (channel 1)"
        );
    }

    #[test]
    fn oversized_sysex_is_refused() {
        let mut assembler = Assembler::default();
        let mut bytes = vec![0xf0];
        bytes.extend(std::iter::repeat_n(0, SYSEX_LIMIT));
        assert!(assembler.feed(&bytes).is_err());
    }
}
