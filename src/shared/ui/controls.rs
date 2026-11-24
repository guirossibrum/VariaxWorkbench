//! Switch, selector, LED, and one-line readout.

use eframe::egui::{self, RichText, Sense, Widget};

use super::theme::{self, AMBER, DIM, LINE, MUTED, TEXT};

/// LED plus the words On or Off. Colour is not the only signal.
pub struct Switch<'a> {
    on: &'a mut bool,
    enabled: bool,
}

impl<'a> Switch<'a> {
    pub fn new(on: &'a mut bool) -> Self {
        Self { on, enabled: true }
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

impl Widget for Switch<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let enabled = self.enabled;
        let response = ui.add_enabled_ui(enabled, |ui| {
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(72.0, theme::TARGET), Sense::click());
            if response.clicked() {
                *self.on = !*self.on;
            }
            let painter = ui.painter();
            let lamp = rect.left_center() + egui::vec2(12.0, 0.0);
            paint_led(painter, lamp, *self.on, enabled);
            painter.text(
                lamp + egui::vec2(14.0, 0.0),
                egui::Align2::LEFT_CENTER,
                if *self.on { "On" } else { "Off" },
                egui::FontId::proportional(theme::BODY),
                if enabled { TEXT } else { DIM },
            );
            response
        });
        response.inner
    }
}

pub fn paint_led(painter: &egui::Painter, center: egui::Pos2, on: bool, enabled: bool) {
    let fill = if !enabled {
        LINE
    } else if on {
        AMBER
    } else {
        egui::Color32::from_rgb(0x2a, 0x24, 0x1c)
    };
    painter.circle_filled(center, 4.0, fill);
    painter.circle_stroke(
        center,
        4.0,
        egui::Stroke::new(1.0_f32, egui::Color32::BLACK),
    );
}

/// Raised menu. An empty selection shows `empty` (`No amp model`). A disabled
/// selector keeps the current name and does not open.
pub struct Selector<'a> {
    id: egui::Id,
    selected: &'a mut Option<usize>,
    options: &'a [&'a str],
    empty: &'a str,
    enabled: bool,
}

impl<'a> Selector<'a> {
    pub fn new(
        id: egui::Id,
        selected: &'a mut Option<usize>,
        options: &'a [&'a str],
        empty: &'a str,
    ) -> Self {
        Self {
            id,
            selected,
            options,
            empty,
            enabled: true,
        }
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

impl Widget for Selector<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let label = self
            .selected
            .and_then(|index| self.options.get(index).copied())
            .unwrap_or(self.empty);
        if !self.enabled {
            return ui
                .add_enabled(
                    false,
                    egui::Button::new(RichText::new(label).size(theme::BODY))
                        .min_size(egui::vec2(theme::TARGET, theme::TARGET)),
                )
                .on_disabled_hover_text(label);
        }
        let combo = egui::ComboBox::from_id_salt(self.id)
            .selected_text(RichText::new(label).size(theme::BODY))
            .width(180.0)
            .show_ui(ui, |ui| {
                for (index, option) in self.options.iter().enumerate() {
                    ui.selectable_value(self.selected, Some(index), *option);
                }
            });
        combo.response
    }
}

/// Read-only one-line value. It does not draw a meter.
pub fn readout(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.label(RichText::new(text).size(theme::STATUS).color(MUTED))
}
