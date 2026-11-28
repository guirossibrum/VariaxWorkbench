use super::super::locker::{
    Document, Drag, FileBrowser, bank_group, list_height, read_file, read_library_button,
};
use super::*;
use podxt::{StoredTone, TonePayload};
use std::collections::{BTreeMap, VecDeque};
use variax_workbench::pod_preset::{
    ContainerKind, PODXT_BANKS, PodBundleLibrary, PodTone, pod_bundle_from_tones, pod_variax_link,
    save_podxt_html,
};

#[derive(Default)]
pub(super) struct Locker {
    selected: Option<Location>,
    staged: BTreeMap<u16, StagedTone>,
    queue: VecDeque<Location>,
    backup: Option<PathBuf>,
    awaiting: Option<Location>,
    new_name: String,
    status: String,
    switch_preview: Option<Location>,
}

#[derive(Clone)]
struct StagedTone {
    original: TonePayload,
    candidate: TonePayload,
}

#[derive(Default)]
pub(super) struct Actions {
    pub(super) get: bool,
    pub(super) open: Option<Document>,
}

pub(super) struct View<'a> {
    pub(super) snapshot: &'a Snapshot,
    pub(super) idle: bool,
    pub(super) can_get: bool,
    pub(super) editor_tone: Option<&'a PodTone>,
}

fn kind(bank: BankKind) -> ContainerKind {
    match bank {
        BankKind::Tones => ContainerKind::L6pa,
        BankKind::AmpSetups => ContainerKind::L6as,
        BankKind::FxSetups => ContainerKind::L6fs,
    }
}

fn decode(payload: &TonePayload, location: Location) -> Result<PodTone, String> {
    PodTone::from_podxt_payload(payload.as_bytes(), kind(location.kind()))
        .map_err(|error| error.detail())
}

fn backup_bundle(snapshot: &Snapshot) -> Result<PodBundleLibrary, String> {
    let mut banks = Vec::new();
    for bank in BankKind::ALL {
        let entries = Location::all(&[bank])
            .map(|slot| {
                let tone = snapshot.library.get(&slot.address()).ok_or_else(|| {
                    format!(
                        "Read the full device library before exporting. Missing {}.",
                        slot.slot_name()
                    )
                })?;
                decode(&TonePayload::from_dump(tone)?, slot)
            })
            .collect::<Result<Vec<_>, String>>()?;
        banks.push((bank.name(), entries));
    }
    pod_bundle_from_tones(true, &banks).map_err(|error| error.detail())
}

fn stage_bundle(
    bundle: &PodBundleLibrary,
    snapshot: &Snapshot,
) -> Result<BTreeMap<u16, StagedTone>, String> {
    if bundle.bank_count() != PODXT_BANKS.len() || snapshot.library.len() != 299 {
        return Err(
            "Send bundle requires a complete device read and an L6BA with all three banks.".into(),
        );
    }
    if bundle.as_bytes().get(8..12) != Some(b"L6BA") {
        return Err("Send bundle needs a fixed L6BA bundle.".into());
    }
    let mut staged = BTreeMap::new();
    for (index, (name, _, count)) in PODXT_BANKS.into_iter().enumerate() {
        if bundle.bank_name(index) != Some(name) || bundle.preset_count(index) != Some(count) {
            return Err(format!("Send bundle needs {count} entries in {name}."));
        }
        for slot in Location::all(&[BankKind::ALL[index]]) {
            let tone = bundle
                .tone(index, usize::from(slot.index()))
                .map_err(|error| error.detail())?;
            let original = snapshot
                .library
                .get(&slot.address())
                .ok_or("Device slot is missing.")?;
            if let Some(tone) = changed_payload(&tone, slot, original)? {
                staged.insert(slot.address(), tone);
            }
        }
    }
    Ok(staged)
}

fn changed_payload(
    file: &PodTone,
    slot: Location,
    original: &EditBuffer,
) -> Result<Option<StagedTone>, String> {
    if file.container() != kind(slot.kind()) {
        return Err(format!(
            "{} accepts {} files.",
            slot.slot_name(),
            kind(slot.kind()).tag()
        ));
    }
    let original = TonePayload::from_dump(original)?;
    // Patch known fields over the captured raw bytes. A Java-format backup
    // cannot carry the device's unassigned bytes or physical pedal position.
    let canonical = decode(&original, slot)?
        .to_podxt_payload()
        .map_err(|error| error.detail())?;
    let changed = file.to_podxt_payload().map_err(|error| error.detail())?;
    let mut target = *original.as_bytes();
    for (index, (old, new)) in canonical.into_iter().zip(changed).enumerate() {
        if old != new {
            target[index] = new;
        }
    }
    let target = TonePayload::from_bytes(target)?;
    Ok((target != original).then_some(StagedTone {
        original,
        candidate: target,
    }))
}

fn link_warning(snapshot: &Snapshot, slot: Location) -> Option<String> {
    let target = snapshot
        .library
        .get(&slot.address())
        .and_then(|tone| TonePayload::from_dump(tone).ok())?;
    let link = pod_variax_link(target.as_bytes());
    let active = snapshot
        .tone
        .as_ref()
        .and_then(|tone| TonePayload::from_dump(tone).ok())
        .map(|tone| pod_variax_link(tone.as_bytes()));
    if (link.recalls_model() || (5..127).contains(&link.family)) && active.as_ref() != Some(&link) {
        Some(format!(
            "Channel {} can recall {} on the linked guitar. Standalone recall was owner-verified. USB-triggered recall is unverified.",
            slot.label(),
            link.label
        ))
    } else {
        None
    }
}

impl Locker {
    pub(super) fn dirty(&self) -> bool {
        !self.staged.is_empty()
    }

    pub(super) fn syncing(&self) -> bool {
        !self.queue.is_empty() || self.awaiting.is_some()
    }

    pub(super) fn discard(&mut self, store: &mut store::StorePanel) {
        self.staged.clear();
        self.end_sync(store);
    }

    fn end_sync(&mut self, store: &mut store::StorePanel) {
        self.queue.clear();
        self.awaiting = None;
        store.clear_preparation();
    }

    pub(super) fn result(
        &mut self,
        result: Result<StoredTone, ()>,
        snapshot: &mut Snapshot,
        store: &mut store::StorePanel,
    ) {
        match result {
            Ok(stored) => {
                let slot = stored.location;
                if self.awaiting == Some(slot) {
                    self.staged.remove(&slot.address());
                    self.awaiting = None;
                    self.queue.pop_front();
                    self.status = format!(
                        "{} verified. {} slots still queued.",
                        slot.slot_name(),
                        self.queue.len()
                    );
                }
                snapshot.library.insert(slot.address(), stored.tone);
            }
            Err(()) if self.awaiting.is_some() => {
                self.end_sync(store);
                self.status = "Sync stopped. Unverified slots are still staged.".into();
            }
            Err(()) => {}
        }
    }

    pub(super) fn detach(&mut self, store: &mut store::StorePanel) {
        self.selected = None;
        self.switch_preview = None;
        if !store.busy() {
            self.end_sync(store);
        }
    }

    fn payload(&self, slot: Location, snapshot: &Snapshot) -> Result<TonePayload, String> {
        if let Some(staged) = self.staged.get(&slot.address()) {
            return Ok(staged.candidate.clone());
        }
        let raw = snapshot
            .library
            .get(&slot.address())
            .ok_or("Read this slot first.")?;
        TonePayload::from_dump(raw)
    }

    fn revise(
        &mut self,
        slot: Location,
        source: &PodTone,
        snapshot: &Snapshot,
    ) -> Result<(), String> {
        let original = snapshot
            .library
            .get(&slot.address())
            .ok_or("Read this slot first.")?;
        match changed_payload(source, slot, original)? {
            Some(tone) => {
                self.staged.insert(slot.address(), tone);
            }
            None => {
                self.staged.remove(&slot.address());
            }
        }
        Ok(())
    }

    fn remember(&mut self, result: Result<(), String>, success: &str) {
        self.status = match result {
            Ok(()) => success.to_owned(),
            Err(error) => error,
        };
    }

    pub(super) fn show(
        &mut self,
        ui: &mut egui::Ui,
        services: &mut EditorServices<'_>,
        view: View<'_>,
        store: &mut store::StorePanel,
        files: &mut FileBrowser,
    ) -> Actions {
        let View {
            snapshot,
            idle,
            can_get,
            editor_tone,
        } = view;
        let mut action = Actions::default();
        ui.group(|ui| {
            self.toolbar(
                ui,
                services,
                snapshot,
                can_get && !self.syncing(),
                &mut action,
            );
            ui.columns(2, |columns| {
                action.open = files.show(
                    &mut columns[0],
                    services,
                    Some(&|slot| decode(&self.payload(slot, snapshot)?, slot)),
                );
                columns[1].strong("PODxt Live");
                if !BankKind::ALL.into_iter().all(|bank| {
                    Location::all(&[bank])
                        .all(|slot| snapshot.library.contains_key(&slot.address()))
                }) {
                    columns[1].weak("Use Read full library (Get bundle) to load all three banks.");
                }
                self.device_tree(&mut columns[1], snapshot);
            });
            self.selection(ui, snapshot, editor_tone, files, &mut action);
            self.sync(ui, services, snapshot, store, idle);
            if !self.status.is_empty() {
                ui.label(&self.status);
            }
        });
        action
    }

    fn toolbar(
        &mut self,
        ui: &mut egui::Ui,
        services: &mut EditorServices<'_>,
        snapshot: &Snapshot,
        idle: bool,
        action: &mut Actions,
    ) {
        ui.horizontal_wrapped(|ui| {
            action.get = read_library_button(ui, idle);
            let complete = snapshot.library.len() == 299;
            if ui.add_enabled(complete, egui::Button::new("Save PODxt backup to file…")).clicked() {
                let snapshot = snapshot.clone();
                services.request_picker::<PodxtEditor>(ui.ctx(), PickRequest::SaveTone {
                    name: "PODxt Live Backup.l6b".into(),
                }, move |editor, path, _| {
                    if let Some(path) = path {
                        let result = backup_bundle(&snapshot)
                            .and_then(|bundle| bundle.save_copy(&path).map_err(|error| error.detail()));
                        editor.live.locker.remember(result, "Saved Line 6 Edit backup bundle.");
                    }
                });
            }
            if ui.add_enabled(complete && !self.dirty() && !self.syncing(), egui::Button::new("Stage bundle to PODxt Live…")).clicked() {
                let original = snapshot.library.clone();
                services.request_picker::<PodxtEditor>(ui.ctx(), PickRequest::OpenTone, move |editor, path, _| {
                    let Some(path) = path else { return; };
                    let live = &mut editor.live;
                    if live.locker.dirty() || live.locker.syncing() || live.snapshot.library != original {
                        live.locker.status = "The library or staged edits changed. Review and select the bundle again.".into();
                        return;
                    }
                    let result = read_file(&path)
                        .and_then(|bytes| PodBundleLibrary::parse(bytes).map_err(|error| error.detail()))
                        .and_then(|bundle| stage_bundle(&bundle, &live.snapshot));
                    match result {
                        Ok(staged) => {
                            live.locker.staged = staged;
                            live.locker.status = format!("{} changed slots staged for Sync.", live.locker.staged.len());
                        }
                        Err(error) => live.locker.status = error,
                    }
                });
            }
            if ui.add_enabled(!snapshot.library.is_empty(), egui::Button::new("Export device list")).clicked() {
                let list: Vec<_> = Location::all(&BankKind::ALL).filter_map(|slot| {
                    snapshot.library.get(&slot.address()).map(|tone| (slot.slot_name(), tone.name()))
                }).collect();
                services.request_picker::<PodxtEditor>(ui.ctx(), PickRequest::SaveTone {
                    name: "PODxt Live Tones.html".into(),
                }, move |editor, path, _| {
                    if let Some(path) = path {
                        let result = save_podxt_html(&path, list.iter().map(|(slot, name)| (slot.as_str(), name.as_str())))
                            .map_err(|error| error.detail());
                        editor.live.locker.remember(result, "Exported tone list.");
                    }
                });
            }
        });
    }

    fn device_tree(&mut self, ui: &mut egui::Ui, snapshot: &Snapshot) {
        egui::ScrollArea::vertical()
            .id_salt("locker-device")
            .max_height(list_height(ui))
            .show(ui, |ui| {
                for bank in BankKind::ALL {
                    let loaded = Location::all(&[bank])
                        .filter(|slot| snapshot.library.contains_key(&slot.address()))
                        .count();
                    bank_group(ui, bank, loaded, |ui| {
                        for slot in Location::all(&[bank]) {
                            let Some(raw) = snapshot.library.get(&slot.address()) else {
                                continue;
                            };
                            let staged = self.staged.get(&slot.address());
                            let name =
                                staged.map_or_else(|| raw.name(), |tone| tone.candidate.name());
                            let suffix = if staged.is_some() { " ! · staged" } else { "" };
                            let linked = staged
                                .map(|tone| tone.candidate.clone())
                                .or_else(|| TonePayload::from_dump(raw).ok())
                                .map(|tone| pod_variax_link(tone.as_bytes()))
                                .filter(|link| link.family != 127);
                            let label = linked.map_or_else(
                                || format!("{} · {name}{suffix}", slot.slot_name()),
                                |link| {
                                    format!(
                                        "{} · {name} · {}{suffix}",
                                        slot.slot_name(),
                                        link.label
                                    )
                                },
                            );
                            let response = ui.selectable_label(self.selected == Some(slot), label);
                            let response = response
                                .interact(egui::Sense::click_and_drag())
                                .on_hover_text(format!(
                                    "Drop here to stage changes for {}.",
                                    slot.slot_name()
                                ));
                            response.dnd_set_drag_payload(Drag::Device(slot));
                            if response.clicked() {
                                self.selected = Some(slot);
                                self.new_name = name;
                            }
                            if !self.syncing()
                                && let Some(payload) = response.dnd_release_payload::<Drag>()
                            {
                                let source = match payload.as_ref() {
                                    Drag::Device(from) => self
                                        .payload(*from, snapshot)
                                        .and_then(|tone| decode(&tone, *from)),
                                    Drag::File(path) => read_file(path).and_then(|bytes| {
                                        PodTone::parse(bytes).map_err(|error| error.detail())
                                    }),
                                };
                                let result =
                                    source.and_then(|tone| self.revise(slot, &tone, snapshot));
                                self.remember(
                                    result,
                                    &format!("Changes staged for {}.", slot.slot_name()),
                                );
                            }
                        }
                    });
                }
            });
    }

    fn selection(
        &mut self,
        ui: &mut egui::Ui,
        snapshot: &Snapshot,
        editor_tone: Option<&PodTone>,
        files: &mut FileBrowser,
        action: &mut Actions,
    ) {
        let Some(slot) = self.selected else {
            return;
        };
        let Ok(payload) = self.payload(slot, snapshot) else {
            return;
        };
        ui.label(format!(
            "PODxt Live slot: {} · {} · {}",
            slot.slot_name(),
            payload.name(),
            pod_variax_link(payload.as_bytes()).label
        ));
        ui.horizontal_wrapped(|ui| {
            if ui.button("Open PODxt copy in editor").clicked() {
                match decode(&payload, slot) {
                    Ok(tone) => action.open = Some(Document::Device(slot, tone)),
                    Err(error) => self.status = error,
                }
            }
            if ui.button("Copy device tone").clicked() {
                match decode(&payload, slot) {
                    Ok(tone) => files.clipboard = Some(tone),
                    Err(error) => self.status = error,
                }
            }
            if ui
                .add_enabled(
                    files.clipboard.is_some() && !self.syncing(),
                    egui::Button::new(format!("Stage clipboard → {}", slot.slot_name())),
                )
                .clicked()
            {
                let tone = files.clipboard.clone().expect("clipboard available");
                let result = self.revise(slot, &tone, snapshot);
                self.remember(result, &format!("Changes staged for {}.", slot.slot_name()));
            }
            if let Some(tone) = editor_tone
                && ui
                    .add_enabled(
                        !self.syncing(),
                        egui::Button::new(format!("Stage file editor tone → {}", slot.slot_name())),
                    )
                    .clicked()
            {
                let result = self.revise(slot, tone, snapshot);
                self.remember(result, "Editor tone staged for Sync.");
            }
            if slot.kind() == BankKind::Tones && ui.button("Review channel change").clicked() {
                self.switch_preview = Some(slot);
            }
        });
        ui.add_enabled_ui(!self.syncing(), |ui| {
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut self.new_name);
                if ui.button("Stage rename").clicked() {
                    let result = decode(&payload, slot).and_then(|mut tone| {
                        tone.rename(&self.new_name)
                            .map_err(|error| error.detail())?;
                        self.revise(slot, &tone, snapshot)
                    });
                    self.remember(result, "Rename staged for Sync.");
                }
            });
        });
        if let Some(target) = self.switch_preview {
            egui::Window::new("Review PODxt channel change")
                .collapsible(false)
                .show(ui.ctx(), |ui| {
                    ui.label(format!("Select {} on the PODxt", target.label()));
                    if let Some(warning) = link_warning(snapshot, target) {
                        ui.label(warning);
                    }
                    ui.add_enabled(false, egui::Button::new("Switch channel · not approved"));
                    if ui.button("Close").clicked() {
                        self.switch_preview = None;
                    }
                });
        }
    }

    fn sync(
        &mut self,
        ui: &mut egui::Ui,
        services: &mut EditorServices<'_>,
        snapshot: &Snapshot,
        store: &mut store::StorePanel,
        idle: bool,
    ) {
        if !self.dirty() {
            return;
        }
        ui.separator();
        ui.label(format!(
            "{} device slots staged · Sync is not approved",
            self.staged.len()
        ));
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    idle && !self.syncing(),
                    egui::Button::new("Sync modified slots"),
                )
                .clicked()
            {
                store.clear_preparation();
                self.queue = Location::all(&BankKind::ALL)
                    .filter(|slot| self.staged.contains_key(&slot.address()))
                    .collect();
            }
            if let Some(slot) = self.selected
                && self.staged.contains_key(&slot.address())
                && !self.syncing()
            {
                if ui
                    .add_enabled(idle, egui::Button::new("Write selected slot"))
                    .clicked()
                {
                    store.clear_preparation();
                    self.queue.push_back(slot);
                }
                if ui
                    .add_enabled(idle, egui::Button::new("Revert selected"))
                    .clicked()
                {
                    self.staged.remove(&slot.address());
                }
            }
            if ui
                .add_enabled(
                    idle && !self.syncing(),
                    egui::Button::new("Revert all staged"),
                )
                .clicked()
            {
                self.discard(store);
            }
        });
        if let Some(&slot) = self.queue.front() {
            ui.label(format!(
                "{} queued. Confirm {} separately.",
                self.queue.len(),
                slot.slot_name()
            ));
            if ui
                .add_enabled(
                    idle && self.awaiting.is_none(),
                    egui::Button::new("Choose library backup…"),
                )
                .clicked()
            {
                services.request_picker::<PodxtEditor>(
                    ui.ctx(),
                    PickRequest::Folder,
                    move |editor, path, _| {
                        let locker = &mut editor.live.locker;
                        if let Some(path) = path
                            && locker.queue.front() == Some(&slot)
                            && locker.awaiting.is_none()
                        {
                            locker.backup = Some(path);
                        }
                    },
                );
            }
            if ui
                .add_enabled(
                    idle && self.awaiting.is_none() && self.backup.is_some(),
                    egui::Button::new("Prepare next slot"),
                )
                .clicked()
            {
                let folder = self.backup.as_ref().expect("backup selected");
                let tone = self
                    .staged
                    .get(&slot.address())
                    .expect("queued slot is staged");
                let observed = snapshot
                    .library
                    .get(&slot.address())
                    .and_then(|dump| TonePayload::from_dump(dump).ok());
                let prepared = if observed.as_ref() == Some(&tone.original) {
                    store.prepare_replacement(folder, slot, tone.candidate.clone(), &tone.original)
                } else {
                    Err("The slot baseline changed. Refresh and review the staged change.".into())
                };
                match prepared {
                    Ok(()) => {
                        self.awaiting = Some(slot);
                        self.status.clear();
                    }
                    Err(error) => self.status = error,
                }
            }
            if ui
                .add_enabled(idle, egui::Button::new("Cancel Sync"))
                .clicked()
            {
                self.end_sync(store);
            }
        }
    }
}

#[cfg(test)]
mod tests;
