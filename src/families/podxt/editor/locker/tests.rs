use super::*;
use crate::editor::FakeDialogs;
use std::{cell::RefCell, rc::Rc};

#[derive(Default)]
struct Child {
    events: Vec<egui::Event>,
    closing: bool,
    size: Option<[f32; 2]>,
    frame: Option<(egui::ViewportId, egui::ViewportBuilder, egui::FullOutput)>,
}

pub(in crate::editor) struct HeadlessLocker {
    child: Rc<RefCell<Child>>,
}

impl HeadlessLocker {
    pub(in crate::editor) fn new(ctx: &egui::Context) -> Self {
        let child = Rc::new(RefCell::new(Child::default()));
        let render_child = Rc::clone(&child);
        ctx.set_embed_viewports(false);
        egui::Context::set_immediate_viewport_renderer(move |ctx, mut viewport| {
            let (events, closing, size) = {
                let mut child = render_child.borrow_mut();
                (
                    std::mem::take(&mut child.events),
                    std::mem::take(&mut child.closing),
                    child.size.unwrap_or(INITIAL_SIZE),
                )
            };
            let mut input = input(size, events);
            input.viewport_id = viewport.ids.this;
            let info = input.viewports.entry(viewport.ids.this).or_default();
            info.parent = Some(viewport.ids.parent);
            info.focused = Some(true);
            if closing {
                info.events.push(egui::ViewportEvent::Close);
            }
            let output = ctx.run(input, |ctx| (viewport.viewport_ui_cb)(ctx));
            render_child.borrow_mut().frame = Some((viewport.ids.this, viewport.builder, output));
        });
        Self { child }
    }

    pub(in crate::editor) fn render(
        &self,
        ctx: &egui::Context,
        app: &mut Workbench,
        root_events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        self.child.borrow_mut().frame = None;
        ctx.run(input([1280.0, 820.0], root_events), |ctx| app.render(ctx))
    }

    pub(in crate::editor) fn labels(&self) -> Vec<String> {
        let child = self.child.borrow();
        let (_, _, output) = child.frame.as_ref().expect("Tone Locker viewport");
        visible_text(output)
            .into_iter()
            .map(|(label, _)| label)
            .collect()
    }

    pub(in crate::editor) fn click(
        &self,
        ctx: &egui::Context,
        app: &mut Workbench,
        child: bool,
        label: &str,
    ) {
        let output = self.render(ctx, app, vec![]);
        let pos = if child {
            let child = self.child.borrow();
            let (_, _, output) = child.frame.as_ref().expect("Tone Locker viewport");
            label_position(output, label)
        } else {
            label_position(&output, label)
        };
        for pressed in [true, false] {
            let events = vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ];
            if child {
                self.child.borrow_mut().events = events;
                self.render(ctx, app, vec![]);
            } else {
                self.render(ctx, app, events);
            }
        }
        self.render(ctx, app, vec![]);
    }
}

fn input(size: [f32; 2], events: Vec<egui::Event>) -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size.into())),
        events,
        ..Default::default()
    }
}

fn key(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

fn visible_text(output: &egui::FullOutput) -> Vec<(String, egui::Pos2)> {
    fn visit(shape: &egui::Shape, clip: egui::Rect, labels: &mut Vec<(String, egui::Pos2)>) {
        match shape {
            egui::Shape::Text(text) => {
                let position = text.pos + text.galley.rect.center().to_vec2();
                if clip.contains(position) {
                    labels.push((text.galley.text().to_owned(), position));
                }
            }
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    visit(shape, clip, labels);
                }
            }
            _ => {}
        }
    }
    let mut labels = Vec::new();
    for shape in &output.shapes {
        visit(&shape.shape, shape.clip_rect, &mut labels);
    }
    labels
}

fn label_position(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
    let labels = visible_text(output);
    labels
        .iter()
        .find_map(|(text, pos)| (text == label).then_some(*pos))
        .unwrap_or_else(|| panic!("missing {label:?} from {labels:?}"))
}

fn backup_fixture() -> LoadedPodxt {
    use line6_devices::family::podxt::{EDIT_BUFFER_LEN, EditBuffer, StoredTone};
    let entries = BankKind::ALL
        .into_iter()
        .zip(["Saved tone", "Saved amp", "Saved FX"])
        .map(|(bank, name)| {
            let mut raw = vec![0; EDIT_BUFFER_LEN];
            raw[..7].copy_from_slice(&[0xf0, 0, 1, 0x0c, 3, 0x74, 0x0a]);
            raw[7..39].fill(b' ');
            raw[7..7 + name.len()].copy_from_slice(name.as_bytes());
            raw[EDIT_BUFFER_LEN - 1] = 0xf7;
            StoredTone {
                location: Location::new(bank, 0).unwrap(),
                tone: EditBuffer::parse(raw).unwrap(),
            }
        })
        .collect();
    LoadedPodxt {
        before: None,
        after: None,
        entries,
    }
}

#[test]
fn tone_locker_opens_by_keyboard_and_keeps_file_and_editor_state_across_close_routes() {
    let ctx = egui::Context::default();
    let mut app = Workbench::new(
        &ctx,
        EditorDefaults {
            artwork: PathBuf::new(),
            preferences: None,
            startup_panel: None,
        },
    );
    app.theme_polled = Instant::now();
    #[cfg(feature = "usb")]
    {
        app.detection = None;
    }
    app.new_tab(&super::super::FAMILY);
    app.podxt_mut().table_view_in_test = true;
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("local.l6t");
    let bytes = PodTone::new_podxt_tone("Local file").unwrap().into_bytes();
    fs::write(&path, &bytes).unwrap();
    app.dialogs = Box::new(FakeDialogs {
        folder: [Some(folder.path().to_path_buf())].into(),
        ..Default::default()
    });

    let locker = HeadlessLocker::new(&ctx);
    locker.child.borrow_mut().size = Some(MIN_SIZE);
    locker.render(&ctx, &mut app, vec![]);
    assert!(locker.child.borrow().frame.is_none());
    let toolbar = app.podxt().locker.toolbar_id.unwrap();
    ctx.memory_mut(|memory| memory.request_focus(toolbar));
    let root = locker.render(&ctx, &mut app, vec![key(egui::Key::Enter)]);
    label_position(&root, "Default Tone");
    assert!(
        !visible_text(&root)
            .iter()
            .any(|(text, _)| text == "Choose folder…")
    );

    let first_id = {
        let child = locker.child.borrow();
        let (id, builder, content) = child.frame.as_ref().expect("opens on the activation frame");
        assert_eq!(builder.resizable, Some(true));
        assert_eq!(builder.min_inner_size, Some(MIN_SIZE.into()));
        let files = label_position(content, "Files");
        let pod = label_position(content, "PODxt Live");
        assert!(files.x < pod.x);
        assert!((files.y - pod.y).abs() < 1.0, "{files:?} and {pod:?}");
        label_position(content, "Amp Setups · Not loaded");
        label_position(content, "FX Setups · Not loaded");
        *id
    };
    locker.child.borrow_mut().size = Some(INITIAL_SIZE);
    locker.click(&ctx, &mut app, true, "Read full library");
    assert!(locker.labels().contains(&"FX Setups · Not loaded".into()));
    locker.click(&ctx, &mut app, true, "Choose folder…");
    locker.click(&ctx, &mut app, true, "local.l6t");
    locker.click(&ctx, &mut app, true, "Copy file tone");
    locker.click(&ctx, &mut app, true, "Open in editor");
    assert_eq!(app.podxt().source_path(), Some(path.as_path()));
    app.podxt_mut().pod_name = "Unsaved edit".into();
    app.podxt_mut().apply_pod_name();
    let edited = app.podxt().pod_library.tone(0).unwrap().as_bytes().to_vec();
    assert_ne!(edited, bytes);

    locker.click(&ctx, &mut app, true, "Close");
    assert!(!app.podxt().locker.open);
    locker.render(&ctx, &mut app, vec![]);
    assert!(locker.child.borrow().frame.is_none());
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(toolbar));

    let root = locker.render(&ctx, &mut app, vec![key(egui::Key::Space)]);
    label_position(&root, "Unsaved edit");
    assert!(locker.labels().contains(&"Clipboard: Local file".into()));
    assert!(locker.labels().contains(&"Open in editor".into()));
    assert_eq!(locker.child.borrow().frame.as_ref().unwrap().0, first_id);

    locker.child.borrow_mut().events = vec![key(egui::Key::Escape)];
    locker.render(&ctx, &mut app, vec![]);
    assert!(!app.podxt().locker.open);
    locker.render(&ctx, &mut app, vec![key(egui::Key::Enter)]);
    locker.child.borrow_mut().closing = true;
    locker.render(&ctx, &mut app, vec![]);
    assert!(!app.podxt().locker.open);
    assert_eq!(app.podxt().pod_library.tone(0).unwrap().as_bytes(), edited);
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(fs::read_dir(folder.path()).unwrap().count(), 1);
}

#[test]
fn tone_locker_browses_imported_amp_and_fx_setups_offline() {
    let ctx = egui::Context::default();
    let mut app = Workbench::new(
        &ctx,
        EditorDefaults {
            artwork: PathBuf::new(),
            preferences: None,
            startup_panel: None,
        },
    );
    app.theme_polled = Instant::now();
    #[cfg(feature = "usb")]
    {
        app.detection = None;
    }
    app.new_tab(&super::super::FAMILY);
    app.podxt_mut().table_view_in_test = true;
    app.podxt_mut().stage.backup = Some(backup_fixture());
    let locker = HeadlessLocker::new(&ctx);

    locker.click(&ctx, &mut app, false, "Tone Locker");
    let labels = locker.labels();
    assert!(labels.contains(&"Source: imported library backup".into()));
    #[cfg(not(feature = "usb"))]
    assert!(labels.contains(&"Device actions are unavailable in this build.".into()));
    assert!(labels.contains(&"Tones · 1/128 loaded".into()));
    locker.click(&ctx, &mut app, true, "Amp Setups · 1/107 loaded");
    let labels = locker.labels();
    assert!(
        labels.iter().any(|label| label.ends_with("· Saved amp")),
        "{labels:?}"
    );
    locker.click(&ctx, &mut app, true, "Amp Setups · 1/107 loaded");
    locker.click(&ctx, &mut app, true, "FX Setups · 1/64 loaded");
    let slot = Location::new(BankKind::FxSetups, 0).unwrap();
    locker.click(
        &ctx,
        &mut app,
        true,
        &format!("{} · Saved FX", slot.slot_name()),
    );
    locker.click(&ctx, &mut app, true, "Open backup copy in editor");
    let editor = app.podxt();
    let tone = editor.pod_library.tone(0).unwrap();
    assert_eq!(tone.container(), ContainerKind::L6fs);
    assert_eq!(tone.name(), "Saved FX");
    assert!(editor.source_path().is_none());
}
