//! Screen tokens from the approved layout specification.
//!
//! Sizes are logical points. egui scales them by `pixels_per_point`. There is
//! no second UI font.

use eframe::egui::{self, Color32, CornerRadius, Margin, Stroke, Style, Visuals, vec2};

pub const BG: Color32 = Color32::from_rgb(0x10, 0x11, 0x14);
pub const BAR: Color32 = Color32::from_rgb(0x17, 0x1a, 0x1f);
pub const RAISED: Color32 = Color32::from_rgb(0x22, 0x26, 0x2c);
pub const LINE: Color32 = Color32::from_rgb(0x3a, 0x40, 0x48);
pub const TEXT: Color32 = Color32::from_rgb(0xf4, 0xf1, 0xea);
pub const MUTED: Color32 = Color32::from_rgb(0xc4, 0xbf, 0xb4);
pub const DIM: Color32 = Color32::from_rgb(0x9a, 0x94, 0x88);
pub const AMBER: Color32 = Color32::from_rgb(0xf0, 0xa2, 0x02);
pub const FOCUS: Color32 = Color32::from_rgb(0xff, 0xc1, 0x4d);
pub const DANGER: Color32 = Color32::from_rgb(0xff, 0xb4, 0xa8);
pub const OK: Color32 = Color32::from_rgb(0xb6, 0xe3, 0xa2);
pub const VARIAX: Color32 = Color32::from_rgb(0xe0, 0xb1, 0x5a);
pub const PODXT: Color32 = Color32::from_rgb(0xe4, 0x57, 0x2e);
pub const HD: Color32 = Color32::from_rgb(0x8a, 0xa0, 0xb4);
pub const INK: Color32 = Color32::from_rgb(0x1c, 0x14, 0x0c);
pub const PANEL: Color32 = Color32::from_rgb(0x1a, 0x1d, 0x22);

pub const TITLE: f32 = 20.0;
pub const SECTION: f32 = 15.0;
pub const BODY: f32 = 14.0;
pub const COMPACT: f32 = 13.0;
pub const KNOB_LABEL: f32 = 11.0;
pub const STATUS: f32 = 13.0;

pub const SPACE_4: f32 = 4.0;
pub const SPACE_8: f32 = 8.0;
pub const SPACE_12: f32 = 12.0;
pub const SPACE_16: f32 = 16.0;
pub const SPACE_24: f32 = 24.0;
pub const RADIUS_CONTROL: u8 = 6;
pub const RADIUS_PANEL: u8 = 8;
pub const TARGET: f32 = 32.0;
pub const FOCUS_RING: f32 = 2.0;

/// Install the dark shell theme. Family editors can still paint the light
/// Variax inspector with [`INK`] on a metal fill.
pub fn apply(ctx: &egui::Context) {
    ctx.set_style(style());
}

pub fn style() -> Style {
    let mut style = Style {
        visuals: visuals(),
        ..Default::default()
    };
    style.spacing.item_spacing = vec2(SPACE_8, SPACE_8);
    style.spacing.button_padding = vec2(SPACE_12, SPACE_4);
    style.spacing.interact_size = vec2(TARGET, TARGET);
    style.spacing.window_margin = Margin::same(SPACE_16 as i8);
    style
}

fn widget(fill: Color32, fg: Color32, radius: u8) -> egui::style::WidgetVisuals {
    egui::style::WidgetVisuals {
        bg_fill: fill,
        weak_bg_fill: fill,
        bg_stroke: Stroke::new(1.0_f32, LINE),
        fg_stroke: Stroke::new(1.0_f32, fg),
        corner_radius: CornerRadius::same(radius),
        expansion: 0.0,
    }
}

pub fn visuals() -> Visuals {
    let mut visuals = Visuals::dark();
    visuals.window_fill = RAISED;
    visuals.panel_fill = BG;
    visuals.faint_bg_color = PANEL;
    visuals.extreme_bg_color = BG;
    visuals.window_stroke = Stroke::new(1.0_f32, LINE);
    visuals.window_corner_radius = CornerRadius::same(RADIUS_PANEL);
    visuals.menu_corner_radius = CornerRadius::same(RADIUS_CONTROL);
    visuals.widgets.noninteractive = widget(BG, MUTED, RADIUS_CONTROL);
    visuals.widgets.inactive = widget(RAISED, TEXT, RADIUS_CONTROL);
    visuals.widgets.hovered = widget(Color32::from_rgb(0x2c, 0x31, 0x38), TEXT, RADIUS_CONTROL);
    visuals.widgets.active = widget(Color32::from_rgb(0x3a, 0x2a, 0x12), TEXT, RADIUS_CONTROL);
    visuals.widgets.open = widget(RAISED, TEXT, RADIUS_CONTROL);
    visuals.selection.bg_fill = Color32::from_rgb(0x3a, 0x2a, 0x12);
    visuals.selection.stroke = Stroke::new(FOCUS_RING, FOCUS);
    visuals.hyperlink_color = AMBER;
    visuals.error_fg_color = DANGER;
    visuals.warn_fg_color = AMBER;
    visuals.override_text_color = Some(TEXT);
    visuals
}
