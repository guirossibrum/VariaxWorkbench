//! Shared geometry for the Variax editor body, inspector, and tuning band.
//!
//! Sizes are logical points. They do not depend on pixels per point.
//! [`body_canvas::fitted_stage`] supplies the one scale. This module does not
//! move pickup inches, change hit testing, or paint the shell.
//!
//! # Lane 3 wiring after #289
//!
//! Call [`editor_regions`] with the rect under the guitar header, the body
//! art size from [`body_canvas::placement_for_body`], and whether
//! `can_edit(TUNING, Some(MIX))` is true. Replace the equal `ui.columns(2)`
//! split and the full-width `guitar-tuning` panel with one child per rect:
//!
//! - [`EditorRegions::body`] is the well passed to `guitar()`. Its size is the
//!   fitted art, so [`BodyCanvas`] fills it and keeps the placement map.
//! - [`EditorRegions::inspector`] is the leftover column. The existing
//!   inspector frame and scroll area go inside it.
//! - [`EditorRegions::tuning`] is the whole tuning band. Its width is capped
//!   so the semitone neck stays at most [`NECK_MAX`].
//! - [`EditorRegions::twelve_string_width`] is [`TWELVE_STRING_COLUMN`] only
//!   when the section is available. Inside the tuning child, add that space
//!   and show `twelve_string_panel` only when the width is greater than zero.
//!   Do not paint “No 12-String available” in a reserved column.
//!
//! `guitar()` reads `ui.available_width()`. The body child has to be created
//! with `max_rect` equal to [`EditorRegions::body`] before that call.

use eframe::egui::{Pos2, Rect, Vec2, vec2};

use super::body_canvas::fitted_stage;

/// Family twelve-string column (`twelve_string::WIDTH`).
pub const TWELVE_STRING_COLUMN: f32 = 204.0;
/// String, Shift, and Note table inside the tuning frame (`88 + 148 + 40 + 16`).
pub const TUNING_TABLE_WIDTH: f32 = 292.0;
/// Tuning frame side margins (`12 + 12`) and the gap before the neck (`8`).
pub const TUNING_CHROME_WIDTH: f32 = 32.0;
/// Preferred narrowest semitone neck, in points.
pub const NECK_MIN: f32 = 220.0;
/// Widest semitone neck. A wider page does not stretch the scale past this.
pub const NECK_MAX: f32 = 420.0;
/// Gap between the body well and the inspector column.
pub const COLUMN_GAP: f32 = 16.0;
/// Design height of the six-string tuning block.
pub const TUNING_BLOCK_HEIGHT: f32 = 300.0;
/// Inspector keeps at least this much width when the page can spare it.
pub const INSPECTOR_MIN_WIDTH: f32 = 320.0;
/// Side-by-side layout is used only when the body can keep this scale.
pub const SIDE_MIN_SCALE: f32 = 0.75;
/// Default shell window. The editor test reserves chrome below this.
pub const LAPTOP_WINDOW: Vec2 = vec2(1120.0, 760.0);
/// One-line footer. The merged shell stores this as its first-frame height.
pub const SHELL_FOOTER_HEIGHT: f32 = 29.0;
/// Title row plus the wrapped command row at the laptop width.
pub const SHELL_HEADER_ALLOWANCE: f32 = 96.0;
/// Bank strip, model row, and the export line under the wood header.
pub const GUITAR_HEADER_ALLOWANCE: f32 = 108.0;

/// Body, inspector, and tuning rects inside one workspace.
///
/// The origin is the workspace origin. Every size is in points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EditorRegions {
    /// Fitted photo. One scale, the same aspect as the requested art.
    pub body_art: Vec2,
    /// Well for the body canvas. Same size as [`Self::body_art`].
    pub body: Rect,
    /// Column to the right of the body, or beside it in the top band.
    pub inspector: Rect,
    /// Tuning band, including the twelve-string column when that width is set.
    pub tuning: Rect,
    /// Zero when the twelve-string section is unavailable.
    pub twelve_string_width: f32,
}

/// Width reserved for Mix and Detune. Zero collapses the unavailable section.
pub fn twelve_string_column(available: bool) -> f32 {
    if available { TWELVE_STRING_COLUMN } else { 0.0 }
}

/// Preferred outer width of the tuning band: `(minimum, maximum)`.
///
/// The minimum is the table, the frame, [`NECK_MIN`], and the twelve-string
/// column when `twelve_string` is set. A page narrower than the minimum keeps
/// the table and lets the neck shrink; see [`contained_tuning_width`].
pub fn tuning_width_bounds(twelve_string: bool) -> (f32, f32) {
    let fixed = twelve_string_column(twelve_string) + TUNING_TABLE_WIDTH + TUNING_CHROME_WIDTH;
    (fixed + NECK_MIN, fixed + NECK_MAX)
}

/// Tuning width for a row that is `available_width` points wide.
///
/// The result is never wider than the neck cap, and never wider than the row.
pub fn contained_tuning_width(available_width: f32, twelve_string: bool) -> f32 {
    let available = finite_extent(available_width);
    let (_, preferred_max) = tuning_width_bounds(twelve_string);
    available.min(preferred_max)
}

/// Workspace under the guitar header at [`LAPTOP_WINDOW`].
///
/// Header and footer allowances match the merged shell. The guitar-header
/// allowance is the wood bank row plus the export line.
pub fn laptop_workspace() -> Rect {
    let height =
        LAPTOP_WINDOW.y - SHELL_HEADER_ALLOWANCE - SHELL_FOOTER_HEIGHT - GUITAR_HEADER_ALLOWANCE;
    Rect::from_min_size(Pos2::ZERO, vec2(LAPTOP_WINDOW.x, height))
}

/// Place the body, the inspector, and the tuning band in `workspace`.
///
/// When the body can stay at [`SIDE_MIN_SCALE`] while the tuning column keeps
/// its preferred minimum, the body uses the full workspace height and the
/// tuning band sits in the right column. Otherwise the tuning band sits along
/// the bottom at a contained width and the body uses the band above it.
pub fn editor_regions(workspace: Rect, art: Vec2, twelve_string: bool) -> EditorRegions {
    let art = finite_art(art);
    let workspace = finite_workspace(workspace);
    side_regions(workspace, art, twelve_string)
        .unwrap_or_else(|| stack_regions(workspace, art, twelve_string))
}

fn side_regions(workspace: Rect, art: Vec2, twelve_string: bool) -> Option<EditorRegions> {
    let (preferred_min, _) = tuning_width_bounds(twelve_string);
    let width = workspace.width();
    let height = workspace.height();
    let max_body_w = width - COLUMN_GAP - preferred_min;
    if max_body_w < 1.0 {
        return None;
    }
    let fitted = fit_inside(art, vec2(max_body_w, height));
    let scale = fitted.x / art.x;
    if !scale.is_finite() || scale < SIDE_MIN_SCALE {
        return None;
    }
    let column_left = workspace.left() + fitted.x + COLUMN_GAP;
    let column_width = workspace.right() - column_left;
    if column_width + 0.5 < preferred_min {
        return None;
    }
    let tune_h = tuning_block_height(height);
    let inspector_h = height - tune_h - COLUMN_GAP;
    if inspector_h < 80.0 {
        return None;
    }
    let body = Rect::from_min_size(workspace.min, fitted);
    let tuning_w = contained_tuning_width(column_width, twelve_string);
    let tuning = Rect::from_min_size(
        Pos2::new(column_left, workspace.bottom() - tune_h),
        vec2(tuning_w, tune_h),
    );
    let inspector = Rect::from_min_max(
        Pos2::new(column_left, workspace.top()),
        Pos2::new(workspace.right(), tuning.top() - COLUMN_GAP),
    );
    Some(EditorRegions {
        body_art: fitted,
        body,
        inspector,
        tuning,
        twelve_string_width: twelve_string_column(twelve_string),
    })
}

fn stack_regions(workspace: Rect, art: Vec2, twelve_string: bool) -> EditorRegions {
    let width = workspace.width();
    let height = workspace.height();
    let (top_h, tune_h) = stacked_heights(height);
    let tuning_w = contained_tuning_width(width, twelve_string);
    let tuning = Rect::from_min_size(
        Pos2::new(workspace.left(), workspace.bottom() - tune_h),
        vec2(tuning_w, tune_h),
    );
    let top = Rect::from_min_size(workspace.min, vec2(width, top_h));
    let inspector_budget = INSPECTOR_MIN_WIDTH
        .min(width * 0.46)
        .min((width - COLUMN_GAP - 1.0).max(1.0));
    let body_budget = (width - COLUMN_GAP - inspector_budget).max(1.0);
    let fitted = fit_inside(art, vec2(body_budget, top_h));
    let body = Rect::from_min_size(top.min, fitted);
    let inspector = Rect::from_min_max(
        Pos2::new(body.right() + COLUMN_GAP, top.top()),
        Pos2::new(workspace.right(), top.bottom()),
    );
    EditorRegions {
        body_art: fitted,
        body,
        inspector,
        tuning,
        twelve_string_width: twelve_string_column(twelve_string),
    }
}

fn tuning_block_height(workspace_height: f32) -> f32 {
    let room = (workspace_height - COLUMN_GAP - 80.0).max(1.0);
    TUNING_BLOCK_HEIGHT.min(room).max(1.0)
}

fn stacked_heights(height: f32) -> (f32, f32) {
    if height <= COLUMN_GAP + 2.0 {
        let half = (height * 0.5).max(1.0);
        return (half, half);
    }
    let inner = height - COLUMN_GAP;
    let tuning = TUNING_BLOCK_HEIGHT.min(inner * 0.5).max(1.0);
    let top = (inner - tuning).max(1.0);
    (top, tuning)
}

/// `fitted_stage` can land a fraction past the budget. Scale back on that axis.
fn fit_inside(art: Vec2, budget: Vec2) -> Vec2 {
    let fitted = fitted_stage(art, budget);
    let fix = (budget.x / fitted.x).min(budget.y / fitted.y).min(1.0);
    if fix.is_finite() && fix > 0.0 {
        fitted * fix
    } else {
        fitted
    }
}

fn finite_art(art: Vec2) -> Vec2 {
    vec2(finite_extent(art.x), finite_extent(art.y))
}

fn finite_workspace(workspace: Rect) -> Rect {
    let width = finite_extent(workspace.width());
    let height = finite_extent(workspace.height());
    let origin = if workspace.min.x.is_finite() && workspace.min.y.is_finite() {
        workspace.min
    } else {
        Pos2::ZERO
    };
    Rect::from_min_size(origin, vec2(width, height))
}

fn finite_extent(value: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        1.0
    }
}
