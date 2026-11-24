//! Alternate-tuning panel from the approved mockups.
//!
//! The table is String, Shift, and Note, high string at the top when the
//! caller orders the rows that way. [`TuningRow::id`] comes back on each
//! event. Lock, Reset, the global ♭ / ♯ pair, and the −12…+12 neck report
//! events only. The caller owns the shifts, the note names, and the shift
//! text. There is no fretboard image and there are no inlay dots.

use eframe::egui::{
    self, Align2, CornerRadius, FontId, Margin, Pos2, Rect, RichText, Sense, Stroke, Vec2, vec2,
};

use super::theme::{
    self, AMBER, BODY, COMPACT, DIM, FOCUS, INK, LINE, MUTED, PANEL, RADIUS_CONTROL, RADIUS_PANEL,
    TARGET, TEXT,
};

pub const SEMITONE_MIN: i32 = -12;
pub const SEMITONE_MAX: i32 = 12;
/// Inset of the −12 and +12 lines from the neck ends, matching the mockup.
pub const SCALE_INSET: f32 = 18.0;
const NAME_W: f32 = 88.0;
const SHIFT_W: f32 = 148.0;
const NOTE_W: f32 = 40.0;
const TABLE_W: f32 = NAME_W + SHIFT_W + NOTE_W + 16.0;

const SCALE_LABELS: [(i32, &str); 7] = [
    (-12, "−12"),
    (-7, "−7"),
    (-5, "−5"),
    (0, "0"),
    (5, "+5"),
    (7, "+7"),
    (12, "+12"),
];

/// One string, already formatted by the caller.
pub struct TuningRow<'a> {
    /// Returned on events. The family uses its low-to-high string index.
    pub id: usize,
    pub name: &'a str,
    pub shift: i32,
    /// Shown in the shift column, for example `0 semitones`.
    pub shift_label: &'a str,
    pub note: &'a str,
}

/// What the user did. A marker drag is [`TuningEvent::DragStarted`], one or
/// more [`TuningEvent::Shift`] values, then [`TuningEvent::DragFinished`],
/// so the caller can keep the whole drag as one undo step. A click on the
/// lane is a single [`TuningEvent::Shift`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TuningEvent {
    Enable,
    Presets,
    Lock(bool),
    Reset,
    FlattenAll,
    SharpenAll,
    DragStarted { id: usize },
    Shift { id: usize, semitones: i32 },
    DragFinished { id: usize },
}

pub struct TuningPanel<'a> {
    rows: &'a [TuningRow<'a>],
    enabled: bool,
    interactive: bool,
    lock_enabled: bool,
    locked: bool,
    presets_enabled: bool,
    transpose_enabled: bool,
    reset_enabled: bool,
}

impl<'a> TuningPanel<'a> {
    pub fn new(rows: &'a [TuningRow<'a>]) -> Self {
        Self {
            rows,
            enabled: false,
            interactive: true,
            lock_enabled: false,
            locked: false,
            presets_enabled: true,
            transpose_enabled: true,
            reset_enabled: true,
        }
    }

    /// Alternate tuning is on. Markers stay visible either way.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn interactive(mut self, interactive: bool) -> Self {
        self.interactive = interactive;
        self
    }

    /// Lock is enabled only for a twelve-string body with six mix controls.
    pub fn lock(mut self, enabled: bool, locked: bool) -> Self {
        self.lock_enabled = enabled;
        self.locked = locked;
        self
    }

    pub fn presets_enabled(mut self, enabled: bool) -> Self {
        self.presets_enabled = enabled;
        self
    }

    pub fn transpose_enabled(mut self, enabled: bool) -> Self {
        self.transpose_enabled = enabled;
        self
    }

    pub fn reset_enabled(mut self, enabled: bool) -> Self {
        self.reset_enabled = enabled;
        self
    }

    pub fn show(self, ui: &mut egui::Ui) -> TuningOutput {
        let frame = egui::Frame::new()
            .fill(PANEL)
            .stroke(Stroke::new(1.0_f32, LINE))
            .corner_radius(CornerRadius::same(RADIUS_PANEL))
            .inner_margin(Margin::symmetric(12, 8));
        let inner = frame.show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(theme::SPACE_8, 0.0);
            ui.spacing_mut().button_padding = vec2(theme::SPACE_12, theme::SPACE_4);
            ui.spacing_mut().interact_size.y = TARGET;
            ui.horizontal_top(|ui| {
                let table = ui.vertical(|ui| {
                    ui.set_width(TABLE_W);
                    self.table(ui)
                });
                let column_height = table.response.rect.height().max(1.0);
                let (column, _) = ui.allocate_exact_size(
                    vec2(ui.available_width().max(48.0), column_height),
                    Sense::hover(),
                );
                let neck = neck_rect(column, &table.inner.rows);
                let (flat, sharp, flattened, sharpened) =
                    accidentals(ui, &self, neck, table.inner.tools_y);
                paint_neck(ui, neck, &table.inner.rows);
                let mut events = table.inner.events;
                if flattened {
                    events.push(TuningEvent::FlattenAll);
                }
                if sharpened {
                    events.push(TuningEvent::SharpenAll);
                }
                let mut rows = Vec::new();
                for row in &table.inner.rows {
                    let lane = Rect::from_center_size(
                        Pos2::new(neck.center().x, row.y),
                        vec2(neck.width(), TARGET),
                    );
                    if let Some(event) = lane_input(ui, &self, row, neck, lane) {
                        events.extend(event);
                    }
                    rows.push(TuningRowGeom {
                        id: row.id,
                        y: row.y,
                        lane,
                    });
                }
                paint_labels(ui, neck, table.inner.footer_y);
                TablePaint {
                    events,
                    enable: table.inner.enable,
                    presets: table.inner.presets,
                    lock: table.inner.lock,
                    reset: table.inner.reset,
                    flat,
                    sharp,
                    neck,
                    rows,
                }
            })
            .inner
        });
        let painted = inner.inner;
        TuningOutput {
            events: painted.events,
            enable: painted.enable,
            presets: painted.presets,
            lock: painted.lock,
            reset: painted.reset,
            flat: painted.flat,
            sharp: painted.sharp,
            neck: painted.neck,
            rows: painted.rows,
        }
    }
}

pub struct TuningOutput {
    pub events: Vec<TuningEvent>,
    pub enable: Rect,
    pub presets: Rect,
    pub lock: Rect,
    pub reset: Rect,
    pub flat: Rect,
    pub sharp: Rect,
    pub neck: Rect,
    pub rows: Vec<TuningRowGeom>,
}

pub struct TuningRowGeom {
    pub id: usize,
    pub y: f32,
    pub lane: Rect,
}

struct LaidRow {
    id: usize,
    y: f32,
    low: bool,
}

struct TableInner {
    events: Vec<TuningEvent>,
    enable: Rect,
    presets: Rect,
    lock: Rect,
    reset: Rect,
    tools_y: f32,
    footer_y: f32,
    rows: Vec<LaidRow>,
}

struct TablePaint {
    events: Vec<TuningEvent>,
    enable: Rect,
    presets: Rect,
    lock: Rect,
    reset: Rect,
    flat: Rect,
    sharp: Rect,
    neck: Rect,
    rows: Vec<TuningRowGeom>,
}

impl TuningPanel<'_> {
    fn table(&self, ui: &mut egui::Ui) -> TableInner {
        let mut events = Vec::new();
        let tools = ui.horizontal(|ui| {
            ui.set_min_height(TARGET);
            let enable = text_button(
                ui,
                "Enable",
                self.interactive,
                self.enabled,
                if self.enabled {
                    "Turn off alternate tuning"
                } else {
                    "Turn on alternate tuning"
                },
            );
            if enable.clicked() {
                events.push(TuningEvent::Enable);
            }
            let presets = text_button(
                ui,
                "Presets",
                self.interactive && self.presets_enabled,
                false,
                "",
            );
            if presets.clicked() {
                events.push(TuningEvent::Presets);
            }
            (enable.rect, presets.rect, ui.min_rect().center().y)
        });
        ui.add_space(4.0);
        header(ui);
        let mut rows = Vec::new();
        for (index, row) in self.rows.iter().enumerate() {
            let y = string_row(ui, row);
            rows.push(LaidRow {
                id: row.id,
                y,
                low: index + 2 >= self.rows.len() && self.rows.len() >= 2,
            });
        }
        ui.add_space(4.0);
        let footer = ui.horizontal(|ui| {
            ui.set_min_height(TARGET);
            let mut locked = self.locked;
            let can_lock = self.interactive && self.lock_enabled;
            let lock = ui
                .add_enabled(can_lock, egui::Checkbox::new(&mut locked, "Lock"))
                .on_hover_text("Link all twelve-string mix controls")
                .on_disabled_hover_text(
                    "Mix lock requires a twelve-string body and six mix controls",
                );
            if can_lock && locked != self.locked {
                events.push(TuningEvent::Lock(locked));
            }
            let reset = text_button(
                ui,
                "Reset",
                self.interactive && self.reset_enabled,
                false,
                "Return all six strings to standard tuning",
            );
            if reset.clicked() {
                events.push(TuningEvent::Reset);
            }
            (lock.rect, reset.rect, ui.min_rect().center().y)
        });
        TableInner {
            events,
            enable: tools.inner.0,
            presets: tools.inner.1,
            lock: footer.inner.0,
            reset: footer.inner.1,
            tools_y: tools.inner.2,
            footer_y: footer.inner.2,
            rows,
        }
    }
}

fn header(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.set_min_height(18.0);
        for (width, label) in [(NAME_W, "String"), (SHIFT_W, "Shift"), (NOTE_W, "Note")] {
            let (rect, _) = ui.allocate_exact_size(vec2(width, 18.0), Sense::hover());
            ui.painter().text(
                rect.left_center(),
                Align2::LEFT_CENTER,
                label,
                FontId::proportional(COMPACT),
                DIM,
            );
        }
    });
}

fn string_row(ui: &mut egui::Ui, row: &TuningRow<'_>) -> f32 {
    let line = ui.horizontal(|ui| {
        ui.set_min_height(TARGET);
        cell(ui, NAME_W, row.name);
        let (shift, response) = ui.allocate_exact_size(vec2(SHIFT_W, TARGET), Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(shift, RADIUS_CONTROL, theme::RAISED);
        painter.rect_stroke(
            shift,
            RADIUS_CONTROL,
            Stroke::new(1.0_f32, LINE),
            egui::StrokeKind::Inside,
        );
        painter.text(
            shift.left_center() + vec2(8.0, 0.0),
            Align2::LEFT_CENTER,
            row.shift_label,
            FontId::proportional(BODY),
            TEXT,
        );
        response.on_hover_text("Drag or click to set semitones");
        cell(ui, NOTE_W, row.note);
        ui.min_rect().center().y
    });
    line.inner
}

fn cell(ui: &mut egui::Ui, width: f32, text: &str) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, TARGET), Sense::hover());
    ui.painter().text(
        rect.left_center(),
        Align2::LEFT_CENTER,
        text,
        FontId::proportional(COMPACT),
        TEXT,
    );
}

fn text_button(
    ui: &mut egui::Ui,
    label: &str,
    enabled: bool,
    selected: bool,
    tip: &str,
) -> egui::Response {
    let response = ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(label).size(BODY))
            .min_size(vec2(TARGET, TARGET))
            .selected(selected),
    );
    if tip.is_empty() {
        response
    } else {
        response.on_hover_text(tip).on_disabled_hover_text(tip)
    }
}

/// Horizontal center of one semitone. −12 and +12 sit `SCALE_INSET` in from
/// the neck, and 0 is the center.
pub fn scale_x(neck: Rect, semitone: i32) -> f32 {
    let semitone = semitone.clamp(SEMITONE_MIN, SEMITONE_MAX);
    let left = neck.left() + SCALE_INSET;
    let width = (neck.width() - SCALE_INSET * 2.0).max(1.0);
    left + width * (semitone - SEMITONE_MIN) as f32 / (SEMITONE_MAX - SEMITONE_MIN) as f32
}

/// Nearest semitone to a screen x, clamped to −12…+12.
pub fn semitone_at(neck: Rect, x: f32) -> i32 {
    let left = neck.left() + SCALE_INSET;
    let width = (neck.width() - SCALE_INSET * 2.0).max(1.0);
    if !x.is_finite() {
        return 0;
    }
    let t = (x - left) / width * (SEMITONE_MAX - SEMITONE_MIN) as f32 + SEMITONE_MIN as f32;
    t.round().clamp(SEMITONE_MIN as f32, SEMITONE_MAX as f32) as i32
}

/// The 14px marker, with a 32px hit target so it meets the control size.
pub fn marker_center(neck: Rect, row_y: f32, semitone: i32) -> Pos2 {
    Pos2::new(scale_x(neck, semitone), row_y)
}

pub fn marker_hit(pointer: Pos2, center: Pos2) -> bool {
    pointer.is_finite() && center.is_finite() && pointer.distance(center) <= TARGET * 0.5
}

fn neck_rect(column: Rect, rows: &[LaidRow]) -> Rect {
    let Some(first) = rows.first() else {
        return Rect::from_min_size(
            Pos2::new(column.left(), column.top() + TARGET),
            Vec2::new(column.width(), (column.height() - TARGET).max(TARGET)),
        );
    };
    let last = rows.last().unwrap_or(first);
    Rect::from_min_max(
        Pos2::new(column.left(), first.y - TARGET * 0.5),
        Pos2::new(column.right(), last.y + TARGET * 0.5),
    )
}

fn accidentals(
    ui: &mut egui::Ui,
    panel: &TuningPanel<'_>,
    neck: Rect,
    tools_y: f32,
) -> (Rect, Rect, bool, bool) {
    let enabled = panel.interactive && panel.transpose_enabled;
    let zero = scale_x(neck, 0);
    let (flat, flattened) = place_symbol(
        ui,
        Rect::from_center_size(Pos2::new(zero - 20.0, tools_y), Vec2::splat(TARGET)),
        "♭",
        enabled,
        "Lower every string by one semitone",
    );
    let (sharp, sharpened) = place_symbol(
        ui,
        Rect::from_center_size(Pos2::new(zero + 20.0, tools_y), Vec2::splat(TARGET)),
        "♯",
        enabled,
        "Raise every string by one semitone",
    );
    (flat, sharp, flattened, sharpened)
}

fn place_symbol(
    ui: &mut egui::Ui,
    rect: Rect,
    label: &str,
    enabled: bool,
    tip: &str,
) -> (Rect, bool) {
    let mut builder = egui::UiBuilder::new().id_salt(label).max_rect(rect).layout(
        egui::Layout::centered_and_justified(egui::Direction::TopDown),
    );
    if !enabled {
        builder = builder.disabled();
    }
    let response = ui
        .new_child(builder)
        .add_sized(
            rect.size(),
            egui::Button::new(RichText::new(label).size(18.0)).min_size(rect.size()),
        )
        .on_hover_text(tip)
        .on_disabled_hover_text(tip);
    (response.rect, enabled && response.clicked())
}

fn paint_neck(ui: &egui::Ui, neck: Rect, rows: &[LaidRow]) {
    let painter = ui.painter();
    for semitone in SEMITONE_MIN..=SEMITONE_MAX {
        let x = scale_x(neck, semitone);
        let (width, color) = if semitone == 0 {
            (2.0_f32, FOCUS)
        } else {
            (1.0_f32, LINE)
        };
        painter.vline(x, neck.y_range(), Stroke::new(width, color));
    }
    for row in rows {
        painter.hline(
            (neck.left() + SCALE_INSET)..=(neck.right() - SCALE_INSET),
            row.y,
            Stroke::new(if row.low { 2.0_f32 } else { 1.0_f32 }, MUTED),
        );
    }
}

fn paint_labels(ui: &egui::Ui, neck: Rect, footer_y: f32) {
    let painter = ui.painter();
    for (semitone, label) in SCALE_LABELS {
        painter.text(
            Pos2::new(scale_x(neck, semitone), footer_y),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(COMPACT),
            if semitone == 0 { FOCUS } else { MUTED },
        );
    }
}

#[derive(Clone, Copy)]
struct LaneDrag {
    last: i32,
}

fn lane_input(
    ui: &mut egui::Ui,
    panel: &TuningPanel<'_>,
    row: &LaidRow,
    neck: Rect,
    lane: Rect,
) -> Option<Vec<TuningEvent>> {
    let stored = panel.rows.iter().find(|item| item.id == row.id);
    let current = stored.map(|item| item.shift).unwrap_or(0);
    // The marker is painted at the stored shift. Repaint it here so a
    // non-zero shift is not left on 0 by paint_neck.
    if let Some(item) = stored {
        let painter = ui.painter();
        painter.circle(
            marker_center(neck, row.y, item.shift.clamp(SEMITONE_MIN, SEMITONE_MAX)),
            7.0,
            AMBER,
            Stroke::new(2.0_f32, INK),
        );
        painter.circle_stroke(
            marker_center(neck, row.y, item.shift.clamp(SEMITONE_MIN, SEMITONE_MAX)),
            8.0,
            Stroke::new(1.0_f32, FOCUS),
        );
    }
    let id = ui.id().with(("tuning-lane", row.id));
    let sense = if panel.interactive {
        Sense::click_and_drag()
    } else {
        Sense::hover()
    };
    let response = ui.interact(lane, id, sense).on_hover_text(format!(
        "Drag left or right to shift {}",
        stored.map(|item| item.name).unwrap_or("this string")
    ));
    if !panel.interactive {
        if ui.data(|data| data.get_temp::<LaneDrag>(id)).is_some() {
            ui.data_mut(|data| data.remove::<LaneDrag>(id));
            return Some(vec![TuningEvent::DragFinished { id: row.id }]);
        }
        return None;
    }
    let mut events = Vec::new();
    let pointer = response
        .interact_pointer_pos()
        .or_else(|| ui.input(|input| input.pointer.press_origin()));
    if let Some(mut drag) = ui.data(|data| data.get_temp::<LaneDrag>(id)) {
        if response.dragged()
            && let Some(pos) = pointer
        {
            push_shift(&mut events, &mut drag, row.id, semitone_at(neck, pos.x));
            ui.data_mut(|data| data.insert_temp(id, drag));
        }
        if response.drag_stopped() || !ui.input(|input| input.pointer.primary_down()) {
            ui.data_mut(|data| data.remove::<LaneDrag>(id));
            events.push(TuningEvent::DragFinished { id: row.id });
        }
        return Some(events).filter(|events| !events.is_empty());
    }
    if response.drag_started() {
        let mut drag = LaneDrag { last: current };
        events.push(TuningEvent::DragStarted { id: row.id });
        if let Some(pos) = pointer {
            push_shift(&mut events, &mut drag, row.id, semitone_at(neck, pos.x));
        }
        ui.data_mut(|data| data.insert_temp(id, drag));
        return Some(events);
    }
    if response.clicked()
        && let Some(pos) = pointer
    {
        let semitones = semitone_at(neck, pos.x);
        if semitones != current {
            events.push(TuningEvent::Shift {
                id: row.id,
                semitones,
            });
        }
    }
    Some(events).filter(|events| !events.is_empty())
}

fn push_shift(events: &mut Vec<TuningEvent>, drag: &mut LaneDrag, id: usize, semitones: i32) {
    if semitones != drag.last {
        drag.last = semitones;
        events.push(TuningEvent::Shift { id, semitones });
    }
}
