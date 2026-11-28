use super::detection::{DetectedDevice, DeviceKey, DeviceSupport};
use super::{FileDialogs, PickRequest, Picker, PickerAction, egui};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use variax_workbench::firmware_notice::FirmwareNotice;

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
    pub picker: &'a mut Picker<PickerAction>,
    #[allow(dead_code)] // Editor-lane migration, see docs/non-blocking-pickers.md.
    pub tab: TabId,
    pub close: CloseResult,
    #[cfg(test)]
    pub prompt_buttons: &'a mut std::collections::HashMap<&'static str, egui::Rect>,
}

impl EditorServices<'_> {
    /// Complete on the originating tab, even if the user has switched tabs.
    #[allow(dead_code)] // Editor-lane migration, see docs/non-blocking-pickers.md.
    pub fn request_picker<E: FamilyTab>(
        &mut self,
        ctx: &egui::Context,
        request: PickRequest,
        complete: impl FnOnce(&mut E, Option<PathBuf>, &mut EditorServices<'_>) + 'static,
    ) -> bool {
        let action = PickerAction::Tab {
            id: self.tab,
            complete: Box::new(move |editor, path, services| {
                let editor = (editor as &mut dyn std::any::Any)
                    .downcast_mut::<E>()
                    .expect("the picker completion belongs to its editor");
                complete(editor, path, services);
            }),
        };
        let started = self
            .picker
            .start(action, || self.dialogs.start_picker(request));
        if started {
            ctx.request_repaint();
        }
        started
    }

    pub fn finish_close_if(&mut self, ready: bool) {
        if ready {
            self.close = CloseResult::Ready;
        }
    }
}

pub(super) enum DeviceEvent {
    #[cfg_attr(not(any(feature = "usb", test)), allow(dead_code))]
    GuitarDisconnected(DeviceKey),
    #[cfg_attr(not(any(feature = "usb", test)), allow(dead_code))]
    GuitarCleanupWarning {
        attachment: DeviceKey,
        message: String,
    },
}

#[cfg_attr(not(feature = "usb"), allow(dead_code))]
pub(super) enum SyncAdmission {
    Busy,
    Blocked(&'static str),
    Consent(&'static str),
    Complete,
    Ready(std::num::NonZeroUsize),
    Unavailable(&'static str),
}

#[cfg_attr(not(feature = "usb"), allow(dead_code))]
pub(super) enum SyncUpdate {
    WaitingForRig,
    Reading(usize),
    Finished(Result<(), String>),
}

pub(super) const PODXT_SYNC_RESOURCE: &str = "the PODxt Live connection";

pub(super) trait FamilyTab: std::any::Any {
    fn show(&mut self, ui: &mut egui::Ui, services: &mut EditorServices<'_>);
    fn dirty(&self) -> bool;
    fn close_prompt(&mut self, ctx: &egui::Context, services: &mut EditorServices<'_>);
    fn status(&self) -> (&str, Option<&str>);
    fn source_path(&self) -> Option<&Path> {
        None
    }
    fn save_copy(&mut self, ctx: &egui::Context, services: &mut EditorServices<'_>);
    fn undo(&mut self) {}
    fn redo(&mut self) {}
    /// Called before polling the editor, with the tab's current device.
    /// Use `DeviceTab::verified_attachment` for attachment-scoped USB work
    /// and `DeviceTab::guitar_firmware_notice` for an unverified Variax reply.
    fn observe_device(&mut self, _device: Option<&DeviceTab>) {}
    fn poll(&mut self, _ctx: &egui::Context) {}
    /// One-shot device observation keyed to the attachment that produced it.
    fn take_device_event(&mut self) -> Option<DeviceEvent> {
        None
    }
    fn connection_sync_admission(&self, _consent: bool) -> SyncAdmission {
        SyncAdmission::Unavailable("Library download is unavailable for this connection.")
    }
    fn start_connection_sync(&mut self, _ctx: &egui::Context) -> Result<(), String> {
        Err("Library download is unavailable for this connection.".into())
    }
    fn connection_sync_update(&mut self) -> Option<SyncUpdate> {
        None
    }
    fn cancel_connection_sync(&mut self) {}
    /// A complete checked bank can be kept locally, even after its source
    /// attachment ceases to be current.
    fn has_session_library(&self) -> bool {
        false
    }
    fn local_prompt(&mut self, _ctx: &egui::Context, _services: &mut EditorServices<'_>) {}
    fn busy(&self) -> bool {
        false
    }
    fn shows_operation_progress(&self) -> bool {
        self.busy()
    }
    fn cancel_operation(&mut self) {}
    fn blocking_prompt(&self) -> bool {
        false
    }
    fn actions(&mut self, _ui: &mut egui::Ui, _services: &mut EditorServices<'_>) {}
}

pub(super) type TabId = u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Connection {
    Present,
    Uncertain,
    Disconnected,
}

pub(super) const RECONNECTED_WITH_EDITS: &str =
    "Reconnected. Your unsaved edits are kept; nothing was sent.";

pub(super) struct DeviceTab {
    pub device: DetectedDevice,
    pub connection: Connection,
    disconnected_at: Option<u64>,
    pub reconnected_with_edits: bool,
}

impl DeviceTab {
    fn present(device: DetectedDevice, reconnected_with_edits: bool) -> Self {
        Self {
            device,
            connection: Connection::Present,
            disconnected_at: None,
            reconnected_with_edits,
        }
    }

    /// Linux bus/port path and current USB address for a verified live tab.
    /// Disconnected, uncertain and refreshing tabs have no openable attachment.
    #[allow(dead_code)] // Used by the Follow and Refresh editor integration.
    pub fn verified_attachment(&self) -> Option<(&str, u8)> {
        if self.connection == Connection::Present
            && matches!(self.device.support, DeviceSupport::Verified(_))
        {
            return self.device.key.attachment.usb_location();
        }
        None
    }

    /// The current Variax attachment's notice, for display in its offline editor.
    pub fn guitar_firmware_notice(&self) -> Option<FirmwareNotice> {
        match (&self.connection, &self.device.support) {
            (Connection::Present, DeviceSupport::UnverifiedGuitarFirmware(notice)) => Some(*notice),
            _ => None,
        }
    }

    pub fn status(&self) -> String {
        match self.connection {
            Connection::Disconnected => "Disconnected · offline editing".into(),
            Connection::Uncertain => "USB presence could not be verified".into(),
            Connection::Present => match &self.device.support {
                DeviceSupport::IdentityPending => "USB detected · identity pending".into(),
                DeviceSupport::UsbOnly => "USB detected · identity unverified".into(),
                DeviceSupport::PresenceUncertain => "USB connection temporarily unverified".into(),
                DeviceSupport::Verified(firmware) => {
                    format!("Identity verified · firmware {firmware}")
                }
                DeviceSupport::Refreshing(firmware) => {
                    format!("Checking connection · last verified firmware {firmware}")
                }
                DeviceSupport::UnverifiedGuitarFirmware(_) => {
                    "Unverified firmware · offline editing".into()
                }
                DeviceSupport::FileEditingOnly => "Live connection not supported yet".into(),
            },
        }
    }
}

pub(super) struct TabEntry {
    pub id: TabId,
    pub family: &'static dyn ProductFamily,
    pub file: Option<PathBuf>,
    pub device: Option<DeviceTab>,
    pub editor: Box<dyn FamilyTab>,
    pub connection_library_complete: bool,
}

impl TabEntry {
    pub fn has_session_library(&self) -> bool {
        self.connection_library_complete || self.editor.has_session_library()
    }

    pub fn title(&self) -> String {
        if let Some(device) = &self.device {
            let name = device.device.model;
            return match device.connection {
                Connection::Present if device.device.support == DeviceSupport::IdentityPending => {
                    format!("{name} · identity pending")
                }
                Connection::Present
                    if matches!(device.device.support, DeviceSupport::Refreshing(_)) =>
                {
                    format!("{name} · checking")
                }
                Connection::Present if device.guitar_firmware_notice().is_some() => {
                    format!("{name} · offline only")
                }
                Connection::Present => name.into(),
                Connection::Uncertain => format!("{name} · presence unknown"),
                Connection::Disconnected => format!("{name} · disconnected"),
            };
        }
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
    disconnect_sequence: u64,
    dismissed: BTreeSet<DeviceKey>,
    parked: BTreeMap<DeviceKey, TabEntry>,
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
            device: None,
            editor,
            connection_library_complete: false,
        });
        Ok(id)
    }

    pub fn open_device(
        &mut self,
        device: DetectedDevice,
        families: &[&'static dyn ProductFamily],
        defaults: &EditorDefaults,
    ) -> Result<(TabId, bool), String> {
        if let Some(entry) = self.entries.iter().find(|entry| {
            entry.device.as_ref().is_some_and(|tab| {
                tab.device.key == device.key && tab.connection == Connection::Present
            })
        }) {
            self.active = Some(entry.id);
            return Ok((entry.id, false));
        }
        if let Some(mut entry) = self.parked.remove(&device.key) {
            self.dismissed.remove(&device.key);
            entry.device = Some(DeviceTab::present(device, false));
            let id = entry.id;
            self.entries.push(entry);
            self.active = Some(id);
            return Ok((id, false));
        }
        let family = families
            .iter()
            .copied()
            .find(|family| family.info().id == device.key.family_id)
            .ok_or_else(|| format!("No editor is registered for {}.", device.model))?;
        let id = self.open(family, defaults, TabSource::New)?;
        self.dismissed.remove(&device.key);
        let entry = self.entries.last_mut().expect("the device tab just opened");
        entry.device = Some(DeviceTab::present(device, false));
        Ok((id, true))
    }

    pub fn index(&self, id: TabId) -> Option<usize> {
        self.entries.iter().position(|entry| entry.id == id)
    }

    pub fn close(&mut self, id: TabId) {
        let Some(index) = self.index(id) else {
            return;
        };
        let mut entry = self.entries.remove(index);
        if let Some(device) = &entry.device {
            let key = device.device.key.clone();
            self.dismissed.insert(key.clone());
            if device.connection == Connection::Present
                && matches!(device.device.support, DeviceSupport::Verified(_))
                && entry.has_session_library()
                && !entry.editor.dirty()
            {
                // Retain document and library state, and release the route.
                // Only the exact currently verified key may recover this tab.
                entry.editor.observe_device(None);
                self.parked.insert(key, entry);
            }
        }
        if self.active == Some(id) {
            self.active = self
                .entries
                .get(index)
                .or_else(|| self.entries.last())
                .map(|entry| entry.id);
        }
    }

    pub fn poll_parked(&mut self, ctx: &egui::Context) {
        for entry in self.parked.values_mut() {
            entry.editor.poll(ctx);
        }
    }

    pub fn mark_presence_uncertain(&mut self) {
        for entry in &mut self.entries {
            if let Some(device) = &mut entry.device
                && device.connection == Connection::Present
            {
                device.connection = Connection::Uncertain;
            }
        }
    }

    pub fn reconcile_devices(
        &mut self,
        detected: Vec<DetectedDevice>,
        families: &[&'static dyn ProductFamily],
        defaults: &EditorDefaults,
    ) -> Result<(), String> {
        let mut remaining: BTreeMap<_, _> = detected
            .into_iter()
            .map(|device| (device.key.clone(), device))
            .collect();
        self.dismissed.retain(|key| remaining.contains_key(key));
        self.parked.retain(|key, _| remaining.contains_key(key));
        for entry in &mut self.entries {
            let Some(device) = &mut entry.device else {
                continue;
            };
            if device.connection == Connection::Disconnected {
                continue;
            }
            if let Some(present) = remaining.remove(&device.device.key) {
                device.device = present;
                device.connection = Connection::Present;
            } else {
                // Keep the editor instance, including any uncommitted input the
                // family keeps outside its applied dirty state.
                self.disconnect_sequence += 1;
                device.connection = Connection::Disconnected;
                device.disconnected_at = Some(self.disconnect_sequence);
                device.reconnected_with_edits = false;
            }
        }
        for (key, device) in remaining {
            if self.dismissed.contains(&key) {
                continue;
            }
            let recent = self
                .entries
                .iter_mut()
                .filter_map(|entry| {
                    let previous = entry.device.as_ref()?;
                    if previous.device.key.family_id != key.family_id {
                        return None;
                    }
                    Some((previous.disconnected_at?, entry))
                })
                .max_by_key(|(sequence, _)| *sequence);
            if let Some((_, entry)) = recent {
                let with_edits = match device.support {
                    DeviceSupport::Verified(_) | DeviceSupport::FileEditingOnly => {
                        entry.editor.dirty()
                    }
                    DeviceSupport::UnverifiedGuitarFirmware(_) => false,
                    // Keep the old tab offline while an automatic identity is pending.
                    _ => continue,
                };
                entry.device = Some(DeviceTab::present(device, with_edits));
                continue;
            }
            let active = self.active;
            let had_tabs = !self.entries.is_empty();
            self.open_device(device, families, defaults)?;
            // Keep the current workspace selection when editors are open.
            if had_tabs {
                self.active = active;
            }
        }
        Ok(())
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
