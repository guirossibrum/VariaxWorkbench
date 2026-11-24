//! Every shared control, so a headless test can see the component names.
//!
//! The window is the example binary `variax-workbench-ui-gallery`. A hidden
//! shell tab would edit `src/app/shell`, which this library does not do.

use super::assets::UiImage;
use super::chrome::{self, DialogButton, DocumentTab, Family, LinkState};
use super::controls::{self, Selector, Switch};
use super::knob::Knob;
use super::theme::{self, BODY, SECTION, TEXT};
use eframe::egui::{self, RichText};

pub struct GalleryState {
    pub drive: f64,
    pub switch_on: bool,
    pub model: Option<usize>,
    pub block_on: bool,
    pub dialog_open: bool,
    pub wide: UiImage,
    pub stomp: UiImage,
}

impl Default for GalleryState {
    fn default() -> Self {
        let mut wide = UiImage::missing("pod-farm/sine-chorus.png");
        wide.pixels = [800, 210];
        let mut stomp = UiImage::missing("pod-farm/vetta-wah.png");
        stomp.pixels = [174, 171];
        Self {
            drive: 50.0,
            switch_on: true,
            model: None,
            block_on: true,
            dialog_open: false,
            wide,
            stomp,
        }
    }
}

impl GalleryState {
    pub fn show(&mut self, ctx: &egui::Context) {
        theme::apply(ctx);
        egui::CentralPanel::default().show(ctx, |ui| self.page(ui));
        let _ = chrome::dialog(
            ctx,
            &mut self.dialog_open,
            "Unsaved changes",
            "The open file stays as it is.",
            &[
                DialogButton {
                    label: "Discard",
                    safe: false,
                },
                DialogButton {
                    label: "Keep editing",
                    safe: true,
                },
            ],
        );
    }

    fn page(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                caption(ui, "Knob");
                let _ = Knob::percent("Drive", &mut self.drive).show(ui);
                caption(ui, "Switch");
                ui.add(Switch::new(&mut self.switch_on));
                caption(ui, "Selector");
                ui.add(Selector::new(
                    ui.id().with("gallery-amp"),
                    &mut self.model,
                    &["Blackface Lux", "Tweed"],
                    "No amp model",
                ));
                caption(ui, "LED");
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(theme::TARGET, theme::TARGET),
                    egui::Sense::hover(),
                );
                controls::paint_led(ui.painter(), rect.center(), self.switch_on, true);
                caption(ui, "Display");
                controls::readout(ui, "120 BPM");
            });
            ui.vertical(|ui| {
                caption(ui, "Block");
                let _ = chrome::ChainBlock::new("Mod", &mut self.block_on)
                    .selected(true)
                    .image(&self.wide)
                    .show(ui);
                let mut stomp_on = true;
                let _ = chrome::ChainBlock::new("Wah", &mut stomp_on)
                    .image(&self.stomp)
                    .show(ui);
            });
            ui.vertical(|ui| {
                caption(ui, "Tab");
                let _ = DocumentTab {
                    title: &LinkState::IdentityPending.tab_title("PODxt Live"),
                    family: Family::Podxt,
                    active: true,
                    unsaved: true,
                }
                .show(ui);
                caption(ui, "Toolbar");
                let _ = chrome::toolbar_button(ui, "Open file…", true, "");
                let _ = chrome::toolbar_button(
                    ui,
                    "Detect devices",
                    false,
                    chrome::DETECTION_UNAVAILABLE,
                );
                caption(ui, "Status");
                chrome::status_chip(ui, LinkState::NoDevice);
                caption(ui, "Dialog");
                if ui
                    .add(
                        egui::Button::new(RichText::new("Open dialog").size(BODY))
                            .min_size(egui::vec2(theme::TARGET, theme::TARGET)),
                    )
                    .clicked()
                {
                    self.dialog_open = true;
                }
                caption(ui, "Editor");
                let _ = chrome::section_panel(ui, "Amp", |ui| {
                    ui.label(RichText::new("Blackface Lux").size(BODY).color(TEXT));
                });
            });
        });
    }
}

fn caption(ui: &mut egui::Ui, name: &str) {
    ui.label(RichText::new(name).size(SECTION).strong().color(TEXT));
}
