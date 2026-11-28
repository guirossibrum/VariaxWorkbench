//! Passive PODxt Live input shared by Variax presence and channel Follow.
#[cfg(any(test, feature = "usb"))]
use line6_devices::family::podxt::Location;
#[cfg(feature = "usb")]
use line6_devices::family::podxt::StoredTone;
#[cfg(any(test, all(feature = "usb", target_os = "linux")))]
use line6_devices::{
    family::podxt::save_notification,
    midi::{Assembler, Event},
};
use std::time::Instant;
#[cfg(feature = "usb")]
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, Weak},
};
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

/// Save evidence is shared by the listener and the editor for one USB
/// attachment. A replacement attachment always gets a new instance.
#[cfg(feature = "usb")]
#[derive(Debug, Default)]
pub(in crate::editor) struct PodxtSaveIngress(Mutex<SaveState>);

#[cfg(feature = "usb")]
#[derive(Debug, Default)]
struct SaveState {
    revision: u64,
    last_gap: u64,
    listener: Option<u64>,
    slots: BTreeMap<u16, u64>,
}

// Workbench's current foreground reads leave a gap in save observation. The
// editor can use this API once its addressed reader shares an uninterrupted
// save observer.
#[cfg(feature = "usb")]
#[allow(dead_code)]
pub(in crate::editor) struct SavedSlotRefresh {
    ingress: Weak<PodxtSaveIngress>,
    window: u64,
    slot: Location,
    last_save: Option<u64>,
}

#[cfg(feature = "usb")]
#[allow(dead_code)]
pub(in crate::editor) struct FreshSavedSlot {
    refresh: SavedSlotRefresh,
    stored: StoredTone,
}

#[cfg(feature = "usb")]
#[allow(dead_code)]
impl SavedSlotRefresh {
    fn matches(&self, ingress: &PodxtSaveIngress, slot: Location) -> bool {
        if !std::ptr::eq(ingress, self.ingress.as_ptr()) || self.slot != slot {
            return false;
        }
        let state = ingress.0.lock().expect("PODxt save ingress");
        state.observes(self.window) && state.slots.get(&slot.address()).copied() == self.last_save
    }
}

#[cfg(feature = "usb")]
impl PodxtSaveIngress {
    pub fn begin(self: &Arc<Self>) -> SaveListener {
        let mut state = self.0.lock().expect("PODxt save ingress");
        state.revision += 1;
        state.last_gap = state.revision;
        state.listener = Some(state.last_gap);
        SaveListener {
            ingress: Arc::clone(self),
            window: state.last_gap,
        }
    }

    pub fn interrupt(&self) {
        let mut state = self.0.lock().expect("PODxt save ingress");
        if let Some(window) = state.listener.take() {
            state.gap(window);
        }
    }

    pub fn revision(&self) -> u64 {
        self.0.lock().expect("PODxt save ingress").revision
    }

    /// An observed save can supersede a download even when that download
    /// closes cleanly. A quiet interval supplies no positive freshness proof.
    pub fn saved_since(&self, revision: u64) -> bool {
        self.0
            .lock()
            .expect("PODxt save ingress")
            .slots
            .values()
            .any(|&saved| saved > revision)
    }

    /// Accept the result of the exact checked stored-slot request started
    /// after capturing `refresh`. The reader must use the same verified link
    /// and forward save notices through the reply and settle interval before
    /// passing the result here. A failed, missing or ambiguous reply supplies
    /// no result.
    #[allow(dead_code)]
    pub fn reconcile_saved_slot(
        &self,
        refresh: SavedSlotRefresh,
        stored: StoredTone,
    ) -> Option<FreshSavedSlot> {
        refresh
            .matches(self, stored.location)
            .then_some(FreshSavedSlot { refresh, stored })
    }

    /// Revalidate at consumption time. Keep the proof with these exact bytes.
    /// This check does not reserve the link or exclude subsequent saves.
    #[allow(dead_code)]
    pub fn fresh_saved_slot<'a>(
        &self,
        slot: Location,
        fresh: &'a FreshSavedSlot,
    ) -> Option<&'a StoredTone> {
        fresh.refresh.matches(self, slot).then_some(&fresh.stored)
    }
}

#[cfg(feature = "usb")]
impl SaveState {
    fn observes(&self, window: u64) -> bool {
        self.listener == Some(window) && self.last_gap == window
    }

    fn gap(&mut self, window: u64) {
        if window == self.last_gap {
            self.revision += 1;
            self.last_gap = self.revision;
        }
    }
}

#[cfg(feature = "usb")]
pub(in crate::editor) struct SaveListener {
    ingress: Arc<PodxtSaveIngress>,
    window: u64,
}

#[cfg(feature = "usb")]
impl SaveListener {
    pub fn observe(&self, slot: Location) {
        let mut state = self.ingress.0.lock().expect("PODxt save ingress");
        if state.observes(self.window) {
            state.revision += 1;
            let revision = state.revision;
            state.slots.insert(slot.address(), revision);
        }
    }

    /// Capture the window and slot epoch before sending the addressed read.
    /// Only call with an open, caught-up save observer on the read's verified
    /// attachment. End the window on any coverage gap, ambiguous input or
    /// write whose effects are not covered by save observation.
    #[allow(dead_code)]
    pub fn begin_saved_slot_refresh(&self, slot: Location) -> Option<SavedSlotRefresh> {
        let state = self.ingress.0.lock().expect("PODxt save ingress");
        state.observes(self.window).then(|| SavedSlotRefresh {
            ingress: Arc::downgrade(&self.ingress),
            window: self.window,
            slot,
            last_save: state.slots.get(&slot.address()).copied(),
        })
    }

    /// A checked playing-tone result can finish after passive coverage ends,
    /// but must be discarded after the attachment observation is revoked.
    pub fn is_current(&self) -> bool {
        self.ingress.0.lock().expect("PODxt save ingress").listener == Some(self.window)
    }

    pub fn gap(&self) {
        self.ingress
            .0
            .lock()
            .expect("PODxt save ingress")
            .gap(self.window);
    }
}

#[cfg(feature = "usb")]
impl Drop for SaveListener {
    fn drop(&mut self) {
        self.gap();
    }
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
struct NoticeStream<F> {
    sysex: SysexDecoder,
    midi: Assembler,
    save: F,
}

#[cfg(any(test, all(feature = "usb", target_os = "linux")))]
impl<F: FnMut(Location)> NoticeStream<F> {
    fn new(save: F) -> Self {
        Self {
            sysex: SysexDecoder::default(),
            midi: Assembler::default(),
            save,
        }
    }

    fn receive(&mut self, packet: &[u8], notices: &mut Vec<Notice>, mut log: impl FnMut(&str)) {
        // Keep the Variax decoder's existing malformed-packet handling.
        // Assemble PODxt program changes and addressed saves from the same bytes.
        if let Ok(events) = self.midi.feed(packet) {
            let observed_at = Instant::now();
            notices.extend(events.into_iter().filter_map(|event| {
                let message = match event {
                    Event::Short(message) => message,
                    Event::Sysex(frame) => {
                        if let Some(slot) = save_notification(&frame) {
                            (self.save)(slot);
                        }
                        return None;
                    }
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

    fn finish(self, mut log: impl FnMut(&str)) {
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
    save: impl FnMut(Location),
    mut log: impl FnMut(&str),
) -> Vec<Notice> {
    let mut stream = NoticeStream::new(save);
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

    #[cfg(feature = "usb")]
    fn simulated_stored(slot: Location) -> StoredTone {
        use line6_devices::family::podxt::EditBuffer;

        // The dump itself is untagged. The checked reader supplies the
        // requested location after matching the reply to that exchange.
        let mut bytes = vec![0; 168];
        bytes[..6].copy_from_slice(&[0xf0, 0, 1, 12, 3, 0x74]);
        bytes[7..15].copy_from_slice(b"New tone");
        bytes[167] = 0xf7;
        StoredTone {
            location: slot,
            tone: EditBuffer::parse(bytes).unwrap(),
        }
    }

    #[cfg(feature = "usb")]
    #[test]
    fn addressed_refresh_after_a_gap_restores_only_its_exact_slot() {
        use line6_devices::family::podxt::BankKind;

        let ingress = Arc::new(PodxtSaveIngress::default());
        let slot = Location::new(BankKind::Tones, 5).unwrap();
        let setup = Location::new(BankKind::AmpSetups, 5).unwrap();
        let original = ingress.begin();
        let stale_read = original.begin_saved_slot_refresh(slot).unwrap();
        original.gap();
        assert!(original.begin_saved_slot_refresh(slot).is_none());

        let restored = ingress.begin();
        assert!(
            ingress
                .reconcile_saved_slot(stale_read, simulated_stored(slot))
                .is_none()
        );
        let refresh = restored.begin_saved_slot_refresh(slot).unwrap();
        assert!(
            ingress
                .reconcile_saved_slot(refresh, simulated_stored(setup))
                .is_none()
        );

        restored.observe(slot);
        let refresh = restored.begin_saved_slot_refresh(slot).unwrap();
        let fresh = ingress
            .reconcile_saved_slot(refresh, simulated_stored(slot))
            .unwrap();
        assert_eq!(
            ingress.fresh_saved_slot(slot, &fresh).unwrap().tone.name(),
            "New tone"
        );
        assert!(ingress.fresh_saved_slot(setup, &fresh).is_none());

        drop(restored);
        assert!(ingress.fresh_saved_slot(slot, &fresh).is_none());
        let _reopened = ingress.begin();
        assert!(ingress.fresh_saved_slot(slot, &fresh).is_none());
    }

    #[cfg(feature = "usb")]
    #[test]
    fn interleaved_save_packets_reject_same_slot_and_keep_other_bank_refresh() {
        use line6_devices::family::podxt::BankKind;

        let ingress = Arc::new(PodxtSaveIngress::default());
        let listener = ingress.begin();
        let slot = Location::new(BankKind::Tones, 5).unwrap();
        let setup = Location::new(BankKind::AmpSetups, 5).unwrap();
        let mut stream = NoticeStream::new(|saved| listener.observe(saved));
        let mut receive = |packet| stream.receive(packet, &mut Vec::new(), |e| panic!("{e}"));

        let slot_read = listener.begin_saved_slot_refresh(slot).unwrap();
        let setup_read = listener.begin_saved_slot_refresh(setup).unwrap();
        receive(&[0xf0, 0, 1, 12, 3, 0x24, 1]);
        receive(&[5, 0xf7]);
        assert!(
            ingress
                .reconcile_saved_slot(slot_read, simulated_stored(slot))
                .is_none()
        );
        let setup_fresh = ingress
            .reconcile_saved_slot(setup_read, simulated_stored(setup))
            .unwrap();

        let retry = listener.begin_saved_slot_refresh(slot).unwrap();
        let fresh = ingress
            .reconcile_saved_slot(retry, simulated_stored(slot))
            .unwrap();
        receive(&[0xf0, 0, 1, 12, 3, 0x24, 1, 5, 0xf7]);
        assert!(ingress.fresh_saved_slot(slot, &fresh).is_none());
        assert!(ingress.fresh_saved_slot(setup, &setup_fresh).is_some());
    }

    #[cfg(feature = "usb")]
    #[test]
    fn equal_epochs_on_different_attachments_never_match() {
        let original = Arc::new(PodxtSaveIngress::default());
        let replacement = Arc::new(PodxtSaveIngress::default());
        let listener = original.begin();
        let _replacement_listener = replacement.begin();
        assert_eq!(original.revision(), replacement.revision());
        let slot = Location::tone_from_label("32D").unwrap();
        let refresh = listener.begin_saved_slot_refresh(slot).unwrap();
        assert!(
            replacement
                .reconcile_saved_slot(refresh, simulated_stored(slot))
                .is_none()
        );

        let refresh = listener.begin_saved_slot_refresh(slot).unwrap();
        let fresh = original
            .reconcile_saved_slot(refresh, simulated_stored(slot))
            .unwrap();
        assert!(replacement.fresh_saved_slot(slot, &fresh).is_none());
        original.interrupt();
        assert!(original.fresh_saved_slot(slot, &fresh).is_none());
    }

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
        assert!(
            read_notices(
                &mut read,
                || true,
                |_| panic!("yield before save"),
                |_| panic!("yield before reading")
            )
            .is_empty()
        );
        assert_eq!(
            read_notices(
                read,
                || false,
                |_| panic!("unexpected save"),
                |line| logged.push(line.to_owned())
            ),
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
            |_| panic!("unexpected save"),
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
    fn linux_save_ingress_keeps_all_addresses_and_occurrences_in_order() {
        use line6_devices::family::podxt::BankKind;

        let save = |bank, slot| vec![0xf0, 0, 1, 12, 3, 0x24, bank, slot, 0xf7];
        let tone = save(1, 127);
        let mut next = tone[7..].to_vec();
        next.extend(save(2, 106));
        next.extend(save(3, 63));
        next.extend(save(2, 107)); // outside the amp bank
        next.extend(save(0, 0)); // invalid bank
        next.extend(save(3, 64)); // outside the FX bank
        next.extend(save(1, 127)); // a second save to the same address
        next.extend([0xc0, 127]);
        let mut packets = std::collections::VecDeque::from([tone[..7].to_vec(), next]);
        let mut saved = Vec::new();
        let result = read_notices(
            |_| Ok(packets.pop_front()),
            || false,
            |slot| saved.push((slot.kind(), slot.index())),
            |error| panic!("{error}"),
        );
        assert!(matches!(result.as_slice(), [Notice::PodxtProgram(_)]));
        assert_eq!(
            saved,
            [
                (BankKind::Tones, 127),
                (BankKind::AmpSetups, 106),
                (BankKind::FxSetups, 63),
                (BankKind::Tones, 127),
            ]
        );
    }

    #[cfg(feature = "usb")]
    #[test]
    fn ending_a_listener_records_a_gap_and_rejects_late_notices() {
        use line6_devices::family::podxt::BankKind;

        let ingress = Arc::new(PodxtSaveIngress::default());
        let slot = Location::new(BankKind::Tones, 5).unwrap();
        let old = ingress.begin();
        let before_gap = ingress.revision();
        old.gap();
        let after_gap = ingress.revision();
        assert!(after_gap > before_gap);
        assert!(!ingress.saved_since(before_gap));
        old.observe(slot);
        assert_eq!(ingress.revision(), after_gap);

        let new = ingress.begin();
        let baseline = ingress.revision();
        assert!(!old.is_current());
        drop(old);
        assert_eq!(ingress.revision(), baseline);
        new.observe(slot);
        assert!(ingress.saved_since(baseline));

        ingress.interrupt();
        assert!(!new.is_current());
        assert!(!ingress.saved_since(ingress.revision()));
    }

    #[test]
    fn fragmented_notices_keep_the_latest_state_and_ignore_malformed_or_unknown_frames() {
        let connected = [0xf0, 0, 1, 12, 3, 0x11, 0, 0, 0, 7, 0xf7];
        let mut input = NoticeStream::new(|_| panic!("unexpected save"));
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
