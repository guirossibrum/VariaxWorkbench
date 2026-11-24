//! Vertical drag knob. Shift drags in finer steps. Double-click or Enter
//! edits the value as text, which is the approved specification.

use std::ops::RangeInclusive;

use eframe::egui::text::{CCursor, CCursorRange};
use eframe::egui::{self, Key, RichText, Sense, Vec2, Widget, vec2};

use super::theme::{self, AMBER, DIM, FOCUS, LINE, MUTED, TEXT};

const DIAL: f32 = 64.0;
const COARSE_PIXELS: f32 = 120.0;
const FINE_PIXELS: f32 = 400.0;

#[derive(Clone)]
struct EditState {
    text: String,
    focus: bool,
}

/// A round dial with the value and unit under it.
pub struct Knob<'a> {
    label: &'a str,
    value: &'a mut f64,
    range: RangeInclusive<f64>,
    decimals: usize,
    suffix: &'a str,
    enabled: bool,
}

impl<'a> Knob<'a> {
    pub fn percent(label: &'a str, value: &'a mut f64) -> Self {
        Self {
            label,
            value,
            range: 0.0..=100.0,
            decimals: 0,
            suffix: "%",
            enabled: true,
        }
    }

    pub fn range(mut self, range: RangeInclusive<f64>) -> Self {
        self.range = range;
        self
    }

    pub fn decimals(mut self, decimals: usize) -> Self {
        self.decimals = decimals;
        self
    }

    pub fn suffix(mut self, suffix: &'a str) -> Self {
        self.suffix = suffix;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

pub struct KnobOutput {
    pub response: egui::Response,
    pub changed: bool,
}

impl Widget for Knob<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        self.show(ui).response
    }
}

impl Knob<'_> {
    pub fn show(self, ui: &mut egui::Ui) -> KnobOutput {
        let Knob {
            label,
            value,
            range,
            decimals,
            suffix,
            enabled,
        } = self;
        let id = ui.make_persistent_id(("knob", label));
        let mut changed = false;
        let laid_out = ui.vertical(|ui| {
            ui.set_min_width(DIAL + theme::SPACE_8);
            let sense = if enabled {
                Sense::click_and_drag()
            } else {
                Sense::hover()
            };
            let (rect, response) = ui.allocate_exact_size(Vec2::splat(DIAL), sense);
            if enabled {
                if response.clicked() {
                    response.request_focus();
                }
                changed |= apply_drag(ui, &response, value, &range);
                changed |= edit_value(ui, id, &response, value, &range, decimals, suffix);
            }
            paint_dial(ui, rect, &response, *value, &range, enabled);
            ui.label(
                RichText::new(format_value(*value, decimals, suffix))
                    .size(theme::COMPACT)
                    .strong()
                    .color(if enabled { TEXT } else { MUTED }),
            );
            ui.label(
                RichText::new(label)
                    .size(theme::KNOB_LABEL)
                    .color(if enabled { MUTED } else { DIM }),
            );
            KnobOutput { response, changed }
        });
        laid_out.inner
    }
}

fn apply_drag(
    ui: &egui::Ui,
    response: &egui::Response,
    value: &mut f64,
    range: &RangeInclusive<f64>,
) -> bool {
    if !response.dragged() {
        return false;
    }
    let span = range.end() - range.start();
    let fine = ui.input(|input| input.modifiers.shift);
    let pixels = f64::from(if fine { FINE_PIXELS } else { COARSE_PIXELS });
    let delta = f64::from(response.drag_delta().y);
    let next = (*value - delta * span / pixels).clamp(*range.start(), *range.end());
    if (next - *value).abs() < f64::EPSILON {
        return false;
    }
    *value = next;
    true
}

fn edit_value(
    ui: &mut egui::Ui,
    id: egui::Id,
    response: &egui::Response,
    value: &mut f64,
    range: &RangeInclusive<f64>,
    decimals: usize,
    suffix: &str,
) -> bool {
    let editing = ui.data(|data| data.get_temp::<EditState>(id));
    let keyboard_open = editing.is_none()
        && response.has_focus()
        && ui.input(|input| input.key_pressed(Key::Enter));
    let opening = keyboard_open || (editing.is_none() && response.double_clicked());
    if opening {
        ui.data_mut(|data| {
            data.insert_temp(
                id,
                EditState {
                    text: format_value(*value, decimals, suffix),
                    focus: true,
                },
            );
        });
    }
    let Some(mut state) = ui.data(|data| data.get_temp::<EditState>(id)) else {
        return false;
    };
    // Escape is read before the field, which would otherwise consume it.
    let cancel = ui.input(|input| input.key_pressed(Key::Escape));
    if cancel {
        ui.data_mut(|data| {
            data.remove::<EditState>(id);
        });
        return false;
    }
    if keyboard_open {
        // The Enter that opened the field must not also submit it.
        ui.input_mut(|input| {
            input.consume_key(egui::Modifiers::NONE, Key::Enter);
        });
    }
    let mut edit = egui::TextEdit::singleline(&mut state.text)
        .desired_width(DIAL)
        .id(id.with("text"))
        .show(ui);
    if state.focus {
        edit.response.request_focus();
        let end = CCursor::new(state.text.chars().count());
        edit.state
            .cursor
            .set_char_range(Some(CCursorRange::two(CCursor::new(0), end)));
        edit.state.store(ui.ctx(), edit.response.id);
        state.focus = false;
    }
    let enter = edit.response.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter));
    ui.data_mut(|data| data.insert_temp(id, state.clone()));
    // The Enter or double-click that opened the field must not also commit it.
    if opening || !enter {
        return false;
    }
    let parsed = parse_value(&state.text, suffix);
    ui.data_mut(|data| {
        data.remove::<EditState>(id);
    });
    let Some(parsed) = parsed else {
        return false;
    };
    let next = parsed.clamp(*range.start(), *range.end());
    if (next - *value).abs() < f64::EPSILON {
        return false;
    }
    *value = next;
    true
}

fn paint_dial(
    ui: &egui::Ui,
    rect: egui::Rect,
    response: &egui::Response,
    value: f64,
    range: &RangeInclusive<f64>,
    enabled: bool,
) {
    let painter = ui.painter();
    let center = rect.center();
    let radius = rect.width() * 0.5 - 2.0;
    let fill = if enabled {
        egui::Color32::from_rgb(0x3a, 0x34, 0x2c)
    } else {
        LINE
    };
    painter.circle_filled(center, radius, fill);
    painter.circle_stroke(
        center,
        radius,
        egui::Stroke::new(2.0_f32, if enabled { theme::RAISED } else { LINE }),
    );
    if response.has_focus() && enabled {
        painter.circle_stroke(
            center,
            radius + 3.0,
            egui::Stroke::new(theme::FOCUS_RING, FOCUS),
        );
    }
    let span = (range.end() - range.start()).max(f64::EPSILON);
    let unit = ((value - range.start()) / span).clamp(0.0, 1.0);
    let angle = (-140.0_f32 + (unit as f32) * 280.0).to_radians();
    let tick = if enabled && response.hovered() {
        FOCUS
    } else if enabled {
        AMBER
    } else {
        DIM
    };
    let tip = center
        + vec2(
            angle.sin() * (radius - 10.0),
            -angle.cos() * (radius - 10.0),
        );
    let origin = center + vec2(angle.sin() * 8.0, -angle.cos() * 8.0);
    painter.line_segment([origin, tip], egui::Stroke::new(3.0_f32, tick));
}

pub fn format_value(value: f64, decimals: usize, suffix: &str) -> String {
    let number = match decimals {
        0 => format!("{value:.0}"),
        1 => format!("{value:.1}"),
        2 => format!("{value:.2}"),
        _ => format!("{value:.3}"),
    };
    format!("{number}{suffix}")
}

fn parse_value(text: &str, suffix: &str) -> Option<f64> {
    let trimmed = text.trim();
    let trimmed = trimmed
        .strip_suffix(suffix)
        .unwrap_or(trimmed)
        .trim()
        .trim_end_matches('%')
        .trim();
    trimmed.parse().ok()
}
