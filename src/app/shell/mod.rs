//! Application navigation, tab lifecycle, and shared commands.
use super::variax_panel::VariaxEditor;
use super::*;
use variax_workbench::ui::{assets, chrome, connections, theme};

mod connection_inventory;
mod detect;
pub(super) use connection_inventory::ConnectionInventory;
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
            connections: connections::Connections::default(),
            startup_connection: false,
            connection_inventory: ConnectionInventory::default(),
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
            #[cfg(test)]
            clear_window_controls: cfg!(target_os = "macos"),
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
        // Screenshot runs target the requested editor view.
        app.startup_connection = options.screenshot.is_none();
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
                } else if restart && self.device_check.identification_timed_out() {
                    self.device_check_notice =
                        "Report save canceled. Device Check is still stopped at step 2.".into();
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
        let mut services = EditorServices {
            dialogs: self.dialogs.as_mut(),
            picker: &mut self.picker,
            tab: id,
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
        self.tabs.poll_parked(ctx);
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
            while let Some(event) = entry.editor.take_device_event() {
                device_events.push((entry.id, event));
            }
        }
        for (tab, event) in device_events {
            self.apply_device_event(tab, event);
        }
        self.poll_connection_sync(ctx);
        if self.startup_connection {
            if ctx.input(|input| {
                input.viewport().close_requested()
                    || input.key_pressed(egui::Key::Q)
                        && input.modifiers.matches_exact(egui::Modifiers::CTRL)
            }) {
                self.request_close(ctx);
            }
            let action = connections::show_startup(
                ctx,
                &mut self.connections,
                Instant::now(),
                connection_artwork,
            );
            self.apply_connection_action(action, ctx);
            self.pending_dialog(ctx);
            return;
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
                // The focused text edit owns its keyboard history.
                if (undo || redo)
                    && ctx
                        .memory(|memory| memory.focused())
                        .is_none_or(|id| egui::TextEdit::load_state(ctx, id).is_none())
                {
                    if undo {
                        entry.editor.undo();
                    } else {
                        entry.editor.redo();
                    }
                }
            });
        }
        if help {
            self.help = !self.help;
        }
        if quit || ctx.input(|i| i.viewport().close_requested()) {
            self.request_close(ctx);
        }
        let active_family = self
            .tabs
            .active
            .and_then(|id| self.tabs.index(id))
            .map(|index| self.tabs.entries[index].family.info().id);
        let variax_active = active_family == Some("variax");
        // The first frame needs room for wrapped commands before the panel has a saved height.
        egui::TopBottomPanel::top("header")
            .resizable(false)
            .show_separator_line(false)
            .default_height(220.0)
            .frame(shell_bar(4, self.clears_window_controls()))
            .show(ctx, |ui| {
                if !variax_active {
                    ui.horizontal(toolbar_title);
                }
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(theme::SPACE_4, theme::SPACE_4);
                    ui.spacing_mut().button_padding = egui::vec2(6.0, 2.0);
                    ui.spacing_mut().interact_size = egui::vec2(24.0, 24.0);
                    if variax_active {
                        ui.horizontal(|ui| {
                            toolbar_title(ui);
                            ui.add_space(theme::SPACE_4);
                        });
                    }
                    if self.has_prompt() {
                        ui.add_enabled_ui(false, |ui| {
                            self.editor_commands(ui, ctx, active_family);
                        });
                    } else {
                        self.editor_commands(ui, ctx, active_family);
                    }
                    if variax_active {
                        let tools = ui.menu_button("Tools", |ui| {
                            if self.tool_commands(ui) {
                                ui.close();
                            }
                        });
                        #[cfg(test)]
                        self.recording_buttons
                            .insert("tools-menu", tools.response.rect);
                        #[cfg(not(test))]
                        let _ = tools;
                    } else {
                        self.tool_commands(ui);
                    }
                    if toolbar_command(ui, "Help", true).clicked() {
                        self.help = true;
                    }
                });
            });
        egui::TopBottomPanel::top("tabs")
            .resizable(false)
            .show_separator_line(false)
            .frame(shell_bar(0, false))
            .show(ctx, |ui| {
                ui.set_min_height(40.0);
                self.tab_strip(ui, ctx);
            });
        egui::TopBottomPanel::bottom("status")
            .resizable(false)
            .show_separator_line(false)
            // A one-line footer is 29 points. A taller first frame would
            // shrink on the next redraw and move an editor scroll offset.
            .default_height(29.0)
            .frame(shell_bar(0, false))
            .show(ctx, |ui| {
                let mut guitar_refresh = None;
                let allow_refresh = !self.has_prompt() && !self.permit_close;
                // The build number stays at the bottom-right. Status text
                // wraps in the width that remains, so a device line, an
                // error, or a long filename cannot cover it. A right-to-left
                // row hides later wrapped lines, so the caption is reserved
                // and painted on its own.
                let caption_galley = ui.painter().layout_no_wrap(
                    build_caption(),
                    egui::FontId::proportional(theme::STATUS),
                    theme::MUTED,
                );
                let caption_size = caption_galley.size();
                let bounds = ui.available_rect_before_wrap();
                let status_width = (bounds.width() - caption_size.x - theme::SPACE_16).max(1.0);
                // A tall max rect makes the first wrapped row as tall as the
                // panel, so the footer grows past the window. Keep the row
                // at one control high and let the text wrap downward.
                let status = ui.allocate_ui_with_layout(
                    egui::vec2(status_width, ui.spacing().interact_size.y),
                    egui::Layout::left_to_right(egui::Align::Center).with_main_wrap(true),
                    |ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(theme::SPACE_16, theme::SPACE_4);
                        if self.connections.progress_hidden()
                            && command_button(ui, "Show sync progress", true).clicked()
                        {
                            self.connections.show_progress();
                        }
                        self.with_active(|entry, _| {
                            if entry.editor.shows_operation_progress() {
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
                            if let Some(editor) = (entry.editor.as_ref() as &dyn std::any::Any)
                                .downcast_ref::<VariaxEditor>()
                            {
                                status_line(ui, editor.guitar_connection_status(), theme::MUTED);
                            }
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
                                    let details = device.device.mac_identity_details.clone();
                                    ui.horizontal_wrapped(|ui| {
                                        status_line(ui, &status, theme::MUTED);
                                        if let Some(details) = &details {
                                            ui.collapsing("USB details", |ui| {
                                                status_line(ui, details, theme::MUTED);
                                            });
                                        }
                                    });
                                }
                            } else if !variax {
                                chrome::status_chip(ui, chrome::LinkState::NoDevice);
                            }
                            let attachment = entry.device.as_ref().and_then(|tab| {
                                self.connection_inventory
                                    .guitar_retry_routes()
                                    .find(|relay| relay.key.attachment == tab.device.key.attachment)
                                    .map(|relay| relay.key.attachment.clone())
                            });
                            if let Some(attachment) = attachment
                                && self.retry_guitar_button(ui, allow_refresh)
                            {
                                guitar_refresh = Some(attachment);
                            }
                        } else if self
                            .connection_inventory
                            .verified_devices()
                            .next()
                            .is_none()
                        {
                            status_line(ui, "No file open", theme::MUTED);
                            chrome::status_chip(ui, chrome::LinkState::NoDevice);
                        }
                        if let Some(notice) = variax_workbench::offline_device_notice() {
                            status_line(ui, notice, theme::MUTED);
                        }
                        if let Some(service) = &self.detection
                            && self.detection_status.is_none()
                        {
                            // Reattachment waits for verified identity. Keep passive
                            // observations visible while a previous editor stays offline.
                            for device in service.passive_pods().filter(|device| {
                                !self.tabs.entries.iter().any(|tab| {
                                    tab.device.as_ref().is_some_and(|attached| {
                                        attached.connection == tabs::Connection::Present
                                            && attached.device.key == device.key
                                    })
                                })
                            }) {
                                if let Some(status) = &device.guitar_status {
                                    status_line(ui, status, theme::MUTED);
                                }
                                if let Some(details) = &device.mac_identity_details {
                                    ui.push_id(&device.key.attachment.path, |ui| {
                                        ui.collapsing("USB details", |ui| {
                                            status_line(ui, details, theme::MUTED);
                                        });
                                    });
                                }
                            }
                        }
                        if let Some(error) = &self.detection_status {
                            status_line(ui, error, theme::DANGER);
                        }
                    },
                );
                // A long filename in the wrapped row is indented from the
                // first row and can start left of the window. Keep the
                // filename and the shell error in their own rows, clear of
                // the build number.
                let mut block_bottom = status
                    .response
                    .rect
                    .bottom()
                    .max(bounds.top() + caption_size.y);
                if !self.shell_status.is_empty() {
                    let saved = ui.allocate_ui_with_layout(
                        egui::vec2(status_width, 1.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.set_min_width(status_width);
                            status_line(ui, &self.shell_status, theme::MUTED);
                        },
                    );
                    block_bottom = block_bottom.max(saved.response.rect.bottom());
                }
                if let Some(error) = &self.shell_error {
                    let error_row = ui.allocate_ui_with_layout(
                        egui::vec2(status_width, 1.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.set_min_width(status_width);
                            status_line(ui, error, theme::DANGER);
                        },
                    );
                    block_bottom = block_bottom.max(error_row.response.rect.bottom());
                }
                let caption_pos = egui::pos2(
                    bounds.right() - caption_size.x,
                    block_bottom - caption_size.y,
                );
                ui.painter()
                    .galley(caption_pos, caption_galley, theme::MUTED);
                ui.expand_to_include_rect(egui::Rect::from_min_size(caption_pos, caption_size));
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
                self.home_page(ui);
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
                        egui::RichText::new(build_caption())
                            .size(theme::BODY)
                            .color(theme::TEXT),
                    );
                    ui.separator();
                    ui.heading("Keyboard shortcuts");
                    shortcut_help(ui, active_family);
                    ui.separator();
                    for &family in registry::FAMILIES {
                        family.help(ui);
                    }
                });
        }
        self.pending_dialog(ctx);
        self.recording_window(ctx);
        self.device_check_window(ctx);
        self.detect_dialog(ctx);
        self.connection_window(ctx);
        // Window closes discard their actions before any late picker result runs.
        self.poll_picker(ctx);
    }

    fn editor_commands(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, family: Option<&str>) {
        let variax_active = family == Some("variax");
        if matches!(family, Some("variax" | "podxt")) {
            let enabled = !self.picker.is_open()
                && self
                    .tabs
                    .active
                    .and_then(|id| self.tabs.index(id))
                    .is_some_and(|index| {
                        let editor = &self.tabs.entries[index].editor;
                        !editor.busy() && !editor.blocking_prompt()
                    });
            if toolbar_command(ui, "Open…", enabled)
                .on_hover_text("Open a file in this editor")
                .clicked()
            {
                self.request_editor_open(ctx);
            }
        }
        if variax_active {
            self.with_active(|entry, services| {
                let Some(editor) =
                    (&mut *entry.editor as &mut dyn std::any::Any).downcast_mut::<VariaxEditor>()
                else {
                    return;
                };
                let enabled =
                    !services.picker.is_open() && !editor.busy() && !editor.blocking_prompt();
                let can_save = enabled && editor.has_document();
                if toolbar_command(ui, "Save", can_save).clicked() {
                    editor.save_file(ctx, services, false);
                }
                if toolbar_command(ui, "Save As…", can_save).clicked() {
                    editor.save_copy(ctx, services);
                }
            });
        }
        if family.is_some()
            && toolbar_command(ui, "Open in new tab…", !self.picker.is_open()).clicked()
        {
            self.request_open_file(ctx);
        }
        self.open_commands(ui, false);
        self.with_active(|entry, services| {
            let enabled = !services.picker.is_open()
                && !entry.editor.busy()
                && !entry.editor.blocking_prompt();
            if variax_active && enabled {
                entry.editor.actions(ui, services);
                return;
            }
            if !variax_active {
                if family != Some("podxt") && toolbar_command(ui, "Save copy…", enabled).clicked()
                {
                    entry.editor.save_copy(ctx, services);
                }
                // Keep the other family editors' actions together.
                if ui.available_size_before_wrap().x < 340.0 {
                    ui.end_row();
                }
            }
            ui.add_enabled_ui(enabled, |ui| entry.editor.actions(ui, services));
        });
    }

    fn tool_commands(&mut self, ui: &mut egui::Ui) -> bool {
        let response = toolbar_command(ui, "Device check…", true);
        #[cfg(test)]
        self.device_check_buttons.insert("open", response.rect);
        let device_check = response.clicked();
        if device_check {
            self.device_check_open = true;
        }
        let response = toolbar_command(ui, "Test recording…", true);
        #[cfg(test)]
        self.recording_buttons
            .insert("open-recording", response.rect);
        if response.clicked() {
            self.recording_open = true;
        }
        device_check || response.clicked()
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

    fn connection_window(&mut self, ctx: &egui::Context) {
        let action = connections::show(
            ctx,
            &mut self.connections,
            Instant::now(),
            connection_artwork,
        );
        self.apply_connection_action(action, ctx);
    }

    fn apply_connection_action(
        &mut self,
        action: Option<connections::Action>,
        ctx: &egui::Context,
    ) {
        use connections::Action;
        let Some(action) = action else { return };
        if self.startup_connection
            && matches!(
                action,
                Action::EnterWorkbench | Action::EditOffline | Action::OpenOfflineEditor(_)
            )
        {
            if action != Action::EnterWorkbench {
                self.skip_startup_sync();
            }
            self.startup_connection = false;
            self.connections.dismiss();
            ctx.request_repaint();
        }
        match action {
            Action::OpenEditor(attachment) => {
                if self.startup_connection
                    && !self.has_prompt()
                    && !self.permit_close
                    && let Some(tab) = self.completed_connection_tab(attachment)
                {
                    self.tabs.active = Some(tab);
                    self.startup_connection = false;
                    self.connections.continue_in_background();
                    ctx.request_repaint();
                }
            }
            Action::EnterWorkbench => {}
            Action::EditOffline => self.tabs.active = None,
            Action::OpenOfflineEditor(editor) => {
                if let Some(family) = registry::FAMILIES
                    .iter()
                    .find(|family| family.info().id == editor)
                {
                    self.new_tab(*family);
                }
            }
            action => self.connection_sync_action(action, ctx),
        }
    }

    fn home_page(&mut self, ui: &mut egui::Ui) {
        let enabled = !self.has_prompt();
        egui::ScrollArea::vertical()
            .id_salt("home-page")
            .show(ui, |ui| {
                ui.add_enabled_ui(enabled, |ui| {
                    ui.add_space(theme::SPACE_16);
                    self.connected_home(ui);
                    if command_button(ui, "Open file…", true).clicked() {
                        self.request_open_file(ui.ctx());
                    }
                    ui.add_space(theme::SPACE_16);
                    ui.label(
                        egui::RichText::new("Edit offline")
                            .size(theme::TITLE)
                            .strong()
                            .color(theme::TEXT),
                    );
                    ui.add_space(theme::SPACE_8);
                    ui.spacing_mut().item_spacing = egui::vec2(theme::SPACE_16, theme::SPACE_16);
                    let columns =
                        start_column_count(ui.available_width(), registry::FAMILIES.len());
                    let mut chosen = None;
                    for row in registry::FAMILIES.chunks(columns) {
                        ui.columns(columns, |columns| {
                            for (column, family) in columns.iter_mut().zip(row.iter().copied()) {
                                if column
                                    .push_id(family.info().id, |ui| start_card(ui, family))
                                    .inner
                                    .clicked()
                                {
                                    chosen = Some(family);
                                }
                            }
                        });
                    }
                    if let Some(family) = chosen {
                        self.new_tab(family);
                    }
                });
            });
    }

    fn connected_home(&mut self, ui: &mut egui::Ui) {
        let devices: Vec<_> = self
            .connection_inventory
            .verified_devices()
            .map(|device| {
                (
                    device.key.clone(),
                    connection_inventory::device_name(device),
                )
            })
            .collect();
        if !devices.is_empty() {
            ui.label(
                egui::RichText::new("Connected devices")
                    .size(theme::TITLE)
                    .strong()
                    .color(theme::TEXT),
            );
            for (key, name) in &devices {
                ui.push_id(("connected", key), |ui| {
                    ui.horizontal_wrapped(|ui| {
                        let open = toolbar_command(ui, &format!("Open {name}"), true);
                        #[cfg(test)]
                        ui.ctx().data_mut(|data| {
                            data.insert_temp(egui::Id::new(("test-connected-open", key)), open.id);
                        });
                        if open.clicked() {
                            self.open_connected_editor(key);
                            ui.ctx().request_repaint();
                        }
                        if devices.iter().filter(|(_, model)| model == name).count() > 1 {
                            ui.label(&key.attachment.path);
                        } else if key.podxt_upstream().is_some() {
                            ui.label("via PODxt Live");
                        }
                    });
                });
            }
            if toolbar_command(ui, "Open connected editors", true).clicked() {
                let mut first = None;
                for (key, _) in devices {
                    let opened = self.open_connected_editor(&key);
                    first = first.or(opened);
                }
                self.tabs.active = first;
                ui.ctx().request_repaint();
            }
            ui.add_space(theme::SPACE_16);
        }
        let retry_routes: Vec<_> = self
            .connection_inventory
            .guitar_retry_routes()
            .map(|relay| (relay.key.attachment.clone(), relay.guitar_status.clone()))
            .collect();
        for (attachment, status) in retry_routes {
            ui.push_id(("guitar-retry", &attachment), |ui| {
                ui.horizontal_wrapped(|ui| {
                    if let Some(status) = status {
                        status_line(ui, &status, theme::MUTED);
                    }
                    if self.retry_guitar_button(ui, true) {
                        self.refresh_guitar(&attachment);
                    }
                });
            });
        }
    }

    fn tab_strip(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let mut close = None;
        ui.add_enabled_ui(!self.has_prompt(), |ui| {
            // Home stays in view while the document tabs scroll.
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = theme::SPACE_4;
                let home = ui.add(
                    egui::Button::selectable(self.tabs.active.is_none(), "Home")
                        .min_size(egui::vec2(82.0, 40.0)),
                );
                #[cfg(test)]
                self.tab_buttons.insert("home", home.rect);
                if home.clicked() {
                    self.tabs.active = None;
                }
                egui::ScrollArea::horizontal()
                    .id_salt("editor-tabs")
                    .show(ui, |ui| {
                        ui.horizontal_top(|ui| {
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
                                    {
                                        self.tab_buttons.insert(family_id, output.response.rect);
                                        ctx.data_mut(|data| {
                                            data.insert_temp(
                                                egui::Id::new(("test-close-tab", tab_id)),
                                                output.close.rect,
                                            );
                                        });
                                    }
                                    if output.response.clicked() {
                                        self.tabs.active = Some(tab_id);
                                    }
                                    if output.close.clicked() {
                                        close = Some(tab_id);
                                    }
                                });
                            }
                        });
                    });
            });
        });
        if let Some(id) = close {
            self.close_tab(id, ctx);
        }
    }
}

/// Points kept clear of the macOS traffic lights. The margin is in points,
/// so a Retina scale does not shrink the title or the commands.
const MAC_TRAFFIC_INSET: i8 = 78;

fn shell_bar(vertical: i8, clear_window_controls: bool) -> egui::Frame {
    let left = if clear_window_controls {
        MAC_TRAFFIC_INSET
    } else {
        16
    };
    egui::Frame::new()
        .fill(theme::BAR)
        .inner_margin(egui::Margin {
            left,
            right: 16,
            top: vertical,
            bottom: vertical,
        })
}

impl Workbench {
    fn clears_window_controls(&self) -> bool {
        #[cfg(test)]
        {
            self.clear_window_controls
        }
        #[cfg(not(test))]
        {
            cfg!(target_os = "macos")
        }
    }
}

fn toolbar_title(ui: &mut egui::Ui) {
    paint_product_mark(ui);
    ui.add(
        egui::Label::new(
            egui::RichText::new("Variax Workbench")
                .size(theme::SECTION)
                .strong()
                .color(theme::TEXT),
        )
        .extend(),
    );
}

fn toolbar_command(ui: &mut egui::Ui, label: &str, enabled: bool) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(egui::RichText::new(label).size(theme::BODY))
            .min_size(egui::vec2(24.0, 24.0)),
    )
}

fn command_button(ui: &mut egui::Ui, label: &str, enabled: bool) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(egui::RichText::new(label).size(theme::BODY))
            .min_size(egui::vec2(theme::TARGET, theme::TARGET)),
    )
}

fn build_caption() -> String {
    format!("Build {}", crate::BUILD_NUMBER)
}

fn status_line(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    ui.add(
        egui::Label::new(egui::RichText::new(text).size(theme::STATUS).color(color))
            .wrap_mode(egui::TextWrapMode::Wrap),
    );
}

fn connection_artwork(ui: &egui::Ui, path: &'static str, rect: egui::Rect) -> bool {
    let image = shell_image(ui.ctx(), path);
    if image.missing {
        return false;
    }
    assets::paint(ui, &image, rect);
    true
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
    paint_shell_icon(ui, &mark, rect, "WB");
}

/// Draw shell art inside `rect`, keeping its aspect. A missing file draws
/// `fallback` inside the slot. The asset path is not painted.
pub(super) fn paint_shell_icon(
    ui: &egui::Ui,
    image: &assets::UiImage,
    rect: egui::Rect,
    fallback: &str,
) {
    if image.missing || image.texture.is_none() {
        paint_mark(ui, rect, fallback);
    } else {
        assets::paint(ui, image, rect);
    }
}

fn paint_mark(ui: &egui::Ui, rect: egui::Rect, label: &str) {
    let painter = ui.painter();
    painter.rect_filled(
        rect,
        theme::RADIUS_CONTROL,
        egui::Color32::from_rgb(0x2a, 0x31, 0x38),
    );
    painter.rect_stroke(
        rect,
        theme::RADIUS_CONTROL,
        egui::Stroke::new(1.0_f32, theme::LINE),
        egui::StrokeKind::Inside,
    );
    let max_width = (rect.width() - 8.0).max(1.0);
    let mut size = (rect.height() * 0.42).clamp(9.0, theme::TITLE);
    let galley = loop {
        let galley = painter.layout_no_wrap(
            label.to_owned(),
            egui::FontId::proportional(size),
            theme::TEXT,
        );
        if galley.size().x <= max_width || size <= 9.0 {
            break galley;
        }
        size -= 1.0;
    };
    painter.galley(rect.center() - galley.size() * 0.5, galley, theme::TEXT);
}

fn chrome_family(id: &str) -> chrome::Family {
    match id {
        "variax" => chrome::Family::Variax,
        "podxt" => chrome::Family::Podxt,
        _ => chrome::Family::Hd,
    }
}

fn shortcut_help(ui: &mut egui::Ui, family: Option<&str>) {
    let command = if ui.ctx().os().is_mac() {
        "Cmd"
    } else {
        "Ctrl"
    };
    let open = match family {
        Some("variax") => "Open a file in the active Variax editor",
        Some("podxt") => "Open a tone or bank in the active PODxt editor",
        _ => "Open a file in a new tab",
    };
    let mut rows = vec![(format!("{command}+O"), open)];
    match family {
        Some("variax") => rows.extend([
            (format!("{command}+S"), "Save the active Variax file"),
            (
                format!("{command}+Shift+S"),
                "Save the active Variax file as a new copy",
            ),
        ]),
        Some("podxt") => rows.push((
            format!("{command}+S or {command}+Shift+S"),
            "Save a new copy of the PODxt file selection",
        )),
        Some(_) => rows.push((
            format!("{command}+S or {command}+Shift+S"),
            "Save a new copy of the active file",
        )),
        None => {}
    }
    if family.is_some() {
        let (undo, redo) = if family == Some("podxt") {
            ("Undo a PODxt file edit", "Redo a PODxt file edit")
        } else {
            ("Undo an editor change", "Redo an editor change")
        };
        rows.extend([
            (format!("{command}+Z"), undo),
            (format!("{command}+Shift+Z"), redo),
        ]);
    }
    rows.extend([
        ("Ctrl+Q".into(), "Quit"),
        ("?".into(), "Toggle Help when no control has keyboard focus"),
    ]);
    for (key, action) in rows {
        ui.horizontal_wrapped(|ui| {
            ui.monospace(key);
            ui.label(action);
        });
    }
}

fn card_icon(id: &str) -> Option<&'static str> {
    match id {
        "variax" => Some("shell/variax.png"),
        "podxt" => Some("shell/podxt-live.png"),
        _ => None,
    }
}

fn card_mark(id: &str) -> &'static str {
    match id {
        "variax" => "Variax",
        "podxt" => "PODxt",
        _ => "HD",
    }
}

/// Minimum card width before the start page uses another row.
const START_CARD_MIN: f32 = 240.0;
const START_ICON: f32 = 64.0;

fn start_column_count(width: f32, families: usize) -> usize {
    if families == 0 || !width.is_finite() || width <= 0.0 {
        return 1;
    }
    let span = START_CARD_MIN + theme::SPACE_16;
    let count = ((width + theme::SPACE_16) / span).floor();
    let count = if count.is_finite() && count > 0.0 {
        count as usize
    } else {
        1
    };
    count.clamp(1, families)
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
    let width = ui.available_width();
    let inner_width = (width - theme::SPACE_16 * 2.0).max(1.0);
    let title = ui.painter().layout(
        info.new_label.to_owned(),
        egui::FontId::proportional(theme::SECTION),
        theme::TEXT,
        inner_width,
    );
    let blurb = ui.painter().layout(
        card_blurb(info.id).to_owned(),
        egui::FontId::proportional(theme::BODY),
        theme::MUTED,
        inner_width,
    );
    let height = theme::SPACE_16
        + START_ICON
        + theme::SPACE_8
        + title.size().y
        + theme::SPACE_8
        + blurb.size().y
        + theme::SPACE_16;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
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
    let icon = egui::Rect::from_min_size(content.min, egui::vec2(START_ICON, START_ICON));
    if let Some(path) = card_icon(info.id) {
        let image = shell_image(ui.ctx(), path);
        paint_shell_icon(ui, &image, icon, card_mark(info.id));
    } else {
        paint_mark(ui, icon, card_mark(info.id));
    }
    let title_pos = egui::pos2(content.left(), icon.bottom() + theme::SPACE_8);
    let title_height = title.size().y;
    painter.galley(title_pos, title, theme::TEXT);
    painter.galley(
        egui::pos2(content.left(), title_pos.y + title_height + theme::SPACE_8),
        blurb,
        theme::MUTED,
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), info.new_label)
    });
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn shell_image(ctx: &egui::Context, relative: &'static str) -> assets::UiImage {
    #[cfg(test)]
    if ctx.data(|data| data.get_temp::<bool>(egui::Id::new("shell-art-force-missing")))
        == Some(true)
    {
        return assets::UiImage::missing(relative);
    }
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

#[cfg(test)]
pub(super) fn force_missing_shell_art(ctx: &egui::Context, missing: bool) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::new("shell-art-force-missing"), missing));
}
