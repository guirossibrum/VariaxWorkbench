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

/// Release workers receive one build number along with the pinned source SHA.
const BUILD_NUMBER: &str = match option_env!("VARIAX_BUILD_NUMBER") {
    Some(number) => number,
    None => "dev",
};

fn main() -> eframe::Result {
    let title = format!("Variax Workbench {APP_VERSION} (build {BUILD_NUMBER})");
    editor::run(&title)
}
