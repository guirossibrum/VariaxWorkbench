use super::*;

impl VariaxEditor {
    pub(super) fn electrical_controls(&mut self, ui: &mut egui::Ui) {
        let resistances = [
            (25.0, "25k Ω"),
            (50.0, "50k Ω"),
            (100.0, "100k Ω"),
            (250.0, "250k Ω"),
            (470.0, "470k Ω"),
            (500.0, "500k Ω"),
            (1000.0, "1M Ω"),
        ];
        let body = self.model_id(BODY);
        let sitar = body == Some(0x0310_0018);
        let Some(preset) = self.document.as_ref().map(|document| document.preset()) else {
            return;
        };
        let pickups = [PICKUP1, PICKUP2].map(|slot| {
            preset
                .model(slot)
                .filter(|model| model.enabled)
                .map(|model| model.id)
        });
        let electric = body.is_some_and(|id| variax_workbench::edit::model_allowed(BODY, id))
            && !sitar
            && pickups.iter().any(Option::is_some)
            && pickups
                .into_iter()
                .flatten()
                .all(|id| variax_workbench::edit::model_allowed(PICKUP1, id));
        let spank_bridge = body == Some(0x0310_0003)
            && matches!(
                pickups,
                [Some(0x0311_0004), None] | [None, Some(0x0311_0004)]
            );
        let bypass = preset.has_tone_bypass_marker();
        let active = self
            .float(WIRING, 0x200003)
            .is_some_and(|cap| cap.is_finite() && cap > 0.0);

        ui.strong("Volume knob");
        if electric {
            ui.label("Rolling off volume can darken the sound.");
        }
        ui.columns(2, |columns| {
            self.float_choices(&mut columns[0], 0x200000, "Resistance", &resistances);
            self.taper_control(&mut columns[1]);
        });
        ui.separator();
        ui.strong("Tone knob");
        if sitar {
            ui.label("Changes the level of the drone strings.");
        } else if electric && bypass {
            ui.label("Tone knob bypassed. Choose a positive capacitance to enable it.");
        } else if electric && active {
            ui.label(if spank_bridge {
                "Shapes treble on the Spank bridge pickup."
            } else {
                "Shapes the treble response."
            });
        }
        ui.columns(2, |columns| {
            self.float_choices(&mut columns[0], 0x200002, "Resistance", &resistances);
            self.float_choices(
                &mut columns[1],
                0x200003,
                "Capacitance",
                &[
                    (-0.047, "Bypass tone (47 nF)"),
                    (0.0011, "1.1 nF"),
                    (0.0022, "2.2 nF"),
                    (0.0047, "4.7 nF"),
                    (0.011, "11 nF"),
                    (0.022, "22 nF"),
                    (0.047, "47 nF"),
                    (0.1, "0.1 µF"),
                ],
            );
        });
        self.float_control(
            ui,
            WIRING,
            0x200004,
            if sitar {
                "Drone control position"
            } else {
                "Tone position"
            },
            ValueFormat::Percent(1.0),
        );
        self.boolean_control(ui, WIRING, 0x200005, "Save tone-knob position");
        let mut auto = self.auto_select_controls;
        let response = ui
            .checkbox(
                &mut auto,
                "Auto-select Tone/Volume defaults when selecting pickup types",
            )
            .on_hover_text(
                "Applies resistance and capacitance defaults for the pickup types. A stored tone bypass is preserved.",
            );
        #[cfg(test)]
        {
            self.auto_select_checkbox = Some(response.rect);
        }
        if response.changed() {
            self.set_auto_select_controls(auto);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::tests::app;
    use super::*;

    fn frame(
        context: &egui::Context,
        app: &mut Workbench,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        context.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1440.0, 960.0))),
                events,
                ..Default::default()
            },
            |ctx| app.render(ctx),
        )
    }

    fn text_position(output: &egui::FullOutput, label: &str) -> Option<Pos2> {
        fn find(shape: &egui::Shape, label: &str, clip: Rect) -> Option<Pos2> {
            match shape {
                egui::Shape::Text(text) if text.galley.text() == label => {
                    let pos = text.galley.rect.translate(text.pos.to_vec2()).center();
                    clip.contains(pos).then_some(pos)
                }
                egui::Shape::Vec(shapes) => {
                    shapes.iter().find_map(|shape| find(shape, label, clip))
                }
                _ => None,
            }
        }
        output
            .shapes
            .iter()
            .find_map(|shape| find(&shape.shape, label, shape.clip_rect))
    }

    #[track_caller]
    fn click_text(ctx: &egui::Context, app: &mut Workbench, label: &str) {
        let pos = text_position(&frame(ctx, app, vec![]), label)
            .unwrap_or_else(|| panic!("missing {label}"));
        frame(ctx, app, vec![egui::Event::PointerMoved(pos)]);
        for pressed in [true, false] {
            frame(
                ctx,
                app,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
    }

    #[test]
    fn knob_help_tracks_the_edited_body_pickup_and_wiring() {
        let (ctx, mut app) = app();
        app.variax_mut().tab = Tab::Controls;
        let spank = PresetFile::default_legacy_model().unwrap();
        app.variax_mut().change(|preset| {
            *preset = spank;
            preset.rename("Reso 2")
        });
        let output = frame(&ctx, &mut app, vec![]);
        for label in [
            "Rolling off volume can darken the sound.",
            "Shapes treble on the Spank bridge pickup.",
        ] {
            assert!(text_position(&output, label).is_some(), "missing {label}");
        }

        // The stored bypass also takes effect on a Spank bridge preset.
        app.variax_mut()
            .change(|preset| preset.set_parameter(WIRING, 0x200003, 1, (-0.047_f32).to_bits()));
        let output = frame(&ctx, &mut app, vec![]);
        assert!(
            text_position(
                &output,
                "Tone knob bypassed. Choose a positive capacitance to enable it."
            )
            .is_some()
        );
        assert!(text_position(&output, "Shapes treble on the Spank bridge pickup.").is_none());
        app.variax_mut().undo();

        // Enable the middle pickup, leaving the same body and capacitance.
        app.variax_mut()
            .change(|preset| preset.set_enabled(PICKUP2, true));
        let output = frame(&ctx, &mut app, vec![]);
        assert!(text_position(&output, "Shapes the treble response.").is_some());
        assert!(text_position(&output, "Shapes treble on the Spank bridge pickup.").is_none());

        // The Reso bank also contains the electric Danelectro.
        for (body, expected) in [
            (0x0310_0019, "Shapes the treble response."),
            (0x0310_0017, "Changes the modeled microphone position."),
            (0x0310_001a, "Changes the modeled microphone position."),
            (0x0310_001b, "Changes the modeled microphone position."),
            (0x0310_0018, "Changes the level of the drone strings."),
        ] {
            app.variax_mut()
                .change(|preset| preset.select_model(BODY, body));
            let output = frame(&ctx, &mut app, vec![]);
            assert!(
                text_position(&output, expected).is_some(),
                "missing {expected} on {body:08x}"
            );
        }
    }

    #[test]
    fn explicit_capacitance_choices_change_bypass_and_are_undoable() {
        let (ctx, mut app) = app();
        ctx.style_mut(|style| style.animation_time = 0.0);
        app.variax_mut().tab = Tab::Controls;
        app.variax_mut().change(|preset| {
            *preset = PresetFile::default_legacy_model()?;
            preset.select_model(BODY, 0x0310_0000)?;
            preset.select_model(PICKUP1, 0x0311_0000)?;
            preset.set_parameter(WIRING, 0x200003, 1, (-0.047_f32).to_bits())
        });
        app.variax_mut().select_pickup_model(PICKUP2, 0x0311_8000);
        let before = app.variax().document.as_ref().unwrap().as_bytes().to_vec();
        let bypass = "Tone knob bypassed. Choose a positive capacitance to enable it.";
        assert!(text_position(&frame(&ctx, &mut app, vec![]), bypass).is_some());

        click_text(&ctx, &mut app, "Bypass tone (47 nF)");
        click_text(&ctx, &mut app, "11 nF");
        assert_eq!(app.variax().float(WIRING, 0x200003), Some(0.011));
        let output = frame(&ctx, &mut app, vec![]);
        assert!(text_position(&output, "Shapes the treble response.").is_some());
        assert!(text_position(&output, bypass).is_none());

        click_text(&ctx, &mut app, "11 nF");
        // The selected value is still 11 nF while the popup offers bypass.
        click_text(&ctx, &mut app, "Bypass tone (47 nF)");
        assert_eq!(app.variax().float(WIRING, 0x200003), Some(-0.047));
        app.variax_mut().undo();
        assert_eq!(app.variax().float(WIRING, 0x200003), Some(0.011));
        app.variax_mut().undo();
        assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), before);
    }
}
