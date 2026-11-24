use super::*;
use std::collections::{BTreeMap, BTreeSet};
use variax_workbench::pod_preset::{
    PodControl, PodMetadata, PodModelChange, PodTone, UnknownPodDefaults, pod_parameter_table,
};

#[derive(Default)]
pub(super) struct ParameterPanel {
    packs: u8,
    keep_amp_settings: bool,
    undo: Vec<Revision>,
    redo: Vec<Revision>,
    unknown: BTreeMap<usize, BTreeSet<u32>>,
    metadata: Option<MetadataDraft>,
    slider: Option<(usize, u32, f64)>,
}

struct Revision {
    index: usize,
    bytes: Vec<u8>,
    unknown: BTreeSet<u32>,
}

struct MetadataDraft {
    index: usize,
    original: BTreeMap<PodMetadata, String>,
    values: BTreeMap<PodMetadata, String>,
}

impl ParameterPanel {
    pub(super) fn clear_history(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.unknown.clear();
        self.metadata = None;
        self.slider = None;
    }

    pub(super) fn clear_tone_history(&mut self, index: usize) {
        self.undo.retain(|item| item.index != index);
        self.redo.retain(|item| item.index != index);
        self.unknown.remove(&index);
    }

    fn push_undo(&mut self, revision: Revision) {
        self.undo.push(revision);
        if self.undo.len() > 32 {
            self.undo.remove(0);
        }
    }
}

pub(super) const GROUPS: [(&str, &str); 16] = [
    ("amp", "Amp"),
    ("drive", "Drive"),
    ("cab", "Cab and room"),
    ("mic", "Microphone"),
    ("gate", "Noise gate"),
    ("compressor", "Compressor"),
    ("eq", "EQ"),
    ("stomp", "Stomp"),
    ("mod", "Modulation"),
    ("delay", "Delay"),
    ("reverb", "Reverb"),
    ("wah", "Wah"),
    ("volume pedal", "Volume pedal"),
    ("tempo", "Tempo"),
    ("system", "Pedal, Tweak and footswitch"),
    ("Variax link", "Linked Variax"),
];

impl PodxtEditor {
    pub(super) fn table_edit(
        &mut self,
        index: usize,
        edit: impl FnOnce(&mut PodTone) -> Result<PodModelChange, pod_preset::ToneError>,
    ) -> Result<(), pod_preset::ToneError> {
        let tone = self
            .pod_library
            .tone(index)
            .ok_or_else(|| pod_preset::ToneError::format("Select a PODxt tone"))?;
        let mut candidate = tone.clone();
        let change = edit(&mut candidate)?;
        let prior_unknown = self
            .parameter_panel
            .unknown
            .get(&index)
            .cloned()
            .unwrap_or_default();
        let old_controls = tone.table_controls(self.parameter_panel.packs);
        let new_controls = candidate.table_controls(self.parameter_panel.packs);
        let bits_for = |controls: &[PodControl<'_>], selector| {
            controls
                .iter()
                .find(|control| control.parameter.selector == Some(selector))
                .and_then(|control| control.bits)
        };
        let mut unknown = prior_unknown.clone();
        unknown.retain(|selector| {
            !change.affected.contains(selector)
                && bits_for(&old_controls, *selector) == bits_for(&new_controls, *selector)
        });
        unknown.extend(change.default_unknown);
        unknown.extend(
            change
                .preserved
                .into_iter()
                .filter_map(|(before, after)| prior_unknown.contains(&before).then_some(after)),
        );
        if candidate.as_bytes() == tone.as_bytes() && unknown == prior_unknown {
            return Ok(());
        }
        let revision = Revision {
            index,
            bytes: tone.as_bytes().to_vec(),
            unknown: prior_unknown,
        };
        self.replace_pod_tone(index, candidate.into_bytes())?;
        self.parameter_panel.unknown.insert(index, unknown);
        self.parameter_panel.push_undo(revision);
        self.parameter_panel.redo.clear();
        self.pod_message = "Tone edited offline.".into();
        Ok(())
    }

    pub(super) fn table_history(&mut self, redo: bool) {
        let panel = &mut self.parameter_panel;
        let stack = if redo {
            &mut panel.redo
        } else {
            &mut panel.undo
        };
        let Some(revision) = stack.pop() else {
            return;
        };
        let index = revision.index;
        let Some(current) = self.pod_library.tone(index) else {
            return;
        };
        let inverse = Revision {
            index,
            bytes: current.as_bytes().to_vec(),
            unknown: panel.unknown.get(&index).cloned().unwrap_or_default(),
        };
        match self.replace_pod_tone(index, revision.bytes.clone()) {
            Ok(()) => {
                self.parameter_panel.unknown.insert(index, revision.unknown);
                if redo {
                    self.parameter_panel.push_undo(inverse);
                } else {
                    self.parameter_panel.redo.push(inverse);
                }
                self.select_pod_index(index);
                self.refresh_selected_pod_name();
                self.pod_message = if redo {
                    "Tone edit redone."
                } else {
                    "Tone edit undone."
                }
                .into();
            }
            Err(error) => {
                let panel = &mut self.parameter_panel;
                if redo {
                    &mut panel.redo
                } else {
                    &mut panel.undo
                }
                .push(revision);
                self.pod_message = error.detail();
            }
        }
    }

    pub(super) fn parameter_panel(&mut self, ui: &mut egui::Ui) {
        let Some(index) = self.pod_selected else {
            return;
        };
        let Some(tone) = self.pod_library.tone(index) else {
            return;
        };
        let can_reset_eq = tone.podxt_eq_reset_context().is_ok();
        let controls = tone.table_controls(self.parameter_panel.packs);
        let variax_family = controls
            .iter()
            .find(|row| row.parameter.key == "VARIAX_FAMILY")
            .and_then(|row| row.bits);
        ui.horizontal_wrapped(|ui| {
            if self.pod_button_enabled(
                ui,
                "pod-undo",
                "Undo",
                !self.parameter_panel.undo.is_empty(),
            ) {
                self.table_history(false);
            }
            if self.pod_button_enabled(
                ui,
                "pod-redo",
                "Redo",
                !self.parameter_panel.redo.is_empty(),
            ) {
                self.table_history(true);
            }
            let mut amp_defaults = !self.parameter_panel.keep_amp_settings;
            if ui.checkbox(&mut amp_defaults, "Load amp defaults")
                .on_hover_text("Until default values are available, shared amp controls keep their settings and new controls use neutral values.")
                .changed()
            {
                self.parameter_panel.keep_amp_settings = !amp_defaults;
            }
            ui.menu_button("Model packs", |ui| {
                for (bit, label) in [
                    (1, "Metal Shop"),
                    (2, "Collector Classics"),
                    (4, "FX Junkie"),
                    (8, "Bass Expansion"),
                ] {
                    let mut selected = self.parameter_panel.packs & bit != 0;
                    if ui.checkbox(&mut selected, label).changed() {
                        self.parameter_panel.packs ^= bit;
                    }
                }
            })
            .response
            .on_hover_text(
                "Select the packs installed on your PODxt to make those models available offline.",
            );
        });

        // Navigation above can select a different tone during Undo or Redo.
        if self.pod_selected != Some(index) {
            return;
        }
        let table = pod_parameter_table();
        let mut change = None;
        let active = match self.pod_block {
            PodBlock::Amp => "amp",
            PodBlock::Stomp => "stomp",
            PodBlock::Mod => "mod",
            PodBlock::Delay => "delay",
        };
        let mut reset_eq = false;
        for (group, heading) in GROUPS {
            if matches!(group, "amp" | "drive" | "stomp" | "mod" | "delay")
                && group != active
                && !(group == "drive" && active == "amp")
            {
                continue;
            }
            let mut render = |ui: &mut egui::Ui| {
                for control in controls.iter().filter(|control| {
                    control.parameter.block == group
                        && control
                            .parameter
                            .variax_family
                            .is_none_or(|family| Some(family) == variax_family)
                }) {
                    let selector = control.parameter.selector.expect("editable selector");
                    let key = control.parameter.key.as_str();
                    let unknown = self
                        .parameter_panel
                        .unknown
                        .get(&index)
                        .is_some_and(|markers| markers.contains(&selector));
                    ui.push_id((index, selector), |ui| {
                        if let Some(bits) = self.parameter_widget(ui, index, control, unknown) {
                            change = Some((key, bits, control.parameter.catalog.is_some()));
                        }
                    });
                }
                if group == "eq" && can_reset_eq {
                    reset_eq = ui.button("Reset EQ").clicked();
                }
            };
            if group == active || group == "drive" {
                render(ui);
            } else {
                ui.collapsing(heading, render);
            }
        }
        if reset_eq
            && let Err(error) = self.table_edit(index, |tone| {
                tone.reset_podxt_eq()?;
                Ok(PodModelChange::default())
            })
        {
            self.pod_message = error.detail();
        }
        if let Some((key, bits, is_model)) = change {
            let packs = self.parameter_panel.packs;
            let amp_defaults = !self.parameter_panel.keep_amp_settings;
            let result = self.table_edit(index, |candidate| {
                if is_model {
                    candidate.set_table_model(key, bits, packs, amp_defaults, &UnknownPodDefaults)
                } else {
                    candidate.set_table_parameter(key, bits)?;
                    Ok(PodModelChange {
                        affected: table
                            .parameter(key)
                            .and_then(|row| row.selector)
                            .into_iter()
                            .collect(),
                        ..Default::default()
                    })
                }
            });
            if let Err(error) = result {
                self.pod_message = error.detail();
            }
        }
        self.metadata_panel(ui, index);
    }

    fn parameter_widget(
        &mut self,
        ui: &mut egui::Ui,
        index: usize,
        control: &PodControl<'_>,
        unknown: bool,
    ) -> Option<u32> {
        let mut changed = None;
        ui.horizontal(|ui| {
            ui.label(control.label());
            if !control.choices.is_empty() {
                let current = control
                    .bits
                    .and_then(|bits| {
                        control
                            .choices
                            .iter()
                            .find(|(raw, _)| *raw == bits)
                            .map(|(_, label)| label.as_ref())
                            .or_else(|| {
                                control.parameter.catalog.as_ref()?;
                                pod_parameter_table().model_name(bits)
                            })
                    })
                    .unwrap_or(if control.bits.is_some() {
                        "Unknown selection"
                    } else {
                        "Select…"
                    });
                let mut next = control.bits;
                egui::ComboBox::from_id_salt("choice")
                    .selected_text(current)
                    .show_ui(ui, |ui| {
                        for (value, label) in &control.choices {
                            if ui
                                .selectable_value(&mut next, Some(*value), label.as_ref())
                                .clicked()
                            {
                                changed = Some(*value);
                            }
                        }
                    });
            } else if let (Some([min, max]), Some(format)) =
                (control.display_range(), control.format)
                && max > min
            {
                let selector = control.parameter.selector.unwrap();
                let draft = self
                    .parameter_panel
                    .slider
                    .filter(|&(tone, key, _)| tone == index && key == selector);
                let mut value = draft
                    .map(|(_, _, value)| value)
                    .or_else(|| {
                        control
                            .bits
                            .or_else(|| control.neutral_bits())
                            .and_then(|bits| control.displayed(bits))
                    })
                    .unwrap_or(min);
                let slider = egui::Slider::new(&mut value, min..=max)
                    .fixed_decimals(format.decimals.unwrap_or(0).max(0) as usize)
                    .suffix(format.unit.as_deref().unwrap_or(""));
                let response = ui.add(slider);
                if response.dragged() || response.drag_started() {
                    self.parameter_panel.slider = Some((index, selector, value));
                } else if response.drag_stopped() || response.changed() {
                    self.parameter_panel.slider = None;
                    changed = control.bits_from_display(value);
                }
            }
            if unknown {
                ui.small("default unknown");
            } else if control.bits.is_none() && control.parameter.catalog.is_none() {
                ui.small("Not stored");
            }
        });
        changed
    }

    fn metadata_panel(&mut self, ui: &mut egui::Ui, index: usize) {
        ui.collapsing("Tone metadata", |ui| {
            let Some(tone) = self.pod_library.tone(index) else {
                return;
            };
            let original = PodMetadata::ALL
                .into_iter()
                .filter_map(|key| {
                    tone.metadata(key)
                        .ok()
                        .map(|stored| (key, stored.unwrap_or_default()))
                })
                .collect::<BTreeMap<_, _>>();
            if self
                .parameter_panel
                .metadata
                .as_ref()
                .is_none_or(|draft| draft.index != index || draft.original != original)
            {
                self.parameter_panel.metadata = Some(MetadataDraft {
                    index,
                    values: original.clone(),
                    original,
                });
            }
            let draft = self.parameter_panel.metadata.as_mut().unwrap();
            for key in PodMetadata::ALL {
                ui.horizontal(|ui| {
                    ui.label(key.label());
                    if let Some(value) = draft.values.get_mut(&key) {
                        if key == PodMetadata::Notes {
                            ui.text_edit_multiline(value);
                        } else {
                            ui.text_edit_singleline(value);
                        }
                    } else {
                        ui.small("Stored field unavailable");
                    }
                });
            }
            if self.pod_button(ui, "apply-metadata", "Apply metadata") {
                let draft = self
                    .parameter_panel
                    .metadata
                    .take()
                    .expect("visible metadata draft");
                if let Err(error) = self.table_edit(index, |tone| {
                    for (key, value) in &draft.values {
                        tone.set_metadata(*key, value)?;
                    }
                    Ok(PodModelChange::default())
                }) {
                    self.parameter_panel.metadata = Some(draft);
                    self.pod_message = error.detail();
                }
            }
        });
    }
}
