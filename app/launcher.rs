use std::path::PathBuf;
use variax_workbench::session_log;

pub(crate) use variax_workbench::BUILD_NUMBER;

pub(crate) const APP_VERSION: &str = if env!("CARGO_PKG_VERSION_MINOR").len() == 1 {
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

pub(crate) fn launch(arguments: impl IntoIterator<Item = String>) -> eframe::Result {
    let title = format!("Variax Workbench {APP_VERSION} (build {BUILD_NUMBER})");
    let arguments: Vec<_> = arguments
        .into_iter()
        .filter(|arg| !cfg!(target_os = "macos") || !arg.starts_with("-psn_"))
        .collect();
    let metadata_only = arguments == ["--version"]
        || arguments
            .iter()
            .any(|arg| matches!(arg.as_str(), "--help" | "-h"));
    let logging = (!cfg!(test) && !metadata_only)
        .then(|| session_log::start(log_data_root(&arguments)))
        .flatten();
    let result = crate::editor::run(&title, arguments);
    if logging.is_some()
        && result.is_err()
        && let Some(log) = session_log::current()
    {
        log.gui_startup_failed();
    }
    result
}

fn log_data_root(arguments: &[String]) -> Option<PathBuf> {
    if let Some(data_root) = arguments.windows(2).rev().find_map(|pair| {
        (pair[0] == "--data-dir" && !pair[1].starts_with("--")).then(|| PathBuf::from(&pair[1]))
    }) {
        return if data_root.is_absolute() {
            Some(data_root)
        } else {
            std::env::current_dir().ok().map(|cwd| cwd.join(data_root))
        };
    }

    let absolute = |key: &str| {
        std::env::var_os(key)
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
    };
    log_platform_root(absolute)
}

fn log_platform_root(absolute: impl Fn(&str) -> Option<PathBuf>) -> Option<PathBuf> {
    if let Some(override_path) = absolute("VARIAX_WORKBENCH_DATA_DIR") {
        return Some(override_path);
    }
    #[cfg(target_os = "windows")]
    return absolute("LOCALAPPDATA").map(|path| path.join("VariaxWorkbench"));
    #[cfg(target_os = "macos")]
    return absolute("HOME").map(|path| path.join("Library/Application Support/VariaxWorkbench"));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    return absolute("XDG_DATA_HOME")
        .map(|path| path.join("variax-workbench"))
        .or_else(|| absolute("HOME").map(|home| home.join(".local/share/variax-workbench")));
}
