use super::*;
use crate::{
    family::podxt::{PODXT_LIVE_VERSION, PodxtRequest},
    identity::tests::PODXT_LIVE,
    session::{
        Path, Quiet,
        testing::{AllowAll, Fake, packets},
    },
};

fn identity(version: [u8; 4]) -> Vec<u8> {
    let mut raw = PODXT_LIVE.to_vec();
    raw[12..16].copy_from_slice(&version);
    raw
}

#[test]
fn closed_approval_refuses_before_identity() {
    let mut log = Quiet;
    let fake = Fake::new::<PodxtRequest>(Path::PodxtMidi, vec![]);
    let mut session = Session::new(fake, &AllowAll, &mut log);
    assert!(
        read(&mut session, || false, |_, _| panic!("unexpected reply"))
            .unwrap_err()
            .contains("no owner approval")
    );
    assert!(session.sent.is_empty());
    session.link.assert_finished();
}

#[test]
fn both_versions_read_the_current_tone_and_every_slot_without_selecting() {
    let tone = super::super::tests::synthetic();
    for version in [PODXT_LIVE_VERSION, [0, 2, 0, 7]] {
        let locations: Vec<_> = Location::all(&BankKind::ALL).collect();
        let mut script = vec![
            (PodxtRequest::Identity, packets(&identity(version))),
            (PodxtRequest::EditBuffer, packets(&tone)),
        ];
        script.extend(
            locations
                .iter()
                .map(|&loc| (PodxtRequest::Stored(loc), packets(&tone))),
        );
        script.push((PodxtRequest::EditBuffer, packets(&tone)));
        let mut log = Quiet;
        let mut session = Session::new(Fake::new(Path::PodxtMidi, script), &AllowAll, &mut log);
        let mut observed = Vec::new();
        read_on(&mut session, || false, |kind, _| observed.push(kind)).unwrap();
        assert_eq!(observed[0..2], [Reply::Identity, Reply::Current]);
        assert_eq!(observed.last(), Some(&Reply::Closing));
        assert_eq!(
            observed[2..301],
            locations.into_iter().map(Reply::Stored).collect::<Vec<_>>()
        );
        session.link.assert_finished();
    }
}

#[test]
fn malformed_or_silent_tone_stops_the_sweep_after_one_request() {
    let invalid = vec![0xf0, 0, 1, 0x0c, 3, 0x74, 0x0a, 0xf7];
    for (reply, captured) in [(vec![invalid], true), (vec![], false)] {
        let fake = Fake::new(
            Path::PodxtMidi,
            vec![
                (PodxtRequest::Identity, packets(&identity([0, 2, 0, 7]))),
                (PodxtRequest::EditBuffer, reply),
            ],
        );
        let mut log = Quiet;
        let mut session = Session::new(fake, &AllowAll, &mut log);
        let mut observed = Vec::new();
        assert!(
            read_on(
                &mut session,
                || false,
                |kind, data| observed.push((kind, data.len()))
            )
            .is_err()
        );
        assert_eq!(observed[0], (Reply::Identity, 17));
        assert_eq!(
            observed.get(1).copied(),
            captured.then_some((Reply::Current, 8))
        );
        assert_eq!(session.sent.len(), 2);
        session.link.assert_finished();
    }
}

#[test]
fn closed_policy_and_malformed_identity_send_no_tone_request() {
    let mut quiet = Quiet;
    let mut closed = Session::new(
        Fake::new::<PodxtRequest>(Path::PodxtMidi, vec![]),
        &crate::gate::AllClosed,
        &mut quiet,
    );
    assert!(
        read_on(&mut closed, || false, |_, _| {})
            .unwrap_err()
            .contains("permissions")
    );
    assert!(closed.sent.is_empty());

    let script = vec![(PodxtRequest::Identity, packets(&identity([0, 2, 0, 0x7f])))];
    let mut log = Quiet;
    let mut session = Session::new(Fake::new(Path::PodxtMidi, script), &AllowAll, &mut log);
    assert!(
        read_on(&mut session, || false, |_, _| {})
            .unwrap_err()
            .contains("numeric")
    );
    session.link.assert_finished();
}

#[test]
fn unexpected_reply_during_stored_settle_stops_before_the_next_slot() {
    let tone = super::super::tests::synthetic();
    let mut stored = packets(&tone);
    stored.push(vec![0xf0, 0, 1, 0x0c, 3, 0x50, 0xf7]);
    let slot = Location::tone_from_label("1A").unwrap();
    let script = vec![
        (PodxtRequest::Identity, packets(&identity([0, 2, 0, 7]))),
        (PodxtRequest::EditBuffer, packets(&tone)),
        (PodxtRequest::Stored(slot), stored),
    ];
    let mut log = Quiet;
    let mut session = Session::new(Fake::new(Path::PodxtMidi, script), &AllowAll, &mut log);
    let mut observed = Vec::new();
    let error = read_on(&mut session, || false, |kind, _| observed.push(kind)).unwrap_err();
    assert!(error.contains("Unexpected SysEx"));
    assert_eq!(
        observed,
        [Reply::Identity, Reply::Current, Reply::Stored(slot)]
    );
    session.link.assert_finished();
}
