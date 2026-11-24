//! Every shared control, so a headless test can see the component names.
//!
//! The window is the example binary `variax-workbench-ui-gallery`. A hidden
//! shell tab would edit `src/app/shell`, which this library does not do.

use super::assets::UiImage;
use super::body_canvas::{BodyCanvas, BodyEvent, PickupSprite};
use super::chrome::{self, DialogButton, DocumentTab, Family, LinkState};
use super::controls::{self, Selector, Switch};
use super::knob::Knob;
use super::theme::{self, BODY, SECTION, TEXT};
use super::tuning_panel::{TuningEvent, TuningPanel, TuningRow};
use eframe::egui::{self, RichText};

pub struct GalleryState {
    pub drive: f64,
    pub switch_on: bool,
    pub model: Option<usize>,
    pub block_on: bool,
    pub dialog_open: bool,
    pub wide: UiImage,
    pub stomp: UiImage,
    pub body: UiImage,
    pub pickup: UiImage,
    pub outline: UiImage,
    pub show_outline: bool,
    pub pickup_position: [f32; 2],
    pub pickup_angle: [f32; 2],
    pub pickup_on: [bool; 2],
    pub selected_pickup: usize,
    pub tuning_on: bool,
    pub shifts: [i32; 6],
    pub tuning_locked: bool,
}

impl Default for GalleryState {
    fn default() -> Self {
        let mut wide = UiImage::missing("pod-farm/sine-chorus.png");
        wide.pixels = [800, 210];
        let mut stomp = UiImage::missing("pod-farm/vetta-wah.png");
        stomp.pixels = [174, 171];
        let mut body = UiImage::missing("variax/spank-body.png");
        body.pixels = [426, 412];
        let mut pickup = UiImage::missing("variax/spank-pickup.png");
        pickup.pixels = [120, 120];
        Self {
            drive: 50.0,
            switch_on: true,
            model: None,
            block_on: true,
            dialog_open: false,
            wide,
            stomp,
            body,
            pickup,
            outline: UiImage::missing("variax/outline.png"),
            show_outline: false,
            pickup_position: [1.8, 3.88],
            pickup_angle: [8.7, 0.0],
            pickup_on: [true, true],
            selected_pickup: 0,
            tuning_on: false,
            shifts: [0; 6],
            tuning_locked: false,
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
            ui.allocate_ui(egui::vec2(360.0, 300.0), |ui| self.body_gallery(ui));
            ui.allocate_ui(egui::vec2(700.0, 300.0), |ui| self.tuning_gallery(ui));
        });
        ui.add_space(theme::SPACE_16);
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

impl GalleryState {
    fn body_gallery(&mut self, ui: &mut egui::Ui) {
        caption(ui, "Body");
        ui.checkbox(&mut self.show_outline, "Outline");
        let pickups = [
            PickupSprite {
                position_inches: self.pickup_position[0],
                angle_degrees: self.pickup_angle[0],
                enabled: self.pickup_on[0],
                selected: self.selected_pickup == 0,
                image: &self.pickup,
            },
            PickupSprite {
                position_inches: self.pickup_position[1],
                angle_degrees: self.pickup_angle[1],
                enabled: self.pickup_on[1],
                selected: self.selected_pickup == 1,
                image: &self.pickup,
            },
        ];
        let mut canvas = BodyCanvas::new(&self.body, &pickups);
        if self.show_outline {
            canvas = canvas.outline(Some(&self.outline));
        }
        let events = canvas.show(ui).events;
        self.apply_body(events);
    }

    fn apply_body(&mut self, events: Vec<BodyEvent>) {
        for event in events {
            match event {
                BodyEvent::Moved { index, inches } => {
                    if let Some(position) = self.pickup_position.get_mut(index) {
                        *position = inches;
                    }
                }
                BodyEvent::Rotated { index, degrees } => {
                    if let Some(angle) = self.pickup_angle.get_mut(index) {
                        *angle = degrees;
                    }
                }
                BodyEvent::PickupClicked { index } => self.selected_pickup = index,
                BodyEvent::PickupToggled { index } => {
                    let Some(on) = self.pickup_on.get(index).copied() else {
                        continue;
                    };
                    let another = self
                        .pickup_on
                        .iter()
                        .enumerate()
                        .any(|(other, enabled)| other != index && *enabled);
                    if !on || another {
                        self.pickup_on[index] = !on;
                    }
                }
                BodyEvent::DragStarted { .. }
                | BodyEvent::DragFinished { .. }
                | BodyEvent::BodyClicked => {}
            }
        }
    }

    fn tuning_gallery(&mut self, ui: &mut egui::Ui) {
        caption(ui, "Tuning");
        let labels: Vec<String> = self.shifts.iter().copied().map(shift_text).collect();
        let notes: Vec<&str> = (0..6).map(|id| note_name(id, self.shifts[id])).collect();
        let names = ["High E", "B", "G", "D", "A", "Low E"];
        let rows: Vec<TuningRow<'_>> = (0..6)
            .map(|row| {
                let id = 5 - row;
                TuningRow {
                    id,
                    name: names[row],
                    shift: self.shifts[id],
                    shift_label: &labels[id],
                    note: notes[id],
                }
            })
            .collect();
        let events = TuningPanel::new(&rows)
            .enabled(self.tuning_on)
            .lock(false, self.tuning_locked)
            .show(ui)
            .events;
        for event in events {
            match event {
                TuningEvent::Enable => self.tuning_on = !self.tuning_on,
                TuningEvent::Reset => self.shifts = [0; 6],
                TuningEvent::FlattenAll => {
                    for shift in &mut self.shifts {
                        *shift = shift.saturating_sub(1).clamp(-12, 12);
                    }
                }
                TuningEvent::SharpenAll => {
                    for shift in &mut self.shifts {
                        *shift = shift.saturating_add(1).clamp(-12, 12);
                    }
                }
                TuningEvent::Shift { id, semitones } => {
                    if let Some(shift) = self.shifts.get_mut(id) {
                        *shift = semitones;
                    }
                }
                TuningEvent::Lock(locked) => self.tuning_locked = locked,
                TuningEvent::Presets
                | TuningEvent::DragStarted { .. }
                | TuningEvent::DragFinished { .. } => {}
            }
        }
    }
}

fn shift_text(shift: i32) -> String {
    if shift == 0 {
        "0 semitones".to_owned()
    } else if shift > 0 {
        format!("+{shift} semitones")
    } else {
        format!("{shift} semitones")
    }
}

fn note_name(id: usize, shift: i32) -> &'static str {
    const OPEN: [i32; 6] = [4, 9, 2, 7, 11, 4];
    const NOTES: [&str; 12] = [
        "C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B",
    ];
    let open = OPEN.get(id).copied().unwrap_or(0);
    NOTES[(open + shift).rem_euclid(12) as usize]
}

fn caption(ui: &mut egui::Ui, name: &str) {
    ui.label(RichText::new(name).size(SECTION).strong().color(TEXT));
}
