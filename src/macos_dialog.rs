//! Catalina-compatible Finder-style dialogs through macOS Standard Additions.

use std::path::PathBuf;

pub enum Dialog {
    Open,
    Folder,
    Save(String),
}

impl Dialog {
    pub fn select(self) -> Option<PathBuf> {
        let (action, name) = match &self {
            Self::Open => ("choose file", None),
            Self::Folder => ("choose folder", None),
            Self::Save(name) => ("choose file name default name (item 1 of argv)", Some(name)),
        };
        let script = format!(
            "on run argv\nactivate\nset chosenPath to {action}\nreturn POSIX path of chosenPath\nend run"
        );
        let output = std::process::Command::new("/usr/bin/osascript")
            .args(["-e", &script, "--"])
            .args(name)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let path = String::from_utf8(output.stdout).ok()?;
        let path = path.strip_suffix('\n').unwrap_or(&path);
        (!path.is_empty()).then(|| PathBuf::from(path))
    }
}
