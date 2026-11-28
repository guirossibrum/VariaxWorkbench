//! Presentation for an editor-owned foreground tuner session.

use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use eframe::egui::{self, Key, Modifiers, RichText, Stroke, pos2, vec2};

use super::theme;

const READING_LIFETIME: Duration = Duration::from_secs(1);
const IN_TUNE: i16 = 2;
const METER_LIMIT: i16 = 50;

fn window_id() -> egui::Id {
    egui::Id::new("shared-tuner-popup")
}

/// The caller distinguishes an absent or invalid reply from an explicit no-note reply.
#[derive(Clone, Copy, Debug)]
pub enum TunerNote<'a> {
    Missing,
    NoSignal,
    Named(&'a str),
}

/// One paired sample from the current session and attachment.
#[derive(Clone, Copy, Debug)]
pub struct TunerReading<'a> {
    pub note: TunerNote<'a>,
    pub cents: Option<i16>,
    /// Host monotonic receipt time, independent of session elapsed time.
    pub received_at: Instant,
}

#[derive(Clone, Copy, Debug)]
pub enum TunerState<'a> {
    Starting,
    Listening {
        reading: Option<TunerReading<'a>>,
    },
    Stopping,
    Stopped,
    Disconnected,
    /// The caller supplies a user-facing error, including any uncertain cleanup.
    Error(&'a str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TunerAction {
    /// Cancel once and keep showing the popup with `TunerState::Stopping`
    /// until the worker finishes.
    Stop,
    /// Dismiss a terminal status.
    Dismiss,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum View {
    #[default]
    Digital,
    Analog,
}

impl View {
    fn load(path: &Path) -> Self {
        match fs::read_to_string(path).as_deref().map(str::trim) {
            Ok("analog") => Self::Analog,
            _ => Self::Digital,
        }
    }

    fn save(self, path: &Path) -> io::Result<()> {
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent)?;
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        file.write_all(match self {
            Self::Digital => b"digital\n",
            Self::Analog => b"analog\n",
        })?;
        file.as_file().sync_all()?;
        file.persist(path)?;
        Ok(())
    }
}

/// A movable nonmodal window. The editor controls its visibility and the
/// tuner lifecycle. All instances in the app share the view preference.
#[derive(Default)]
pub struct TunerPopup {
    preference_path: Option<PathBuf>,
    last_frame: Option<u64>,
    controls: Option<[egui::Id; 3]>,
    last_status: Option<String>,
}

impl TunerPopup {
    /// Use the app's editor-preferences location to locate the sibling tuner
    /// preference. With no location, the choice lasts for the app context.
    pub fn with_preferences(editor_preferences: Option<&Path>) -> Self {
        Self {
            preference_path: editor_preferences.map(|path| path.with_file_name("tuner-view.txt")),
            ..Self::default()
        }
    }

    /// Call while the popup should be visible, before editor Escape shortcuts.
    /// The caller supplies only current-session samples and retains terminal
    /// states until the user dismisses them.
    pub fn show(&mut self, ctx: &egui::Context, state: TunerState<'_>) -> Option<TunerAction> {
        self.show_at(ctx, state, Instant::now())
    }

    /// Pass the toolbar response, or a focusable fallback when it is unavailable.
    pub fn restore_focus(&mut self, target: &egui::Response) {
        self.last_frame = None;
        self.last_status = None;
        target.request_focus();
    }

    fn show_at(
        &mut self,
        ctx: &egui::Context,
        state: TunerState<'_>,
        now: Instant,
    ) -> Option<TunerAction> {
        let id = window_id();
        let view_id = id.with("view-preference");
        let mut view = ctx
            .data(|data| data.get_temp::<View>(view_id))
            .unwrap_or_else(|| {
                let view = self
                    .preference_path
                    .as_deref()
                    .map_or(View::Digital, View::load);
                ctx.data_mut(|data| data.insert_temp(view_id, view));
                view
            });
        let previous_view = view;
        let frame = ctx.cumulative_frame_nr();
        let opening = self
            .last_frame
            .is_none_or(|last| frame.saturating_sub(last) > 1);

        let display = Display::new(state, now);
        if let Some(expires_in) = display.expires_in {
            ctx.request_repaint_after(expires_in);
        }
        let command = match state {
            TunerState::Starting | TunerState::Listening { .. } => Some(TunerAction::Stop),
            TunerState::Stopping => None,
            TunerState::Stopped | TunerState::Disconnected | TunerState::Error(_) => {
                Some(TunerAction::Dismiss)
            }
        };

        let escape = !opening && self.keyboard(ctx, &mut view, command.is_some());
        let bounds = ctx.content_rect().shrink(8.0);
        let compact = bounds.width() < 360.0 || bounds.height() < 300.0;
        let padding = if compact { 6 } else { 12 };
        let width = (bounds.width() - 2.0 * (f32::from(padding) + 1.0)).clamp(1.0, 372.0);
        // Reserve room for the title, source, selector, frame margins and action row.
        let chrome_height = 166.0;
        let body_height = (bounds.height() - chrome_height).clamp(12.0, 138.0);
        let window = egui::Window::new("Tuner")
            .id(id)
            .order(egui::Order::Foreground)
            .default_pos(
                bounds.center()
                    - vec2(
                        width / 2.0 + f32::from(padding),
                        (body_height + chrome_height).min(bounds.height()) / 2.0,
                    ),
            )
            .constrain_to(bounds)
            .default_size(vec2(width, body_height + 118.0))
            .min_width(width)
            .max_width(width)
            .resizable(false)
            .collapsible(false)
            .fade_in(false)
            .frame(egui::Frame::window(&ctx.style()).inner_margin(padding));
        let mut open = true;
        let window = if command.is_some() {
            window.open(&mut open)
        } else {
            window
        };
        let close_requested = window
            .show(ctx, |ui| {
                if compact {
                    ui.spacing_mut().item_spacing.y = 4.0;
                }
                ui.label(RichText::new("PODxt Live").color(theme::MUTED));
                let selectors = ui
                    .horizontal(|ui| {
                        let button_width =
                            (ui.available_width() - ui.spacing().item_spacing.x) / 2.0;
                        [View::Digital, View::Analog].map(|option| {
                            ui.add(
                                egui::Button::selectable(
                                    view == option,
                                    match option {
                                        View::Digital => "Digital",
                                        View::Analog => "Analog",
                                    },
                                )
                                .min_size(vec2(button_width, theme::TARGET)),
                            )
                        })
                    })
                    .inner;
                for (option, selector) in [View::Digital, View::Analog].into_iter().zip(&selectors)
                {
                    if selector.clicked() {
                        view = option;
                        selector.request_focus();
                    }
                }

                egui::Frame::new()
                    .fill(theme::PANEL)
                    .stroke(Stroke::new(1.0_f32, theme::LINE))
                    .corner_radius(theme::RADIUS_PANEL)
                    .inner_margin(if compact { 4 } else { 8 })
                    .show(ui, |ui| {
                        let reading = egui::ScrollArea::vertical()
                            .max_height(body_height)
                            .min_scrolled_height(0.0)
                            .auto_shrink([false, false])
                            .show(ui, |ui| draw_reading(ui, view, &display));
                        let response = ui.interact(
                            reading.inner_rect,
                            id.with("reading"),
                            egui::Sense::hover(),
                        );
                        let label =
                            |text| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, text);
                        let status = display.announcement();
                        if ui.is_visible() && self.last_status.as_deref() != Some(&status) {
                            response.output_event(egui::output::OutputEvent::ValueChanged(label(
                                status.clone(),
                            )));
                            self.last_status = Some(status);
                        }
                        response.widget_info(|| label(display.description()));
                    });
                let stop = ui
                    .allocate_ui_with_layout(
                        vec2(ui.available_width(), theme::TARGET),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.label(RichText::new(display.status).strong().color(display.color));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let label = if command == Some(TunerAction::Dismiss) {
                                    "Close"
                                } else {
                                    "Stop tuner"
                                };
                                ui.add_enabled(command.is_some(), egui::Button::new(label))
                            })
                            .inner
                        },
                    )
                    .inner;

                if ui.is_visible() {
                    self.last_frame = Some(frame);
                    self.controls = Some([selectors[0].id, selectors[1].id, stop.id]);
                    let selected = usize::from(view == View::Analog);
                    if opening || (command.is_none() && stop.has_focus()) {
                        selectors[selected].request_focus();
                    }
                    for control in [&selectors[0], &selectors[1], &stop] {
                        if control.has_focus() {
                            ui.painter().rect_stroke(
                                control.rect.shrink(1.0),
                                theme::RADIUS_CONTROL,
                                Stroke::new(theme::FOCUS_RING, theme::FOCUS),
                                egui::StrokeKind::Inside,
                            );
                        }
                    }
                }
                stop.clicked()
            })
            .and_then(|response| response.inner)
            .unwrap_or(false);
        if view != previous_view {
            ctx.data_mut(|data| data.insert_temp(view_id, view));
            if let Some(path) = &self.preference_path
                && let Err(error) = view.save(path)
            {
                eprintln!("Could not save tuner view preference: {error}");
            }
            ctx.request_repaint();
        }
        if close_requested || !open || escape {
            command
        } else {
            None
        }
    }

    fn keyboard(&self, ctx: &egui::Context, view: &mut View, actionable: bool) -> bool {
        let Some(controls) = self.controls else {
            return false;
        };
        if !ctx.input(|input| input.focused) {
            return false;
        }
        let escape_pressed = ctx.input(|input| input.key_pressed(Key::Escape));
        let focused = ctx.memory(|memory| {
            if !memory.allows_interaction(egui::LayerId::new(egui::Order::Foreground, window_id()))
            {
                return None;
            }
            controls.iter().position(|&id| {
                memory.has_focus(id)
                    || (escape_pressed
                        && memory.focused().is_none()
                        && memory.had_focus_last_frame(id))
            })
        });
        let Some(index) = focused else {
            return false;
        };
        let (escape, navigation) = ctx.input_mut(|input| {
            let escape = input.consume_key(Modifiers::NONE, Key::Escape);
            let navigation = if input.consume_key(Modifiers::SHIFT, Key::Tab) {
                Some(-1)
            } else if input.consume_key(Modifiers::NONE, Key::Tab) {
                Some(1)
            } else if index < 2 && input.consume_key(Modifiers::NONE, Key::ArrowLeft) {
                *view = View::Digital;
                Some(-(index as isize))
            } else if index < 2 && input.consume_key(Modifiers::NONE, Key::ArrowRight) {
                *view = View::Analog;
                Some(1 - index as isize)
            } else {
                None
            };
            (escape, navigation)
        });
        if let Some(offset) = navigation {
            let count = if actionable { 3 } else { 2 };
            let next = (index as isize + offset).rem_euclid(count) as usize;
            ctx.memory_mut(|memory| {
                // Egui decides default focus movement at the start of the pass.
                memory.move_focus(egui::FocusDirection::None);
                memory.request_focus(controls[next]);
            });
        } else if escape {
            // Egui releases the focus on Escape before widgets run. Keep
            // ownership while the caller shows the stopping state.
            ctx.memory_mut(|memory| memory.request_focus(controls[index]));
        }
        escape
    }
}

struct Display<'a> {
    note: &'a str,
    cents: Option<i16>,
    status: &'a str,
    detail: Option<&'a str>,
    color: egui::Color32,
    expires_in: Option<Duration>,
}

impl<'a> Display<'a> {
    fn new(state: TunerState<'a>, now: Instant) -> Self {
        let mut display = Self {
            note: "—",
            cents: None,
            status: "Waiting for reading",
            detail: None,
            color: theme::MUTED,
            expires_in: None,
        };
        match state {
            TunerState::Starting => display.status = "Starting tuner…",
            TunerState::Listening { reading: None } => display.status = "Pluck a string",
            TunerState::Listening {
                reading: Some(reading),
            } => {
                if let Some(age) = now.checked_duration_since(reading.received_at)
                    && age < READING_LIFETIME
                {
                    display.expires_in = Some(READING_LIFETIME - age);
                    match reading.note {
                        TunerNote::NoSignal => {
                            display.status = "No signal";
                            display.detail = Some("Pluck one string");
                        }
                        TunerNote::Named(note) if valid_note(note) => {
                            display.note = note;
                            display.cents = reading.cents;
                            if let Some(cents) = reading.cents {
                                (display.status, display.color) = if cents < -IN_TUNE {
                                    ("◀ Flat", theme::FOCUS)
                                } else if cents > IN_TUNE {
                                    ("Sharp ▶", theme::FOCUS)
                                } else {
                                    ("In tune", theme::OK)
                                };
                            }
                        }
                        _ => {}
                    }
                }
            }
            TunerState::Stopping => display.status = "Stopping tuner…",
            TunerState::Stopped => display.status = "Tuner stopped",
            TunerState::Disconnected => display.status = "PODxt Live disconnected",
            TunerState::Error(error) => {
                display.status = "Tuner error";
                display.color = theme::DANGER;
                display.detail = Some(error);
            }
        }
        display
    }

    fn announcement(&self) -> String {
        if self.cents.is_some() {
            return "Pitch detected".to_owned();
        }
        self.detail.map_or_else(
            || self.status.to_owned(),
            |detail| format!("{}. {detail}", self.status),
        )
    }

    fn description(&self) -> String {
        let status = self.status.trim_matches(['◀', '▶', ' ']);
        match (self.note, self.cents, self.detail) {
            (note, Some(cents), _) => format!("{note}, {}, {status}", signed_cents(cents)),
            (note, None, _) if note != "—" => format!("{note}, {status}"),
            (_, _, Some(detail)) => format!("{status}. {detail}"),
            _ => status.to_owned(),
        }
    }
}

fn valid_note(note: &str) -> bool {
    matches!(
        note,
        "A" | "Bb" | "B" | "C" | "Db" | "D" | "Eb" | "E" | "F" | "Gb" | "G" | "Ab"
    )
}

fn signed_cents(cents: i16) -> String {
    match cents.cmp(&0) {
        std::cmp::Ordering::Less => format!("−{} cents", cents.unsigned_abs()),
        std::cmp::Ordering::Equal => "0 cents".to_owned(),
        std::cmp::Ordering::Greater => format!("+{cents} cents"),
    }
}

fn draw_reading(ui: &mut egui::Ui, view: View, display: &Display<'_>) {
    ui.vertical_centered(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.label(RichText::new(display.note).strong().size(match view {
            View::Digital => 44.0,
            View::Analog => 31.0,
        }));
        match view {
            View::Digital => {
                let cents = display
                    .cents
                    .map_or_else(|| "— cents".to_owned(), signed_cents);
                ui.label(RichText::new(cents).monospace().size(23.0));
            }
            View::Analog => draw_meter(ui, display.cents, display.color),
        }
        if let Some(detail) = display.detail {
            ui.label(RichText::new(detail).color(theme::MUTED));
        }
    });
}

fn draw_meter(ui: &mut egui::Ui, cents: Option<i16>, color: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 69.0), egui::Sense::hover());
    let pivot = pos2(rect.center().x, rect.bottom() - 6.0);
    let horizontal_radius = (rect.width() * 0.36).min(104.0);
    let vertical_radius = 49.0;
    let point = |cents: f32, radius: f32| {
        let angle = cents / f32::from(METER_LIMIT) * 0.95;
        pivot
            + vec2(
                horizontal_radius * angle.sin(),
                -vertical_radius * angle.cos(),
            ) * radius
    };
    let painter = ui.painter();
    let arc = |left: i16, right: i16| {
        (left..=right)
            .map(|cents| point(f32::from(cents), 1.0))
            .collect()
    };
    painter.add(egui::Shape::line(
        arc(-METER_LIMIT, METER_LIMIT),
        Stroke::new(2.0_f32, theme::LINE),
    ));
    painter.add(egui::Shape::line(
        arc(-IN_TUNE, IN_TUNE),
        Stroke::new(4.0_f32, theme::OK),
    ));
    for tick in (-METER_LIMIT..=METER_LIMIT).step_by(10) {
        let end = point(f32::from(tick), 1.0);
        painter.line_segment(
            [point(f32::from(tick), 0.91), end],
            Stroke::new(1.0_f32, if tick == 0 { theme::OK } else { theme::MUTED }),
        );
    }
    for (tick, label, align, offset) in [
        (-50.0, "−50¢", egui::Align2::RIGHT_CENTER, vec2(-5.0, 0.0)),
        (0.0, "0", egui::Align2::CENTER_BOTTOM, vec2(0.0, -3.0)),
        (50.0, "+50¢", egui::Align2::LEFT_CENTER, vec2(5.0, 0.0)),
    ] {
        painter.text(
            point(tick, 1.0) + offset,
            align,
            label,
            egui::FontId::monospace(11.0),
            theme::MUTED,
        );
    }
    if let Some(cents) = cents {
        let tip = point(f32::from(cents.clamp(-METER_LIMIT, METER_LIMIT)), 0.87);
        painter.line_segment([pivot, tip], Stroke::new(2.5_f32, color));
        painter.circle_filled(pivot, 3.5, color);
    }
}

#[cfg(test)]
mod tests;
