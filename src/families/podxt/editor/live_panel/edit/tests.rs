use super::super::tests::{FakeLink, FakeLock};
use super::*;
use podxt::{Global, PodxtRequest, Value};
use std::{cell::Cell, collections::VecDeque};

fn sample(drive: u8, index: u16) -> EditorBuffer {
    let mut raw = vec![0; podxt::EDIT_BUFFER_LEN];
    raw[..7].copy_from_slice(&[0xf0, 0, 1, 0x0c, 3, 0x74, 0x0a]);
    raw[39 + 13] = drive;
    raw[167] = 0xf7;
    EditorBuffer {
        channel: Location::new(BankKind::Tones, index).unwrap(),
        tone: EditBuffer::parse(raw).unwrap(),
        packs: Packs(4),
    }
}

fn drive(value: u8) -> LiveTarget {
    LiveTarget::Basic(Control::Drive, Target::Value(Value::new(value).unwrap()))
}

#[test]
fn queued_drag_edits_keep_the_latest_value_and_honor_the_dispatch_interval() {
    let now = Instant::now();
    let mut editor = LiveEditor {
        last_dispatch: Some(now),
        stage_only: false,
        ..Default::default()
    };
    let control = LiveControl::Basic(Control::Drive);
    let in_flight = [drive(20)];
    assert_eq!(editor.pending.overlay(&in_flight, control, 0), 20);

    for value in [22, 40, 57] {
        editor.pending.set(drive(value));
    }
    assert_eq!(editor.pending.0, [drive(57)]);
    assert_eq!(editor.pending.overlay(&in_flight, control, 0), 57);
    assert!(editor.busy());
    assert_eq!(
        editor.dispatch_wait(now + Duration::from_millis(40)),
        Duration::from_millis(110)
    );
    assert!(editor.dispatch_wait(now + EDIT_INTERVAL).is_zero());

    editor.cancel();
    assert!(!editor.busy());
    editor.stage_only = true;
    editor.pending.set(drive(84));
    assert!(!editor.busy(), "staged values never dispatch automatically");
    editor.detach();
    assert!(editor.pending.0.is_empty());
}

#[test]
fn a_failed_edit_keeps_the_original_snapshot_until_the_playing_channel_changes() {
    let mut editor = LiveEditor::default();
    editor.completed(Action::Get, Ok(sample(12, 17)));
    editor.completed(Action::Controls, Ok(sample(54, 17)));
    editor.pending.set(drive(58));
    editor.completed(Action::Controls, Err("missing readback".into()));
    assert!(editor.needs_get);
    assert!(editor.pending.0.is_empty());
    editor.completed(Action::Get, Ok(sample(55, 17)));
    assert!(!editor.needs_get);
    let active = editor.session.as_ref().unwrap();
    assert_eq!(active.original.tone.cc(13), 12);
    assert_eq!(active.current.tone.cc(13), 55);

    editor.completed(Action::Get, Ok(sample(90, 18)));
    assert_eq!(editor.session.as_ref().unwrap().original.tone.cc(13), 90);
    editor.detach();
    assert!(editor.session.is_none());
}

#[test]
fn g6_transaction_holds_the_rig_lock_for_cc_and_readback_and_denies_other_writes() {
    let before = sample(20, 17);
    let after = sample(45, 17);
    let channel = || {
        (
            PodxtRequest::QueryGlobal(Global::CurrentChannel),
            vec![0xf0, 0, 1, 0x0c, 3, 0x56, 17, 0, 0, 1, 1, 0xf7],
        )
    };
    let job = Job {
        action: Action::Controls,
        whole_tone_backup: None,
        current: Some(before.clone()),
        original: Some(before.clone()),
        targets: vec![drive(45)],
    };
    let held = Cell::new(false);
    let writes = Cell::new(0);
    let stop = AtomicBool::new(false);
    let result = edit_transaction(
        &job,
        &stop,
        || {
            assert!(!held.replace(true));
            Ok(FakeLock(&held))
        },
        |_, run| {
            assert!(held.get(), "the lock must cover opening the port");
            run(&mut FakeLink {
                held: &held,
                script: VecDeque::from([
                    (
                        PodxtRequest::Identity,
                        variax_workbench::identity_stage::bridge_reply(),
                    ),
                    channel(),
                    (PodxtRequest::EditBuffer, before.tone.raw),
                    channel(),
                    (
                        PodxtRequest::Set(Control::Drive, Value::new(45).unwrap()),
                        Vec::new(),
                    ),
                    (PodxtRequest::EditBuffer, after.tone.raw.clone()),
                    channel(),
                ]),
                reply: None,
                time: &Cell::new(Duration::ZERO),
                writes: &writes,
            })
        },
    )
    .unwrap();
    assert_eq!(result.tone, after.tone);
    assert_eq!(writes.get(), 7);
    assert!(!held.get());

    let busy = edit_transaction(
        &job,
        &stop,
        || Err::<(), _>("Rig busy".into()),
        |_, _| panic!("busy rig must prevent port open"),
    );
    assert_eq!(busy.unwrap_err(), "Rig busy");
    for denied in [
        Operation::PodxtChannelSwitch,
        Operation::PodxtLibraryRead,
        Operation::PodxtLinkedVariaxModelEdit,
        Operation::PodxtVariaxControlRelease,
        Operation::VariaxStoreRestore,
    ] {
        assert!(!EditPolicy(Action::Send).allows(denied));
    }
    assert!(!EditPolicy(Action::Get).allows(Operation::PodxtEditBufferWrite));
}

#[test]
fn model_specific_controls_only_appear_with_their_line6_layout() {
    let table = variax_workbench::pod_preset::pod_parameter_table();
    let names = table.wire_label(Control::StompParam2.cc(), |_| Some(33882112));
    assert_eq!(names.unwrap().label, "Drive");
    assert!(
        table
            .wire_label(Control::StompParam6.cc(), |_| Some(33882112))
            .is_none()
    );
    assert!(
        table.wire_label(96, |_| None).is_some(),
        "the table can describe linked model fields"
    );
    assert!(!Control::ALL.into_iter().any(|control| control.cc() == 96));
    assert!(
        !Setting::ALL
            .into_iter()
            .any(|setting| LiveControl::Additional(setting).cc() == 96)
    );
}

#[test]
fn the_whole_tone_resend_has_its_own_closed_owner_gate() {
    const {
        assert!(!WHOLE_TONE_SEND_APPROVED);
    }
    assert!(!LiveEditor::admitted(Action::ResendWholeTone));
    let policy = EditPolicy(Action::ResendWholeTone);
    assert_eq!(
        policy.allows(Operation::PodxtLibraryRead),
        !hardware_gates::is_locked(Gate::FullBackupRead)
    );
    assert!(!policy.allows(Operation::PodxtStoreRestore));
    assert!(!policy.allows(Operation::PodxtChannelSwitch));
}
