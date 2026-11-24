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
    let mut render = |block| {
        editor.stage.block = block;
        let output = context.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1024.0, 768.0))),
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
    let amp = render(Block::Amp);
    assert!(
        amp.contains("Amp ·") && amp.contains("Cab") && amp.contains('%'),
        "{amp}"
    );
    assert!(!amp.contains("Band 1"));
    let eq = render(Block::Eq);
    for band in 1..=4 {
        assert!(eq.contains(&format!("Band {band}")), "{eq}");
    }
    assert!(eq.contains("Hz") && eq.contains("dB"));
    assert!(!eq.contains("Band 5") && !eq.contains("Amp model"));
}
