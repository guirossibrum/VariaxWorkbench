use std::time::Instant;

use eframe::egui::{self, Align2, Color32, Rect, RichText, Stroke, StrokeKind, vec2};

use super::{Attachment, Connections, Detection, Row, Status};
use crate::ui::theme;

pub(super) const WINDOW_ID: &str = "device-connections";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    EnterWorkbench,
    EditOffline,
    OpenEditor(Attachment),
    OpenOfflineEditor(&'static str),
    Retry(Attachment),
    AllowDownload(Attachment),
    SkipDownload(Attachment),
    CancelDownload(Attachment),
}

/// Fill the viewport before the shell mounts the workspace.
pub fn show_startup(
    ctx: &egui::Context,
    connections: &mut Connections,
    now: Instant,
    mut paint_artwork: impl FnMut(&egui::Ui, &'static str, Rect) -> bool,
) -> Option<Action> {
    let was_visible = connections.visible;
    if let Some(delay) = connections.advance(now) {
        ctx.request_repaint_after(delay);
    }
    let complete = was_visible && !connections.visible;
    egui::CentralPanel::default()
        .show(ctx, |ui| {
            let width = ui.available_width().clamp(0.0, 490.0);
            ui.add_space((ui.available_height() * 0.12).min(80.0));
            ui.vertical_centered(|ui| {
                ui.heading(RichText::new("Variax Workbench").size(theme::TITLE));
                ui.label(RichText::new("Device connections").size(theme::SECTION));
                ui.add_space(theme::SPACE_16);
                let height = (ui.available_height() - theme::TARGET - theme::SPACE_16).max(32.0);
                ui.allocate_ui_with_layout(
                    vec2(width, height + theme::TARGET + theme::SPACE_16),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        let action = show_rows(ui, connections, height, &mut paint_artwork, true);
                        ui.add_space(theme::SPACE_8);
                        let offline = ui.add(action_button("Edit offline")).clicked();
                        if offline {
                            Some(Action::EditOffline)
                        } else {
                            action
                        }
                    },
                )
                .inner
            })
            .inner
        })
        .inner
        .or(complete.then_some(Action::EnterWorkbench))
}

/// Render after draining discovery and read events. A Retry action is a
/// request to the caller and does not start or authorize a library read.
pub fn show(
    ctx: &egui::Context,
    connections: &mut Connections,
    now: Instant,
    mut paint_artwork: impl FnMut(&egui::Ui, &'static str, Rect) -> bool,
) -> Option<Action> {
    let was_hidden = connections.progress_hidden();
    if let Some(delay) = connections.advance(now) {
        ctx.request_repaint_after(delay);
    }
    if was_hidden && !connections.visible {
        ctx.request_repaint();
    }
    if !connections.visible || connections.hidden {
        return None;
    }

    let id = egui::Id::new(WINDOW_ID);
    let screen = ctx.content_rect();
    let width = (screen.width() - 64.0).clamp(128.0, 440.0);
    let max_rows_height = (screen.height() - 148.0).clamp(48.0, 356.0);
    let first_layout = !ctx.data(|data| data.get_temp::<bool>(id.with("placed")).unwrap_or(false));
    let mut window = egui::Window::new("Device connections")
        .id(id)
        .default_size(vec2(width, max_rows_height + 48.0))
        .resizable(false)
        .collapsible(false)
        .fade_in(false);
    if first_layout {
        // Center using the measured size. After the sizing pass, pin the
        // top-left corner so a changed row height cannot shift the title bar.
        window = window
            .pivot(Align2::CENTER_CENTER)
            .default_pos(screen.center());
    } else if let Some(top_left) =
        ctx.data_mut(|data| data.remove_temp::<egui::Pos2>(id.with("initial-corner")))
    {
        window = window.current_pos(top_left);
    }

    let rendered = window.show(ctx, |ui| {
        ui.set_width(width);
        ui.spacing_mut().item_spacing.y = theme::SPACE_8;
        let action = show_rows(ui, connections, max_rows_height, &mut paint_artwork, false);
        ui.separator();
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), theme::TARGET),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                let busy = connections.detection == Detection::Checking
                    || connections.shown_rows().any(|row| {
                        matches!(
                            row.status,
                            Status::Checking | Status::Waiting | Status::Reading { .. }
                        )
                    });
                let mut close = false;
                if busy && connections.background {
                    if ui.add(action_button("Hide progress")).clicked() {
                        connections.hidden = true;
                        ctx.request_repaint();
                    }
                } else {
                    close = ui.add_enabled(!busy, action_button("Close")).clicked();
                }
                (action, close)
            },
        )
        .inner
    })?;

    if first_layout {
        // The first area pass is invisible and only measures the window.
        let top_left = rendered.response.rect.min;
        ctx.data_mut(|data| {
            data.insert_temp(id.with("placed"), true);
            data.insert_temp(id.with("initial-corner"), top_left);
        });
        ctx.request_repaint();
    }
    let (action, close) = rendered.inner?;
    if close || action == Some(Action::EditOffline) {
        connections.dismiss();
    }
    action
}

fn show_rows(
    ui: &mut egui::Ui,
    connections: &Connections,
    height: f32,
    paint_artwork: &mut impl FnMut(&egui::Ui, &'static str, Rect) -> bool,
    startup: bool,
) -> Option<Action> {
    egui::ScrollArea::vertical()
        .id_salt("connection-rows")
        .max_height(height)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            let mut action = detection_summary(ui, connections, !startup);
            for diagnostic in &connections.diagnostics {
                ui.label(RichText::new(diagnostic).color(theme::DANGER));
            }
            for row in connections.shown_rows() {
                let row_action = ui
                    .push_id(row.attachment.device, |ui| {
                        device_row(ui, row, paint_artwork, startup)
                    })
                    .inner;
                action = action.or(row_action);
            }
            action
        })
        .inner
}

fn detection_summary(
    ui: &mut egui::Ui,
    connections: &Connections,
    offline_action: bool,
) -> Option<Action> {
    let (title, details) = match &connections.detection {
        Detection::Idle | Detection::Checking => ("Checking for devices…", None),
        Detection::Unavailable(reason) => ("Device detection unavailable", Some(reason.as_str())),
        Detection::Failed(reason) => ("Device check failed", Some(reason.as_str())),
        Detection::Complete
            if connections
                .shown_rows()
                .all(|row| row.status == Status::Unsupported) =>
        {
            (
                "No device connected",
                Some("No supported device is connected"),
            )
        }
        _ => return None,
    };
    ui.label(RichText::new(title).size(theme::SECTION).strong());
    if let Some(details) = details {
        ui.label(details);
        if offline_action && ui.add(action_button("Edit offline")).clicked() {
            return Some(Action::EditOffline);
        }
    }
    None
}

fn device_row(
    ui: &mut egui::Ui,
    row: &Row,
    paint_artwork: &mut impl FnMut(&egui::Ui, &'static str, Rect) -> bool,
    startup: bool,
) -> Option<Action> {
    let mut draw = |ui: &mut egui::Ui| {
        egui::Frame::group(ui.style())
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    let (icon, _) = ui.allocate_exact_size(vec2(48.0, 48.0), egui::Sense::hover());
                    if !row
                        .device
                        .artwork
                        .is_some_and(|path| paint_artwork(ui, path, icon))
                    {
                        neutral_icon(ui, icon);
                    }
                    ui.vertical(|ui| {
                        ui.set_width(ui.available_width());
                        let connected = !row.presence_uncertain
                            && matches!(
                                row.status,
                                Status::Connected
                                    | Status::Unsupported
                                    | Status::Consent(_)
                                    | Status::Skipped(_)
                                    | Status::Waiting
                                    | Status::Reading { .. }
                                    | Status::Complete
                                    | Status::Failed(_)
                            );
                        let title = if connected {
                            format!("{} Connected", row.device.name)
                        } else {
                            row.device.name.clone()
                        };
                        ui.label(RichText::new(title).size(theme::BODY).strong());
                        if row.reconnected && row.status != Status::Disconnected {
                            ui.label("Reconnected after disconnect");
                        }
                        row_status(ui, row, startup)
                    })
                    .inner
                })
                .inner
            })
            .inner
    };
    if !startup || row.presence_uncertain || row.status != Status::Complete {
        return draw(ui);
    }

    // The card senses pointer clicks behind its controls. Open editor
    // remains the keyboard focus target.
    let card = ui.scope_builder(egui::UiBuilder::new().sense(egui::Sense::CLICK), |ui| {
        ui.style_mut().interaction.selectable_labels = false;
        draw(ui)
    });
    card.inner.or_else(|| {
        card.response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
            .then_some(Action::OpenEditor(row.attachment))
    })
}

fn row_status(ui: &mut egui::Ui, row: &Row, startup: bool) -> Option<Action> {
    if row.presence_uncertain && row.status != Status::Disconnected {
        ui.label("Connection temporarily unverified");
        return None;
    }
    match &row.status {
        Status::Checking => ui.label("Checking connection…"),
        Status::Connected => ui.label("Library sync has not started"),
        Status::Unverified(reason) => {
            ui.label("USB identity unverified");
            ui.label(reason);
            return offline_editor(ui, row);
        }
        Status::UnverifiedFirmware(reason) => {
            ui.label(RichText::new("Unverified firmware").color(theme::AMBER));
            ui.label(reason);
            return offline_editor(ui, row);
        }
        Status::Unsupported => {
            ui.label("Live connection not supported");
            return offline_editor(ui, row);
        }
        Status::Consent(prompt) => {
            ui.label(*prompt);
            return ui
                .horizontal_wrapped(|ui| {
                    let download = ui.add(action_button("Download")).clicked();
                    let skip = ui
                        .add(action_button("Skip"))
                        .on_hover_text("Skip the library download and keep the guitar connected.")
                        .clicked();
                    if download {
                        Some(Action::AllowDownload(row.attachment))
                    } else if skip {
                        Some(Action::SkipDownload(row.attachment))
                    } else {
                        None
                    }
                })
                .inner;
        }
        Status::Skipped(reason) => ui.label(*reason),
        Status::Waiting => return waiting(ui, row, startup),
        Status::Reading { done, total, .. } => {
            if row.wait_reason.is_some() {
                return waiting(ui, row, false);
            }
            ui.label(format!("Model Sync {done} of {total}"));
            ui.add(
                egui::ProgressBar::new(*done as f32 / total.get() as f32)
                    .desired_width(ui.available_width())
                    .desired_height(theme::SPACE_8),
            );
            return cancel(ui, row);
        }
        Status::Complete => {
            ui.label("Model Sync Complete");
            return (startup && ui.add(action_button("Open editor")).clicked())
                .then_some(Action::OpenEditor(row.attachment));
        }
        Status::Failed(reason) => {
            ui.label(RichText::new("Sync failed").color(theme::DANGER));
            ui.label(reason);
            return ui
                .add(action_button("Retry"))
                .clicked()
                .then_some(Action::Retry(row.attachment));
        }
        Status::Disconnected => ui.label("Disconnected"),
    };
    None
}

fn waiting(ui: &mut egui::Ui, row: &Row, can_skip: bool) -> Option<Action> {
    if let Some(resource) = row.wait_reason {
        ui.label(format!("Waiting to sync · {resource}"));
    } else {
        ui.label("Waiting to sync");
    }
    ui.horizontal_wrapped(|ui| {
        let action = cancel(ui, row);
        let skip = can_skip && ui.add(action_button("Skip")).clicked();
        action.or(skip.then_some(Action::SkipDownload(row.attachment)))
    })
    .inner
}

fn cancel(ui: &mut egui::Ui, row: &Row) -> Option<Action> {
    ui.add(action_button("Cancel download"))
        .clicked()
        .then_some(Action::CancelDownload(row.attachment))
}

fn offline_editor(ui: &mut egui::Ui, row: &Row) -> Option<Action> {
    let editor = row.device.offline_editor?;
    ui.add(action_button("Open offline editor"))
        .clicked()
        .then_some(Action::OpenOfflineEditor(editor))
}

fn action_button(label: &str) -> egui::Button<'_> {
    egui::Button::new(label).min_size(vec2(theme::TARGET, theme::TARGET))
}

fn neutral_icon(ui: &egui::Ui, rect: Rect) {
    let painter = ui.painter();
    painter.rect_filled(rect, theme::RADIUS_CONTROL, ui.visuals().faint_bg_color);
    let panel = Rect::from_center_size(rect.center(), vec2(26.0, 16.0));
    painter.rect_stroke(
        panel,
        3.0,
        Stroke::new(1.5_f32, ui.visuals().weak_text_color()),
        StrokeKind::Inside,
    );
    for x in [-6.0, 0.0, 6.0] {
        painter.circle_filled(panel.center() + vec2(x, 0.0), 1.5, Color32::GRAY);
    }
}
