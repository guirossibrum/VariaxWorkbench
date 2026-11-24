use super::*;
use variax_workbench::pod_preset::{HdEditFile, HdEditorLibrary};

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PendingHdType9Choice {
    pub(super) file_index: usize,
    pub(super) slot: pod_preset::HdFxSlot,
    pub(super) model_id: u32,
    pub(super) choice: pod_preset::HdType9Choice,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PendingHdType8Choice {
    pub(super) file_index: usize,
    pub(super) slot: pod_preset::HdFxSlot,
    pub(super) model_id: u32,
    pub(super) choice: pod_preset::HdType8Choice,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PendingHdType10Choice {
    pub(super) file_index: usize,
    pub(super) slot: pod_preset::HdFxSlot,
    pub(super) model_id: u32,
    pub(super) choice: pod_preset::HdType10Choice,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PendingHdType11Choice {
    pub(super) file_index: usize,
    pub(super) slot: pod_preset::HdFxSlot,
    pub(super) model_id: u32,
    pub(super) control: pod_preset::HdType11Control,
    pub(super) choice: pod_preset::HdType11Choice,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PendingHdType12Choice {
    pub(super) file_index: usize,
    pub(super) slot: pod_preset::HdFxSlot,
    pub(super) model_id: u32,
    pub(super) control: pod_preset::HdType12Control,
    pub(super) choice: pod_preset::HdType12Choice,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PendingHdType13Choice {
    pub(super) file_index: usize,
    pub(super) slot: pod_preset::HdFxSlot,
    pub(super) model_id: u32,
    pub(super) control: pod_preset::HdType13Control,
    pub(super) choice: pod_preset::HdType13Choice,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PendingHdType19Choice {
    pub(super) file_index: usize,
    pub(super) slot: pod_preset::HdFxSlot,
    pub(super) model_id: u32,
    pub(super) control: pod_preset::HdType19Control,
    pub(super) choice: pod_preset::HdType19Choice,
}

#[cfg(test)]
fn hd_type12_combo_button() -> &'static str {
    "hd-type12-combo"
}

#[cfg(test)]
fn hd_type12_option_button(choice: pod_preset::HdType12Choice) -> &'static str {
    ["hd-type12-option-0", "hd-type12-option-1"][choice.index() as usize]
}

fn hd_type12_apply_button() -> &'static str {
    "apply-hd-type12"
}

fn hd_type12_discard_button() -> &'static str {
    "discard-hd-type12"
}

#[cfg(test)]
fn hd_type13_option_button(choice: pod_preset::HdType13Choice) -> &'static str {
    [
        "hd-type13-option-0",
        "hd-type13-option-1",
        "hd-type13-option-2",
        "hd-type13-option-3",
    ][choice.index() as usize]
}

fn hd_type13_apply_button() -> &'static str {
    "apply-hd-type13"
}

fn hd_type13_discard_button() -> &'static str {
    "discard-hd-type13"
}

#[cfg(test)]
fn hd_type19_option_button(choice: pod_preset::HdType19Choice) -> &'static str {
    [
        "hd-type19-option-0",
        "hd-type19-option-1",
        "hd-type19-option-2",
        "hd-type19-option-3",
        "hd-type19-option-4",
        "hd-type19-option-5",
        "hd-type19-option-6",
        "hd-type19-option-7",
    ][choice.index() as usize]
}

fn hd_type19_apply_button() -> &'static str {
    "apply-hd-type19"
}

fn hd_type19_discard_button() -> &'static str {
    "discard-hd-type19"
}

fn hd_type11_control_index(control: pod_preset::HdType11Control) -> usize {
    match control {
        pod_preset::HdType11Control::Model020a001eKey3f100002 => 0,
        pod_preset::HdType11Control::Model020a001eKey3f100003 => 1,
        pod_preset::HdType11Control::Model020a001dKey3f100001 => 2,
        pod_preset::HdType11Control::Model020a001dKey3f100002 => 3,
    }
}

#[cfg(test)]
fn hd_type11_combo_button(control: pod_preset::HdType11Control) -> &'static str {
    [
        "hd-type11-combo-0",
        "hd-type11-combo-1",
        "hd-type11-combo-2",
        "hd-type11-combo-3",
    ][hd_type11_control_index(control)]
}

#[cfg(test)]
fn hd_type11_option_button(
    control: pod_preset::HdType11Control,
    choice: pod_preset::HdType11Choice,
) -> &'static str {
    const IDS: [[&str; 5]; 4] = [
        [
            "hd-type11-option-0-0",
            "hd-type11-option-0-1",
            "hd-type11-option-0-2",
            "hd-type11-option-0-3",
            "hd-type11-option-0-4",
        ],
        [
            "hd-type11-option-1-0",
            "hd-type11-option-1-1",
            "hd-type11-option-1-2",
            "hd-type11-option-1-3",
            "hd-type11-option-1-4",
        ],
        [
            "hd-type11-option-2-0",
            "hd-type11-option-2-1",
            "hd-type11-option-2-2",
            "hd-type11-option-2-3",
            "hd-type11-option-2-4",
        ],
        [
            "hd-type11-option-3-0",
            "hd-type11-option-3-1",
            "hd-type11-option-3-2",
            "hd-type11-option-3-3",
            "hd-type11-option-3-4",
        ],
    ];
    IDS[hd_type11_control_index(control)][choice.index() as usize]
}

fn hd_type11_apply_button(control: pod_preset::HdType11Control) -> &'static str {
    [
        "apply-hd-type11-0",
        "apply-hd-type11-1",
        "apply-hd-type11-2",
        "apply-hd-type11-3",
    ][hd_type11_control_index(control)]
}

fn hd_type11_discard_button(control: pod_preset::HdType11Control) -> &'static str {
    [
        "discard-hd-type11-0",
        "discard-hd-type11-1",
        "discard-hd-type11-2",
        "discard-hd-type11-3",
    ][hd_type11_control_index(control)]
}
pub(super) struct HdEditor {
    hd_library: HdEditorLibrary,
    hd_selected: Option<usize>,
    hd_effect_slot: usize,
    hd_default_target: pod_preset::HdDistortionDefault,
    hd_dynamics_default_target: pod_preset::HdDynamicsDefault,
    hd_reverb_default_target: pod_preset::HdReverbDefault,
    hd_control: pod_preset::HdType1Control,
    hd_mapped_control: pod_preset::HdType1MappedControl,
    hd_control_percent: u8,
    hd_numeric_control: pod_preset::HdNumericControl,
    hd_numeric_value: i32,
    hd_type9_pending: Option<PendingHdType9Choice>,
    hd_type8_pending: Option<PendingHdType8Choice>,
    hd_type10_pending: Option<PendingHdType10Choice>,
    hd_type11_pending:
        HashMap<(usize, pod_preset::HdFxSlot, pod_preset::HdType11Control), PendingHdType11Choice>,
    hd_type12_pending:
        HashMap<(usize, pod_preset::HdFxSlot, pod_preset::HdType12Control), PendingHdType12Choice>,
    hd_type13_pending:
        HashMap<(usize, pod_preset::HdFxSlot, pod_preset::HdType13Control), PendingHdType13Choice>,
    hd_type19_pending:
        HashMap<(usize, pod_preset::HdFxSlot, pod_preset::HdType19Control), PendingHdType19Choice>,
    hd_setlist_index: usize,
    hd_preset_index: usize,
    hd_message: String,
    #[cfg(test)]
    hd_buttons: HashMap<&'static str, Rect>,
    #[cfg(test)]
    hd_scroll_area: Option<(egui::Id, Rect, f32)>,
}

impl HdEditor {
    fn new() -> Self {
        Self {
            hd_library: Session::initial_rig().new_hd_editor_library(),
            hd_selected: None,
            hd_effect_slot: 0,
            hd_default_target: pod_preset::HdDistortionDefault::JetFuzz,
            hd_dynamics_default_target: pod_preset::HdDynamicsDefault::HardGate,
            hd_reverb_default_target: pod_preset::HdReverbDefault::Octo,
            hd_control: pod_preset::HdType1Control::Screamer(pod_preset::ScreamerControl::Drive),
            hd_mapped_control: pod_preset::HD_TYPE1_MAPPED_CONTROLS[0],
            hd_control_percent: 0,
            hd_numeric_control: pod_preset::HD_NUMERIC_CONTROLS[0],
            hd_numeric_value: 0,
            hd_type9_pending: None,
            hd_type8_pending: None,
            hd_type10_pending: None,
            hd_type11_pending: HashMap::new(),
            hd_type12_pending: HashMap::new(),
            hd_type13_pending: HashMap::new(),
            hd_type19_pending: HashMap::new(),
            hd_setlist_index: 0,
            hd_preset_index: 0,
            hd_message: String::new(),
            #[cfg(test)]
            hd_buttons: HashMap::new(),
            #[cfg(test)]
            hd_scroll_area: None,
        }
    }

    pub(super) fn show_hd_panel(&mut self, services: &mut EditorServices<'_>, ui: &mut egui::Ui) {
        if self.hd_button(ui, "load-hd-file", "Open HD Edit file…") {
            self.load_hd_editor_file(services);
        }
        let selected_enable = self.hd_selected.and_then(|index| {
            let slot = pod_preset::HdFxSlot::new(self.hd_effect_slot).ok()?;
            let HdEditFile::H5ep(preset) = self.hd_library.file(index)? else {
                return None;
            };
            Some((index, slot, preset.effect_enabled(slot)?))
        });
        if let Some((index, slot, enabled)) = selected_enable {
            let (button_id, label) = if enabled {
                ("disable-hd-effect", "Disable effect offline")
            } else {
                ("enable-hd-effect", "Enable effect offline")
            };
            if self.hd_button(ui, button_id, label) {
                let result = Session::extension(ProductProfile::PodHd500x).and_then(|session| {
                    session.set_hd_effect_enabled(&mut self.hd_library, index, slot, !enabled)
                });
                match result {
                    Ok(()) => {
                        self.hd_message = format!(
                            "Effect {} updated in this offline preset. Nothing was sent; save a new copy to keep the change.",
                            if enabled { "disabled" } else { "enabled" }
                        );
                    }
                    Err(error) => self.hd_message = hd_rejection_text(&error),
                }
            }
        }
        if let Some(pending) = self.hd_type9_pending
            && self.hd_selected == Some(pending.file_index)
            && self.hd_effect_slot == pending.slot.index()
        {
            ui.label(format!(
                "{} for FX slot {} is staged.",
                pending.choice.label(),
                pending.slot.index()
            ));
            ui.horizontal(|ui| {
                if self.hd_button(ui, "apply-hd-type9-choice", "Apply offline choice") {
                    self.apply_hd_type9_choice(
                        pending.file_index,
                        pending.slot,
                        pending.model_id,
                        pending.choice,
                    );
                }
                if self.hd_button(ui, "discard-hd-type9-choice", "Discard staged choice") {
                    self.hd_type9_pending = None;
                    self.hd_message = "Discarded the staged HD type-9 choice.".into();
                }
            });
        }
        if let Some(pending) = self.hd_type8_pending
            && self.hd_selected == Some(pending.file_index)
            && self.hd_effect_slot == pending.slot.index()
        {
            ui.label(format!(
                "{} for FX slot {} is staged.",
                pending.choice.label(),
                pending.slot.index()
            ));
            ui.horizontal(|ui| {
                if self.hd_button(ui, "apply-hd-type8-choice", "Apply offline choice") {
                    self.apply_hd_type8_choice(
                        pending.file_index,
                        pending.slot,
                        pending.model_id,
                        pending.choice,
                    );
                }
                if self.hd_button(ui, "discard-hd-type8-choice", "Discard staged choice") {
                    self.hd_type8_pending = None;
                    self.hd_message = "Discarded the staged HD type-8 choice.".into();
                }
            });
        }
        if let Some(pending) = self.hd_type10_pending
            && self.hd_selected == Some(pending.file_index)
            && self.hd_effect_slot == pending.slot.index()
        {
            ui.label(format!(
                "{} for FX slot {} is staged.",
                pending.choice.label(),
                pending.slot.index()
            ));
            ui.horizontal(|ui| {
                if self.hd_button(ui, "apply-hd-type10-choice", "Apply offline choice") {
                    self.apply_hd_type10_choice(
                        pending.file_index,
                        pending.slot,
                        pending.model_id,
                        pending.choice,
                    );
                }
                if self.hd_button(ui, "discard-hd-type10-choice", "Discard staged choice") {
                    self.hd_type10_pending = None;
                    self.hd_message = "Discarded the staged HD type-10 choice.".into();
                }
            });
        }
        if self
            .hd_selected
            .is_some_and(|index| self.hd_library.file(index).is_some())
            && self.hd_button(ui, "save-hd-copy", "Save HD file copy…")
        {
            self.write_hd_copy(services, self.hd_selected.expect("selected HD file"));
        }

        ui.separator();
        let scroll = egui::ScrollArea::vertical()
            .max_height(560.0)
            .id_salt("pod-hd-editor-panel")
            .show(ui, |ui| self.hd_editor_panel(ui));
        #[cfg(test)]
        {
            self.hd_scroll_area = Some((scroll.id, scroll.inner_rect, scroll.content_size.y));
        }
        #[cfg(not(test))]
        let _ = scroll;
    }

    pub(super) fn save_next_dirty_hd(&mut self, services: &mut EditorServices<'_>) -> bool {
        if let Some(index) = self.hd_library.first_dirty() {
            return self.write_hd_copy(services, index);
        }
        false
    }

    pub(super) fn discard_next_dirty_hd(&mut self) -> bool {
        let Some(index) = self.hd_library.first_dirty() else {
            return false;
        };
        match self.hd_library.revert_unsaved(index) {
            Ok(()) => {
                if self
                    .hd_type9_pending
                    .is_some_and(|pending| pending.file_index == index)
                {
                    self.hd_type9_pending = None;
                }
                if self
                    .hd_type8_pending
                    .is_some_and(|pending| pending.file_index == index)
                {
                    self.hd_type8_pending = None;
                }
                if self
                    .hd_type10_pending
                    .is_some_and(|pending| pending.file_index == index)
                {
                    self.hd_type10_pending = None;
                }
                self.hd_type11_pending
                    .retain(|(file_index, _, _), _| *file_index != index);
                self.hd_type12_pending
                    .retain(|(file_index, _, _), _| *file_index != index);
                self.hd_type13_pending
                    .retain(|(file_index, _, _), _| *file_index != index);
                self.hd_type19_pending
                    .retain(|(file_index, _, _), _| *file_index != index);
                self.hd_message = "Discarded one unsaved HD Edit file change.".into();
                true
            }
            Err(error) => {
                self.hd_message = error.detail();
                false
            }
        }
    }

    pub(super) fn hd_quit_dialog(
        &mut self,
        services: &mut EditorServices<'_>,
        ctx: &egui::Context,
    ) {
        if let Some(staged) = self.hd_type8_pending {
            let mut apply = false;
            let mut discard = false;
            let mut cancel = false;
            egui::Window::new("Staged HD type-8 choice")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(format!(
                        "{} for H5EP entry {} / FX slot {} is staged but not applied.",
                        staged.choice.label(),
                        staged.file_index,
                        staged.slot.index()
                    ));
                    ui.label("Apply updates only this offline preset value. Discard clears the staged choice.");
                    ui.horizontal(|ui| {
                        let response = ui.button("Apply staged choice");
                        #[cfg(test)]
                        services.prompt_buttons.insert("apply-type8", response.rect);
                        apply = response.clicked();
                        let response = ui.button("Keep editing");
                        #[cfg(test)]
                        services.prompt_buttons.insert("cancel", response.rect);
                        cancel = response.clicked();
                        let response = ui.button("Discard staged choice");
                        #[cfg(test)]
                        services.prompt_buttons.insert("discard", response.rect);
                        discard = response.clicked();
                    });
                });
            if cancel {
                services.close = CloseResult::Cancelled;
            } else if apply {
                self.apply_hd_type8_choice(
                    staged.file_index,
                    staged.slot,
                    staged.model_id,
                    staged.choice,
                );
                services.finish_close_if(!self.needs_close());
            } else if discard {
                self.hd_type8_pending = None;
                self.hd_message = "Discarded the staged HD type-8 choice.".into();
                services.finish_close_if(!self.needs_close());
            }
            return;
        }
        if let Some(staged) = self.hd_type9_pending {
            let mut apply = false;
            let mut discard = false;
            let mut cancel = false;
            egui::Window::new("Staged HD type-9 choice")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(format!(
                        "{} for H5EP entry {} / FX slot {} is staged but not applied.",
                        staged.choice.label(),
                        staged.file_index,
                        staged.slot.index()
                    ));
                    ui.label("Apply updates only this offline preset value. Discard clears the staged choice.");
                    ui.horizontal(|ui| {
                        let response = ui.button("Apply staged choice");
                        #[cfg(test)]
                        services.prompt_buttons.insert("apply-type9", response.rect);
                        apply = response.clicked();
                        let response = ui.button("Keep editing");
                        #[cfg(test)]
                        services.prompt_buttons.insert("cancel", response.rect);
                        cancel = response.clicked();
                        let response = ui.button("Discard staged choice");
                        #[cfg(test)]
                        services.prompt_buttons.insert("discard", response.rect);
                        discard = response.clicked();
                    });
                });
            if cancel {
                services.close = CloseResult::Cancelled;
            } else if apply {
                self.apply_hd_type9_choice(
                    staged.file_index,
                    staged.slot,
                    staged.model_id,
                    staged.choice,
                );
                services.finish_close_if(!self.needs_close());
            } else if discard {
                self.hd_type9_pending = None;
                self.hd_message = "Discarded the staged HD type-9 choice.".into();
                services.finish_close_if(!self.needs_close());
            }
            return;
        }
        if let Some(staged) = self.hd_type10_pending {
            let mut apply = false;
            let mut discard = false;
            let mut cancel = false;
            egui::Window::new("Staged HD type-10 choice")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(format!(
                        "{} for H5EP entry {} / FX slot {} is staged but not applied.",
                        staged.choice.label(),
                        staged.file_index,
                        staged.slot.index()
                    ));
                    ui.label("Apply changes only the selected offline H5EP value.");
                    ui.horizontal(|ui| {
                        let r = ui.button("Apply staged choice");
                        #[cfg(test)]
                        services.prompt_buttons.insert("apply-type10", r.rect);
                        apply = r.clicked();
                        let r = ui.button("Keep editing");
                        #[cfg(test)]
                        services.prompt_buttons.insert("cancel", r.rect);
                        cancel = r.clicked();
                        let r = ui.button("Discard staged choice");
                        #[cfg(test)]
                        services.prompt_buttons.insert("discard", r.rect);
                        discard = r.clicked();
                    });
                });
            if cancel {
                services.close = CloseResult::Cancelled;
            } else if apply {
                self.apply_hd_type10_choice(
                    staged.file_index,
                    staged.slot,
                    staged.model_id,
                    staged.choice,
                );
                services.finish_close_if(!self.needs_close());
            } else if discard {
                self.hd_type10_pending = None;
                self.hd_message = "Discarded the staged HD type-10 choice.".into();
                services.finish_close_if(!self.needs_close());
            }
            return;
        }
        if let Some(staged) = self.hd_type11_pending.values().next().copied() {
            let mut apply = false;
            let mut discard = false;
            let mut cancel = false;
            egui::Window::new("Staged HD type-11 choice")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(format!(
                        "{} ({}) for H5EP entry {} / FX slot {} is staged but not applied.",
                        staged.choice.label(),
                        staged.control.label(),
                        staged.file_index,
                        staged.slot.index()
                    ));
                    ui.label("Apply changes only the selected offline H5EP value.");
                    ui.horizontal(|ui| {
                        let r = ui.button("Apply staged choice");
                        #[cfg(test)]
                        services.prompt_buttons.insert("apply-type11", r.rect);
                        apply = r.clicked();
                        let r = ui.button("Keep editing");
                        #[cfg(test)]
                        services.prompt_buttons.insert("cancel", r.rect);
                        cancel = r.clicked();
                        let r = ui.button("Discard staged choice");
                        #[cfg(test)]
                        services.prompt_buttons.insert("discard", r.rect);
                        discard = r.clicked();
                    });
                });
            if cancel {
                services.close = CloseResult::Cancelled;
            } else if apply {
                self.apply_hd_type11_choice(staged);
                services.finish_close_if(!self.needs_close());
            } else if discard {
                self.hd_type11_pending
                    .remove(&(staged.file_index, staged.slot, staged.control));
                self.hd_message =
                    format!("Discarded the staged {} choice.", staged.control.label());
                services.finish_close_if(!self.needs_close());
            }
            return;
        }
        if let Some(staged) = self.hd_type12_pending.values().next().copied() {
            let mut apply = false;
            let mut discard = false;
            let mut cancel = false;
            egui::Window::new("Staged HD type-12 choice")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(format!(
                        "{} for H5EP entry {} / FX slot {} is staged but not applied.",
                        staged.choice.label(),
                        staged.file_index,
                        staged.slot.index()
                    ));
                    ui.label("Apply changes only the selected offline H5EP value.");
                    ui.horizontal(|ui| {
                        let r = ui.button("Apply staged choice");
                        #[cfg(test)]
                        services.prompt_buttons.insert("apply-type12", r.rect);
                        apply = r.clicked();
                        let r = ui.button("Keep editing");
                        #[cfg(test)]
                        services.prompt_buttons.insert("cancel", r.rect);
                        cancel = r.clicked();
                        let r = ui.button("Discard staged choice");
                        #[cfg(test)]
                        services.prompt_buttons.insert("discard", r.rect);
                        discard = r.clicked();
                    });
                });
            let key = (staged.file_index, staged.slot, staged.control);
            if cancel {
                services.close = CloseResult::Cancelled;
            } else if apply {
                self.apply_hd_type12_choice(staged);
                services.finish_close_if(!self.needs_close());
            } else if discard {
                self.hd_type12_pending.remove(&key);
                self.hd_message = "Discarded the staged HD type-12 choice.".into();
                services.finish_close_if(!self.needs_close());
            }
            return;
        }
        if let Some(staged) = self.hd_type13_pending.values().next().copied() {
            let mut apply = false;
            let mut discard = false;
            let mut cancel = false;
            egui::Window::new("Staged HD type-0x0D choice")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(format!("{} for H5EP entry {} / FX slot {} is staged but not applied.", staged.choice.label(), staged.file_index, staged.slot.index()));
                    ui.label("Static-derived neutral choice; Apply changes only this offline H5EP value word.");
                    ui.horizontal(|ui| {
                        let r = ui.button("Apply staged choice");
                        #[cfg(test)] services.prompt_buttons.insert("apply-type13", r.rect);
                        apply = r.clicked();
                        let r = ui.button("Keep editing");
                        #[cfg(test)] services.prompt_buttons.insert("cancel", r.rect);
                        cancel = r.clicked();
                        let r = ui.button("Discard staged choice");
                        #[cfg(test)] services.prompt_buttons.insert("discard", r.rect);
                        discard = r.clicked();
                    });
                });
            let key = (staged.file_index, staged.slot, staged.control);
            if cancel {
                services.close = CloseResult::Cancelled;
            } else if apply {
                self.apply_hd_type13_choice(staged);
                services.finish_close_if(!self.needs_close());
            } else if discard {
                self.hd_type13_pending.remove(&key);
                self.hd_message = "Discarded the staged type-0x0D choice.".into();
                services.finish_close_if(!self.needs_close());
            }
            return;
        }
        if let Some(staged) = self.hd_type19_pending.values().next().copied() {
            let mut apply = false;
            let mut discard = false;
            let mut cancel = false;
            egui::Window::new("Staged HD type-0x13 choice")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(format!("{} for H5EP entry {} / FX slot {} is staged but not applied.", staged.choice.label(), staged.file_index, staged.slot.index()));
                    ui.label("Static-derived neutral choice; Apply changes only this offline H5EP value word.");
                    ui.horizontal(|ui| {
                        let r = ui.button("Apply staged choice");
                        #[cfg(test)] services.prompt_buttons.insert("apply-type19", r.rect);
                        apply = r.clicked();
                        let r = ui.button("Keep editing");
                        #[cfg(test)] services.prompt_buttons.insert("cancel", r.rect);
                        cancel = r.clicked();
                        let r = ui.button("Discard staged choice");
                        #[cfg(test)] services.prompt_buttons.insert("discard", r.rect);
                        discard = r.clicked();
                    });
                });
            let key = (staged.file_index, staged.slot, staged.control);
            if cancel {
                services.close = CloseResult::Cancelled;
            } else if apply {
                self.apply_hd_type19_choice(staged);
                services.finish_close_if(!self.needs_close());
            } else if discard {
                self.hd_type19_pending.remove(&key);
                self.hd_message = "Discarded the staged type-0x13 choice.".into();
                services.finish_close_if(!self.needs_close());
            }
            return;
        }
        let Some(index) = self.hd_library.first_dirty() else {
            services.finish_close_if(!self.needs_close());
            return;
        };
        let kind = self
            .hd_library
            .file(index)
            .map(HdEditFile::kind)
            .unwrap_or("HD editor file");
        let remaining = self.hd_library.dirty_count();
        let mut discard = false;
        let mut save_continue = false;
        let mut cancel = false;
        egui::Window::new("Unsaved HD Edit file")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(format!(
                    "The selected {kind} has an offline edit that is not saved. {remaining} HD file(s) are unsaved."
                ));
                ui.label("Save writes a new copy only; no device operation is available.");
                ui.horizontal(|ui| {
                    let response = ui.button("Save copy and continue…");
                    #[cfg(test)]
                    services.prompt_buttons.insert("save", response.rect);
                    save_continue = response.clicked();
                    let response = ui.button("Keep editing");
                    #[cfg(test)]
                    services.prompt_buttons.insert("cancel", response.rect);
                    cancel = response.clicked();
                    let response = ui.button("Discard this edit");
                    #[cfg(test)]
                    services.prompt_buttons.insert("discard", response.rect);
                    discard = response.clicked();
                });
            });
        if cancel {
            services.close = CloseResult::Cancelled;
        } else if save_continue {
            if self.save_next_dirty_hd(services) {
                services.finish_close_if(!self.needs_close());
            }
        } else if discard && self.discard_next_dirty_hd() {
            services.finish_close_if(!self.needs_close());
        }
    }

    fn hd_button(&mut self, ui: &mut egui::Ui, id: &'static str, label: &str) -> bool {
        let response = ui.button(label);
        #[cfg(test)]
        self.hd_buttons.insert(id, response.rect);
        #[cfg(not(test))]
        let _ = id;
        response.clicked()
    }
    fn hd_editor_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("POD HD Edit files · offline");
        ui.label(pod_preset::HD_EDITOR_OFFLINE_NOTICE);
        ui.label("Fixed H5EP/H5ES/H5EB framing and selected source-traced type-1 and numeric controls are supported across nine effect models. Auxiliary widgets and all other rows remain opaque; file association does not prove physical target compatibility.");
        ui.label("No device, audio, firmware or driver operation is available here. Copy saves never replace an existing file.");

        let selected_octave_fuzz = self.hd_selected.and_then(|index| {
            self.hd_library.file(index).and_then(|file| match file {
                HdEditFile::H5ep(preset) => preset.effect_model_id(self.hd_effect_slot),
                _ => None,
            })
        }) == Some(
            pod_preset::HdType1Control::OctaveFuzz(pod_preset::OctaveFuzzControl::Drive).model_id(),
        );
        if selected_octave_fuzz {
            let _reverb_defaults = egui::CollapsingHeader::new(
                "Additional source-derived reverb defaults",
            )
            .id_salt("hd-octave-fuzz-reverb-defaults")
            .default_open(true)
            .show(ui, |ui| {
                egui::ComboBox::from_id_salt("hd-octave-fuzz-reverb-default-target")
                    .selected_text(self.hd_reverb_default_target.label())
                    .show_ui(ui, |ui| {
                        for target in pod_preset::HdReverbDefault::ALL {
                            ui.selectable_value(
                                &mut self.hd_reverb_default_target,
                                target,
                                target.label(),
                            );
                        }
                    });
                if self.hd_button(
                    ui,
                    "replace-octave-fuzz-admitted-reverb-default",
                    "Apply selected reverb defaults",
                ) {
                    let index = self.hd_selected.expect("selected HD file");
                    match pod_preset::HdFxSlot::new(self.hd_effect_slot) {
                        Ok(slot) => match Session::initial_rig()
                            .replace_hd_octave_fuzz_with_admitted_reverb_defaults(
                                &mut self.hd_library,
                                index,
                                slot,
                                self.hd_reverb_default_target,
                            )
                        {
                            Ok(()) => {
                                self.hd_message = format!(
                                    "Replaced this offline effect with {} defaults. Nothing was sent; save a new copy to keep the change.",
                                    self.hd_reverb_default_target.label()
                                );
                            }
                            Err(error) => self.hd_message = hd_rejection_text(&error),
                        },
                        Err(error) => self.hd_message = error.detail(),
                    }
                }
            });
            ui.label("Reset five values to Vintage Pre defaults.");
            if self.hd_button(
                ui,
                "replace-octave-fuzz-vintage-pre",
                "Replace Octave Fuzz with Vintage Pre defaults",
            ) {
                let index = self.hd_selected.expect("selected HD file");
                match pod_preset::HdFxSlot::new(self.hd_effect_slot) {
                    Ok(slot) => match Session::initial_rig()
                        .replace_hd_octave_fuzz_with_vintage_pre_defaults(
                            &mut self.hd_library,
                            index,
                            slot,
                        ) {
                        Ok(()) => {
                            self.hd_message = "Replaced this offline effect with Vintage Pre defaults. Nothing was sent; save a new copy to keep the change.".into();
                        }
                        Err(error) => self.hd_message = hd_rejection_text(&error),
                    },
                    Err(error) => self.hd_message = error.detail(),
                }
            }
            ui.label("Reset five values to Screamer defaults.");
            if self.hd_button(
                ui,
                "replace-octave-fuzz-screamer",
                "Replace Octave Fuzz with Screamer defaults",
            ) {
                let index = self.hd_selected.expect("selected HD file");
                match pod_preset::HdFxSlot::new(self.hd_effect_slot) {
                    Ok(slot) => match Session::initial_rig()
                        .replace_hd_octave_fuzz_with_screamer_defaults(
                            &mut self.hd_library,
                            index,
                            slot,
                        ) {
                        Ok(()) => {
                            self.hd_message = "Replaced this offline effect with Screamer defaults. Nothing was sent; save a new copy to keep the change.".into();
                        }
                        Err(error) => self.hd_message = hd_rejection_text(&error),
                    },
                    Err(error) => self.hd_message = error.detail(),
                }
            }
            ui.label("Reset five values to Sub Octave Fuzz defaults.");
            if self.hd_button(
                ui,
                "replace-octave-fuzz-sub-octave-fuzz",
                "Replace Octave Fuzz with Sub Octave Fuzz defaults",
            ) {
                let index = self.hd_selected.expect("selected HD file");
                match pod_preset::HdFxSlot::new(self.hd_effect_slot) {
                    Ok(slot) => match Session::initial_rig()
                        .replace_hd_octave_fuzz_with_sub_octave_fuzz_defaults(
                            &mut self.hd_library,
                            index,
                            slot,
                        ) {
                        Ok(()) => {
                            self.hd_message = "Replaced this offline effect with Sub Octave Fuzz defaults. Nothing was sent; save a new copy to keep the change.".into();
                        }
                        Err(error) => self.hd_message = hd_rejection_text(&error),
                    },
                    Err(error) => self.hd_message = error.detail(),
                }
            }
            ui.label("Replace with Noise Gate defaults.");
            if self.hd_button(ui, "replace-octave-fuzz-noise-gate", "Noise Gate defaults") {
                let index = self.hd_selected.expect("selected HD file");
                match pod_preset::HdFxSlot::new(self.hd_effect_slot) {
                    Ok(slot) => match Session::initial_rig()
                        .replace_hd_octave_fuzz_with_noise_gate_defaults(
                            &mut self.hd_library,
                            index,
                            slot,
                        ) {
                        Ok(()) => {
                            self.hd_message = "Replaced this offline effect with Noise Gate defaults. Nothing was sent; save a new copy to keep the change.".into();
                        }
                        Err(error) => self.hd_message = hd_rejection_text(&error),
                    },
                    Err(error) => self.hd_message = error.detail(),
                }
            }
            ui.label("Additional source-derived distortion/fuzz defaults");
            egui::ComboBox::from_id_salt("hd-octave-fuzz-default-target")
                .selected_text(self.hd_default_target.label())
                .show_ui(ui, |ui| {
                    for target in pod_preset::HdDistortionDefault::ALL {
                        ui.selectable_value(&mut self.hd_default_target, target, target.label());
                    }
                });
            if self.hd_button(
                ui,
                "replace-octave-fuzz-admitted-default",
                "Apply selected source defaults",
            ) {
                let index = self.hd_selected.expect("selected HD file");
                match pod_preset::HdFxSlot::new(self.hd_effect_slot) {
                    Ok(slot) => match Session::initial_rig()
                        .replace_hd_octave_fuzz_with_admitted_defaults(
                            &mut self.hd_library,
                            index,
                            slot,
                            self.hd_default_target,
                        ) {
                        Ok(()) => {
                            self.hd_message = format!(
                                "Replaced this offline effect with {} defaults. Nothing was sent; save a new copy to keep the change.",
                                self.hd_default_target.label()
                            );
                        }
                        Err(error) => self.hd_message = hd_rejection_text(&error),
                    },
                    Err(error) => self.hd_message = error.detail(),
                }
            }
            ui.label("Additional source-derived dynamics / FX defaults");
            egui::ComboBox::from_id_salt("hd-octave-fuzz-dynamics-default-target")
                .selected_text(self.hd_dynamics_default_target.label())
                .show_ui(ui, |ui| {
                    for target in pod_preset::HdDynamicsDefault::ALL {
                        ui.selectable_value(
                            &mut self.hd_dynamics_default_target,
                            target,
                            target.label(),
                        );
                    }
                });
            if self.hd_button(
                ui,
                "replace-octave-fuzz-admitted-dynamics-default",
                "Apply selected dynamics / FX defaults",
            ) {
                let index = self.hd_selected.expect("selected HD file");
                match pod_preset::HdFxSlot::new(self.hd_effect_slot) {
                    Ok(slot) => match Session::initial_rig()
                        .replace_hd_octave_fuzz_with_admitted_dynamics_defaults(
                            &mut self.hd_library,
                            index,
                            slot,
                            self.hd_dynamics_default_target,
                        ) {
                        Ok(()) => {
                            self.hd_message = format!(
                                "Replaced this offline effect with {} defaults. Nothing was sent; save a new copy to keep the change.",
                                self.hd_dynamics_default_target.label()
                            );
                        }
                        Err(error) => self.hd_message = hd_rejection_text(&error),
                    },
                    Err(error) => self.hd_message = error.detail(),
                }
            }
            ui.separator();
        }

        let rows: Vec<_> = (0..self.hd_library.len())
            .filter_map(|index| {
                let file = self.hd_library.file(index)?;
                Some((
                    index,
                    format!(
                        "{index} · {} · {} bytes{}",
                        file.kind(),
                        file.as_bytes().len(),
                        if self.hd_library.is_dirty(index) {
                            " · unsaved"
                        } else {
                            ""
                        },
                    ),
                ))
            })
            .collect();
        for (index, label) in rows {
            if ui
                .selectable_label(self.hd_selected == Some(index), label)
                .clicked()
            {
                self.select_hd_file(index);
            }
        }

        let Some(index) = self.hd_selected else {
            return;
        };
        let Some(file) = self.hd_library.file(index).cloned() else {
            self.hd_selected = None;
            return;
        };
        ui.label("The editor-group/file-format association is static evidence; internal context words are not treated as a product ID.");
        match &file {
            HdEditFile::H5ep(preset) => {
                ui.label("Single preset. Source header and unmapped body bytes are preserved.");
                let slot_changed = ui
                    .add(egui::Slider::new(&mut self.hd_effect_slot, 0..=7).text("Effect slot"))
                    .changed();
                if slot_changed {
                    if self.hd_type9_pending.take().is_some() {
                        self.hd_message =
                            "Cleared the staged type-9 choice after changing FX slot.".into();
                    }
                    if self.hd_type8_pending.take().is_some() {
                        self.hd_message =
                            "Cleared the staged type-8 choice after changing FX slot.".into();
                    }
                    if self.hd_type10_pending.take().is_some() {
                        self.hd_message =
                            "Cleared the staged type-10 choice after changing FX slot.".into();
                    }
                    if self.hd_selected.is_some_and(|index| {
                        let before = self.hd_type11_pending.len();
                        self.hd_type11_pending
                            .retain(|(file_index, _, _), _| *file_index != index);
                        before != self.hd_type11_pending.len()
                    }) {
                        self.hd_message =
                            "Cleared staged type-11 choices after changing FX slot.".into();
                    }
                    if self.hd_selected.is_some_and(|index| {
                        let before = self.hd_type12_pending.len();
                        self.hd_type12_pending
                            .retain(|(file_index, _, _), _| *file_index != index);
                        before != self.hd_type12_pending.len()
                    }) {
                        self.hd_message =
                            "Cleared staged type-12 choice after changing FX slot.".into();
                    }
                    if self.hd_selected.is_some_and(|index| {
                        let before = self.hd_type13_pending.len();
                        self.hd_type13_pending
                            .retain(|(file_index, _, _), _| *file_index != index);
                        before != self.hd_type13_pending.len()
                    }) {
                        self.hd_message =
                            "Cleared the staged type-0x0D choice after changing FX slot.".into();
                    }
                    if self.hd_selected.is_some_and(|index| {
                        let before = self.hd_type19_pending.len();
                        self.hd_type19_pending
                            .retain(|(file_index, _, _), _| *file_index != index);
                        before != self.hd_type19_pending.len()
                    }) {
                        self.hd_message =
                            "Cleared the staged type-0x13 choice after changing FX slot.".into();
                    }
                    let selected_model_id = preset.effect_model_id(self.hd_effect_slot);
                    if let Some(model_id) = selected_model_id
                        && let Some(control) = pod_preset::HdType1Control::ALL
                            .into_iter()
                            .find(|control| control.model_id() == model_id)
                    {
                        self.hd_control = control;
                    }
                    self.hd_control_percent = preset
                        .hd_type1_control_percent(self.hd_effect_slot, self.hd_control)
                        .unwrap_or(0);
                    if let Some(model_id) = selected_model_id
                        && let Some(control) = pod_preset::HD_TYPE1_MAPPED_CONTROLS
                            .into_iter()
                            .find(|control| {
                                control.model_id() == model_id
                                    && preset
                                        .hd_type1_mapped_control_percent(
                                            self.hd_effect_slot,
                                            *control,
                                        )
                                        .is_some()
                            })
                    {
                        self.hd_mapped_control = control;
                        self.hd_control_percent = preset
                            .hd_type1_mapped_control_percent(self.hd_effect_slot, control)
                            .unwrap_or(self.hd_control_percent);
                    }
                    if let Some(control) =
                        pod_preset::HD_NUMERIC_CONTROLS.into_iter().find(|control| {
                            control.model_id()
                                == preset
                                    .effect_model_id(self.hd_effect_slot)
                                    .unwrap_or_default()
                                && preset
                                    .hd_numeric_control_value(self.hd_effect_slot, *control)
                                    .is_some()
                        })
                    {
                        self.hd_numeric_control = control;
                        self.hd_numeric_value = preset
                            .hd_numeric_control_value(self.hd_effect_slot, control)
                            .unwrap_or(control.min());
                    }
                }
                let model_id = preset
                    .effect_model_id(self.hd_effect_slot)
                    .unwrap_or_default();
                if self.hd_type8_pending.is_some_and(|pending| {
                    pending.file_index == index
                        && (pending.slot.index() != self.hd_effect_slot
                            || pending.model_id != model_id)
                }) {
                    self.hd_type8_pending = None;
                    self.hd_message =
                        "Cleared a stale staged type-8 choice after the selected model changed."
                            .into();
                }
                if self.hd_type9_pending.is_some_and(|pending| {
                    pending.file_index == index
                        && (pending.slot.index() != self.hd_effect_slot
                            || pending.model_id != model_id)
                }) {
                    self.hd_type9_pending = None;
                    self.hd_message =
                        "Cleared a stale staged type-9 choice after the selected model changed."
                            .into();
                }
                if self.hd_type10_pending.is_some_and(|pending| {
                    pending.file_index == index
                        && (pending.slot.index() != self.hd_effect_slot
                            || pending.model_id != model_id)
                }) {
                    self.hd_type10_pending = None;
                    self.hd_message =
                        "Cleared a stale staged type-10 choice after the selected model changed."
                            .into();
                }
                let current_slot = pod_preset::HdFxSlot::new(self.hd_effect_slot)
                    .expect("the visible FX slot slider is bounded to 0..=7");
                self.hd_type11_pending
                    .retain(|(_, slot, control), pending| {
                        !(pending.file_index == index
                            && (*slot != current_slot
                                || control.model_id() != model_id
                                || pending.model_id != model_id))
                    });
                self.hd_type12_pending
                    .retain(|(_, slot, control), pending| {
                        !(pending.file_index == index
                            && (*slot != current_slot
                                || control.model_id() != model_id
                                || pending.model_id != model_id))
                    });
                self.hd_type13_pending
                    .retain(|(_, slot, control), pending| {
                        !(pending.file_index == index
                            && (*slot != current_slot
                                || control.model_id() != model_id
                                || pending.model_id != model_id))
                    });
                self.hd_type19_pending
                    .retain(|(_, slot, control), pending| {
                        !(pending.file_index == index
                            && (*slot != current_slot
                                || control.model_id() != model_id
                                || pending.model_id != model_id))
                    });
                if pod_preset::hd_type8_choice_key(model_id).is_some() {
                    let slot = pod_preset::HdFxSlot::new(self.hd_effect_slot)
                        .expect("the visible FX slot slider is bounded to 0..=7");
                    let value = preset.hd_type8_choice_value(slot);
                    let staged = self.hd_type8_pending.filter(|pending| {
                        pending.file_index == index
                            && pending.slot == slot
                            && pending.model_id == model_id
                    });
                    let mut selected = staged.map(|pending| pending.choice).or(match value {
                        pod_preset::HdType8ChoiceValue::Choice(choice) => Some(choice),
                        _ => None,
                    });
                    let selected_text = staged.map_or_else(
                        || match value {
                            pod_preset::HdType8ChoiceValue::Choice(choice) => {
                                choice.label().to_owned()
                            }
                            pod_preset::HdType8ChoiceValue::UnknownFinite(bits) => {
                                format!("Unknown imported value 0x{bits:08x}")
                            }
                            pod_preset::HdType8ChoiceValue::NonFinite(bits) => {
                                format!("Non-finite imported value 0x{bits:08x}")
                            }
                            pod_preset::HdType8ChoiceValue::Unavailable => {
                                "Unavailable (row or guard not admitted)".to_owned()
                            }
                        },
                        |pending| format!("{} · staged", pending.choice.label()),
                    );
                    ui.label("Recovered type-8 two-choice control");
                    let _response = egui::ComboBox::from_id_salt((
                        "hd-type8-choice",
                        index,
                        self.hd_effect_slot,
                    ))
                    .selected_text(selected_text)
                    .show_ui(ui, |ui| {
                        ui.add_enabled_ui(
                            matches!(
                                value,
                                pod_preset::HdType8ChoiceValue::Choice(_)
                                    | pod_preset::HdType8ChoiceValue::UnknownFinite(_)
                            ),
                            |ui| {
                                for choice in pod_preset::HdType8Choice::ALL {
                                    let response = ui.selectable_value(
                                        &mut selected,
                                        Some(choice),
                                        choice.label(),
                                    );
                                    #[cfg(test)]
                                    self.hd_buttons.insert(
                                        match choice {
                                            pod_preset::HdType8Choice::Choice0 => {
                                                "hd-type8-choice-option-0"
                                            }
                                            pod_preset::HdType8Choice::Choice1 => {
                                                "hd-type8-choice-option-1"
                                            }
                                        },
                                        response.rect,
                                    );
                                    if response.changed() {
                                        self.stage_hd_type8_choice(index, slot, model_id, choice);
                                    }
                                }
                            },
                        );
                    });
                    #[cfg(test)]
                    self.hd_buttons
                        .insert("hd-type8-choice-combo", _response.response.rect);
                    ui.label("Choice labels are neutral. Imported unknown values are preserved until you choose an explicit value.");
                    if matches!(value, pod_preset::HdType8ChoiceValue::NonFinite(_)) {
                        ui.label("A non-finite imported value cannot be replaced by this editor operation.");
                    }
                }
                if pod_preset::hd_type9_choice_key(model_id).is_some() {
                    let slot = pod_preset::HdFxSlot::new(self.hd_effect_slot)
                        .expect("the visible FX slot slider is bounded to 0..=7");
                    let value = preset.hd_type9_choice_value(slot);
                    let staged = self.hd_type9_pending.filter(|pending| {
                        pending.file_index == index
                            && pending.slot == slot
                            && pending.model_id == model_id
                    });
                    let current_choice = match value {
                        pod_preset::HdType9ChoiceValue::Choice(choice) => Some(choice),
                        _ => None,
                    };
                    let mut selected = staged.map(|pending| pending.choice).or(current_choice);
                    let selected_text = staged.map_or_else(
                        || match value {
                            pod_preset::HdType9ChoiceValue::Choice(choice) => {
                                choice.label().to_owned()
                            }
                            pod_preset::HdType9ChoiceValue::UnknownFinite(bits) => {
                                format!("Unknown imported value 0x{bits:08x}")
                            }
                            pod_preset::HdType9ChoiceValue::NonFinite(bits) => {
                                format!("Non-finite imported value 0x{bits:08x}")
                            }
                            pod_preset::HdType9ChoiceValue::Unavailable => {
                                "Unavailable (row or guard not admitted)".to_owned()
                            }
                        },
                        |pending| format!("{} · staged", pending.choice.label()),
                    );
                    ui.label("Recovered type-9 choice");
                    let _response = egui::ComboBox::from_id_salt((
                        "hd-type9-choice",
                        index,
                        self.hd_effect_slot,
                    ))
                    .selected_text(selected_text)
                    .show_ui(ui, |ui| {
                        ui.add_enabled_ui(
                            matches!(
                                value,
                                pod_preset::HdType9ChoiceValue::Choice(_)
                                    | pod_preset::HdType9ChoiceValue::UnknownFinite(_)
                            ),
                            |ui| {
                                for choice in pod_preset::HdType9Choice::ALL {
                                    let response = ui.selectable_value(
                                        &mut selected,
                                        Some(choice),
                                        choice.label(),
                                    );
                                    #[cfg(test)]
                                    self.hd_buttons.insert(
                                        match choice {
                                            pod_preset::HdType9Choice::Choice0 => {
                                                "hd-type9-choice-option-0"
                                            }
                                            pod_preset::HdType9Choice::Choice1 => {
                                                "hd-type9-choice-option-1"
                                            }
                                            pod_preset::HdType9Choice::Choice2 => {
                                                "hd-type9-choice-option-2"
                                            }
                                        },
                                        response.rect,
                                    );
                                    if response.changed() {
                                        self.stage_hd_type9_choice(index, slot, model_id, choice);
                                    }
                                }
                            },
                        );
                    });
                    #[cfg(test)]
                    self.hd_buttons
                        .insert("hd-type9-choice-combo", _response.response.rect);
                    ui.label("Choice labels are neutral. Imported unknown values are preserved until you choose an explicit value.");
                    if matches!(value, pod_preset::HdType9ChoiceValue::NonFinite(_)) {
                        ui.label("A non-finite imported value cannot be replaced by this editor operation.");
                    }
                }
                if pod_preset::hd_type10_choice_key(model_id).is_some() {
                    let slot = pod_preset::HdFxSlot::new(self.hd_effect_slot).unwrap();
                    let value = preset.hd_type10_choice_value(slot);
                    let staged = self.hd_type10_pending.filter(|p| {
                        p.file_index == index && p.slot == slot && p.model_id == model_id
                    });
                    let choice = match value {
                        pod_preset::HdType10ChoiceValue::Choice(c) => Some(c),
                        _ => None,
                    };
                    let mut selected = staged.map(|p| p.choice).or(choice);
                    let label = staged.map_or_else(
                        || match value {
                            pod_preset::HdType10ChoiceValue::Choice(c) => c.label().to_owned(),
                            pod_preset::HdType10ChoiceValue::UnknownFinite(bits) => {
                                format!("Unknown imported value 0x{bits:08x}")
                            }
                            pod_preset::HdType10ChoiceValue::NonFinite(bits) => {
                                format!("Non-finite imported value 0x{bits:08x}")
                            }
                            pod_preset::HdType10ChoiceValue::Unavailable => {
                                "Unavailable (row or guard not admitted)".to_owned()
                            }
                        },
                        |p| format!("{} · staged", p.choice.label()),
                    );
                    ui.label("Recovered type-10 choice");
                    let _response = egui::ComboBox::from_id_salt((
                        "hd-type10-choice",
                        index,
                        self.hd_effect_slot,
                    ))
                    .selected_text(label)
                    .show_ui(ui, |ui| {
                        ui.add_enabled_ui(
                            matches!(
                                value,
                                pod_preset::HdType10ChoiceValue::Choice(_)
                                    | pod_preset::HdType10ChoiceValue::UnknownFinite(_)
                            ),
                            |ui| {
                                for choice in pod_preset::HdType10Choice::ALL {
                                    let r = ui.selectable_value(
                                        &mut selected,
                                        Some(choice),
                                        choice.label(),
                                    );
                                    #[cfg(test)]
                                    self.hd_buttons.insert(
                                        if choice.index() == 0 {
                                            "hd-type10-choice-option-0"
                                        } else {
                                            "hd-type10-choice-option-1"
                                        },
                                        r.rect,
                                    );
                                    if r.changed() {
                                        self.stage_hd_type10_choice(index, slot, model_id, choice);
                                    }
                                }
                            },
                        );
                    });
                    #[cfg(test)]
                    self.hd_buttons
                        .insert("hd-type10-choice-combo", _response.response.rect);
                    ui.label("Choice labels are neutral; the selected value is preserved as an opaque float word.");
                    if matches!(value, pod_preset::HdType10ChoiceValue::NonFinite(_)) {
                        ui.label("A non-finite imported value cannot be replaced by this editor operation.");
                    }
                }
                for control in pod_preset::HdType11Control::for_model(model_id) {
                    let control = *control;
                    let slot = pod_preset::HdFxSlot::new(self.hd_effect_slot)
                        .expect("the visible FX slot slider is bounded to 0..=7");
                    let value = preset.hd_type11_choice_value(slot, control);
                    let pending_key = (index, slot, control);
                    let staged = self.hd_type11_pending.get(&pending_key).copied();
                    let mut selected = staged.map(|pending| pending.choice).or(match value {
                        pod_preset::HdType11ChoiceValue::Choice(choice) => Some(choice),
                        _ => None,
                    });
                    let selected_text = staged.map_or_else(
                        || match value {
                            pod_preset::HdType11ChoiceValue::Choice(choice) => {
                                choice.label().to_owned()
                            }
                            pod_preset::HdType11ChoiceValue::UnknownFinite(bits) => {
                                format!("Unknown imported value 0x{bits:08x}")
                            }
                            pod_preset::HdType11ChoiceValue::NonFinite(bits) => {
                                format!("Non-finite imported value 0x{bits:08x}")
                            }
                            pod_preset::HdType11ChoiceValue::Unavailable => {
                                "Unavailable (row or guard not admitted)".to_owned()
                            }
                        },
                        |pending| format!("{} · staged", pending.choice.label()),
                    );
                    ui.label(format!("Recovered type-11 menu · {}", control.label()));
                    let _combo = egui::ComboBox::from_id_salt((
                        "hd-type11-choice",
                        index,
                        self.hd_effect_slot,
                        control.key(),
                    ))
                    .selected_text(selected_text)
                    .show_ui(ui, |ui| {
                        ui.add_enabled_ui(
                            matches!(
                                value,
                                pod_preset::HdType11ChoiceValue::Choice(_)
                                    | pod_preset::HdType11ChoiceValue::UnknownFinite(_)
                            ),
                            |ui| {
                                for choice in pod_preset::HdType11Choice::ALL {
                                    let response = ui.selectable_value(
                                        &mut selected,
                                        Some(choice),
                                        choice.label(),
                                    );
                                    #[cfg(test)]
                                    self.hd_buttons.insert(
                                        hd_type11_option_button(control, choice),
                                        response.rect,
                                    );
                                    if response.changed() {
                                        self.stage_hd_type11_choice(
                                            index, slot, model_id, control, choice,
                                        );
                                    }
                                }
                            },
                        );
                    });
                    #[cfg(test)]
                    self.hd_buttons
                        .insert(hd_type11_combo_button(control), _combo.response.rect);
                    ui.label(
                        "Choice labels are neutral. Each admitted key is staged independently.",
                    );
                    if matches!(value, pod_preset::HdType11ChoiceValue::NonFinite(_)) {
                        ui.label("A non-finite imported value cannot be replaced by this editor operation.");
                    }
                    if let Some(pending) = staged {
                        ui.horizontal(|ui| {
                            if self.hd_button(
                                ui,
                                hd_type11_apply_button(control),
                                "Apply offline choice",
                            ) {
                                self.apply_hd_type11_choice(pending);
                            }
                            if self.hd_button(
                                ui,
                                hd_type11_discard_button(control),
                                "Discard staged choice",
                            ) {
                                self.hd_type11_pending.remove(&pending_key);
                                self.hd_message =
                                    format!("Discarded the staged {} choice.", control.label());
                            }
                        });
                    }
                }
                for control in pod_preset::HdType12Control::for_model(model_id) {
                    let slot =
                        pod_preset::HdFxSlot::new(self.hd_effect_slot).expect("bounded FX slot");
                    let value = preset.hd_type12_choice_value(slot, *control);
                    let pending_key = (index, slot, *control);
                    let staged = self.hd_type12_pending.get(&pending_key).copied();
                    let mut selected = staged.map(|pending| pending.choice).or(match value {
                        pod_preset::HdType12ChoiceValue::Choice(choice) => Some(choice),
                        _ => None,
                    });
                    let selected_text = staged.map_or_else(
                        || match value {
                            pod_preset::HdType12ChoiceValue::Choice(choice) => {
                                choice.label().to_owned()
                            }
                            pod_preset::HdType12ChoiceValue::UnknownFinite(bits) => {
                                format!("Unknown imported value 0x{bits:08x}")
                            }
                            pod_preset::HdType12ChoiceValue::NonFinite(bits) => {
                                format!("Non-finite imported value 0x{bits:08x}")
                            }
                            pod_preset::HdType12ChoiceValue::Unavailable => {
                                "Unavailable (row or guard not admitted)".to_owned()
                            }
                        },
                        |pending| format!("{} · staged", pending.choice.label()),
                    );
                    ui.label("Recovered type-12 menu · neutral Choice0/Choice1");
                    let _combo = egui::ComboBox::from_id_salt((
                        "hd-type12-choice",
                        index,
                        self.hd_effect_slot,
                        control.key(),
                    ))
                    .selected_text(selected_text)
                    .show_ui(ui, |ui| {
                        ui.add_enabled_ui(
                            matches!(
                                value,
                                pod_preset::HdType12ChoiceValue::Choice(_)
                                    | pod_preset::HdType12ChoiceValue::UnknownFinite(_)
                            ),
                            |ui| {
                                for choice in pod_preset::HdType12Choice::ALL {
                                    let r = ui.selectable_value(
                                        &mut selected,
                                        Some(choice),
                                        choice.label(),
                                    );
                                    #[cfg(test)]
                                    self.hd_buttons
                                        .insert(hd_type12_option_button(choice), r.rect);
                                    if r.changed() {
                                        self.stage_hd_type12_choice(
                                            index, slot, model_id, *control, choice,
                                        );
                                    }
                                }
                            },
                        );
                    });
                    #[cfg(test)]
                    self.hd_buttons
                        .insert(hd_type12_combo_button(), _combo.response.rect);
                    if matches!(value, pod_preset::HdType12ChoiceValue::NonFinite(_)) {
                        ui.label("A non-finite imported value cannot be replaced by this editor operation.");
                    }
                    if let Some(pending) = staged {
                        ui.horizontal(|ui| {
                            if self.hd_button(ui, hd_type12_apply_button(), "Apply offline choice")
                            {
                                self.apply_hd_type12_choice(pending);
                            }
                            if self.hd_button(
                                ui,
                                hd_type12_discard_button(),
                                "Discard staged choice",
                            ) {
                                self.hd_type12_pending.remove(&pending_key);
                                self.hd_message = "Discarded the staged type-12 choice.".into();
                            }
                        });
                    }
                }
                for control in pod_preset::HdType13Control::for_model(model_id) {
                    let slot =
                        pod_preset::HdFxSlot::new(self.hd_effect_slot).expect("bounded FX slot");
                    let value = preset.hd_type13_choice_value(slot, *control);
                    let pending_key = (index, slot, *control);
                    let staged = self.hd_type13_pending.get(&pending_key).copied();
                    let mut selected = staged.map(|pending| pending.choice).or(match value {
                        pod_preset::HdType13ChoiceValue::Choice(choice) => Some(choice),
                        _ => None,
                    });
                    let selected_text = staged.map_or_else(
                        || match value {
                            pod_preset::HdType13ChoiceValue::Choice(choice) => {
                                choice.label().to_owned()
                            }
                            pod_preset::HdType13ChoiceValue::UnknownFinite(bits) => {
                                format!("Unknown imported value 0x{bits:08x}")
                            }
                            pod_preset::HdType13ChoiceValue::NonFinite(bits) => {
                                format!("Non-finite imported value 0x{bits:08x}")
                            }
                            pod_preset::HdType13ChoiceValue::Unavailable => {
                                "Unavailable (row or guard not admitted)".to_owned()
                            }
                        },
                        |pending| format!("{} · staged", pending.choice.label()),
                    );
                    ui.label("Static-derived type-0x0D menu · neutral Choice 0–3");
                    let _combo = egui::ComboBox::from_id_salt((
                        "hd-type13-choice",
                        index,
                        self.hd_effect_slot,
                        control.key(),
                    ))
                    .selected_text(selected_text)
                    .show_ui(ui, |ui| {
                        ui.add_enabled_ui(
                            matches!(
                                value,
                                pod_preset::HdType13ChoiceValue::Choice(_)
                                    | pod_preset::HdType13ChoiceValue::UnknownFinite(_)
                            ),
                            |ui| {
                                for choice in pod_preset::HdType13Choice::ALL {
                                    let response = ui.selectable_value(
                                        &mut selected,
                                        Some(choice),
                                        choice.label(),
                                    );
                                    #[cfg(test)]
                                    self.hd_buttons
                                        .insert(hd_type13_option_button(choice), response.rect);
                                    if response.changed() {
                                        self.stage_hd_type13_choice(
                                            index, slot, model_id, *control, choice,
                                        );
                                    }
                                }
                            },
                        );
                    });
                    #[cfg(test)]
                    self.hd_buttons
                        .insert("hd-type13-choice-combo", _combo.response.rect);
                    ui.label("Values are the static-derived binary32 words; names, units, and physical mapping remain unknown.");
                    if matches!(value, pod_preset::HdType13ChoiceValue::NonFinite(_)) {
                        ui.label("A non-finite imported value cannot be replaced by this editor operation.");
                    }
                    if let Some(pending) = staged {
                        ui.horizontal(|ui| {
                            if self.hd_button(ui, hd_type13_apply_button(), "Apply offline choice")
                            {
                                self.apply_hd_type13_choice(pending);
                            }
                            if self.hd_button(
                                ui,
                                hd_type13_discard_button(),
                                "Discard staged choice",
                            ) {
                                self.hd_type13_pending.remove(&pending_key);
                                self.hd_message = "Discarded the staged type-0x0D choice.".into();
                            }
                        });
                    }
                }
                for control in pod_preset::HdType19Control::for_model(model_id) {
                    let slot =
                        pod_preset::HdFxSlot::new(self.hd_effect_slot).expect("bounded FX slot");
                    let value = preset.hd_type19_choice_value(slot, *control);
                    let pending_key = (index, slot, *control);
                    let staged = self.hd_type19_pending.get(&pending_key).copied();
                    let mut selected = staged.map(|pending| pending.choice).or(match value {
                        pod_preset::HdType19ChoiceValue::Choice(choice) => Some(choice),
                        _ => None,
                    });
                    let selected_text = staged.map_or_else(
                        || match value {
                            pod_preset::HdType19ChoiceValue::Choice(choice) => {
                                choice.label().to_owned()
                            }
                            pod_preset::HdType19ChoiceValue::UnknownFinite(bits) => {
                                format!("Unknown imported value 0x{bits:08x}")
                            }
                            pod_preset::HdType19ChoiceValue::NonFinite(bits) => {
                                format!("Non-finite imported value 0x{bits:08x}")
                            }
                            pod_preset::HdType19ChoiceValue::Unavailable => {
                                "Unavailable (row or guard not admitted)".to_owned()
                            }
                        },
                        |pending| format!("{} · staged", pending.choice.label()),
                    );
                    ui.label("Static-derived type-0x13 menu · neutral Choice 0–7");
                    let _combo = egui::ComboBox::from_id_salt((
                        "hd-type19-choice",
                        index,
                        self.hd_effect_slot,
                        control.key(),
                    ))
                    .height(400.0)
                    .selected_text(selected_text)
                    .show_ui(ui, |ui| {
                        ui.add_enabled_ui(
                            matches!(
                                value,
                                pod_preset::HdType19ChoiceValue::Choice(_)
                                    | pod_preset::HdType19ChoiceValue::UnknownFinite(_)
                            ),
                            |ui| {
                                for choice in pod_preset::HdType19Choice::ALL {
                                    let response = ui.selectable_value(
                                        &mut selected,
                                        Some(choice),
                                        choice.label(),
                                    );
                                    #[cfg(test)]
                                    self.hd_buttons
                                        .insert(hd_type19_option_button(choice), response.rect);
                                    if response.changed() {
                                        self.stage_hd_type19_choice(
                                            index, slot, model_id, *control, choice,
                                        );
                                    }
                                }
                            },
                        );
                    });
                    #[cfg(test)]
                    self.hd_buttons
                        .insert("hd-type19-choice-combo", _combo.response.rect);
                    ui.label("Eight static-derived binary32 candidates; labels, units, and physical mapping remain unknown.");
                    if matches!(value, pod_preset::HdType19ChoiceValue::NonFinite(_)) {
                        ui.label("A non-finite imported value cannot be replaced by this editor operation.");
                    }
                    if let Some(pending) = staged {
                        ui.horizontal(|ui| {
                            if self.hd_button(ui, hd_type19_apply_button(), "Apply offline choice")
                            {
                                self.apply_hd_type19_choice(pending);
                            }
                            if self.hd_button(
                                ui,
                                hd_type19_discard_button(),
                                "Discard staged choice",
                            ) {
                                self.hd_type19_pending.remove(&pending_key);
                                self.hd_message = "Discarded the staged type-0x13 choice.".into();
                            }
                        });
                    }
                }
                let has_mapped_type1 = pod_preset::HD_TYPE1_MAPPED_CONTROLS.iter().any(|control| {
                    control.model_id() == model_id
                        && preset
                            .hd_type1_mapped_control_percent(self.hd_effect_slot, *control)
                            .is_some()
                });
                if has_mapped_type1 {
                    egui::ComboBox::from_label("Recovered effect control")
                        .selected_text(format!(
                            "{} {}",
                            self.hd_mapped_control.model_label(),
                            self.hd_mapped_control.label()
                        ))
                        .show_ui(ui, |ui| {
                            for control in pod_preset::HD_TYPE1_MAPPED_CONTROLS {
                                if control.model_id() != model_id
                                    || preset
                                        .hd_type1_mapped_control_percent(
                                            self.hd_effect_slot,
                                            control,
                                        )
                                        .is_none()
                                {
                                    continue;
                                }
                                if ui
                                    .selectable_value(
                                        &mut self.hd_mapped_control,
                                        control,
                                        format!("{} {}", control.model_label(), control.label()),
                                    )
                                    .changed()
                                {
                                    self.hd_control_percent = preset
                                        .hd_type1_mapped_control_percent(
                                            self.hd_effect_slot,
                                            control,
                                        )
                                        .unwrap_or(0);
                                }
                            }
                        });
                    ui.add(
                        egui::Slider::new(&mut self.hd_control_percent, 0..=100).text(format!(
                            "{} {}",
                            self.hd_mapped_control.model_label(),
                            self.hd_mapped_control.label()
                        )),
                    );
                    if self.hd_button(ui, "apply-hd-mapped-type1-control", "Apply offline control")
                    {
                        self.apply_hd_mapped_control(index);
                    }
                } else {
                    egui::ComboBox::from_label("Recovered control")
                        .selected_text(format!(
                            "{} {}",
                            self.hd_control.model_label(),
                            self.hd_control.label()
                        ))
                        .show_ui(ui, |ui| {
                            for control in pod_preset::HdType1Control::ALL {
                                if control.model_id()
                                    != preset
                                        .effect_model_id(self.hd_effect_slot)
                                        .unwrap_or_default()
                                {
                                    continue;
                                }
                                if ui
                                    .selectable_value(
                                        &mut self.hd_control,
                                        control,
                                        format!("{} {}", control.model_label(), control.label()),
                                    )
                                    .changed()
                                {
                                    self.hd_control_percent = preset
                                        .hd_type1_control_percent(self.hd_effect_slot, control)
                                        .unwrap_or(0);
                                }
                            }
                        });
                    if preset
                        .hd_type1_control_percent(self.hd_effect_slot, self.hd_control)
                        .is_some()
                    {
                        ui.add(
                            egui::Slider::new(&mut self.hd_control_percent, 0..=100).text(format!(
                                "{} {}",
                                self.hd_control.model_label(),
                                self.hd_control.label()
                            )),
                        );
                        if self.hd_button(ui, "apply-hd-drive", "Apply offline control") {
                            self.apply_hd_control(index);
                        }
                    } else {
                        ui.label(
                            "No matching type-1 control is available; other rows remain unchanged.",
                        );
                    }
                }
                if pod_preset::HD_NUMERIC_CONTROLS.iter().any(|control| {
                    control.model_id()
                        == preset
                            .effect_model_id(self.hd_effect_slot)
                            .unwrap_or_default()
                        && preset
                            .hd_numeric_control_value(self.hd_effect_slot, *control)
                            .is_some()
                }) {
                    egui::ComboBox::from_label("Recovered numeric control")
                        .selected_text(format!(
                            "{} {}",
                            self.hd_numeric_control.model_label(),
                            self.hd_numeric_control.label()
                        ))
                        .show_ui(ui, |ui| {
                            for control in pod_preset::HD_NUMERIC_CONTROLS {
                                if control.model_id()
                                    != preset
                                        .effect_model_id(self.hd_effect_slot)
                                        .unwrap_or_default()
                                    || preset
                                        .hd_numeric_control_value(self.hd_effect_slot, control)
                                        .is_none()
                                {
                                    continue;
                                }
                                if ui
                                    .selectable_value(
                                        &mut self.hd_numeric_control,
                                        control,
                                        format!("{} {}", control.model_label(), control.label()),
                                    )
                                    .changed()
                                {
                                    self.hd_numeric_value = preset
                                        .hd_numeric_control_value(self.hd_effect_slot, control)
                                        .unwrap_or(control.min());
                                }
                            }
                        });
                    ui.label("Editor integer control values; source labels and bounds do not declare physical units.");
                    ui.add(
                        egui::Slider::new(
                            &mut self.hd_numeric_value,
                            self.hd_numeric_control.min()..=self.hd_numeric_control.max(),
                        )
                        .text(format!(
                            "{} {}",
                            self.hd_numeric_control.model_label(),
                            self.hd_numeric_control.label()
                        )),
                    );
                    if self.hd_button(
                        ui,
                        "apply-hd-numeric-control",
                        "Apply offline numeric control",
                    ) {
                        self.apply_hd_numeric_control(index);
                    }
                }
            }
            HdEditFile::H5es(setlist) => {
                ui.label(format!("One setlist with {0} fixed preset entries. Extracted children become separate library items; the parent remains unchanged.", pod_preset::H5ES_PRESET_COUNT));
                ui.add(egui::Slider::new(&mut self.hd_preset_index, 0..=63).text("Preset index"));
                if self.hd_button(ui, "extract-hd-child", "Open child as separate preset") {
                    match Session::initial_rig().extract_hd_editor_preset(
                        &mut self.hd_library,
                        index,
                        0,
                        self.hd_preset_index,
                    ) {
                        Ok(child) => self.select_hd_file(child.index),
                        Err(error) => self.hd_message = hd_rejection_text(&error),
                    }
                }
                let _ = setlist;
            }
            HdEditFile::H5eb(bundle) => {
                ui.label(format!("Bundle with {0} fixed setlists × {1} preset entries. Extracted children become separate library items; the parent remains unchanged.", pod_preset::H5EB_SETLIST_COUNT, pod_preset::H5ES_PRESET_COUNT));
                ui.add(egui::Slider::new(&mut self.hd_setlist_index, 0..=7).text("Setlist index"));
                ui.add(egui::Slider::new(&mut self.hd_preset_index, 0..=63).text("Preset index"));
                if self.hd_button(ui, "extract-hd-child", "Open child as separate preset") {
                    match Session::initial_rig().extract_hd_editor_preset(
                        &mut self.hd_library,
                        index,
                        self.hd_setlist_index,
                        self.hd_preset_index,
                    ) {
                        Ok(child) => self.select_hd_file(child.index),
                        Err(error) => self.hd_message = hd_rejection_text(&error),
                    }
                }
                let _ = bundle;
            }
        }
        ui.label(&self.hd_message);
    }

    fn select_hd_file(&mut self, index: usize) {
        if let Some(previous) = self.hd_selected {
            self.hd_type11_pending
                .retain(|(file_index, _, _), _| *file_index != previous);
            self.hd_type12_pending
                .retain(|(file_index, _, _), _| *file_index != previous);
            self.hd_type13_pending
                .retain(|(file_index, _, _), _| *file_index != previous);
            self.hd_type19_pending
                .retain(|(file_index, _, _), _| *file_index != previous);
        }
        if self.hd_type9_pending.take().is_some() {
            self.hd_message =
                "Cleared a staged type-9 choice after changing HD file selection.".into();
        }
        if self.hd_type8_pending.take().is_some() {
            self.hd_message =
                "Cleared a staged type-8 choice after changing HD file selection.".into();
        }
        if self.hd_type10_pending.take().is_some() {
            self.hd_message =
                "Cleared a staged type-10 choice after changing HD file selection.".into();
        }
        self.hd_selected = Some(index);
        self.hd_effect_slot = 0;
        self.hd_control = self
            .hd_library
            .file(index)
            .and_then(|file| match file {
                HdEditFile::H5ep(preset) => {
                    let model = preset.effect_model_id(0)?;
                    pod_preset::HdType1Control::ALL
                        .into_iter()
                        .find(|control| control.model_id() == model)
                }
                _ => None,
            })
            .unwrap_or(pod_preset::HdType1Control::Screamer(
                pod_preset::ScreamerControl::Drive,
            ));
        self.hd_control_percent = self
            .hd_library
            .file(index)
            .and_then(|file| match file {
                HdEditFile::H5ep(preset) => preset.hd_type1_control_percent(0, self.hd_control),
                _ => None,
            })
            .unwrap_or(0);
        self.hd_mapped_control = self
            .hd_library
            .file(index)
            .and_then(|file| match file {
                HdEditFile::H5ep(preset) => {
                    let model = preset.effect_model_id(0)?;
                    pod_preset::HD_TYPE1_MAPPED_CONTROLS
                        .into_iter()
                        .find(|control| {
                            control.model_id() == model
                                && preset
                                    .hd_type1_mapped_control_percent(0, *control)
                                    .is_some()
                        })
                }
                _ => None,
            })
            .unwrap_or(pod_preset::HD_TYPE1_MAPPED_CONTROLS[0]);
        self.hd_control_percent = self
            .hd_library
            .file(index)
            .and_then(|file| match file {
                HdEditFile::H5ep(preset) => {
                    preset.hd_type1_mapped_control_percent(0, self.hd_mapped_control)
                }
                _ => None,
            })
            .unwrap_or(self.hd_control_percent);
        self.hd_numeric_control = self
            .hd_library
            .file(index)
            .and_then(|file| match file {
                HdEditFile::H5ep(preset) => {
                    let model = preset.effect_model_id(0)?;
                    pod_preset::HD_NUMERIC_CONTROLS.into_iter().find(|control| {
                        control.model_id() == model
                            && preset.hd_numeric_control_value(0, *control).is_some()
                    })
                }
                _ => None,
            })
            .unwrap_or(pod_preset::HD_NUMERIC_CONTROLS[0]);
        self.hd_numeric_value = self
            .hd_library
            .file(index)
            .and_then(|file| match file {
                HdEditFile::H5ep(preset) => {
                    preset.hd_numeric_control_value(0, self.hd_numeric_control)
                }
                _ => None,
            })
            .unwrap_or(self.hd_numeric_control.min());
    }

    fn apply_hd_control(&mut self, index: usize) {
        let result = Session::initial_rig().set_hd_type1_control_percent(
            &mut self.hd_library,
            index,
            self.hd_effect_slot,
            self.hd_control,
            self.hd_control_percent,
        );
        match result {
            Ok(()) => {
                self.hd_message =
                    "Updated only the traced in-memory value. Nothing was sent.".into();
            }
            Err(error) => self.hd_message = hd_rejection_text(&error),
        }
    }

    fn apply_hd_mapped_control(&mut self, index: usize) {
        let result = Session::initial_rig().set_hd_type1_mapped_control_percent(
            &mut self.hd_library,
            index,
            self.hd_effect_slot,
            self.hd_mapped_control,
            self.hd_control_percent,
        );
        match result {
            Ok(()) => {
                self.hd_message =
                    "Updated only the traced in-memory value. Nothing was sent.".into();
            }
            Err(error) => self.hd_message = hd_rejection_text(&error),
        }
    }

    fn apply_hd_numeric_control(&mut self, index: usize) {
        let result = Session::initial_rig().set_hd_numeric_control_value(
            &mut self.hd_library,
            index,
            self.hd_effect_slot,
            self.hd_numeric_control,
            self.hd_numeric_value,
        );
        match result {
            Ok(()) => {
                self.hd_message =
                    "Updated only the traced in-memory value. Nothing was sent.".into();
            }
            Err(error) => self.hd_message = hd_rejection_text(&error),
        }
    }

    fn apply_hd_type9_choice(
        &mut self,
        file_index: usize,
        slot: pod_preset::HdFxSlot,
        model_id: u32,
        choice: pod_preset::HdType9Choice,
    ) {
        let current_model_id = self
            .hd_library
            .file(file_index)
            .and_then(|file| match file {
                HdEditFile::H5ep(preset) => preset.effect_model_id(slot.index()),
                _ => None,
            });
        if current_model_id != Some(model_id) {
            if self
                .hd_type9_pending
                .is_some_and(|pending| pending.file_index == file_index && pending.slot == slot)
            {
                self.hd_type9_pending = None;
            }
            self.hd_message =
                "Cleared a staged type-9 choice after its model context changed.".into();
            return;
        }
        let result = Session::extension(ProductProfile::PodHd500x).and_then(|session| {
            session.set_hd_type9_choice(&mut self.hd_library, file_index, slot, choice)
        });
        match result {
            Ok(()) => {
                if self.hd_type9_pending.is_some_and(|pending| {
                    pending.file_index == file_index
                        && pending.slot == slot
                        && pending.choice == choice
                }) {
                    self.hd_type9_pending = None;
                }
                self.hd_message = "Applied the type-9 choice to this offline H5EP entry. Nothing was sent; save a new copy to keep the change.".into();
            }
            Err(error) => self.hd_message = hd_rejection_text(&error),
        }
    }

    fn apply_hd_type8_choice(
        &mut self,
        file_index: usize,
        slot: pod_preset::HdFxSlot,
        model_id: u32,
        choice: pod_preset::HdType8Choice,
    ) {
        let current_model_id = self
            .hd_library
            .file(file_index)
            .and_then(|file| match file {
                HdEditFile::H5ep(preset) => preset.effect_model_id(slot.index()),
                _ => None,
            });
        if current_model_id != Some(model_id) || pod_preset::hd_type8_choice_key(model_id).is_none()
        {
            if self
                .hd_type8_pending
                .is_some_and(|pending| pending.file_index == file_index && pending.slot == slot)
            {
                self.hd_type8_pending = None;
            }
            self.hd_message =
                "Cleared a staged type-8 choice after its model context changed.".into();
            return;
        }
        let result = Session::extension(ProductProfile::PodHd500x).and_then(|session| {
            session.set_hd_type8_choice(&mut self.hd_library, file_index, slot, choice)
        });
        match result {
            Ok(()) => {
                if self.hd_type8_pending.is_some_and(|pending| {
                    pending.file_index == file_index
                        && pending.slot == slot
                        && pending.model_id == model_id
                        && pending.choice == choice
                }) {
                    self.hd_type8_pending = None;
                }
                self.hd_message = "Applied the type-8 two-choice value to this offline H5EP entry. Nothing was sent; save a new copy to keep the change.".into();
            }
            Err(error) => self.hd_message = hd_rejection_text(&error),
        }
    }

    fn apply_hd_type10_choice(
        &mut self,
        file_index: usize,
        slot: pod_preset::HdFxSlot,
        model_id: u32,
        choice: pod_preset::HdType10Choice,
    ) {
        let current = self.hd_library.file(file_index).and_then(|f| match f {
            HdEditFile::H5ep(p) => p.effect_model_id(slot.index()),
            _ => None,
        });
        if current != Some(model_id) || pod_preset::hd_type10_choice_key(model_id).is_none() {
            if self
                .hd_type10_pending
                .is_some_and(|p| p.file_index == file_index && p.slot == slot)
            {
                self.hd_type10_pending = None;
            }
            self.hd_message =
                "Cleared a staged type-10 choice after its model context changed.".into();
            return;
        }
        let result = Session::extension(ProductProfile::PodHd500x)
            .and_then(|s| s.set_hd_type10_choice(&mut self.hd_library, file_index, slot, choice));
        match result {
            Ok(()) => {
                if self.hd_type10_pending.is_some_and(|p| {
                    p.file_index == file_index
                        && p.slot == slot
                        && p.model_id == model_id
                        && p.choice == choice
                }) {
                    self.hd_type10_pending = None;
                }
                self.hd_message = "Applied the type-10 choice to this offline H5EP entry. Nothing was sent; save a new copy to keep the change.".into();
            }
            Err(error) => self.hd_message = hd_rejection_text(&error),
        }
    }

    fn apply_hd_type11_choice(&mut self, pending: PendingHdType11Choice) {
        let key = (pending.file_index, pending.slot, pending.control);
        let current_model = self
            .hd_library
            .file(pending.file_index)
            .and_then(|file| match file {
                HdEditFile::H5ep(preset) => preset.effect_model_id(pending.slot.index()),
                _ => None,
            });
        if current_model != Some(pending.model_id)
            || pending.model_id != pending.control.model_id()
            || !pod_preset::HD_TYPE11_CHOICE_MODEL_KEYS
                .contains(&(pending.control.model_id(), pending.control.key()))
        {
            self.hd_type11_pending.remove(&key);
            self.hd_message = format!(
                "Cleared the staged {} choice after its model context changed.",
                pending.control.label()
            );
            return;
        }
        let result = Session::extension(ProductProfile::PodHd500x).and_then(|session| {
            session.set_hd_type11_choice(
                &mut self.hd_library,
                pending.file_index,
                pending.slot,
                pending.control,
                pending.choice,
            )
        });
        match result {
            Ok(()) => {
                if self.hd_type11_pending.get(&key) == Some(&pending) {
                    self.hd_type11_pending.remove(&key);
                }
                self.hd_message = format!(
                    "Applied the {} menu value to this offline H5EP entry. Nothing was sent; save a new copy to keep the change.",
                    pending.control.label()
                );
            }
            Err(error) => self.hd_message = hd_rejection_text(&error),
        }
    }

    fn apply_hd_type12_choice(&mut self, pending: PendingHdType12Choice) {
        let key = (pending.file_index, pending.slot, pending.control);
        let current_model = self
            .hd_library
            .file(pending.file_index)
            .and_then(|file| match file {
                HdEditFile::H5ep(preset) => preset.effect_model_id(pending.slot.index()),
                _ => None,
            });
        if current_model != Some(pending.model_id) || pending.model_id != pending.control.model_id()
        {
            self.hd_type12_pending.remove(&key);
            self.hd_message =
                "Cleared the staged type-12 choice after its model context changed.".into();
            return;
        }
        let result = Session::extension(ProductProfile::PodHd500x).and_then(|session| {
            session.set_hd_type12_choice(
                &mut self.hd_library,
                pending.file_index,
                pending.slot,
                pending.control,
                pending.choice,
            )
        });
        match result {
            Ok(()) => {
                if self.hd_type12_pending.get(&key) == Some(&pending) {
                    self.hd_type12_pending.remove(&key);
                }
                self.hd_message = "Applied the type-12 menu value to this offline H5EP entry. Nothing was sent; save a new copy to keep the change.".into();
            }
            Err(error) => self.hd_message = hd_rejection_text(&error),
        }
    }

    fn apply_hd_type13_choice(&mut self, pending: PendingHdType13Choice) {
        let key = (pending.file_index, pending.slot, pending.control);
        let current_model = self
            .hd_library
            .file(pending.file_index)
            .and_then(|file| match file {
                HdEditFile::H5ep(preset) => preset.effect_model_id(pending.slot.index()),
                _ => None,
            });
        if current_model != Some(pending.model_id) || pending.model_id != pending.control.model_id()
        {
            self.hd_type13_pending.remove(&key);
            self.hd_message =
                "Cleared the staged type-0x0D choice after its model context changed.".into();
            return;
        }
        let result = Session::extension(ProductProfile::PodHd500x).and_then(|session| {
            session.set_hd_type13_choice(
                &mut self.hd_library,
                pending.file_index,
                pending.slot,
                pending.control,
                pending.choice,
            )
        });
        match result {
            Ok(()) => {
                if self.hd_type13_pending.get(&key) == Some(&pending) {
                    self.hd_type13_pending.remove(&key);
                }
                self.hd_message = "Applied the static-derived type-0x0D choice to this offline H5EP entry. Nothing was sent; save a new copy to keep the change.".into();
            }
            Err(error) => self.hd_message = hd_rejection_text(&error),
        }
    }

    fn stage_hd_type13_choice(
        &mut self,
        file_index: usize,
        slot: pod_preset::HdFxSlot,
        model_id: u32,
        control: pod_preset::HdType13Control,
        choice: pod_preset::HdType13Choice,
    ) {
        if model_id != control.model_id() {
            return;
        }
        self.hd_type13_pending.insert(
            (file_index, slot, control),
            PendingHdType13Choice {
                file_index,
                slot,
                model_id,
                control,
                choice,
            },
        );
    }

    fn apply_hd_type19_choice(&mut self, pending: PendingHdType19Choice) {
        let key = (pending.file_index, pending.slot, pending.control);
        let current_model = self
            .hd_library
            .file(pending.file_index)
            .and_then(|file| match file {
                HdEditFile::H5ep(preset) => preset.effect_model_id(pending.slot.index()),
                _ => None,
            });
        if current_model != Some(pending.model_id) || pending.model_id != pending.control.model_id()
        {
            self.hd_type19_pending.remove(&key);
            self.hd_message =
                "Cleared the staged type-0x13 choice after its model context changed.".into();
            return;
        }
        let result = Session::extension(ProductProfile::PodHd500x).and_then(|session| {
            session.set_hd_type19_choice(
                &mut self.hd_library,
                pending.file_index,
                pending.slot,
                pending.control,
                pending.choice,
            )
        });
        match result {
            Ok(()) => {
                if self.hd_type19_pending.get(&key) == Some(&pending) {
                    self.hd_type19_pending.remove(&key);
                }
                self.hd_message = "Applied the static-derived type-0x13 choice to this offline H5EP entry. Nothing was sent; save a new copy to keep the change.".into();
            }
            Err(error) => self.hd_message = hd_rejection_text(&error),
        }
    }

    fn stage_hd_type19_choice(
        &mut self,
        file_index: usize,
        slot: pod_preset::HdFxSlot,
        model_id: u32,
        control: pod_preset::HdType19Control,
        choice: pod_preset::HdType19Choice,
    ) {
        if model_id != control.model_id() {
            return;
        }
        self.hd_type19_pending.insert(
            (file_index, slot, control),
            PendingHdType19Choice {
                file_index,
                slot,
                model_id,
                control,
                choice,
            },
        );
    }

    fn stage_hd_type12_choice(
        &mut self,
        file_index: usize,
        slot: pod_preset::HdFxSlot,
        model_id: u32,
        control: pod_preset::HdType12Control,
        choice: pod_preset::HdType12Choice,
    ) {
        if model_id != control.model_id() {
            return;
        }
        self.hd_type12_pending.insert(
            (file_index, slot, control),
            PendingHdType12Choice {
                file_index,
                slot,
                model_id,
                control,
                choice,
            },
        );
    }

    fn stage_hd_type11_choice(
        &mut self,
        file_index: usize,
        slot: pod_preset::HdFxSlot,
        model_id: u32,
        control: pod_preset::HdType11Control,
        choice: pod_preset::HdType11Choice,
    ) {
        if model_id != control.model_id() {
            return;
        }
        self.hd_type11_pending.insert(
            (file_index, slot, control),
            PendingHdType11Choice {
                file_index,
                slot,
                model_id,
                control,
                choice,
            },
        );
    }

    fn stage_hd_type10_choice(
        &mut self,
        file_index: usize,
        slot: pod_preset::HdFxSlot,
        model_id: u32,
        choice: pod_preset::HdType10Choice,
    ) {
        self.hd_type10_pending = Some(PendingHdType10Choice {
            file_index,
            slot,
            model_id,
            choice,
        });
    }

    fn stage_hd_type9_choice(
        &mut self,
        file_index: usize,
        slot: pod_preset::HdFxSlot,
        model_id: u32,
        choice: pod_preset::HdType9Choice,
    ) {
        self.hd_type8_pending = None;
        self.hd_type9_pending = Some(PendingHdType9Choice {
            file_index,
            slot,
            model_id,
            choice,
        });
    }

    fn stage_hd_type8_choice(
        &mut self,
        file_index: usize,
        slot: pod_preset::HdFxSlot,
        model_id: u32,
        choice: pod_preset::HdType8Choice,
    ) {
        self.hd_type9_pending = None;
        self.hd_type8_pending = Some(PendingHdType8Choice {
            file_index,
            slot,
            model_id,
            choice,
        });
    }

    pub(super) fn load_hd_editor_file(&mut self, services: &mut EditorServices<'_>) {
        let Some(path) = services.dialogs.open_pod_tone() else {
            self.hd_message = "Load cancelled. No file was read.".into();
            return;
        };
        let bytes = pod_preset::read_regular_hd_editor_file(pod_preset::path_facts(&path), || {
            pod_preset::open_regular_file(&path)
        });
        if let Err(error) = bytes.and_then(|bytes| {
            self.import_hd_bytes(bytes)
                .map_err(pod_preset::ToneError::format)
        }) {
            self.hd_message = error.detail();
        }
    }

    fn import_hd_bytes(&mut self, bytes: Vec<u8>) -> Result<(), String> {
        let summary = Session::initial_rig()
            .import_hd_editor_file(&mut self.hd_library, bytes)
            .map_err(|error| hd_rejection_text(&error))?;
        self.select_hd_file(summary.index);
        self.hd_message = "Loaded an offline HD Edit file. Internal product target and physical compatibility remain unverified.".into();
        Ok(())
    }

    pub(super) fn write_hd_copy(
        &mut self,
        services: &mut EditorServices<'_>,
        index: usize,
    ) -> bool {
        let Some(file) = self.hd_library.file(index) else {
            self.hd_message = "Select an HD Edit file before saving a copy.".into();
            return false;
        };
        let suggested = match file {
            HdEditFile::H5es(_) => "hd-edit-setlist.5xs",
            HdEditFile::H5eb(_) => "hd-edit-bundle.5xb",
            _ => "hd-edit-preset.5xe",
        };
        let Some(path) = services.dialogs.save_pod_tone(suggested) else {
            self.hd_message = "Save cancelled. No file was written.".into();
            return false;
        };
        match Session::initial_rig().save_hd_editor_copy(&self.hd_library, &path, index) {
            Ok(()) => match self.hd_library.note_saved(index) {
                Ok(()) => {
                    self.hd_message = format!(
                        "Saved a new offline copy to {}. No device operation occurred.",
                        path.display()
                    );
                    true
                }
                Err(error) => {
                    self.hd_message = error.detail();
                    false
                }
            },
            Err(error) => {
                self.hd_message = hd_rejection_text(&error);
                false
            }
        }
    }
}

fn hd_rejection_text(rejection: &Rejection) -> String {
    match rejection {
        Rejection::Format { detail } => detail.clone(),
        other => format!("{other:?}"),
    }
}

impl HdEditor {
    fn needs_close(&self) -> bool {
        self.hd_library.any_dirty()
            || self.hd_type9_pending.is_some()
            || self.hd_type8_pending.is_some()
            || self.hd_type10_pending.is_some()
            || !self.hd_type11_pending.is_empty()
            || !self.hd_type12_pending.is_empty()
            || !self.hd_type13_pending.is_empty()
            || !self.hd_type19_pending.is_empty()
    }
}

impl FamilyTab for HdEditor {
    fn show(&mut self, ui: &mut egui::Ui, services: &mut EditorServices<'_>) {
        #[cfg(test)]
        self.hd_buttons.clear();
        self.show_hd_panel(services, ui);
    }
    fn dirty(&self) -> bool {
        self.needs_close()
    }
    fn close_prompt(&mut self, ctx: &egui::Context, services: &mut EditorServices<'_>) {
        self.hd_quit_dialog(services, ctx);
    }
    fn status(&self) -> (&str, Option<&str>) {
        (&self.hd_message, None)
    }
    fn save_copy(&mut self, services: &mut EditorServices<'_>) {
        if let Some(index) = self.hd_selected {
            self.write_hd_copy(services, index);
        }
    }
}

pub(super) struct Family;

pub(super) const FAMILY: Family = Family;

impl ProductFamily for Family {
    fn info(&self) -> FamilyInfo {
        FamilyInfo {
            id: "hd500x",
            title: "POD HD500X",
            new_label: "POD HD500X",
            extensions: &["5xe", "5xs", "5xb"],
            ceiling: pod_preset::HD_BUNDLE_BYTES_CEILING as u64,
        }
    }

    fn claims(&self, bytes: &[u8]) -> bool {
        matches!(bytes.get(..4), Some(b"H5EP" | b"H5ES" | b"H5EB"))
    }

    fn open(
        &self,
        _defaults: &EditorDefaults,
        source: TabSource,
    ) -> Result<Box<dyn FamilyTab>, String> {
        let mut editor = HdEditor::new();
        if let TabSource::File(_, bytes) = source {
            editor.import_hd_bytes(bytes)?;
        }
        Ok(Box::new(editor))
    }
}
