#[path = "../src/gui.rs"]
mod editor;

use variax_workbench::BUILD_NUMBER;

const APP_VERSION: &str = if env!("CARGO_PKG_VERSION_MINOR").len() == 1 {
    concat!(
        env!("CARGO_PKG_VERSION_MAJOR"),
        ".0",
        env!("CARGO_PKG_VERSION_MINOR")
    )
} else {
    concat!(
        env!("CARGO_PKG_VERSION_MAJOR"),
        ".",
        env!("CARGO_PKG_VERSION_MINOR")
    )
};

fn main() -> eframe::Result {
    let title = format!("Variax Workbench {APP_VERSION} (build {BUILD_NUMBER})");
    editor::run(&title)
}
