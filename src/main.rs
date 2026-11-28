mod cli;
#[cfg(feature = "gui")]
#[path = "gui.rs"]
mod editor;
#[cfg(feature = "gui")]
#[path = "../app/launcher.rs"]
mod launcher;

#[cfg(feature = "gui")]
pub(crate) use launcher::{APP_VERSION, BUILD_NUMBER};

use std::{env, ffi::OsStr, process::ExitCode};

fn main() -> ExitCode {
    let result = if env::args_os().nth(1).as_deref() == Some(OsStr::new("gui")) {
        launch_gui()
    } else {
        // The debug coefficient/parser frames exceed the Windows main-thread stack.
        // Keep the CLI's bounded workspace independent of the platform default.
        std::thread::Builder::new()
            .name("variax-offline-command".into())
            .stack_size(8 * 1024 * 1024)
            .spawn(cli::run)
            .map_err(|error| format!("cannot start command worker: {error}"))
            .and_then(|worker| worker.join().map_err(|_| "command worker panicked".into()))
            .and_then(|result| result)
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(feature = "gui")]
fn launch_gui() -> Result<(), String> {
    launcher::launch(env::args().skip(2)).map_err(|error| error.to_string())
}

#[cfg(not(feature = "gui"))]
fn launch_gui() -> Result<(), String> {
    Err("GUI unavailable in this build. Build with the gui feature to use this command.".into())
}
