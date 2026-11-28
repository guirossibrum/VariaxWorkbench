#[path = "../src/gui.rs"]
mod editor;
mod launcher;

pub(crate) use launcher::{APP_VERSION, BUILD_NUMBER};

fn main() -> eframe::Result {
    launcher::launch(std::env::args().skip(1))
}
