//! Every shared control, so a headless test can see the component names.
//!
//! The window is the example binary `variax-workbench-ui-gallery`. A hidden
//! shell tab would edit `src/app/shell`, which this library does not do.

use super::assets::UiImage;
use super::body_canvas::{self, BodyCanvas, BodyEvent, PickupSprite};
use super::chrome::{self, DialogButton, DocumentTab, Family, LinkState};
use super::controls::{self, Selector, Switch};
use super::knob::Knob;
use super::theme::{self, BODY, SECTION, TEXT};
use super::tuning_panel::{TuningEvent, TuningPanel, TuningRow, TwelveStringRow};
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
    pickup: UiImage,
    /// Unmultiplied RGBA of `pickup`, shared by both gallery sprites.
    pickup_rgba: Option<Vec<u8>>,
    pub outline: UiImage,
    pub show_outline: bool,
    pub pickup_position: [f32; 2],
    pub pickup_angle: [f32; 2],
    pub pickup_on: [bool; 2],
    pub selected_pickup: usize,
    pub tuning_on: bool,
    pub shifts: [i32; 6],
    pub tuning_locked: bool,
    pub twelve_string: bool,
    pub mix: [f32; 6],
    pub detune: [f32; 6],
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
            pickup_rgba: None,
            outline: UiImage::missing("variax/outline.png"),
            show_outline: false,
            pickup_position: [1.8, 3.88],
            pickup_angle: [8.7, 0.0],
            pickup_on: [true, true],
            selected_pickup: 0,
            tuning_on: false,
            shifts: [0; 6],
            tuning_locked: false,
            twelve_string: false,
            mix: [0.5; 6],
            detune: [3.25, -4.12, 5.0, 3.5, -0.5, 0.0],
        }
    }
}

impl GalleryState {
    /// Load `variax/spank-pickup.png` and keep the unmultiplied pixels the
    /// canvas uses for hits. A missing file leaves the placeholder and no buffer.
    pub fn load_pickup(&mut self, ctx: &egui::Context, root: &std::path::Path) {
        let name = "variax/spank-pickup.png";
        let Ok(decoded) = image::open(root.join(name)) else {
            return;
        };
        let rgba = decoded.into_rgba8();
        let pixels = [rgba.width() as usize, rgba.height() as usize];
        if pixels[0] == 0 || pixels[1] == 0 {
            return;
        }
        let color = egui::ColorImage::from_rgba_unmultiplied(pixels, rgba.as_raw());
        self.pickup = UiImage {
            name: name.into(),
            pixels,
            texture: Some(ctx.load_texture(name, color, egui::TextureOptions::LINEAR)),
            missing: false,
        };
        self.pickup_rgba = Some(rgba.into_raw());
    }

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
        ui.vertical(|ui| {
            caption(ui, "Body");
            ui.checkbox(&mut self.show_outline, "Outline");
            if self.show_outline {
                let slot = body_canvas::outline_rect(egui::Pos2::ZERO, &self.outline, 1.0).size();
                let (space, _) = ui.allocate_exact_size(slot, egui::Sense::hover());
                body_canvas::paint_body_outline(
                    ui,
                    &self.outline,
                    space.min - egui::vec2(body_canvas::OUTLINE_X, body_canvas::OUTLINE_Y),
                    1.0,
                );
            }
            for (index, label) in ["Pickup 1", "Pickup 2"].into_iter().enumerate() {
                ui.selectable_value(
                    &mut self.selected_pickup,
                    index,
                    RichText::new(label).size(BODY),
                );
            }
        });
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
        let alphas = [self.pickup_rgba.as_deref(); 2];
        let events = BodyCanvas::new(&self.body, &pickups)
            .alphas(&alphas)
            .show(ui)
            .events;
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
        ui.checkbox(&mut self.twelve_string, "Twelve-string");
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
        let secondary: [TwelveStringRow; 6] = std::array::from_fn(|id| TwelveStringRow {
            id,
            mix: Some(self.mix[id]),
            detune: Some(self.detune[id]),
        });
        let mut panel = TuningPanel::new(&rows)
            .enabled(self.tuning_on)
            .lock(self.twelve_string, self.tuning_locked);
        if self.twelve_string {
            panel = panel.twelve_string(&secondary);
        }
        let events = panel.show(ui).events;
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
                TuningEvent::Mix { id, fraction } => {
                    if self.tuning_locked {
                        self.mix.fill(fraction);
                    } else {
                        self.mix[id] = fraction;
                    }
                }
                TuningEvent::Detune { id, cents } => self.detune[id] = cents,
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

#[cfg(test)]
mod tests {
    use super::GalleryState;

    #[test]
    fn load_pickup_keeps_unmultiplied_rgba_for_both_sprites() {
        let directory = tempfile::tempdir().unwrap();
        let variax = directory.path().join("variax");
        std::fs::create_dir(&variax).unwrap();
        let mut image = image::RgbaImage::new(2, 2);
        image.put_pixel(0, 0, image::Rgba([1, 2, 3, 204]));
        image.put_pixel(1, 0, image::Rgba([4, 5, 6, 255]));
        image.put_pixel(0, 1, image::Rgba([7, 8, 9, 0]));
        image.put_pixel(1, 1, image::Rgba([10, 11, 12, 205]));
        image.save(variax.join("spank-pickup.png")).unwrap();

        let ctx = eframe::egui::Context::default();
        let mut missing = GalleryState::default();
        missing.load_pickup(&ctx, &directory.path().join("absent"));
        assert!(missing.pickup.missing);
        assert!(missing.pickup_rgba.is_none());

        let mut state = GalleryState::default();
        state.load_pickup(&ctx, directory.path());
        assert!(!state.pickup.missing);
        assert_eq!(state.pickup.pixels, [2, 2]);
        assert_eq!(
            state.pickup_rgba.as_deref(),
            Some(&[1, 2, 3, 204, 4, 5, 6, 255, 7, 8, 9, 0, 10, 11, 12, 205][..])
        );
    }
}
