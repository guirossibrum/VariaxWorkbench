use super::super::tests::{app, click_prompt, prompt_frame, recording_frame};
use super::*;

fn hd_editor_frame(context: &egui::Context, app: &mut Workbench, events: Vec<egui::Event>) {
    // 820px leaves the HD scroll area shorter than its content, so the
    // idle-scroll fixture can move the type-9 control fully off screen.
    let _ = context.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 820.0))),
            events,
            ..Default::default()
        },
        |ctx| app.render(ctx),
    );
}

fn click_hd_editor(context: &egui::Context, app: &mut Workbench, id: &'static str) {
    app.select_test_editor::<HdEditor>();
    hd_editor_frame(context, app, vec![]);
    if !matches!(
        id,
        "apply-hd-type9-choice"
            | "discard-hd-type9-choice"
            | "apply-hd-type10-choice"
            | "discard-hd-type10-choice"
            | "save-hd-copy"
            | "load-hd-file"
            | "hd-type9-choice-option-0"
            | "hd-type9-choice-option-1"
            | "hd-type9-choice-option-2"
            | "hd-type10-choice-option-0"
            | "hd-type10-choice-option-1"
            | "hd-type13-choice-option-0"
            | "hd-type13-choice-option-1"
            | "hd-type13-choice-option-2"
            | "hd-type13-choice-option-3"
            | "hd-type19-choice-option-0"
            | "hd-type19-choice-option-1"
            | "hd-type19-choice-option-2"
            | "hd-type19-choice-option-3"
            | "hd-type19-choice-option-4"
            | "hd-type19-choice-option-5"
            | "hd-type19-choice-option-6"
            | "hd-type19-choice-option-7"
    ) {
        scroll_hd_editor_control_into_view(context, app, id);
        hd_editor_frame(context, app, vec![]);
    }
    let pos = app
        .hd()
        .hd_buttons
        .get(id)
        .unwrap_or_else(|| {
            panic!(
                "expected rendered POD button {id:?}; registered: {:?}",
                app.hd().hd_buttons.keys().collect::<Vec<_>>()
            )
        })
        .center();
    hd_editor_frame(
        context,
        app,
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ],
    );
    hd_editor_frame(
        context,
        app,
        vec![egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );
}

fn scroll_hd_editor_control_into_view(context: &egui::Context, app: &Workbench, id: &'static str) {
    let rect = app.hd().hd_buttons[id];
    let (scroll_id, viewport, content_height) = app
        .hd()
        .hd_scroll_area
        .expect("rendered HD editor scroll area");
    let mut state = egui::scroll_area::State::load(context, scroll_id).unwrap_or_default();
    let max_offset = (content_height - viewport.height()).max(0.0);
    state.offset.y =
        (state.offset.y + rect.center().y - viewport.center().y).clamp(0.0, max_offset);
    state.store(context, scroll_id);
}

fn click_hd_type9_choice(context: &egui::Context, app: &mut Workbench, choice: usize) {
    click_hd_editor(context, app, "hd-type9-choice-combo");
    let option_id = match choice {
        0 => "hd-type9-choice-option-0",
        1 => "hd-type9-choice-option-1",
        2 => "hd-type9-choice-option-2",
        _ => panic!("invalid type-9 choice index"),
    };
    click_hd_editor(context, app, option_id);
}

fn click_hd_type8_choice(context: &egui::Context, app: &mut Workbench, choice: usize) {
    click_hd_editor(context, app, "hd-type8-choice-combo");
    let option_id = match choice {
        0 => "hd-type8-choice-option-0",
        1 => "hd-type8-choice-option-1",
        _ => panic!("invalid type-8 choice index"),
    };
    click_hd_editor(context, app, option_id);
}

fn click_hd_type10_choice(context: &egui::Context, app: &mut Workbench, choice: usize) {
    click_hd_editor(context, app, "hd-type10-choice-combo");
    let option_id = if choice == 0 {
        "hd-type10-choice-option-0"
    } else if choice == 1 {
        "hd-type10-choice-option-1"
    } else {
        panic!("invalid type-10 choice index")
    };
    click_hd_editor(context, app, option_id);
}

fn click_hd_type11_choice(
    context: &egui::Context,
    app: &mut Workbench,
    control: pod_preset::HdType11Control,
    choice: usize,
) {
    let choice = pod_preset::HdType11Choice::from_index(choice as u8).unwrap();
    click_hd_editor(context, app, hd_type11_combo_button(control));
    click_hd_editor(context, app, hd_type11_option_button(control, choice));
}

fn click_hd_type12_choice(context: &egui::Context, app: &mut Workbench, choice: usize) {
    let choice = pod_preset::HdType12Choice::from_index(choice as u8).unwrap();
    click_hd_editor(context, app, hd_type12_combo_button());
    click_hd_editor(context, app, hd_type12_option_button(choice));
}

fn click_hd_type13_choice(context: &egui::Context, app: &mut Workbench, choice: usize) {
    let choice = pod_preset::HdType13Choice::from_index(choice as u8).unwrap();
    click_hd_editor(context, app, "hd-type13-choice-combo");
    click_hd_editor(context, app, hd_type13_option_button(choice));
}

fn click_hd_type19_choice(context: &egui::Context, app: &mut Workbench, choice: usize) {
    let choice = pod_preset::HdType19Choice::from_index(choice as u8).unwrap();
    click_hd_editor(context, app, "hd-type19-choice-combo");
    click_hd_editor(context, app, hd_type19_option_button(choice));
}

fn assert_hd_bytes_eq(actual: &[u8], expected: &[u8], context: &str) {
    let differences = actual
        .iter()
        .zip(expected)
        .enumerate()
        .filter_map(|(offset, (actual, expected))| {
            (actual != expected).then_some((offset, *actual, *expected))
        })
        .take(8)
        .collect::<Vec<_>>();
    assert!(
        actual.len() == expected.len() && differences.is_empty(),
        "{context}: actual length {}, expected length {}, first differences (offset, actual, expected): {differences:?}",
        actual.len(),
        expected.len()
    );
}

fn sample_hd_editor_preset() -> Vec<u8> {
    let mut bytes = vec![0; pod_preset::H5EP_MIN_BYTES];
    bytes[..4].copy_from_slice(b"H5EP");
    bytes[4..8].copy_from_slice(&1u32.to_be_bytes());
    let body = &mut bytes[pod_preset::H5EP_HEADER_BYTES..];
    for offset in [0x228, 0x328] {
        body[offset..offset + 4].copy_from_slice(&0x0100_0000u32.to_be_bytes());
    }
    for index in 0..8 {
        let block = 0x428 + index * 0x100;
        body[block..block + 4].copy_from_slice(&0x0200_0000u32.to_be_bytes());
    }
    let block = 0x428;
    body[block..block + 4].copy_from_slice(&0x0205_000du32.to_be_bytes());
    let row = block + 0x10;
    body[row..row + 4].copy_from_slice(&0x3f10_0001u32.to_be_bytes());
    body[row + 4..row + 8].copy_from_slice(&0.5f32.to_bits().to_be_bytes());
    let bass_row = row + 0x14;
    body[bass_row..bass_row + 4].copy_from_slice(&0x3f10_0002u32.to_be_bytes());
    body[bass_row + 4..bass_row + 8].copy_from_slice(&0.25f32.to_bits().to_be_bytes());
    let mid_row = bass_row + 0x14;
    body[mid_row..mid_row + 4].copy_from_slice(&0x3f10_0003u32.to_be_bytes());
    body[mid_row + 4..mid_row + 8].copy_from_slice(&0.35f32.to_bits().to_be_bytes());
    bytes
}

fn sample_hd_type9_preset(slot: usize, row: usize, bits: u32) -> Vec<u8> {
    sample_hd_type9_preset_for(
        slot,
        row,
        bits,
        pod_preset::HD_TYPE9_CHOICE_MODEL_ID,
        0x3f10_0004,
    )
}

fn sample_hd_type9_preset_for(
    slot: usize,
    row: usize,
    bits: u32,
    model_id: u32,
    key: u32,
) -> Vec<u8> {
    let mut bytes = sample_hd_editor_preset();
    let block = pod_preset::H5EP_HEADER_BYTES + 0x428 + slot * 0x100;
    bytes[block..block + 4].copy_from_slice(&model_id.to_be_bytes());
    bytes[block + 8] = 0;
    bytes[block + 0x0b] = 1;
    bytes[block + 0x0f] = 12;
    let row_at = block + 0x10 + row * 0x14;
    bytes[row_at..row_at + 4].copy_from_slice(&key.to_be_bytes());
    bytes[row_at + 4..row_at + 8].copy_from_slice(&bits.to_be_bytes());
    bytes[row_at + 8..row_at + 0x10].fill(0x31);
    bytes[row_at + 0x10..row_at + 0x14].copy_from_slice(&[0, 0x42, 0x43, 0x44]);
    bytes.extend_from_slice(b"type9-tail");
    bytes
}

fn sample_hd_type8_preset(slot: usize, row: usize, bits: u32) -> Vec<u8> {
    sample_hd_type9_preset_for(
        slot,
        row,
        bits,
        pod_preset::HD_TYPE8_CHOICE_MODEL_ID,
        pod_preset::HD_TYPE8_CHOICE_KEY,
    )
}

fn sample_hd_type10_preset_for(
    slot: usize,
    row: usize,
    bits: u32,
    model_id: u32,
    key: u32,
) -> Vec<u8> {
    let mut bytes = sample_hd_type9_preset_for(slot, row, bits, model_id, key);
    let block = pod_preset::H5EP_HEADER_BYTES + 0x428 + slot * 0x100;
    let other_row = (row + 1) % 12;
    let at = block + 0x10 + other_row * 0x14;
    bytes[at..at + 4].copy_from_slice(&0x3f10_0004u32.to_be_bytes());
    bytes[at + 4..at + 8].copy_from_slice(&0x0000_0000u32.to_be_bytes());
    bytes
}

fn sample_hd_type11_preset(model_id: u32) -> Vec<u8> {
    let controls = pod_preset::HdType11Control::for_model(model_id);
    assert_eq!(controls.len(), 2);
    let mut bytes = sample_hd_type9_preset_for(5, 3, 0x3e12_3456, model_id, controls[0].key());
    let block = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100;
    if model_id == pod_preset::HD_TYPE12_CHOICE_MODEL_ID {
        let type12 = block + 0x10 + 4 * 0x14;
        bytes[type12..type12 + 4].copy_from_slice(&pod_preset::HD_TYPE12_CHOICE_KEY.to_be_bytes());
        bytes[type12 + 4..type12 + 8].copy_from_slice(&0x3e65_9999u32.to_be_bytes());
    }
    if model_id == pod_preset::HD_TYPE13_CHOICE_MODEL_ID {
        let type13 = block + 0x10 + 4 * 0x14;
        bytes[type13..type13 + 4].copy_from_slice(&pod_preset::HD_TYPE13_CHOICE_KEY.to_be_bytes());
        bytes[type13 + 4..type13 + 8].copy_from_slice(&0x3eaaaaaau32.to_be_bytes());
    }
    let second = block + 0x10 + 8 * 0x14;
    bytes[second..second + 4].copy_from_slice(&controls[1].key().to_be_bytes());
    bytes[second + 4..second + 8].copy_from_slice(&0x3e65_4321u32.to_be_bytes());
    bytes
}

#[test]
fn hd_type11_gui_reaches_all_five_choices_on_all_four_explicit_controls() {
    for model_id in [0x020a_001e, 0x020a_001d] {
        let controls = pod_preset::HdType11Control::for_model(model_id);
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let original = sample_hd_type11_preset(model_id);
        let index = app
            .hd_mut()
            .hd_library
            .import_bytes(original.clone())
            .unwrap();
        app.hd_mut().select_hd_file(index);
        app.select_test_editor::<HdEditor>();
        app.hd_mut().hd_effect_slot = 5;
        let slot = pod_preset::HdFxSlot::new(5).unwrap();
        for control in controls.iter().copied() {
            for choice in pod_preset::HdType11Choice::ALL {
                hd_editor_frame(&context, &mut app, vec![]);
                click_hd_type11_choice(&context, &mut app, control, choice.index() as usize);
                let expected = PendingHdType11Choice {
                    file_index: index,
                    slot,
                    model_id,
                    control,
                    choice,
                };
                assert_eq!(
                    app.hd().hd_type11_pending.get(&(index, slot, control)),
                    Some(&expected),
                    "{model_id:#010x} / {:#010x} / {choice:?}",
                    control.key()
                );
                assert_eq!(app.hd().hd_type11_pending.len(), 1);
                assert_hd_bytes_eq(
                    app.hd().hd_library.file(index).unwrap().as_bytes(),
                    &original,
                    "type-11 menu stages without changing bytes",
                );
                click_hd_editor(&context, &mut app, hd_type11_discard_button(control));
                assert!(app.hd().hd_type11_pending.is_empty());
                assert_hd_bytes_eq(
                    app.hd().hd_library.file(index).unwrap().as_bytes(),
                    &original,
                    "discarding one staged type-11 choice leaves source bytes",
                );
            }
        }
    }
}

#[test]
fn hd_type19_rendered_combo_stages_applies_and_discards_all_eight_exact_words() {
    let control = pod_preset::HdType19Control::ALL[0];
    let slot = pod_preset::HdFxSlot::new(5).unwrap();
    let model = pod_preset::HD_TYPE19_CHOICE_MODEL_ID;
    for choice in pod_preset::HdType19Choice::ALL {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let original =
            sample_hd_type9_preset_for(5, 9, 0x3e12_3456, model, pod_preset::HD_TYPE19_CHOICE_KEY);
        let index = app
            .hd_mut()
            .hd_library
            .import_bytes(original.clone())
            .unwrap();
        app.hd_mut().select_hd_file(index);
        app.select_test_editor::<HdEditor>();
        app.hd_mut().hd_effect_slot = 5;
        click_hd_type19_choice(&context, &mut app, choice.index() as usize);
        let staged = PendingHdType19Choice {
            file_index: index,
            slot,
            model_id: model,
            control,
            choice,
        };
        assert_eq!(
            app.hd().hd_type19_pending.get(&(index, slot, control)),
            Some(&staged)
        );
        assert_hd_bytes_eq(
            app.hd().hd_library.file(index).unwrap().as_bytes(),
            &original,
            "type-0x13 selection stages without changing bytes",
        );
        click_hd_editor(&context, &mut app, hd_type19_apply_button());
        let value = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + 9 * 0x14 + 4;
        let mut expected = original.clone();
        expected[value..value + 4].copy_from_slice(&choice.bits().to_be_bytes());
        assert_hd_bytes_eq(
            app.hd().hd_library.file(index).unwrap().as_bytes(),
            &expected,
            "type-0x13 Apply changes only the selected word",
        );
        assert!(app.hd().hd_type19_pending.is_empty());
        click_hd_type19_choice(&context, &mut app, ((choice.index() + 1) % 8) as usize);
        click_hd_editor(&context, &mut app, hd_type19_discard_button());
        assert!(app.hd().hd_type19_pending.is_empty());
        assert_hd_bytes_eq(
            app.hd().hd_library.file(index).unwrap().as_bytes(),
            &expected,
            "discard preserves applied bytes",
        );
    }
}

#[test]
fn hd_type19_quit_handles_stage_only_apply_discard_keep_and_rejected_apply() {
    let model = pod_preset::HD_TYPE19_CHOICE_MODEL_ID;
    let control = pod_preset::HdType19Control::ALL[0];
    let slot = pod_preset::HdFxSlot::new(5).unwrap();
    let source = sample_hd_type9_preset_for(5, 9, 0x3e12_3456, model, control.key());
    let pending = |index| PendingHdType19Choice {
        file_index: index,
        slot,
        model_id: model,
        control,
        choice: pod_preset::HdType19Choice::Choice5,
    };

    for action in ["keep", "discard", "apply", "reject"] {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let index = app
            .hd_mut()
            .hd_library
            .import_bytes(source.clone())
            .unwrap();
        app.hd_mut().select_hd_file(index);
        app.select_test_editor::<HdEditor>();
        app.hd_mut().hd_effect_slot = 5;
        click_hd_type19_choice(&context, &mut app, 5);
        let exact = pending(index);
        assert_eq!(
            app.hd().hd_type19_pending.get(&(index, slot, control)),
            Some(&exact)
        );
        let mut before_quit = source.clone();
        if action == "reject" {
            let marker = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + 9 * 0x14 + 0x10;
            before_quit[marker] = 1;
            *app.hd_mut().hd_library.file_mut(index).unwrap() =
                HdEditFile::parse(before_quit.clone()).unwrap();
            assert!(
                matches!(
                    app.hd().hd_library.file(index),
                    Some(HdEditFile::H5ep(preset)) if preset.effect_model_id(5) == Some(model)
                ),
                "reject case retains model identity"
            );
        }
        assert_hd_bytes_eq(
            app.hd().hd_library.file(index).unwrap().as_bytes(),
            &before_quit,
            "stage-only quit setup bytes",
        );
        app.request_close(&context);
        assert!(matches!(app.test_pending(), Some(Pending::QuitHd)));
        assert_eq!(
            app.hd().hd_type19_pending.get(&(index, slot, control)),
            Some(&exact)
        );
        assert!(!app.permit_close);

        match action {
            "keep" => {
                click_prompt(&context, &mut app, "cancel");
                assert!(app.test_pending().is_none());
                assert_eq!(
                    app.hd().hd_type19_pending.get(&(index, slot, control)),
                    Some(&exact)
                );
                assert_hd_bytes_eq(
                    app.hd().hd_library.file(index).unwrap().as_bytes(),
                    &source,
                    "Keep editing preserves stage-only bytes",
                );
                assert!(!app.permit_close);
            }
            "discard" => {
                click_prompt(&context, &mut app, "discard");
                assert!(app.hd().hd_type19_pending.is_empty());
                assert!(
                    app.permit_close,
                    "stage discard closes only after clearing the stage"
                );
                assert_hd_bytes_eq(
                    app.hd().hd_library.file(index).unwrap().as_bytes(),
                    &source,
                    "stage-only discard leaves bytes unchanged",
                );
            }
            "apply" => {
                click_prompt(&context, &mut app, "apply-type19");
                let value = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + 9 * 0x14 + 4;
                let mut expected = source.clone();
                expected[value..value + 4].copy_from_slice(&exact.choice.bits().to_be_bytes());
                assert!(app.hd().hd_type19_pending.is_empty());
                assert!(app.hd().hd_library.is_dirty(index));
                assert!(!app.permit_close);
                assert!(
                    matches!(app.test_pending(), Some(Pending::QuitHd)),
                    "Apply transitions to the dirty-file prompt"
                );
                assert_hd_bytes_eq(
                    app.hd().hd_library.file(index).unwrap().as_bytes(),
                    &expected,
                    "quit Apply changes only the target word",
                );
                click_prompt(&context, &mut app, "cancel");
                assert!(app.test_pending().is_none());
                assert!(!app.permit_close);
                assert_hd_bytes_eq(
                    app.hd().hd_library.file(index).unwrap().as_bytes(),
                    &expected,
                    "canceling dirty-file prompt retains applied bytes",
                );
            }
            "reject" => {
                click_prompt(&context, &mut app, "apply-type19");
                assert_eq!(
                    app.hd().hd_type19_pending.get(&(index, slot, control)),
                    Some(&exact)
                );
                assert!(matches!(app.test_pending(), Some(Pending::QuitHd)));
                assert!(app.hd().hd_library.is_dirty(index));
                assert!(
                    !app.permit_close,
                    "rejected quit Apply keeps the window open"
                );
                assert_hd_bytes_eq(
                    app.hd().hd_library.file(index).unwrap().as_bytes(),
                    &before_quit,
                    "rejected quit Apply preserves complete bytes",
                );
                assert!(app.hd().hd_message.contains("unassigned marker"));
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn hd_type19_failed_apply_preserves_dirty_saved_baseline_and_reverts_exactly() {
    let model = pod_preset::HD_TYPE19_CHOICE_MODEL_ID;
    let control = pod_preset::HdType19Control::ALL[0];
    let slot = pod_preset::HdFxSlot::new(5).unwrap();
    let source = sample_hd_type9_preset_for(5, 9, 0x3e12_3456, model, control.key());
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let index = app
        .hd_mut()
        .hd_library
        .import_bytes(source.clone())
        .unwrap();
    app.hd_mut().select_hd_file(index);
    app.select_test_editor::<HdEditor>();
    app.hd_mut().hd_effect_slot = 5;

    app.hd_mut()
        .hd_library
        .set_effect_enabled(index, slot, true)
        .unwrap();
    app.hd_mut().hd_library.note_saved(index).unwrap();
    let saved_baseline = app.hd().hd_library.file(index).unwrap().as_bytes().to_vec();
    assert!(!app.hd().hd_library.is_dirty(index));
    app.hd_mut()
        .hd_library
        .set_effect_enabled(index, slot, false)
        .unwrap();
    let independently_dirty = app.hd().hd_library.file(index).unwrap().as_bytes().to_vec();
    assert!(app.hd().hd_library.is_dirty(index));
    click_hd_type19_choice(&context, &mut app, 6);
    let exact = PendingHdType19Choice {
        file_index: index,
        slot,
        model_id: model,
        control,
        choice: pod_preset::HdType19Choice::Choice6,
    };
    assert_eq!(
        app.hd().hd_type19_pending.get(&(index, slot, control)),
        Some(&exact)
    );
    assert_hd_bytes_eq(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        &independently_dirty,
        "actual type-19 combo stages over the independent dirty enable edit",
    );

    let target = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + 9 * 0x14;
    let mut invalid = independently_dirty.clone();
    invalid[target + 0x10] = 1;
    *app.hd_mut().hd_library.file_mut(index).unwrap() = HdEditFile::parse(invalid.clone()).unwrap();
    assert!(matches!(
        app.hd().hd_library.file(index),
        Some(HdEditFile::H5ep(preset)) if preset.effect_model_id(5) == Some(model)
    ));
    assert_eq!(app.hd().hd_library.file(index).unwrap().as_bytes(), invalid);
    assert!(app.hd().hd_library.is_dirty(index));
    assert_eq!(
        app.hd().hd_type19_pending.get(&(index, slot, control)),
        Some(&exact)
    );
    click_hd_editor(&context, &mut app, hd_type19_apply_button());
    assert_eq!(
        app.hd().hd_type19_pending.get(&(index, slot, control)),
        Some(&exact)
    );
    assert_eq!(app.hd().hd_library.file(index).unwrap().as_bytes(), invalid);
    assert!(app.hd().hd_library.is_dirty(index));
    assert!(app.hd().hd_message.contains("unassigned marker"));
    assert!(app.hd_mut().discard_next_dirty_hd());
    assert!(app.hd().hd_type19_pending.is_empty());
    assert_hd_bytes_eq(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        &saved_baseline,
        "revert restores the complete saved baseline",
    );
    assert!(!app.hd().hd_library.is_dirty(index));
}

#[test]
fn hd_type19_stage_lifecycle_covers_context_open_save_collision_and_revert() {
    let model = pod_preset::HD_TYPE19_CHOICE_MODEL_ID;
    let control = pod_preset::HdType19Control::ALL[0];
    let slot = pod_preset::HdFxSlot::new(5).unwrap();
    let source = sample_hd_type9_preset_for(5, 9, 0x3e12_3456, model, control.key());
    let pending = |index, choice| PendingHdType19Choice {
        file_index: index,
        slot,
        model_id: model,
        control,
        choice,
    };

    let (context, mut by_slot) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let index = by_slot
        .hd_mut()
        .hd_library
        .import_bytes(source.clone())
        .unwrap();
    by_slot.hd_mut().select_hd_file(index);
    by_slot.select_test_editor::<HdEditor>();
    by_slot.hd_mut().hd_effect_slot = 5;
    click_hd_type19_choice(&context, &mut by_slot, 2);
    let exact = pending(index, pod_preset::HdType19Choice::Choice2);
    assert_eq!(
        by_slot.hd().hd_type19_pending.get(&(index, slot, control)),
        Some(&exact)
    );
    by_slot.hd_mut().hd_effect_slot = 4;
    hd_editor_frame(&context, &mut by_slot, vec![]);
    assert!(
        by_slot.hd().hd_type19_pending.is_empty(),
        "slot change clears only stale type-19 stage"
    );
    assert_hd_bytes_eq(
        by_slot.hd().hd_library.file(index).unwrap().as_bytes(),
        &source,
        "slot invalidation preserves full source",
    );

    let (context, mut by_model) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let index = by_model
        .hd_mut()
        .hd_library
        .import_bytes(source.clone())
        .unwrap();
    by_model.hd_mut().select_hd_file(index);
    by_model.select_test_editor::<HdEditor>();
    by_model.hd_mut().hd_effect_slot = 5;
    click_hd_type19_choice(&context, &mut by_model, 1);
    let exact = pending(index, pod_preset::HdType19Choice::Choice1);
    assert_eq!(
        by_model.hd().hd_type19_pending.get(&(index, slot, control)),
        Some(&exact)
    );
    let mut replacement = source.clone();
    let block = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100;
    replacement[block..block + 4].copy_from_slice(&0x020a_001e_u32.to_be_bytes());
    *by_model.hd_mut().hd_library.file_mut(index).unwrap() =
        HdEditFile::parse(replacement.clone()).unwrap();
    hd_editor_frame(&context, &mut by_model, vec![]);
    assert!(
        by_model.hd().hd_type19_pending.is_empty(),
        "model change clears stale type-19 stage"
    );
    assert_hd_bytes_eq(
        by_model.hd().hd_library.file(index).unwrap().as_bytes(),
        &replacement,
        "model invalidation preserves complete replacement bytes",
    );

    let (context, mut by_entry) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let index = by_entry
        .hd_mut()
        .hd_library
        .import_bytes(source.clone())
        .unwrap();
    by_entry.hd_mut().select_hd_file(index);
    by_entry.select_test_editor::<HdEditor>();
    by_entry.hd_mut().hd_effect_slot = 5;
    click_hd_type19_choice(&context, &mut by_entry, 0);
    let exact = pending(index, pod_preset::HdType19Choice::Choice0);
    assert_eq!(
        by_entry.hd().hd_type19_pending.get(&(index, slot, control)),
        Some(&exact)
    );
    let other = by_entry
        .hd_mut()
        .hd_library
        .import_bytes(source.clone())
        .unwrap();
    by_entry.hd_mut().select_hd_file(other);
    assert!(
        by_entry.hd().hd_type19_pending.is_empty(),
        "selection replacement invalidates old-entry stage"
    );
    assert_hd_bytes_eq(
        by_entry.hd().hd_library.file(index).unwrap().as_bytes(),
        &source,
        "entry invalidation preserves old complete bytes",
    );

    let (context, mut open) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let index = open
        .hd_mut()
        .hd_library
        .import_bytes(source.clone())
        .unwrap();
    open.hd_mut().select_hd_file(index);
    open.select_test_editor::<HdEditor>();
    open.hd_mut().hd_effect_slot = 5;
    click_hd_type19_choice(&context, &mut open, 3);
    let exact = pending(index, pod_preset::HdType19Choice::Choice3);
    assert_eq!(
        open.hd().hd_type19_pending.get(&(index, slot, control)),
        Some(&exact)
    );
    let replacement_dir = tempfile::tempdir().unwrap();
    let replacement_path = replacement_dir.path().join("type19-open.5xe");
    let replacement_bytes = sample_hd_type9_preset_for(5, 9, 0x3e00_0000, model, control.key());
    std::fs::write(&replacement_path, &replacement_bytes).unwrap();
    open.dialogs = Box::new(FakeDialogs {
        pod_open: [None, Some(replacement_path)].into(),
        ..Default::default()
    });
    click_hd_editor(&context, &mut open, "load-hd-file");
    assert_eq!(
        open.hd().hd_type19_pending.get(&(index, slot, control)),
        Some(&exact),
        "canceled Open preserves exact stage"
    );
    assert_hd_bytes_eq(
        open.hd().hd_library.file(index).unwrap().as_bytes(),
        &source,
        "canceled Open preserves all bytes",
    );
    click_hd_editor(&context, &mut open, "load-hd-file");
    let opened = open
        .hd()
        .hd_selected
        .expect("successful Open selects replacement");
    assert_ne!(opened, index);
    assert!(
        open.hd().hd_type19_pending.is_empty(),
        "successful Open invalidates only the old entry stage"
    );
    assert_hd_bytes_eq(
        open.hd().hd_library.file(index).unwrap().as_bytes(),
        &source,
        "successful Open preserves old entry bytes",
    );
    assert_hd_bytes_eq(
        open.hd().hd_library.file(opened).unwrap().as_bytes(),
        &replacement_bytes,
        "successful Open preserves complete replacement bytes",
    );

    let (context, mut save) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let index = save
        .hd_mut()
        .hd_library
        .import_bytes(source.clone())
        .unwrap();
    save.hd_mut().select_hd_file(index);
    save.select_test_editor::<HdEditor>();
    save.hd_mut().hd_effect_slot = 5;
    click_hd_type19_choice(&context, &mut save, 4);
    let exact = pending(index, pod_preset::HdType19Choice::Choice4);
    assert_eq!(
        save.hd().hd_type19_pending.get(&(index, slot, control)),
        Some(&exact)
    );
    save.hd_mut()
        .hd_library
        .set_effect_enabled(index, slot, true)
        .unwrap();
    let mut dirty_bytes = source.clone();
    dirty_bytes[pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 8] = 1;
    assert!(save.hd().hd_library.is_dirty(index));
    assert_eq!(
        save.hd().hd_type19_pending.get(&(index, slot, control)),
        Some(&exact),
        "independent edit retains exact stage"
    );
    assert_hd_bytes_eq(
        save.hd().hd_library.file(index).unwrap().as_bytes(),
        &dirty_bytes,
        "independent edit preserves full type-19 source word",
    );
    let directory = tempfile::tempdir().unwrap();
    let collision = directory.path().join("occupied.5xe");
    let saved = directory.path().join("type19-lifecycle.5xe");
    std::fs::write(&collision, b"keep existing").unwrap();
    save.dialogs = Box::new(FakeDialogs {
        pod_save: [Some(collision.clone()), Some(saved.clone())].into(),
        ..Default::default()
    });
    assert!(!save.write_hd_copy(index));
    assert_eq!(std::fs::read(&collision).unwrap(), b"keep existing");
    assert_eq!(
        save.hd().hd_type19_pending.get(&(index, slot, control)),
        Some(&exact),
        "collision preserves exact stage"
    );
    assert_hd_bytes_eq(
        save.hd().hd_library.file(index).unwrap().as_bytes(),
        &dirty_bytes,
        "collision preserves every in-memory byte",
    );
    assert!(save.write_hd_copy(index));
    assert_eq!(
        std::fs::read(&saved).unwrap(),
        dirty_bytes,
        "successful save writes complete expected bytes"
    );
    assert!(!save.hd().hd_library.is_dirty(index));
    assert_eq!(
        save.hd().hd_type19_pending.get(&(index, slot, control)),
        Some(&exact),
        "successful save preserves exact stage"
    );
    let reopen_path = directory.path().join("reopen.5xe");
    std::fs::write(&reopen_path, std::fs::read(&saved).unwrap()).unwrap();
    save.dialogs = Box::new(FakeDialogs {
        pod_open: [Some(reopen_path)].into(),
        ..Default::default()
    });
    click_hd_editor(&context, &mut save, "load-hd-file");
    let reopened = save.hd().hd_selected.expect("saved copy reopens");
    assert!(
        save.hd().hd_type19_pending.is_empty(),
        "reopen invalidates stage belonging to prior entry"
    );
    assert_hd_bytes_eq(
        save.hd().hd_library.file(reopened).unwrap().as_bytes(),
        &dirty_bytes,
        "reopened saved file matches complete saved bytes",
    );
    assert!(!save.hd().hd_library.is_dirty(reopened));

    save.hd_mut().select_hd_file(index);
    save.hd_mut().hd_effect_slot = 5;
    click_hd_type19_choice(&context, &mut save, 7);
    let revert_stage = pending(index, pod_preset::HdType19Choice::Choice7);
    assert_eq!(
        save.hd().hd_type19_pending.get(&(index, slot, control)),
        Some(&revert_stage)
    );
    save.hd_mut()
        .hd_library
        .set_effect_enabled(index, slot, false)
        .unwrap();
    assert!(save.hd().hd_library.is_dirty(index));
    assert_eq!(
        save.hd().hd_type19_pending.get(&(index, slot, control)),
        Some(&revert_stage),
        "dirty independent edit retains the exact stage before revert"
    );
    assert!(save.hd_mut().discard_next_dirty_hd());
    assert!(save.hd().hd_type19_pending.is_empty());
    assert_hd_bytes_eq(
        save.hd().hd_library.file(index).unwrap().as_bytes(),
        &dirty_bytes,
        "revert returns to successful-save baseline",
    );
    assert!(!save.hd().hd_library.is_dirty(index));
}

#[test]
fn hd_type13_gui_applies_all_four_static_words_and_preserves_both_type11_controls() {
    let controls = pod_preset::HdType11Control::for_model(pod_preset::HD_TYPE13_CHOICE_MODEL_ID);
    let type13 = pod_preset::HdType13Control::ALL[0];
    let slot = pod_preset::HdFxSlot::new(5).unwrap();
    for choice in pod_preset::HdType13Choice::ALL {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let original = sample_hd_type11_preset(pod_preset::HD_TYPE13_CHOICE_MODEL_ID);
        let index = app
            .hd_mut()
            .hd_library
            .import_bytes(original.clone())
            .unwrap();
        app.hd_mut().select_hd_file(index);
        app.select_test_editor::<HdEditor>();
        app.hd_mut().hd_effect_slot = 5;
        click_hd_type13_choice(&context, &mut app, choice.index() as usize);
        let staged = PendingHdType13Choice {
            file_index: index,
            slot,
            model_id: type13.model_id(),
            control: type13,
            choice,
        };
        assert_eq!(
            app.hd().hd_type13_pending.get(&(index, slot, type13)),
            Some(&staged)
        );
        assert_hd_bytes_eq(
            app.hd().hd_library.file(index).unwrap().as_bytes(),
            &original,
            "type-0x0D selection stages without changing bytes",
        );
        click_hd_editor(&context, &mut app, hd_type13_apply_button());
        let value = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + 4 * 0x14 + 4;
        let mut expected = original.clone();
        expected[value..value + 4].copy_from_slice(&choice.bits().to_be_bytes());
        assert_hd_bytes_eq(
            app.hd().hd_library.file(index).unwrap().as_bytes(),
            &expected,
            &format!(
                "type-0x0D {:?} Apply changes only its value word; status {}",
                choice,
                app.hd().hd_message
            ),
        );
        assert!(app.hd().hd_type13_pending.is_empty());
        click_hd_type13_choice(&context, &mut app, ((choice.index() + 1) % 4) as usize);
        assert_hd_bytes_eq(
            app.hd().hd_library.file(index).unwrap().as_bytes(),
            &expected,
            "staging another type-0x0D choice remains byte neutral",
        );
        click_hd_editor(&context, &mut app, hd_type13_discard_button());
        assert!(app.hd().hd_type13_pending.is_empty());
        assert_hd_bytes_eq(
            app.hd().hd_library.file(index).unwrap().as_bytes(),
            &expected,
            "discard changes no bytes",
        );
        for control in controls {
            let row = if control.key() == 0x3f10_0001 { 3 } else { 8 };
            let at = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + row * 0x14 + 4;
            assert_eq!(
                &app.hd().hd_library.file(index).unwrap().as_bytes()[at..at + 4],
                &original[at..at + 4],
                "type-11 sibling remains unchanged"
            );
        }
    }
    // Three independent stages coexist; applying the new control retains
    // both type-11 records and alters only its own word.
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let source = sample_hd_type11_preset(pod_preset::HD_TYPE13_CHOICE_MODEL_ID);
    let index = app
        .hd_mut()
        .hd_library
        .import_bytes(source.clone())
        .unwrap();
    app.hd_mut().select_hd_file(index);
    app.select_test_editor::<HdEditor>();
    app.hd_mut().hd_effect_slot = 5;
    click_hd_type11_choice(&context, &mut app, controls[0], 1);
    click_hd_type11_choice(&context, &mut app, controls[1], 4);
    click_hd_type13_choice(&context, &mut app, 3);
    assert_eq!(app.hd().hd_type11_pending.len(), 2);
    assert_eq!(app.hd().hd_type13_pending.len(), 1);
    click_hd_editor(&context, &mut app, hd_type11_discard_button(controls[0]));
    assert_eq!(app.hd().hd_type11_pending.len(), 1);
    assert!(
        app.hd()
            .hd_type11_pending
            .contains_key(&(index, slot, controls[1]))
    );
    assert_eq!(
        app.hd().hd_type13_pending.len(),
        1,
        "type-11 Discard preserves type-0x0D stage"
    );
    click_hd_type11_choice(&context, &mut app, controls[0], 1);
    click_hd_editor(&context, &mut app, hd_type13_discard_button());
    assert_eq!(
        app.hd().hd_type11_pending.len(),
        2,
        "type-0x0D Discard preserves both type-11 stages"
    );
    assert!(app.hd().hd_type13_pending.is_empty());
    click_hd_type13_choice(&context, &mut app, 3);
    let pending_type13 =
        app.hd().hd_type13_pending[&(index, slot, pod_preset::HdType13Control::ALL[0])];
    click_hd_editor(&context, &mut app, hd_type11_apply_button(controls[0]));
    assert_eq!(app.hd().hd_type11_pending.len(), 1);
    assert!(
        app.hd()
            .hd_type11_pending
            .contains_key(&(index, slot, controls[1]))
    );
    assert_eq!(
        app.hd()
            .hd_type13_pending
            .get(&(index, slot, pod_preset::HdType13Control::ALL[0])),
        Some(&pending_type13),
        "type-11 Apply preserves type-0x0D pending state"
    );
    let type11_value = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + 3 * 0x14 + 4;
    let mut after_type11 = source.clone();
    after_type11[type11_value..type11_value + 4].copy_from_slice(&0x3e80_0000u32.to_be_bytes());
    assert_hd_bytes_eq(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        &after_type11,
        "type-11 Apply changes only its own word",
    );
    let before = app.hd().hd_type11_pending.clone();
    click_hd_editor(&context, &mut app, hd_type13_apply_button());
    assert_eq!(
        app.hd().hd_type11_pending,
        before,
        "type-0x0D Apply preserves the remaining type-11 stage"
    );
    assert!(app.hd().hd_type13_pending.is_empty());
    let at = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + 4 * 0x14 + 4;
    assert_eq!(
        &app.hd().hd_library.file(index).unwrap().as_bytes()[at..at + 4],
        &0x3f80_0000u32.to_be_bytes()
    );
    after_type11[at..at + 4].copy_from_slice(&0x3f80_0000u32.to_be_bytes());
    assert_hd_bytes_eq(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        &after_type11,
        "type-0x0D Apply preserves prior type-11 write",
    );
}

#[test]
fn hd_type13_apply_and_discard_preserve_exact_simultaneous_sibling_stages() {
    let controls = pod_preset::HdType11Control::for_model(pod_preset::HD_TYPE13_CHOICE_MODEL_ID);
    let type13 = pod_preset::HdType13Control::ALL[0];
    let slot = pod_preset::HdFxSlot::new(5).unwrap();
    let source = sample_hd_type11_preset(pod_preset::HD_TYPE13_CHOICE_MODEL_ID);
    for apply in [true, false] {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let index = app
            .hd_mut()
            .hd_library
            .import_bytes(source.clone())
            .unwrap();
        app.hd_mut().select_hd_file(index);
        app.select_test_editor::<HdEditor>();
        app.hd_mut().hd_effect_slot = 5;
        click_hd_type11_choice(&context, &mut app, controls[0], 1);
        click_hd_type11_choice(&context, &mut app, controls[1], 4);
        click_hd_type13_choice(&context, &mut app, 3);
        let first = PendingHdType11Choice {
            file_index: index,
            slot,
            model_id: pod_preset::HD_TYPE13_CHOICE_MODEL_ID,
            control: controls[0],
            choice: pod_preset::HdType11Choice::Choice1,
        };
        let second = PendingHdType11Choice {
            file_index: index,
            slot,
            model_id: pod_preset::HD_TYPE13_CHOICE_MODEL_ID,
            control: controls[1],
            choice: pod_preset::HdType11Choice::Choice4,
        };
        let type13_stage = PendingHdType13Choice {
            file_index: index,
            slot,
            model_id: pod_preset::HD_TYPE13_CHOICE_MODEL_ID,
            control: type13,
            choice: pod_preset::HdType13Choice::Choice3,
        };
        assert_eq!(
            app.hd().hd_type11_pending.get(&(index, slot, controls[0])),
            Some(&first)
        );
        assert_eq!(
            app.hd().hd_type11_pending.get(&(index, slot, controls[1])),
            Some(&second)
        );
        assert_eq!(
            app.hd().hd_type13_pending.get(&(index, slot, type13)),
            Some(&type13_stage)
        );

        if apply {
            click_hd_editor(&context, &mut app, hd_type13_apply_button());
            let type13_at = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + 4 * 0x14 + 4;
            let mut expected = source.clone();
            expected[type13_at..type13_at + 4]
                .copy_from_slice(&pod_preset::HdType13Choice::Choice3.bits().to_be_bytes());
            assert_hd_bytes_eq(
                app.hd().hd_library.file(index).unwrap().as_bytes(),
                &expected,
                "type-0x0D Apply changes only its own value with both siblings staged",
            );
            assert!(app.hd().hd_type13_pending.is_empty());
            assert_eq!(
                app.hd().hd_type11_pending.get(&(index, slot, controls[0])),
                Some(&first)
            );
            assert_eq!(
                app.hd().hd_type11_pending.get(&(index, slot, controls[1])),
                Some(&second)
            );
        } else {
            click_hd_editor(&context, &mut app, hd_type13_discard_button());
            assert_hd_bytes_eq(
                app.hd().hd_library.file(index).unwrap().as_bytes(),
                &source,
                "type-0x0D Discard changes no bytes with both siblings staged",
            );
            assert_eq!(app.hd().hd_type13_pending.len(), 0);
            assert_eq!(
                app.hd().hd_type11_pending.get(&(index, slot, controls[0])),
                Some(&first)
            );
            assert_eq!(
                app.hd().hd_type11_pending.get(&(index, slot, controls[1])),
                Some(&second)
            );
        }
    }

    for sibling_index in 0..2 {
        for apply in [true, false] {
            let (context, mut app) = app();
            context.style_mut(|style| style.animation_time = 0.0);
            let index = app
                .hd_mut()
                .hd_library
                .import_bytes(source.clone())
                .unwrap();
            app.hd_mut().select_hd_file(index);
            app.select_test_editor::<HdEditor>();
            app.hd_mut().hd_effect_slot = 5;
            click_hd_type11_choice(&context, &mut app, controls[0], 1);
            click_hd_type11_choice(&context, &mut app, controls[1], 4);
            click_hd_type13_choice(&context, &mut app, 2);
            let kept_control = controls[1 - sibling_index];
            let kept_choice = if sibling_index == 0 {
                pod_preset::HdType11Choice::Choice4
            } else {
                pod_preset::HdType11Choice::Choice1
            };
            let kept = PendingHdType11Choice {
                file_index: index,
                slot,
                model_id: pod_preset::HD_TYPE13_CHOICE_MODEL_ID,
                control: kept_control,
                choice: kept_choice,
            };
            let type13_stage = PendingHdType13Choice {
                file_index: index,
                slot,
                model_id: pod_preset::HD_TYPE13_CHOICE_MODEL_ID,
                control: type13,
                choice: pod_preset::HdType13Choice::Choice2,
            };
            let acted_control = controls[sibling_index];
            if apply {
                click_hd_editor(&context, &mut app, hd_type11_apply_button(acted_control));
            } else {
                click_hd_editor(&context, &mut app, hd_type11_discard_button(acted_control));
            }
            assert_eq!(app.hd().hd_type11_pending.len(), 1);
            assert_eq!(
                app.hd().hd_type11_pending.get(&(index, slot, kept_control)),
                Some(&kept)
            );
            assert_eq!(
                app.hd().hd_type13_pending.get(&(index, slot, type13)),
                Some(&type13_stage)
            );
            let mut expected = source.clone();
            if apply {
                let row = if sibling_index == 0 { 3 } else { 8 };
                let at = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + row * 0x14 + 4;
                let choice = if sibling_index == 0 {
                    pod_preset::HdType11Choice::Choice1
                } else {
                    pod_preset::HdType11Choice::Choice4
                };
                expected[at..at + 4].copy_from_slice(&choice.bits().to_be_bytes());
            }
            assert_hd_bytes_eq(
                app.hd().hd_library.file(index).unwrap().as_bytes(),
                &expected,
                "type-11 action changes only the acted-on control",
            );
        }
    }
}

#[test]
fn hd_type13_stage_lifecycle_covers_context_open_save_collision_and_revert() {
    let control = pod_preset::HdType13Control::ALL[0];
    let slot = pod_preset::HdFxSlot::new(5).unwrap();
    let model_id = pod_preset::HD_TYPE13_CHOICE_MODEL_ID;
    let source = sample_hd_type11_preset(model_id);
    let pending = |index, choice| PendingHdType13Choice {
        file_index: index,
        slot,
        model_id,
        control,
        choice,
    };

    // Changing the selected FX slot clears only the stale type-0x0D stage.
    let (context, mut by_slot) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let index = by_slot
        .hd_mut()
        .hd_library
        .import_bytes(source.clone())
        .unwrap();
    by_slot.hd_mut().select_hd_file(index);
    by_slot.select_test_editor::<HdEditor>();
    by_slot.hd_mut().hd_effect_slot = 5;
    click_hd_type13_choice(&context, &mut by_slot, 2);
    assert_eq!(
        by_slot.hd().hd_type13_pending.get(&(index, slot, control)),
        Some(&pending(index, pod_preset::HdType13Choice::Choice2))
    );
    by_slot.hd_mut().hd_effect_slot = 4;
    hd_editor_frame(&context, &mut by_slot, vec![]);
    assert!(
        by_slot.hd().hd_type13_pending.is_empty(),
        "slot change clears stale stage"
    );
    assert_hd_bytes_eq(
        by_slot.hd().hd_library.file(index).unwrap().as_bytes(),
        &source,
        "slot invalidation leaves full source unchanged",
    );

    // Replacing the model in the staged slot invalidates the stage without
    // altering the replacement record or any unrelated bytes.
    let (context, mut by_model) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let index = by_model
        .hd_mut()
        .hd_library
        .import_bytes(source.clone())
        .unwrap();
    by_model.hd_mut().select_hd_file(index);
    by_model.select_test_editor::<HdEditor>();
    by_model.hd_mut().hd_effect_slot = 5;
    click_hd_type13_choice(&context, &mut by_model, 1);
    let mut changed_model = source.clone();
    let model_at = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100;
    changed_model[model_at..model_at + 4].copy_from_slice(&0x020a_001e_u32.to_be_bytes());
    *by_model.hd_mut().hd_library.file_mut(index).unwrap() =
        HdEditFile::parse(changed_model.clone()).unwrap();
    hd_editor_frame(&context, &mut by_model, vec![]);
    assert!(
        by_model.hd().hd_type13_pending.is_empty(),
        "model change clears stale stage"
    );
    assert_hd_bytes_eq(
        by_model.hd().hd_library.file(index).unwrap().as_bytes(),
        &changed_model,
        "model invalidation preserves all replacement bytes",
    );

    // Selecting another file clears the prior entry's exact stage.
    let (context, mut by_entry) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let index = by_entry
        .hd_mut()
        .hd_library
        .import_bytes(source.clone())
        .unwrap();
    by_entry.hd_mut().select_hd_file(index);
    by_entry.select_test_editor::<HdEditor>();
    by_entry.hd_mut().hd_effect_slot = 5;
    click_hd_type13_choice(&context, &mut by_entry, 0);
    assert_eq!(
        by_entry.hd().hd_type13_pending.get(&(index, slot, control)),
        Some(&pending(index, pod_preset::HdType13Choice::Choice0))
    );
    let other = by_entry
        .hd_mut()
        .hd_library
        .import_bytes(source.clone())
        .unwrap();
    by_entry.hd_mut().select_hd_file(other);
    assert!(
        by_entry.hd().hd_type13_pending.is_empty(),
        "selection change clears prior entry stage"
    );
    assert_hd_bytes_eq(
        by_entry.hd().hd_library.file(index).unwrap().as_bytes(),
        &source,
        "entry invalidation preserves old bytes",
    );

    // Cancelled Open retains its stage; successful Open clears it and
    // leaves exact complete-file bytes in both the old and new entries.
    let (context, mut open) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let index = open
        .hd_mut()
        .hd_library
        .import_bytes(source.clone())
        .unwrap();
    open.hd_mut().select_hd_file(index);
    open.select_test_editor::<HdEditor>();
    open.hd_mut().hd_effect_slot = 5;
    click_hd_type13_choice(&context, &mut open, 3);
    let staged = pending(index, pod_preset::HdType13Choice::Choice3);
    let replacement_dir = tempfile::tempdir().unwrap();
    let replacement_path = replacement_dir.path().join("type13-open.5xe");
    let replacement_bytes = sample_hd_type11_preset(0x020a_001e);
    std::fs::write(&replacement_path, &replacement_bytes).unwrap();
    open.dialogs = Box::new(FakeDialogs {
        pod_open: [None, Some(replacement_path)].into(),
        ..Default::default()
    });
    click_hd_editor(&context, &mut open, "load-hd-file");
    assert_eq!(
        open.hd().hd_type13_pending.get(&(index, slot, control)),
        Some(&staged),
        "cancelled Open retains exact stage"
    );
    assert_hd_bytes_eq(
        open.hd().hd_library.file(index).unwrap().as_bytes(),
        &source,
        "cancelled Open retains full bytes",
    );
    click_hd_editor(&context, &mut open, "load-hd-file");
    let opened = open
        .hd()
        .hd_selected
        .expect("successful Open selects replacement");
    assert_ne!(opened, index);
    assert!(
        open.hd().hd_type13_pending.is_empty(),
        "successful Open clears old-entry stage"
    );
    assert_hd_bytes_eq(
        open.hd().hd_library.file(index).unwrap().as_bytes(),
        &source,
        "successful Open preserves old-entry bytes",
    );
    assert_hd_bytes_eq(
        open.hd().hd_library.file(opened).unwrap().as_bytes(),
        &replacement_bytes,
        "successful Open stores complete replacement bytes",
    );

    // An applied sibling makes the entry dirty while type-0x0D remains
    // staged. Both failed collision and successful save preserve the exact
    // pending record and current bytes; reopen intentionally invalidates it.
    let (context, mut save) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let index = save
        .hd_mut()
        .hd_library
        .import_bytes(source.clone())
        .unwrap();
    save.hd_mut().select_hd_file(index);
    save.select_test_editor::<HdEditor>();
    save.hd_mut().hd_effect_slot = 5;
    click_hd_type13_choice(&context, &mut save, 2);
    let staged = pending(index, pod_preset::HdType13Choice::Choice2);
    let controls = pod_preset::HdType11Control::for_model(model_id);
    click_hd_type11_choice(&context, &mut save, controls[0], 1);
    click_hd_editor(&context, &mut save, hd_type11_apply_button(controls[0]));
    let type11_at = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + 3 * 0x14 + 4;
    let mut dirty_bytes = source.clone();
    dirty_bytes[type11_at..type11_at + 4]
        .copy_from_slice(&pod_preset::HdType11Choice::Choice1.bits().to_be_bytes());
    assert!(save.hd().hd_library.is_dirty(index));
    assert_eq!(
        save.hd().hd_type13_pending.get(&(index, slot, control)),
        Some(&staged)
    );
    assert_hd_bytes_eq(
        save.hd().hd_library.file(index).unwrap().as_bytes(),
        &dirty_bytes,
        "sibling Apply preserves staged source bytes",
    );
    let directory = tempfile::tempdir().unwrap();
    let collision = directory.path().join("occupied.5xe");
    let saved = directory.path().join("type13-lifecycle.5xe");
    std::fs::write(&collision, b"keep existing").unwrap();
    save.dialogs = Box::new(FakeDialogs {
        pod_save: [Some(collision.clone()), Some(saved.clone())].into(),
        ..Default::default()
    });
    assert!(!save.write_hd_copy(index));
    assert_eq!(std::fs::read(&collision).unwrap(), b"keep existing");
    assert_eq!(
        save.hd().hd_type13_pending.get(&(index, slot, control)),
        Some(&staged),
        "collision retains exact stage"
    );
    assert_hd_bytes_eq(
        save.hd().hd_library.file(index).unwrap().as_bytes(),
        &dirty_bytes,
        "collision retains complete in-memory bytes",
    );
    assert!(save.write_hd_copy(index));
    assert_eq!(
        std::fs::read(&saved).unwrap(),
        dirty_bytes,
        "successful save writes complete expected bytes"
    );
    assert!(!save.hd().hd_library.is_dirty(index));
    assert_eq!(
        save.hd().hd_type13_pending.get(&(index, slot, control)),
        Some(&staged),
        "successful save preserves exact stage"
    );
    assert_hd_bytes_eq(
        save.hd().hd_library.file(index).unwrap().as_bytes(),
        &dirty_bytes,
        "save preserves complete current bytes",
    );
    let reopened_bytes = std::fs::read(&saved).unwrap();
    let reopen_path = directory.path().join("reopen.5xe");
    std::fs::write(&reopen_path, &reopened_bytes).unwrap();
    save.dialogs = Box::new(FakeDialogs {
        pod_open: [Some(reopen_path)].into(),
        ..Default::default()
    });
    click_hd_editor(&context, &mut save, "load-hd-file");
    let reopened = save.hd().hd_selected.expect("saved copy reopens");
    assert!(
        save.hd().hd_type13_pending.is_empty(),
        "successful reopen clears old stage"
    );
    assert_hd_bytes_eq(
        save.hd().hd_library.file(reopened).unwrap().as_bytes(),
        &dirty_bytes,
        "reopened saved copy has complete expected bytes",
    );
    assert!(!save.hd().hd_library.is_dirty(reopened));

    save.hd_mut().select_hd_file(index);
    save.hd_mut().hd_effect_slot = 5;
    click_hd_type11_choice(&context, &mut save, controls[0], 4);
    click_hd_editor(&context, &mut save, hd_type11_apply_button(controls[0]));
    assert!(save.hd().hd_library.is_dirty(index));
    assert!(save.hd_mut().discard_next_dirty_hd());
    assert_hd_bytes_eq(
        save.hd().hd_library.file(index).unwrap().as_bytes(),
        &dirty_bytes,
        "revert restores the exact successful-save baseline",
    );
    assert!(!save.hd().hd_library.is_dirty(index));
    assert!(save.hd().hd_type13_pending.is_empty());
}

#[test]
fn hd_type13_failed_apply_retains_dirty_baseline_and_all_three_stages() {
    let controls = pod_preset::HdType11Control::for_model(pod_preset::HD_TYPE13_CHOICE_MODEL_ID);
    let control = pod_preset::HdType13Control::ALL[0];
    let slot = pod_preset::HdFxSlot::new(5).unwrap();
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let original = sample_hd_type11_preset(pod_preset::HD_TYPE13_CHOICE_MODEL_ID);
    let index = app
        .hd_mut()
        .hd_library
        .import_bytes(original.clone())
        .unwrap();
    app.hd_mut().select_hd_file(index);
    app.select_test_editor::<HdEditor>();
    app.hd_mut().hd_effect_slot = 5;
    click_hd_type13_choice(&context, &mut app, 2);
    let type13_stage = app.hd().hd_type13_pending[&(index, slot, control)];
    click_hd_type11_choice(&context, &mut app, controls[0], 1);
    click_hd_editor(&context, &mut app, hd_type11_apply_button(controls[0]));
    assert!(app.hd().hd_library.is_dirty(index));
    click_hd_type11_choice(&context, &mut app, controls[0], 3);
    click_hd_type11_choice(&context, &mut app, controls[1], 4);
    assert_eq!(app.hd().hd_type11_pending.len(), 2);
    assert_eq!(
        app.hd().hd_type13_pending.get(&(index, slot, control)),
        Some(&type13_stage)
    );

    let mut invalid = app.hd().hd_library.file(index).unwrap().as_bytes().to_vec();
    let marker = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + 4 * 0x14 + 0x10;
    invalid[marker] = 1;
    match app.hd_mut().hd_library.file_mut(index).unwrap() {
        HdEditFile::H5ep(preset) => {
            *preset = pod_preset::OpaqueH5epPreset::parse(invalid.clone()).unwrap()
        }
        _ => panic!("test fixture is H5EP"),
    }
    let type11_stages = app.hd().hd_type11_pending.clone();
    assert!(app.hd().hd_library.is_dirty(index));
    click_hd_editor(&context, &mut app, hd_type13_apply_button());
    assert_eq!(app.hd().hd_library.file(index).unwrap().as_bytes(), invalid);
    assert!(app.hd().hd_library.is_dirty(index));
    assert_eq!(
        app.hd().hd_type13_pending.get(&(index, slot, control)),
        Some(&type13_stage)
    );
    assert_eq!(app.hd().hd_type11_pending, type11_stages);
    assert!(app.hd().hd_message.contains("unassigned marker"));
    assert!(app.hd_mut().discard_next_dirty_hd());
    assert_hd_bytes_eq(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        &original,
        "revert restores saved baseline after failed target Apply",
    );
    assert!(!app.hd().hd_library.is_dirty(index));
    assert!(app.hd().hd_type11_pending.is_empty());
    assert!(app.hd().hd_type13_pending.is_empty());
}

#[test]
fn hd_type13_quit_keep_discard_and_apply_resolve_all_three_stages_first() {
    let controls = pod_preset::HdType11Control::for_model(pod_preset::HD_TYPE13_CHOICE_MODEL_ID);
    let source = sample_hd_type11_preset(pod_preset::HD_TYPE13_CHOICE_MODEL_ID);

    let (context, mut keep) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let keep_index = keep
        .hd_mut()
        .hd_library
        .import_bytes(source.clone())
        .unwrap();
    keep.hd_mut().select_hd_file(keep_index);
    keep.select_test_editor::<HdEditor>();
    keep.hd_mut().hd_effect_slot = 5;
    click_hd_type11_choice(&context, &mut keep, controls[0], 1);
    click_hd_type11_choice(&context, &mut keep, controls[1], 4);
    click_hd_type13_choice(&context, &mut keep, 2);
    let keep_first = PendingHdType11Choice {
        file_index: keep_index,
        slot: pod_preset::HdFxSlot::new(5).unwrap(),
        model_id: pod_preset::HD_TYPE13_CHOICE_MODEL_ID,
        control: controls[0],
        choice: pod_preset::HdType11Choice::Choice1,
    };
    let keep_second = PendingHdType11Choice {
        file_index: keep_index,
        slot: pod_preset::HdFxSlot::new(5).unwrap(),
        model_id: pod_preset::HD_TYPE13_CHOICE_MODEL_ID,
        control: controls[1],
        choice: pod_preset::HdType11Choice::Choice4,
    };
    let keep_type13 = PendingHdType13Choice {
        file_index: keep_index,
        slot: pod_preset::HdFxSlot::new(5).unwrap(),
        model_id: pod_preset::HD_TYPE13_CHOICE_MODEL_ID,
        control: pod_preset::HdType13Control::ALL[0],
        choice: pod_preset::HdType13Choice::Choice2,
    };
    keep.request_close(&context);
    assert!(matches!(keep.test_pending(), Some(Pending::QuitHd)));
    click_prompt(&context, &mut keep, "cancel");
    assert!(keep.test_pending().is_none());
    assert_eq!(keep.hd().hd_type11_pending.len(), 2);
    assert_eq!(
        keep.hd()
            .hd_type11_pending
            .get(&(keep_index, keep_type13.slot, controls[0])),
        Some(&keep_first)
    );
    assert_eq!(
        keep.hd()
            .hd_type11_pending
            .get(&(keep_index, keep_type13.slot, controls[1])),
        Some(&keep_second)
    );
    assert_eq!(
        keep.hd()
            .hd_type13_pending
            .get(&(keep_index, keep_type13.slot, keep_type13.control)),
        Some(&keep_type13)
    );
    assert!(!keep.permit_close, "Keep editing must not close the window");
    assert_hd_bytes_eq(
        keep.hd().hd_library.file(keep_index).unwrap().as_bytes(),
        &source,
        "Keep editing preserves all three stages and bytes",
    );

    let (context, mut discard) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let discard_index = discard
        .hd_mut()
        .hd_library
        .import_bytes(source.clone())
        .unwrap();
    discard.hd_mut().select_hd_file(discard_index);
    discard.select_test_editor::<HdEditor>();
    discard.hd_mut().hd_effect_slot = 5;
    click_hd_type11_choice(&context, &mut discard, controls[0], 1);
    click_hd_type11_choice(&context, &mut discard, controls[1], 4);
    click_hd_type13_choice(&context, &mut discard, 2);
    let discard_first = PendingHdType11Choice {
        file_index: discard_index,
        slot: pod_preset::HdFxSlot::new(5).unwrap(),
        model_id: pod_preset::HD_TYPE13_CHOICE_MODEL_ID,
        control: controls[0],
        choice: pod_preset::HdType11Choice::Choice1,
    };
    let discard_second = PendingHdType11Choice {
        file_index: discard_index,
        slot: pod_preset::HdFxSlot::new(5).unwrap(),
        model_id: pod_preset::HD_TYPE13_CHOICE_MODEL_ID,
        control: controls[1],
        choice: pod_preset::HdType11Choice::Choice4,
    };
    let discard_type13 = PendingHdType13Choice {
        file_index: discard_index,
        slot: pod_preset::HdFxSlot::new(5).unwrap(),
        model_id: pod_preset::HD_TYPE13_CHOICE_MODEL_ID,
        control: pod_preset::HdType13Control::ALL[0],
        choice: pod_preset::HdType13Choice::Choice2,
    };
    discard.request_close(&context);
    click_prompt(&context, &mut discard, "discard");
    assert_eq!(discard.hd().hd_type11_pending.len(), 1);
    let remaining = discard
        .hd()
        .hd_type11_pending
        .values()
        .next()
        .copied()
        .unwrap();
    assert!(remaining == discard_first || remaining == discard_second);
    assert_eq!(
        discard.hd().hd_type13_pending.get(&(
            discard_index,
            discard_type13.slot,
            discard_type13.control
        )),
        Some(&discard_type13)
    );
    assert!(
        !discard.permit_close,
        "one remaining type-11 stage blocks close"
    );
    assert_hd_bytes_eq(
        discard
            .hd()
            .hd_library
            .file(discard_index)
            .unwrap()
            .as_bytes(),
        &source,
        "first quit Discard keeps complete source bytes",
    );
    click_prompt(&context, &mut discard, "discard");
    assert!(discard.hd().hd_type11_pending.is_empty());
    assert_eq!(
        discard.hd().hd_type13_pending.get(&(
            discard_index,
            discard_type13.slot,
            discard_type13.control
        )),
        Some(&discard_type13)
    );
    assert!(
        !discard.permit_close,
        "remaining type-0x0D stage blocks close"
    );
    assert_hd_bytes_eq(
        discard
            .hd()
            .hd_library
            .file(discard_index)
            .unwrap()
            .as_bytes(),
        &source,
        "second quit Discard keeps complete source bytes",
    );
    click_prompt(&context, &mut discard, "discard");
    assert!(discard.hd().hd_type13_pending.is_empty());
    assert!(
        discard.permit_close,
        "quit discard closes only after all three stages are cleared"
    );
    assert_hd_bytes_eq(
        discard
            .hd()
            .hd_library
            .file(discard_index)
            .unwrap()
            .as_bytes(),
        &source,
        "quit discard leaves preset bytes unchanged",
    );

    let (context, mut apply) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let apply_index = apply
        .hd_mut()
        .hd_library
        .import_bytes(source.clone())
        .unwrap();
    apply.hd_mut().select_hd_file(apply_index);
    apply.select_test_editor::<HdEditor>();
    apply.hd_mut().hd_effect_slot = 5;
    click_hd_type11_choice(&context, &mut apply, controls[0], 1);
    click_hd_type11_choice(&context, &mut apply, controls[1], 4);
    click_hd_type13_choice(&context, &mut apply, 3);
    let apply_type13 = PendingHdType13Choice {
        file_index: apply_index,
        slot: pod_preset::HdFxSlot::new(5).unwrap(),
        model_id: pod_preset::HD_TYPE13_CHOICE_MODEL_ID,
        control: pod_preset::HdType13Control::ALL[0],
        choice: pod_preset::HdType13Choice::Choice3,
    };
    let apply_first = PendingHdType11Choice {
        file_index: apply_index,
        slot: pod_preset::HdFxSlot::new(5).unwrap(),
        model_id: pod_preset::HD_TYPE13_CHOICE_MODEL_ID,
        control: controls[0],
        choice: pod_preset::HdType11Choice::Choice1,
    };
    let apply_second = PendingHdType11Choice {
        file_index: apply_index,
        slot: pod_preset::HdFxSlot::new(5).unwrap(),
        model_id: pod_preset::HD_TYPE13_CHOICE_MODEL_ID,
        control: controls[1],
        choice: pod_preset::HdType11Choice::Choice4,
    };
    assert_eq!(
        apply
            .hd()
            .hd_type11_pending
            .get(&(apply_index, apply_first.slot, controls[0])),
        Some(&apply_first)
    );
    assert_eq!(
        apply
            .hd()
            .hd_type11_pending
            .get(&(apply_index, apply_second.slot, controls[1])),
        Some(&apply_second)
    );
    assert_eq!(
        apply
            .hd()
            .hd_type13_pending
            .get(&(apply_index, apply_type13.slot, apply_type13.control)),
        Some(&apply_type13)
    );
    apply.request_close(&context);
    let before_first_apply = apply.hd().hd_type11_pending.clone();
    click_prompt(&context, &mut apply, "apply-type11");
    assert_eq!(apply.hd().hd_type11_pending.len(), 1);
    assert_eq!(
        apply
            .hd()
            .hd_type13_pending
            .get(&(apply_index, apply_type13.slot, apply_type13.control)),
        Some(&apply_type13)
    );
    let first_applied = before_first_apply
        .values()
        .find(|pending| {
            !apply.hd().hd_type11_pending.contains_key(&(
                apply_index,
                pending.slot,
                pending.control,
            ))
        })
        .copied()
        .unwrap();
    let remaining_type11 = apply
        .hd()
        .hd_type11_pending
        .values()
        .next()
        .copied()
        .unwrap();
    let mut expected_after_first = source.clone();
    let first_row = if first_applied.control == controls[0] {
        3
    } else {
        8
    };
    let first_at = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + first_row * 0x14 + 4;
    expected_after_first[first_at..first_at + 4]
        .copy_from_slice(&first_applied.choice.bits().to_be_bytes());
    assert_eq!(
        apply.hd().hd_type11_pending.get(&(
            apply_index,
            remaining_type11.slot,
            remaining_type11.control
        )),
        Some(&remaining_type11)
    );
    assert_hd_bytes_eq(
        apply.hd().hd_library.file(apply_index).unwrap().as_bytes(),
        &expected_after_first,
        "first quit Apply changes only its exact type-11 stage",
    );
    let before_second_apply = apply
        .hd()
        .hd_type11_pending
        .values()
        .next()
        .copied()
        .unwrap();
    click_prompt(&context, &mut apply, "apply-type11");
    assert!(apply.hd().hd_type11_pending.is_empty());
    assert_eq!(
        apply
            .hd()
            .hd_type13_pending
            .get(&(apply_index, apply_type13.slot, apply_type13.control)),
        Some(&apply_type13)
    );
    let second_row = if before_second_apply.control == controls[0] {
        3
    } else {
        8
    };
    let second_at =
        pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + second_row * 0x14 + 4;
    let mut expected_after_second = expected_after_first.clone();
    expected_after_second[second_at..second_at + 4]
        .copy_from_slice(&before_second_apply.choice.bits().to_be_bytes());
    assert_hd_bytes_eq(
        apply.hd().hd_library.file(apply_index).unwrap().as_bytes(),
        &expected_after_second,
        "second quit Apply changes only the remaining type-11 stage",
    );
    click_prompt(&context, &mut apply, "apply-type13");
    assert!(apply.hd().hd_type13_pending.is_empty());
    let type13_at = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + 4 * 0x14 + 4;
    let mut expected_after_type13 = expected_after_second;
    expected_after_type13[type13_at..type13_at + 4]
        .copy_from_slice(&apply_type13.choice.bits().to_be_bytes());
    assert_hd_bytes_eq(
        apply.hd().hd_library.file(apply_index).unwrap().as_bytes(),
        &expected_after_type13,
        "quit type-0x0D Apply follows both exact type-11 writes",
    );
    assert!(apply.hd().hd_library.is_dirty(apply_index));
    assert!(
        matches!(apply.test_pending(), Some(Pending::QuitHd)),
        "the unsaved file decision follows all staged controls"
    );
    assert!(
        !apply.permit_close,
        "unsaved file decision keeps the window open"
    );
    click_prompt(&context, &mut apply, "discard");
    assert!(apply.permit_close);
    assert_hd_bytes_eq(
        apply.hd().hd_library.file(apply_index).unwrap().as_bytes(),
        &source,
        "file discard restores original saved baseline after all control Apply operations",
    );
}

#[test]
fn hd_type11_same_model_controls_stage_apply_and_discard_independently() {
    for model_id in [0x020a_001e, 0x020a_001d] {
        let controls = pod_preset::HdType11Control::for_model(model_id);
        let first = controls[0];
        let second = controls[1];
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let original = sample_hd_type11_preset(model_id);
        let index = app
            .hd_mut()
            .hd_library
            .import_bytes(original.clone())
            .unwrap();
        app.hd_mut().select_hd_file(index);
        app.select_test_editor::<HdEditor>();
        app.hd_mut().hd_effect_slot = 5;
        let slot = pod_preset::HdFxSlot::new(5).unwrap();
        let first_at = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + 3 * 0x14 + 4;
        let second_at = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + 8 * 0x14 + 4;

        click_hd_type11_choice(&context, &mut app, first, 1);
        let first_pending = app.hd().hd_type11_pending[&(index, slot, first)];
        assert_eq!(first_pending.choice, pod_preset::HdType11Choice::Choice1);
        assert!(
            !app.hd()
                .hd_type11_pending
                .contains_key(&(index, slot, second))
        );
        assert_hd_bytes_eq(
            app.hd().hd_library.file(index).unwrap().as_bytes(),
            &original,
            "first stage",
        );
        click_hd_type11_choice(&context, &mut app, second, 3);
        let second_pending = app.hd().hd_type11_pending[&(index, slot, second)];
        assert_eq!(second_pending.choice, pod_preset::HdType11Choice::Choice3);
        assert_eq!(
            app.hd().hd_type11_pending[&(index, slot, first)],
            first_pending
        );
        assert_hd_bytes_eq(
            app.hd().hd_library.file(index).unwrap().as_bytes(),
            &original,
            "both stages",
        );

        click_hd_editor(&context, &mut app, hd_type11_apply_button(first));
        let mut after_first = original.clone();
        after_first[first_at..first_at + 4].copy_from_slice(&0x3e80_0000u32.to_be_bytes());
        assert_hd_bytes_eq(
            app.hd().hd_library.file(index).unwrap().as_bytes(),
            &after_first,
            "apply first key only",
        );
        assert!(
            !app.hd()
                .hd_type11_pending
                .contains_key(&(index, slot, first))
        );
        assert_eq!(
            app.hd().hd_type11_pending.get(&(index, slot, second)),
            Some(&second_pending)
        );

        click_hd_type11_choice(&context, &mut app, first, 4);
        let first_pending = app.hd().hd_type11_pending[&(index, slot, first)];
        assert_eq!(
            app.hd().hd_type11_pending.get(&(index, slot, second)),
            Some(&second_pending)
        );
        click_hd_editor(&context, &mut app, hd_type11_apply_button(second));
        let mut after_second = after_first.clone();
        after_second[second_at..second_at + 4].copy_from_slice(&0x3f40_0000u32.to_be_bytes());
        assert_hd_bytes_eq(
            app.hd().hd_library.file(index).unwrap().as_bytes(),
            &after_second,
            "apply second key while both stages exist",
        );
        assert_eq!(
            app.hd().hd_type11_pending.get(&(index, slot, first)),
            Some(&first_pending),
            "reverse Apply preserves the first exact stage"
        );
        assert!(
            !app.hd()
                .hd_type11_pending
                .contains_key(&(index, slot, second))
        );

        click_hd_editor(&context, &mut app, hd_type11_apply_button(first));
        let mut after_both = after_second;
        after_both[first_at..first_at + 4].copy_from_slice(&0x3f80_0000u32.to_be_bytes());
        assert_hd_bytes_eq(
            app.hd().hd_library.file(index).unwrap().as_bytes(),
            &after_both,
            "apply preserved first stage after reverse Apply",
        );
        assert!(app.hd().hd_type11_pending.is_empty());

        click_hd_type11_choice(&context, &mut app, first, 2);
        let first_pending = app.hd().hd_type11_pending[&(index, slot, first)];
        click_hd_type11_choice(&context, &mut app, second, 4);
        let second_pending = app.hd().hd_type11_pending[&(index, slot, second)];
        assert_eq!(
            app.hd().hd_type11_pending.get(&(index, slot, first)),
            Some(&first_pending)
        );
        click_hd_editor(&context, &mut app, hd_type11_discard_button(first));
        assert_eq!(
            app.hd().hd_type11_pending.get(&(index, slot, second)),
            Some(&second_pending)
        );
        assert!(
            !app.hd()
                .hd_type11_pending
                .contains_key(&(index, slot, first))
        );
        assert_hd_bytes_eq(
            app.hd().hd_library.file(index).unwrap().as_bytes(),
            &after_both,
            "discard first preserves second stage and applied bytes",
        );
        click_hd_editor(&context, &mut app, hd_type11_apply_button(second));
        let mut final_bytes = after_both;
        final_bytes[second_at..second_at + 4].copy_from_slice(&0x3f80_0000u32.to_be_bytes());
        assert_hd_bytes_eq(
            app.hd().hd_library.file(index).unwrap().as_bytes(),
            &final_bytes,
            "apply second key only",
        );
        assert!(app.hd().hd_type11_pending.is_empty());

        click_hd_type11_choice(&context, &mut app, first, 1);
        let first_pending = app.hd().hd_type11_pending[&(index, slot, first)];
        click_hd_type11_choice(&context, &mut app, second, 2);
        assert_eq!(app.hd().hd_type11_pending.len(), 2);
        click_hd_editor(&context, &mut app, hd_type11_discard_button(second));
        assert_eq!(
            app.hd().hd_type11_pending.get(&(index, slot, first)),
            Some(&first_pending),
            "discarding second preserves the first exact stage"
        );
        assert!(
            !app.hd()
                .hd_type11_pending
                .contains_key(&(index, slot, second))
        );
        assert_hd_bytes_eq(
            app.hd().hd_library.file(index).unwrap().as_bytes(),
            &final_bytes,
            "discard second leaves both stored words intact",
        );
        click_hd_editor(&context, &mut app, hd_type11_apply_button(first));
        let mut after_discard_second = final_bytes;
        after_discard_second[first_at..first_at + 4].copy_from_slice(&0x3e80_0000u32.to_be_bytes());
        assert_hd_bytes_eq(
            app.hd().hd_library.file(index).unwrap().as_bytes(),
            &after_discard_second,
            "apply first after discarding second",
        );
    }
}

#[test]
fn hd_type11_failed_apply_retains_stage_and_file_state() {
    for model_id in [0x020a_001e, 0x020a_001d] {
        for control in pod_preset::HdType11Control::for_model(model_id)
            .iter()
            .copied()
        {
            hd_type11_failed_apply_retains_stage_and_file_state_for(model_id, control);
        }
    }
}

fn hd_type11_failed_apply_retains_stage_and_file_state_for(
    model_id: u32,
    control: pod_preset::HdType11Control,
) {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let sibling = pod_preset::HdType11Control::for_model(model_id)
        .iter()
        .copied()
        .find(|candidate| *candidate != control)
        .unwrap();
    let original = sample_hd_type11_preset(model_id);
    let index = app
        .hd_mut()
        .hd_library
        .import_bytes(original.clone())
        .unwrap();
    app.hd_mut().select_hd_file(index);
    app.select_test_editor::<HdEditor>();
    app.hd_mut().hd_effect_slot = 5;
    let slot = pod_preset::HdFxSlot::new(5).unwrap();

    let directory = tempfile::tempdir().unwrap();
    let baseline_path = directory.path().join("type11-baseline.5xe");
    app.dialogs = Box::new(FakeDialogs {
        pod_save: [Some(baseline_path.clone())].into(),
        ..Default::default()
    });
    hd_editor_frame(&context, &mut app, vec![]);
    click_hd_editor(&context, &mut app, "save-hd-copy");
    assert_hd_bytes_eq(
        &std::fs::read(&baseline_path).unwrap(),
        &original,
        "saved baseline contains the original source bytes",
    );
    assert!(!app.hd().hd_library.is_dirty(index));

    click_hd_type11_choice(&context, &mut app, control, 4);
    let target_stage = app.hd().hd_type11_pending[&(index, slot, control)];
    assert_eq!(
        target_stage,
        PendingHdType11Choice {
            file_index: index,
            slot,
            model_id,
            control,
            choice: pod_preset::HdType11Choice::Choice4,
        },
        "target stage is created through the visible choice control"
    );

    click_hd_type11_choice(&context, &mut app, sibling, 1);
    let sibling_initial = app.hd().hd_type11_pending[&(index, slot, sibling)];
    assert_eq!(sibling_initial.choice, pod_preset::HdType11Choice::Choice1);
    click_hd_editor(&context, &mut app, hd_type11_apply_button(sibling));
    let mut dirty_bytes = original.clone();
    let sibling_row = if sibling == pod_preset::HdType11Control::for_model(model_id)[0] {
        3
    } else {
        8
    };
    let sibling_value =
        pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + sibling_row * 0x14 + 4;
    dirty_bytes[sibling_value..sibling_value + 4].copy_from_slice(&0x3e80_0000u32.to_be_bytes());
    assert_hd_bytes_eq(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        &dirty_bytes,
        "applying sibling creates dirty state without altering target source word",
    );
    assert!(app.hd().hd_library.is_dirty(index));
    assert_eq!(
        app.hd().hd_type11_pending.get(&(index, slot, control)),
        Some(&target_stage),
        "sibling Apply preserves target stage"
    );

    click_hd_type11_choice(&context, &mut app, sibling, 3);
    let sibling_stage = app.hd().hd_type11_pending[&(index, slot, sibling)];
    assert_eq!(sibling_stage.choice, pod_preset::HdType11Choice::Choice3);
    assert_eq!(
        app.hd().hd_type11_pending.get(&(index, slot, control)),
        Some(&target_stage)
    );
    assert_eq!(app.hd().hd_type11_pending.len(), 2);

    let target_marker = pod_preset::H5EP_HEADER_BYTES
        + 0x428
        + 5 * 0x100
        + 0x10
        + (if control == pod_preset::HdType11Control::for_model(model_id)[0] {
            3
        } else {
            8
        }) * 0x14
        + 0x10;
    let mut invalid_bytes = dirty_bytes;
    invalid_bytes[target_marker] = 1;
    let target_model_at = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100;
    *app.hd_mut().hd_library.file_mut(index).unwrap() =
        HdEditFile::parse(invalid_bytes.clone()).unwrap();
    assert_eq!(
        u32::from_be_bytes(
            app.hd().hd_library.file(index).unwrap().as_bytes()
                [target_model_at..target_model_at + 4]
                .try_into()
                .unwrap(),
        ),
        model_id,
        "making the target marker invalid does not change its model identity"
    );
    assert!(app.hd().hd_library.is_dirty(index));
    hd_editor_frame(&context, &mut app, vec![]);
    click_hd_editor(&context, &mut app, hd_type11_apply_button(control));
    assert_hd_bytes_eq(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        &invalid_bytes,
        "failed GUI Apply preserves invalid target marker and all file bytes",
    );
    assert!(app.hd().hd_library.is_dirty(index));
    assert_eq!(
        app.hd().hd_type11_pending.get(&(index, slot, control)),
        Some(&target_stage),
        "failed Apply retains the exact target pending record"
    );
    assert_eq!(
        app.hd().hd_type11_pending.get(&(index, slot, sibling)),
        Some(&sibling_stage),
        "failed Apply retains the exact sibling pending record"
    );

    assert!(app.hd_mut().discard_next_dirty_hd());
    assert_hd_bytes_eq(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        &original,
        "revert restores the exact saved baseline",
    );
    assert!(!app.hd().hd_library.is_dirty(index));
    assert!(
        !app.hd()
            .hd_type11_pending
            .keys()
            .any(|(file_index, _, _)| *file_index == index),
        "revert clears only stages belonging to the reverted file"
    );
    assert!(
        app.hd()
            .hd_message
            .contains("Discarded one unsaved HD Edit file change")
    );
}

#[test]
fn hd_type11_cancelled_and_successful_open_handle_stages_and_model_context() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let controls = pod_preset::HdType11Control::for_model(0x020a_001e);
    let original = sample_hd_type11_preset(0x020a_001e);
    let index = app
        .hd_mut()
        .hd_library
        .import_bytes(original.clone())
        .unwrap();
    app.hd_mut().select_hd_file(index);
    app.select_test_editor::<HdEditor>();
    app.hd_mut().hd_effect_slot = 5;
    click_hd_type11_choice(&context, &mut app, controls[0], 1);
    click_hd_type11_choice(&context, &mut app, controls[1], 3);
    assert_eq!(app.hd().hd_type11_pending.len(), 2);
    click_hd_type12_choice(&context, &mut app, 1);
    assert_eq!(app.hd().hd_type12_pending.len(), 1);

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("opened.5xe");
    let replacement_bytes = sample_hd_type11_preset(0x020a_001d);
    std::fs::write(&path, &replacement_bytes).unwrap();
    app.dialogs = Box::new(FakeDialogs {
        pod_open: [None, Some(path)].into(),
        ..Default::default()
    });
    app.load_hd_editor_file();
    assert_eq!(
        app.hd().hd_type11_pending.len(),
        2,
        "cancelled Open preserves stages"
    );
    assert_eq!(app.hd().hd_type12_pending.len(), 1);
    assert_hd_bytes_eq(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        &original,
        "cancelled Open does not mutate the existing entry",
    );
    app.load_hd_editor_file();
    assert!(
        app.hd().hd_type11_pending.is_empty(),
        "successful Open invalidates prior entry stages"
    );
    assert!(
        app.hd().hd_type12_pending.is_empty(),
        "successful Open invalidates prior type-12 stage"
    );
    let opened = app.hd().hd_selected.unwrap();
    assert_ne!(opened, index);
    assert_hd_bytes_eq(
        app.hd().hd_library.file(opened).unwrap().as_bytes(),
        &replacement_bytes,
        "successful Open retains the newly parsed source",
    );
    assert_hd_bytes_eq(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        &original,
        "successful Open preserves the previous entry bytes",
    );

    app.hd_mut().select_hd_file(index);
    app.hd_mut().hd_effect_slot = 5;
    click_hd_type11_choice(&context, &mut app, controls[0], 4);
    assert_eq!(app.hd().hd_type11_pending.len(), 1);
    click_hd_type12_choice(&context, &mut app, 0);
    assert_eq!(app.hd().hd_type12_pending.len(), 1);
    let replacement = sample_hd_type11_preset(0x020a_001d);
    let parsed = pod_preset::OpaqueH5epPreset::parse(replacement).unwrap();
    *app.hd_mut().hd_library.file_mut(index).unwrap() = HdEditFile::H5ep(parsed);
    hd_editor_frame(&context, &mut app, vec![]);
    assert!(
        app.hd().hd_type11_pending.is_empty(),
        "same-entry model change invalidates old-model control stage"
    );
    assert!(
        app.hd().hd_type12_pending.is_empty(),
        "same-entry model change invalidates type-12 stage"
    );
}

#[test]
fn hd_type11_save_collision_reopen_and_revert_keep_stage_context_safe() {
    for model_id in [0x020a_001e, 0x020a_001d] {
        hd_type11_save_collision_reopen_and_revert_for_model(model_id);
    }
}

fn hd_type11_save_collision_reopen_and_revert_for_model(model_id: u32) {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let controls = pod_preset::HdType11Control::for_model(model_id);
    let type12_control = pod_preset::HdType12Control::for_model(model_id)
        .first()
        .copied();
    let original = sample_hd_type11_preset(model_id);
    let index = app
        .hd_mut()
        .hd_library
        .import_bytes(original.clone())
        .unwrap();
    app.hd_mut().select_hd_file(index);
    app.select_test_editor::<HdEditor>();
    app.hd_mut().hd_effect_slot = 5;
    let slot = pod_preset::HdFxSlot::new(5).unwrap();
    click_hd_type11_choice(&context, &mut app, controls[0], 1);
    click_hd_type11_choice(&context, &mut app, controls[1], 3);
    if type12_control.is_some() {
        click_hd_type12_choice(&context, &mut app, 1);
    }
    let second_stage = app.hd().hd_type11_pending[&(index, slot, controls[1])];
    let type12_stage =
        type12_control.map(|control| app.hd().hd_type12_pending[&(index, slot, control)]);
    click_hd_editor(&context, &mut app, hd_type11_apply_button(controls[0]));
    let value = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + 3 * 0x14 + 4;
    let mut applied_first = original.clone();
    applied_first[value..value + 4].copy_from_slice(&0x3e80_0000u32.to_be_bytes());
    assert_hd_bytes_eq(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        &applied_first,
        "first control applied before save",
    );
    assert_eq!(
        app.hd().hd_type11_pending.get(&(index, slot, controls[1])),
        Some(&second_stage)
    );
    if let (Some(control), Some(stage)) = (type12_control, type12_stage) {
        assert_eq!(
            app.hd().hd_type12_pending.get(&(index, slot, control)),
            Some(&stage)
        );
    } else {
        assert!(app.hd().hd_type12_pending.is_empty());
    }

    let directory = tempfile::tempdir().unwrap();
    let collision = directory.path().join("occupied.5xe");
    let saved = directory.path().join("type11-copy.5xe");
    std::fs::write(&collision, b"preserve existing").unwrap();
    app.dialogs = Box::new(FakeDialogs {
        pod_save: [Some(collision.clone()), Some(saved.clone())].into(),
        ..Default::default()
    });
    assert!(!app.write_hd_copy(index));
    assert_eq!(std::fs::read(&collision).unwrap(), b"preserve existing");
    assert!(app.hd().hd_library.is_dirty(index));
    assert_eq!(
        app.hd().hd_type11_pending.get(&(index, slot, controls[1])),
        Some(&second_stage)
    );
    if let (Some(control), Some(stage)) = (type12_control, type12_stage) {
        assert_eq!(
            app.hd().hd_type12_pending.get(&(index, slot, control)),
            Some(&stage)
        );
    } else {
        assert!(app.hd().hd_type12_pending.is_empty());
    }
    assert!(app.write_hd_copy(index));
    assert_eq!(std::fs::read(&saved).unwrap(), applied_first);
    assert!(!app.hd().hd_library.is_dirty(index));
    assert_eq!(
        app.hd().hd_type11_pending.get(&(index, slot, controls[1])),
        Some(&second_stage)
    );
    if let (Some(control), Some(stage)) = (type12_control, type12_stage) {
        assert_eq!(
            app.hd().hd_type12_pending.get(&(index, slot, control)),
            Some(&stage)
        );
    } else {
        assert!(app.hd().hd_type12_pending.is_empty());
    }

    let reopened_bytes = std::fs::read(&saved).unwrap();
    let reopened = app
        .hd_mut()
        .hd_library
        .import_bytes(reopened_bytes.clone())
        .unwrap();
    app.hd_mut().select_hd_file(reopened);
    assert_eq!(
        app.hd().hd_library.file(reopened).unwrap().as_bytes(),
        reopened_bytes
    );
    assert!(app.hd().hd_type11_pending.is_empty());
    assert!(app.hd().hd_type12_pending.is_empty());
    assert!(!app.hd().hd_library.is_dirty(reopened));

    app.hd_mut().select_hd_file(index);
    app.hd_mut().hd_effect_slot = 5;
    click_hd_type11_choice(&context, &mut app, controls[1], 2);
    if type12_control.is_some() {
        click_hd_type12_choice(&context, &mut app, 1);
    }
    assert_eq!(app.hd().hd_type11_pending.len(), 1);
    click_hd_editor(&context, &mut app, hd_type11_apply_button(controls[1]));
    assert!(app.hd().hd_library.is_dirty(index));
    assert!(app.hd_mut().discard_next_dirty_hd());
    assert_hd_bytes_eq(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        &applied_first,
        "revert restores the post-save baseline",
    );
    assert!(!app.hd().hd_library.is_dirty(index));
    assert!(app.hd().hd_type11_pending.is_empty());
    assert!(app.hd().hd_type12_pending.is_empty());

    app.hd_mut().select_hd_file(index);
    app.hd_mut().hd_effect_slot = 5;
    click_hd_type11_choice(&context, &mut app, controls[0], 4);
    assert_eq!(app.hd().hd_type11_pending.len(), 1);
    app.hd_mut().select_hd_file(reopened);
    assert!(
        app.hd().hd_type11_pending.is_empty(),
        "selection invalidates stale entry stage"
    );
    app.hd_mut().hd_effect_slot = 5;
    click_hd_type11_choice(&context, &mut app, controls[0], 2);
    assert_eq!(app.hd().hd_type11_pending.len(), 1);
    app.hd_mut().hd_effect_slot = 4;
    hd_editor_frame(&context, &mut app, vec![]);
    assert!(
        app.hd().hd_type11_pending.is_empty(),
        "slot context change clears stale stages"
    );
}

#[test]
fn hd_type12_gui_coexists_with_both_same_model_type11_stages() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let model_id = 0x020a_001e;
    let controls = pod_preset::HdType11Control::for_model(0x020a_001e);
    let control12 = pod_preset::HdType12Control::ALL[0];
    let source = sample_hd_type11_preset(model_id);
    let index = app
        .hd_mut()
        .hd_library
        .import_bytes(source.clone())
        .unwrap();
    app.hd_mut().select_hd_file(index);
    app.select_test_editor::<HdEditor>();
    app.hd_mut().hd_effect_slot = 5;
    let slot = pod_preset::HdFxSlot::new(5).unwrap();
    let expected_type11 = |control, choice| PendingHdType11Choice {
        file_index: index,
        slot,
        model_id,
        control,
        choice,
    };
    let expected_type12 = |choice| PendingHdType12Choice {
        file_index: index,
        slot,
        model_id,
        control: control12,
        choice,
    };

    // Every independent discard starts with all three exact GUI stages.
    click_hd_type12_choice(&context, &mut app, 1);
    click_hd_type11_choice(&context, &mut app, controls[0], 1);
    click_hd_type11_choice(&context, &mut app, controls[1], 4);
    let type12_one = expected_type12(pod_preset::HdType12Choice::Choice1);
    let type11_first = expected_type11(controls[0], pod_preset::HdType11Choice::Choice1);
    let type11_second = expected_type11(controls[1], pod_preset::HdType11Choice::Choice4);
    assert_eq!(app.hd().hd_type12_pending.len(), 1);
    assert_eq!(app.hd().hd_type11_pending.len(), 2);
    assert_eq!(
        app.hd().hd_type12_pending.get(&(index, slot, control12)),
        Some(&type12_one)
    );
    assert_eq!(
        app.hd().hd_type11_pending.get(&(index, slot, controls[0])),
        Some(&type11_first)
    );
    assert_eq!(
        app.hd().hd_type11_pending.get(&(index, slot, controls[1])),
        Some(&type11_second)
    );
    assert_hd_bytes_eq(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        &source,
        "staging all controls is byte neutral",
    );

    click_hd_editor(&context, &mut app, hd_type12_discard_button());
    assert!(app.hd().hd_type12_pending.is_empty());
    assert_eq!(
        app.hd().hd_type11_pending.get(&(index, slot, controls[0])),
        Some(&type11_first)
    );
    assert_eq!(
        app.hd().hd_type11_pending.get(&(index, slot, controls[1])),
        Some(&type11_second)
    );
    assert_hd_bytes_eq(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        &source,
        "discarding type-12 leaves all preset bytes unchanged",
    );

    click_hd_type12_choice(&context, &mut app, 0);
    assert_eq!(
        app.hd().hd_type12_pending.get(&(index, slot, control12)),
        Some(&expected_type12(pod_preset::HdType12Choice::Choice0))
    );
    assert_eq!(
        app.hd().hd_type11_pending.get(&(index, slot, controls[0])),
        Some(&type11_first)
    );
    assert_eq!(
        app.hd().hd_type11_pending.get(&(index, slot, controls[1])),
        Some(&type11_second)
    );
    click_hd_editor(&context, &mut app, hd_type11_discard_button(controls[0]));
    assert!(
        !app.hd()
            .hd_type11_pending
            .contains_key(&(index, slot, controls[0]))
    );
    assert_eq!(
        app.hd().hd_type11_pending.get(&(index, slot, controls[1])),
        Some(&type11_second)
    );
    assert_eq!(
        app.hd().hd_type12_pending.get(&(index, slot, control12)),
        Some(&expected_type12(pod_preset::HdType12Choice::Choice0))
    );
    assert_hd_bytes_eq(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        &source,
        "discarding first type-11 leaves all preset bytes unchanged",
    );

    click_hd_type11_choice(&context, &mut app, controls[0], 1);
    assert_eq!(
        app.hd().hd_type12_pending.get(&(index, slot, control12)),
        Some(&expected_type12(pod_preset::HdType12Choice::Choice0))
    );
    assert_eq!(
        app.hd().hd_type11_pending.get(&(index, slot, controls[0])),
        Some(&type11_first)
    );
    assert_eq!(
        app.hd().hd_type11_pending.get(&(index, slot, controls[1])),
        Some(&type11_second)
    );
    click_hd_editor(&context, &mut app, hd_type11_discard_button(controls[1]));
    assert!(
        !app.hd()
            .hd_type11_pending
            .contains_key(&(index, slot, controls[1]))
    );
    assert_eq!(
        app.hd().hd_type11_pending.get(&(index, slot, controls[0])),
        Some(&type11_first)
    );
    assert_eq!(
        app.hd().hd_type12_pending.get(&(index, slot, control12)),
        Some(&expected_type12(pod_preset::HdType12Choice::Choice0))
    );
    assert_hd_bytes_eq(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        &source,
        "discarding second type-11 leaves all preset bytes unchanged",
    );

    // Each Apply starts with all three exact records pending. Expected full
    // buffers prove the two other rows and all unrelated bytes survive.
    let (context, mut app) = self::app();
    context.style_mut(|style| style.animation_time = 0.0);
    let index = app
        .hd_mut()
        .hd_library
        .import_bytes(source.clone())
        .unwrap();
    app.hd_mut().select_hd_file(index);
    app.select_test_editor::<HdEditor>();
    app.hd_mut().hd_effect_slot = 5;
    let slot = pod_preset::HdFxSlot::new(5).unwrap();
    let row_value_offset =
        |row: usize| pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + row * 0x14 + 4;
    let expected_type11 = |control, choice| PendingHdType11Choice {
        file_index: index,
        slot,
        model_id,
        control,
        choice,
    };
    let expected_type12 = |choice| PendingHdType12Choice {
        file_index: index,
        slot,
        model_id,
        control: control12,
        choice,
    };
    let type11_first = expected_type11(controls[0], pod_preset::HdType11Choice::Choice1);
    let type11_second = expected_type11(controls[1], pod_preset::HdType11Choice::Choice4);
    let type12_one = expected_type12(pod_preset::HdType12Choice::Choice1);

    click_hd_type11_choice(&context, &mut app, controls[0], 1);
    click_hd_type11_choice(&context, &mut app, controls[1], 4);
    click_hd_type12_choice(&context, &mut app, 1);
    assert_eq!(
        app.hd().hd_type12_pending.get(&(index, slot, control12)),
        Some(&type12_one)
    );
    assert_eq!(
        app.hd().hd_type11_pending.get(&(index, slot, controls[0])),
        Some(&type11_first)
    );
    assert_eq!(
        app.hd().hd_type11_pending.get(&(index, slot, controls[1])),
        Some(&type11_second)
    );
    let mut expected_bytes = source.clone();
    let row12 = row_value_offset(4);
    let row11a = row_value_offset(3);
    let row11b = row_value_offset(8);
    expected_bytes[row12..row12 + 4].copy_from_slice(&0x3f80_0000u32.to_be_bytes());
    click_hd_editor(&context, &mut app, hd_type12_apply_button());
    assert!(app.hd().hd_type12_pending.is_empty());
    assert_eq!(
        app.hd().hd_type11_pending.get(&(index, slot, controls[0])),
        Some(&type11_first)
    );
    assert_eq!(
        app.hd().hd_type11_pending.get(&(index, slot, controls[1])),
        Some(&type11_second)
    );
    assert_hd_bytes_eq(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        &expected_bytes,
        "type-12 Apply changes only its own value word",
    );

    click_hd_type12_choice(&context, &mut app, 0);
    let type12_zero = expected_type12(pod_preset::HdType12Choice::Choice0);
    assert_eq!(
        app.hd().hd_type12_pending.get(&(index, slot, control12)),
        Some(&type12_zero)
    );
    assert_eq!(
        app.hd().hd_type11_pending.get(&(index, slot, controls[0])),
        Some(&type11_first)
    );
    assert_eq!(
        app.hd().hd_type11_pending.get(&(index, slot, controls[1])),
        Some(&type11_second)
    );
    expected_bytes[row11a..row11a + 4].copy_from_slice(&0x3e80_0000u32.to_be_bytes());
    click_hd_editor(&context, &mut app, hd_type11_apply_button(controls[0]));
    assert_eq!(
        app.hd().hd_type12_pending.get(&(index, slot, control12)),
        Some(&type12_zero),
        "type-11 Apply preserves the exact type-12 record"
    );
    assert!(
        !app.hd()
            .hd_type11_pending
            .contains_key(&(index, slot, controls[0]))
    );
    assert_eq!(
        app.hd().hd_type11_pending.get(&(index, slot, controls[1])),
        Some(&type11_second)
    );
    assert_hd_bytes_eq(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        &expected_bytes,
        "first type-11 Apply changes only its own value word",
    );

    // Restage the already-applied first control so this Apply also begins
    // with all three pending records and verifies preservation exactly.
    click_hd_type11_choice(&context, &mut app, controls[0], 2);
    let type11_first_again = expected_type11(controls[0], pod_preset::HdType11Choice::Choice2);
    assert_eq!(
        app.hd().hd_type12_pending.get(&(index, slot, control12)),
        Some(&type12_zero)
    );
    assert_eq!(
        app.hd().hd_type11_pending.get(&(index, slot, controls[0])),
        Some(&type11_first_again)
    );
    assert_eq!(
        app.hd().hd_type11_pending.get(&(index, slot, controls[1])),
        Some(&type11_second)
    );
    expected_bytes[row12..row12 + 4].copy_from_slice(&0u32.to_be_bytes());
    click_hd_editor(&context, &mut app, hd_type12_apply_button());
    assert!(app.hd().hd_type12_pending.is_empty());
    assert_eq!(
        app.hd().hd_type11_pending.get(&(index, slot, controls[0])),
        Some(&type11_first_again)
    );
    assert_eq!(
        app.hd().hd_type11_pending.get(&(index, slot, controls[1])),
        Some(&type11_second)
    );
    assert_hd_bytes_eq(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        &expected_bytes,
        "type-12 Choice 0 Apply changes only its own value word",
    );

    let type12_one_again = expected_type12(pod_preset::HdType12Choice::Choice1);
    click_hd_type12_choice(&context, &mut app, 1);
    assert_eq!(
        app.hd().hd_type12_pending.get(&(index, slot, control12)),
        Some(&type12_one_again)
    );
    assert_eq!(
        app.hd().hd_type11_pending.get(&(index, slot, controls[0])),
        Some(&type11_first_again)
    );
    assert_eq!(
        app.hd().hd_type11_pending.get(&(index, slot, controls[1])),
        Some(&type11_second)
    );
    expected_bytes[row11b..row11b + 4].copy_from_slice(&0x3f80_0000u32.to_be_bytes());
    click_hd_editor(&context, &mut app, hd_type11_apply_button(controls[1]));
    assert_eq!(
        app.hd().hd_type12_pending.get(&(index, slot, control12)),
        Some(&type12_one_again),
        "second type-11 Apply preserves the exact type-12 record"
    );
    assert_eq!(
        app.hd().hd_type11_pending.get(&(index, slot, controls[0])),
        Some(&type11_first_again)
    );
    assert!(
        !app.hd()
            .hd_type11_pending
            .contains_key(&(index, slot, controls[1]))
    );
    assert_hd_bytes_eq(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        &expected_bytes,
        "second type-11 Apply changes only its own value word",
    );

    // Rebuild all three records before the final type-12 discard.
    click_hd_type11_choice(&context, &mut app, controls[1], 3);
    let type11_second_again = expected_type11(controls[1], pod_preset::HdType11Choice::Choice3);
    assert_eq!(
        app.hd().hd_type12_pending.get(&(index, slot, control12)),
        Some(&type12_one_again)
    );
    assert_eq!(
        app.hd().hd_type11_pending.get(&(index, slot, controls[0])),
        Some(&type11_first_again)
    );
    assert_eq!(
        app.hd().hd_type11_pending.get(&(index, slot, controls[1])),
        Some(&type11_second_again)
    );
    click_hd_editor(&context, &mut app, hd_type12_discard_button());
    assert!(app.hd().hd_type12_pending.is_empty());
    assert_eq!(
        app.hd().hd_type11_pending.get(&(index, slot, controls[0])),
        Some(&type11_first_again)
    );
    assert_eq!(
        app.hd().hd_type11_pending.get(&(index, slot, controls[1])),
        Some(&type11_second_again)
    );
    assert_hd_bytes_eq(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        &expected_bytes,
        "discarding type-12 leaves applied type-11 output unchanged",
    );
    assert!(app.hd().hd_library.is_dirty(index));
    assert_eq!(
        control12,
        pod_preset::HdType12Control::Model020a001eKey3f100004
    );
}

#[test]
fn hd_type12_failed_apply_retains_dirty_baseline_and_sibling_stages() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let controls = pod_preset::HdType11Control::for_model(0x020a_001e);
    let source = sample_hd_type11_preset(0x020a_001e);
    let index = app
        .hd_mut()
        .hd_library
        .import_bytes(source.clone())
        .unwrap();
    app.hd_mut().select_hd_file(index);
    app.select_test_editor::<HdEditor>();
    app.hd_mut().hd_effect_slot = 5;
    let slot = pod_preset::HdFxSlot::new(5).unwrap();
    click_hd_type12_choice(&context, &mut app, 1);
    click_hd_type11_choice(&context, &mut app, controls[0], 1);
    click_hd_type11_choice(&context, &mut app, controls[1], 4);
    assert_eq!(app.hd().hd_type12_pending.len(), 1);
    assert_eq!(app.hd().hd_type11_pending.len(), 2);

    // An independent sibling-row edit creates real dirty state while the
    // target type-12 and other type-11 stages remain pending.
    click_hd_editor(&context, &mut app, hd_type11_apply_button(controls[0]));
    assert!(app.hd().hd_library.is_dirty(index));
    assert_eq!(app.hd().hd_type12_pending.len(), 1);
    assert_eq!(app.hd().hd_type11_pending.len(), 1);
    let type12 = app.hd().hd_type12_pending[&(index, slot, pod_preset::HdType12Control::ALL[0])];
    let sibling = app.hd().hd_type11_pending[&(index, slot, controls[1])];

    let mut invalid = app.hd().hd_library.file(index).unwrap().as_bytes().to_vec();
    let marker = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + 4 * 0x14 + 0x10;
    invalid[marker] = 1;
    *app.hd_mut().hd_library.file_mut(index).unwrap() =
        HdEditFile::H5ep(pod_preset::OpaqueH5epPreset::parse(invalid).unwrap());
    let invalid_before = app.hd().hd_library.file(index).unwrap().as_bytes().to_vec();
    assert_eq!(
        app.hd()
            .hd_type12_pending
            .get(&(index, slot, pod_preset::HdType12Control::ALL[0])),
        Some(&type12),
        "the exact staged type-12 record remains before invalidation and rejected Apply"
    );
    click_hd_editor(&context, &mut app, hd_type12_apply_button());
    assert_eq!(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        invalid_before
    );
    assert!(app.hd().hd_library.is_dirty(index));
    assert_eq!(
        app.hd()
            .hd_type12_pending
            .get(&(index, slot, pod_preset::HdType12Control::ALL[0])),
        Some(&type12),
        "rejected Apply retains the exact type-12 record"
    );
    assert_eq!(
        app.hd().hd_type11_pending.get(&(index, slot, controls[1])),
        Some(&sibling)
    );

    assert!(app.hd_mut().discard_next_dirty_hd());
    assert_eq!(app.hd().hd_library.file(index).unwrap().as_bytes(), source);
    assert!(!app.hd().hd_library.is_dirty(index));
    assert!(app.hd().hd_type12_pending.is_empty());
    assert!(app.hd().hd_type11_pending.is_empty());
}

#[test]
fn hd_type12_quit_resolves_all_three_stages_before_dirty_file_decision() {
    let controls = pod_preset::HdType11Control::for_model(0x020a_001e);
    let source = sample_hd_type11_preset(0x020a_001e);

    let (context, mut app1) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let index = app1
        .hd_mut()
        .hd_library
        .import_bytes(source.clone())
        .unwrap();
    app1.hd_mut().select_hd_file(index);
    app1.select_test_editor::<HdEditor>();
    app1.hd_mut().hd_effect_slot = 5;
    click_hd_type12_choice(&context, &mut app1, 1);
    click_hd_type11_choice(&context, &mut app1, controls[0], 1);
    click_hd_type11_choice(&context, &mut app1, controls[1], 4);
    assert_eq!(app1.hd().hd_type12_pending.len(), 1);
    assert_eq!(app1.hd().hd_type11_pending.len(), 2);
    app1.request_close(&context);
    click_prompt(&context, &mut app1, "discard");
    click_prompt(&context, &mut app1, "discard");
    click_prompt(&context, &mut app1, "discard");
    assert!(app1.permit_close);
    assert!(app1.hd().hd_type12_pending.is_empty());
    assert!(app1.hd().hd_type11_pending.is_empty());
    assert_eq!(app1.hd().hd_library.file(index).unwrap().as_bytes(), source);

    let (context2, mut app2) = app();
    context2.style_mut(|style| style.animation_time = 0.0);
    let index2 = app2
        .hd_mut()
        .hd_library
        .import_bytes(source.clone())
        .unwrap();
    app2.hd_mut().select_hd_file(index2);
    app2.select_test_editor::<HdEditor>();
    app2.hd_mut().hd_effect_slot = 5;
    click_hd_type12_choice(&context2, &mut app2, 0);
    click_hd_type11_choice(&context2, &mut app2, controls[0], 1);
    click_hd_type11_choice(&context2, &mut app2, controls[1], 3);
    app2.request_close(&context2);
    click_prompt(&context2, &mut app2, "apply-type11");
    assert_eq!(app2.hd().hd_type11_pending.len(), 1);
    click_prompt(&context2, &mut app2, "apply-type11");
    assert!(app2.hd().hd_type11_pending.is_empty());
    assert_eq!(app2.hd().hd_type12_pending.len(), 1);
    assert!(!app2.permit_close);
    click_prompt(&context2, &mut app2, "apply-type12");
    assert!(app2.hd().hd_type12_pending.is_empty());
    assert!(app2.hd().hd_library.is_dirty(index2));
    assert!(!app2.permit_close, "dirty-file decision still blocks close");
    click_prompt(&context2, &mut app2, "cancel");
    assert!(!app2.permit_close, "Keep editing must not close");
    app2.request_close(&context2);
    click_prompt(&context2, &mut app2, "discard");
    assert!(app2.permit_close);
    assert_eq!(
        app2.hd().hd_library.file(index2).unwrap().as_bytes(),
        source
    );

    // Keep editing must dismiss the staged-control quit prompt before any
    // of the three records can be applied or discarded.
    let (context3, mut app3) = app();
    context3.style_mut(|style| style.animation_time = 0.0);
    let index3 = app3
        .hd_mut()
        .hd_library
        .import_bytes(source.clone())
        .unwrap();
    app3.hd_mut().select_hd_file(index3);
    app3.select_test_editor::<HdEditor>();
    app3.hd_mut().hd_effect_slot = 5;
    let slot3 = pod_preset::HdFxSlot::new(5).unwrap();
    let control12 = pod_preset::HdType12Control::ALL[0];
    click_hd_type12_choice(&context3, &mut app3, 1);
    click_hd_type11_choice(&context3, &mut app3, controls[0], 1);
    click_hd_type11_choice(&context3, &mut app3, controls[1], 4);
    let expected_type12 = PendingHdType12Choice {
        file_index: index3,
        slot: slot3,
        model_id: 0x020a_001e,
        control: control12,
        choice: pod_preset::HdType12Choice::Choice1,
    };
    let expected_type11_first = PendingHdType11Choice {
        file_index: index3,
        slot: slot3,
        model_id: 0x020a_001e,
        control: controls[0],
        choice: pod_preset::HdType11Choice::Choice1,
    };
    let expected_type11_second = PendingHdType11Choice {
        file_index: index3,
        slot: slot3,
        model_id: 0x020a_001e,
        control: controls[1],
        choice: pod_preset::HdType11Choice::Choice4,
    };
    assert_eq!(app3.hd().hd_type12_pending.len(), 1);
    assert_eq!(app3.hd().hd_type11_pending.len(), 2);
    assert_eq!(
        app3.hd().hd_type12_pending.get(&(index3, slot3, control12)),
        Some(&expected_type12)
    );
    assert_eq!(
        app3.hd()
            .hd_type11_pending
            .get(&(index3, slot3, controls[0])),
        Some(&expected_type11_first)
    );
    assert_eq!(
        app3.hd()
            .hd_type11_pending
            .get(&(index3, slot3, controls[1])),
        Some(&expected_type11_second)
    );
    app3.request_close(&context3);
    assert!(matches!(app3.test_pending(), Some(Pending::QuitHd)));
    click_prompt(&context3, &mut app3, "cancel");
    assert!(
        app3.test_pending().is_none(),
        "Keep editing dismisses the staged-control prompt"
    );
    assert!(!app3.permit_close, "Keep editing never closes the app");
    assert_eq!(
        app3.hd().hd_type12_pending.get(&(index3, slot3, control12)),
        Some(&expected_type12)
    );
    assert_eq!(
        app3.hd()
            .hd_type11_pending
            .get(&(index3, slot3, controls[0])),
        Some(&expected_type11_first)
    );
    assert_eq!(
        app3.hd()
            .hd_type11_pending
            .get(&(index3, slot3, controls[1])),
        Some(&expected_type11_second)
    );
    assert_hd_bytes_eq(
        app3.hd().hd_library.file(index3).unwrap().as_bytes(),
        &source,
        "Keep editing with three staged controls leaves all bytes unchanged",
    );
}

#[test]
fn hd_type11_quit_processes_every_staged_control_before_dirty_file_prompt() {
    for model_id in [0x020a_001e, 0x020a_001d] {
        hd_type11_quit_processes_every_staged_control_for(model_id);
    }
}

fn hd_type11_quit_processes_every_staged_control_for(model_id: u32) {
    let (context3, mut app3) = app();
    context3.style_mut(|style| style.animation_time = 0.0);
    let (context2, mut app2) = app();
    context2.style_mut(|style| style.animation_time = 0.0);
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let original = sample_hd_type11_preset(model_id);
    let controls = pod_preset::HdType11Control::for_model(model_id);
    let index = app
        .hd_mut()
        .hd_library
        .import_bytes(original.clone())
        .unwrap();
    app.hd_mut().select_hd_file(index);
    app.select_test_editor::<HdEditor>();
    app.hd_mut().hd_effect_slot = 5;
    let slot = pod_preset::HdFxSlot::new(5).unwrap();
    click_hd_type11_choice(&context, &mut app, controls[0], 1);
    click_hd_type11_choice(&context, &mut app, controls[1], 4);
    assert_eq!(app.hd().hd_type11_pending.len(), 2);
    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitHd)));
    click_prompt(&context, &mut app, "discard");
    assert_eq!(app.hd().hd_type11_pending.len(), 1);
    assert!(!app.permit_close);
    click_prompt(&context, &mut app, "discard");
    assert!(app.hd().hd_type11_pending.is_empty());
    assert_hd_bytes_eq(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        &original,
        "quit discard processes both pending controls",
    );
    assert!(app.permit_close);

    let index2 = app2
        .hd_mut()
        .hd_library
        .import_bytes(original.clone())
        .unwrap();
    app2.hd_mut().select_hd_file(index2);
    app2.select_test_editor::<HdEditor>();
    app2.hd_mut().hd_effect_slot = 5;
    click_hd_type11_choice(&context2, &mut app2, controls[0], 1);
    click_hd_type11_choice(&context2, &mut app2, controls[1], 3);
    assert_eq!(app2.hd().hd_type11_pending.len(), 2);
    app2.request_close(&context2);
    click_prompt(&context2, &mut app2, "apply-type11");
    assert_eq!(app2.hd().hd_type11_pending.len(), 1);
    assert!(!app2.permit_close);
    click_prompt(&context2, &mut app2, "apply-type11");
    assert!(app2.hd().hd_type11_pending.is_empty());
    assert!(app2.hd().hd_library.is_dirty(index2));
    assert!(!app2.permit_close);
    click_prompt(&context2, &mut app2, "discard");
    assert!(!app2.hd().hd_library.is_dirty(index2));
    assert_hd_bytes_eq(
        app2.hd().hd_library.file(index2).unwrap().as_bytes(),
        &original,
        "quit apply processes both pending controls before file revert",
    );
    assert!(app2.permit_close);
    assert_eq!(slot.index(), 5);

    let index3 = app3
        .hd_mut()
        .hd_library
        .import_bytes(original.clone())
        .unwrap();
    app3.hd_mut().select_hd_file(index3);
    app3.select_test_editor::<HdEditor>();
    app3.hd_mut().hd_effect_slot = 5;
    click_hd_type11_choice(&context3, &mut app3, controls[0], 2);
    click_hd_type11_choice(&context3, &mut app3, controls[1], 4);
    let first = app3.hd().hd_type11_pending[&(index3, slot, controls[0])];
    let second = app3.hd().hd_type11_pending[&(index3, slot, controls[1])];
    assert_eq!(app3.hd().hd_type11_pending.len(), 2);
    app3.request_close(&context3);
    assert!(matches!(app3.test_pending(), Some(Pending::QuitHd)));
    click_prompt(&context3, &mut app3, "cancel");
    assert!(app3.test_pending().is_none());
    assert!(!app3.permit_close);
    assert_eq!(
        app3.hd()
            .hd_type11_pending
            .get(&(index3, slot, controls[0])),
        Some(&first),
        "Keep editing preserves the exact first stage"
    );
    assert_eq!(
        app3.hd()
            .hd_type11_pending
            .get(&(index3, slot, controls[1])),
        Some(&second),
        "Keep editing preserves the exact second stage"
    );
    assert_hd_bytes_eq(
        app3.hd().hd_library.file(index3).unwrap().as_bytes(),
        &original,
        "Keep editing leaves source bytes unchanged",
    );
}

#[test]
fn hd_editor_library_workflow_edits_and_saves_a_new_copy_offline() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let source = app.variax().document.as_ref().unwrap().as_bytes().to_vec();
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("source.5xe");
    let output = directory.path().join("copy.5xe");
    let blocked_output = directory.path().join("blocked-output");
    std::fs::create_dir(&blocked_output).unwrap();
    let sentinel = blocked_output.join("keep.bin");
    std::fs::write(&sentinel, b"keep").unwrap();
    let original = sample_hd_editor_preset();
    std::fs::write(&input, &original).unwrap();
    app.dialogs = Box::new(FakeDialogs {
        pod_open: [Some(input.clone())].into(),
        pod_save: [Some(blocked_output.clone()), Some(output.clone())].into(),
        ..Default::default()
    });

    app.load_hd_editor_file();
    assert_eq!(app.hd().hd_library.len(), 1);
    assert_eq!(app.podxt().pod_library.len(), 0);
    assert_eq!(app.hd().hd_selected, Some(0));
    assert_eq!(app.hd().hd_control_percent, 50);
    app.hd_mut().hd_control_percent = 75;
    app.hd_mut().apply_hd_control(0);
    app.hd_mut().hd_control =
        pod_preset::HdType1Control::TubeDrive(pod_preset::TubeDriveControl::Mid);
    assert_eq!(
        app.hd().hd_library.file(0).and_then(|file| match file {
            HdEditFile::H5ep(preset) =>
                preset.tube_drive_control_percent(0, pod_preset::TubeDriveControl::Mid),
            _ => None,
        }),
        Some(35)
    );
    app.hd_mut().hd_control_percent = 40;
    app.hd_mut().apply_hd_control(0);
    assert!(app.hd().hd_library.is_dirty(0));
    assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), source);
    assert_eq!(std::fs::read(&input).unwrap(), original);

    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitHd)));
    prompt_frame(&context, &mut app, vec![]);
    click_prompt(&context, &mut app, "save");
    assert!(!app.permit_close);
    assert!(app.hd().hd_library.is_dirty(0));
    assert_eq!(std::fs::read(&sentinel).unwrap(), b"keep");
    click_prompt(&context, &mut app, "save");
    assert!(app.permit_close);
    assert!(!app.hd().hd_library.any_dirty());
    let written = std::fs::read(&output).unwrap();
    assert_eq!(written.len(), original.len());
    assert_eq!(
        &written[0x28 + 0x428 + 0x10 + 4..0x28 + 0x428 + 0x10 + 8],
        &0.75f32.to_bits().to_be_bytes()
    );
    assert_eq!(
        &written[0x28 + 0x428 + 0x10 + 2 * 0x14 + 4..0x28 + 0x428 + 0x10 + 2 * 0x14 + 8],
        &0.4f32.to_bits().to_be_bytes()
    );
    assert!(app.variax().read_worker.is_none());
    assert!(!app.variax().is_reading());
    assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
}

#[derive(Debug, Default)]
struct HdExplicitReplacementTargets {
    distortion: Option<pod_preset::HdDistortionDefault>,
    dynamics: Option<pod_preset::HdDynamicsDefault>,
    reverb: Option<pod_preset::HdReverbDefault>,
}

fn exercise_hd_editor_explicit_replacement_lifecycle(
    target_model: u32,
    default_keys: &[u32],
    button_id: &'static str,
    message_label: &str,
    output_name: &str,
    targets: HdExplicitReplacementTargets,
) {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let source_document = app.variax().document.as_ref().unwrap().as_bytes().to_vec();
    let mut original = sample_hd_editor_preset();
    let block = pod_preset::H5EP_HEADER_BYTES + 0x428;
    original[block..block + 4].copy_from_slice(&0x0205_000cu32.to_be_bytes());
    original[block + 4..block + 8].copy_from_slice(&0x1234_5678u32.to_be_bytes());
    original[block + 8..block + 0x0c].copy_from_slice(&[1, 0x5a, 0xa5, 1]);
    original[block + 0x0f] = 8;
    let keys = [
        0x3f10_0005u32,
        0x3f10_0001,
        0x3f10_0003,
        0x3f10_0002,
        0x3f10_0004,
    ];
    for (row, key) in keys.iter().copied().enumerate().chain((5..12).map(|row| {
        let key = if row == 6 {
            0x3f10_0005
        } else {
            0x6abc_0000 + row as u32
        };
        (row, key)
    })) {
        let row_at = block + 0x10 + row * 0x14;
        original[row_at..row_at + 4].copy_from_slice(&key.to_be_bytes());
        original[row_at + 4..row_at + 8]
            .copy_from_slice(&(0x3e80_0000u32 + row as u32).to_be_bytes());
        original[row_at + 8..row_at + 12]
            .copy_from_slice(&(0x7100_0000u32 + row as u32).to_be_bytes());
        original[row_at + 0x10] = (row % 3) as u8;
        original[row_at + 0x11..row_at + 0x14].copy_from_slice(&[
            0xc1 + row as u8,
            0xd1 + row as u8,
            0xe1 + row as u8,
        ]);
    }
    let index = app
        .hd_mut()
        .hd_library
        .import_bytes(original.clone())
        .unwrap();
    app.hd_mut().select_hd_file(index);
    if let Some(target) = targets.distortion {
        app.hd_mut().hd_default_target = target;
    }
    if let Some(target) = targets.dynamics {
        app.hd_mut().hd_dynamics_default_target = target;
    }
    if let Some(target) = targets.reverb {
        app.hd_mut().hd_reverb_default_target = target;
    }
    app.select_test_editor::<HdEditor>();
    recording_frame(&context, &mut app, vec![]);
    assert_eq!(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        original
    );

    let mut expected = original.clone();
    expected[block..block + 4].copy_from_slice(&target_model.to_be_bytes());
    for (row, key) in default_keys.iter().copied().enumerate() {
        let row_at = block + 0x10 + row * 0x14;
        expected[row_at..row_at + 4].copy_from_slice(&key.to_be_bytes());
        expected[row_at + 4..row_at + 8].copy_from_slice(&0u32.to_be_bytes());
    }
    expected[block + 0x0f] = default_keys.len() as u8;
    for row in 0..12 {
        expected[block + 0x10 + row * 0x14 + 0x10] = 0;
    }

    click_hd_editor(&context, &mut app, button_id);
    assert!(
        app.hd().hd_message.contains(message_label),
        "button={button_id:?}, targets={targets:?}, message={:?}, button_rect={:?}, scroll={:?}",
        app.hd().hd_message,
        app.hd().hd_buttons.get(button_id),
        app.hd().hd_scroll_area
    );
    assert_eq!(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        expected
    );
    assert!(
        app.hd().hd_library.is_dirty(index),
        "targets={targets:?}, source_matches_expected={}, button_rect={:?}",
        original == expected,
        app.hd().hd_buttons.get(button_id)
    );
    assert_eq!(
        app.variax().document.as_ref().unwrap().as_bytes(),
        source_document
    );
    app.hd_mut().hd_library.revert_unsaved(index).unwrap();
    assert_eq!(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        original
    );
    assert!(!app.hd().hd_library.is_dirty(index));

    click_hd_editor(&context, &mut app, button_id);
    assert!(
        app.hd().hd_library.is_dirty(index),
        "second apply left the file clean: message={:?}, button_rect={:?}, scroll={:?}",
        app.hd().hd_message,
        app.hd().hd_buttons.get(button_id),
        app.hd().hd_scroll_area
    );
    let directory = tempfile::tempdir().unwrap();
    let blocked = directory.path().join("blocked-directory");
    std::fs::create_dir(&blocked).unwrap();
    let collision = directory.path().join("existing.5xe");
    let output = directory.path().join(output_name);
    std::fs::write(&collision, b"keep destination").unwrap();
    app.dialogs = Box::new(FakeDialogs {
        pod_save: [
            None,
            Some(blocked.clone()),
            Some(collision.clone()),
            Some(output.clone()),
        ]
        .into(),
        ..Default::default()
    });
    assert!(!app.write_hd_copy(index)); // Save dialog cancellation.
    assert!(app.hd().hd_library.is_dirty(index));
    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitHd)));
    click_prompt(&context, &mut app, "cancel");
    assert!(app.test_pending().is_none());
    assert!(app.hd().hd_library.is_dirty(index));

    app.request_close(&context);
    click_prompt(&context, &mut app, "save"); // A directory cannot be the destination.
    assert!(app.test_pending().is_some());
    assert!(app.hd().hd_library.is_dirty(index));
    click_prompt(&context, &mut app, "save"); // Existing destination is preserved.
    assert!(app.test_pending().is_some());
    assert!(app.hd().hd_library.is_dirty(index));
    assert_eq!(std::fs::read(&collision).unwrap(), b"keep destination");
    click_prompt(&context, &mut app, "save");
    assert!(app.permit_close);
    assert!(!app.hd().hd_library.any_dirty());
    let written = std::fs::read(&output).unwrap();
    assert_eq!(written, expected);
    let reopened = pod_preset::OpaqueH5epPreset::parse(written).unwrap();
    assert_eq!(reopened.effect_model_id(0), Some(target_model));
    assert!(reopened.trailing_bytes().is_empty());
    assert!(app.variax().read_worker.is_none());
    assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
}

#[test]
fn hd_effect_enable_ui_edits_one_flag_and_guards_copy_save_and_quit() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let mut original = sample_hd_editor_preset();
    let block = pod_preset::H5EP_HEADER_BYTES + 0x428;
    original[block + 0x0b] = 1;
    original[block + 8] = 0;
    let index = app
        .hd_mut()
        .hd_library
        .import_bytes(original.clone())
        .unwrap();
    app.hd_mut().select_hd_file(index);
    app.select_test_editor::<HdEditor>();
    hd_editor_frame(&context, &mut app, vec![]);
    assert!(app.hd().hd_buttons.contains_key("enable-hd-effect"));

    click_hd_editor(&context, &mut app, "enable-hd-effect");
    let mut expected = original.clone();
    expected[block + 8] = 1;
    let actual = app.hd().hd_library.file(index).unwrap().as_bytes();
    let differences = actual
        .iter()
        .zip(&expected)
        .enumerate()
        .filter_map(|(offset, (actual, expected))| (actual != expected).then_some(offset))
        .collect::<Vec<_>>();
    assert!(
        differences.is_empty(),
        "unexpected byte offsets: {differences:?}; actual={}, expected={}, button={:?}; message={}",
        actual[block + 8],
        expected[block + 8],
        app.hd().hd_buttons.get("enable-hd-effect"),
        app.hd().hd_message
    );
    assert!(app.hd().hd_library.is_dirty(index));
    assert!(app.hd().hd_message.contains("Nothing was sent"));

    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitHd)));
    click_prompt(&context, &mut app, "cancel");
    assert!(app.test_pending().is_none());
    assert!(app.hd().hd_library.is_dirty(index));

    let directory = tempfile::tempdir().unwrap();
    let blocked = directory.path().join("blocked-directory");
    std::fs::create_dir(&blocked).unwrap();
    let collision = directory.path().join("existing.5xe");
    let output = directory.path().join("enabled-copy.5xe");
    std::fs::write(&collision, b"keep destination").unwrap();
    app.dialogs = Box::new(FakeDialogs {
        pod_save: [
            Some(blocked.clone()),
            Some(collision.clone()),
            Some(output.clone()),
        ]
        .into(),
        ..Default::default()
    });

    app.request_close(&context);
    click_prompt(&context, &mut app, "save");
    assert!(app.test_pending().is_some());
    assert!(app.hd().hd_library.is_dirty(index));
    click_prompt(&context, &mut app, "save");
    assert!(app.test_pending().is_some());
    assert!(app.hd().hd_library.is_dirty(index));
    assert_eq!(std::fs::read(&collision).unwrap(), b"keep destination");
    click_prompt(&context, &mut app, "save");
    assert!(app.permit_close);
    assert!(!app.hd().hd_library.any_dirty());
    let saved = std::fs::read(&output).unwrap();
    assert_eq!(saved, expected);
    let reopened = pod_preset::OpaqueH5epPreset::parse(saved).unwrap();
    assert_eq!(
        reopened.effect_enabled(pod_preset::HdFxSlot::new(0).unwrap()),
        Some(true)
    );
    assert!(app.variax().read_worker.is_none());
    assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
}

#[test]
fn hd_editor_explicit_default_replacements_stages_reverts_saves_and_guards_quit() {
    exercise_hd_editor_explicit_replacement_lifecycle(
        0x020c_000e,
        &[
            0x3f10_0002,
            0x3f10_0003,
            0x3f10_0004,
            0x3f10_0005,
            0x3f10_0001,
        ],
        "replace-octave-fuzz-vintage-pre",
        "Vintage Pre defaults",
        "vintage-pre-copy.5xe",
        HdExplicitReplacementTargets::default(),
    );
    exercise_hd_editor_explicit_replacement_lifecycle(
        0x0205_000e,
        &[
            0x3f10_0001,
            0x3f10_0002,
            0x3f10_0003,
            0x3f10_0004,
            0x3f10_0005,
        ],
        "replace-octave-fuzz-screamer",
        "Screamer defaults",
        "screamer-copy.5xe",
        HdExplicitReplacementTargets::default(),
    );
    exercise_hd_editor_explicit_replacement_lifecycle(
        0x0205_0014,
        &[
            0x3f10_0001,
            0x3f10_0002,
            0x3f10_0003,
            0x3f10_0004,
            0x3f10_0005,
        ],
        "replace-octave-fuzz-sub-octave-fuzz",
        "Sub Octave Fuzz defaults",
        "sub-octave-fuzz-copy.5xe",
        HdExplicitReplacementTargets::default(),
    );
    exercise_hd_editor_explicit_replacement_lifecycle(
        0x0200_0011,
        &[0x3f10_0001, 0x3f10_0002],
        "replace-octave-fuzz-noise-gate",
        "Noise Gate defaults",
        "noise-gate-copy.5xe",
        HdExplicitReplacementTargets::default(),
    );

    const BATCH: [(pod_preset::HdDistortionDefault, u32, &str, &str); 12] = [
        (
            pod_preset::HdDistortionDefault::JetFuzz,
            0x0205_000a,
            "Jet Fuzz defaults",
            "jet-fuzz-copy.5xe",
        ),
        (
            pod_preset::HdDistortionDefault::BuzzSaw,
            0x0205_0015,
            "Buzz Saw defaults",
            "buzz-saw-copy.5xe",
        ),
        (
            pod_preset::HdDistortionDefault::ClassicDistortion,
            0x0205_000b,
            "Classic Distortion defaults",
            "classic-distortion-copy.5xe",
        ),
        (
            pod_preset::HdDistortionDefault::ColorDrive,
            0x0205_0018,
            "Color Drive defaults",
            "color-drive-copy.5xe",
        ),
        (
            pod_preset::HdDistortionDefault::FacialFuzz,
            0x0205_0011,
            "Facial Fuzz defaults",
            "facial-fuzz-copy.5xe",
        ),
        (
            pod_preset::HdDistortionDefault::FuzzPi,
            0x0205_000f,
            "Fuzz Pi defaults",
            "fuzz-pi-copy.5xe",
        ),
        (
            pod_preset::HdDistortionDefault::HeavyDistortion,
            0x0205_0016,
            "Heavy Distortion defaults",
            "heavy-distortion-copy.5xe",
        ),
        (
            pod_preset::HdDistortionDefault::JumboFuzz,
            0x0205_0017,
            "Jumbo Fuzz defaults",
            "jumbo-fuzz-copy.5xe",
        ),
        (
            pod_preset::HdDistortionDefault::Line6Distortion,
            0x0205_0012,
            "Line 6 Distortion defaults",
            "line6-distortion-copy.5xe",
        ),
        (
            pod_preset::HdDistortionDefault::Line6Drive,
            0x0205_0013,
            "Line 6 Drive defaults",
            "line6-drive-copy.5xe",
        ),
        (
            pod_preset::HdDistortionDefault::Overdrive,
            0x0205_0010,
            "Overdrive defaults",
            "overdrive-copy.5xe",
        ),
        (
            pod_preset::HdDistortionDefault::TubeDrive,
            0x0205_000d,
            "Tube Drive defaults",
            "tube-drive-copy.5xe",
        ),
    ];
    for (target, model_id, label, output_name) in BATCH {
        exercise_hd_editor_explicit_replacement_lifecycle(
            model_id,
            &[
                0x3f10_0001,
                0x3f10_0002,
                0x3f10_0003,
                0x3f10_0004,
                0x3f10_0005,
            ],
            "replace-octave-fuzz-admitted-default",
            label,
            output_name,
            HdExplicitReplacementTargets {
                distortion: Some(target),
                ..Default::default()
            },
        );
    }

    const DYNAMICS: [(pod_preset::HdDynamicsDefault, u32, &[u32], &str, &str); 8] = [
        (
            pod_preset::HdDynamicsDefault::HardGate,
            0x0200_0013,
            &[0x3f10_0000, 0x3f10_0001, 0x3f10_0002, 0x3f10_0003],
            "Hard Gate defaults",
            "hard-gate-copy.5xe",
        ),
        (
            pod_preset::HdDynamicsDefault::BoostComp,
            0x0200_0012,
            &[
                0x3f10_0001,
                0x3f10_0002,
                0x3f10_0003,
                0x3f10_0004,
                0x3f10_0005,
            ],
            "Boost Comp defaults",
            "boost-comp-copy.5xe",
        ),
        (
            pod_preset::HdDynamicsDefault::RedComp,
            0x0200_000b,
            &[0x3f10_0001, 0x3f10_0002],
            "Red Comp defaults",
            "red-comp-copy.5xe",
        ),
        (
            pod_preset::HdDynamicsDefault::BlueComp,
            0x0200_000c,
            &[0x3f10_0001, 0x3f10_0002],
            "Blue Comp defaults",
            "blue-comp-copy.5xe",
        ),
        (
            pod_preset::HdDynamicsDefault::BlueCompTreb,
            0x0200_000d,
            &[0x3f10_0002, 0x3f10_0001],
            "Blue Comp Treb defaults",
            "blue-comp-treb-copy.5xe",
        ),
        (
            pod_preset::HdDynamicsDefault::VettaComp,
            0x0200_000f,
            &[0x3f10_0001, 0x3f10_0002],
            "Vetta Comp defaults",
            "vetta-comp-copy.5xe",
        ),
        (
            pod_preset::HdDynamicsDefault::VettaJuice,
            0x0200_0010,
            &[0x3f10_0001, 0x3f10_0002],
            "Vetta Juice defaults",
            "vetta-juice-copy.5xe",
        ),
        (
            pod_preset::HdDynamicsDefault::TubeComp,
            0x0200_000e,
            &[0x3f10_0001, 0x3f10_0002],
            "Tube Comp defaults",
            "tube-comp-copy.5xe",
        ),
    ];
    for (target, model_id, keys, label, output_name) in DYNAMICS {
        exercise_hd_editor_explicit_replacement_lifecycle(
            model_id,
            keys,
            "replace-octave-fuzz-admitted-dynamics-default",
            label,
            output_name,
            HdExplicitReplacementTargets {
                dynamics: Some(target),
                ..Default::default()
            },
        );
    }

    const REVERBS: [(pod_preset::HdReverbDefault, u32, &str, &str); 8] = [
        (
            pod_preset::HdReverbDefault::Octo,
            0x0204_0023,
            "Octo defaults",
            "octo-copy.5xe",
        ),
        (
            pod_preset::HdReverbDefault::Ducking,
            0x0204_0022,
            "Ducking defaults",
            "ducking-copy.5xe",
        ),
        (
            pod_preset::HdReverbDefault::Plate,
            0x0204_001e,
            "Plate defaults",
            "plate-copy.5xe",
        ),
        (
            pod_preset::HdReverbDefault::Room,
            0x0204_001f,
            "Room defaults",
            "room-copy.5xe",
        ),
        (
            pod_preset::HdReverbDefault::Chamber,
            0x0204_0020,
            "Chamber defaults",
            "chamber-copy.5xe",
        ),
        (
            pod_preset::HdReverbDefault::Hall,
            0x0204_0021,
            "Hall defaults",
            "hall-copy.5xe",
        ),
        (
            pod_preset::HdReverbDefault::Cave,
            0x0204_0024,
            "Cave defaults",
            "cave-copy.5xe",
        ),
        (
            pod_preset::HdReverbDefault::Tile,
            0x0204_0025,
            "Tile defaults",
            "tile-copy.5xe",
        ),
    ];
    for (target, model_id, label, output_name) in REVERBS {
        exercise_hd_editor_explicit_replacement_lifecycle(
            model_id,
            &[0x3f10_0001, 0x3f10_0002, 0x3f10_0003, 0x3f10_0005],
            "replace-octave-fuzz-admitted-reverb-default",
            label,
            output_name,
            HdExplicitReplacementTargets {
                reverb: Some(target),
                ..Default::default()
            },
        );
    }
}

#[test]
fn hd_editor_numeric_control_updates_only_its_traced_value_offline() {
    let (_context, mut app) = app();
    let source_document = app.variax().document.as_ref().unwrap().as_bytes().to_vec();
    let mut bytes = sample_hd_editor_preset();
    let body = pod_preset::H5EP_HEADER_BYTES;
    let block = body + 0x428;
    bytes[block..block + 4].copy_from_slice(&0x020c_0009u32.to_be_bytes());
    let controls = pod_preset::HD_NUMERIC_CONTROLS
        .into_iter()
        .filter(|control| control.model_id() == 0x020c_0009);
    for (index, control) in controls.enumerate() {
        let row = block + 0x10 + index * 0x14;
        bytes[row..row + 4].copy_from_slice(
            &match index {
                0 => 0x3f10_0001u32,
                1 => 0x3f10_0002u32,
                2 => 0x3f10_0003u32,
                3 => 0x3f10_0004u32,
                _ => 0x3f10_0005u32,
            }
            .to_be_bytes(),
        );
        bytes[row + 4..row + 8].copy_from_slice(&0.5f32.to_bits().to_be_bytes());
        bytes[row + 0x10] = 0;
        assert_eq!(control.max(), 240);
    }
    let original = bytes.clone();
    let index = app.hd_mut().hd_library.import_bytes(bytes).unwrap();
    app.hd_mut().select_hd_file(index);
    let control = app.hd().hd_numeric_control;
    assert_eq!(control.model_label(), "Graphic EQ");
    assert_eq!(control.label(), "80Hz");
    assert_eq!(app.hd().hd_numeric_value, 120);
    app.hd_mut().hd_numeric_value = 180;
    app.hd_mut().apply_hd_numeric_control(index);
    assert!(app.hd().hd_library.is_dirty(index));
    assert_eq!(
        app.variax().document.as_ref().unwrap().as_bytes(),
        source_document
    );
    let edited = app.hd().hd_library.file(index).unwrap().as_bytes();
    let value_at = block + 0x10 + 4;
    assert_eq!(&edited[..value_at], &original[..value_at]);
    assert_eq!(
        &edited[value_at..value_at + 4],
        &0.75f32.to_bits().to_be_bytes()
    );
    assert_eq!(&edited[value_at + 4..], &original[value_at + 4..]);
    assert!(app.variax().read_worker.is_none());
    assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
}

#[test]
fn hd_editor_vintage_pre_type1b_value_patches_through_gui_save_and_quit_offline() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let source_document = app.variax().document.as_ref().unwrap().as_bytes().to_vec();
    let control = pod_preset::HD_NUMERIC_CONTROLS
        .into_iter()
        .find(|control| control.raw_type() == 0x1b)
        .unwrap();
    let mut original = sample_hd_editor_preset();
    let block = pod_preset::H5EP_HEADER_BYTES + 0x428;
    original[block..block + 4].copy_from_slice(&control.model_id().to_be_bytes());
    let row = block + 0x10;
    original[row..row + 4].copy_from_slice(&0x3f10_0005u32.to_be_bytes());
    original[row + 4..row + 8].copy_from_slice(&0x3f00_0000u32.to_be_bytes());
    original[row + 0x10] = 0;
    let index = app
        .hd_mut()
        .hd_library
        .import_bytes(original.clone())
        .unwrap();
    app.hd_mut().select_hd_file(index);

    assert_eq!(app.hd().hd_numeric_control, control);
    assert_eq!(app.hd().hd_numeric_control.raw_type(), 0x1b);
    assert_eq!(
        (
            app.hd().hd_numeric_control.min(),
            app.hd().hd_numeric_control.max()
        ),
        (0, 15_000)
    );
    assert_eq!(app.hd().hd_numeric_value, 7_500);

    // The existing library revert restores the exact loaded baseline.
    app.hd_mut().hd_numeric_value = 12_000;
    app.hd_mut().apply_hd_numeric_control(index);
    assert!(app.hd().hd_library.is_dirty(index));
    app.hd_mut().hd_library.revert_unsaved(index).unwrap();
    assert!(!app.hd().hd_library.is_dirty(index));
    assert_eq!(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        original
    );
    assert_eq!(
        app.variax().document.as_ref().unwrap().as_bytes(),
        source_document
    );

    app.hd_mut().hd_numeric_value = 14_999;
    app.hd_mut().apply_hd_numeric_control(index);
    assert!(app.hd().hd_library.is_dirty(index));
    let mut expected = original.clone();
    expected[row + 4..row + 8].copy_from_slice(&0x3f7f_fba2u32.to_be_bytes());
    assert_eq!(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        expected
    );

    let directory = tempfile::tempdir().unwrap();
    let collision = directory.path().join("existing.h5p");
    let output = directory.path().join("vintage-pre-copy.h5p");
    std::fs::write(&collision, b"keep existing destination").unwrap();
    app.dialogs = Box::new(FakeDialogs {
        pod_save: [Some(collision.clone()), Some(output.clone())].into(),
        ..Default::default()
    });
    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitHd)));
    click_prompt(&context, &mut app, "save");
    assert!(app.hd().hd_library.is_dirty(index));
    assert!(!app.permit_close);
    assert_eq!(
        std::fs::read(&collision).unwrap(),
        b"keep existing destination"
    );
    click_prompt(&context, &mut app, "save");
    assert!(app.permit_close);
    assert!(!app.hd().hd_library.any_dirty());
    assert!(app.variax().read_worker.is_none());
    assert_eq!(std::fs::read(&output).unwrap(), expected);
    let reopened = pod_preset::OpaqueH5epPreset::parse(std::fs::read(output).unwrap())
        .expect("reopen saved type-0x1B copy");
    assert_eq!(reopened.hd_numeric_control_value(0, control), Some(14_999));
    assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
}

#[test]
fn hd_editor_mapped_type1_control_updates_its_traced_value_offline() {
    let (_context, mut app) = app();
    let control = pod_preset::HD_TYPE1_MAPPED_CONTROLS[0];
    let mut bytes = sample_hd_editor_preset();
    let body = pod_preset::H5EP_HEADER_BYTES;
    let block = body + 0x428;
    bytes[block..block + 4].copy_from_slice(&control.model_id().to_be_bytes());
    let row = block + 0x10;
    bytes[row..row + 4].copy_from_slice(&control.key().to_be_bytes());
    bytes[row + 4..row + 8].copy_from_slice(&0.5f32.to_bits().to_be_bytes());
    bytes[row + 0x10] = 0;
    let original = bytes.clone();
    let index = app.hd_mut().hd_library.import_bytes(bytes).unwrap();
    app.hd_mut().select_hd_file(index);
    assert_eq!(app.hd().hd_mapped_control, control);
    assert_eq!(app.hd().hd_control_percent, 50);

    app.hd_mut().hd_control_percent = 75;
    app.hd_mut().apply_hd_mapped_control(index);
    assert_eq!(
        app.hd().hd_library.file(index).and_then(|file| match file {
            HdEditFile::H5ep(preset) => {
                preset.hd_type1_mapped_control_percent(0, control)
            }
            _ => None,
        }),
        Some(75)
    );
    let mut expected = original;
    expected[row + 4..row + 8].copy_from_slice(&0.75f32.to_bits().to_be_bytes());
    assert_eq!(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        expected
    );
    assert!(app.hd().hd_library.is_dirty(index));
    assert!(app.variax().read_worker.is_none());
}

#[test]
fn hd_type9_choice_stages_per_entry_slot_then_applies_or_discards_explicitly() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let slot_index = 5;
    let row_index = 11;
    let original = sample_hd_type9_preset(slot_index, row_index, 0x3f40_0000);
    let index = app
        .hd_mut()
        .hd_library
        .import_bytes(original.clone())
        .unwrap();
    app.hd_mut().select_hd_file(index);
    app.select_test_editor::<HdEditor>();
    app.hd_mut().hd_effect_slot = slot_index;
    let slot = pod_preset::HdFxSlot::new(slot_index).unwrap();

    assert!(matches!(
        app.hd().hd_library.file(index),
        Some(HdEditFile::H5ep(preset))
            if preset.hd_type9_choice_value(slot)
                == pod_preset::HdType9ChoiceValue::UnknownFinite(0x3f40_0000)
    ));
    click_hd_type9_choice(&context, &mut app, 2);
    hd_editor_frame(&context, &mut app, vec![]);
    assert_hd_bytes_eq(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        &original,
        "staging must not alter file bytes",
    );
    assert_eq!(
        app.hd().hd_type9_pending,
        Some(PendingHdType9Choice {
            file_index: index,
            slot,
            model_id: pod_preset::HD_TYPE9_CHOICE_MODEL_ID,
            choice: pod_preset::HdType9Choice::Choice2,
        })
    );
    click_hd_editor(&context, &mut app, "apply-hd-type9-choice");
    let value_at =
        pod_preset::H5EP_HEADER_BYTES + 0x428 + slot_index * 0x100 + 0x10 + row_index * 0x14 + 4;
    let mut expected = original.clone();
    expected[value_at..value_at + 4].copy_from_slice(&[0x3f, 0x80, 0x00, 0x00]);
    assert_hd_bytes_eq(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        &expected,
        "applying the staged GUI choice",
    );
    assert!(app.hd().hd_library.is_dirty(index));
    assert!(app.hd().hd_type9_pending.is_none());
    assert!(app.hd().hd_message.contains("Nothing was sent"));

    let directory = tempfile::tempdir().unwrap();
    let saved_copy = directory.path().join("type9-copy.5xe");
    app.dialogs = Box::new(FakeDialogs {
        pod_save: [Some(saved_copy.clone())].into(),
        pod_open: [Some(saved_copy.clone())].into(),
        ..Default::default()
    });
    click_hd_editor(&context, &mut app, "save-hd-copy");
    assert_eq!(std::fs::read(&saved_copy).unwrap(), expected);
    assert!(!app.hd().hd_library.is_dirty(index));
    click_hd_editor(&context, &mut app, "load-hd-file");
    let reopened_index = app
        .hd()
        .hd_selected
        .expect("GUI open selects the saved copy");
    assert_ne!(reopened_index, index);
    assert_hd_bytes_eq(
        app.hd().hd_library.file(reopened_index).unwrap().as_bytes(),
        &expected,
        "GUI save and reopen",
    );
    app.hd_mut().select_hd_file(index);
    app.hd_mut().hd_effect_slot = slot_index;

    click_hd_type9_choice(&context, &mut app, 0);
    hd_editor_frame(&context, &mut app, vec![]);
    click_hd_editor(&context, &mut app, "discard-hd-type9-choice");
    assert_hd_bytes_eq(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        &expected,
        "discarding a later staged GUI choice",
    );
    assert!(app.hd().hd_type9_pending.is_none());

    let other_index = app
        .hd_mut()
        .hd_library
        .import_bytes(sample_hd_editor_preset())
        .unwrap();
    app.hd_mut().stage_hd_type9_choice(
        index,
        slot,
        pod_preset::HD_TYPE9_CHOICE_MODEL_ID,
        pod_preset::HdType9Choice::Choice1,
    );
    app.hd_mut().select_hd_file(other_index);
    assert!(app.hd().hd_type9_pending.is_none());
    app.hd_mut().select_hd_file(index);
    app.hd_mut().hd_effect_slot = slot_index;
    app.hd_mut().stage_hd_type9_choice(
        index,
        slot,
        pod_preset::HD_TYPE9_CHOICE_MODEL_ID,
        pod_preset::HdType9Choice::Choice1,
    );
    app.hd_mut().hd_effect_slot = 4;
    hd_editor_frame(&context, &mut app, vec![]);
    assert!(app.hd().hd_type9_pending.is_none());
    app.hd_mut().hd_effect_slot = slot_index;

    click_hd_type9_choice(&context, &mut app, 0);
    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitHd)));
    click_prompt(&context, &mut app, "cancel");
    assert!(app.test_pending().is_none());
    assert!(app.hd().hd_type9_pending.is_some());
    assert!(!app.permit_close);

    app.request_close(&context);
    click_prompt(&context, &mut app, "apply-type9");
    assert!(app.hd().hd_type9_pending.is_none());
    assert!(app.hd().hd_library.is_dirty(index));
    assert!(matches!(app.test_pending(), Some(Pending::QuitHd)));
    click_prompt(&context, &mut app, "cancel");
    assert!(app.test_pending().is_none());
    assert!(app.hd().hd_library.is_dirty(index));
    assert!(!app.permit_close);

    click_hd_type9_choice(&context, &mut app, 1);
    app.request_close(&context);
    click_prompt(&context, &mut app, "discard");
    assert!(app.hd().hd_type9_pending.is_none());
    assert!(app.hd().hd_library.is_dirty(index));
    assert!(matches!(app.test_pending(), Some(Pending::QuitHd)));
    click_prompt(&context, &mut app, "discard");
    assert!(!app.hd().hd_library.is_dirty(index));
    assert_hd_bytes_eq(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        &expected,
        "discarding edited type-9 value restores saved baseline",
    );
    assert!(app.permit_close);
    assert!(app.variax().read_worker.is_none());
}

#[test]
fn hd_type9_catalog_additions_use_the_existing_gui_staging_flow() {
    let associations = [
        (0x020a_0010, 0x3f10_0004, 1),
        (0x020a_000f, 0x3f10_0004, 3),
        (0x020a_0012, 0x3f10_0003, 7),
    ];
    for (model_id, key, slot_index) in associations {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let row_index = 5;
        let original =
            sample_hd_type9_preset_for(slot_index, row_index, 0x3f40_0000, model_id, key);
        let index = app
            .hd_mut()
            .hd_library
            .import_bytes(original.clone())
            .unwrap();
        app.hd_mut().select_hd_file(index);
        app.select_test_editor::<HdEditor>();
        app.hd_mut().hd_effect_slot = slot_index;

        click_hd_type9_choice(&context, &mut app, 2);
        hd_editor_frame(&context, &mut app, vec![]);
        assert_hd_bytes_eq(
            app.hd().hd_library.file(index).unwrap().as_bytes(),
            &original,
            "new catalog selection stages without editing bytes",
        );
        assert_eq!(
            app.hd().hd_type9_pending,
            Some(PendingHdType9Choice {
                file_index: index,
                slot: pod_preset::HdFxSlot::new(slot_index).unwrap(),
                model_id,
                choice: pod_preset::HdType9Choice::Choice2,
            })
        );

        app.hd_mut().hd_effect_slot = (slot_index + 1) % 8;
        hd_editor_frame(&context, &mut app, vec![]);
        assert!(app.hd().hd_type9_pending.is_none());
        app.hd_mut().hd_effect_slot = slot_index;

        click_hd_type9_choice(&context, &mut app, 1);
        hd_editor_frame(&context, &mut app, vec![]);
        click_hd_editor(&context, &mut app, "apply-hd-type9-choice");
        let value_at = pod_preset::H5EP_HEADER_BYTES
            + 0x428
            + slot_index * 0x100
            + 0x10
            + row_index * 0x14
            + 4;
        let mut expected = original;
        expected[value_at..value_at + 4].copy_from_slice(&0x3f00_0000u32.to_be_bytes());
        assert_hd_bytes_eq(
            app.hd().hd_library.file(index).unwrap().as_bytes(),
            &expected,
            "new catalog GUI apply changes only the selected value word",
        );
        assert!(app.hd().hd_library.is_dirty(index));

        click_hd_type9_choice(&context, &mut app, 0);
        hd_editor_frame(&context, &mut app, vec![]);
        click_hd_editor(&context, &mut app, "discard-hd-type9-choice");
        assert_hd_bytes_eq(
            app.hd().hd_library.file(index).unwrap().as_bytes(),
            &expected,
            "discarding a staged new catalog choice preserves applied bytes",
        );
        assert!(app.hd().hd_type9_pending.is_none());
    }
}

#[test]
fn hd_type9_gui_does_not_stage_nonfinite_or_unavailable_values() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    app.select_test_editor::<HdEditor>();
    for (index, bits, invalid_marker) in [(0, 0x7fc0_0000, false), (1, 0x3f40_0000, true)] {
        let mut bytes = sample_hd_type9_preset(2, 3, bits);
        if invalid_marker {
            let row = pod_preset::H5EP_HEADER_BYTES + 0x428 + 2 * 0x100 + 0x10 + 3 * 0x14;
            bytes[row + 0x10] = 1;
        }
        let file_index = app.hd_mut().hd_library.import_bytes(bytes.clone()).unwrap();
        assert_eq!(file_index, index);
        app.hd_mut().select_hd_file(file_index);
        app.hd_mut().hd_effect_slot = 2;
        hd_editor_frame(&context, &mut app, vec![]);
        click_hd_type9_choice(&context, &mut app, 1);
        assert!(app.hd().hd_type9_pending.is_none());
        assert_hd_bytes_eq(
            app.hd().hd_library.file(file_index).unwrap().as_bytes(),
            &bytes,
            "refused non-finite or unavailable GUI value",
        );
    }

    let mut malformed = sample_hd_type9_preset(2, 3, 0x3f40_0000);
    malformed[7] = 2;
    assert!(app.hd_mut().hd_library.import_bytes(malformed).is_err());
}

#[test]
fn hd_type9_idle_redraw_preserves_the_users_library_scroll_position() {
    fn scroll_frame(context: &egui::Context, app: &mut Workbench) {
        let _ = context.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 415.0))),
                ..Default::default()
            },
            |ctx| app.render(ctx),
        );
    }
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let bytes = sample_hd_type9_preset(0, 3, 0x3f40_0000);
    let index = app.hd_mut().hd_library.import_bytes(bytes).unwrap();
    app.hd_mut().select_hd_file(index);
    app.select_test_editor::<HdEditor>();
    scroll_frame(&context, &mut app);

    let (scroll_id, viewport, content_height) = app
        .hd()
        .hd_scroll_area
        .expect("rendered HD editor scroll area");
    let max_offset = (content_height - viewport.height()).max(0.0);
    assert!(
        max_offset > 0.0,
        "fixture must require vertical scrolling; content={content_height} viewport={viewport:?}"
    );
    let mut state = egui::scroll_area::State::load(&context, scroll_id).unwrap();
    state.offset.y = max_offset;
    state.store(&context, scroll_id);

    scroll_frame(&context, &mut app);
    let (scroll_id, viewport, _) = app.hd().hd_scroll_area.unwrap();
    let after_idle = egui::scroll_area::State::load(&context, scroll_id).unwrap();
    assert!((after_idle.offset.y - max_offset).abs() < 1.0);
    assert!(
        !viewport.intersects(app.hd().hd_buttons["hd-type9-choice-combo"]),
        "the scrolled-away type-9 control must stay offscreen after redraw: {viewport:?}, {:?}",
        app.hd().hd_buttons["hd-type9-choice-combo"]
    );

    scroll_frame(&context, &mut app);
    let after_second_idle = egui::scroll_area::State::load(&context, scroll_id).unwrap();
    assert!((after_second_idle.offset.y - after_idle.offset.y).abs() < 1.0);
}

#[test]
fn hd_type8_choice_uses_distinct_two_option_ui_and_clears_cross_type_staging() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let slot_index = 5;
    let row_index = 11;
    let type8_original = sample_hd_type8_preset(slot_index, row_index, 0x3f40_0000);
    let type8 = app
        .hd_mut()
        .hd_library
        .import_bytes(type8_original.clone())
        .unwrap();
    let type9 = app
        .hd_mut()
        .hd_library
        .import_bytes(sample_hd_type9_preset(2, 3, 0x3f40_0000))
        .unwrap();
    app.hd_mut().select_hd_file(type8);
    app.select_test_editor::<HdEditor>();
    app.hd_mut().hd_effect_slot = slot_index;

    click_hd_type8_choice(&context, &mut app, 1);
    hd_editor_frame(&context, &mut app, vec![]);
    assert_hd_bytes_eq(
        app.hd().hd_library.file(type8).unwrap().as_bytes(),
        &type8_original,
        "type-8 staging leaves source bytes untouched",
    );
    assert_eq!(
        app.hd().hd_type8_pending,
        Some(PendingHdType8Choice {
            file_index: type8,
            slot: pod_preset::HdFxSlot::new(slot_index).unwrap(),
            model_id: pod_preset::HD_TYPE8_CHOICE_MODEL_ID,
            choice: pod_preset::HdType8Choice::Choice1,
        })
    );
    assert!(app.hd().hd_type9_pending.is_none());
    click_hd_editor(&context, &mut app, "apply-hd-type8-choice");
    let value_at =
        pod_preset::H5EP_HEADER_BYTES + 0x428 + slot_index * 0x100 + 0x10 + row_index * 0x14 + 4;
    let mut expected = type8_original.clone();
    expected[value_at..value_at + 4].copy_from_slice(&[0x3f, 0x80, 0x00, 0x00]);
    assert_hd_bytes_eq(
        app.hd().hd_library.file(type8).unwrap().as_bytes(),
        &expected,
        "type-8 apply patches only value word",
    );
    assert!(app.hd().hd_type8_pending.is_none());
    assert!(app.hd().hd_library.is_dirty(type8));

    let directory = tempfile::tempdir().unwrap();
    let saved_copy = directory.path().join("type8-copy.5xe");
    app.dialogs = Box::new(FakeDialogs {
        pod_save: [Some(saved_copy.clone())].into(),
        pod_open: [Some(saved_copy.clone())].into(),
        ..Default::default()
    });
    click_hd_editor(&context, &mut app, "save-hd-copy");
    assert_eq!(std::fs::read(&saved_copy).unwrap(), expected);
    assert!(!app.hd().hd_library.is_dirty(type8));
    click_hd_editor(&context, &mut app, "load-hd-file");
    let reopened = app
        .hd()
        .hd_selected
        .expect("GUI open selects the saved type-8 copy");
    assert_ne!(reopened, type8);
    assert_hd_bytes_eq(
        app.hd().hd_library.file(reopened).unwrap().as_bytes(),
        &expected,
        "GUI type-8 save and reopen",
    );
    app.hd_mut().select_hd_file(type8);
    app.hd_mut().hd_effect_slot = slot_index;

    click_hd_type8_choice(&context, &mut app, 0);
    hd_editor_frame(&context, &mut app, vec![]);
    click_hd_editor(&context, &mut app, "discard-hd-type8-choice");
    assert!(app.hd().hd_type8_pending.is_none());
    assert_hd_bytes_eq(
        app.hd().hd_library.file(type8).unwrap().as_bytes(),
        &expected,
        "discard clears staged type-8 choice only",
    );

    // Pending edits are tied to exact file, slot and model context.
    app.hd_mut().stage_hd_type8_choice(
        type8,
        pod_preset::HdFxSlot::new(slot_index).unwrap(),
        pod_preset::HD_TYPE8_CHOICE_MODEL_ID,
        pod_preset::HdType8Choice::Choice0,
    );
    app.hd_mut().select_hd_file(type9);
    assert!(app.hd().hd_type8_pending.is_none());
    app.hd_mut().hd_effect_slot = 2;
    click_hd_type9_choice(&context, &mut app, 2);
    hd_editor_frame(&context, &mut app, vec![]);
    assert!(app.hd().hd_type9_pending.is_some());
    app.hd_mut().select_hd_file(type8);
    assert!(app.hd().hd_type9_pending.is_none());
    assert!(app.hd().hd_type8_pending.is_none());

    // Same entry and same file slot can change type context while a choice
    // is staged. A real editor frame must discard each stale stage without
    // applying its previous value.
    app.hd_mut().hd_effect_slot = slot_index;
    click_hd_type8_choice(&context, &mut app, 0);
    hd_editor_frame(&context, &mut app, vec![]);
    assert!(app.hd().hd_type8_pending.is_some());
    let type9_same_context = sample_hd_type9_preset(slot_index, row_index, 0x3f40_0000);
    *app.hd_mut().hd_library.file_mut(type8).unwrap() =
        HdEditFile::parse(type9_same_context.clone()).unwrap();
    hd_editor_frame(&context, &mut app, vec![]);
    assert!(app.hd().hd_type8_pending.is_none());
    assert!(app.hd().hd_type9_pending.is_none());
    assert_hd_bytes_eq(
        app.hd().hd_library.file(type8).unwrap().as_bytes(),
        &type9_same_context,
        "type-8 pending stage invalidated by same-file type-9 model",
    );

    click_hd_type9_choice(&context, &mut app, 2);
    hd_editor_frame(&context, &mut app, vec![]);
    assert!(app.hd().hd_type9_pending.is_some());
    *app.hd_mut().hd_library.file_mut(type8).unwrap() =
        HdEditFile::parse(expected.clone()).unwrap();
    hd_editor_frame(&context, &mut app, vec![]);
    assert!(app.hd().hd_type9_pending.is_none());
    assert!(app.hd().hd_type8_pending.is_none());
    assert_hd_bytes_eq(
        app.hd().hd_library.file(type8).unwrap().as_bytes(),
        &expected,
        "type-9 pending stage invalidated by same-file type-8 model",
    );

    app.hd_mut().hd_effect_slot = slot_index;
    click_hd_type8_choice(&context, &mut app, 0);
    hd_editor_frame(&context, &mut app, vec![]);
    let staged = app
        .hd()
        .hd_type8_pending
        .expect("GUI type-8 combo stages a value");
    let mut bad_marker = expected.clone();
    bad_marker[0xa4c] = 1;
    *app.hd_mut().hd_library.file_mut(type8).unwrap() =
        HdEditFile::parse(bad_marker.clone()).unwrap();
    assert!(app.hd().hd_library.is_dirty(type8));
    hd_editor_frame(&context, &mut app, vec![]);
    click_hd_editor(&context, &mut app, "apply-hd-type8-choice");
    assert_hd_bytes_eq(
        app.hd().hd_library.file(type8).unwrap().as_bytes(),
        &bad_marker,
        "invalid row marker makes actual Apply preserve current bytes",
    );
    assert!(app.hd().hd_library.is_dirty(type8));
    assert_eq!(app.hd().hd_type8_pending, Some(staged));
    assert!(app.hd().hd_message.contains("marker"));

    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitHd)));
    click_prompt(&context, &mut app, "discard");
    assert!(app.hd().hd_type8_pending.is_none());
    assert!(app.hd().hd_library.is_dirty(type8));
    click_prompt(&context, &mut app, "discard");
    assert_hd_bytes_eq(
        app.hd().hd_library.file(type8).unwrap().as_bytes(),
        &expected,
        "quit discard restores the previously saved baseline",
    );
    assert!(!app.hd().hd_library.is_dirty(type8));

    assert!(app.variax().read_worker.is_none());
}

#[test]
fn hd_type10_and_type9_choices_stage_and_apply_independently_on_same_model() {
    for model_id in [pod_preset::HD_TYPE10_CHOICE_MODEL_ID, 0x020a_000f] {
        hd_type10_and_type9_choices_stage_and_apply_independently_for(model_id);
    }
}

fn hd_type10_and_type9_choices_stage_and_apply_independently_for(model_id: u32) {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let slot_index = 5;
    let row_index = 8;
    let key = pod_preset::hd_type10_choice_key(model_id).unwrap();
    let original = sample_hd_type10_preset_for(slot_index, row_index, 0x3eaa_aaab, model_id, key);
    let index = app
        .hd_mut()
        .hd_library
        .import_bytes(original.clone())
        .unwrap();
    app.hd_mut().select_hd_file(index);
    app.select_test_editor::<HdEditor>();
    app.hd_mut().hd_effect_slot = slot_index;
    click_hd_type10_choice(&context, &mut app, 1);
    hd_editor_frame(&context, &mut app, vec![]);
    assert!(app.hd().hd_type10_pending.is_some());
    assert_hd_bytes_eq(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        &original,
        "type-10 stage is display only",
    );
    click_hd_type9_choice(&context, &mut app, 1);
    hd_editor_frame(&context, &mut app, vec![]);
    assert_eq!(
        app.hd().hd_type9_pending,
        Some(PendingHdType9Choice {
            file_index: index,
            slot: pod_preset::HdFxSlot::new(slot_index).unwrap(),
            model_id,
            choice: pod_preset::HdType9Choice::Choice1,
        })
    );
    assert!(
        app.hd().hd_type10_pending.is_some(),
        "staging same-model type 9 must retain type-10 stage"
    );

    let type10_value =
        pod_preset::H5EP_HEADER_BYTES + 0x428 + slot_index * 0x100 + 0x10 + row_index * 0x14 + 4;
    let type9_row = (row_index + 1) % 12;
    let type9_value =
        pod_preset::H5EP_HEADER_BYTES + 0x428 + slot_index * 0x100 + 0x10 + type9_row * 0x14 + 4;
    let staged_type10 = app.hd().hd_type10_pending;
    assert_eq!(
        staged_type10,
        Some(PendingHdType10Choice {
            file_index: index,
            slot: pod_preset::HdFxSlot::new(slot_index).unwrap(),
            model_id,
            choice: pod_preset::HdType10Choice::Choice1,
        })
    );
    click_hd_editor(&context, &mut app, "apply-hd-type9-choice");
    let after9 = app.hd().hd_library.file(index).unwrap().as_bytes().to_vec();
    assert_eq!(&after9[type9_value..type9_value + 4], &[0x3f, 0, 0, 0]);
    assert_eq!(
        &after9[type10_value..type10_value + 4],
        &original[type10_value..type10_value + 4],
        "applying type 9 changes only its value word"
    );
    assert_eq!(
        app.hd().hd_type10_pending,
        staged_type10,
        "applying type 9 preserves the exact staged type-10 value"
    );

    click_hd_editor(&context, &mut app, "apply-hd-type10-choice");
    let after10 = app.hd().hd_library.file(index).unwrap().as_bytes().to_vec();
    assert_eq!(&after10[type9_value..type9_value + 4], &[0x3f, 0, 0, 0]);
    assert_eq!(
        &after10[type10_value..type10_value + 4],
        &[0x3f, 0x80, 0, 0]
    );
    assert!(app.hd().hd_type10_pending.is_none());
    assert!(app.hd().hd_type9_pending.is_none());

    click_hd_type10_choice(&context, &mut app, 0);
    hd_editor_frame(&context, &mut app, vec![]);
    click_hd_type9_choice(&context, &mut app, 2);
    hd_editor_frame(&context, &mut app, vec![]);
    let staged_type10 = app.hd().hd_type10_pending;
    let staged_type9 = app.hd().hd_type9_pending;
    assert!(staged_type10.is_some() && staged_type9.is_some());
    click_hd_editor(&context, &mut app, "discard-hd-type9-choice");
    assert!(app.hd().hd_type9_pending.is_none());
    assert_eq!(
        app.hd().hd_type10_pending,
        staged_type10,
        "discard type 9 preserves the exact type-10 stage"
    );
    assert_eq!(app.hd().hd_library.file(index).unwrap().as_bytes(), after10);
    click_hd_editor(&context, &mut app, "discard-hd-type10-choice");
    assert!(app.hd().hd_type10_pending.is_none());
    assert_eq!(app.hd().hd_library.file(index).unwrap().as_bytes(), after10);

    click_hd_type10_choice(&context, &mut app, 0);
    hd_editor_frame(&context, &mut app, vec![]);
    click_hd_type9_choice(&context, &mut app, 2);
    hd_editor_frame(&context, &mut app, vec![]);
    let staged_type9 = app.hd().hd_type9_pending;
    let staged_type10 = app.hd().hd_type10_pending;
    assert!(staged_type10.is_some() && staged_type9.is_some());
    click_hd_editor(&context, &mut app, "discard-hd-type10-choice");
    assert!(app.hd().hd_type10_pending.is_none());
    assert_eq!(
        app.hd().hd_type9_pending,
        staged_type9,
        "discard type 10 preserves the exact type-9 stage"
    );
    assert_eq!(app.hd().hd_library.file(index).unwrap().as_bytes(), after10);
    click_hd_editor(&context, &mut app, "discard-hd-type9-choice");
    assert!(app.hd().hd_type9_pending.is_none());
    assert_eq!(app.hd().hd_library.file(index).unwrap().as_bytes(), after10);

    let directory = tempfile::tempdir().unwrap();
    let collision = directory.path().join("type10-existing.5xe");
    std::fs::write(&collision, b"keep existing").unwrap();
    app.dialogs = Box::new(FakeDialogs {
        pod_save: [Some(collision.clone())].into(),
        ..Default::default()
    });
    click_hd_editor(&context, &mut app, "save-hd-copy");
    assert_eq!(std::fs::read(&collision).unwrap(), b"keep existing");
    assert!(app.hd().hd_library.is_dirty(index));
    let copy = directory.path().join("type10-copy.5xe");
    app.dialogs = Box::new(FakeDialogs {
        pod_save: [Some(copy.clone())].into(),
        pod_open: [Some(copy.clone())].into(),
        ..Default::default()
    });
    click_hd_editor(&context, &mut app, "save-hd-copy");
    assert_hd_bytes_eq(
        &std::fs::read(&copy).unwrap(),
        &after10,
        "type-10 copy save",
    );
    assert!(!app.hd().hd_library.is_dirty(index));
    click_hd_editor(&context, &mut app, "load-hd-file");
    let reopened = app.hd().hd_selected.expect("saved type-10 copy reopens");
    assert_hd_bytes_eq(
        app.hd().hd_library.file(reopened).unwrap().as_bytes(),
        &after10,
        "type-10 copy reopen",
    );
    app.hd_mut().select_hd_file(index);
    app.hd_mut().hd_effect_slot = slot_index;

    click_hd_type10_choice(&context, &mut app, 0);
    hd_editor_frame(&context, &mut app, vec![]);
    let staged = app
        .hd()
        .hd_type10_pending
        .expect("second type-10 choice is staged");
    let mut bad_marker = after10.clone();
    let marker =
        pod_preset::H5EP_HEADER_BYTES + 0x428 + slot_index * 0x100 + 0x10 + row_index * 0x14 + 0x10;
    bad_marker[marker] = 1;
    *app.hd_mut().hd_library.file_mut(index).unwrap() =
        HdEditFile::parse(bad_marker.clone()).unwrap();
    assert!(app.hd().hd_library.is_dirty(index));
    hd_editor_frame(&context, &mut app, vec![]);
    click_hd_editor(&context, &mut app, "apply-hd-type10-choice");
    assert_hd_bytes_eq(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        &bad_marker,
        "failed type-10 Apply preserves bytes",
    );
    assert_eq!(app.hd().hd_type10_pending, Some(staged));
    assert!(app.hd().hd_library.is_dirty(index));
    app.hd_mut().hd_library.revert_unsaved(index).unwrap();
    assert_hd_bytes_eq(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        &after10,
        "type-10 revert returns to saved baseline",
    );
    assert!(
        app.hd().hd_type10_pending.is_some(),
        "failed Apply keeps its intended stage"
    );
    click_hd_type9_choice(&context, &mut app, 0);
    assert!(app.hd().hd_type9_pending.is_some());
    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitHd)));
    click_prompt(&context, &mut app, "discard");
    assert!(app.hd().hd_type9_pending.is_none());
    assert!(
        app.hd().hd_type10_pending.is_some(),
        "quit processes the remaining type-10 stage"
    );
    click_prompt(&context, &mut app, "discard");
    assert!(app.hd().hd_type10_pending.is_none());
    assert!(
        app.permit_close,
        "quit closes only after all staged choices are resolved"
    );
}

#[test]
fn hd_type10_pending_is_invalidated_on_same_entry_model_change_and_file_replacement() {
    for model_id in [pod_preset::HD_TYPE10_CHOICE_MODEL_ID, 0x020a_000f] {
        hd_type10_pending_is_invalidated_for(model_id);
    }
}

fn hd_type10_pending_is_invalidated_for(model_id: u32) {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let slot_index = 5;
    let key = pod_preset::hd_type10_choice_key(model_id).unwrap();
    let bytes = sample_hd_type10_preset_for(slot_index, 8, 0x3eaa_aaab, model_id, key);
    let index = app.hd_mut().hd_library.import_bytes(bytes.clone()).unwrap();
    app.hd_mut().select_hd_file(index);
    app.select_test_editor::<HdEditor>();
    app.hd_mut().hd_effect_slot = slot_index;

    click_hd_type10_choice(&context, &mut app, 1);
    hd_editor_frame(&context, &mut app, vec![]);
    app.hd_mut().hd_effect_slot = 4;
    hd_editor_frame(&context, &mut app, vec![]);
    assert!(
        app.hd().hd_type10_pending.is_none(),
        "slot change clears stale stage"
    );

    let another_index = app
        .hd_mut()
        .hd_library
        .import_bytes(sample_hd_type10_preset_for(
            3,
            4,
            0x3eaa_aaab,
            model_id,
            key,
        ))
        .unwrap();
    app.hd_mut().hd_effect_slot = slot_index;
    click_hd_type10_choice(&context, &mut app, 1);
    hd_editor_frame(&context, &mut app, vec![]);
    assert_eq!(
        app.hd().hd_type10_pending,
        Some(PendingHdType10Choice {
            file_index: index,
            slot: pod_preset::HdFxSlot::new(slot_index).unwrap(),
            model_id,
            choice: pod_preset::HdType10Choice::Choice1,
        }),
        "combo establishes the exact stage before selection changes"
    );
    app.hd_mut().select_hd_file(another_index);
    assert!(
        app.hd().hd_type10_pending.is_none(),
        "selection clears stage"
    );
    app.hd_mut().select_hd_file(index);
    app.hd_mut().hd_effect_slot = slot_index;
    click_hd_type10_choice(&context, &mut app, 1);
    hd_editor_frame(&context, &mut app, vec![]);
    click_hd_type9_choice(&context, &mut app, 1);
    hd_editor_frame(&context, &mut app, vec![]);
    assert!(app.hd().hd_type10_pending.is_some());
    assert!(app.hd().hd_type9_pending.is_some());

    let model_offset = pod_preset::H5EP_HEADER_BYTES + 0x428 + slot_index * 0x100;
    let mut changed_model = bytes.clone();
    let other_admitted_model = if model_id == pod_preset::HD_TYPE10_CHOICE_MODEL_ID {
        0x020a_000f
    } else {
        pod_preset::HD_TYPE10_CHOICE_MODEL_ID
    };
    changed_model[model_offset..model_offset + 4]
        .copy_from_slice(&other_admitted_model.to_be_bytes());
    *app.hd_mut().hd_library.file_mut(index).unwrap() =
        HdEditFile::parse(changed_model.clone()).unwrap();
    hd_editor_frame(&context, &mut app, vec![]);
    assert!(app.hd().hd_type10_pending.is_none());
    assert!(app.hd().hd_type9_pending.is_none());
    assert_hd_bytes_eq(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        &changed_model,
        "same-entry model change clears both stale stages without editing bytes",
    );

    app.hd_mut().hd_library.revert_unsaved(index).unwrap();
    app.hd_mut().select_hd_file(index);
    app.hd_mut().hd_effect_slot = slot_index;
    hd_editor_frame(&context, &mut app, vec![]);
    click_hd_type10_choice(&context, &mut app, 0);
    hd_editor_frame(&context, &mut app, vec![]);
    let staged = app.hd().hd_type10_pending;
    assert_eq!(
        staged,
        Some(PendingHdType10Choice {
            file_index: index,
            slot: pod_preset::HdFxSlot::new(slot_index).unwrap(),
            model_id,
            choice: pod_preset::HdType10Choice::Choice0,
        }),
        "actual combo interaction establishes the stage before Open"
    );

    let directory = tempfile::tempdir().unwrap();
    let replacement_path = directory.path().join("replacement.5xe");
    let replacement_bytes = sample_hd_type10_preset_for(2, 4, 0x3eaa_aaab, model_id, key);
    std::fs::write(&replacement_path, &replacement_bytes).unwrap();
    app.dialogs = Box::new(FakeDialogs {
        pod_open: [None, Some(replacement_path)].into(),
        ..Default::default()
    });
    click_hd_editor(&context, &mut app, "load-hd-file");
    assert_eq!(
        app.hd().hd_selected,
        Some(index),
        "cancel keeps the selected entry"
    );
    assert_eq!(
        app.hd().hd_type10_pending,
        staged,
        "cancel keeps the established stage"
    );
    assert!(app.hd().hd_message.contains("Load cancelled"));

    click_hd_editor(&context, &mut app, "load-hd-file");
    let replacement = app
        .hd()
        .hd_selected
        .expect("successful Open selects imported file");
    assert_ne!(replacement, index);
    assert!(
        app.hd().hd_type10_pending.is_none(),
        "successful Open clears stale stage"
    );
    assert_hd_bytes_eq(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        &bytes,
        "successful Open leaves the previously staged source bytes/baseline intact",
    );
    assert!(!app.hd().hd_library.is_dirty(index));
    assert_hd_bytes_eq(
        app.hd().hd_library.file(replacement).unwrap().as_bytes(),
        &replacement_bytes,
        "successful Open imports the selected offline replacement",
    );
}

#[test]
fn hd_revert_clears_only_pending_type10_for_the_reverted_file() {
    for model_id in [pod_preset::HD_TYPE10_CHOICE_MODEL_ID, 0x020a_000f] {
        hd_revert_clears_type10_pending_for(model_id);
    }
}

fn hd_revert_clears_type10_pending_for(model_id: u32) {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let key = pod_preset::hd_type10_choice_key(model_id).unwrap();
    let bytes = sample_hd_type10_preset_for(5, 8, 0x3eaa_aaab, model_id, key);
    let reverted = app.hd_mut().hd_library.import_bytes(bytes.clone()).unwrap();
    let unrelated = app.hd_mut().hd_library.import_bytes(bytes.clone()).unwrap();
    app.hd_mut().select_hd_file(reverted);
    app.select_test_editor::<HdEditor>();
    app.hd_mut().hd_effect_slot = 5;
    click_hd_type10_choice(&context, &mut app, 1);
    hd_editor_frame(&context, &mut app, vec![]);
    let staged = app.hd().hd_type10_pending;
    assert_eq!(
        staged,
        Some(PendingHdType10Choice {
            file_index: reverted,
            slot: pod_preset::HdFxSlot::new(5).unwrap(),
            model_id,
            choice: pod_preset::HdType10Choice::Choice1,
        }),
        "actual GUI interaction establishes stage before revert"
    );
    let model_offset = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100;
    let mut dirty_bytes = bytes.clone();
    let other_admitted_model = if model_id == pod_preset::HD_TYPE10_CHOICE_MODEL_ID {
        0x020a_000f
    } else {
        pod_preset::HD_TYPE10_CHOICE_MODEL_ID
    };
    dirty_bytes[model_offset..model_offset + 4]
        .copy_from_slice(&other_admitted_model.to_be_bytes());
    *app.hd_mut().hd_library.file_mut(reverted).unwrap() = HdEditFile::parse(dirty_bytes).unwrap();
    assert!(app.hd().hd_library.is_dirty(reverted));
    let slot = pod_preset::HdFxSlot::new(5).unwrap();
    app.hd_mut().hd_type9_pending = Some(PendingHdType9Choice {
        file_index: unrelated,
        slot,
        model_id,
        choice: pod_preset::HdType9Choice::Choice1,
    });

    assert!(app.hd_mut().discard_next_dirty_hd());
    assert!(app.hd().hd_type10_pending.is_none());
    assert!(
        app.hd()
            .hd_type9_pending
            .is_some_and(|pending| pending.file_index == unrelated)
    );
    assert_eq!(
        app.hd().hd_library.file(reverted).unwrap().as_bytes(),
        bytes
    );
    assert!(!app.hd().hd_library.is_dirty(reverted));
}

#[test]
fn hd_type10_quit_apply_and_keep_editing_resolve_only_the_staged_choice() {
    for model_id in [pod_preset::HD_TYPE10_CHOICE_MODEL_ID, 0x020a_000f] {
        hd_type10_quit_apply_and_keep_editing_for(model_id);
    }
}

fn hd_type10_quit_apply_and_keep_editing_for(model_id: u32) {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let slot_index = 5;
    let key = pod_preset::hd_type10_choice_key(model_id).unwrap();
    let bytes = sample_hd_type10_preset_for(slot_index, 8, 0x3eaa_aaab, model_id, key);
    let index = app.hd_mut().hd_library.import_bytes(bytes.clone()).unwrap();
    app.hd_mut().select_hd_file(index);
    app.select_test_editor::<HdEditor>();
    app.hd_mut().hd_effect_slot = slot_index;
    click_hd_type10_choice(&context, &mut app, 1);
    hd_editor_frame(&context, &mut app, vec![]);

    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitHd)));
    click_prompt(&context, &mut app, "cancel");
    assert!(
        app.test_pending().is_none(),
        "Keep editing dismisses the quit prompt"
    );
    assert!(
        app.hd().hd_type10_pending.is_some(),
        "Keep editing retains the stage"
    );
    assert!(!app.permit_close);

    app.request_close(&context);
    click_prompt(&context, &mut app, "apply-type10");
    assert!(app.hd().hd_type10_pending.is_none());
    assert!(app.hd().hd_library.is_dirty(index));
    assert!(
        !app.permit_close,
        "applied edit still requires save or discard"
    );
    let value_at = pod_preset::H5EP_HEADER_BYTES + 0x428 + slot_index * 0x100 + 0x10 + 8 * 0x14 + 4;
    assert_eq!(
        &app.hd().hd_library.file(index).unwrap().as_bytes()[value_at..value_at + 4],
        &[0x3f, 0x80, 0, 0]
    );
}

#[test]
fn hd_type10_gui_refuses_nonfinite_and_unavailable_rows() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    app.select_test_editor::<HdEditor>();
    for model_id in [pod_preset::HD_TYPE10_CHOICE_MODEL_ID, 0x020a_000f] {
        let key = pod_preset::hd_type10_choice_key(model_id).unwrap();
        for (bits, invalid_key) in [(0x7fc0_0001u32, false), (0x3f40_0000, true)] {
            let mut bytes = sample_hd_type10_preset_for(3, 4, bits, model_id, key);
            if invalid_key {
                let row = pod_preset::H5EP_HEADER_BYTES + 0x428 + 3 * 0x100 + 0x10 + 4 * 0x14;
                bytes[row..row + 4].fill(0);
            }
            let index = app.hd_mut().hd_library.import_bytes(bytes.clone()).unwrap();
            app.hd_mut().select_hd_file(index);
            app.hd_mut().hd_effect_slot = 3;
            hd_editor_frame(&context, &mut app, vec![]);
            click_hd_type10_choice(&context, &mut app, 1);
            assert!(app.hd().hd_type10_pending.is_none());
            assert_hd_bytes_eq(
                app.hd().hd_library.file(index).unwrap().as_bytes(),
                &bytes,
                "uneditable type-10 value stays untouched",
            );
        }
    }
}

#[test]
fn hd_type8_gui_does_not_stage_nonfinite_or_unavailable_values() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    app.select_test_editor::<HdEditor>();
    for (bits, invalid_marker) in [(0x7fc0_0001u32, false), (0x3f40_0000, true)] {
        let mut bytes = sample_hd_type8_preset(3, 4, bits);
        if invalid_marker {
            let marker = pod_preset::H5EP_HEADER_BYTES + 0x428 + 3 * 0x100 + 0x10 + 4 * 0x14 + 0x10;
            bytes[marker] = 1;
        }
        let index = app.hd_mut().hd_library.import_bytes(bytes.clone()).unwrap();
        app.hd_mut().select_hd_file(index);
        app.hd_mut().hd_effect_slot = 3;
        hd_editor_frame(&context, &mut app, vec![]);
        click_hd_type8_choice(&context, &mut app, 1);
        assert!(app.hd().hd_type8_pending.is_none());
        assert!(app.hd().hd_type9_pending.is_none());
        assert_hd_bytes_eq(
            app.hd().hd_library.file(index).unwrap().as_bytes(),
            &bytes,
            "non-finite or unavailable input cannot stage a choice",
        );
    }
    assert!(app.variax().read_worker.is_none());
}

#[test]
fn second_type10_association_gui_stage_apply_and_discard() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let slot = 5;
    let row = 8;
    let original = sample_hd_type10_preset_for(
        slot,
        row,
        0x3eaa_aaab,
        0x020a_000f,
        pod_preset::HD_TYPE10_CHOICE_KEY,
    );
    let index = app
        .hd_mut()
        .hd_library
        .import_bytes(original.clone())
        .unwrap();
    app.hd_mut().select_hd_file(index);
    app.select_test_editor::<HdEditor>();
    app.hd_mut().hd_effect_slot = slot;
    click_hd_type10_choice(&context, &mut app, 1);
    hd_editor_frame(&context, &mut app, vec![]);
    assert!(app.hd().hd_type10_pending.is_some());
    assert_eq!(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        original
    );
    click_hd_editor(&context, &mut app, "discard-hd-type10-choice");
    assert!(app.hd().hd_type10_pending.is_none());
    assert_eq!(
        app.hd().hd_library.file(index).unwrap().as_bytes(),
        original
    );

    click_hd_type10_choice(&context, &mut app, 1);
    hd_editor_frame(&context, &mut app, vec![]);
    click_hd_editor(&context, &mut app, "apply-hd-type10-choice");
    let changed = app.hd().hd_library.file(index).unwrap().as_bytes();
    let value_at = pod_preset::H5EP_HEADER_BYTES + 0x428 + slot * 0x100 + 0x10 + row * 0x14 + 4;
    assert_eq!(&changed[value_at..value_at + 4], &[0x3f, 0x80, 0, 0]);
    assert!(app.hd().hd_type10_pending.is_none());
    assert!(app.variax().read_worker.is_none());
}

#[test]
fn session_log_records_actual_hd_button_switch_and_drag_without_repaint_events() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let temp = tempfile::tempdir().unwrap();
    let (log, guard) =
        session_log::SessionLog::open(Some(temp.path().join("logs")), session_log::Severity::Trace);
    app.hd_mut().session_log = Some(log.clone());
    app.dialogs = Box::new(FakeDialogs {
        pod_open: [None].into(),
        ..Default::default()
    });
    click_hd_editor(&context, &mut app, "load-hd-file");

    let mut preset = sample_hd_editor_preset();
    let block = pod_preset::H5EP_HEADER_BYTES + 0x428;
    preset[block + 0x0b] = 1;
    preset[block + 8] = 0;
    // Keep the same controls visible as the drag moves through FX slots.
    for slot in 1..8 {
        preset.copy_within(block..block + 0x100, block + slot * 0x100);
    }
    let index = app.hd_mut().hd_library.import_bytes(preset).unwrap();
    app.hd_mut().select_hd_file(index);
    click_hd_editor(&context, &mut app, "enable-hd-effect");

    scroll_hd_editor_control_into_view(&context, &app, "hd-effect-slot");
    hd_editor_frame(&context, &mut app, vec![]);
    let rect = app.hd().hd_buttons["hd-effect-slot"];
    let pos = |x| Pos2::new(rect.left() + x, rect.center().y);
    let pointer = |x, pressed| egui::Event::PointerButton {
        pos: pos(x),
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    hd_editor_frame(
        &context,
        &mut app,
        vec![egui::Event::PointerMoved(pos(10.0)), pointer(10.0, true)],
    );
    for x in [32.0, 59.0, 92.0] {
        hd_editor_frame(&context, &mut app, vec![egui::Event::PointerMoved(pos(x))]);
        hd_editor_frame(&context, &mut app, vec![]);
    }
    hd_editor_frame(&context, &mut app, vec![pointer(92.0, false)]);
    hd_editor_frame(&context, &mut app, vec![]);
    drop(guard);

    let text = std::fs::read_to_string(log.status().path.unwrap()).unwrap();
    let ui: Vec<serde_json::Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .filter(|row: &serde_json::Value| row["category"] == "ui")
        .collect();
    assert_eq!(ui[0]["control"], "hd_open_file");
    assert_eq!(ui[0]["operation"], ui[1]["operation"]);
    assert_eq!(ui[1]["disposition"], "cancelled");
    assert_eq!(ui[2]["control"], "hd_effect_enabled");
    assert_eq!(ui[2]["operation"], ui[3]["operation"]);
    assert_eq!(
        ui[3]["action"],
        serde_json::json!({"kind":"switch","old":false,"new":true})
    );
    assert_eq!(ui[3]["disposition"], "applied");
    let dial = &ui[4..];
    assert_eq!(
        dial.len(),
        3,
        "expected one event for each changed frame: {dial:?}"
    );
    assert!(
        dial.iter()
            .all(|row| row["control"] == "hd_effect_slot"
                && row["operation"] == dial[0]["operation"])
    );
    assert!(
        dial.windows(2)
            .all(|pair| pair[0]["action"]["new"] == pair[1]["action"]["old"])
    );
    assert_eq!(
        dial.last().unwrap()["action"]["new"],
        app.hd().hd_effect_slot
    );
}
