use super::tuning_presets::PITCH;
use super::{
    BODY, EditorDocument, EditorServices, ModelArt, PICKUP1, PICKUP2, PickRequest, PresetFile,
    TUNING, VariaxEditor, egui,
};
use std::fmt::Write as _;
use variax_workbench::{document::save_new, edit::is_acoustic, preset::PresetText};

impl VariaxEditor {
    pub(super) fn html_export_button(
        &mut self,
        ui: &mut egui::Ui,
        services: &mut EditorServices<'_>,
        available: bool,
    ) -> bool {
        let button = ui.add_enabled(available, egui::Button::new("Export HTML Model List…"));
        #[cfg(test)]
        {
            self.html_export_button = Some(button.rect);
        }
        if button.clicked()
            && let Some(document) = &self.document
        {
            let html = render_model_list(document, &self.catalog);
            let request = PickRequest::SaveWorkbenchFile {
                name: "Variax Model List.html".into(),
                extension: "html".into(),
            };
            services.request_picker::<Self>(ui.ctx(), request, move |editor, path, _| {
                let Some(path) = path else {
                    return;
                };
                let path = if path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("htm"))
                {
                    path
                } else {
                    super::slot_files::with_file_extension(path, "html")
                };
                match save_new(&path, html.as_bytes()) {
                    Ok(()) => {
                        editor.error = None;
                        editor.status = format!("Exported model list to {}", path.display());
                    }
                    Err(error) => editor.error = Some(error),
                }
            });
        }
        button.clicked()
    }
}

fn render_model_list(document: &EditorDocument, catalog: &[ModelArt]) -> String {
    let mut html = String::from(concat!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\n",
        "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n",
        "<title>Variax Model List</title>\n",
        "<style>body{font-family:system-ui,sans-serif;margin:2rem}",
        "table{border-collapse:collapse;width:100%}th,td{text-align:left;vertical-align:top;",
        "padding:.45rem;border-bottom:1px solid #888}.location,.tuning{white-space:nowrap}",
        ".notes{white-space:pre-wrap}tbody tr:nth-child(even){background:rgba(128,128,128,.12)}",
        "</style></head><body><h1>Variax Model List</h1>\n",
        "<table><thead><tr><th scope=\"col\">Slot</th><th scope=\"col\">Model</th>",
        "<th scope=\"col\">Body</th><th scope=\"col\">Pickup 1</th><th scope=\"col\">Pickup 2</th>",
        "<th scope=\"col\">Tuning (low to high)</th><th scope=\"col\">Notes</th>",
        "</tr></thead>\n"
    ));
    if let Some(banks) = document.banks() {
        for (index, bank) in banks.iter().enumerate() {
            let fallback = format!("Bank {}", index + 1);
            let name = if bank.name.is_empty() {
                &fallback
            } else {
                &bank.name
            };
            html.push_str("<tbody><tr><th colspan=\"7\" scope=\"rowgroup\">");
            escape_html(&mut html, name);
            html.push_str("</th></tr>\n");
            if bank.presets().is_empty() {
                html.push_str("<tr><td colspan=\"7\">No models</td></tr>\n");
            }
            for (slot, preset) in bank.presets().iter().enumerate() {
                write_model(&mut html, &(slot + 1).to_string(), preset, catalog);
            }
            html.push_str("</tbody>\n");
        }
    } else if let Some(preset) = document.selected_preset() {
        html.push_str("<tbody>\n");
        write_model(&mut html, "—", preset, catalog);
        html.push_str("</tbody>\n");
    }
    html.push_str("</table></body></html>\n");
    html
}

fn write_model(html: &mut String, location: &str, preset: &PresetFile, catalog: &[ModelArt]) {
    let body = preset.model(BODY);
    let acoustic = body.is_some_and(|model| is_acoustic(model.id));
    html.push_str("<tr><td class=\"location\">");
    escape_html(html, location);
    html.push_str("</td><th scope=\"row\">");
    escape_html(html, preset.name());
    html.push_str("</th><td>");
    if let Some(model) = body {
        write_model_name(html, catalog, model.id);
    } else {
        html.push('—');
    }
    for slot in [PICKUP1, PICKUP2] {
        html.push_str("</td><td>");
        if let Some(model) = preset.model(slot).filter(|_| !acoustic) {
            write_model_name(html, catalog, model.id);
            if !model.enabled {
                html.push_str(" (off)");
            }
        } else {
            html.push('—');
        }
    }
    html.push_str("</td><td class=\"tuning\">");
    match preset.model(TUNING) {
        Some(tuning) if tuning.enabled => {
            for string in 0..6 {
                if string != 0 {
                    html.push(' ');
                }
                let note = preset
                    .parameter(TUNING, PITCH + string as u32)
                    .filter(|offset| offset.value_type == 0)
                    .and_then(|offset| super::guitar::tuning_note(string, offset.bits as i32));
                html.push_str(note.unwrap_or("—"));
            }
        }
        Some(_) => html.push_str("Standard (alternate tuning off)"),
        None => html.push('—'),
    }
    html.push_str("</td><td class=\"notes\">");
    if let Ok(notes) = preset.metadata(PresetText::Notes) {
        escape_html(html, &notes);
    }
    html.push_str("</td></tr>\n");
}

fn write_model_name(html: &mut String, catalog: &[ModelArt], id: u32) {
    if let Some(model) = catalog.iter().find(|model| model.id == id) {
        escape_html(html, &model.name);
    } else {
        write!(html, "Unknown ({id:#010x})").expect("write to String");
    }
}

fn escape_html(html: &mut String, text: &str) {
    for character in text.chars() {
        match character {
            '&' => html.push_str("&amp;"),
            '<' => html.push_str("&lt;"),
            '>' => html.push_str("&gt;"),
            '"' => html.push_str("&quot;"),
            '\'' => html.push_str("&#39;"),
            _ => html.push(character),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::{FakeDialogs, Workbench, tests::app};
    use std::fs;
    use variax_workbench::bundle::BundleFile;

    #[test]
    fn model_list_escapes_fields_and_shows_tuning_and_pickup_state() {
        let model = PresetFile::default_legacy_model().unwrap();
        let mut document =
            EditorDocument::from_bundle(BundleFile::new_collection().unwrap(), Default::default())
                .unwrap();
        let catalog = super::super::load_catalog();
        let empty = render_model_list(&document, &catalog);
        assert!(empty.contains(">New Folder</th>"));
        assert!(empty.contains(">No models</td>"));
        document.rename_selected_bank("Rock & <Jazz>").unwrap();
        document.append_collection_model(model).unwrap();
        document
            .apply(|model| {
                model.rename("A <B> & C")?;
                model.set_metadata(PresetText::Notes, "Résumé: \"A\" & 'B'\n<script>")?;
                model.set_parameter(TUNING, PITCH, 0, (-2_i32) as u32)?;
                model.set_enabled(TUNING, true)
            })
            .unwrap();

        let html = render_model_list(&document, &catalog);
        assert!(html.contains(">Rock &amp; &lt;Jazz&gt;</th>"));
        assert!(html.contains(">A &lt;B&gt; &amp; C</th>"));
        assert!(html.contains(">Spank</td>"));
        assert!(html.contains(">Spank Single Coil Bridge</td>"));
        assert!(html.contains(">Spank Single Coil Middle (off)</td>"));
        assert!(html.contains(">D A D G B E</td>"));
        assert!(html.contains("Résumé: &quot;A&quot; &amp; &#39;B&#39;\n&lt;script&gt;"));
        assert!(!html.contains("<script>"));
    }

    #[test]
    fn html_export_picker_cancels_and_refuses_collisions_without_changing_the_document() {
        fn frame(ctx: &egui::Context, app: &mut Workbench, events: Vec<egui::Event>) {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1120.0, 760.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| app.render(ctx),
            );
        }
        fn export(ctx: &egui::Context, app: &mut Workbench) {
            frame(ctx, app, vec![]);
            frame(ctx, app, vec![]);
            let click = |app: &mut Workbench, position| {
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
            };
            click(
                app,
                app.variax().slot_file_buttons["library-actions"].center(),
            );
            click(app, app.variax().html_export_button.unwrap().center());
        }

        let (ctx, mut app) = app();
        app.variax_mut().open_example();
        app.variax_mut()
            .change(|model| model.rename("Edited <model>"));
        let before = app.variax().document.as_ref().unwrap().as_bytes().to_vec();
        let directory = tempfile::tempdir().unwrap();
        let collision = directory.path().join("Existing.HTM");
        let output = directory.path().join("model-list");
        save_new(&collision, b"Keep this").unwrap();
        app.dialogs = Box::new(FakeDialogs {
            save: [None, Some(collision.clone()), Some(output.clone())].into(),
            ..Default::default()
        });
        export(&ctx, &mut app);
        assert!(app.variax().error.is_none());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        export(&ctx, &mut app);
        assert!(app.variax().error.is_some());
        assert_eq!(fs::read(collision).unwrap(), b"Keep this");
        export(&ctx, &mut app);
        assert!(app.variax().error.is_none());

        let html = fs::read_to_string(output.with_extension("html")).unwrap();
        for label in [
            "Custom 1",
            "Custom 2",
            "Edited &lt;model&gt;",
            "Lester Goldtop example",
            "Standard (alternate tuning off)",
        ] {
            assert!(html.contains(label), "missing {label}");
        }
        let document = app.variax().document.as_ref().unwrap();
        assert_eq!(document.as_bytes(), before);
        assert!(document.is_dirty());
    }
}
