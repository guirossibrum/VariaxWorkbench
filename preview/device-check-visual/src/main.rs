//! Visual walkthrough. No USB, MIDI, or audio is opened.
//! Yes moves to the next step even when no hardware is found.

use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Device check — visual")
            .with_inner_size([960.0, 720.0])
            .with_min_inner_size([960.0, 720.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Device check — visual",
        options,
        Box::new(|_cc| Ok(Box::new(Preview::new()))),
    )
}

struct Stage {
    title: &'static str,
    risk: u8,
    risk_label: &'static str,
    intent: &'static str,
    risk_text: &'static str,
    finding: &'static str,
}

const STAGES: &[Stage] = &[
    Stage {
        title: "USB identification",
        risk: 1,
        risk_label: "Low risk",
        intent: "Look for a USB vendor and product number. This preview does not claim an interface and does not send a message.",
        risk_text: "Low risk. With no device plugged in, the list stays empty. You can still continue.",
        finding: "usb=none finding=no-device-found continue=yes",
    },
    Stage {
        title: "Device identification",
        risk: 2,
        risk_label: "Higher risk",
        intent: "Show the identity step. This preview does not send an identity request.",
        risk_text: "Higher risk than USB identification on a live instrument. Here no instrument is present, so nothing is sent.",
        finding: "identity=none finding=no-hardware continue=yes",
    },
    Stage {
        title: "Inventory check",
        risk: 3,
        risk_label: "Higher risk",
        intent: "Show how a product id would be matched to one inventory row. There is no product id in this preview.",
        risk_text: "Higher risk than identification when a name could be mistaken for permission. No row is selected.",
        finding: "registry_match=none finding=not-in-registry continue=yes",
    },
    Stage {
        title: "Settings read",
        risk: 4,
        risk_label: "High risk",
        intent: "Show the settings card. This preview does not read string1_tuning or any other setting.",
        risk_text: "High risk on a live instrument. No metadata is read because no hardware was found.",
        finding: "setting=none body_model=none finding=no-settings continue=yes",
    },
    Stage {
        title: "Change check",
        risk: 5,
        risk_label: "Highest risk",
        intent: "Show the change-check card. There is no named setting to change, and this preview does not ask the instrument to change one.",
        risk_text: "Highest risk on a live instrument. With no reading, the second pass has nothing to compare. You can still finish the walk.",
        finding: "setting=none change=not-compared continue=yes",
    },
];

struct Preview {
    index: Option<usize>,
    passed: usize,
    stopped: bool,
    report: String,
    notice: String,
}

impl Preview {
    fn new() -> Self {
        Self {
            index: Some(0),
            passed: 0,
            stopped: false,
            report: "probe=device-check-visual\nschema=preview\nwrites=no\nusb=none\nhardware=none\n".into(),
            notice: String::new(),
        }
    }

    fn choose_no(&mut self) {
        if let Some(index) = self.index {
            let stage = &STAGES[index];
            self.report.push_str(&format!(
                "stage={} risk={} decision=no finding=declined opened_device=no message_sent=no\n",
                stage.title, stage.risk
            ));
        }
        self.index = None;
        self.stopped = true;
        self.notice = "Stopped. No device was contacted.".into();
    }

    fn choose_yes(&mut self) {
        let Some(index) = self.index else {
            return;
        };
        let stage = &STAGES[index];
        self.report.push_str(&format!(
            "stage={} risk={} decision=yes {} opened_device=no message_sent=no\n",
            stage.title, stage.risk, stage.finding
        ));
        self.passed = index + 1;
        if index + 1 >= STAGES.len() {
            self.index = None;
            self.notice = "Walk finished. No USB device was found, and no hardware was contacted.".into();
        } else {
            self.index = Some(index + 1);
            self.notice = "No USB device found. The next step is open.".into();
        }
    }

    fn reset(&mut self) {
        *self = Self::new();
    }
}

impl eframe::App for Preview {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = bg();
        visuals.window_fill = bg();
        visuals.extreme_bg_color = card();
        visuals.override_text_color = Some(text());
        ctx.set_visuals(visuals);

        egui::SidePanel::left("rail")
            .exact_width(220.0)
            .frame(egui::Frame::new().fill(card()).inner_margin(12))
            .show(ctx, |ui| {
                ui.label(egui::RichText::new("Device check").strong().color(text()));
                ui.label(egui::RichText::new("Visual only").color(muted()));
                ui.add_space(10.0);
                for (index, stage) in STAGES.iter().enumerate() {
                    let (mark, color) = if self.stopped && index == self.passed {
                        ("Stopped", stop())
                    } else if index < self.passed {
                        ("Passed", pass())
                    } else if Some(index) == self.index {
                        ("Current", text())
                    } else {
                        ("Waiting", muted())
                    };
                    ui.label(egui::RichText::new(format!("{}. {}", index + 1, stage.title)).color(text()));
                    ui.label(
                        egui::RichText::new(format!("Risk {} · {mark}", stage.risk)).color(color),
                    );
                    ui.add_space(8.0);
                }
            });

        egui::TopBottomPanel::bottom("footer")
            .frame(egui::Frame::new().fill(bg()).inner_margin(10))
            .show(ctx, |ui| {
                ui.label(egui::RichText::new("This preview does not open USB, MIDI, or audio devices.").color(muted()));
                ui.label(egui::RichText::new("No hardware is required. Yes continues when nothing is found.").color(muted()));
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(bg()).inner_margin(16))
            .show(ctx, |ui| {
                if let Some(index) = self.index {
                    let stage = &STAGES[index];
                    ui.label(egui::RichText::new(stage.title).heading().color(text()));
                    ui.label(egui::RichText::new(format!("Risk {} · {}", stage.risk, stage.risk_label)).color(stop()));
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new(stage.intent).color(text()));
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(stage.risk_text).color(text()));
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("No USB device is visible. You can still continue to the next step.")
                            .color(text()),
                    );
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        let no = egui::Button::new(egui::RichText::new("No, stop").color(ink())).fill(stop());
                        if ui.add(no).clicked() {
                            self.choose_no();
                        }
                        let yes = egui::Button::new(
                            egui::RichText::new("Yes, continue at my own risk").color(text()),
                        )
                        .fill(card());
                        if ui.add(yes).clicked() {
                            self.choose_yes();
                        }
                    });
                } else {
                    ui.label(egui::RichText::new("This walk has stopped.").heading().color(text()));
                    ui.label(egui::RichText::new(&self.notice).color(muted()));
                }
                if !self.notice.is_empty() && self.index.is_some() {
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new(&self.notice).color(muted()));
                }
                ui.add_space(8.0);
                if ui.button("New check").clicked() {
                    self.reset();
                }
                ui.add_space(8.0);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.label(egui::RichText::new(&self.report).monospace().color(text()));
                });
            });
    }
}

fn bg() -> egui::Color32 {
    egui::Color32::from_rgb(0x16, 0x16, 0x14)
}
fn card() -> egui::Color32 {
    egui::Color32::from_rgb(0x23, 0x24, 0x20)
}
fn text() -> egui::Color32 {
    egui::Color32::from_rgb(0xf3, 0xf0, 0xe8)
}
fn muted() -> egui::Color32 {
    egui::Color32::from_rgb(0x9a, 0xa1, 0x94)
}
fn pass() -> egui::Color32 {
    egui::Color32::from_rgb(0x2f, 0x8f, 0x5b)
}
fn stop() -> egui::Color32 {
    egui::Color32::from_rgb(0xc4, 0xa1, 0x5a)
}
fn ink() -> egui::Color32 {
    egui::Color32::from_rgb(0x16, 0x16, 0x14)
}
