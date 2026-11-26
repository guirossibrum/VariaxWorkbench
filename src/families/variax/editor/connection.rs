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
        has_download: bool,
    ) -> Option<egui::Response> {
        ui.weak(self.guitar_connection_status());
        #[cfg(feature = "usb")]
        if self.follow.active() {
            ui.weak("Following the guitar");
        }
        if has_download || self.connection_status.is_none() {
            return None;
        }
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
                    button = editor.connection_controls(ui, &mut services, false);
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

    fn count_labels(output: &egui::FullOutput, label: &str) -> usize {
        fn count(shape: &egui::Shape, label: &str) -> usize {
            match shape {
                egui::Shape::Text(text) => usize::from(text.galley.text() == label),
                egui::Shape::Vec(shapes) => shapes.iter().map(|shape| count(shape, label)).sum(),
                _ => 0,
            }
        }
        output
            .shapes
            .iter()
            .map(|shape| count(&shape.shape, label))
            .sum()
    }

    #[test]
    fn active_variax_tab_has_one_connection_status_and_one_guitar_action() {
        let (ctx, mut app) = app();
        let index = app.tabs.index(app.tabs.active.unwrap()).unwrap();
        app.tabs.entries[index].device = Some(super::super::refresh::tests::verified_tab());
        let render = |app: &mut crate::editor::Workbench| {
            ctx.run(egui::RawInput::default(), |ctx| app.render(ctx))
        };
        let connected = render(&mut app);
        assert_eq!(count_labels(&connected, "Guitar: Variax 600 3.10"), 1);
        assert_eq!(count_labels(&connected, "Download from guitar…"), 1);
        assert_eq!(count_labels(&connected, "Refresh guitar"), 0);
        assert_eq!(
            count_labels(&connected, "Identity verified · firmware 3.10"),
            0
        );

        let tab = app.tabs.entries[index].device.as_mut().unwrap();
        tab.device.support = DeviceSupport::Refreshing("3.10");
        tab.device.guitar_status = Some("Guitar: checking…".into());
        let checking = render(&mut app);
        assert_eq!(count_labels(&checking, "Guitar: checking…"), 1);
        assert_eq!(count_labels(&checking, "Download from guitar…"), 0);
        assert_eq!(count_labels(&checking, "Refresh guitar"), 1);
    }

    #[test]
    fn firmware_notice_stays_with_the_offline_editor_and_clears_on_verification() {
        use super::super::{EditorDocument, FamilyTab};
        use variax_workbench::firmware_notice::FirmwareNotice;

        let (ctx, mut app) = app();
        let id = app.tabs.active.unwrap();
        let index = app.tabs.index(id).unwrap();
        let mut tab = super::super::refresh::tests::verified_tab();
        let verified = tab.device.support.clone();
        let mut reply = variax_workbench::identity_stage::guitar_reply();
        reply[12..16].copy_from_slice(b"0309");
        let identity = line6_devices::identity::Identity::parse(&reply).unwrap();
        let notice = FirmwareNotice::variax(&identity).unwrap();
        let message = notice.message();
        assert!(message.contains("reports firmware 3.09"));
        assert!(message.contains("firmware 3.10"));
        assert!(message.contains("You can open, edit and save files"));
        assert!(message.contains("Device actions are unavailable"));
        assert!(!message.to_lowercase().contains("update"));

        tab.device.support = DeviceSupport::UnverifiedGuitarFirmware(notice);
        tab.device.guitar_status = Some("Guitar: identity unverified".into());
        tab.device.refresh_guitar = false;
        assert!(tab.verified_attachment().is_none());
        app.tabs.entries[index].device = Some(tab);

        let render = |app: &mut crate::editor::Workbench| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1440.0, 960.0),
                    )),
                    ..Default::default()
                },
                |ctx| app.render(ctx),
            )
        };
        let offline = render(&mut app);
        assert_eq!(count_labels(&offline, &message), 1);
        assert_eq!(count_labels(&offline, "Download from guitar…"), 0);

        let folder = tempfile::tempdir().unwrap();
        let input = folder.path().join("original.vwm");
        let output = folder.path().join("offline-copy.vwm");
        let original = include_bytes!("../../../../tests/fixtures/editor-preset.vwm");
        std::fs::write(&input, original).unwrap();
        app.dialogs = Box::new(FakeDialogs {
            open: [Some(input.clone())].into(),
            save: [Some(output.clone())].into(),
            ..Default::default()
        });
        app.with_tab(id, |entry, services| {
            let editor = (entry.editor.as_mut() as &mut dyn std::any::Any)
                .downcast_mut::<VariaxEditor>()
                .unwrap();
            editor.open_dialog(&ctx, services);
        });
        app.poll_picker(&ctx);
        assert_eq!(app.variax().source_path(), Some(input.as_path()));
        app.variax_mut()
            .change(|preset| preset.rename("Offline edit"));
        assert!(app.variax().dirty());
        app.with_tab(id, |entry, services| entry.editor.save_copy(&ctx, services));
        app.poll_picker(&ctx);
        assert_eq!(
            EditorDocument::open(&output).unwrap().preset().name(),
            "Offline edit"
        );
        assert_eq!(std::fs::read(&input).unwrap(), original);
        assert_eq!(count_labels(&render(&mut app), &message), 1);

        app.variax_mut()
            .change(|preset| preset.rename("Keep my edits"));
        app.tabs.entries[index].device.as_mut().unwrap().connection = Connection::Disconnected;
        assert_eq!(count_labels(&render(&mut app), &message), 0);
        let tab = app.tabs.entries[index].device.as_mut().unwrap();
        tab.connection = Connection::Present;
        tab.device.support = verified;
        assert_eq!(count_labels(&render(&mut app), &message), 0);
        assert_eq!(
            app.variax().document.as_ref().unwrap().preset().name(),
            "Keep my edits"
        );
        assert!(app.variax().dirty());
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
