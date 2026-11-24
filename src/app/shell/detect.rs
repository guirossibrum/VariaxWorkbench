//! Presence updates and the consent prompt for a manual scan.
use super::super::detection::AttachmentKey;
use super::*;

const PRESENCE_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone, Copy)]
pub(in crate::editor) enum DetectConfirmation {
    Start,
    Waiting,
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

    pub(super) fn refresh_guitar_button(&mut self, ui: &mut egui::Ui, enabled: bool) -> bool {
        let response = ui.add_enabled(enabled, egui::Button::new("Refresh guitar"));
        #[cfg(test)]
        self.detection_buttons
            .insert("refresh-guitar", response.rect);
        response.clicked()
    }

    pub(super) fn refresh_guitar(&mut self, attachment: &AttachmentKey) {
        if self
            .detection
            .as_mut()
            .is_some_and(|service| service.queue_guitar_refresh(attachment))
        {
            self.refresh_detection(Scan::Automatic);
        }
    }

    pub(super) fn detect_button(&mut self, ui: &mut egui::Ui, _start_panel: bool) {
        let response = ui
            .add_enabled(
                self.detection.is_some(),
                egui::Button::new("Detect devices"),
            )
            .on_disabled_hover_text("USB presence detection is unavailable in this build.");
        #[cfg(test)]
        self.detection_buttons.insert(
            if _start_panel {
                "start-detect"
            } else {
                "detect"
            },
            response.rect,
        );
        if response.clicked() {
            self.detect_confirmation = Some(DetectConfirmation::Start);
        }
    }

    pub(super) fn detect_dialog(&mut self, ctx: &egui::Context) {
        let Some(confirmation) = self.detect_confirmation else {
            return;
        };
        let starting = matches!(confirmation, DetectConfirmation::Start);
        let dialog = egui::Modal::new(egui::Id::new("confirm-device-detection")).show(ctx, |ui| {
            ui.heading("Detect devices");
            ui.label("Tested on Linux: PODxt Live 3.01 and Variax 600 3.10.");
            ui.label("Scan cached USB information for supported devices? This scan sends no device requests.");
            ui.horizontal(|ui| {
                let yes = ui.button("Yes, scan");
                let no = ui.button("No");
                if starting {
                    no.request_focus();
                }
                #[cfg(test)]
                {
                    self.detection_buttons.insert("yes", yes.rect);
                    self.detection_buttons.insert("no", no.rect);
                }
                (yes.clicked(), no.clicked())
            }).inner
        });
        let (yes, no) = dialog.inner;
        if no || dialog.should_close() {
            self.detect_confirmation = None;
        } else if yes && !starting {
            self.detect_confirmation = None;
            self.refresh_detection(Scan::Confirmed);
        } else {
            self.detect_confirmation = Some(DetectConfirmation::Waiting);
        }
    }
}
