//! Simulated PODxt Live and Variax 600 for practice mode and tests.
//!
//! Byte-level models of the two devices answer the same messages the shared
//! device library sends (identity, PODxt dumps, Variax `0x3C` reads, program
//! changes, working-buffer uploads, stores). Everything is synthetic: tone
//! and program contents come from a fixed pseudo-random generator, never from
//! a vendor file or a real capture. Time is virtual, so a full run finishes
//! instantly. Faults can be injected to rehearse failures.

use line6_devices::family::variax::Preset;
use line6_devices::session::{Link, Path};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::Duration;

pub const PROGRAM_WORDS: usize = 1092;
pub const TONE_BYTES: usize = 160;
/// Tones, amp setups and FX setups, by `Location::address` order of kinds.
pub const PODXT_LOCATIONS: usize = 128 + 107 + 64;
const PACKET: usize = 16;
const LATENCY: Duration = Duration::from_millis(3);
const PACKET_GAP: Duration = Duration::from_millis(1);

pub type Program = [u32; PROGRAM_WORDS];

/// Faults to rehearse failures in practice mode and tests.
#[derive(Clone, Debug, Default)]
pub struct Faults {
    /// The PODxt Live is not on USB.
    pub podxt_absent: bool,
    /// Lose the first packet of the next Variax identity reply (#213 behaviour).
    pub drop_first_variax_identity_packet: bool,
    /// Drop one byte of the n-th Variax `0x3C` reply (1-based), once.
    pub damage_span_reply: Option<usize>,
    pub change_uploaded_words_once: bool,
    /// The Variax never answers (VDI unplugged from the start).
    pub variax_silent: bool,
    /// Cable changes affect identity replies but no connection notices arrive.
    pub suppress_vdi_notices: bool,
    #[cfg(test)]
    pub fail_variax_read: bool,
    /// The Variax store is not confirmed with `0x71`.
    pub silent_variax_store: bool,
    /// The PODxt store is not confirmed.
    pub silent_podxt_store: bool,
    /// The person does not perform a guided action in time.
    pub user_does_not_act: bool,
    pub podxt_knob_events: Option<Vec<(u8, u8)>>,
    pub guided_action_delay: Option<Duration>,
    pub missing_model_data: bool,
    /// Report the Variax as firmware 3.09 (unverified).
    pub unverified_variax: bool,
    /// Report the PODxt Live as firmware 2.07 (unverified).
    pub unverified_podxt: bool,
    /// Suppress the PODxt identity reply without removing it from USB.
    pub podxt_identity_silent: bool,
    /// The guitar interface is not given back to its driver after release.
    pub driver_not_returned: bool,
    /// Another program holds the guitar interface.
    pub interface_busy: bool,
    /// This user may not open the PODxt Live.
    pub permission_denied: bool,
}

/// What the simulated person does when a guided step asks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UserAction {
    TurnSelector(Preset),
    SweepVariaxKnobs,
    PodxtFootswitch(u16),
    MovePedal,
    SweepPodxtKnobs,
    UnplugVdi,
    ReplugVdi,
}

pub struct PodxtModel {
    pub firmware: [u8; 4],
    pub stored: Vec<[u8; TONE_BYTES]>,
    pub playing: [u8; TONE_BYTES],
    pub channel: Option<u16>,
    pub stores: usize,
}

pub struct VariaxModel {
    pub member: u16,
    pub firmware: [u8; 4],
    /// Stored programs by flash index (`bank * 5 + (4 - slot)`).
    pub stored: Vec<Program>,
    pub active: Program,
    pub selected: Option<Preset>,
    pub session_on: bool,
    pub connected: bool,
    pub stores: usize,
    pub working_uploads: usize,
}

struct Queued {
    at: Duration,
    packet: Vec<u8>,
}

#[cfg(test)]
pub(super) enum TestPodxtDump {
    Missing,
    Delayed(Duration),
    Interrupted { channel: u8, delay: Duration },
}

pub struct SimRig {
    pub clock: Duration,
    pub podxt: PodxtModel,
    pub variax: VariaxModel,
    pub faults: Faults,
    podxt_out: VecDeque<Queued>,
    variax_out: VecDeque<Queued>,
    variax_pending: Vec<u8>,
    span_replies: usize,
    /// Every message written by the host, with its path name.
    pub written: Vec<(&'static str, Vec<u8>)>,
    #[cfg(test)]
    pub(super) next_podxt_dump: Option<TestPodxtDump>,
    pub variax_claimed: bool,
    pub driver_bound: bool,
}

/// Small deterministic generator for synthetic contents.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }
}

pub fn flash_index(preset: Preset) -> usize {
    usize::from(preset.bank()) * 5 + usize::from(4 - preset.slot())
}

pub fn preset_for_program_number(number: u8) -> Option<Preset> {
    Preset::all().find(|preset| preset.program_number() == number)
}

pub fn preset_label(preset: Preset) -> String {
    format!("{} {}", preset.bank_name(), preset.slot() + 1)
}

/// A synthetic 1,092-word program with a valid name field and checksums.
pub fn synthetic_program(name: &str, seed: u64) -> Program {
    let mut rng = Lcg(seed);
    let mut words = [0u32; PROGRAM_WORDS];
    for word in words.iter_mut().take(1089) {
        *word = rng.next() & 0xff_ffff;
    }
    let mut metadata = [0u8; 192];
    for byte in metadata.iter_mut() {
        *byte = (rng.next() & 0x7f) as u8;
    }
    let mut field = [b' '; 18];
    field[..name.len().min(18)].copy_from_slice(&name.as_bytes()[..name.len().min(18)]);
    metadata[..18].copy_from_slice(&field);
    metadata[18..22].copy_from_slice(&0x0310_0000u32.to_be_bytes());
    metadata[65..75].fill(0);
    for (i, chunk) in metadata.chunks(3).enumerate() {
        words[851 + i] = u32::from_be_bytes([0, chunk[0], chunk[1], chunk[2]]);
    }
    for (word, version) in words[767..771].iter_mut().zip(*b"0310") {
        *word = u32::from(version);
    }
    seal(&mut words);
    words
}

/// Recompute the host checksum in word 1091 (words 1089 and 1090 stay zero).
pub fn seal(words: &mut Program) {
    words[1089] = 0;
    words[1090] = 0;
    let sum: u64 = words[..1089].iter().map(|&w| u64::from(w)).sum();
    words[1091] = (sum & 0xff_ffff) as u32;
}

pub fn packed(words: &Program) -> [u8; PROGRAM_WORDS * 3] {
    let mut out = [0u8; PROGRAM_WORDS * 3];
    for (i, word) in words.iter().enumerate() {
        out[i * 3..i * 3 + 3].copy_from_slice(&word.to_be_bytes()[1..]);
    }
    out
}

pub fn program_name(words: &Program) -> String {
    let bytes: Vec<u8> = words[851..857]
        .iter()
        .flat_map(|w| w.to_be_bytes()[1..].to_vec())
        .collect();
    String::from_utf8_lossy(&bytes)
        .trim_end_matches([' ', '\0'])
        .to_owned()
}

fn synthetic_tone(name: &str, seed: u64) -> [u8; TONE_BYTES] {
    let mut rng = Lcg(seed);
    let mut tone = [0u8; TONE_BYTES];
    for byte in tone.iter_mut() {
        *byte = (rng.next() & 0x7f) as u8;
    }
    let mut field = [b' '; 16];
    field[..name.len().min(16)].copy_from_slice(&name.as_bytes()[..name.len().min(16)]);
    tone[..16].copy_from_slice(&field);
    for setting in line6_devices::family::podxt::Setting::ALL {
        if setting.model_list().is_some() {
            tone[32
                + usize::from(
                    line6_devices::family::podxt::LiveControl::Additional(setting).cc(),
                )] = 0;
        }
    }
    tone[32 + 89] = (1200 >> 7) as u8;
    tone[32 + 90] = (1200 & 127) as u8;
    tone
}

/// The PODxt Live channel label for a tone index (`1A`..`32D`).
pub fn channel_label(index: u16) -> String {
    format!(
        "{}{}",
        index / 4 + 1,
        ['A', 'B', 'C', 'D'][usize::from(index % 4)]
    )
}

/// Index into the simulated library (tones, amp setups, FX setups) for a
/// PODxt `Location::address` (Line 6 Edit `TranslateAddr`).
fn location_from_address(address: u16) -> Option<usize> {
    let a = usize::from(address);
    Some(match a {
        0..=63 => a,
        64..=127 => 128 + (a - 64),
        128..=191 => 235 + (a - 128),
        192..=255 => a - 128,
        256..=298 => 128 + (a - 192),
        _ => return None,
    })
}

fn tone_address_index(address: u16) -> Option<usize> {
    location_from_address(address).filter(|&index| index < 128)
}

impl SimRig {
    /// The practice rig: PODxt Live 3.01 on channel 5B, Variax 600 3.10 on Spank 1.
    pub fn new(faults: Faults) -> Self {
        let mut stored_tones = Vec::with_capacity(PODXT_LOCATIONS);
        for index in 0..128u16 {
            stored_tones.push(synthetic_tone(
                &format!("Practice {}", channel_label(index)),
                1000 + u64::from(index),
            ));
        }
        for index in 0..107u64 {
            stored_tones.push(synthetic_tone(&format!("Amp {}", index + 1), 2000 + index));
        }
        for index in 0..64u64 {
            stored_tones.push(synthetic_tone(&format!("FX {}", index + 1), 3000 + index));
        }
        let start_channel = 17; // 5B
        let mut programs = vec![[0u32; PROGRAM_WORDS]; 60];
        for preset in Preset::all() {
            programs[flash_index(preset)] =
                synthetic_program(&preset_label(preset), 77 + flash_index(preset) as u64);
        }
        let start = Preset::new(2, 0).expect("Spank 1");
        let firmware = if faults.unverified_variax {
            *b"0309"
        } else {
            *b"0310"
        };
        Self {
            clock: Duration::ZERO,
            podxt: PodxtModel {
                firmware: if faults.unverified_podxt {
                    [0, 2, 0, 7]
                } else {
                    [0, 3, 0, 1]
                },
                playing: stored_tones[start_channel],
                stored: stored_tones,
                channel: Some(start_channel as u16),
                stores: 0,
            },
            variax: VariaxModel {
                member: 4,
                firmware,
                active: programs[flash_index(start)],
                stored: programs,
                selected: Some(start),
                session_on: false,
                connected: !faults.variax_silent,
                stores: 0,
                working_uploads: 0,
            },
            faults,
            podxt_out: VecDeque::new(),
            variax_out: VecDeque::new(),
            variax_pending: Vec::new(),
            span_replies: 0,
            #[cfg(test)]
            next_podxt_dump: None,
            written: Vec::new(),
            variax_claimed: false,
            driver_bound: true,
        }
    }

    pub fn shared(faults: Faults) -> Rc<RefCell<SimRig>> {
        Rc::new(RefCell::new(Self::new(faults)))
    }

    fn queue(out: &mut VecDeque<Queued>, at: Duration, message: &[u8]) {
        let mut time = at;
        for chunk in message.chunks(PACKET) {
            let index = out.partition_point(|queued| queued.at <= time);
            out.insert(
                index,
                Queued {
                    at: time,
                    packet: chunk.to_vec(),
                },
            );
            time += PACKET_GAP;
        }
    }

    fn reply_podxt(&mut self, message: &[u8]) {
        let at = self.clock + LATENCY;
        Self::queue(&mut self.podxt_out, at, message);
    }

    fn reply_variax(&mut self, message: &[u8]) {
        if !self.variax.connected {
            return;
        }
        let at = self.clock + LATENCY;
        Self::queue(&mut self.variax_out, at, message);
    }

    pub fn podxt_identity(&self) -> Vec<u8> {
        let v = self.podxt.firmware;
        vec![
            0xf0, 0x7e, 0x7f, 0x06, 0x02, 0x00, 0x01, 0x0c, 0x03, 0x00, 0x0a, 0x00, v[0], v[1],
            v[2], v[3], 0xf7,
        ]
    }

    pub fn variax_identity(&self) -> Vec<u8> {
        let v = self.variax.firmware;
        let member = self.variax.member.to_le_bytes();
        vec![
            0xf0, 0x7e, 0x7f, 0x06, 0x02, 0x00, 0x01, 0x0c, 0x07, 0x00, member[0], member[1], v[0],
            v[1], v[2], v[3], 0xf7,
        ]
    }

    pub(super) fn variax_evidence_replies(&self) -> Vec<Vec<u8>> {
        let mut replies = vec![
            self.variax_identity(),
            self.variax_span_reply(851, 32),
            self.variax_span_reply(883, 32),
        ];
        replies.extend(
            (0..PROGRAM_WORDS)
                .step_by(32)
                .map(|start| self.variax_span_reply(start, (PROGRAM_WORDS - start).min(32))),
        );
        replies
    }

    fn variax_span_reply(&self, start: usize, count: usize) -> Vec<u8> {
        let mut reply = vec![0xf0, 0x00, 0x01, 0x0c, 0x07, 0x04, 0x3c];
        for word in &self.variax.active[start..start + count] {
            reply.extend((0..6).map(|i| ((word >> (4 * i)) & 0xf) as u8));
        }
        reply.push(0xf7);
        reply
    }

    fn tone_dump(tone: &[u8; TONE_BYTES]) -> Vec<u8> {
        let mut dump = vec![0xf0, 0x00, 0x01, 0x0c, 0x03, 0x74, 0x0a];
        dump.extend_from_slice(tone);
        dump.push(0xf7);
        dump
    }

    #[cfg(test)]
    fn script_podxt_dump(&mut self, dump: &[u8], reply: TestPodxtDump) {
        let delay = match reply {
            TestPodxtDump::Missing => return,
            TestPodxtDump::Delayed(delay) => delay,
            TestPodxtDump::Interrupted { channel, delay } => {
                Self::queue(&mut self.podxt_out, self.clock + LATENCY, &[0xc0, channel]);
                delay
            }
        };
        Self::queue(&mut self.podxt_out, self.clock + delay, dump);
    }

    fn podxt_write(&mut self, bytes: &[u8]) {
        self.written.push(("PodxtMidi", bytes.to_vec()));
        match bytes {
            [0xf0, 0x7e, 0x7f, 0x06, 0x01, 0xf7] => {
                let reply = self.podxt_identity();
                if !self.faults.podxt_identity_silent {
                    self.reply_podxt(&reply);
                }
            }
            [0xf0, 0x00, 0x01, 0x0c, 0x03, 0x75, 0xf7] => {
                let dump = Self::tone_dump(&self.podxt.playing);
                #[cfg(test)]
                if let Some(reply) = self.next_podxt_dump.take() {
                    self.script_podxt_dump(&dump, reply);
                    return;
                }
                self.reply_podxt(&dump);
            }
            [0xf0, 0x00, 0x01, 0x0c, 0x03, 0x73, hi, lo, 0, 0, 0xf7] => {
                let address = (u16::from(*hi) << 7) | u16::from(*lo);
                if let Some(index) = location_from_address(address) {
                    let dump = Self::tone_dump(&self.podxt.stored[index]);
                    self.reply_podxt(&dump);
                }
            }
            [0xf0, 0, 1, 0x0c, 3, 0x0e, 0, 0xf7] => {
                self.reply_podxt(&[0xf0, 0, 1, 0x0c, 3, 0x0e, 0, 0x0f, 0xf7]);
            }
            [0xf0, 0, 1, 0x0c, 3, 0x57, id, 0xf7] => {
                let value = match *id {
                    17 => self.podxt.channel.unwrap_or(0),
                    22 => 3,
                    _ => 0,
                };
                let mut reply = vec![0xf0, 0, 1, 0x0c, 3, 0x56, *id];
                reply.extend((0..4).rev().map(|n| ((value >> (n * 4)) & 15) as u8));
                reply.push(0xf7);
                self.reply_podxt(&reply);
            }
            [0xf0, 0, 1, 0x0c, 3, 0x74, 0x0a, payload @ .., 0xf7]
                if payload.len() == TONE_BYTES =>
            {
                self.podxt.playing.copy_from_slice(payload);
            }
            [0xb0, 69, _] => {}
            [0xb0, cc, value] if *cc < 128 => {
                self.podxt.playing[32 + usize::from(*cc)] = *value;
            }
            [0xc0, program] => {
                let index = usize::from(*program);
                if index < 128 {
                    self.podxt.playing = self.podxt.stored[index];
                    self.podxt.channel = Some(*program as u16);
                }
            }
            [0xf0, 0x00, 0x01, 0x0c, 0x03, 0x71, 0x0a, hi, lo, rest @ ..]
                if rest.len() == TONE_BYTES + 1 =>
            {
                let address = (u16::from(*hi) << 7) | u16::from(*lo);
                if let Some(index) = tone_address_index(address) {
                    self.podxt.stored[index].copy_from_slice(&rest[..TONE_BYTES]);
                    self.podxt.stores += 1;
                }
            }
            [0xf0, 0x00, 0x01, 0x0c, 0x03, 0x72, 0xf7] if !self.faults.silent_podxt_store => {
                self.reply_podxt(&[0xf0, 0, 1, 0x0c, 3, 0x50, 0xf7]);
            }
            _ => {}
        }
    }

    fn variax_write(&mut self, bytes: &[u8]) {
        // Large SysEx arrives in 256-byte chunks; reassemble to F7.
        self.variax_pending.extend_from_slice(bytes);
        while let Some(end) = self.variax_pending.iter().position(|&b| b == 0xf7) {
            let message: Vec<u8> = self.variax_pending.drain(..=end).collect();
            self.variax_message(&message);
        }
        if self.variax_pending.first().is_some_and(|&b| b != 0xf0) {
            // Short channel messages (program change, CC) have no F7.
            let message = std::mem::take(&mut self.variax_pending);
            self.variax_message(&message);
        }
    }

    fn variax_message(&mut self, message: &[u8]) {
        self.written.push(("VariaxViaPodxt", message.to_vec()));
        match message {
            [0xf0, 0x7e, 0x7f, 0x06, 0x01, 0xf7] => {
                let mut reply = self.variax_identity();
                if std::mem::take(&mut self.faults.drop_first_variax_identity_packet) {
                    reply.drain(..PACKET);
                }
                self.reply_variax(&reply);
            }
            [0xf0, 0x00, 0x01, 0x0c, 0x07, 0x00, 0x2a, on, 0xf7] => {
                self.variax.session_on = *on == 1;
            }
            [0xf0, 0x00, 0x01, 0x0c, 0x07, 0x00, 0x3c, body @ .., 0xf7] if body.len() == 12 => {
                let count = nibbles(&body[..6]) as usize;
                let start = nibbles(&body[6..]) as usize;
                if count == 0 || start + count > PROGRAM_WORDS {
                    return;
                }
                let mut reply = self.variax_span_reply(start, count);
                self.span_replies += 1;
                if self.faults.damage_span_reply == Some(self.span_replies) {
                    self.faults.damage_span_reply = None;
                    reply.remove(reply.len() / 2);
                }
                self.reply_variax(&reply);
            }
            [0xf0, 0, 1, 0x0c, 7, 0, 0x6d, 0xf7] => {
                let mut reply = vec![0xf0, 0, 1, 0x0c, 7, 4, 0x6d];
                let name = self
                    .variax
                    .selected
                    .map_or("Practice", |preset| preset.bank_name());
                reply.extend(name.bytes());
                reply.resize(25, 0);
                reply.push(0xf7);
                self.reply_variax(&reply);
            }
            [0xf0, 0, 1, 0x0c, 7, 0, 0x3b, runs @ .., 0xf7] => {
                let mut runs = runs;
                while runs.len() >= 12 {
                    let count = nibbles(&runs[..6]) as usize;
                    let start = nibbles(&runs[6..12]) as usize;
                    let Some(data) = runs.get(12..12 + 6 * count) else {
                        return;
                    };
                    let Some(words) = self.variax.active.get_mut(start..start + count) else {
                        return;
                    };
                    for (word, nibbles_on_wire) in words.iter_mut().zip(data.as_chunks::<6>().0) {
                        *word = nibbles(nibbles_on_wire);
                    }
                    runs = &runs[12 + 6 * count..];
                }
                self.reply_variax(&[0xf0, 0, 1, 0x0c, 7, 4, 0x71, 0xf7]);
            }
            [0xc0, program] => {
                if let Some(preset) = preset_for_program_number(*program) {
                    self.variax.active = self.variax.stored[flash_index(preset)];
                    self.variax.selected = Some(preset);
                }
            }
            [0xb0, 79, _] | [0xb0, 7, _] => {}
            [0xf0, 0x00, 0x01, 0x0c, 0x07, 0x00, 0x43, body @ .., 0xf7] => {
                if body.len() == 6 * (PROGRAM_WORDS + 1) && nibbles(&body[..6]) as usize == 1092 {
                    for (i, chunk) in body[6..].chunks(6).enumerate() {
                        self.variax.active[i] = nibbles(chunk);
                    }
                    self.variax.working_uploads += 1;
                    if std::mem::take(&mut self.faults.change_uploaded_words_once) {
                        self.variax.active[2] ^= 1;
                        self.variax.active[680] ^= 3;
                    }
                }
            }
            [0xf0, 0x00, 0x01, 0x0c, 0x07, 0x00, 0x61, index, 0xf7] => {
                let index = usize::from(*index);
                if index < 60 {
                    for (word, version) in self.variax.active[767..771].iter_mut().zip(*b"0310") {
                        *word = u32::from(version);
                    }
                    seal(&mut self.variax.active);
                    self.variax.stored[index] = self.variax.active;
                    self.variax.stores += 1;
                    if !self.faults.silent_variax_store {
                        self.reply_variax(&[0xf0, 0, 1, 0x0c, 7, 4, 0x71, 0xf7]);
                    }
                }
            }
            _ => {}
        }
    }

    /// Schedule what the simulated person does, `after` from now.
    pub fn user_acts(&mut self, action: UserAction, after: Duration) {
        if self.faults.user_does_not_act {
            return;
        }
        let at = self.clock + after;
        match action {
            UserAction::TurnSelector(preset) => {
                if after.is_zero() {
                    self.variax.active = self.variax.stored[flash_index(preset)];
                    self.variax.selected = Some(preset);
                }
                if self.variax.connected {
                    Self::queue(&mut self.variax_out, at, &[0xc0, preset.program_number()]);
                }
            }
            UserAction::SweepVariaxKnobs => {
                let mut time = at;
                for cc in [7u8, 79] {
                    for value in [127u8, 64, 0, 64, 127] {
                        Self::queue(&mut self.variax_out, time, &[0xb0, cc, value]);
                        time += Duration::from_millis(150);
                    }
                }
            }
            UserAction::PodxtFootswitch(index) => {
                if after.is_zero() {
                    self.podxt.playing = self.podxt.stored[usize::from(index)];
                    self.podxt.channel = Some(index);
                }
                Self::queue(&mut self.podxt_out, at, &[0xc0, index as u8]);
            }
            UserAction::MovePedal => {
                let mut time = at;
                for value in [0u8, 40, 80, 127, 80, 40, 0] {
                    Self::queue(&mut self.podxt_out, time, &[0xb0, 7, value]);
                    time += Duration::from_millis(100);
                }
            }
            UserAction::SweepPodxtKnobs => {
                let sweep = || {
                    [0, 126, 127]
                        .into_iter()
                        .flat_map(|value| super::guided::PODXT_KNOBS.map(|(cc, _)| (cc, value)))
                };
                let events = self
                    .faults
                    .podxt_knob_events
                    .clone()
                    .unwrap_or_else(|| sweep().collect());
                for (index, (cc, value)) in events.into_iter().enumerate() {
                    Self::queue(
                        &mut self.podxt_out,
                        at + Duration::from_millis(index as u64),
                        &[0xb0, cc, value],
                    );
                }
            }
            UserAction::UnplugVdi => {
                self.variax_out.clear();
                // The PODxt Live reports the guitar's connection state.
                Self::queue(
                    &mut self.variax_out,
                    at,
                    &[0xf0, 0, 1, 0x0c, 3, 0x11, 0, 0, 0, 0, 0xf7],
                );
            }
            UserAction::ReplugVdi => {
                Self::queue(
                    &mut self.variax_out,
                    at,
                    &[0xf0, 0, 1, 0x0c, 3, 0x11, 0, 0, 0, 1, 0xf7],
                );
            }
        }
    }

    fn read(&mut self, podxt: bool, wait: Duration) -> Option<Vec<u8>> {
        let queue = if podxt {
            &mut self.podxt_out
        } else {
            &mut self.variax_out
        };
        let deadline = self.clock + wait;
        while queue.front().is_some_and(|head| head.at <= deadline) {
            let head = queue.pop_front().expect("front exists");
            self.clock = self.clock.max(head.at);
            match (podxt, head.packet.as_slice()) {
                (true, [0xc0, index]) => {
                    self.podxt.playing = self.podxt.stored[usize::from(*index)];
                    self.podxt.channel = Some(u16::from(*index));
                }
                (true, [0xb0, cc, value])
                    if super::guided::PODXT_KNOBS
                        .iter()
                        .any(|(knob, _)| knob == cc) =>
                {
                    self.podxt.playing[32 + usize::from(*cc)] = *value;
                }
                (false, [0xc0, program]) => {
                    if let Some(preset) = preset_for_program_number(*program) {
                        self.variax.active = self.variax.stored[flash_index(preset)];
                        self.variax.selected = Some(preset);
                    }
                }
                (false, [0xf0, 0, 1, 0xc, 3, 0x11, _, _, _, state, 0xf7]) => {
                    self.variax.connected = *state != 0;
                    if self.faults.suppress_vdi_notices {
                        continue;
                    }
                }
                _ => {}
            }
            return Some(head.packet);
        }
        self.clock = deadline;
        None
    }
}

fn nibbles(data: &[u8]) -> u32 {
    data.iter()
        .enumerate()
        .fold(0, |v, (i, &n)| v | (u32::from(n & 0xf) << (4 * i)))
}

/// One simulated link (PODxt MIDI or the guitar interface) on the shared rig.
pub struct SimLink {
    rig: Rc<RefCell<SimRig>>,
    podxt: bool,
}

impl SimLink {
    pub fn podxt(rig: &Rc<RefCell<SimRig>>) -> Self {
        Self {
            rig: rig.clone(),
            podxt: true,
        }
    }

    pub fn variax(rig: &Rc<RefCell<SimRig>>) -> Self {
        Self {
            rig: rig.clone(),
            podxt: false,
        }
    }
}

impl Link for SimLink {
    fn path(&self) -> Path {
        if self.podxt {
            Path::PodxtMidi
        } else {
            Path::VariaxViaPodxt
        }
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        let mut rig = self.rig.borrow_mut();
        rig.clock += Duration::from_micros(200);
        if self.podxt {
            rig.podxt_write(bytes);
        } else {
            rig.variax_write(bytes);
        }
        Ok(())
    }

    fn read(&mut self, wait: Duration) -> Result<Option<Vec<u8>>, String> {
        let mut rig = self.rig.borrow_mut();
        #[cfg(test)]
        if !self.podxt && rig.faults.fail_variax_read {
            return Err("Simulated Variax transport read failed.".into());
        }
        Ok(rig.read(self.podxt, wait))
    }

    fn elapsed(&self) -> Duration {
        self.rig.borrow().clock
    }

    fn pause(&mut self, duration: Duration) {
        self.rig.borrow_mut().clock += duration;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use line6_devices::family::{podxt, variax};
    use line6_devices::gate::{GatePolicy, Operation};
    use line6_devices::session::{Quiet, Session};

    struct All;
    impl GatePolicy for All {
        fn allows(&self, _: Operation) -> bool {
            true
        }
    }

    #[test]
    fn the_library_reads_both_simulated_devices() {
        let rig = SimRig::shared(Faults::default());
        let mut log = Quiet;
        let mut session = Session::new(SimLink::podxt(&rig), &All, &mut log);
        let tone = podxt::read_current(&mut session).unwrap();
        assert_eq!(tone.identity.version_text(), "3.01");
        assert_eq!(tone.tone.name(), "Practice 5B");
        let mut log = Quiet;
        let mut session = Session::new(SimLink::variax(&rig), &All, &mut log);
        let identified = variax::identify(&mut session).unwrap();
        assert_eq!(identified.identity.version_text(), "3.10");
        assert_eq!(identified.requests, 1);
    }

    #[test]
    fn a_lost_identity_packet_is_resent_once() {
        let rig = SimRig::shared(Faults {
            drop_first_variax_identity_packet: true,
            ..Faults::default()
        });
        let mut log = Quiet;
        let mut session = Session::new(SimLink::variax(&rig), &All, &mut log);
        assert_eq!(variax::identify(&mut session).unwrap().requests, 2);
    }

    #[test]
    fn synthetic_programs_are_valid_workbench_programs() {
        let words = synthetic_program("Spank 1", 5);
        let bytes = packed(&words);
        assert!(variax::write::Program::new("Spank 1", &bytes).is_ok());
        assert_eq!(program_name(&words), "Spank 1");
        for preset in Preset::all() {
            assert_eq!(
                preset_for_program_number(preset.program_number()),
                Some(preset)
            );
        }
    }
}
