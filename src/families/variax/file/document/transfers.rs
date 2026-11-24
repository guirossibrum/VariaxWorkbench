//! Document-local model and bank buffers. Buffer history shares the document history.
use super::{EditorDocument, PresetFile, validate_slot_model};
use std::{slice, sync::Arc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotBuffer {
    ModelClipboard,
    BankClipboard,
    ModelSwap,
    BankSwap,
}

impl SlotBuffer {
    pub fn is_bank(self) -> bool {
        matches!(self, Self::BankClipboard | Self::BankSwap)
    }

    fn is_swap(self) -> bool {
        matches!(self, Self::ModelSwap | Self::BankSwap)
    }
}

#[derive(Clone, Default)]
pub(super) struct SlotBuffers {
    entries: [Option<Arc<Snapshot>>; 4],
}

struct Snapshot {
    label: String,
    models: Vec<PresetFile>,
}

fn same_models(left: &[PresetFile], right: &[PresetFile]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| left.as_bytes() == right.as_bytes())
}

impl EditorDocument {
    pub fn slot_buffer_label(&self, buffer: SlotBuffer) -> Option<&str> {
        self.state.buffers.entries[buffer as usize]
            .as_ref()
            .map(|snapshot| snapshot.label.as_str())
    }

    /// Capture the current model or bank contents for use elsewhere in this document.
    /// Recording a new buffer creates a history entry without changing file bytes.
    pub fn copy_to_slot_buffer(&mut self, buffer: SlotBuffer) -> Result<(), String> {
        let (label, models) = self.transfer_selection(buffer)?;
        if self.state.buffers.entries[buffer as usize]
            .as_ref()
            .is_some_and(|snapshot| {
                snapshot.label == label && same_models(&snapshot.models, models)
            })
        {
            return Ok(());
        }
        let snapshot = Snapshot {
            label,
            models: models.to_vec(),
        };
        self.end_gesture();
        let mut next = self.state.clone();
        next.buffers.entries[buffer as usize] = Some(Arc::new(snapshot));
        self.commit_state(next);
        Ok(())
    }

    /// Return destination and source labels after checking that a paste is admissible.
    pub fn slot_buffer_paste_labels(&self, buffer: SlotBuffer) -> Result<(String, &str), String> {
        let (label, _, snapshot) = self.paste_selection(buffer)?;
        Ok((label, &snapshot.label))
    }

    /// Paste or exchange with a buffer in one history entry.
    pub fn paste_from_slot_buffer(
        &mut self,
        buffer: SlotBuffer,
        exchange: bool,
    ) -> Result<(), String> {
        if exchange && !buffer.is_swap() {
            return Err("Choose a Swap Buffer to exchange models.".into());
        }
        let (label, models, snapshot) = self.paste_selection(buffer)?;
        let same_contents = same_models(&snapshot.models, models);
        let buffer_changed = exchange && (!same_contents || label != snapshot.label);
        if same_contents && !buffer_changed {
            return Ok(());
        }

        let mut next = self.state.clone();
        if buffer_changed {
            next.buffers.entries[buffer as usize] = Some(Arc::new(Snapshot {
                label,
                models: models.to_vec(),
            }));
        }
        if !same_contents {
            let bundle = next.bundle.as_mut().expect("validated bank selection");
            let start = if buffer.is_bank() { 0 } else { bundle.preset };
            bundle.file = Arc::new(
                bundle
                    .file
                    .with_replaced_models(bundle.bank, (start..).zip(&snapshot.models))?,
            );
            next.preset = bundle.file.banks()[bundle.bank].presets()[bundle.preset].clone();
        }

        self.end_gesture();
        self.commit_state(next);
        Ok(())
    }

    fn paste_selection(
        &self,
        buffer: SlotBuffer,
    ) -> Result<(String, &[PresetFile], &Snapshot), String> {
        let snapshot = self.state.buffers.entries[buffer as usize]
            .as_deref()
            .ok_or("Copy a model or bank to this buffer first.")?;
        let (label, models) = self.transfer_selection(buffer)?;
        if models.len() != snapshot.models.len() {
            return Err(format!(
                "Choose a bank with {} model slots. The selected bank has {}.",
                snapshot.models.len(),
                models.len()
            ));
        }
        Ok((label, models, snapshot))
    }

    fn transfer_selection(&self, buffer: SlotBuffer) -> Result<(String, &[PresetFile]), String> {
        let (bank, slot) = self
            .selection()
            .ok_or("Open a bank or collection and select a model slot first.")?;
        if buffer.is_bank() {
            let selected = &self.banks().expect("validated bank selection")[bank];
            for model in selected.presets() {
                validate_slot_model(model)?;
            }
            let label = if selected.name.is_empty() {
                format!("Bank {}", bank + 1)
            } else {
                format!("{} (bank {})", selected.name, bank + 1)
            };
            Ok((label, selected.presets()))
        } else {
            let model = self.selected_slot_model()?;
            Ok((
                format!("{} (bank {}, slot {})", model.name(), bank + 1, slot + 1),
                slice::from_ref(model),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn collection() -> EditorDocument {
        EditorDocument::from_file_bytes(
            include_bytes!("../../../../../tests/fixtures/synthetic-collection.l6c").to_vec(),
            Path::new("library.l6c"),
        )
        .unwrap()
    }

    #[test]
    fn model_clipboard_paste_and_swap_have_separate_single_undo_steps() {
        let mut doc = collection();
        let original = doc.as_bytes().to_vec();
        let first = doc.selection().unwrap();
        assert_eq!(first, (1, 0));
        let source = doc.preset().as_bytes().to_vec();
        doc.copy_to_slot_buffer(SlotBuffer::ModelClipboard).unwrap();
        let clipboard_label = doc
            .slot_buffer_label(SlotBuffer::ModelClipboard)
            .unwrap()
            .to_owned();
        assert_eq!(doc.as_bytes(), original);
        assert!(!doc.is_dirty());
        assert!(doc.can_undo());
        assert!(
            collection()
                .slot_buffer_label(SlotBuffer::ModelClipboard)
                .is_none()
        );

        doc.undo();
        assert!(doc.slot_buffer_label(SlotBuffer::ModelClipboard).is_none());
        assert!(!doc.can_undo());
        doc.redo();
        assert_eq!(
            doc.slot_buffer_label(SlotBuffer::ModelClipboard),
            Some(clipboard_label.as_str())
        );

        doc.select(2, 1).unwrap();
        let destination = doc.preset().as_bytes().to_vec();
        doc.paste_from_slot_buffer(SlotBuffer::ModelClipboard, false)
            .unwrap();
        assert_eq!(doc.preset().as_bytes(), source);
        assert!(doc.slot_is_dirty(2, 1));
        assert!(!doc.slot_is_dirty(1, 0));
        doc.undo();
        assert_eq!(doc.preset().as_bytes(), destination);
        assert!(!doc.is_dirty());
        doc.redo();
        assert_eq!(doc.preset().as_bytes(), source);
        doc.undo();

        doc.copy_to_slot_buffer(SlotBuffer::ModelSwap).unwrap();
        let swap_label = doc
            .slot_buffer_label(SlotBuffer::ModelSwap)
            .unwrap()
            .to_owned();
        doc.select(first.0, first.1).unwrap();
        doc.paste_from_slot_buffer(SlotBuffer::ModelSwap, true)
            .unwrap();
        assert_eq!(doc.preset().as_bytes(), destination);
        assert_eq!(
            doc.slot_buffer_label(SlotBuffer::ModelSwap),
            Some(clipboard_label.as_str())
        );
        assert!(doc.slot_is_dirty(first.0, first.1));
        assert!(!doc.slot_is_dirty(2, 1));
        doc.undo();
        assert_eq!(doc.as_bytes(), original);
        assert_eq!(
            doc.slot_buffer_label(SlotBuffer::ModelSwap),
            Some(swap_label.as_str())
        );
        doc.redo();
        assert_eq!(doc.preset().as_bytes(), destination);
        assert_eq!(
            doc.slot_buffer_label(SlotBuffer::ModelSwap),
            Some(clipboard_label.as_str())
        );
        assert_eq!(
            doc.slot_buffer_label(SlotBuffer::ModelClipboard),
            Some(clipboard_label.as_str())
        );

        doc.select(2, 2).unwrap();
        doc.paste_from_slot_buffer(SlotBuffer::ModelSwap, false)
            .unwrap();
        assert_eq!(doc.preset().as_bytes(), source);
        assert_eq!(
            doc.slot_buffer_label(SlotBuffer::ModelSwap),
            Some(clipboard_label.as_str())
        );
    }

    #[test]
    fn mismatched_bank_counts_leave_the_buffer_file_selection_and_redo_intact() {
        let mut doc = collection();
        let original = doc.as_bytes().to_vec();
        doc.copy_to_slot_buffer(SlotBuffer::BankClipboard).unwrap();
        doc.copy_to_slot_buffer(SlotBuffer::BankSwap).unwrap();
        doc.copy_to_slot_buffer(SlotBuffer::ModelClipboard).unwrap();
        doc.undo();
        assert!(doc.can_redo());

        doc.select(2, 1).unwrap();
        for (buffer, exchange) in [
            (SlotBuffer::BankClipboard, false),
            (SlotBuffer::BankSwap, true),
        ] {
            let error = doc.slot_buffer_paste_labels(buffer).unwrap_err();
            assert!(error.contains("2 model slots") && error.contains("has 3"));
            assert!(doc.paste_from_slot_buffer(buffer, exchange).is_err());
        }
        assert!(
            doc.paste_from_slot_buffer(SlotBuffer::BankClipboard, true)
                .is_err()
        );
        assert_eq!(doc.as_bytes(), original);
        assert_eq!(doc.selection(), Some((2, 1)));
        assert!(doc.can_redo());
        assert_eq!(
            doc.slot_buffer_label(SlotBuffer::BankSwap),
            doc.slot_buffer_label(SlotBuffer::BankClipboard)
        );
        assert!(doc.slot_buffer_label(SlotBuffer::ModelClipboard).is_none());
    }
}
