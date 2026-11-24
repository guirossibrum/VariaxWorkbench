use super::{FileDialogs, egui};
use std::path::{Path, PathBuf};

pub(super) struct EditorDefaults {
    pub artwork: PathBuf,
    pub preferences: Option<PathBuf>,
    pub startup_panel: Option<&'static str>,
}

pub(super) struct FamilyInfo {
    pub id: &'static str,
    pub title: &'static str,
    pub new_label: &'static str,
    pub ceiling: u64,
    pub extensions: &'static [&'static str],
}

pub(super) trait ProductFamily: Sync {
    fn info(&self) -> FamilyInfo;
    fn claims(&self, bytes: &[u8]) -> bool;
    fn open(
        &self,
        defaults: &EditorDefaults,
        source: TabSource,
    ) -> Result<Box<dyn FamilyTab>, String>;
    fn help(&self, _ui: &mut egui::Ui) {}
}

pub(super) enum TabSource {
    New,
    File(PathBuf, Vec<u8>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CloseResult {
    Pending,
    Cancelled,
    Ready,
}

pub(super) struct EditorServices<'a> {
    pub dialogs: &'a mut dyn FileDialogs,
    pub close: CloseResult,
    #[cfg(test)]
    pub prompt_buttons: &'a mut std::collections::HashMap<&'static str, egui::Rect>,
}

impl EditorServices<'_> {
    pub fn finish_close_if(&mut self, ready: bool) {
        if ready {
            self.close = CloseResult::Ready;
        }
    }
}

pub(super) trait FamilyTab: std::any::Any {
    fn show(&mut self, ui: &mut egui::Ui, services: &mut EditorServices<'_>);
    fn dirty(&self) -> bool;
    fn close_prompt(&mut self, ctx: &egui::Context, services: &mut EditorServices<'_>);
    fn status(&self) -> (&str, Option<&str>);
    fn source_path(&self) -> Option<&Path> {
        None
    }
    fn save_copy(&mut self, services: &mut EditorServices<'_>);
    fn undo(&mut self) {}
    fn redo(&mut self) {}
    fn poll(&mut self, _ctx: &egui::Context) {}
    fn local_prompt(&mut self, _ctx: &egui::Context, _services: &mut EditorServices<'_>) {}
    fn busy(&self) -> bool {
        false
    }
    fn cancel_operation(&mut self) {}
    fn blocking_prompt(&self) -> bool {
        false
    }
    fn actions(&mut self, _ui: &mut egui::Ui, _services: &mut EditorServices<'_>) {}
}

pub(super) type TabId = u64;

pub(super) struct TabEntry {
    pub id: TabId,
    pub family: &'static dyn ProductFamily,
    pub file: Option<PathBuf>,
    pub editor: Box<dyn FamilyTab>,
}

impl TabEntry {
    pub fn title(&self) -> String {
        match self
            .editor
            .source_path()
            .or(self.file.as_deref())
            .and_then(Path::file_name)
        {
            Some(name) => format!("{} · {}", self.family.info().title, name.to_string_lossy()),
            None => self.family.info().title.to_owned(),
        }
    }
}

#[derive(Default)]
pub(super) struct Tabs {
    pub entries: Vec<TabEntry>,
    pub active: Option<TabId>,
    next: TabId,
}

impl Tabs {
    pub fn open(
        &mut self,
        family: &'static dyn ProductFamily,
        defaults: &EditorDefaults,
        source: TabSource,
    ) -> Result<TabId, String> {
        let file = match &source {
            TabSource::New => None,
            TabSource::File(path, _) => Some(path.clone()),
        };
        let editor = family.open(defaults, source)?;
        let id = self.next;
        self.next += 1;
        self.active = Some(id);
        self.entries.push(TabEntry {
            id,
            family,
            file,
            editor,
        });
        Ok(id)
    }

    pub fn index(&self, id: TabId) -> Option<usize> {
        self.entries.iter().position(|entry| entry.id == id)
    }

    pub fn close(&mut self, id: TabId) {
        let Some(index) = self.index(id) else {
            return;
        };
        self.entries.remove(index);
        if self.active == Some(id) {
            self.active = self
                .entries
                .get(index)
                .or_else(|| self.entries.last())
                .map(|entry| entry.id);
        }
    }
}

pub(super) fn open_candidate(path: &Path, ceiling: u64) -> Result<Vec<u8>, String> {
    use std::io::Read;
    use variax_workbench::{open_regular_file, path_facts};

    let facts = path_facts(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if !facts.regular_file {
        return Err("Choose a regular preset, bank, or collection file.".into());
    }
    if facts.len > ceiling {
        return Err("File exceeds the offline open size limit.".into());
    }
    let mut file = open_regular_file(path)
        .map_err(|error| format!("{}: {error}", path.display()))?
        .take(ceiling + 1);
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    if bytes.len() as u64 > ceiling {
        return Err("File exceeds the offline open size limit.".into());
    }
    Ok(bytes)
}
