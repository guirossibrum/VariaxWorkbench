//! Application navigation, tab lifecycle, and shared commands.
use super::*;

mod detect;
#[cfg(test)]
mod detection_tests;
pub(super) use detect::DetectConfirmation;

pub(super) enum CloseRequest {
    Tab(TabId),
    Quit { approved: Vec<TabId> },
}

impl Workbench {
    pub(super) fn new(ctx: &egui::Context, defaults: EditorDefaults) -> Self {
        ctx.set_visuals(egui::Visuals::dark());
        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = Vec2::new(10.0, 10.0);
        style.spacing.button_padding = Vec2::new(12.0, 7.0);
        style.visuals.window_corner_radius = 0.into();
        style.visuals.widgets.inactive.corner_radius = 0.into();
        style.visuals.widgets.hovered.corner_radius = 0.into();
        style.visuals.widgets.active.corner_radius = 0.into();
        ctx.set_style(style);
        Self {
            #[cfg(not(test))]
            dialogs: Box::new(NativeDialogs),
            #[cfg(test)]
            dialogs: Box::new(FakeDialogs::default()),
            help: false,
            permit_close: false,
            theme_polled: Instant::now() - Duration::from_secs(2),
            theme_text: String::new(),
            screenshot: None,
            frame_count: 0,
            recording_open: false,
            device_check_open: false,
            device_check_notice: String::new(),
            backup_choice: None,
            backup_rx: None,
            tone_choice: None,
            tone_channel: "1A".into(),
            tone_rx: None,
            device_check_step_two_saved: false,
            device_check: ProbeSession::new(RigBackend::Simulated(SimulatedRig::new(
                device_probe::Scenario::Connected,
            ))),
            device_check_scenario: device_probe::Scenario::Connected,
            #[cfg(not(test))]
            device_check_root: PathBuf::from(device_probe::DEFAULT_ROOT),
            #[cfg(test)]
            device_check_root: PathBuf::new(),
            #[cfg(test)]
            host_reader_builds: 0,
            recorder: recording::Panel::new(),
            #[cfg(test)]
            pending_buttons: HashMap::new(),
            #[cfg(test)]
            recording_buttons: HashMap::new(),
            #[cfg(test)]
            recording_visible: String::new(),
            #[cfg(test)]
            device_check_buttons: HashMap::new(),
            #[cfg(test)]
            device_check_button_ids: HashMap::new(),
            #[cfg(test)]
            device_check_visible: String::new(),
            #[cfg(test)]
            recording_capture_attempts: 0,
            defaults,
            tabs: Tabs::default(),
            detection: DetectionService::platform(),
            detect_confirmation: None,
            next_detection: Instant::now(),
            detection_status: None,
            #[cfg(test)]
            detection_buttons: HashMap::new(),
            shell_error: None,
            shell_status: String::new(),
            close_request: None,
            waiting_for_hardware: false,
            closing_tab: None,
            recording_close_pending: false,
            #[cfg(test)]
            tab_buttons: HashMap::new(),
        }
    }

    pub(super) fn from_options(ctx: &egui::Context, options: GuiOptions) -> Self {
        let mut app = Self::new(
            ctx,
            EditorDefaults {
                artwork: options.artwork,
                preferences: Some(options.preferences),
                startup_panel: options.tab,
            },
        );
        if let Some(path) = options.preset {
            app.shell_error = app.open_file(path).err();
        } else if options.tab.is_some() || options.screenshot.is_some() {
            app.new_tab(registry::LEGACY_STARTUP);
        }
        app.screenshot = options.screenshot;
        // Apply the startup view only while opening the startup file or example.
        app.defaults.startup_panel = None;
        app
    }

    fn open_file(&mut self, path: PathBuf) -> Result<TabId, String> {
        let ceiling = registry::FAMILIES
            .iter()
            .map(|family| family.info().ceiling)
            .max()
            .unwrap_or(0);
        let bytes = tabs::open_candidate(&path, ceiling)?;
        let mut matches = registry::FAMILIES
            .iter()
            .copied()
            .filter(|family| bytes.len() as u64 <= family.info().ceiling && family.claims(&bytes));
        let family = matches
            .next()
            .ok_or("No editor recognizes this preset, bank, or collection.")?;
        if matches.next().is_some() {
            return Err("More than one editor recognizes this file.".into());
        }
        self.tabs
            .open(family, &self.defaults, TabSource::File(path, bytes))
    }

    pub(super) fn open_file_dialog(&mut self) {
        let Some(path) = self.dialogs.open_family_file(registry::FAMILIES) else {
            return;
        };
        self.shell_error = self.open_file(path).err();
    }

    pub(super) fn new_tab(&mut self, family: &'static dyn ProductFamily) {
        self.shell_error = self.tabs.open(family, &self.defaults, TabSource::New).err();
    }

    pub(super) fn with_tab(
        &mut self,
        id: TabId,
        action: impl FnOnce(&mut TabEntry, &mut EditorServices<'_>),
    ) -> Option<CloseResult> {
        let index = self.tabs.index(id)?;
        let mut services = EditorServices {
            dialogs: self.dialogs.as_mut(),
            close: CloseResult::Pending,
            #[cfg(test)]
            prompt_buttons: &mut self.pending_buttons,
        };
        action(&mut self.tabs.entries[index], &mut services);
        Some(services.close)
    }

    fn with_active(&mut self, action: impl FnOnce(&mut TabEntry, &mut EditorServices<'_>)) {
        if let Some(id) = self.tabs.active {
            self.with_tab(id, action);
        }
    }

    pub(super) fn has_prompt(&self) -> bool {
        self.close_request.is_some()
            || self.detect_confirmation.is_some()
            || self
                .tabs
                .active
                .and_then(|id| self.tabs.index(id))
                .is_some_and(|index| self.tabs.entries[index].editor.blocking_prompt())
    }

    pub(super) fn pending_dialog(&mut self, ctx: &egui::Context) {
        #[cfg(test)]
        self.pending_buttons.clear();
        if self.recording_close_pending {
            self.recording_quit_dialog(ctx);
        } else if let Some(id) = self.closing_tab {
            let result = self.with_tab(id, |entry, services| {
                entry.editor.close_prompt(ctx, services)
            });
            match result {
                Some(CloseResult::Pending) => {}
                Some(CloseResult::Cancelled) => self.cancel_close(),
                Some(CloseResult::Ready) | None => {
                    self.closing_tab = None;
                    if let Some(CloseRequest::Quit { approved }) = &mut self.close_request {
                        approved.push(id);
                    } else {
                        self.tabs.close(id);
                        self.close_request = None;
                    }
                    self.advance_close(ctx);
                }
            }
        } else {
            self.with_active(|entry, services| entry.editor.local_prompt(ctx, services));
        }
    }

    pub(super) fn cancel_close(&mut self) {
        self.closing_tab = None;
        self.close_request = None;
        self.recording_close_pending = false;
        self.waiting_for_hardware = false;
    }

    fn advance_close(&mut self, ctx: &egui::Context) {
        let Some(request) = &self.close_request else {
            return;
        };
        if matches!(request, CloseRequest::Quit { .. }) {
            let identity_busy = self
                .detection
                .as_mut()
                .is_some_and(DetectionService::cancel_for_quit);
            let was_waiting = self.waiting_for_hardware;
            self.waiting_for_hardware =
                identity_busy || self.backup_rx.is_some() || self.tone_rx.is_some();
            if self.waiting_for_hardware {
                self.shell_status =
                    "Waiting for the device operation to finish before quitting.".into();
                ctx.request_repaint_after(Duration::from_millis(100));
                return;
            }
            if was_waiting
                && self.shell_status
                    == "Waiting for the device operation to finish before quitting."
            {
                self.shell_status.clear();
            }
        }
        let mut target = None;
        for entry in &mut self.tabs.entries {
            let consider = match request {
                CloseRequest::Tab(id) => entry.id == *id,
                CloseRequest::Quit { approved } => !approved.contains(&entry.id),
            };
            if !consider {
                continue;
            }
            if entry.editor.busy() || entry.editor.blocking_prompt() {
                entry.editor.cancel_operation();
                self.tabs.active = Some(entry.id);
                self.cancel_close();
                return;
            }
            if target.is_none() && entry.editor.dirty() {
                target = Some(entry.id);
            }
        }
        if let Some(id) = target {
            self.tabs.active = Some(id);
            self.closing_tab = Some(id);
        } else if let Some(CloseRequest::Tab(id)) = self.close_request {
            self.tabs.close(id);
            self.close_request = None;
        } else if self.recorder.has_unsaved_take() {
            self.recording_close_pending = true;
        } else {
            self.cancel_close();
            self.permit_close = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    pub(super) fn request_close(&mut self, ctx: &egui::Context) {
        self.detect_confirmation = None;
        if self.permit_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        if self.close_request.is_none() {
            self.close_request = Some(CloseRequest::Quit {
                approved: Vec::new(),
            });
            self.advance_close(ctx);
        }
    }

    pub(super) fn close_tab(&mut self, id: TabId, ctx: &egui::Context) {
        self.close_request = Some(CloseRequest::Tab(id));
        self.advance_close(ctx);
    }

    pub(super) fn continue_recording_close(&mut self, ctx: &egui::Context) {
        self.recording_close_pending = false;
        self.advance_close(ctx);
    }

    pub(super) fn render(&mut self, ctx: &egui::Context) {
        self.refresh_theme(ctx);
        self.poll_detection(ctx);
        if self.waiting_for_hardware {
            self.advance_close(ctx);
        }
        for entry in &mut self.tabs.entries {
            entry.editor.observe_device(entry.device.as_ref());
            entry.editor.poll(ctx);
        }
        self.frame_count += 1;
        if self.screenshot.is_some() && self.frame_count <= 5 {
            ctx.request_repaint();
            if self.frame_count == 5 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
        }
        for event in ctx.input(|input| input.events.clone()) {
            if let egui::Event::Screenshot { image: capture, .. } = event
                && let Some(path) = self.screenshot.take()
            {
                let pixels: Vec<u8> = capture
                    .pixels
                    .iter()
                    .flat_map(|pixel| pixel.to_array())
                    .collect();
                if let Err(error) = save_screenshot(
                    &path,
                    &pixels,
                    capture.size[0] as u32,
                    capture.size[1] as u32,
                ) {
                    self.shell_error = Some(format!("Screenshot: {error}"));
                } else {
                    self.request_close(ctx);
                }
            }
        }
        let (open, save, undo, redo, quit, help) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::O) && i.modifiers.command,
                i.key_pressed(egui::Key::S) && i.modifiers.command,
                i.key_pressed(egui::Key::Z) && i.modifiers.command && !i.modifiers.shift,
                i.key_pressed(egui::Key::Z) && i.modifiers.command && i.modifiers.shift,
                i.key_pressed(egui::Key::Q) && !ctx.wants_keyboard_input(),
                i.key_pressed(egui::Key::Questionmark) && !ctx.wants_keyboard_input(),
            )
        });
        if !self.has_prompt() {
            if open {
                self.open_file_dialog();
            }
            self.with_active(|entry, services| {
                if entry.editor.busy() || entry.editor.blocking_prompt() {
                    return;
                }
                if save {
                    entry.editor.save_copy(services);
                }
                if undo {
                    entry.editor.undo();
                }
                if redo {
                    entry.editor.redo();
                }
            });
        }
        if help {
            self.help = !self.help;
        }
        if quit || ctx.input(|i| i.viewport().close_requested()) {
            self.request_close(ctx);
        }
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.heading("Variax Workbench");
                ui.add_enabled_ui(!self.has_prompt(), |ui| {
                    self.open_commands(ui, false);
                    self.with_active(|entry, services| {
                        let enabled = !entry.editor.busy() && !entry.editor.blocking_prompt();
                        if ui
                            .add_enabled(enabled, egui::Button::new("Save copy…"))
                            .clicked()
                        {
                            entry.editor.save_copy(services);
                        }
                        ui.add_enabled_ui(enabled, |ui| entry.editor.actions(ui, services));
                    });
                });
                let response = ui.button("Device check…");
                #[cfg(test)]
                self.device_check_buttons.insert("open", response.rect);
                if response.clicked() {
                    self.device_check_open = true;
                }
                let response = ui.button("Test recording…");
                #[cfg(test)]
                self.recording_buttons
                    .insert("open-recording", response.rect);
                if response.clicked() {
                    self.recording_open = true;
                }
                if ui.button("Milestone & Help").clicked() {
                    self.help = true;
                }
            });
            self.tab_strip(ui, ctx);
        });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            self.with_active(|entry, _| {
                if entry.editor.busy() {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Reading model settings…");
                        if ui.button("Cancel read").clicked() {
                            entry.editor.cancel_operation();
                        }
                    });
                }
            });
            let mut guitar_refresh = None;
            let allow_refresh = !self.has_prompt();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::BOTTOM), |ui| {
                ui.label(format!("build {}", crate::BUILD_NUMBER));
                ui.label(crate::APP_VERSION);
                ui.vertical(|ui| {
                    if let Some(index) = self.tabs.active.and_then(|id| self.tabs.index(id)) {
                        let (status, error) = self.tabs.entries[index].editor.status();
                        ui.weak(status);
                        if let Some(error) = error {
                            ui.colored_label(Color32::LIGHT_RED, error);
                        }
                        if let Some(device) = &self.tabs.entries[index].device {
                            ui.weak(device.status());
                            if device.connection == tabs::Connection::Present
                                && let Some(status) = device.device.guitar_status.clone()
                            {
                                let attachment = device
                                    .device
                                    .refresh_guitar
                                    .then(|| device.device.key.attachment.clone());
                                ui.horizontal(|ui| {
                                    ui.label(status);
                                    if let Some(attachment) = attachment
                                        && self.refresh_guitar_button(ui, allow_refresh)
                                    {
                                        guitar_refresh = Some(attachment);
                                    }
                                });
                            }
                        }
                    }
                    if let Some(error) = &self.detection_status {
                        ui.colored_label(Color32::LIGHT_RED, error);
                    }
                    if !self.shell_status.is_empty() {
                        ui.weak(&self.shell_status);
                    }
                    if let Some(error) = &self.shell_error {
                        ui.colored_label(Color32::LIGHT_RED, error);
                    }
                });
            });
            if let Some(attachment) = guitar_refresh {
                self.refresh_guitar(&attachment);
            }
        });
        let closing = self.close_request.is_some();
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.tabs.active.is_some() {
                self.with_active(|entry, services| {
                    ui.push_id((entry.family.info().id, entry.id), |ui| {
                        ui.add_enabled_ui(!closing, |ui| entry.editor.show(ui, services))
                    });
                });
            } else {
                ui.heading("Open a file or start an offline tab");
                self.open_commands(ui, true);
            }
        });
        if self.help {
            egui::Window::new(format!("Workbench Help · {}", crate::APP_VERSION))
                .open(&mut self.help)
                .collapsible(false)
                .default_width(400.0)
                .default_height(600.0)
                .vscroll(true)
                .show(ctx, |ui| {
                    for &family in registry::FAMILIES {
                        family.help(ui);
                    }
                    ui.separator();
                    ui.heading("Keyboard shortcuts");
                    for (key, action) in [
                        ("Ctrl+O", "Open file in a new tab"),
                        ("Ctrl+S", "Save a new copy"),
                        ("Ctrl+Z", "Undo"),
                        ("Ctrl+Shift+Z", "Redo"),
                        ("Q", "Quit"),
                        ("?", "Toggle Help"),
                    ] {
                        ui.horizontal(|ui| {
                            ui.monospace(key);
                            ui.label(action);
                        });
                    }
                });
        }
        self.pending_dialog(ctx);
        self.recording_window(ctx);
        self.device_check_window(ctx);
        self.detect_dialog(ctx);
    }

    fn open_commands(&mut self, ui: &mut egui::Ui, start_panel: bool) {
        if ui.button("Open file…").clicked() {
            self.open_file_dialog();
        }
        if start_panel {
            for &family in registry::FAMILIES {
                if ui
                    .button(format!("New offline tab · {}", family.info().new_label))
                    .clicked()
                {
                    self.new_tab(family);
                }
            }
        } else {
            ui.menu_button("New offline tab", |ui| {
                for &family in registry::FAMILIES {
                    if ui.button(family.info().new_label).clicked() {
                        self.new_tab(family);
                        ui.close();
                    }
                }
            });
        }
        self.detect_button(ui, start_panel);
    }

    fn tab_strip(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let mut select = None;
        let mut close = None;
        egui::ScrollArea::horizontal()
            .id_salt("editor-tabs")
            .show(ui, |ui| {
                ui.add_enabled_ui(!self.has_prompt(), |ui| {
                    ui.horizontal(|ui| {
                        for entry in &self.tabs.entries {
                            ui.push_id((entry.family.info().id, entry.id), |ui| {
                                let mut label = entry.title();
                                if entry.editor.dirty() {
                                    label.push_str(" *");
                                }
                                let response =
                                    ui.selectable_label(self.tabs.active == Some(entry.id), label);
                                #[cfg(test)]
                                self.tab_buttons
                                    .insert(entry.family.info().id, response.rect);
                                if response.clicked() {
                                    select = Some(entry.id);
                                }
                                if ui.small_button("×").on_hover_text("Close tab").clicked() {
                                    close = Some(entry.id);
                                }
                            });
                        }
                    });
                });
            });
        if let Some(id) = select {
            self.tabs.active = Some(id);
        }
        if let Some(id) = close {
            self.close_tab(id, ctx);
        }
    }
}
