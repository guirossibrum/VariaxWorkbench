//! Guided physical actions 7.1–7.5 (#260).
//!
//! Each action opens a listener: nothing is sent while the person
//! acts (a selector message during a read stopped a #238 run). Detection
//! completes the current step on its own, so the person does not have to
//! walk back to the computer, but the next action waits for their OK. They
//! can also choose Continue to check what arrived, or Stop the run.

use super::differences::{self, Severity};
use super::practice::{UserAction, preset_label};
use super::procedures::{
    same_program, variax_full_read, with_podxt, with_podxt_stopping, with_variax,
    with_variax_progress,
};
use super::runner::{Ctx, Decision, Progress, Retry, SKIPPED, STOPPED};
use super::variax_reads::other_preset;
use line6_devices::family::podxt::{self, BankKind, Location};
use line6_devices::family::variax::{self, FollowUpdate, Preset};
use line6_devices::gate::Operation;
use line6_devices::session::{Link, Session};
use std::cell::Cell;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
use std::time::{Duration, Instant};

pub const READY_DELAY: Duration = Duration::from_secs(10);
pub const ACTION_WINDOW: Duration = Duration::from_secs(60);
const PRACTICE_DELAY: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub struct ActionClock {
    start: Instant,
    extra: AtomicU64,
    observation: Mutex<Option<String>>,
}

impl Default for ActionClock {
    fn default() -> Self {
        Self {
            start: Instant::now(),
            extra: AtomicU64::new(0),
            observation: Mutex::new(None),
        }
    }
}

impl ActionClock {
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    pub fn ready_duration(&self) -> Duration {
        READY_DELAY + Duration::from_secs(self.extra.load(Ordering::Relaxed))
    }

    pub fn add_ten_seconds(&self) {
        if self.elapsed() < self.ready_duration() {
            self.extra.fetch_add(10, Ordering::Relaxed);
        }
    }

    pub fn observe(&self, observation: impl Into<String>) {
        *self.observation.lock().expect("action observation") = Some(observation.into());
    }

    pub fn observation(&self) -> Option<String> {
        self.observation.lock().expect("action observation").clone()
    }
}

pub(super) struct ActiveAction {
    pub clock: Arc<ActionClock>,
    advance: Box<dyn Fn() -> bool>,
    elapsed: Box<dyn Fn() -> Duration>,
}

impl ActiveAction {
    pub fn done(&self) -> bool {
        (self.advance)() || self.expired()
    }

    pub fn expired(&self) -> bool {
        (self.elapsed)() >= self.clock.ready_duration() + ACTION_WINDOW
    }

    pub fn early(&self) -> bool {
        (self.elapsed)() < self.clock.ready_duration()
    }
}

const VARIAX_LISTEN: &[Operation] = &[Operation::VariaxIdentity, Operation::VariaxRead];
const PODXT_LISTEN: &[Operation] = &[Operation::PodxtIdentity, Operation::PodxtRead];

/// Use the editor's decoded Variax Follow stream until the action is observed.
pub(super) fn watch_variax<L: Link>(
    session: &mut Session<L>,
    stopping: &dyn Fn() -> bool,
    action: &ActiveAction,
    mut accepts: impl FnMut(FollowUpdate) -> bool,
) -> Result<bool, String> {
    let seen = Cell::new(false);
    variax::follow_action(
        session,
        || stopping() || seen.get() || action.done(),
        |update| {
            let description = match update {
                FollowUpdate::Connected => "the guitar connected".into(),
                FollowUpdate::Disconnected => "the guitar disconnected".into(),
                FollowUpdate::ProgramChange(preset) => format!("preset {}", preset_label(preset)),
                FollowUpdate::Volume(value) => format!("guitar volume {value}"),
                FollowUpdate::TonePosition(value) => format!("guitar tone {value}"),
                _ => return,
            };
            action.clock.observe(description);
            seen.set(accepts(update));
        },
    )?;
    if stopping() {
        return Err(format!("{STOPPED}Stopped by you."));
    }
    Ok(seen.get())
}

/// Show the instruction, then let the person arm the listener with OK.
pub(super) fn begin_action(
    ctx: &mut Ctx<'_>,
    see: &str,
    hear: &str,
    what: &str,
    actions: &[UserAction],
) -> Result<ActiveAction, String> {
    if ctx.stopping() {
        return Err(format!("{STOPPED}Stopped by you."));
    }
    match ctx.op.prepare_action(ctx.rec.id, see, hear) {
        Decision::Run => {}
        Decision::Skip => return Err(format!("{SKIPPED}You skipped {what}.")),
        Decision::Stop => return Err(format!("{STOPPED}You stopped before {what}.")),
    }
    ctx.op.instruct(ctx.rec.id, see, hear);
    let clock = Arc::new(ActionClock::default());
    ctx.op
        .progress(Progress::watching(format!("Watching for {what}")));
    let advance = ctx.op.watch_action(ctx.rec.id, what, clock.clone());
    let elapsed: Box<dyn Fn() -> Duration> = if let Some(rig) = ctx.port.simulation() {
        let model = rig.borrow();
        let start = model.clock;
        let delay = model
            .faults
            .guided_action_delay
            .unwrap_or(READY_DELAY + PRACTICE_DELAY);
        drop(model);
        for action in actions {
            ctx.port.user_acts(action.clone(), delay);
        }
        Box::new(move || rig.borrow().clock.saturating_sub(start))
    } else {
        let clock = clock.clone();
        Box::new(move || clock.elapsed())
    };
    Ok(ActiveAction {
        clock,
        advance,
        elapsed,
    })
}

/// A by-ear sound comparison is confirmed by the player, without audio capture.
pub(super) fn play_phrase(ctx: &mut Ctx<'_>, see: &str, hear: &str) -> Result<(), String> {
    loop {
        let action = begin_action(ctx, see, hear, "you to finish playing", &[])?;
        if ctx.mode == super::record::Mode::Practice {
            return Ok(());
        }
        while !ctx.stopping() && !action.done() {
            std::thread::sleep(Duration::from_millis(25));
        }
        if ctx.stopping() {
            return Err(format!("{STOPPED}You stopped while playing."));
        }
        if !action.expired() {
            return Ok(());
        }
        missed(
            ctx,
            "Continue after you finished playing",
            "no confirmation",
        )?;
    }
}

pub(super) fn wait_until_ready(ctx: &Ctx<'_>, action: &ActiveAction) -> Result<(), String> {
    if ctx.mode == super::record::Mode::Practice {
        return Ok(());
    }
    while action.early() && !ctx.stopping() && !action.done() {
        std::thread::sleep(Duration::from_millis(25));
    }
    if ctx.stopping() || action.done() {
        return Err(format!("{STOPPED}You stopped before the action started."));
    }
    Ok(())
}

pub(super) fn missed(ctx: &mut Ctx<'_>, what: &str, seen: &str) -> Result<(), String> {
    let observation = format!("Expected {what}. Saw {seen}.");
    ctx.rec.note(observation.clone());
    match ctx.op.not_seen(ctx.rec.id, &observation) {
        Retry::TryAgain => {
            ctx.rec.note("You chose Try again.");
            Ok(())
        }
        Retry::Skip => Err(format!("{SKIPPED}{observation} You chose Skip.")),
        Retry::Stop => Err(format!("{STOPPED}{observation} You chose Stop.")),
    }
}

/// One guided action with retries on the person's request.
fn guided<T>(
    ctx: &mut Ctx<'_>,
    see: &str,
    hear: &str,
    what: &str,
    practice: &[UserAction],
    mut already: impl FnMut(&mut Ctx<'_>) -> Result<Option<T>, String>,
    mut window: impl FnMut(&mut Ctx<'_>, &ActiveAction) -> Result<Option<T>, String>,
) -> Result<T, String> {
    loop {
        if let Some(current) = already(ctx)? {
            ctx.rec
                .show(format!("Already in the expected state for {what}."));
            return Ok(current);
        }
        let advance = begin_action(ctx, see, hear, what, practice)?;
        let before = ctx.rec.evidence.len();
        if let Some(found) = window(ctx, &advance)? {
            let timing = if advance.early() {
                " during the get-ready countdown"
            } else {
                " during the action window"
            };
            ctx.rec.note(format!("Seen: {what}{timing}."));
            return Ok(found);
        }
        let seen = ctx
            .rec
            .evidence
            .get(before..)
            .and_then(|notes| notes.last())
            .cloned()
            .or_else(|| advance.clock.observation())
            .unwrap_or_else(|| "no matching change".into());
        missed(ctx, what, &seen)?;
    }
}

/// Read the real selector location from its two Follow notices. The backup
/// sweeps the logical presets without moving the physical selector.
pub(super) fn starting_variax_selector(
    ctx: &mut Ctx<'_>,
    practice_start: Option<Preset>,
) -> Result<Preset, String> {
    let observe = |ctx: &mut Ctx<'_>, action: &ActiveAction, except| {
        let mut selected = None;
        with_variax(ctx, VARIAX_LISTEN, |session, stopping| {
            watch_variax(session, stopping, action, |update| {
                if let FollowUpdate::ProgramChange(preset) = update
                    && Some(preset) != except
                {
                    selected = Some(preset);
                }
                selected.is_some()
            })
        })?;
        Ok(selected)
    };
    let away_actions: Vec<_> = practice_start
        .map(super::variax_reads::other_preset)
        .map(UserAction::TurnSelector)
        .into_iter()
        .collect();
    let away = guided(
        ctx,
        "Move the Variax selector one position away from where it was at the start. Leave the bank knob where it was.",
        "The guitar may change sound.",
        "the selector moving away",
        &away_actions,
        |_| Ok(None),
        |ctx, action| observe(ctx, action, None),
    )?;
    let return_actions: Vec<_> = practice_start
        .map(UserAction::TurnSelector)
        .into_iter()
        .collect();
    guided(
        ctx,
        "Move the Variax selector back to its original position.",
        "The saved sound for that position. Rig Check has kept a copy of your starting sound.",
        "the selector returning",
        &return_actions,
        |_| Ok(None),
        |ctx, action| observe(ctx, action, Some(away)),
    )
}

fn already_on_variax(
    ctx: &mut Ctx<'_>,
    expected: &[u32; super::practice::PROGRAM_WORDS],
) -> Result<Option<()>, String> {
    let read = with_variax_progress(ctx, VARIAX_LISTEN, |session, stopping, progress| {
        super::procedures::drain(session)?;
        variax_full_read(
            session,
            stopping,
            progress,
            "Checking the guitar's current program",
        )
    })?;
    let observed_checksum = read.words[super::procedures::CHECKSUM_WORD];
    let computed_checksum = super::procedures::firmware_checksum(&read.words);
    if observed_checksum != computed_checksum {
        let reason = "The Variax selector preflight failed the firmware checksum.";
        ctx.rec.difference(
            Severity::Stop,
            reason,
            vec![differences::checksum_line(
                computed_checksum,
                observed_checksum,
            )],
        );
        return Err(reason.into());
    }
    let matches = ctx
        .data
        .variax_library
        .iter()
        .filter(|(_, words)| same_program(&read.words, words))
        .count();
    Ok((matches == 1 && same_program(&read.words, expected)).then_some(()))
}

fn variax_program_window(
    ctx: &mut Ctx<'_>,
    target: Preset,
    advance: &ActiveAction,
) -> Result<Option<()>, String> {
    let seen = with_variax(ctx, VARIAX_LISTEN, |session, stopping| {
        watch_variax(session, stopping, advance, |update| {
            update == FollowUpdate::ProgramChange(target)
        })
    })?;
    Ok(seen.then_some(()))
}

/// 7.1: the person turns the selector to a named preset and back.
pub fn selector(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let start = ctx
        .data
        .start_preset
        .ok_or("The starting preset is unknown (3.4).")?;
    let start_program = ctx
        .data
        .exercise_program()
        .ok_or("No starting program (3.3).")?;
    let target = other_preset(start);
    let name = preset_label(target);
    let backup = ctx
        .data
        .variax_library
        .iter()
        .find(|(preset, _)| *preset == target)
        .map(|(_, words)| *words)
        .ok_or("The target preset is missing from the backup.")?;
    guided(
        ctx,
        &format!("Turn the Variax selector to {name}."),
        &format!("{name}."),
        &format!("program change to {name}"),
        &[UserAction::TurnSelector(target)],
        |ctx| already_on_variax(ctx, &backup),
        |ctx, advance| variax_program_window(ctx, target, advance),
    )?;
    let read = with_variax_progress(ctx, VARIAX_LISTEN, |session, stopping, progress| {
        super::procedures::drain(session)?;
        variax_full_read(session, stopping, progress, &format!("Checking {name}"))
    })?;
    if differences::program(
        ctx.rec,
        "Guided Variax recall",
        &backup,
        &read.words,
        Severity::Stop,
        true,
    ) {
        ctx.rec.show(format!(
            "{name} plays the program from the 3.4 backup (1,092 words)."
        ));
    } else {
        return Err(format!("{name} does not match the 3.4 backup."));
    }
    let start_name = preset_label(start);
    guided(
        ctx,
        &format!("Turn the Variax selector back to {start_name}."),
        &format!("{start_name}, your starting sound."),
        &format!("program change back to {start_name}"),
        &[UserAction::TurnSelector(start)],
        |ctx| already_on_variax(ctx, &start_program),
        |ctx, advance| variax_program_window(ctx, start, advance),
    )?;
    let back = with_variax_progress(ctx, VARIAX_LISTEN, |session, stopping, progress| {
        super::procedures::drain(session)?;
        variax_full_read(
            session,
            stopping,
            progress,
            &format!("Verifying return to {start_name}"),
        )
    })?;
    if !differences::program(
        ctx.rec,
        "Guided Variax return",
        &start_program,
        &back.words,
        Severity::Stop,
        true,
    ) {
        return Err(format!(
            "Back on {start_name}, but the program differs from the start."
        ));
    }
    ctx.rec.show(format!(
        "Back on {start_name}: all 1,092 words match the start."
    ));
    Ok(())
}

#[derive(Default, Clone, Copy)]
struct Range {
    min: Option<u8>,
    max: Option<u8>,
}

const SWEEP_LOW: u8 = 20;
const SWEEP_HIGH: u8 = 107;

impl Range {
    fn add(&mut self, value: u8) {
        self.min = Some(self.min.map_or(value, |m| m.min(value)));
        self.max = Some(self.max.map_or(value, |m| m.max(value)));
    }

    fn full_range(self) -> bool {
        self.min == Some(0) && self.max == Some(127)
    }

    /// Swept far enough to count as a sweep.
    fn swept(self) -> bool {
        self.min.is_some_and(|m| m <= SWEEP_LOW) && self.max.is_some_and(|m| m >= SWEEP_HIGH)
    }

    fn text(self) -> String {
        match (self.min, self.max) {
            (Some(min), Some(max)) => format!("{min}-{max}"),
            _ => "not seen".into(),
        }
    }
}

/// 7.2: the person sweeps the Variax volume (CC 7) and tone (CC 79) knobs.
pub fn knobs(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let (volume, tone) = guided(
        ctx,
        "On the Variax, turn volume down and up, then tone down and up. Leave them where they were.",
        "The volume and the tone change while you turn.",
        "volume (CC 7) and tone (CC 79) sweeps",
        &[UserAction::SweepVariaxKnobs],
        |ctx| {
            with_variax(ctx, VARIAX_LISTEN, |session, _| {
                variax::procedures::identify(session)
            })?;
            Ok(None)
        },
        |ctx, advance| {
            let mut volume = Range::default();
            let mut tone = Range::default();
            let seen = with_variax(ctx, VARIAX_LISTEN, |session, stopping| {
                watch_variax(session, stopping, advance, |update| {
                    match update {
                        FollowUpdate::Volume(value) => volume.add(value),
                        FollowUpdate::TonePosition(value) => tone.add(value),
                        _ => {}
                    }
                    volume.full_range() && tone.full_range()
                })
            })?;
            ctx.rec.note(format!(
                "Volume (CC 7) range {}, tone (CC 79) range {}.",
                volume.text(),
                tone.text()
            ));
            Ok(seen.then_some((volume, tone)))
        },
    )?;
    ctx.rec.show(format!(
        "Both knobs swept: volume {}, tone {}.",
        volume.text(),
        tone.text()
    ));
    Ok(())
}

fn podxt_window(
    ctx: &mut Ctx<'_>,
    advance: &ActiveAction,
    mut accepts: impl FnMut(podxt::ReadUpdate) -> bool,
) -> Result<bool, String> {
    let op = &*ctx.op;
    let step = ctx.rec.id;
    let mut lines = super::procedures::Lines::default();
    let seen = Cell::new(false);
    let policy = super::procedures::StepPolicy {
        step,
        mode: ctx.mode,
        ops: PODXT_LISTEN,
    };
    let result = match ctx.port.podxt(step) {
        Ok(link) => {
            let mut session = Session::new(link, &policy, &mut lines);
            podxt::follow_action(
                &mut session,
                || op.stop_requested() || seen.get() || advance.done(),
                |update| {
                    match &update {
                        podxt::ReadUpdate::Channel(location) => advance
                            .clock
                            .observe(format!("PODxt channel {}", location.label())),
                        podxt::ReadUpdate::Controller { number, value } => {
                            let name = match *number {
                                4 | 7 => "pedal",
                                _ => "control",
                            };
                            advance.clock.observe(format!("PODxt {name} {value}"));
                        }
                        _ => {}
                    }
                    seen.set(accepts(update));
                },
            )
        }
        Err(error) => Err(error),
    };
    super::procedures::absorb(ctx, lines);
    match result {
        Ok(()) => Ok(seen.get()),
        Err(error) if error == "PODxt read cancelled." && !ctx.stopping() => Ok(seen.get()),
        Err(error) => Err(error),
    }
}

fn already_on_podxt(
    ctx: &mut Ctx<'_>,
    target: Location,
    expected: &podxt::EditBuffer,
) -> Result<Option<podxt::EditBuffer>, String> {
    const CHANNEL: &[Operation] = &[
        Operation::PodxtIdentity,
        Operation::PodxtRead,
        Operation::PodxtGlobalRead,
    ];
    with_podxt(ctx, CHANNEL, |session| {
        let playing = podxt::read_current(session)?.tone;
        let channel = podxt::playing_channel(session)?;
        Ok((channel == target && playing.same_tone(expected)).then_some(playing))
    })
}

fn channel_window(
    ctx: &mut Ctx<'_>,
    target: Location,
    expected_name: &str,
    advance: &ActiveAction,
) -> Result<Option<podxt::EditBuffer>, String> {
    let mut primed = false;
    let mut matching_channel = false;
    let mut readback = None;
    podxt_window(ctx, advance, |update| match update {
        podxt::ReadUpdate::Channel(location) if primed => {
            matching_channel = location == target;
            false
        }
        podxt::ReadUpdate::Current(tone) => {
            if primed && matching_channel {
                readback = Some(tone);
                true
            } else {
                primed = true;
                false
            }
        }
        _ => false,
    })?;
    match readback {
        Some(tone) if tone.name() != expected_name => Err(format!(
            "Follow re-read channel {} as {:?}, expected {expected_name:?}.",
            target.label(),
            tone.name()
        )),
        result => Ok(result),
    }
}

/// 7.3: the person switches the PODxt Live to a named channel and back.
pub fn footswitch(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let start = ctx
        .data
        .start_channel
        .ok_or("The starting channel is unknown (2.5).")?;
    let start_tone = ctx
        .data
        .podxt_library
        .iter()
        .find(|(location, _)| *location == start)
        .ok_or("Starting channel is missing from the backup.")?
        .1
        .clone();
    let target = Location::new(
        BankKind::Tones,
        (start.index() & !3) + (start.index() + 1) % 4,
    )
    .expect("same bank");
    let stored = ctx
        .data
        .podxt_library
        .iter()
        .find(|(loc, _)| *loc == target)
        .map(|(_, tone)| tone.clone())
        .ok_or("The target channel is missing from the backup.")?;
    let now = guided(
        ctx,
        &format!(
            "On the PODxt Live, press the footswitch for channel {}.",
            target.label()
        ),
        &format!("Channel {}.", target.label()),
        &format!("channel change to {}", target.label()),
        &[UserAction::PodxtFootswitch(target.index())],
        |ctx| already_on_podxt(ctx, target, &stored),
        |ctx, advance| channel_window(ctx, target, &stored.name(), advance),
    )?;
    if super::differences::tone(
        ctx.rec,
        "PODxt guided recall",
        &stored.raw,
        &now.raw,
        super::differences::Severity::Stop,
        true,
    ) != 0
    {
        return Err(format!(
            "Channel {} does not match the 2.5 backup.",
            target.label()
        ));
    }
    ctx.rec.show(format!(
        "Channel {} plays its stored tone {:?}.",
        target.label(),
        now.name()
    ));
    let back = guided(
        ctx,
        &format!(
            "On the PODxt Live, press the footswitch for channel {} again.",
            start.label()
        ),
        &format!("Channel {}, your starting tone.", start.label()),
        &format!("channel change back to {}", start.label()),
        &[UserAction::PodxtFootswitch(start.index())],
        |ctx| already_on_podxt(ctx, start, &start_tone),
        |ctx, advance| channel_window(ctx, start, &start_tone.name(), advance),
    )?;
    if super::differences::tone(
        ctx.rec,
        "PODxt guided return",
        &start_tone.raw,
        &back.raw,
        super::differences::Severity::Stop,
        true,
    ) != 0
    {
        return Err(format!(
            "Back on {}, but the tone differs from the start.",
            start.label()
        ));
    }
    ctx.rec.show(format!(
        "Back on channel {}: the starting tone.",
        start.label()
    ));
    if ctx.data.podxt_unsaved {
        super::live::restore_podxt_playing(ctx)?;
    }
    ctx.rec.look_at(
        "Check that the PODxt Live display shows the starting channel again.",
        start.label(),
    );
    Ok(())
}

/// 7.4: the person rocks the PODxt Live pedal (CC 7, or CC 4 in wah mode).
pub fn pedal(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let range = guided(
        ctx,
        "Rock the PODxt Live pedal from heel to toe and back, then leave it where it was.",
        "The volume or the wah follows the pedal.",
        "pedal sweep (CC 7 or CC 4)",
        &[UserAction::MovePedal],
        |ctx| {
            with_podxt(ctx, PODXT_LISTEN, |session| {
                podxt::read_current(session).map(|current| current.tone)
            })?;
            Ok(None)
        },
        |ctx, advance| {
            let mut range = Range::default();
            let seen = podxt_window(ctx, advance, |event| {
                if let podxt::ReadUpdate::Controller {
                    number: 7 | 4,
                    value,
                } = event
                {
                    range.add(value);
                }
                range.swept()
            })?;
            ctx.rec.note(format!("Pedal range: {}", range.text()));
            Ok(seen.then_some(range))
        },
    )?;
    ctx.rec.show(format!("Pedal range seen: {}.", range.text()));
    Ok(())
}

pub(super) const PODXT_KNOBS: [(u8, &str); 6] = [
    (13, "Drive"),
    (14, "Bass"),
    (15, "Mid"),
    (16, "Treble"),
    (21, "Presence"),
    (17, "Channel Volume"),
];

fn knob_result((cc, name): (u8, &str), range: Range) -> String {
    let missing = match (range.min, range.max) {
        (None, _) => " · no movement detected",
        (Some(low), Some(high)) if low > SWEEP_LOW && high < SWEEP_HIGH => {
            " · both near-ends missing (≤20 and ≥107)"
        }
        (Some(low), _) if low > SWEEP_LOW => " · low near-end missing (≤20)",
        (_, Some(high)) if high < SWEEP_HIGH => " · high near-end missing (≥107)",
        _ => "",
    };
    let answer = |seen| if seen { "yes" } else { "no" };
    format!(
        "{name} (CC {cc}): {} · reached 0: {} · reached 127: {}{missing}",
        range.text(),
        answer(range.min == Some(0)),
        answer(range.max == Some(127))
    )
}

/// 7.5: one passive listening window for the six physical tone knobs.
pub fn podxt_knobs(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let step = super::catalogue::step(ctx.rec.id).expect("knob sweep catalogue entry");
    guided(
        ctx,
        step.you_do,
        step.hear,
        "all six PODxt Live tone knobs",
        &[UserAction::SweepPodxtKnobs],
        |_| Ok(None),
        |ctx, action| {
            let mut ranges = [Range::default(); PODXT_KNOBS.len()];
            with_podxt_stopping(ctx, PODXT_LISTEN, |session, stopping| {
                session.note("Listening window open for the PODxt tone knobs.");
                while !stopping() && !action.done() && !ranges.iter().all(|r| r.full_range()) {
                    for event in session
                        .poll(line6_devices::session::POLL)?
                        .into_iter()
                        .flatten()
                    {
                        if let Some((number, value @ 0..=127)) =
                            podxt::controller_notification(&event)
                            && let Some(index) =
                                PODXT_KNOBS.iter().position(|(cc, _)| *cc == number)
                        {
                            ranges[index].add(value);
                            let mut seen = vec![format!("{} now {value}", PODXT_KNOBS[index].1)];
                            seen.extend(
                                PODXT_KNOBS
                                    .into_iter()
                                    .zip(ranges)
                                    .map(|(knob, range)| knob_result(knob, range)),
                            );
                            action.clock.observe(seen.join("\n"));
                        }
                    }
                }
                if stopping() {
                    return Err(format!("{STOPPED}Stopped by you."));
                }
                Ok(())
            })?;
            let lines: Vec<_> = PODXT_KNOBS
                .into_iter()
                .zip(ranges)
                .map(|(knob, range)| knob_result(knob, range))
                .collect();
            for line in &lines {
                ctx.rec.show(line);
            }
            if ranges.iter().all(|r| r.swept()) {
                Ok(Some(()))
            } else {
                ctx.rec.note(lines.join("\n"));
                Ok(None)
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use crate::rig_check::catalogue::StepId;
    use crate::rig_check::practice::Faults;
    use crate::rig_check::record::Outcome;
    use crate::rig_check::run_practice;
    use crate::rig_check::runner::{NoReport, Retry};
    use crate::rig_check::steps::RunData;
    use crate::rig_check::testing::Script;

    const GUIDED: [StepId; 5] = [
        StepId(7, 1),
        StepId(7, 2),
        StepId(7, 3),
        StepId(7, 4),
        StepId(7, 5),
    ];

    #[test]
    fn detection_completes_each_guided_step_without_sends_in_the_window() {
        let mut person = Script::run_all();
        let (run, _, _) = run_practice(
            Faults::default(),
            &mut person,
            &mut NoReport,
            RunData::default(),
        );
        for id in GUIDED {
            let step = run.step(id);
            assert!(step.outcome.passed(), "{id}: {step:?}");
            // No message is sent between "window open" and the first received event.
            let mut open = false;
            for line in &step.messages {
                if line.contains("Listening window open") {
                    open = true;
                } else if open && line.contains(" OUT ") {
                    panic!("{id}: sent while listening: {line}");
                } else if line.contains(" IN ") {
                    open = false;
                }
            }
        }
        assert!(
            run.step(StepId(7, 2))
                .evidence
                .iter()
                .any(|l| l.starts_with("Both knobs swept: volume 0-127"))
        );
        assert!(
            run.step(StepId(7, 4))
                .evidence
                .iter()
                .any(|l| l == "Pedal range seen: 0-127.")
        );
        let instructions: Vec<_> = person
            .instructions
            .iter()
            .filter(|(id, _)| id.0 == 7)
            .collect();
        assert_eq!(
            instructions.len(),
            7,
            "two windows each for selector and footswitch; one for each sweep"
        );
        // The next step still showed its own card after each detection.
        for id in GUIDED {
            assert!(person.cards.contains(&id));
        }
    }

    #[test]
    fn the_six_knob_window_records_exact_and_near_end_sweeps_without_sends() {
        for (low, high) in [(0, 127), (20, 107)] {
            let mut events: Vec<_> = [low, 64, high]
                .into_iter()
                .flat_map(|value| super::PODXT_KNOBS.map(|(cc, _)| (cc, value)))
                .collect();
            events.extend([(7, 0), (7, 127), (64, 127)]);
            let mut person = Script::run_all();
            let (run, _, _) = run_practice(
                Faults {
                    podxt_knob_events: Some(events),
                    ..Faults::default()
                },
                &mut person,
                &mut NoReport,
                RunData::default(),
            );
            let step = run.step(StepId(7, 5));
            assert!(step.outcome.passed(), "{step:?}");
            for (cc, name) in super::PODXT_KNOBS {
                assert!(step.result.iter().any(|line| line
                    == &format!(
                        "{name} (CC {cc}): {low}-{high} · reached 0: {} · reached 127: {}",
                        if low == 0 { "yes" } else { "no" },
                        if high == 127 { "yes" } else { "no" },
                    )));
            }
            assert!(!step.messages.iter().any(|line| line.contains(" OUT ")));
        }
    }

    #[test]
    fn incomplete_sweeps_name_missing_knobs_and_each_missing_end() {
        let mut person = Script::run_all();
        person.retry = Retry::Skip;
        let (run, _, _) = run_practice(
            Faults {
                podxt_knob_events: Some(vec![
                    (13, 0),
                    (13, 106),
                    (14, 21),
                    (14, 127),
                    (15, 60),
                    (16, 0),
                    (16, 127),
                    (21, 0),
                    (21, 127),
                    (7, 0),
                    (7, 127),
                ]),
                ..Faults::default()
            },
            &mut person,
            &mut NoReport,
            RunData::default(),
        );
        let step = run.step(StepId(7, 5));
        assert_eq!(step.outcome, Outcome::SkippedByPerson);
        for expected in [
            "Drive (CC 13): 0-106 · reached 0: yes · reached 127: no · high near-end missing",
            "Bass (CC 14): 21-127 · reached 0: no · reached 127: yes · low near-end missing",
            "Mid (CC 15): 60-60 · reached 0: no · reached 127: no · both near-ends missing",
            "Channel Volume (CC 17): not seen · reached 0: no · reached 127: no · no movement detected",
        ] {
            assert!(
                step.result.iter().any(|line| line.starts_with(expected)),
                "{step:?}"
            );
        }
        assert!(!run.step(StepId(5, 2)).outcome.passed());
    }

    #[test]
    fn a_pedal_sweep_during_ready_or_late_in_the_action_window_is_detected() {
        for (when, expected) in [(2, "get-ready countdown"), (50, "action window")] {
            let mut person = Script::run_all();
            let (run, _, _) = run_practice(
                Faults {
                    guided_action_delay: Some(std::time::Duration::from_secs(when)),
                    ..Faults::default()
                },
                &mut person,
                &mut NoReport,
                RunData::default(),
            );
            let pedal = run.step(StepId(7, 4));
            assert!(pedal.outcome.passed(), "{pedal:?}");
            assert!(pedal.evidence.iter().any(|line| line.contains(expected)));
        }
    }

    #[test]
    fn a_missed_action_offers_try_again_or_skip() {
        let mut person = Script::run_all();
        person.retry = Retry::Skip;
        let faults = Faults {
            user_does_not_act: true,
            ..Faults::default()
        };
        let (run, _, rig) = run_practice(faults, &mut person, &mut NoReport, RunData::default());
        for id in GUIDED {
            assert_eq!(run.step(id).outcome, Outcome::SkippedByPerson, "{id}");
            assert!(
                run.step(id)
                    .evidence
                    .iter()
                    .any(|l| l.starts_with("Expected "))
            );
        }
        assert!(
            run.step(StepId(6, 2)).outcome.passed(),
            "the rig is still at its start"
        );
        drop(rig);
    }
}
