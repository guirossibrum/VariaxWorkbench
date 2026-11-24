//! Chain block, tab, toolbar button, status chip, dialog, and section panel.
//!
//! A wide rack skin (Mod, Reverb) uses a wider chain slot than a square pedal.
//! Fitting that image into the narrow slot keeps it a thin strip. [`section_panel`]
//! places its heading on the inner margin, with no empty band above it. #219
//! uses both when it builds the PODxt page.

use eframe::egui::{self, Color32, Frame, Margin, RichText, Sense, Stroke, Vec2, vec2};

use super::assets::{self, UiImage};
use super::controls::paint_led;
use super::theme::{
    AMBER, BAR, BG, BODY, COMPACT, DIM, FOCUS, FOCUS_RING, HD, LINE, MUTED, PODXT, RADIUS_CONTROL,
    RADIUS_PANEL, RAISED, SECTION, SPACE_16, STATUS, TARGET, TEXT, TITLE, VARIAX,
};

/// Aspect at or above this is a rack skin, not a stomp pedal.
pub const RACK_ASPECT: f32 = 2.5;
/// Slot for a wide Mod or Reverb skin. Taller once the image is contained.
pub const RACK_SLOT: Vec2 = vec2(220.0, 96.0);
/// Slot for a square pedal.
pub const STOMP_SLOT: Vec2 = vec2(92.0, 72.0);

/// Hover text for Detect devices when this build cannot scan USB.
pub const DETECTION_UNAVAILABLE: &str = "USB presence detection is unavailable in this build.";

/// Family accent on a tab. Colour is never the only active signal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    Variax,
    Podxt,
    Hd,
}

impl Family {
    pub fn accent(self) -> Color32 {
        match self {
            Self::Variax => VARIAX,
            Self::Podxt => PODXT,
            Self::Hd => HD,
        }
    }
}

/// Connection sentences from the approved specification.
///
/// For [`LinkState::LinkedBridge`], the tab name is the guitar model.
/// For [`LinkState::NoDevice`] and [`LinkState::HdUnsupported`], it is the
/// family and the open file. The chip never says synced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkState {
    NoDevice,
    IdentityPending,
    LinkedBridge,
    Uncertain,
    Disconnected,
    HdUnsupported,
}

impl LinkState {
    pub fn chip(self) -> &'static str {
        match self {
            Self::NoDevice => "No device",
            Self::IdentityPending => "USB detected · identity pending",
            Self::LinkedBridge => "USB bridge detected · guitar identity pending",
            Self::Uncertain => "USB presence could not be verified",
            Self::Disconnected => "Disconnected · editing the open file",
            Self::HdUnsupported => "Live connection not supported yet",
        }
    }

    pub fn tab_title(self, name: &str) -> String {
        match self {
            Self::NoDevice | Self::HdUnsupported => name.to_owned(),
            Self::IdentityPending | Self::LinkedBridge => format!("{name} · identity pending"),
            Self::Uncertain => format!("{name} · presence unknown"),
            Self::Disconnected => format!("{name} · disconnected"),
        }
    }
}

/// Width and height of the chain image. A wide skin gets [`RACK_SLOT`].
pub fn chain_slot(pixels: [usize; 2]) -> Vec2 {
    let aspect = match pixels {
        [0, _] | [_, 0] => 1.0,
        [width, height] => width as f32 / height as f32,
    };
    if aspect >= RACK_ASPECT {
        RACK_SLOT
    } else {
        STOMP_SLOT
    }
}

/// One chain block: image, name, and one LED. Clicking the image selects it.
/// Clicking the lamp toggles bypass. Off dims the image. The name stays readable.
pub struct ChainBlock<'a> {
    name: &'a str,
    on: &'a mut bool,
    selected: bool,
    image: Option<&'a UiImage>,
    enabled: bool,
}

pub struct ChainBlockOutput {
    pub response: egui::Response,
    pub toggled: bool,
}

impl<'a> ChainBlock<'a> {
    pub fn new(name: &'a str, on: &'a mut bool) -> Self {
        Self {
            name,
            on,
            selected: false,
            image: None,
            enabled: true,
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn image(mut self, image: &'a UiImage) -> Self {
        self.image = Some(image);
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn show(self, ui: &mut egui::Ui) -> ChainBlockOutput {
        let ChainBlock {
            name,
            on,
            selected,
            image,
            enabled,
        } = self;
        let slot = chain_slot(image.map(|image| image.pixels).unwrap_or([0, 0]));
        let mut toggled = false;
        let laid_out = ui.vertical(|ui| {
            ui.set_min_width(slot.x);
            ui.horizontal(|ui| {
                let sense = if enabled {
                    Sense::click()
                } else {
                    Sense::hover()
                };
                let (led_rect, led) = ui.allocate_exact_size(vec2(TARGET, TARGET), sense);
                paint_led(ui.painter(), led_rect.center(), *on, enabled);
                if enabled && led.clicked() {
                    *on = !*on;
                    toggled = true;
                }
                ui.label(
                    RichText::new(name)
                        .size(COMPACT)
                        .color(if enabled { TEXT } else { DIM }),
                );
            });
            let (art_rect, art) = ui.allocate_exact_size(
                slot,
                if enabled {
                    Sense::click()
                } else {
                    Sense::hover()
                },
            );
            if let Some(image) = image {
                let tint = if *on && enabled {
                    Color32::WHITE
                } else {
                    Color32::from_gray(140)
                };
                assets::paint_tinted(ui, image, art_rect, tint);
            } else {
                ui.painter().rect_filled(art_rect, RADIUS_CONTROL, RAISED);
            }
            art
        });
        if selected {
            ui.painter().rect_stroke(
                laid_out.response.rect,
                RADIUS_CONTROL,
                Stroke::new(2.0_f32, AMBER),
                egui::StrokeKind::Inside,
            );
        }
        ChainBlockOutput {
            response: laid_out.inner,
            toggled,
        }
    }
}

/// A document tab. The active tab is a filled plate plus its family accent.
/// Dirty state is the word `Unsaved`. Close is a 32px control.
pub struct DocumentTab<'a> {
    pub title: &'a str,
    pub family: Family,
    pub active: bool,
    pub unsaved: bool,
}

pub struct TabOutput {
    pub response: egui::Response,
    pub close: egui::Response,
}

impl DocumentTab<'_> {
    pub fn show(self, ui: &mut egui::Ui) -> TabOutput {
        ui.horizontal(|ui| {
            let (rect, response) = ui.allocate_exact_size(vec2(240.0, 40.0), Sense::click());
            let fill = if response.hovered() {
                Color32::from_rgb(0x2c, 0x31, 0x38)
            } else if self.active {
                RAISED
            } else {
                BG
            };
            let painter = ui.painter();
            painter.rect_filled(rect, RADIUS_CONTROL, fill);
            let accent = egui::Rect::from_min_size(rect.min, vec2(4.0, rect.height()));
            painter.rect_filled(
                accent,
                0.0,
                if self.active {
                    self.family.accent()
                } else {
                    LINE
                },
            );
            painter.text(
                rect.left_center() + vec2(12.0, 0.0),
                egui::Align2::LEFT_CENTER,
                self.title,
                egui::FontId::proportional(BODY),
                TEXT,
            );
            if self.unsaved {
                painter.text(
                    rect.right_center() - vec2(8.0, 0.0),
                    egui::Align2::RIGHT_CENTER,
                    "Unsaved",
                    egui::FontId::proportional(COMPACT),
                    TEXT,
                );
            }
            if response.has_focus() {
                painter.rect_stroke(
                    rect.expand(FOCUS_RING),
                    RADIUS_CONTROL,
                    Stroke::new(FOCUS_RING, FOCUS),
                    egui::StrokeKind::Outside,
                );
            }
            let close = ui
                .add(
                    egui::Button::new(RichText::new("×").size(BODY)).min_size(vec2(TARGET, TARGET)),
                )
                .on_hover_text("Close");
            TabOutput { response, close }
        })
        .inner
    }
}

/// Text button for the top bar. A disabled button explains itself in hover text.
pub fn toolbar_button(
    ui: &mut egui::Ui,
    label: &str,
    enabled: bool,
    disabled_reason: &str,
) -> egui::Response {
    let button = egui::Button::new(RichText::new(label).size(BODY)).min_size(vec2(TARGET, TARGET));
    let response = ui.add_enabled(enabled, button);
    if enabled {
        response
    } else {
        response.on_disabled_hover_text(disabled_reason)
    }
}

/// Short connection phrase. The sentence is the signal. Colour is not required.
pub fn status_chip(ui: &mut egui::Ui, state: LinkState) -> egui::Response {
    Frame::new()
        .fill(BAR)
        .stroke(Stroke::new(1.0_f32, LINE))
        .corner_radius(RADIUS_CONTROL)
        .inner_margin(Margin::symmetric(12, 6))
        .show(ui, |ui| {
            ui.label(RichText::new(state.chip()).size(STATUS).color(MUTED))
        })
        .response
}

pub struct DialogButton<'a> {
    pub label: &'a str,
    pub safe: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogEvent {
    Pressed(usize),
    Dismissed,
}

pub struct DialogOutput {
    pub event: Option<DialogEvent>,
    /// True when the safe button already holds keyboard focus this frame.
    pub safe_focused: bool,
}

/// Raised dialog on a scrim. Focus starts on the safe button. Escape cancels.
/// Tab cycles the buttons through egui. Enter activates the focused button.
///
/// The title is the dialog id. Closing clears the first-focus flag so the next
/// open focuses the safe button again.
pub fn dialog(
    ctx: &egui::Context,
    open: &mut bool,
    title: &str,
    message: &str,
    buttons: &[DialogButton<'_>],
) -> DialogOutput {
    let id = egui::Id::new(("shared-ui-dialog", title));
    let arm_id = id.with("focus-safe");
    if !*open {
        ctx.data_mut(|data| {
            data.remove::<bool>(arm_id);
        });
        return DialogOutput {
            event: None,
            safe_focused: false,
        };
    }
    let arm = ctx.data(|data| data.get_temp::<bool>(arm_id).unwrap_or(true));
    let frame = Frame::new()
        .fill(RAISED)
        .stroke(Stroke::new(1.0_f32, LINE))
        .corner_radius(RADIUS_PANEL)
        .inner_margin(Margin::same(16));
    let mut pressed = None;
    let mut safe_focused = false;
    let any_safe = buttons.iter().any(|button| button.safe);
    let modal = egui::Modal::new(id).frame(frame).show(ctx, |ui| {
        ui.set_min_width(360.0);
        ui.set_max_width(720.0);
        ui.label(RichText::new(title).size(TITLE).strong().color(TEXT));
        egui::ScrollArea::vertical()
            .id_salt(("dialog-body", title))
            .max_height(420.0)
            .show(ui, |ui| {
                ui.label(RichText::new(message).size(BODY).color(TEXT));
            });
        ui.horizontal(|ui| {
            let mut arm_focus = arm;
            for (index, button) in buttons.iter().enumerate() {
                let response = ui.add(
                    egui::Button::new(RichText::new(button.label).size(BODY))
                        .min_size(vec2(TARGET, TARGET)),
                );
                let focus_target = button.safe || (!any_safe && index == 0);
                if arm_focus && focus_target {
                    response.request_focus();
                    arm_focus = false;
                }
                if response.has_focus() && focus_target {
                    safe_focused = true;
                }
                if response.clicked() {
                    pressed = Some(index);
                }
            }
        });
    });
    let event = if modal.should_close() {
        Some(DialogEvent::Dismissed)
    } else {
        pressed.map(DialogEvent::Pressed)
    };
    if event.is_some() {
        *open = false;
        ctx.data_mut(|data| {
            data.remove::<bool>(arm_id);
        });
    } else if arm {
        ctx.data_mut(|data| data.insert_temp(arm_id, false));
    }
    DialogOutput {
        event,
        safe_focused,
    }
}

/// Outer rect and the heading rect of a section.
///
/// The heading is the first child. Its top is the panel top plus the 16px
/// inner margin. There is no spacer above it.
pub struct SectionPanel {
    pub outer: egui::Rect,
    pub heading: egui::Rect,
}

pub fn section_panel(
    ui: &mut egui::Ui,
    title: &str,
    body: impl FnOnce(&mut egui::Ui),
) -> SectionPanel {
    let mut heading = egui::Rect::NOTHING;
    let shown = Frame::new()
        .fill(RAISED)
        .stroke(Stroke::new(1.0_f32, LINE))
        .corner_radius(RADIUS_PANEL)
        .inner_margin(Margin::same(SPACE_16 as i8))
        .show(ui, |ui| {
            let label = ui.label(RichText::new(title).size(SECTION).strong().color(TEXT));
            heading = label.rect;
            body(ui);
        });
    SectionPanel {
        outer: shown.response.rect,
        heading,
    }
}
