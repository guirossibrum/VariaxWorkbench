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

/// Raised by one on every build iteration. The product version stays 0.09.
const BUILD_NUMBER: u32 = 20;

fn main() -> eframe::Result {
    // The normal app includes the identity check for any user. The corner
    // shows this number so an older copy, which still locks step 2, is easy
    // to tell apart.
    let title = format!("Variax Workbench {APP_VERSION} (build {BUILD_NUMBER})");
    editor::run(&title)
}
