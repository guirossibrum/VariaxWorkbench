use super::{EditorServices, VariaxEditor, egui};
use variax_workbench::document::SlotBuffer;

pub(super) struct SlotTransfer {
    selection: (usize, usize),
    buffer: SlotBuffer,
    exchange: bool,
    destination: String,
    source: String,
}

impl VariaxEditor {
    pub(super) fn model_transfer_actions(&mut self, ui: &mut egui::Ui) {
        self.transfer_commands(
            ui,
            "model",
            SlotBuffer::ModelClipboard,
            SlotBuffer::ModelSwap,
        );
    }

    pub(super) fn bank_transfer_actions(&mut self, ui: &mut egui::Ui) {
        self.transfer_commands(ui, "bank", SlotBuffer::BankClipboard, SlotBuffer::BankSwap);
    }

    fn transfer_commands(
        &mut self,
        ui: &mut egui::Ui,
        kind: &str,
        clipboard: SlotBuffer,
        swap: SlotBuffer,
    ) {
        for (label, buffer, paste, exchange) in [
            (format!("Copy {kind}"), clipboard, false, false),
            (format!("Paste {kind}…"), clipboard, true, false),
            (format!("Copy {kind} to Swap Buffer"), swap, false, false),
            (format!("Paste {kind} from Swap Buffer…"), swap, true, false),
            (format!("Swap {kind} with Swap Buffer…"), swap, true, true),
        ] {
            let stored = self
                .document
                .as_ref()
                .and_then(|document| document.slot_buffer_label(buffer));
            let response = ui.add_enabled(!paste || stored.is_some(), egui::Button::new(&label));
            #[cfg(test)]
            self.slot_transfer_buttons.insert(label, response.rect);
            let response = if let Some(stored) = stored {
                response.on_hover_text(format!("In buffer: {stored}"))
            } else {
                response
            };
            if response.clicked() {
                if paste {
                    self.stage_slot_transfer(buffer, exchange);
                } else {
                    self.copy_slot_buffer(buffer);
                }
                ui.close();
            }
        }
    }

    fn copy_slot_buffer(&mut self, buffer: SlotBuffer) {
        let Some(document) = &mut self.document else {
            return;
        };
        match document.copy_to_slot_buffer(buffer) {
            Ok(()) => {
                self.error = None;
                self.status = format!(
                    "Copied {}.",
                    document.slot_buffer_label(buffer).unwrap_or_default()
                );
            }
            Err(error) => self.error = Some(error),
        }
    }

    fn stage_slot_transfer(&mut self, buffer: SlotBuffer, exchange: bool) {
        let Some(document) = &self.document else {
            return;
        };
        let Some(selection) = document.selection() else {
            return;
        };
        match document.slot_buffer_paste_labels(buffer) {
            Ok((destination, source)) => {
                self.error = None;
                self.slot_transfer = Some(SlotTransfer {
                    selection,
                    buffer,
                    exchange,
                    destination,
                    source: source.into(),
                });
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub(super) fn slot_transfer_dialog(
        &mut self,
        services: &mut EditorServices<'_>,
        ctx: &egui::Context,
    ) {
        #[cfg(not(test))]
        let _ = services;
        let Some(transfer) = self.slot_transfer.take() else {
            return;
        };
        let destination = &transfer.destination;
        let source = &transfer.source;
        let target = if transfer.buffer.is_bank() {
            "bank models"
        } else {
            "model"
        };
        let (title, message, confirm) = if transfer.exchange {
            (
                "Swap with buffer?",
                format!(
                    "Exchange {target} from “{destination}” with “{source}” in the Swap Buffer?"
                ),
                "Swap",
            )
        } else {
            (
                "Paste into selection?",
                format!("Replace the {target} in “{destination}” with the copy from “{source}”?"),
                "Paste",
            )
        };
        let modal = egui::Modal::new(egui::Id::new("variax-slot-transfer")).show(ctx, |ui| {
            ui.set_max_width(400.0);
            ui.heading(title);
            ui.label(message);
            ui.horizontal(|ui| {
                let cancel = ui.button("Cancel");
                let confirm = ui.button(confirm);
                #[cfg(test)]
                {
                    services
                        .prompt_buttons
                        .insert("cancel-transfer", cancel.rect);
                    services
                        .prompt_buttons
                        .insert("confirm-transfer", confirm.rect);
                }
                (cancel.clicked(), confirm.clicked())
            })
            .inner
        });
        let (cancel, confirm) = modal.inner;
        if cancel || modal.should_close() {
            return;
        }
        if !confirm {
            self.slot_transfer = Some(transfer);
            return;
        }
        let Some(document) = self
            .document
            .as_mut()
            .filter(|document| document.selection() == Some(transfer.selection))
        else {
            self.error = Some("The selected slot changed. Choose the transfer again.".into());
            return;
        };
        match document.paste_from_slot_buffer(transfer.buffer, transfer.exchange) {
            Ok(()) => {
                self.error = None;
                self.status = if transfer.exchange {
                    format!("Swapped {target} with the buffer.")
                } else {
                    format!("Pasted {target} from “{source}”.")
                };
                self.drag_origin = None;
                self.clear_text_drafts();
            }
            Err(error) => self.error = Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::Workbench;
    use crate::editor::tests::{app, click_prompt};

    fn frame(
        ctx: &egui::Context,
        app: &mut Workbench,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1120.0, 760.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.guitar_workspace(ui));
                app.pending_dialog(ctx);
            },
        )
    }

    fn click(ctx: &egui::Context, app: &mut Workbench, position: egui::Pos2) {
        let button = |pressed| egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(
            ctx,
            app,
            vec![egui::Event::PointerMoved(position), button(true)],
        );
        frame(ctx, app, vec![button(false)]);
        frame(ctx, app, vec![]);
        frame(ctx, app, vec![]);
    }

    fn command(ctx: &egui::Context, app: &mut Workbench, label: &str) {
        frame(ctx, app, vec![]);
        frame(ctx, app, vec![]);
        let menu = if label.contains("bank") {
            "library-actions"
        } else {
            "slot-transfers"
        };
        click(ctx, app, app.variax().slot_file_buttons[menu].center());
        click(ctx, app, app.variax().slot_transfer_buttons[label].center());
    }

    fn choose_bank(ctx: &egui::Context, app: &mut Workbench, bank: usize) {
        frame(ctx, app, vec![]);
        click(ctx, app, app.variax().bank_dropdown.unwrap().center());
        click(ctx, app, app.variax().bank_options[&bank].center());
        assert_eq!(
            app.variax()
                .document
                .as_ref()
                .unwrap()
                .selection()
                .unwrap()
                .0,
            bank
        );
    }

    fn dirty_markers(output: egui::FullOutput) -> usize {
        fn count(shape: &egui::Shape) -> usize {
            match shape {
                egui::Shape::Text(text) if text.galley.text() == "!" => 1,
                egui::Shape::Vec(shapes) => shapes.iter().map(count).sum(),
                _ => 0,
            }
        }
        output.shapes.iter().map(|shape| count(&shape.shape)).sum()
    }

    #[test]
    fn model_transfer_menu_pastes_the_copied_slot_into_the_selected_slot() {
        let (ctx, mut app) = app();
        app.variax_mut().open_example();
        frame(&ctx, &mut app, vec![]);
        let original = app.variax().document.as_ref().unwrap().as_bytes().to_vec();
        let copy = app
            .variax()
            .document
            .as_ref()
            .unwrap()
            .preset()
            .as_bytes()
            .to_vec();
        command(&ctx, &mut app, "Copy model");
        let destination = app.variax().bank_slots[&0].center();
        click(&ctx, &mut app, destination);
        command(&ctx, &mut app, "Paste model…");
        assert!(app.has_prompt());
        assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), original);
        click_prompt(&ctx, &mut app, "confirm-transfer");
        let document = app.variax().document.as_ref().unwrap();
        assert_eq!(document.selection(), Some((0, 0)));
        assert_eq!(document.preset().as_bytes(), copy);
        assert_eq!(dirty_markers(frame(&ctx, &mut app, vec![])), 1);
        app.variax_mut().undo();
        assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), original);
        assert_eq!(dirty_markers(frame(&ctx, &mut app, vec![])), 0);
    }

    #[test]
    fn bank_transfer_menu_confirms_paste_and_swap_and_refreshes_all_slot_markers() {
        let (ctx, mut app) = app();
        app.variax_mut().open_example();
        let original = app.variax().document.as_ref().unwrap().as_bytes().to_vec();
        assert_eq!(dirty_markers(frame(&ctx, &mut app, vec![])), 0);

        command(&ctx, &mut app, "Copy bank");
        assert!(!app.has_prompt());
        assert!(!app.variax().dirty());
        app.variax_mut().undo();
        assert!(
            app.variax()
                .document
                .as_ref()
                .unwrap()
                .slot_buffer_label(SlotBuffer::BankClipboard)
                .is_none()
        );
        app.variax_mut().redo();
        choose_bank(&ctx, &mut app, 1);
        command(&ctx, &mut app, "Paste bank…");
        assert!(app.has_prompt());
        click_prompt(&ctx, &mut app, "cancel-transfer");
        assert!(!app.has_prompt());
        assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), original);

        command(&ctx, &mut app, "Paste bank…");
        assert!(app.has_prompt());
        click_prompt(&ctx, &mut app, "confirm-transfer");
        assert!(!app.has_prompt());
        let document = app.variax().document.as_ref().unwrap();
        let banks = document.banks().unwrap();
        assert_eq!(document.selection(), Some((1, 1)));
        assert_eq!(banks[1].name, "Custom 2");
        for (i, (from, into)) in banks[0]
            .presets()
            .iter()
            .zip(banks[1].presets())
            .enumerate()
        {
            assert_eq!(from.as_bytes(), into.as_bytes());
            assert!(!document.slot_is_dirty(0, i));
            assert!(document.slot_is_dirty(1, i));
        }
        assert_eq!(dirty_markers(frame(&ctx, &mut app, vec![])), 5);
        app.variax_mut().undo();
        assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), original);
        assert_eq!(dirty_markers(frame(&ctx, &mut app, vec![])), 0);
        app.variax_mut().redo();
        assert_eq!(dirty_markers(frame(&ctx, &mut app, vec![])), 5);
        app.variax_mut().undo();

        choose_bank(&ctx, &mut app, 0);
        command(&ctx, &mut app, "Copy bank to Swap Buffer");
        choose_bank(&ctx, &mut app, 1);
        command(&ctx, &mut app, "Swap bank with Swap Buffer…");
        assert!(app.has_prompt());
        click_prompt(&ctx, &mut app, "confirm-transfer");
        assert!(!app.has_prompt());
        let document = app.variax().document.as_ref().unwrap();
        assert_eq!(
            document.slot_buffer_label(SlotBuffer::BankSwap),
            Some("Custom 2 (bank 2)")
        );
        assert_ne!(document.as_bytes(), original);
        app.variax_mut().undo();
        let document = app.variax().document.as_ref().unwrap();
        assert_eq!(document.as_bytes(), original);
        assert_eq!(
            document.slot_buffer_label(SlotBuffer::BankSwap),
            Some("Custom 1 (bank 1)")
        );
    }
}
