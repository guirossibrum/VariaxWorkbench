//! Application navigation, tab lifecycle, and shared commands.
use super::variax_panel::VariaxEditor;
use super::*;
use variax_workbench::ui::{assets, chrome, theme};

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
        theme::apply(ctx);
        // Family editors are still laid out for the previous spacing. The
        // Variax pickup drag is calibrated to that scale. #219 restyles the
        // editors. Shell controls ask for the 32px target themselves.
        ctx.style_mut(|style| {
            style.spacing.item_spacing = egui::vec2(10.0, 10.0);
            style.spacing.button_padding = egui::vec2(12.0, 7.0);
            style.spacing.interact_size = egui::vec2(40.0, 18.0);
        });
        Self {
            #[cfg(not(test))]
            dialogs: Box::new(NativeDialogs),
            #[cfg(test)]
            dialogs: Box::new(FakeDialogs::default()),
            picker: Picker::default(),
            help: false,
            permit_close: false,
            theme_polled: Instant::now() - Duration::from_secs(2),
            theme_text: String::new(),
            screenshot: None,
            window_preferences: None,
            frame_count: 0,
            recording_open: false,
            device_check_open: false,
            device_check_notice: String::new(),
            backup_choice: None,
            backup_rx: None,
            backup_cancel: None,
            tone_choice: None,
            tone_channel: "1A".into(),
            tone_rx: None,
            device_check_step_two_saved: false,
            device_check: Self::new_device_check(),
            device_check_focus: None,
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

    fn request_editor_open(&mut self, ctx: &egui::Context) {
        let mut handled = false;
        self.with_active(|entry, services| {
            let editor = &mut *entry.editor as &mut dyn std::any::Any;
            if let Some(editor) = editor.downcast_mut::<VariaxEditor>() {
                handled = true;
                if !services.picker.is_open() && !editor.busy() && !editor.blocking_prompt() {
                    editor.open_dialog(ctx, services);
                }
            } else if let Some(editor) = editor.downcast_mut::<super::podxt_panel::PodxtEditor>() {
                handled = true;
                if !services.picker.is_open() && !editor.busy() && !editor.blocking_prompt() {
                    editor.open_dialog(ctx, services);
                }
            }
        });
        if !handled {
            self.request_open_file(ctx);
        }
    }

    pub(super) fn request_open_file(&mut self, ctx: &egui::Context) {
        let filters = registry::FAMILIES
            .iter()
            .map(|family| {
                let info = family.info();
                FileFilter {
                    name: info.title,
                    extensions: info.extensions,
                }
            })
            .collect();
        self.request_picker(
            ctx,
            PickRequest::OpenEditorFile(filters),
            PickerAction::OpenFile,
        );
    }

    // Existing editor-lane tests still use their immediate fake dialogs.
    #[cfg(test)]
    pub(super) fn open_file_dialog(&mut self) {
        let context = egui::Context::default();
        self.request_open_file(&context);
        self.poll_picker(&context);
    }

    pub(super) fn request_picker(
        &mut self,
        ctx: &egui::Context,
        request: PickRequest,
        action: PickerAction,
    ) -> bool {
        let started = self
            .picker
            .start(action, || self.dialogs.start_picker(request));
        if started {
            ctx.request_repaint();
        }
        started
    }

    pub(super) fn poll_picker(&mut self, ctx: &egui::Context) {
        let Some((action, path)) = self.picker.poll(ctx) else {
            return;
        };
        match action {
            PickerAction::OpenFile => {
                if let Some(path) = path {
                    self.shell_error = self.open_file(path).err();
                }
            }
            PickerAction::Backup(choice, target) => self.start_backup_in(choice, target, path),
            PickerAction::SaveReport { restart } => {
                if let Some(path) = path {
                    if restart {
                        self.save_step_two_report_then_restart(&path);
                    } else {
                        self.note_sanitized_report_save(&path);
                    }
                }
            }
            PickerAction::LoadRecording => self.load_recording_from(path),
            PickerAction::SaveRecording { aiff, quit } => {
                if self.save_recording_to(path, aiff) && quit {
                    self.continue_recording_close(ctx);
                }
            }
            PickerAction::Tab { id, complete } => {
                let result = self.with_tab(id, |entry, services| {
                    complete(entry.editor.as_mut(), path, services);
                });
                if self.closing_tab == Some(id) {
                    self.apply_tab_close_result(id, result, ctx);
                }
            }
        }
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
        let attachment = self.tabs.entries[index]
            .device
            .as_ref()
            .and_then(tabs::DeviceTab::guitar_refresh_attachment)
            .filter(|_| self.detection.is_some() && !self.has_prompt() && !self.permit_close)
            .cloned();
        let mut refresh_requested = false;
        let mut services = EditorServices {
            dialogs: self.dialogs.as_mut(),
            picker: &mut self.picker,
            tab: id,
            close: CloseResult::Pending,
            guitar_refresh: attachment.as_ref().map(|_| &mut refresh_requested),
            #[cfg(test)]
            prompt_buttons: &mut self.pending_buttons,
        };
        action(&mut self.tabs.entries[index], &mut services);
        let close = services.close;
        if refresh_requested && let Some(attachment) = attachment {
            self.refresh_guitar(&attachment);
        }
        Some(close)
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
        if self.picker.is_open() {
            return;
        }
        if self.recording_close_pending {
            self.recording_quit_dialog(ctx);
        } else if let Some(id) = self.closing_tab {
            let result = self.with_tab(id, |entry, services| {
                entry.editor.close_prompt(ctx, services)
            });
            self.apply_tab_close_result(id, result, ctx);
        } else {
            self.with_active(|entry, services| entry.editor.local_prompt(ctx, services));
        }
    }

    fn apply_tab_close_result(
        &mut self,
        id: TabId,
        result: Option<CloseResult>,
        ctx: &egui::Context,
    ) {
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
            self.cancel_full_backup();
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
        if self.close_request.is_none() && !self.picker.is_open() {
            self.close_request = Some(CloseRequest::Quit {
                approved: Vec::new(),
            });
            self.advance_close(ctx);
        }
    }

    pub(super) fn close_tab(&mut self, id: TabId, ctx: &egui::Context) {
        if self.picker.is_open() {
            return;
        }
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
        let mut device_events = Vec::new();
        for entry in &mut self.tabs.entries {
            if let Some(device) = &mut entry.device
                && device.reconnected_with_edits
                && !entry.editor.dirty()
            {
                device.reconnected_with_edits = false;
            }
            entry.editor.observe_device(entry.device.as_ref());
            entry.editor.poll(ctx);
            if let Some(event) = entry.editor.take_device_event() {
                device_events.push((entry.id, event));
            }
        }
        for (tab, event) in device_events {
            self.apply_device_event(tab, event);
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
        let (open, save, save_as, undo, redo, quit, help) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::O) && i.modifiers.command,
                i.key_pressed(egui::Key::S) && i.modifiers.command,
                i.modifiers.shift,
                i.key_pressed(egui::Key::Z) && i.modifiers.command && !i.modifiers.shift,
                i.key_pressed(egui::Key::Z) && i.modifiers.command && i.modifiers.shift,
                i.key_pressed(egui::Key::Q) && i.modifiers.matches_exact(egui::Modifiers::CTRL),
                i.key_pressed(egui::Key::Questionmark) && !ctx.wants_keyboard_input(),
            )
        });
        if !self.has_prompt() {
            if open {
                self.request_editor_open(ctx);
            }
            self.with_active(|entry, services| {
                if services.picker.is_open()
                    || entry.editor.busy()
                    || entry.editor.blocking_prompt()
                {
                    return;
                }
                if save {
                    if !save_as
                        && let Some(editor) = (&mut *entry.editor as &mut dyn std::any::Any)
                            .downcast_mut::<VariaxEditor>()
                    {
                        editor.save_file(ctx, services, false);
                    } else {
                        entry.editor.save_copy(ctx, services);
                    }
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
        egui::TopBottomPanel::top("header")
            .resizable(false)
            .show_separator_line(false)
            .frame(shell_bar(4))
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(theme::SPACE_8, theme::SPACE_4);
                    ui.spacing_mut().button_padding = egui::vec2(theme::SPACE_12, theme::SPACE_4);
                    ui.spacing_mut().interact_size.y = theme::TARGET;
                    paint_product_mark(ui);
                    ui.label(
                        egui::RichText::new("Variax Workbench")
                            .size(theme::SECTION)
                            .strong()
                            .color(theme::TEXT),
                    );
                    ui.add_enabled_ui(!self.has_prompt(), |ui| {
                        let variax_active = self
                            .tabs
                            .active
                            .and_then(|id| self.tabs.index(id))
                            .is_some_and(|index| {
                                self.tabs.entries[index].family.info().id == "variax"
                            });
                        if variax_active {
                            self.with_active(|entry, services| {
                                let editor = (&mut *entry.editor as &mut dyn std::any::Any)
                                    .downcast_mut::<VariaxEditor>()
                                    .expect("Variax tab has a Variax editor");
                                let enabled = !services.picker.is_open()
                                    && !editor.busy()
                                    && !editor.blocking_prompt();
                                ui.group(|ui| {
                                    ui.horizontal(|ui| {
                                        if command_button(ui, "Open…", enabled).clicked() {
                                            editor.open_dialog(ctx, services);
                                        }
                                        let can_save = enabled && editor.has_document();
                                        if command_button(ui, "Save", can_save).clicked() {
                                            editor.save_file(ctx, services, false);
                                        }
                                        if command_button(ui, "Save As…", can_save).clicked() {
                                            editor.save_copy(ctx, services);
                                        }
                                    });
                                });
                            });
                        } else if command_button(ui, "Open file…", !self.picker.is_open()).clicked()
                        {
                            self.request_open_file(ctx);
                        }
                        self.open_commands(ui, false);
                        self.with_active(|entry, services| {
                            let enabled = !services.picker.is_open()
                                && !entry.editor.busy()
                                && !entry.editor.blocking_prompt();
                            if !variax_active && command_button(ui, "Save copy…", enabled).clicked()
                            {
                                entry.editor.save_copy(ctx, services);
                            }
                            ui.add_enabled_ui(enabled, |ui| entry.editor.actions(ui, services));
                        });
                    });
                    let response = command_button(ui, "Device check…", true);
                    #[cfg(test)]
                    self.device_check_buttons.insert("open", response.rect);
                    if response.clicked() {
                        self.device_check_open = true;
                    }
                    let response = command_button(ui, "Test recording…", true);
                    #[cfg(test)]
                    self.recording_buttons
                        .insert("open-recording", response.rect);
                    if response.clicked() {
                        self.recording_open = true;
                    }
                    if command_button(ui, "Help", true).clicked() {
                        self.help = true;
                    }
                });
            });
        egui::TopBottomPanel::top("tabs")
            .resizable(false)
            .show_separator_line(false)
            .frame(shell_bar(0))
            .show(ctx, |ui| {
                ui.set_min_height(40.0);
                self.tab_strip(ui, ctx);
            });
        egui::TopBottomPanel::bottom("status")
            .resizable(false)
            .show_separator_line(false)
            .frame(shell_bar(0))
            .show(ctx, |ui| {
                let mut guitar_refresh = None;
                let allow_refresh = !self.has_prompt();
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(theme::SPACE_16, theme::SPACE_4);
                    self.with_active(|entry, _| {
                        if entry.editor.busy() {
                            ui.spinner();
                            status_line(ui, "Reading model settings…", theme::TEXT);
                            if command_button(ui, "Cancel read", true).clicked() {
                                entry.editor.cancel_operation();
                            }
                        }
                    });
                    if let Some(index) = self.tabs.active.and_then(|id| self.tabs.index(id)) {
                        let (status, error) = self.tabs.entries[index].editor.status();
                        if !status.is_empty() {
                            status_line(ui, status, theme::MUTED);
                        }
                        if let Some(error) = error {
                            status_line(ui, error, theme::DANGER);
                        }
                        let entry = &self.tabs.entries[index];
                        let variax = entry.family.info().id == "variax";
                        if let Some(device) = &entry.device {
                            if !variax {
                                connection_chip(ui, &device.status());
                            }
                            if device.connection == tabs::Connection::Present
                                && device.reconnected_with_edits
                            {
                                status_line(ui, tabs::RECONNECTED_WITH_EDITS, theme::TEXT);
                            }
                            if device.connection == tabs::Connection::Present {
                                for notice in
                                    [device.device.pod_warning, device.device.guitar_warning]
                                        .into_iter()
                                        .flatten()
                                {
                                    status_line(ui, &notice.message(), theme::AMBER);
                                }
                            }
                            if !variax
                                && device.connection == tabs::Connection::Present
                                && let Some(status) = device.device.guitar_status.clone()
                            {
                                let attachment = device
                                    .device
                                    .refresh_guitar
                                    .then(|| device.device.key.attachment.clone());
                                ui.horizontal(|ui| {
                                    status_line(ui, &status, theme::MUTED);
                                    if let Some(attachment) = attachment
                                        && self.refresh_guitar_button(ui, allow_refresh)
                                    {
                                        guitar_refresh = Some(attachment);
                                    }
                                });
                            }
                        } else if !variax {
                            chrome::status_chip(ui, chrome::LinkState::NoDevice);
                        }
                    } else {
                        status_line(ui, "No file open", theme::MUTED);
                        chrome::status_chip(ui, chrome::LinkState::NoDevice);
                    }
                    if let Some(error) = &self.detection_status {
                        status_line(ui, error, theme::DANGER);
                    }
                    if !self.shell_status.is_empty() {
                        status_line(ui, &self.shell_status, theme::MUTED);
                    }
                    if let Some(error) = &self.shell_error {
                        status_line(ui, error, theme::DANGER);
                    }
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
                        ui.add_enabled_ui(!closing && !services.picker.is_open(), |ui| {
                            entry.editor.show(ui, services)
                        })
                    });
                });
            } else {
                self.start_page(ui);
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
                    ui.label(
                        egui::RichText::new(format!("Version {}", crate::APP_VERSION))
                            .size(theme::BODY)
                            .color(theme::TEXT),
                    );
                    ui.label(
                        egui::RichText::new(format!("Build {}", crate::BUILD_NUMBER))
                            .size(theme::BODY)
                            .color(theme::TEXT),
                    );
                    ui.separator();
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
                        ("Ctrl+Q", "Quit"),
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
        // Window closes discard their actions before any late picker result runs.
        self.poll_picker(ctx);
    }

    fn open_commands(&mut self, ui: &mut egui::Ui, start_panel: bool) {
        ui.menu_button(
            egui::RichText::new("New offline tab").size(theme::BODY),
            |ui| {
                for &family in registry::FAMILIES {
                    if command_button(ui, family.info().new_label, true).clicked() {
                        self.new_tab(family);
                        ui.close();
                    }
                }
            },
        );
        self.detect_button(ui, start_panel);
    }

    fn start_page(&mut self, ui: &mut egui::Ui) {
        ui.add_space(theme::SPACE_24);
        ui.label(
            egui::RichText::new("Open a file or start an offline tab")
                .size(theme::TITLE)
                .strong()
                .color(theme::TEXT),
        );
        ui.add_space(theme::SPACE_16);
        ui.spacing_mut().item_spacing = egui::vec2(theme::SPACE_16, theme::SPACE_16);
        let mut chosen = None;
        ui.columns(registry::FAMILIES.len(), |columns| {
            for (column, family) in columns.iter_mut().zip(registry::FAMILIES.iter().copied()) {
                if start_card(column, family).clicked() {
                    chosen = Some(family);
                }
            }
        });
        if let Some(family) = chosen {
            self.new_tab(family);
        }
    }

    fn tab_strip(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let mut select = None;
        let mut close = None;
        egui::ScrollArea::horizontal()
            .id_salt("editor-tabs")
            .show(ui, |ui| {
                ui.add_enabled_ui(!self.has_prompt(), |ui| {
                    // Top alignment keeps the row at the 40px tab height. Centered
                    // horizontal layout would stretch to the panel and shrink the page.
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = theme::SPACE_4;
                        for entry in &self.tabs.entries {
                            let title = entry.title();
                            let family_id = entry.family.info().id;
                            let tab_id = entry.id;
                            ui.push_id((family_id, tab_id), |ui| {
                                let output = chrome::DocumentTab {
                                    title: &title,
                                    family: chrome_family(family_id),
                                    active: self.tabs.active == Some(tab_id),
                                    unsaved: entry.editor.dirty(),
                                }
                                .show(ui);
                                #[cfg(test)]
                                self.tab_buttons.insert(family_id, output.response.rect);
                                if output.response.clicked() {
                                    select = Some(tab_id);
                                }
                                if output.close.clicked() {
                                    close = Some(tab_id);
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

fn shell_bar(vertical: i8) -> egui::Frame {
    egui::Frame::new()
        .fill(theme::BAR)
        .inner_margin(egui::Margin::symmetric(16, vertical))
}

fn command_button(ui: &mut egui::Ui, label: &str, enabled: bool) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(egui::RichText::new(label).size(theme::BODY))
            .min_size(egui::vec2(theme::TARGET, theme::TARGET)),
    )
}

fn status_line(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    ui.add(
        egui::Label::new(egui::RichText::new(text).size(theme::STATUS).color(color))
            .wrap_mode(egui::TextWrapMode::Wrap),
    );
}

fn connection_chip(ui: &mut egui::Ui, text: &str) {
    egui::Frame::new()
        .fill(theme::BAR)
        .stroke(egui::Stroke::new(1.0_f32, theme::LINE))
        .corner_radius(theme::RADIUS_CONTROL)
        .inner_margin(egui::Margin::symmetric(12, 4))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(text)
                    .size(theme::STATUS)
                    .color(theme::MUTED),
            );
        });
}

fn paint_product_mark(ui: &mut egui::Ui) {
    let mark = shell_image(ui.ctx(), "shell/workbench-mark.png");
    let (rect, _) = ui.allocate_exact_size(egui::vec2(28.0, 28.0), egui::Sense::hover());
    assets::paint(ui, &mark, rect);
}

fn chrome_family(id: &str) -> chrome::Family {
    match id {
        "variax" => chrome::Family::Variax,
        "podxt" => chrome::Family::Podxt,
        _ => chrome::Family::Hd,
    }
}

fn card_icon(id: &str) -> Option<&'static str> {
    match id {
        "variax" => Some("shell/variax.png"),
        "podxt" => Some("shell/podxt-live.png"),
        _ => None,
    }
}

fn card_blurb(id: &str) -> &'static str {
    match id {
        "variax" => "Open a Variax file or start with your saved library.",
        "podxt" => "Example tone. Nothing is sent, and no preset is switched.",
        _ => "File editor. Live connection not supported yet.",
    }
}

fn start_card(ui: &mut egui::Ui, family: &dyn ProductFamily) -> egui::Response {
    let info = family.info();
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 220.0),
        egui::Sense::click(),
    );
    let fill = if response.hovered() {
        egui::Color32::from_rgb(0x2c, 0x31, 0x38)
    } else {
        theme::RAISED
    };
    let painter = ui.painter();
    painter.rect_filled(rect, theme::RADIUS_PANEL, fill);
    painter.rect_stroke(
        rect,
        theme::RADIUS_PANEL,
        egui::Stroke::new(1.0_f32, theme::LINE),
        egui::StrokeKind::Inside,
    );
    if response.has_focus() {
        painter.rect_stroke(
            rect.expand(theme::FOCUS_RING),
            theme::RADIUS_PANEL,
            egui::Stroke::new(theme::FOCUS_RING, theme::FOCUS),
            egui::StrokeKind::Outside,
        );
    }
    let content = rect.shrink(theme::SPACE_16);
    let icon = egui::Rect::from_min_size(content.min, egui::vec2(64.0, 64.0));
    if let Some(path) = card_icon(info.id) {
        let image = shell_image(ui.ctx(), path);
        assets::paint(ui, &image, icon);
    } else {
        painter.rect_filled(
            icon,
            theme::RADIUS_PANEL,
            egui::Color32::from_rgb(0x2a, 0x31, 0x38),
        );
        painter.text(
            icon.center(),
            egui::Align2::CENTER_CENTER,
            "HD",
            egui::FontId::proportional(theme::TITLE),
            theme::HD,
        );
    }
    let title_rect = painter.text(
        egui::pos2(content.left(), icon.bottom() + theme::SPACE_8),
        egui::Align2::LEFT_TOP,
        info.new_label,
        egui::FontId::proportional(theme::SECTION),
        theme::TEXT,
    );
    let blurb = painter.layout(
        card_blurb(info.id).to_owned(),
        egui::FontId::proportional(theme::BODY),
        theme::MUTED,
        content.width(),
    );
    painter.galley(
        egui::pos2(content.left(), title_rect.bottom() + theme::SPACE_8),
        blurb,
        theme::MUTED,
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn shell_image(ctx: &egui::Context, relative: &'static str) -> assets::UiImage {
    #[derive(Clone)]
    struct Cached {
        name: String,
        pixels: [usize; 2],
        texture: Option<egui::TextureHandle>,
        missing: bool,
    }
    let id = egui::Id::new(("shell-art", relative));
    if let Some(cached) = ctx.data(|data| data.get_temp::<Cached>(id)) {
        return assets::UiImage {
            name: cached.name,
            pixels: cached.pixels,
            texture: cached.texture,
            missing: cached.missing,
        };
    }
    let loaded = assets::load(ctx, &assets::workspace_root(), relative);
    ctx.data_mut(|data| {
        data.insert_temp(
            id,
            Cached {
                name: loaded.name.clone(),
                pixels: loaded.pixels,
                texture: loaded.texture.clone(),
                missing: loaded.missing,
            },
        );
    });
    loaded
}
