use super::super::transfer::tests::{events, tone};
use super::*;
use crate::gate::{AllClosed, GatePolicy};
use crate::identity::tests::PODXT_LIVE;
use crate::session::Quiet;
use crate::session::testing::{AllowAll, Fake};

const DAY: u64 = 20_781;

fn identity() -> Vec<u8> {
    let mut identity = PODXT_LIVE.to_vec();
    identity[12..16].copy_from_slice(&[0, 3, 0, 1]);
    identity
}

fn slot(index: u16) -> Location {
    Location::new(BankKind::Tones, index).unwrap()
}

fn plan(slot: Location) -> ConfirmedStore {
    PreparedStore {
        slot,
        day: DAY,
        original: tone(),
        candidate: tone(),
    }
    .confirm(&format!("STORE PODXT {}", slot.label()))
    .unwrap()
}

type Script = Vec<(Vec<u8>, Vec<Vec<u8>>)>;

fn preflight(slot: Location) -> Script {
    vec![
        (PodxtRequest::Identity.bytes(), vec![identity()]),
        (
            PodxtRequest::Stored(slot).bytes(),
            vec![tone().edit_dump().raw],
        ),
    ]
}

fn replay(case: &str, slot: Location, readback: Option<EditBuffer>) -> Script {
    let mut script = preflight(slot);
    let frames = events(case, "out ");
    let replies = events(case, "in ");
    script.push((frames[0].clone(), vec![]));
    script.push((frames[1].clone(), replies));
    if let Some(dump) = readback {
        script.push((PodxtRequest::Stored(slot).bytes(), vec![dump.raw]));
    }
    script
}

#[test]
fn store_and_block_end_match_original_transcripts_at_three_slot_addresses() {
    for (case, index) in [("ok-16d", 63), ("ok-17a", 64), ("ok-32d", 127)] {
        let slot = slot(index);
        let script = replay(case, slot, Some(tone().edit_dump()));
        let control = StoreControl::default();
        let mut quiet = Quiet;
        let mut session = Session::new(
            Fake::from_bytes(Path::PodxtMidi, script),
            &AllowAll,
            &mut quiet,
        );
        let result = store_at(&mut session, plan(slot), &control, || Ok(DAY)).unwrap();
        let StoreResult::Verified {
            slot: actual,
            tone: echoed,
        } = result
        else {
            panic!("expected verified readback: {result:?}");
        };
        assert_eq!(actual, slot);
        assert_eq!(echoed.raw, tone().edit_dump().raw);
        assert!(control.started());
        assert!(!control.cancel());
        session.link.assert_finished();
    }
}

#[test]
fn negative_reply_silence_and_readback_mismatch_report_slot_state_unknown() {
    let slot = slot(64);
    let control = StoreControl::default;
    for (case, expected) in [("fail", "03 51"), ("timeout", "within 7 seconds")] {
        let mut quiet = Quiet;
        let mut session = Session::new(
            Fake::from_bytes(Path::PodxtMidi, replay(case, slot, None)),
            &AllowAll,
            &mut quiet,
        );
        let result = store_at(&mut session, plan(slot), &control(), || Ok(DAY)).unwrap();
        assert!(
            matches!(result, StoreResult::SlotStateUnknown { reason, .. } if reason.contains(expected))
        );
        session.link.assert_finished();
    }
    let mut quiet = Quiet;
    let mut mismatch = tone().edit_dump();
    mismatch.raw[39 + 13] = 23;
    let mut session = Session::new(
        Fake::from_bytes(Path::PodxtMidi, replay("ok-17a", slot, Some(mismatch))),
        &AllowAll,
        &mut quiet,
    );
    let result = store_at(&mut session, plan(slot), &control(), || Ok(DAY)).unwrap();
    assert!(
        matches!(result, StoreResult::SlotStateUnknown { reason, .. } if reason.contains("byte for byte"))
    );
    session.link.assert_finished();
}

#[test]
fn typed_confirmation_closed_policy_and_changed_backup_day_stop_before_store() {
    let slot = slot(64);
    let prepared = plan(slot).0;
    assert!(prepared.confirm("STORE PODXT 17B").is_err());

    let mut quiet = Quiet;
    let mut session = Session::new(
        Fake::from_bytes(Path::PodxtMidi, vec![]),
        &AllClosed,
        &mut quiet,
    );
    let control = StoreControl::default();
    assert!(
        store_at(&mut session, plan(slot), &control, || Ok(DAY))
            .unwrap_err()
            .contains("closed")
    );
    assert!(!control.started());
    assert!(session.link.writes.is_empty());

    let mut session = Session::new(
        Fake::from_bytes(Path::PodxtMidi, preflight(slot)),
        &AllowAll,
        &mut quiet,
    );
    let control = StoreControl::default();
    assert!(
        store_at(&mut session, plan(slot), &control, || Ok(DAY + 1))
            .unwrap_err()
            .contains("UTC day")
    );
    assert!(!control.started());
    session.link.assert_finished();
}

#[test]
fn cancellation_immediately_before_dispatch_prevents_both_write_frames() {
    let slot = slot(64);
    let mut quiet = Quiet;
    let mut session = Session::new(
        Fake::from_bytes(Path::PodxtMidi, preflight(slot)),
        &AllowAll,
        &mut quiet,
    );
    let control = StoreControl::default();
    let result = store_at(&mut session, plan(slot), &control, || {
        assert!(control.cancel());
        Ok(DAY)
    });
    assert!(result.unwrap_err().contains("cancelled"));
    assert!(!control.started());
    session.link.assert_finished();
}

#[test]
fn first_transport_error_is_indeterminate_and_is_never_retried() {
    let slot = slot(64);
    let mut script = preflight(slot);
    script.push((events("fail", "out ").remove(0), vec![]));
    let mut link = Fake::from_bytes(Path::PodxtMidi, script);
    link.fail_at.push(2);
    let mut quiet = Quiet;
    let mut session = Session::new(link, &AllowAll, &mut quiet);
    let result = store_at(&mut session, plan(slot), &StoreControl::default(), || {
        Ok(DAY)
    })
    .unwrap();
    assert!(
        matches!(result, StoreResult::SlotStateUnknown { reason, .. } if reason.contains("simulated write failure"))
    );
    session.link.assert_finished();
}

struct BusyInbox(Duration);

impl Link for BusyInbox {
    fn path(&self) -> Path {
        Path::PodxtMidi
    }
    fn write(&mut self, _: &[u8]) -> Result<(), String> {
        panic!("wait must never write")
    }
    fn read(&mut self, wait: Duration) -> Result<Option<Vec<u8>>, String> {
        self.pause(wait);
        // A foreign ACK with the same status and unrelated PODxt Clip SysEx.
        Ok(Some(vec![
            0xf0, 0, 1, 0x0c, 7, 0x50, 0xf7, 0xf0, 0, 1, 0x0c, 3, 0x0f, 0xf7,
        ]))
    }
    fn elapsed(&self) -> Duration {
        self.0
    }
    fn pause(&mut self, duration: Duration) {
        self.0 += duration;
    }
}

#[test]
fn unrelated_frames_cannot_extend_the_seven_second_deadline() {
    let mut quiet = Quiet;
    let mut session = Session::new(BusyInbox(Duration::ZERO), &AllClosed, &mut quiet);
    assert!(
        wait_for_store(&mut session)
            .unwrap_err()
            .contains("within 7 seconds")
    );
    assert_eq!(session.link.elapsed(), STORE_WAIT);
}

#[test]
fn stale_destination_and_unavailable_readback_gate_prevent_dispatch() {
    let slot = slot(64);
    let mut script = preflight(slot);
    script[1].1[0][39 + 13] = 32;
    let mut quiet = Quiet;
    let mut session = Session::new(
        Fake::from_bytes(Path::PodxtMidi, script),
        &AllowAll,
        &mut quiet,
    );
    assert!(
        store_at(&mut session, plan(slot), &StoreControl::default(), || Ok(
            DAY
        ))
        .unwrap_err()
        .contains("differs")
    );
    session.link.assert_finished();

    struct NoLibrary;
    impl GatePolicy for NoLibrary {
        fn allows(&self, op: Operation) -> bool {
            op != Operation::PodxtLibraryRead
        }
    }
    let mut script = preflight(slot);
    script.truncate(1);
    let mut session = Session::new(
        Fake::from_bytes(Path::PodxtMidi, script),
        &NoLibrary,
        &mut quiet,
    );
    assert!(
        store_at(&mut session, plan(slot), &StoreControl::default(), || Ok(
            DAY
        ))
        .unwrap_err()
        .contains("PodxtLibraryRead")
    );
    session.link.assert_finished();
}

#[test]
fn setup_slots_use_their_own_addresses_and_the_same_store_transaction() {
    for (kind, index, address, label) in [
        (BankKind::AmpSetups, 0, [0, 64], "AMP 1"),
        (BankKind::AmpSetups, 64, [2, 0], "AMP 65"),
        (BankKind::AmpSetups, 106, [2, 42], "AMP 107"),
        (BankKind::FxSetups, 63, [1, 63], "FX 64"),
    ] {
        let slot = Location::new(kind, index).unwrap();
        let original = tone();
        let mut bytes = *original.as_bytes();
        bytes[32 + 13] = 27;
        let candidate = TonePayload::from_bytes(bytes).unwrap();
        let prepared = PreparedStore {
            slot,
            day: DAY,
            original,
            candidate: candidate.clone(),
        };
        assert_eq!(prepared.confirmation(), format!("STORE PODXT {label}"));
        let confirm = prepared.confirm(&format!("STORE PODXT {label}")).unwrap();
        let mut script = preflight(slot);
        let mut payload = vec![0xf0, 0, 1, 0x0c, 3, 0x71, 0x0a, address[0], address[1]];
        payload.extend(bytes);
        payload.push(0xf7);
        script.extend([
            (payload, vec![]),
            (
                vec![0xf0, 0, 1, 0x0c, 3, 0x72, 0xf7],
                vec![vec![0xf0, 0, 1, 0x0c, 3, 0x50, 0xf7]],
            ),
            (
                PodxtRequest::Stored(slot).bytes(),
                vec![candidate.edit_dump().raw],
            ),
        ]);
        let mut quiet = Quiet;
        let mut session = Session::new(
            Fake::from_bytes(Path::PodxtMidi, script),
            &AllowAll,
            &mut quiet,
        );
        let result = store_at(&mut session, confirm, &StoreControl::default(), || Ok(DAY)).unwrap();
        assert!(matches!(result, StoreResult::Verified { slot: stored, .. } if stored == slot));
        session.link.assert_finished();
    }
}
