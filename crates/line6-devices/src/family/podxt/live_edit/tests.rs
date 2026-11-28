use super::*;
use crate::gate::{AllClosed, GatePolicy};
use crate::identity::tests::PODXT_LIVE;
use crate::session::Quiet;
use crate::session::testing::{AllowAll, Fake};

type Script = Vec<(Vec<u8>, Vec<Vec<u8>>)>;

fn frame(values: &[(u8, u8)]) -> EditBuffer {
    let mut raw = vec![0; EDIT_BUFFER_LEN];
    raw[..7].copy_from_slice(&[0xf0, 0, 1, 0x0c, 3, 0x74, 0x0a]);
    raw[7..17].copy_from_slice(b"Live tones");
    raw[39 + 101] = 13; // Linked Variax family. Must survive all transfers.
    raw[39 + 120] = 23; // Linked Variax model. No wrapped messages are allowed.
    for &(cc, value) in values {
        raw[39 + usize::from(cc)] = value;
    }
    raw[167] = 0xf7;
    EditBuffer::parse(raw).unwrap()
}

fn buffer(values: &[(u8, u8)]) -> EditorBuffer {
    EditorBuffer {
        channel: Location::new(BankKind::Tones, 17).unwrap(),
        tone: frame(values),
        packs: Packs(0x04),
    }
}

fn reply(request: PodxtRequest, response: Vec<u8>) -> (Vec<u8>, Vec<Vec<u8>>) {
    (request.bytes(), vec![response])
}

fn identity(mut version: [u8; 4]) -> (Vec<u8>, Vec<Vec<u8>>) {
    let mut message = PODXT_LIVE.to_vec();
    message[12..16].swap_with_slice(&mut version);
    reply(PodxtRequest::Identity, message)
}

fn channel(index: u8) -> (Vec<u8>, Vec<Vec<u8>>) {
    reply(
        PodxtRequest::QueryGlobal(Global::CurrentChannel),
        vec![
            0xf0,
            0,
            1,
            0x0c,
            3,
            0x56,
            17,
            0,
            0,
            index >> 4,
            index & 15,
            0xf7,
        ],
    )
}

fn dump(tone: &EditBuffer) -> (Vec<u8>, Vec<Vec<u8>>) {
    reply(PodxtRequest::EditBuffer, tone.raw.clone())
}

fn packs(mask: u8) -> (Vec<u8>, Vec<Vec<u8>>) {
    reply(
        PodxtRequest::InstalledPacks,
        vec![0xf0, 0, 1, 0x0c, 3, 0x0e, 0x0a, mask, 0xf7],
    )
}

fn begin(before: &EditBuffer) -> Script {
    vec![identity([0, 3, 0, 1]), channel(17), dump(before)]
}

fn cc(number: u8, value: u8) -> (Vec<u8>, Vec<Vec<u8>>) {
    (vec![0xb0, number, value], vec![])
}

fn basic(control: Control, value: u8) -> LiveTarget {
    LiveTarget::Basic(control, Target::Value(Value::new(value).unwrap()))
}

fn change(setting: Setting, value: u16) -> LiveTarget {
    LiveTarget::Additional(setting, value)
}

#[test]
fn get_snapshots_before_any_writes_and_requires_live_301() {
    let original = buffer(&[(13, 42)]);
    let mut script = begin(&original.tone);
    script.extend([packs(4), channel(17)]);
    let mut log = Quiet;
    let mut s = Session::new(
        Fake::from_bytes(Path::PodxtMidi, script),
        &AllowAll,
        &mut log,
    );
    let got = get_editor_buffer(&mut s, || false).unwrap();
    assert_eq!(got.tone, original.tone);
    assert_eq!(got.channel, original.channel);
    assert_eq!(got.packs, Packs(4));
    s.link.assert_finished();

    let script = vec![identity([0, 2, 0, 7])];
    let mut s = Session::new(
        Fake::from_bytes(Path::PodxtMidi, script),
        &AllowAll,
        &mut log,
    );
    assert!(
        get_editor_buffer(&mut s, || false)
            .unwrap_err()
            .contains("3.01")
    );
    s.link.assert_finished();
}

#[test]
fn live_edits_match_encoder_order_and_amp_defaults_stay_off() {
    // Line 6 Edit 3.06 EncodeParamChange, also covered by the #210 oracle.
    // Amp CC12 without defaults, inverted amp enable, NOTE off, high/low time.
    let before = buffer(&[(29, 5), (51, 4), (111, 127)]);
    let after = frame(&[(12, 22), (16, 64), (29, 32), (61, 1), (111, 0)]);
    let mut script = begin(&before.tone);
    script.extend([
        packs(4),
        channel(17),
        cc(12, 22),
        cc(51, 0),
        cc(29, 32),
        cc(61, 1),
        cc(111, 0),
        cc(16, 64),
        dump(&after),
        channel(17),
    ]);
    let targets = [
        change(Setting::AmpModel, 22),
        change(Setting::ModSpeed, 4097),
        basic(Control::AmpEnable, 0),
        basic(Control::Treble, 64),
        change(Setting::ModNote, 0),
    ];
    let mut log = Quiet;
    let mut s = Session::new(
        Fake::from_bytes(Path::PodxtMidi, script),
        &AllowAll,
        &mut log,
    );
    let got =
        edit_editor_buffer(&mut s, &before, EditorEdit::Controls(&targets), || false).unwrap();
    assert_eq!(got.tone, after);
    s.link.assert_finished();
}

#[test]
fn manual_timing_cannot_be_sent_while_synced() {
    let before = buffer(&[(31, 9)]);
    let mut log = Quiet;
    let mut s = Session::new(
        Fake::from_bytes(Path::PodxtMidi, begin(&before.tone)),
        &AllowAll,
        &mut log,
    );
    let edit = EditorEdit::Controls(&[change(Setting::DelayTime, 8193)]);
    assert!(
        edit_editor_buffer(&mut s, &before, edit, || false)
            .unwrap_err()
            .contains("delay-note to Off")
    );
    s.link.assert_finished();
}

#[test]
fn a_model_and_its_undo_model_must_have_installed_packs() {
    let packed = models::AMP_MODELS
        .iter()
        .position(|&id| models::required_pack(id) == Some(1))
        .unwrap() as u16;
    let mut log = Quiet;
    for (original, selected) in [(0, packed), (packed, 0)] {
        let before = buffer(&[(12, original as u8)]);
        let mut script = begin(&before.tone);
        script.push(packs(4));
        let mut s = Session::new(
            Fake::from_bytes(Path::PodxtMidi, script),
            &AllowAll,
            &mut log,
        );
        let edit = EditorEdit::Controls(&[change(Setting::AmpModel, selected)]);
        assert!(
            edit_editor_buffer(&mut s, &before, edit, || false)
                .unwrap_err()
                .contains("inactive")
        );
        s.link.assert_finished();
    }
}

fn chunks(tone: &EditBuffer) -> Script {
    [&tone.raw[..7], &tone.raw[7..167], &tone.raw[167..]]
        .into_iter()
        .flat_map(|part| part.chunks(32))
        .map(|chunk| (chunk.to_vec(), vec![]))
        .collect()
}

#[test]
fn explicit_send_uses_the_oracle_edit_header_and_leaves_file_only_bytes_unchanged() {
    let before = buffer(&[(23, 64), (97, 72)]);
    let mut after = before.tone.clone();
    after.raw[39 + 23] = 48;
    let mut script = begin(&before.tone);
    script.extend([packs(4), channel(17)]);
    script.extend(chunks(&after));
    script.extend([dump(&after), channel(17)]);
    let mut log = Quiet;
    let mut s = Session::new(
        Fake::from_bytes(Path::PodxtMidi, script),
        &AllowAll,
        &mut log,
    );
    let got = edit_editor_buffer(
        &mut s,
        &before,
        EditorEdit::Send(&[change(Setting::GateThreshold, 48)]),
        || false,
    )
    .unwrap();
    assert_eq!(got.tone, after);
    assert_eq!(s.link.writes[5..12].concat(), after.raw);
    s.link.assert_finished();
}

#[test]
fn dump_echo_cannot_pass_as_readback_and_cancelled_send_terminates_sysex() {
    let before = buffer(&[]);
    let mut script = begin(&before.tone);
    script.extend([packs(4), channel(17)]);
    let mut data = chunks(&before.tone);
    data.last_mut().unwrap().1.push(before.tone.raw.clone());
    script.extend(data);
    script.push((PodxtRequest::EditBuffer.bytes(), vec![]));
    let mut log = Quiet;
    let mut s = Session::new(
        Fake::from_bytes(Path::PodxtMidi, script),
        &AllowAll,
        &mut log,
    );
    assert!(
        edit_editor_buffer(&mut s, &before, EditorEdit::Send(&[]), || false)
            .unwrap_err()
            .contains("No 168-byte")
    );
    s.link.assert_finished();

    let mut script = begin(&before.tone);
    script.extend([packs(4), channel(17)]);
    script.extend([
        (before.tone.raw[..7].to_vec(), vec![]),
        (vec![0xf7], vec![]),
    ]);
    let mut s = Session::new(
        Fake::from_bytes(Path::PodxtMidi, script),
        &AllowAll,
        &mut log,
    );
    let count = std::cell::Cell::new(0);
    let result = edit_editor_buffer(&mut s, &before, EditorEdit::Send(&[]), || {
        count.set(count.get() + 1);
        count.get() >= 5
    });
    assert!(result.unwrap_err().contains("cancelled"));
    s.link.assert_finished();
}

#[test]
fn undo_restores_original_model_and_repairs_wah_companion_from_readback() {
    let original = buffer(&[(4, 63)]);
    let current = buffer(&[(4, 16), (91, 1)]);
    let intermediate = frame(&[(4, 0)]);
    let mut script = begin(&current.tone);
    script.extend([
        packs(4),
        channel(17),
        cc(91, 0),
        cc(4, 63),
        dump(&intermediate),
        channel(17),
        cc(4, 63),
        dump(&original.tone),
        channel(17),
    ]);
    let mut log = Quiet;
    let mut s = Session::new(
        Fake::from_bytes(Path::PodxtMidi, script),
        &AllowAll,
        &mut log,
    );
    let got = edit_editor_buffer(&mut s, &current, EditorEdit::Undo(&original), || false).unwrap();
    assert_eq!(got.tone, original.tone);
    s.link.assert_finished();
}

#[test]
fn wrong_channel_or_stale_nonvolatile_bytes_prevent_writes() {
    let before = buffer(&[]);
    let mut log = Quiet;
    let mut s = Session::new(
        Fake::from_bytes(Path::PodxtMidi, vec![identity([0, 3, 0, 1]), channel(18)]),
        &AllowAll,
        &mut log,
    );
    assert!(
        edit_editor_buffer(&mut s, &before, EditorEdit::Send(&[]), || false)
            .unwrap_err()
            .contains("channel changed")
    );
    s.link.assert_finished();

    let script = begin(&frame(&[(96, 10)]));
    let mut s = Session::new(
        Fake::from_bytes(Path::PodxtMidi, script),
        &AllowAll,
        &mut log,
    );
    assert!(
        edit_editor_buffer(&mut s, &before, EditorEdit::Undo(&before), || false)
            .unwrap_err()
            .contains("outside the editor")
    );
    s.link.assert_finished();
}

#[test]
fn gate_denial_and_unconfirmed_readback_are_errors() {
    let before = buffer(&[(13, 63)]);
    let targets = [basic(Control::Drive, 72)];
    let edit = EditorEdit::Controls(&targets);
    let mut log = Quiet;
    struct NoKeep;
    impl GatePolicy for NoKeep {
        fn allows(&self, op: Operation) -> bool {
            op != Operation::PodxtKeepEditBufferChanges
        }
    }
    for policy in [&AllClosed as &dyn GatePolicy, &NoKeep] {
        let mut s = Session::new(Fake::from_bytes(Path::PodxtMidi, vec![]), policy, &mut log);
        assert!(edit_editor_buffer(&mut s, &before, edit, || false).is_err());
        assert!(s.link.writes.is_empty());
    }
    let mut script = begin(&before.tone);
    script.extend([channel(17), cc(13, 72), dump(&before.tone), channel(17)]);
    let mut s = Session::new(
        Fake::from_bytes(Path::PodxtMidi, script),
        &AllowAll,
        &mut log,
    );
    assert!(
        edit_editor_buffer(&mut s, &before, edit, || false)
            .unwrap_err()
            .contains("read back as")
    );
    s.link.assert_finished();
}

#[test]
fn undo_restores_tempo_before_manual_rate_and_reenables_the_original_note_last() {
    let original = buffer(&[(30, 19), (62, 51), (31, 9), (89, 7), (90, 41)]);
    let current = buffer(&[(30, 15), (62, 101), (31, 8), (89, 8), (90, 111)]);
    let mut script = begin(&current.tone);
    script.extend([
        channel(17),
        cc(89, 7),
        cc(90, 41),
        cc(31, 0),
        cc(30, 19),
        cc(62, 51),
        cc(31, 9),
        dump(&original.tone),
        channel(17),
    ]);
    let mut log = Quiet;
    let mut s = Session::new(
        Fake::from_bytes(Path::PodxtMidi, script),
        &AllowAll,
        &mut log,
    );
    let got = edit_editor_buffer(&mut s, &current, EditorEdit::Undo(&original), || false).unwrap();
    assert_eq!(got.tone, original.tone);
    s.link.assert_finished();
}

#[test]
fn transfer_readback_accepts_synchronized_timing_and_rejects_linked_field_changes() {
    let before = frame(&[(30, 19), (62, 51), (31, 9), (89, 7), (90, 41)]);
    let changes = [(LiveControl::Additional(Setting::Tempo), 1135)];
    let after = frame(&[(30, 15), (62, 101), (31, 9), (89, 8), (90, 111)]);
    assert!(transfer_matches(&before, &changes, &after));
    let mut linked = after.clone();
    linked.raw[39 + 96] = 12;
    assert!(!transfer_matches(&before, &changes, &linked));
}

#[test]
fn librarian_recall_refuses_edited_tones_and_verified_recall_only_changes_program() {
    let playing = buffer(&[(13, 42)]);
    let target = Location::new(BankKind::Tones, 18).unwrap();
    let saved_target = frame(&[(13, 77)]);
    let mut log = Quiet;

    let mut edited = begin(&playing.tone);
    edited.push(reply(
        PodxtRequest::Stored(playing.channel),
        frame(&[(13, 41)]).raw,
    ));
    let mut session = Session::new(
        Fake::from_bytes(Path::PodxtMidi, edited),
        &AllowAll,
        &mut log,
    );
    assert!(
        recall_editor_channel(&mut session, target, || false)
            .unwrap_err()
            .contains("Nothing was switched")
    );
    session.link.assert_finished();

    let mut script = begin(&playing.tone);
    script.extend([
        reply(
            PodxtRequest::Stored(playing.channel),
            playing.tone.raw.clone(),
        ),
        reply(PodxtRequest::Stored(target), saved_target.raw.clone()),
        packs(4),
        channel(17),
        (vec![0xc0, 18], vec![]),
        dump(&saved_target),
        channel(18),
    ]);
    let mut session = Session::new(
        Fake::from_bytes(Path::PodxtMidi, script),
        &AllowAll,
        &mut log,
    );
    let recalled = recall_editor_channel(&mut session, target, || false).unwrap();
    assert_eq!(recalled.channel, target);
    assert_eq!(recalled.tone, saved_target);
    session.link.assert_finished();
}

fn live_recall(
    script: Script,
    target: Location,
    consent: Option<&RecallConfirmation>,
    stopped: impl Fn() -> bool,
) -> Result<EditorRecall, String> {
    let mut log = Quiet;
    let mut session = Session::new(
        Fake::from_bytes(Path::PodxtMidi, script),
        &AllowAll,
        &mut log,
    );
    let result = recall_live_channel(&mut session, target, consent, stopped);
    session.link.assert_finished();
    result
}

fn recall_consent(
    playing: &EditorBuffer,
    target: Location,
    saved: EditBuffer,
) -> RecallConfirmation {
    let mut script = begin(&playing.tone);
    script.extend([
        reply(PodxtRequest::Stored(playing.channel), saved.raw),
        packs(4),
        channel(17),
    ]);
    let EditorRecall::ConfirmationRequired(consent) =
        live_recall(script, target, None, || false).unwrap()
    else {
        panic!("the playing difference requires a choice");
    };
    assert_eq!(consent.target(), target);
    assert_eq!(consent.playing().tone, playing.tone);
    consent
}

#[test]
fn live_recall_ignores_proven_live_state_and_name_padding() {
    let mut playing = buffer(&[(13, 42), (7, 0), (17, 86)]);
    let saved = frame(&[(13, 42), (7, 127), (17, 114)]);
    playing.tone.raw[17..23].fill(b' ');
    let target = Location::tone_from_label("1B").unwrap();
    let target_tone = frame(&[(13, 77)]);
    let mut script = begin(&playing.tone);
    script.extend([
        reply(PodxtRequest::Stored(playing.channel), saved.raw),
        reply(PodxtRequest::Stored(target), target_tone.raw.clone()),
        packs(4),
        dump(&playing.tone),
        channel(17),
        (vec![0xc0, 1], vec![]),
        dump(&target_tone),
        channel(1),
    ]);
    let EditorRecall::Playing(buffer) = live_recall(script, target, None, || false).unwrap() else {
        panic!("equivalent playing tone should allow recall");
    };
    assert_eq!(buffer.channel, target);
    assert_eq!(buffer.tone, target_tone);
}

#[test]
fn live_recall_preserves_drive_link_data_and_opaque_bytes_until_confirmed() {
    let playing = buffer(&[(13, 42)]);
    let target = Location::tone_from_label("1B").unwrap();
    let saved_target = frame(&[(13, 77)]);
    let mut opaque_difference = playing.tone.clone();
    opaque_difference.raw[23] = 1;
    for saved in [
        frame(&[(13, 41)]),
        frame(&[(13, 42), (120, 63)]),
        opaque_difference,
    ] {
        let consent = recall_consent(&playing, target, saved);
        let mut script = begin(&playing.tone);
        script.extend([
            reply(PodxtRequest::Stored(target), saved_target.raw.clone()),
            packs(4),
            dump(&playing.tone),
            channel(17),
            (vec![0xc0, 1], vec![]),
            dump(&saved_target),
            channel(1),
        ]);
        assert!(matches!(
            live_recall(script, target, Some(&consent), || false).unwrap(),
            EditorRecall::Playing(buffer) if buffer.channel == target && buffer.tone == saved_target
        ));
    }
}

#[test]
fn live_recall_confirmation_expires_for_another_target_or_a_changed_buffer() {
    let playing = buffer(&[(13, 42)]);
    let target = Location::tone_from_label("1B").unwrap();
    let consent = recall_consent(&playing, target, frame(&[(13, 41)]));
    let changed = frame(&[(13, 43)]);
    let other = Location::tone_from_label("1C").unwrap();
    for (destination, initial, reread) in [
        (other, &playing.tone, None),
        (target, &changed, None),
        (target, &playing.tone, Some(&changed)),
    ] {
        let mut script = begin(initial);
        if reread.is_some() {
            script.push(reply(PodxtRequest::Stored(target), frame(&[(13, 77)]).raw));
        }
        script.push(packs(4));
        if let Some(tone) = reread {
            script.push(dump(tone));
        }
        script.push(channel(17));
        assert!(matches!(
            live_recall(script, destination, Some(&consent), || false).unwrap(),
            EditorRecall::ConfirmationExpired(buffer) if &buffer.tone == reread.unwrap_or(initial)
        ));
    }
    let mut script = vec![identity([0, 3, 0, 1]), channel(18), dump(&playing.tone)];
    script.extend([packs(4), channel(18)]);
    assert!(matches!(
        live_recall(script, target, Some(&consent), || false).unwrap(),
        EditorRecall::ConfirmationExpired(buffer) if buffer.channel.index() == 18
    ));
    assert!(
        live_recall(Vec::new(), target, Some(&consent), || true)
            .unwrap_err()
            .contains("cancelled")
    );
}

#[test]
fn live_recall_requires_the_addressed_target_and_checks_playing_readback() {
    let before = buffer(&[(13, 42)]);
    let target = Location::tone_from_label("1B").unwrap();
    let mut script = begin(&before.tone);
    script.push(reply(
        PodxtRequest::Stored(before.channel),
        before.tone.raw.clone(),
    ));
    let mut missing = script.clone();
    missing.push((PodxtRequest::Stored(target).bytes(), vec![]));
    assert!(
        live_recall(missing, target, None, || false)
            .unwrap_err()
            .contains("No dump for stored tone 1B")
    );
    script.extend([
        reply(PodxtRequest::Stored(target), frame(&[(13, 77)]).raw),
        packs(4),
        dump(&before.tone),
        channel(17),
        (vec![0xc0, 1], vec![]),
        dump(&before.tone),
        channel(1),
    ]);
    assert!(
        live_recall(script, target, None, || false)
            .unwrap_err()
            .contains("channel switch could not be verified")
    );
}

fn addressed_reply(tone: &EditBuffer, split_marker: bool) -> Vec<Vec<u8>> {
    let mut packets = crate::session::testing::packets(&tone.raw[..EDIT_BUFFER_LEN - 1]);
    let tail = vec![0xf7, 0xf0, 0, 1, 0x0c, 3, 0x72];
    if split_marker {
        packets.extend([tail, vec![0xf7]]);
    } else {
        packets.push([tail, vec![0xf7]].concat());
    }
    packets
}

#[test]
fn recall_and_return_accept_split_and_coalesced_addressed_read_end_markers() {
    let start = buffer(&[(13, 42)]);
    let target = Location::new(BankKind::Tones, 18).unwrap();
    let saved_target = frame(&[(13, 77)]);

    for start_split in [false, true] {
        let mut script = begin(&start.tone);
        script.extend([
            (
                PodxtRequest::Stored(start.channel).bytes(),
                addressed_reply(&start.tone, start_split),
            ),
            (
                PodxtRequest::Stored(target).bytes(),
                addressed_reply(&saved_target, false),
            ),
            packs(4),
            channel(17),
            (PodxtRequest::Program(target).bytes(), vec![]),
            dump(&saved_target),
            channel(18),
        ]);
        script.extend([
            identity(PODXT_LIVE_VERSION),
            channel(18),
            dump(&saved_target),
            (
                PodxtRequest::Stored(target).bytes(),
                addressed_reply(&saved_target, true),
            ),
            (
                PodxtRequest::Stored(start.channel).bytes(),
                addressed_reply(&start.tone, false),
            ),
            packs(4),
            channel(18),
            (PodxtRequest::Program(start.channel).bytes(), vec![]),
            dump(&start.tone),
            channel(17),
        ]);
        let mut log = Quiet;
        let mut session = Session::new(
            Fake::from_bytes(Path::PodxtMidi, script),
            &AllowAll,
            &mut log,
        );
        let selected = recall_editor_channel(&mut session, target, || false).unwrap();
        assert_eq!(
            (selected.channel, selected.tone),
            (target, saved_target.clone())
        );
        let returned = recall_editor_channel(&mut session, start.channel, || false).unwrap();
        assert_eq!(
            (returned.channel, returned.tone),
            (start.channel, start.tone.clone())
        );
        session.link.assert_finished();
    }
}
