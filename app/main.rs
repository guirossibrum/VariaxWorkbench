#[path = "../src/gui.rs"]
mod editor;

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
    editor::run(&format!("Variax Workbench {APP_VERSION}"))
}
