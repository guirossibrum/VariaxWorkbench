use super::*;
use variax_workbench::ui::tuner_popup::{
    TunerAction, TunerNote, TunerPopup, TunerReading as DisplayReading, TunerState,
};

/// One completed note/deviation pair, stamped by the read worker on receipt.
#[derive(Clone, Copy)]
pub(super) struct Sample {
    pub(super) pair: TunerReading,
    pub(super) received_at: Instant,
}

impl Sample {
    fn display(self) -> DisplayReading<'static> {
        let note = match self.pair.note.map(podxt::tuner_note_name) {
            None => TunerNote::Missing,
            Some("-") => TunerNote::NoSignal,
            Some(note) => TunerNote::Named(note),
        };
        DisplayReading {
            note,
            cents: self.pair.deviation.map(|raw| raw as i16),
            received_at: self.received_at,
        }
    }
}

#[derive(Default)]
enum State {
    #[default]
    Hidden,
    Running {
        detached: bool,
    },
    Stopped,
    Disconnected,
    Error(String),
}

#[derive(Default)]
pub(super) struct Tuner {
    popup: TunerPopup,
    state: State,
}

impl Tuner {
    pub(super) fn with_preferences(path: Option<&Path>) -> Self {
        Self {
            popup: TunerPopup::with_preferences(path),
            ..Self::default()
        }
    }

    pub(super) fn is_open(&self) -> bool {
        !matches!(self.state, State::Hidden)
    }

    pub(super) fn started(&mut self, result: Result<(), String>) {
        self.state = match result {
            Ok(()) => State::Running { detached: false },
            Err(error) => State::Error(error),
        };
    }

    pub(super) fn detach(&mut self) {
        if let State::Running { detached } = &mut self.state {
            *detached = true;
        }
    }

    pub(super) fn finished(&mut self, stop_requested: bool, result: Result<(), String>) {
        let State::Running { detached } = self.state else {
            return;
        };
        self.state = match result {
            // Combined errors from the tuner-off or route release must survive
            // cancellation and detachment. Only exact cancellation is normal.
            Err(error)
                if !stop_requested
                    || !matches!(
                        error.as_str(),
                        "PODxt read cancelled." | "PODxt Live operation cancelled."
                    ) =>
            {
                State::Error(error)
            }
            _ if detached => State::Disconnected,
            _ => State::Stopped,
        };
    }

    fn presentation<'a>(state: &'a State, worker: Option<&Worker>) -> Option<TunerState<'a>> {
        Some(match state {
            State::Hidden => return None,
            State::Running { detached } => {
                // The live panel retains this exact worker until join. A
                // replacement attachment cannot provide samples to this run.
                let worker = worker.expect("an active tuner owns the read worker");
                if *detached || worker.stop.load(Ordering::Acquire) {
                    TunerState::Stopping
                } else if let Some(sample) = worker.snapshot.lock().expect("snapshot mutex").tuner {
                    TunerState::Listening {
                        reading: Some(sample.display()),
                    }
                } else {
                    TunerState::Starting
                }
            }
            State::Stopped => TunerState::Stopped,
            State::Disconnected => TunerState::Disconnected,
            State::Error(error) => TunerState::Error(error),
        })
    }

    pub(super) fn show(
        &mut self,
        ctx: &egui::Context,
        worker: Option<&Worker>,
        focus_target: &egui::Response,
    ) {
        let Some(state) = Self::presentation(&self.state, worker) else {
            return;
        };
        let action = self.popup.show(ctx, state);
        match action {
            Some(TunerAction::Stop) => {
                let worker = worker.expect("tuner stop requires an active worker");
                worker.request_stop();
                ctx.request_repaint();
            }
            Some(TunerAction::Dismiss) => {
                self.state = State::Hidden;
                self.popup.restore_focus(focus_target);
                ctx.request_repaint();
            }
            None => {}
        }
    }
}
