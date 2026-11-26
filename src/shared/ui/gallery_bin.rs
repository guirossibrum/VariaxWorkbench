//! Developer gallery for the shared controls.
//!
//! Run `cargo run --bin variax-workbench-ui-gallery`. Missing art draws the
//! placeholder from `assets/ui` until those files are on this branch.

#![forbid(unsafe_code)]

use eframe::egui;
use variax_workbench::ui::{self, gallery::GalleryState};

fn main() -> eframe::Result {
    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1100.0, 720.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Variax Workbench components",
        native,
        Box::new(|cc| {
            ui::theme::apply(&cc.egui_ctx);
            let mut state = GalleryState::default();
            let root = ui::assets::workspace_root();
            let loaded = ui::assets::load(&cc.egui_ctx, &root, "pod-farm/sine-chorus.png");
            if !loaded.missing {
                state.wide = loaded;
            }
            let body = ui::assets::load(&cc.egui_ctx, &root, "variax/spank-body.png");
            if !body.missing {
                state.body = body;
            }
            state.load_pickup(&cc.egui_ctx, &root);
            Ok(Box::new(GalleryApp { state }))
        }),
    )
}

struct GalleryApp {
    state: GalleryState,
}

impl eframe::App for GalleryApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.state.show(ctx);
    }
}
