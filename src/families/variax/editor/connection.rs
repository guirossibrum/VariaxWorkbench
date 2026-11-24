use super::{EditorServices, VariaxEditor, egui};
use crate::editor::tabs::{Connection, DeviceTab};
use variax_workbench::ui::theme;

pub(super) fn status(device: &DeviceTab) -> String {
    if device.connection == Connection::Present
        && let Some(status) = &device.device.guitar_status
    {
        return status.clone();
    }
    format!("Guitar: {}", device.status())
}

impl VariaxEditor {
    fn guitar_connection_status(&self) -> &str {
        #[cfg(feature = "usb")]
        if self.follow.disconnected() {
            return "Guitar disconnected";
        }
        self.connection_status
            .as_deref()
            .unwrap_or("Guitar offline")
    }

    pub(super) fn connection_controls(
        &self,
        ui: &mut egui::Ui,
        services: &mut EditorServices<'_>,
    ) -> Option<egui::Response> {
        ui.weak(self.guitar_connection_status());
        self.connection_status.as_ref()?;
        let button = ui.add_enabled(
            services.can_refresh_guitar(),
            egui::Button::new("Refresh guitar").min_size(egui::vec2(0.0, theme::TARGET)),
        );
        if button.clicked() {
            services.request_guitar_refresh();
        }
        Some(button)
    }
}

#[cfg(all(test, feature = "usb"))]
mod tests {
    use super::*;
    use crate::editor::{CloseResult, FakeDialogs, Picker, detection::DeviceSupport, tests::app};
    use std::collections::HashMap;

    fn frame(
        ctx: &egui::Context,
        editor: &VariaxEditor,
        request: Option<&mut bool>,
        events: Vec<egui::Event>,
    ) -> Option<egui::Response> {
        let mut dialogs = FakeDialogs::default();
        let mut picker = Picker::default();
        let mut prompts = HashMap::new();
        let mut services = EditorServices {
            dialogs: &mut dialogs,
            picker: &mut picker,
            tab: 0,
            close: CloseResult::Pending,
            guitar_refresh: request,
            prompt_buttons: &mut prompts,
        };
        let mut button = None;
        let _ = ctx.run(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    button = editor.connection_controls(ui, &mut services);
                });
            },
        );
        button
    }

    fn click(ctx: &egui::Context, editor: &VariaxEditor, pos: egui::Pos2, request: &mut bool) {
        let button = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(
            ctx,
            editor,
            Some(&mut *request),
            vec![egui::Event::PointerMoved(pos)],
        );
        frame(ctx, editor, Some(&mut *request), vec![button(true)]);
        frame(ctx, editor, Some(request), vec![button(false)]);
    }

    #[test]
    fn refresh_guitar_control_tracks_the_tab_and_dispatches_through_services() {
        use crate::editor::tabs::FamilyTab;

        let (ctx, mut app) = app();
        let editor = app.variax_mut();
        editor.change(|preset| preset.rename("Keep this edit"));
        let before = editor.document.as_ref().unwrap().as_bytes().to_vec();

        assert_eq!(editor.guitar_connection_status(), "Guitar offline");
        assert!(frame(&ctx, editor, None, vec![]).is_none());

        let mut tab = super::super::refresh::tests::verified_tab();
        editor.observe_device(Some(&tab));
        assert_eq!(editor.guitar_connection_status(), "Guitar: Variax 600 3.10");
        let mut requested = false;
        let available = frame(&ctx, editor, Some(&mut requested), vec![]).unwrap();
        assert!(available.enabled());
        click(&ctx, editor, available.rect.center(), &mut requested);
        assert!(requested);

        tab.device.support = DeviceSupport::Refreshing("3.10");
        tab.device.guitar_status = Some("Guitar: checking…".into());
        editor.observe_device(Some(&tab));
        assert_eq!(editor.guitar_connection_status(), "Guitar: checking…");
        assert!(!frame(&ctx, editor, None, vec![]).unwrap().enabled());

        tab.connection = Connection::Disconnected;
        editor.observe_device(Some(&tab));
        assert!(editor.guitar_connection_status().contains("Disconnected"));
        assert!(!frame(&ctx, editor, None, vec![]).unwrap().enabled());
        assert!(editor.dirty());
        assert_eq!(editor.document.as_ref().unwrap().as_bytes(), before);
    }
}
