//! Presence updates and the consent prompt for a manual scan.
use super::super::detection::AttachmentKey;
use super::super::tabs::{Connection, DeviceEvent};
use super::*;
use variax_workbench::ui::{chrome, theme};

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
        if self.detection.is_none() || self.has_prompt() || self.permit_close {
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
        match service.refresh(scan, registry::DEVICE_PROFILES) {
            Ok(devices) => {
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
                self.tabs.mark_presence_uncertain();
                self.detection_status = Some(format!("Device detection: {error}."));
            }
        }
    }

    pub(super) fn apply_device_event(&mut self, tab: TabId, event: DeviceEvent) {
        let DeviceEvent::GuitarDisconnected(key) = event;
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
            self.detection_status = self
                .tabs
                .reconcile_devices(devices, registry::FAMILIES, &self.defaults)
                .err();
        }
    }

    pub(super) fn refresh_guitar_button(&mut self, ui: &mut egui::Ui, enabled: bool) -> bool {
        let response = ui.add_enabled(
            enabled,
            egui::Button::new(egui::RichText::new("Refresh guitar").size(theme::BODY))
                .min_size(egui::vec2(theme::TARGET, theme::TARGET)),
        );
        #[cfg(test)]
        self.detection_buttons
            .insert("refresh-guitar", response.rect);
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
        let response = chrome::toolbar_button(
            ui,
            "Detect devices",
            self.detection.is_some(),
            if cfg!(target_os = "macos") {
                variax_workbench::MAC_CONNECTION_UNAVAILABLE
            } else {
                chrome::DETECTION_UNAVAILABLE
            },
        );
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
        #[cfg(not(test))]
        let _ = start_panel;
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
                "Ask Linux to reattach the PODxt Live’s guitar interface driver? The app sends no guitar commands during reattachment. If it succeeds, the usual guitar identity check runs.",
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
