use super::*;
use variax_workbench::pod_audio::RecordRoute;
use variax_workbench::recording::ClockSelection;

#[cfg(test)]
pub(super) mod tests;

fn route_button_id(route: RecordRoute) -> &'static str {
    match route {
        RecordRoute::SendProcessedGuitar => "route-processed",
        RecordRoute::SendCleanGuitar => "route-clean",
        RecordRoute::SendCleanGuitarReampPlayback => "route-reamp-clean",
        RecordRoute::SendReampPlayback => "route-reamp",
    }
}

fn clock_button_id(clock: ClockSelection) -> &'static str {
    match clock {
        ClockSelection::Unselected => "clock-none",
        ClockSelection::Integer(44_100) => "clock-44100",
        ClockSelection::Integer(48_000) => "clock-48000",
        ClockSelection::Integer(_) => "clock-other",
        ClockSelection::NativePodxt => "clock-native",
    }
}

impl Workbench {
    pub(super) fn recording_quit_dialog(&mut self, ctx: &egui::Context) {
        if !self.recorder.has_unsaved_take() {
            self.continue_recording_close(ctx);
            return;
        }
        let native = self.recorder.take_can_save_aiff();
        let can_save = self.recorder.take_can_save_wav() || native;
        let save_label = match (native, self.recorder.phase()) {
            (true, recording::Phase::Recording) => "Stop and save AIFF copy…",
            (true, _) => "Save AIFF copy…",
            (false, recording::Phase::Recording) => "Stop and save WAV copy…",
            (false, _) => "Save WAV copy…",
        };
        let mut save = false;
        let mut discard = false;
        let mut cancel = false;
        egui::Window::new("Unsaved offline test take")
            .default_width(560.0)
            .max_width(640.0)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label("This test take or its pending capture recovery data has not been fully saved or explicitly discarded.");
                if native {
                    ui.label("Exact-clock AIFF preserves 78125/2. Other software may round this rate; verify imports.");
                }
                ui.label(self.recorder.message());
                ui.horizontal(|ui| {
                    let response = ui.add_enabled(can_save, egui::Button::new(save_label));
                    #[cfg(test)]
                    self.pending_buttons.insert("save", response.rect);
                    save = response.clicked();
                    let response = ui.button("Keep editing");
                    #[cfg(test)]
                    self.pending_buttons.insert("cancel", response.rect);
                    cancel = response.clicked();
                    let response = ui.button("Discard this take");
                    #[cfg(test)]
                    self.pending_buttons.insert("discard", response.rect);
                    discard = response.clicked();
                });
            });
        if cancel {
            self.cancel_close();
        } else if save {
            if self.recorder.phase() == recording::Phase::Recording
                && let Err(error) = self.recorder.stop()
            {
                self.recorder.note(error.to_string());
                return;
            }
            self.save_recording(ctx, true);
        } else if discard {
            match self.recorder.cancel() {
                Ok(()) => self.continue_recording_close(ctx),
                Err(error) => self.recorder.note(error.to_string()),
            }
        }
    }

    pub(super) fn recording_window(&mut self, ctx: &egui::Context) {
        if !self.recording_open {
            return;
        }
        let mut open = true;
        egui::Window::new("Offline test recording")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::LEFT_TOP, egui::vec2(24.0, 64.0))
            .show(ctx, |ui| {
                if self.has_prompt() || self.picker.is_open() {
                    ui.disable();
                }
                ui.label(recording::OFFLINE_BANNER);
                let capture_notice = if cfg!(target_os = "macos") {
                    variax_workbench::MAC_CONNECTION_UNAVAILABLE
                } else {
                    variax_workbench::offline_device_notice()
                        .unwrap_or(recording::DEVICE_CAPTURE_LOCKED)
                };
                ui.label(capture_notice);
                ui.label(recording::NO_AUTO_START);
                ui.label(self.recorder.phase_label());
                ui.label(self.recorder.meter_line());
                ui.label(self.recorder.guide_line());
                ui.label(self.recorder.message());
                ui.separator();
                for (route, label) in recording::ROUTE_CHOICES {
                    if self.record_choice(
                        ui,
                        route_button_id(route),
                        self.recorder.route() == Some(route),
                        label,
                    ) && let Err(error) = self.recorder.select_route(route)
                    {
                        self.recorder.note(error.to_string());
                    }
                }
                for (clock, label) in recording::CLOCK_CHOICES {
                    if self.record_choice(
                        ui,
                        clock_button_id(clock),
                        self.recorder.clock() == clock,
                        label,
                    ) && let Err(error) = self.recorder.select_clock(clock)
                    {
                        self.recorder.note(error.to_string());
                    }
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if self.record_button(ui, "inspect-native-plan", "Inspect PODxt Live plan…")
                        && let Err(error) = self.recorder.inspect_native_plan()
                    {
                        self.recorder.note(error.to_string());
                    }
                    if self.record_button(ui, "check-external-plan", "Check external interface…")
                        && let Err(error) = self.recorder.external_plan_status()
                    {
                        self.recorder.note(error.to_string());
                    }
                });
                if self.record_choice(
                    ui,
                    "input-synthetic",
                    self.recorder.input() == recording::InputSelection::Synthetic,
                    "Synthetic generator",
                ) && let Err(error) = self
                    .recorder
                    .select_input(recording::InputSelection::Synthetic)
                {
                    self.recorder.note(error.to_string());
                }
                if matches!(self.recorder.clock(), recording::ClockSelection::Integer(_))
                    && self.record_choice(
                        ui,
                        "input-simulated-callback",
                        self.recorder.input() == recording::InputSelection::SimulatedCallback,
                        "Simulated callback test data",
                    )
                    && let Err(error) = self
                        .recorder
                        .select_input(recording::InputSelection::SimulatedCallback)
                {
                    self.recorder.note(error.to_string());
                }
                #[cfg(target_os = "linux")]
                if self.recorder.clock() == recording::ClockSelection::NativePodxt
                    && self.record_choice(
                        ui,
                        "input-static-podxt-synthetic",
                        self.recorder.input() == recording::InputSelection::StaticPodxtSynthetic,
                        "PODxt static plan + generated test PCM",
                    )
                    && let Err(error) = self
                        .recorder
                        .select_input(recording::InputSelection::StaticPodxtSynthetic)
                {
                    self.recorder.note(error.to_string());
                }
                if self.record_choice(
                    ui,
                    "input-file",
                    self.recorder.input() == recording::InputSelection::File,
                    "WAV file",
                ) && let Err(error) = self.recorder.select_input(recording::InputSelection::File)
                {
                    self.recorder.note(error.to_string());
                }
                if self.record_button(ui, "load-wav", "Load test WAV…") {
                    self.load_recording_wav(ctx);
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if self.record_button(ui, "start", "Start") {
                        self.arm_recording();
                    }
                    if self.record_button(ui, "feed", "Feed chunk")
                        && let Err(error) = self.recorder.feed()
                    {
                        self.recorder.note(error.to_string());
                    }
                    if self.record_button(ui, "stop", "Stop")
                        && let Err(error) = self.recorder.stop()
                    {
                        self.recorder.note(error.to_string());
                    }
                    if self.record_button(ui, "cancel", "Cancel")
                        && let Err(error) = self.recorder.cancel()
                    {
                        self.recorder.note(error.to_string());
                    }
                });
                let label = if self.recorder.take_can_save_aiff() {
                    "Save exact-clock AIFF…"
                } else {
                    "Save WAV copy…"
                };
                if self.record_button(ui, "save", label) {
                    self.save_recording(ctx, false);
                }
                #[cfg(test)]
                {
                    self.recording_visible = format!(
                        "{}\n{}\n{}\n{}\n{}\n{}\n{}",
                        recording::OFFLINE_BANNER,
                        capture_notice,
                        recording::NO_AUTO_START,
                        self.recorder.phase_label(),
                        self.recorder.meter_line(),
                        self.recorder.guide_line(),
                        self.recorder.message(),
                    );
                }
            });
        self.recording_open = open;
        if !open {
            self.picker.discard_if(|action| {
                matches!(
                    action,
                    PickerAction::LoadRecording | PickerAction::SaveRecording { quit: false, .. }
                )
            });
        }
    }

    fn record_choice(
        &mut self,
        ui: &mut egui::Ui,
        id: &'static str,
        selected: bool,
        label: &str,
    ) -> bool {
        let response = ui.selectable_label(selected, label);
        #[cfg(test)]
        self.recording_buttons.insert(id, response.rect);
        #[cfg(not(test))]
        let _ = id;
        response.clicked()
    }

    fn record_button(&mut self, ui: &mut egui::Ui, id: &'static str, label: &str) -> bool {
        let response = ui.button(label);
        #[cfg(test)]
        self.recording_buttons.insert(id, response.rect);
        #[cfg(not(test))]
        let _ = id;
        response.clicked()
    }

    fn arm_recording(&mut self) {
        let Some(route) = self.recorder.route() else {
            self.recorder
                .note("Select a route, an input, and a clock before starting.");
            return;
        };
        let mut attempts = 0;
        let gate = variax_workbench::rig::Session::initial_rig().gate_offline_recording(
            route,
            recording::OFFLINE_ROUTE_PERIOD_BYTES,
            recording::OFFLINE_ROUTE_PERIODS,
            || {
                attempts += 1;
                Ok(())
            },
        );
        #[cfg(test)]
        {
            self.recording_capture_attempts = attempts;
        }
        if attempts != 0 {
            self.recorder
                .note("Capture attempt ran. The take was not started.");
            return;
        }
        match gate {
            Err(error) => {
                self.recorder
                    .note(format!("Route gate refused the take: {error:?}"));
            }
            Ok(gate)
                if gate.stream_opened || gate.outbound_len != 0 || !gate.capture_stays_locked =>
            {
                self.recorder.note("Route plan opened a stream or produced outbound bytes. The take was not started.");
            }
            Ok(_) => {
                if let Err(error) = self.recorder.start() {
                    self.recorder.note(error.to_string());
                }
            }
        }
    }

    fn load_recording_wav(&mut self, ctx: &egui::Context) {
        if self.recorder.phase() != recording::Phase::Idle {
            self.recorder
                .note("Cancel the current take before loading a file.");
            return;
        }
        self.request_picker(ctx, PickRequest::OpenWav, PickerAction::LoadRecording);
    }

    pub(super) fn load_recording_from(&mut self, path: Option<PathBuf>) {
        let Some(path) = path else {
            self.recorder.note("Load cancelled. No file was read.");
            return;
        };
        let label = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "audio.wav".into());
        match recording::read_regular_bytes(
            recording::path_facts(&path),
            recording::TAKE_BYTES_CEILING,
            || fs::read(&path),
        ) {
            Ok(bytes) => {
                if let Err(error) = self.recorder.load_wav(&label, &bytes) {
                    self.recorder.note(error.to_string());
                }
            }
            Err(recording::TakeError::CapExceeded) => {
                self.recorder
                    .note("That file is larger than the offline take ceiling. It was not loaded.");
            }
            Err(error) => self.recorder.note(error.to_string()),
        }
    }

    fn save_recording(&mut self, ctx: &egui::Context, quit: bool) {
        if self.recorder.phase() != recording::Phase::Stopped {
            self.recorder.note("Stop the take before saving.");
            return;
        }
        let aiff = self.recorder.take_can_save_aiff();
        let name = if aiff {
            "exact-clock-test-take.aiff"
        } else {
            "offline-test-take.wav"
        };
        self.request_picker(
            ctx,
            PickRequest::SaveAudio {
                name: name.into(),
                aiff,
            },
            PickerAction::SaveRecording { aiff, quit },
        );
    }

    pub(super) fn save_recording_to(&mut self, destination: Option<PathBuf>, native: bool) -> bool {
        let Some(path) = destination else {
            self.recorder.note("Save cancelled. No file was written.");
            return false;
        };
        let result = if native {
            self.recorder.save_aiff(&path).map(|_| ())
        } else {
            self.recorder.save(&path).map(|_| ())
        };
        match result {
            Ok(()) => true,
            Err(error) => {
                self.recorder.note(error.to_string());
                false
            }
        }
    }
}
