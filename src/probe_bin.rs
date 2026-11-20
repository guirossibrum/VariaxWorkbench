//! Device check. The window is available with the `gui` feature.
//! `--synthetic-self-check` runs offline and does not open a device.

use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.as_slice() {
        [flag] if flag == "--synthetic-self-check" => {
            match variax_workbench::probe_check::synthetic_self_check() {
                Ok(text) => {
                    println!("{text}");
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::from(1)
                }
            }
        }
        [] => launch(),
        _ => {
            eprintln!(
                "Usage: variax-device-probe [--synthetic-self-check]\nThe self-check does not open USB, MIDI, or audio."
            );
            ExitCode::from(2)
        }
    }
}

#[cfg(feature = "gui")]
fn launch() -> ExitCode {
    probe_window::run()
}

#[cfg(not(feature = "gui"))]
fn launch() -> ExitCode {
    eprintln!(
        "This build has no device-check window. Run variax-device-probe --features gui, or use --synthetic-self-check."
    );
    ExitCode::from(2)
}

#[cfg(feature = "gui")]
mod probe_window {
    use eframe::egui;
    use std::process::ExitCode;
    use variax_workbench::device_probe::{RailState, StageId};
    use variax_workbench::probe_check::{AUDIO_PIN, CheckScreen, FOOTER, STAGE_LIMIT};

    pub fn run() -> ExitCode {
        let screen = match CheckScreen::new() {
            Ok(screen) => screen,
            Err(error) => {
                eprintln!("{error}");
                return ExitCode::from(1);
            }
        };
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title("Device check")
                .with_inner_size([960.0, 720.0])
                .with_min_inner_size([960.0, 720.0]),
            ..Default::default()
        };
        match eframe::run_native(
            "Device check",
            options,
            Box::new(|_cc| Ok(Box::new(ProbeWindow { screen }))),
        ) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{error}");
                ExitCode::from(1)
            }
        }
    }

    struct ProbeWindow {
        screen: CheckScreen,
    }

    impl eframe::App for ProbeWindow {
        fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
            style(ctx);
            egui::SidePanel::left("rail")
                .exact_width(220.0)
                .show(ctx, |ui| rail(ui, &self.screen));
            egui::TopBottomPanel::bottom("footer").show(ctx, |ui| {
                ui.label(egui::RichText::new(FOOTER).color(muted()));
                ui.label(egui::RichText::new(AUDIO_PIN).color(muted()));
            });
            egui::CentralPanel::default().show(ctx, |ui| {
                card(ui, &mut self.screen);
            });
        }
    }

    fn rail(ui: &mut egui::Ui, screen: &CheckScreen) {
        ui.add_space(12.0);
        ui.label(egui::RichText::new("Device check").strong().color(text()));
        ui.add_space(8.0);
        for (index, state) in screen.rail().iter().enumerate() {
            let stage = StageId::LADDER[index];
            let mark = match state {
                RailState::Passed => "Passed",
                RailState::Current => "Current",
                RailState::Stopped => "Stopped",
                RailState::Locked => "Waiting",
            };
            let color = match state {
                RailState::Passed => pass(),
                RailState::Stopped => stop(),
                _ => muted(),
            };
            ui.label(
                egui::RichText::new(format!("{}. {}", index + 1, stage.title())).color(text()),
            );
            ui.label(egui::RichText::new(format!("Risk {} · {mark}", stage.risk())).color(color));
            ui.add_space(8.0);
        }
    }

    fn card(ui: &mut egui::Ui, screen: &mut CheckScreen) {
        ui.add_space(12.0);
        if let Some(copy) = screen.disclaimer() {
            ui.label(egui::RichText::new(copy.title).heading().color(text()));
            ui.label(egui::RichText::new(format!("Risk {}", copy.risk)).color(stop()));
            ui.add_space(8.0);
            ui.label(egui::RichText::new(copy.intent).color(text()));
            ui.add_space(6.0);
            ui.label(egui::RichText::new(copy.risk_text).color(text()));
            ui.add_space(6.0);
            ui.label(egui::RichText::new(STAGE_LIMIT).color(text()));
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("A green result is this stage only. It does not mean the equipment is safe or supported.")
                    .color(muted()),
            );
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                let no =
                    egui::Button::new(egui::RichText::new("No, stop").color(ink())).fill(stop());
                if ui.add(no).clicked() {
                    screen.choose_no();
                }
                let yes = egui::Button::new(
                    egui::RichText::new("Yes, continue at my own risk").color(text()),
                )
                .fill(card_color());
                if ui.add(yes).clicked() {
                    screen.choose_yes();
                }
            });
        } else {
            ui.label(
                egui::RichText::new("This check has stopped.")
                    .heading()
                    .color(text()),
            );
            ui.label(
                egui::RichText::new("The report below keeps every pass from this run.")
                    .color(muted()),
            );
        }
        ui.add_space(8.0);
        if !screen.notice().is_empty() {
            ui.label(egui::RichText::new(screen.notice()).color(text()));
        }
        ui.horizontal(|ui| {
            if ui.button("New check").clicked() {
                let _ = screen.new_check();
            }
            if ui.button("Save report").clicked() {
                save_report(screen);
            }
        });
        ui.add_space(8.0);
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.label(
                egui::RichText::new(screen.report())
                    .monospace()
                    .color(text()),
            );
        });
    }

    fn save_report(screen: &mut CheckScreen) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Save device-check report")
            .set_file_name("device-check.txt")
            .save_file()
        else {
            return;
        };
        let _ = screen.save(&path);
    }

    fn style(ctx: &egui::Context) {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = bg();
        visuals.window_fill = bg();
        visuals.extreme_bg_color = card_color();
        visuals.override_text_color = Some(text());
        ctx.set_visuals(visuals);
    }

    fn bg() -> egui::Color32 {
        egui::Color32::from_rgb(0x16, 0x16, 0x14)
    }
    fn card_color() -> egui::Color32 {
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
}
