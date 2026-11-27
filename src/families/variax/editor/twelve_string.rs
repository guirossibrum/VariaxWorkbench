use super::{
    Color32, Rect, TUNING, VariaxEditor, egui,
    guitar::STRINGS,
    theme,
    tuning_presets::{DETUNE, MIX, set_tuning_values},
    value_format::ValueFormat,
};
use variax_workbench::ui::{editor_layout::TWELVE_STRING_COLUMN, tuning_panel::TuningOutput};

#[derive(Clone, Copy)]
enum Control {
    Mix,
    Detune,
}

impl Control {
    fn label(self) -> &'static str {
        match self {
            Self::Mix => "Mix",
            Self::Detune => "Detune",
        }
    }

    fn entry(self) -> ValueFormat {
        match self {
            Self::Mix => ValueFormat::Percent(1.0),
            Self::Detune => ValueFormat::CENTS,
        }
    }

    fn caption(self, value: f64) -> String {
        match self {
            Self::Mix => self.entry().display(value),
            Self::Detune if value.round() == 0.0 => "0 cents".into(),
            Self::Detune => format!("{value:+.0} cents"),
        }
    }
}

impl VariaxEditor {
    pub(super) fn twelve_string_panel(
        &mut self,
        ui: &mut egui::Ui,
        left: f32,
        tuning: &TuningOutput,
        editable: bool,
        complete_mix: bool,
    ) {
        let panel = Rect::from_min_max(
            egui::pos2(left, tuning.enable.top() - 8.0),
            egui::pos2(
                left + TWELVE_STRING_COLUMN,
                tuning.lock.bottom().max(tuning.reset.bottom()) + 8.0,
            ),
        );
        let painter = ui.painter().clone();
        painter.rect(
            panel,
            theme::RADIUS_PANEL,
            theme::PANEL,
            egui::Stroke::new(1.0_f32, theme::LINE),
            egui::StrokeKind::Inside,
        );
        let columns = [
            (Control::Mix, MIX, left + 10.0, 80.0),
            (Control::Detune, DETUNE, left + 98.0, 96.0),
        ];
        let first_y = tuning.rows.first().map_or(panel.center().y, |row| row.y);
        for (control, base, x, width) in columns {
            painter.text(
                egui::pos2(x, first_y - 29.0),
                egui::Align2::LEFT_CENTER,
                control.label(),
                egui::FontId::proportional(theme::COMPACT),
                theme::MUTED,
            );
            for row in &tuning.rows {
                let id = base + row.id as u32;
                let cell =
                    Rect::from_min_size(egui::pos2(x, row.y - 12.0), egui::vec2(width, 24.0));
                self.twelve_string_knob(
                    ui,
                    cell,
                    control,
                    id,
                    editable && (base != MIX || !self.link_tuning_mix || complete_mix),
                );
            }
        }
        let mut controls = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("reset-detune")
                .max_rect(panel),
        );
        let reset = controls
            .add_enabled_ui(
                editable && (DETUNE..DETUNE + 6).all(|id| self.tuning_parameter(id, 1)),
                |ui| {
                    ui.put(
                        Rect::from_center_size(
                            egui::pos2(panel.center().x, tuning.reset.center().y),
                            egui::vec2(TWELVE_STRING_COLUMN - 16.0, 22.0),
                        ),
                        egui::Button::new("Zero detune").small(),
                    )
                },
            )
            .inner
            .on_hover_text("Reset all 12-string detune amounts to zero");
        if reset.gained_focus() && !ui.clip_rect().contains_rect(reset.rect) {
            reset.scroll_to_me_animation(
                Some(egui::Align::Center),
                egui::style::ScrollAnimation::none(),
            );
        }
        #[cfg(test)]
        self.tuning_widgets.insert("Zero detune", reset.rect);
        if reset.clicked() {
            self.change(|preset| set_tuning_values(preset, DETUNE, 1, [0; 6]));
        }
    }

    fn twelve_string_knob(
        &mut self,
        ui: &mut egui::Ui,
        cell: Rect,
        control: Control,
        id: u32,
        editable: bool,
    ) {
        let mut ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("twelve-string-knob", id))
                .max_rect(cell),
        );
        let Some(stored) = self
            .float(TUNING, id)
            .filter(|value| value.is_finite())
            .map(f64::from)
        else {
            ui.painter().text(
                cell.left_center(),
                egui::Align2::LEFT_CENTER,
                "—",
                egui::FontId::proportional(theme::COMPACT),
                theme::DIM,
            );
            return;
        };
        let (min, max, speed, base) = match control {
            Control::Mix => (0.0, 1.0, 0.01, MIX),
            Control::Detune => (-100.0, 100.0, 1.0, DETUNE),
        };
        let editable = editable && self.tuning_parameter(id, 1);
        let editing = editable && ui.memory(|memory| memory.has_focus(ui.next_auto_id()));
        if !editing {
            ui.visuals_mut().override_text_color = Some(Color32::TRANSPARENT);
            let widgets = &mut ui.visuals_mut().widgets;
            for state in [
                &mut widgets.inactive,
                &mut widgets.hovered,
                &mut widgets.active,
                &mut widgets.noninteractive,
            ] {
                state.weak_bg_fill = Color32::TRANSPARENT;
                state.bg_fill = Color32::TRANSPARENT;
                state.bg_stroke = egui::Stroke::NONE;
            }
        }
        ui.spacing_mut().interact_size = cell.size();
        let mut value = stored;
        let format = control.entry();
        let response = ui
            .add_enabled(
                editable,
                egui::DragValue::new(&mut value)
                    .range(min..=max)
                    .speed(speed)
                    .clamp_existing_to_range(false)
                    .custom_formatter(move |value, _| format.display(value))
                    .custom_parser(move |text| format.parse_edit(text, stored))
                    .update_while_editing(false),
            )
            .on_hover_text(format!(
                "{} {}",
                STRINGS[(id - base) as usize].0,
                control.label()
            ));
        if response.gained_focus() && !ui.clip_rect().contains_rect(response.rect) {
            response.scroll_to_me_animation(
                Some(egui::Align::Center),
                egui::style::ScrollAnimation::none(),
            );
        }
        if !editing {
            let center = cell.left_center() + egui::vec2(11.0, 0.0);
            let painter = ui.painter();
            let edge = if editable && response.hovered() {
                theme::FOCUS
            } else {
                theme::LINE
            };
            painter.circle(
                center,
                10.0,
                theme::RAISED,
                egui::Stroke::new(1.0_f32, edge),
            );
            let angle = egui::lerp(
                -2.4..=2.4,
                ((value - min) / (max - min)).clamp(0.0, 1.0) as f32,
            );
            let tip = egui::vec2(angle.sin(), -angle.cos());
            painter.line_segment(
                [center + tip * 3.0, center + tip * 8.0],
                egui::Stroke::new(1.5_f32, if editable { theme::AMBER } else { theme::DIM }),
            );
            painter.text(
                cell.left_center() + egui::vec2(25.0, 0.0),
                egui::Align2::LEFT_CENTER,
                control.caption(value),
                egui::FontId::proportional(theme::COMPACT),
                theme::MUTED,
            );
        }
        #[cfg(test)]
        if id == MIX || id == DETUNE {
            self.tuning_widgets.insert(
                if id == MIX {
                    "Low E mix"
                } else {
                    "Low E detune"
                },
                response.rect,
            );
        }
        if response.drag_started()
            && let Some(document) = &mut self.document
        {
            document.begin_gesture();
        }
        if editable && value != stored {
            let linked = base == MIX && self.link_tuning_mix;
            let bits = (value as f32).to_bits();
            self.change(|preset| {
                if linked {
                    set_tuning_values(preset, MIX, 1, [bits; 6])
                } else {
                    preset.set_parameter(TUNING, id, 1, bits)
                }
            });
        }
        if response.drag_stopped()
            && let Some(document) = &mut self.document
        {
            document.end_gesture();
        }
    }
}
