use super::*;
use std::collections::{BTreeMap, BTreeSet};
use variax_workbench::pod_preset::{PodControl, PodMetadata, PodModelChange, PodTone};

#[derive(Default)]
pub(super) struct ParameterPanel {
    pub(super) packs: u8,
    pub(super) keep_amp_settings: bool,
    histories: BTreeMap<usize, ToneHistory>,
    pub(super) unknown: BTreeMap<usize, BTreeSet<u32>>,
    metadata: Option<MetadataDraft>,
    pub(super) slider: Option<(usize, u32, f64)>,
}

#[derive(Default)]
struct ToneHistory {
    undo: Vec<Revision>,
    redo: Vec<Revision>,
}

struct Revision {
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
        self.histories.clear();
        self.unknown.clear();
        self.metadata = None;
        self.slider = None;
    }

    pub(super) fn clear_tone_history(&mut self, index: usize) {
        self.histories.remove(&index);
        self.unknown.remove(&index);
    }

    pub(super) fn can_history(&self, index: Option<usize>, redo: bool) -> bool {
        index
            .and_then(|index| self.histories.get(&index))
            .is_some_and(|history| !if redo { &history.redo } else { &history.undo }.is_empty())
    }
}

impl ToneHistory {
    fn stack(&mut self, redo: bool) -> &mut Vec<Revision> {
        if redo { &mut self.redo } else { &mut self.undo }
    }

    fn push(&mut self, redo: bool, revision: Revision) {
        let stack = self.stack(redo);
        stack.push(revision);
        if stack.len() > 32 {
            stack.remove(0);
        }
    }
}

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
            bytes: tone.as_bytes().to_vec(),
            unknown: prior_unknown,
        };
        self.replace_pod_tone(index, candidate.into_bytes())?;
        self.parameter_panel.unknown.insert(index, unknown);
        let history = self.parameter_panel.histories.entry(index).or_default();
        history.push(false, revision);
        history.redo.clear();
        self.pod_message = "Tone edited offline.".into();
        Ok(())
    }

    pub(super) fn table_history(&mut self, redo: bool) {
        let Some(index) = self.pod_selected else {
            return;
        };
        let panel = &mut self.parameter_panel;
        let Some(revision) = panel
            .histories
            .get_mut(&index)
            .and_then(|history| history.stack(redo).pop())
        else {
            return;
        };
        let Some(current) = self.pod_library.tone(index) else {
            return;
        };
        let inverse = Revision {
            bytes: current.as_bytes().to_vec(),
            unknown: panel.unknown.get(&index).cloned().unwrap_or_default(),
        };
        match self.replace_pod_tone(index, revision.bytes.clone()) {
            Ok(()) => {
                self.parameter_panel.unknown.insert(index, revision.unknown);
                self.parameter_panel
                    .histories
                    .entry(index)
                    .or_default()
                    .push(!redo, inverse);
                self.refresh_selected_pod_name();
                self.pod_message = if redo {
                    "Tone edit redone."
                } else {
                    "Tone edit undone."
                }
                .into();
            }
            Err(error) => {
                self.parameter_panel
                    .histories
                    .entry(index)
                    .or_default()
                    .push(redo, revision);
                self.pod_message = error.detail();
            }
        }
    }

    pub(super) fn metadata_panel(&mut self, ui: &mut egui::Ui, index: usize) {
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
