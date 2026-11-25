use super::*;

#[test]
fn chain_respects_pre_post_without_moving_eq_or_comp() {
    assert_eq!(
        ordered_blocks(|_| false),
        [
            Block::Gate,
            Block::Wah,
            Block::Stomp,
            Block::Amp,
            Block::Comp,
            Block::Eq,
            Block::Vol,
            Block::Mod,
            Block::Delay,
            Block::Reverb
        ]
    );
    assert_eq!(
        ordered_blocks(|block| block == Block::Delay),
        [
            Block::Gate,
            Block::Wah,
            Block::Stomp,
            Block::Delay,
            Block::Amp,
            Block::Comp,
            Block::Eq,
            Block::Vol,
            Block::Mod,
            Block::Reverb
        ]
    );
    assert_eq!(
        ordered_blocks(|_| true),
        [
            Block::Gate,
            Block::Wah,
            Block::Stomp,
            Block::Vol,
            Block::Mod,
            Block::Delay,
            Block::Reverb,
            Block::Amp,
            Block::Comp,
            Block::Eq
        ]
    );
}

#[test]
fn routing_edit_moves_the_file_slot_and_can_be_undone() {
    let mut editor = PodxtEditor::new();
    let tone = PodTone::new_podxt_tone("Pre/Post test").unwrap();
    editor.import_individual_pod_tone(tone.into_bytes());
    let rows = editor.pod_library.tone(0).unwrap().table_controls(0);
    let route = get(&rows, "DELAY_PREPOST").unwrap();
    assert_eq!(route.bits, Some(1));

    editor.edit_stage_value(0, route, 0);
    assert_eq!(
        editor
            .pod_library
            .tone(0)
            .unwrap()
            .table_tone_field("DELAY_PREPOST"),
        Some(vec![(32 + 87, 0)])
    );
    assert!(editor.pod_dirty());
    editor.table_history(false);
    assert_eq!(
        editor
            .pod_library
            .tone(0)
            .unwrap()
            .table_tone_field("DELAY_PREPOST"),
        Some(vec![(32 + 87, 127)])
    );
    assert!(!editor.pod_dirty());
}

#[test]
fn selected_editor_draws_amp_or_four_eq_bands() {
    fn text_from(shape: &egui::epaint::Shape, text: &mut String) {
        match shape {
            egui::epaint::Shape::Text(item) => {
                text.push_str(item.galley.text());
                text.push('\n');
            }
            egui::epaint::Shape::Vec(items) => {
                items.iter().for_each(|shape| text_from(shape, text))
            }
            _ => {}
        }
    }

    let context = egui::Context::default();
    let mut editor = PodxtEditor::new();
    let tone = PodTone::new_podxt_tone("Four Band test").unwrap();
    let rows = tone.table_controls(0);
    let mut render = |block, width| {
        editor.stage.block = block;
        let output = context.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 768.0))),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default()
                    .show(ctx, |ui| editor.stage_editor(ui, 0, &tone, &rows));
            },
        );
        let mut text = String::new();
        output
            .shapes
            .iter()
            .for_each(|shape| text_from(&shape.shape, &mut text));
        text
    };
    let amp = render(Block::Amp, 1024.0);
    assert!(
        amp.contains("Amp ·") && amp.contains("Cab") && amp.contains('%'),
        "{amp}"
    );
    assert!(!amp.contains("Band 1"));
    let eq = render(Block::Eq, 628.0);
    for band in 1..=4 {
        assert!(eq.contains(&format!("Band {band}")), "{eq}");
    }
    assert!(eq.contains("Hz") && eq.contains("dB"));
    assert_eq!(eq.lines().filter(|&label| label == "Frequency").count(), 4);
    assert_eq!(eq.lines().filter(|&label| label == "Gain").count(), 4);
    assert!(!eq.contains("Band 5") && !eq.contains("Amp model"));
}

#[cfg(feature = "usb")]
#[test]
fn stage_targets_use_verified_live_encoding_and_exclude_variax() {
    use line6_devices::family::podxt::{LiveTarget, Setting};
    let tone = PodTone::new_podxt_tone("Live encoding").unwrap();
    let rows = tone.table_controls(0);
    let delay = get(&rows, "DELAY_TIME").unwrap();
    let bits = (1.75_f32 / 16383.0).to_bits();
    assert_eq!(
        delay.live_target(bits),
        Some(LiveTarget::Additional(Setting::DelayTime, 2))
    );
    assert!(
        get(&rows, "VARIAX_FAMILY")
            .unwrap()
            .live_target(1)
            .is_none()
    );
}

#[test]
fn undo_and_redo_follow_the_selected_bank_position() {
    let mut editor = PodxtEditor::new();
    let a = PodTone::new_podxt_tone("First").unwrap();
    let b = PodTone::new_podxt_tone("Second").unwrap();
    let bundle = pod_preset::pod_bundle_from_tones(false, &[("Set", vec![a, b])]).unwrap();
    editor.open_stage_document(bundle.as_bytes().to_vec());
    let edit_name = |tone: &mut PodTone, name| {
        tone.rename(name)?;
        Ok(PodModelChange::default())
    };
    editor
        .table_edit(0, |tone| edit_name(tone, "One edited"))
        .unwrap();
    editor
        .table_edit(1, |tone| edit_name(tone, "Two edited"))
        .unwrap();
    assert!(editor.pod_library.is_dirty(0) && editor.pod_library.is_dirty(1));

    editor.select_pod_index(0);
    editor.table_history(false);
    assert_eq!(editor.pod_selected, Some(0));
    assert_eq!(editor.pod_library.tone(0).unwrap().name(), "First");
    assert!(!editor.pod_library.is_dirty(0));
    assert_eq!(editor.pod_library.tone(1).unwrap().name(), "Two edited");
    editor.select_pod_index(1);
    editor
        .table_edit(1, |tone| edit_name(tone, "Two again"))
        .unwrap();
    editor.select_pod_index(0);
    editor.table_history(true);
    assert_eq!(editor.pod_library.tone(0).unwrap().name(), "One edited");
    assert_eq!(editor.pod_library.tone(1).unwrap().name(), "Two again");
}

#[test]
fn opening_replaces_one_document_only_after_dirty_edits_are_resolved() {
    let mut editor = PodxtEditor::new();
    let tone = |name| PodTone::new_podxt_tone(name).unwrap().into_bytes();
    editor.open_stage_document(tone("First"));
    editor
        .table_edit(0, |tone| {
            tone.rename("Editing")?;
            Ok(PodModelChange::default())
        })
        .unwrap();

    editor.open_stage_document(tone("Second"));
    assert_eq!(editor.pod_library.tone(0).unwrap().name(), "Editing");
    assert!(editor.stage_has_pending_document());
    let candidate = editor.stage.pending_document.take().unwrap();
    editor.adopt_stage_document(candidate);
    assert_eq!(editor.pod_library.len(), 1);
    assert_eq!(editor.pod_library.tone(0).unwrap().name(), "Second");
    assert!(!editor.pod_dirty());
    assert!(!editor.parameter_panel.can_history(Some(0), false));
}
