//! Presence updates and the consent prompt for a manual scan.
use super::super::detection::AttachmentKey;
#[cfg(target_os = "macos")]
use super::super::detection::MAC_DISCOVERY_INTERVAL;
use super::super::tabs::{Connection, DeviceEvent};
use super::*;
use variax_workbench::ui::{chrome, theme};

#[cfg(target_os = "macos")]
const PRESENCE_INTERVAL: Duration = MAC_DISCOVERY_INTERVAL;
#[cfg(not(target_os = "macos"))]
const PRESENCE_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone)]
pub(in crate::editor) enum DetectConfirmation {
    Start,
    Waiting,
    Reattach {
        attachment: AttachmentKey,
        starting: bool,
    },
}

impl Workbench {
    pub(super) fn poll_detection(&mut self, ctx: &egui::Context) {
        if self.has_prompt() || self.permit_close {
            return;
        }
        if self.detection.is_none() {
            // A file or editor chosen at launch can be used immediately in
            // an offline build. Show the unavailable state when Home opens.
            if (self.startup_connection || self.tabs.active.is_none())
                && self.connections.begin_detection()
            {
                let reason = variax_workbench::offline_device_notice()
                    .unwrap_or(chrome::DETECTION_UNAVAILABLE);
                self.connections
                    .detection(connections::Detection::Unavailable(reason.into()));
            }
            return;
        }
        if Instant::now() >= self.next_detection {
            self.refresh_detection(Scan::Automatic);
        }
        ctx.request_repaint_after(
            self.next_detection
                .saturating_duration_since(Instant::now()),
        );
    }

    fn refresh_detection(&mut self, scan: Scan) {
        self.next_detection = Instant::now() + PRESENCE_INTERVAL;
        let Some(service) = &mut self.detection else {
            return;
        };
        self.connections.begin_detection();
        match service.refresh(scan, registry::DEVICE_PROFILES) {
            Ok(devices) => {
                self.connections.detection(connections::Detection::Complete);
                self.reconcile_connection_inventory(&devices);
                if matches!(scan, Scan::Confirmed) {
                    self.shell_status = if devices.is_empty() {
                        "No supported devices found.".into()
                    } else {
                        format!("Detected {} supported device(s).", devices.len())
                    };
                }
                self.detection_status = self
                    .tabs
                    .reconcile_devices(devices, registry::FAMILIES, &self.defaults)
                    .err();
            }
            Err(error) => {
                self.connections
                    .detection(connections::Detection::Failed(error.to_string()));
                self.mark_connection_inventory_uncertain();
                self.tabs.mark_presence_uncertain();
                self.detection_status = Some(format!("Device detection: {error}."));
            }
        }
    }

    pub(super) fn apply_device_event(&mut self, tab: TabId, event: DeviceEvent) {
        let key = match event {
            DeviceEvent::GuitarDisconnected(key) => key,
            DeviceEvent::GuitarCleanupWarning {
                attachment,
                message,
            } => {
                // This outcome belongs to the finished worker, even if this
                // tab has since reattached or has no verified device.
                self.connections.report_diagnostic(format!(
                    "Earlier Variax download on USB {}: {message}",
                    attachment.attachment.path,
                ));
                return;
            }
        };
        let Some(device) = self
            .tabs
            .index(tab)
            .and_then(|index| self.tabs.entries[index].device.as_ref())
        else {
            return;
        };
        if device.connection != Connection::Present || device.device.key != key {
            return;
        }
        if let Some(devices) = self
            .detection
            .as_mut()
            .and_then(|service| service.report_guitar_disconnect(&key, registry::DEVICE_PROFILES))
        {
            self.reconcile_connection_inventory(&devices);
            self.detection_status = self
                .tabs
                .reconcile_devices(devices, registry::FAMILIES, &self.defaults)
                .err();
        }
    }

    pub(super) fn retry_guitar_button(&mut self, ui: &mut egui::Ui, enabled: bool) -> bool {
        let response = ui.add_enabled(
            enabled,
            egui::Button::new(egui::RichText::new("Retry guitar connection").size(theme::BODY))
                .min_size(egui::vec2(theme::TARGET, theme::TARGET)),
        );
        #[cfg(test)]
        self.detection_buttons.insert("retry-guitar", response.rect);
        response.clicked()
    }

    pub(super) fn refresh_guitar(&mut self, attachment: &AttachmentKey) {
        let Some(service) = &mut self.detection else {
            return;
        };
        if service.needs_guitar_driver_consent(attachment) {
            self.detect_confirmation = Some(DetectConfirmation::Reattach {
                attachment: attachment.clone(),
                starting: true,
            });
        } else if service.queue_guitar_refresh(attachment, false) {
            self.refresh_detection(Scan::Automatic);
        }
    }

    pub(super) fn detect_button(&mut self, ui: &mut egui::Ui, start_panel: bool) {
        let enabled = self.detection.is_some();
        let reason =
            variax_workbench::offline_device_notice().unwrap_or(chrome::DETECTION_UNAVAILABLE);
        let response = if start_panel {
            chrome::toolbar_button(ui, "Detect devices", enabled, reason)
        } else {
            super::toolbar_command(ui, "Detect devices", enabled).on_disabled_hover_text(reason)
        };
        #[cfg(test)]
        {
            self.detection_buttons.insert(
                if start_panel {
                    "start-detect"
                } else {
                    "detect"
                },
                response.rect,
            );
            if !start_panel && self.tabs.entries.is_empty() {
                self.detection_buttons.insert("start-detect", response.rect);
            }
        }
        if response.clicked() {
            self.detect_confirmation = Some(DetectConfirmation::Start);
        }
    }

    pub(super) fn detect_dialog(&mut self, ctx: &egui::Context) {
        let Some(confirmation) = self.detect_confirmation.clone() else {
            return;
        };
        let (starting, repair) = match &confirmation {
            DetectConfirmation::Start => (true, None),
            DetectConfirmation::Waiting => (false, None),
            DetectConfirmation::Reattach {
                attachment,
                starting,
            } => (*starting, Some(attachment)),
        };
        let (title, question, accept) = if repair.is_some() {
            (
                "Reattach guitar interface driver",
                "Open and release the PODxt Live’s guitar interface so Linux can reattach its driver? The app sends no guitar commands during recovery. Once the driver is verified, the usual guitar identity check runs.",
                "Yes, reattach",
            )
        } else {
            (
                "Detect devices",
                "Scan cached USB information for supported devices? This scan sends no device requests.",
                "Yes, scan",
            )
        };
        let frame = egui::Frame::new()
            .fill(theme::RAISED)
            .stroke(egui::Stroke::new(1.0_f32, theme::LINE))
            .corner_radius(theme::RADIUS_PANEL)
            .inner_margin(egui::Margin::same(16));
        let dialog = egui::Modal::new(egui::Id::new("confirm-device-detection"))
            .frame(frame)
            .show(ctx, |ui| {
                ui.set_min_width(360.0);
                if repair.is_some() {
                    ui.set_max_width(440.0);
                }
                ui.label(
                    egui::RichText::new(title)
                        .size(theme::TITLE)
                        .strong()
                        .color(theme::TEXT),
                );
                if repair.is_none() {
                    ui.label(
                        egui::RichText::new(
                            "Tested on Linux: PODxt Live 3.01 and Variax 600 3.10.",
                        )
                        .size(theme::BODY)
                        .color(theme::TEXT),
                    );
                }
                ui.label(
                    egui::RichText::new(question)
                        .size(theme::BODY)
                        .color(theme::TEXT),
                );
                ui.horizontal(|ui| {
                    let yes = detect_choice(ui, accept);
                    let no = detect_choice(ui, "No");
                    if starting {
                        no.request_focus();
                    }
                    #[cfg(test)]
                    {
                        self.detection_buttons.insert("yes", yes.rect);
                        self.detection_buttons.insert("no", no.rect);
                    }
                    (yes.clicked(), no.clicked())
                })
                .inner
            });
        let (yes, no) = dialog.inner;
        if no || dialog.should_close() {
            self.detect_confirmation = None;
        } else if yes && !starting {
            self.detect_confirmation = None;
            if let Some(attachment) = repair {
                if self
                    .detection
                    .as_mut()
                    .is_some_and(|service| service.queue_guitar_refresh(attachment, true))
                {
                    self.refresh_detection(Scan::Automatic);
                }
            } else {
                self.refresh_detection(Scan::Confirmed);
            }
        } else {
            self.detect_confirmation = Some(match confirmation {
                DetectConfirmation::Reattach { attachment, .. } => DetectConfirmation::Reattach {
                    attachment,
                    starting: false,
                },
                _ => DetectConfirmation::Waiting,
            });
        }
    }
}

fn detect_choice(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(egui::RichText::new(label).size(theme::BODY))
            .min_size(egui::vec2(theme::TARGET, theme::TARGET)),
    )
}
