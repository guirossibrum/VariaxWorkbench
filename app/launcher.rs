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
    crate::editor::run(&title, arguments)
}
