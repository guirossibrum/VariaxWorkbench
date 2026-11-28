#[cfg(test)]
use super::tuning_presets::DETUNE;
use super::tuning_presets::{MIX, PITCH, TUNINGS, TuningDialog, set_tuning_values};
use super::{
    BODY, Color32, FRET_MARKER, GUITAR_SURFACE, Pos2, Rect, SYSTEM, Sense, TUNING, TextureHandle,
    ValueFormat, VariaxEditor, egui,
};
#[cfg(test)]
use super::{EditorDocument, PICKUP1, PICKUP2, PresetFile, Tab, WIRING};
use std::{collections::HashSet, path::Path, process::Command};
#[cfg(test)]
use variax_workbench::bundle::BundleFile;
use variax_workbench::{
    preset::PresetText,
    ui::{
        theme,
        tuning_panel::{TuningEvent, TuningPanel, TuningRow, semitone_at},
    },
};

#[cfg(test)]
const SPREAD: u32 = 0x200012;

pub(super) const STRINGS: [(&str, i32); 6] = [
    ("Low E", 4),
    ("A", 9),
    ("D", 2),
    ("G", 7),
    ("B", 11),
    ("High E", 4),
];
const NOTES: [&str; 12] = [
    "C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B",
];

pub(super) fn tuning_note(string: usize, offset: i32) -> Option<&'static str> {
    let (_, pitch_class) = *STRINGS.get(string)?;
    (-12..=12)
        .contains(&offset)
        .then(|| NOTES[(pitch_class + offset).rem_euclid(12) as usize])
}

pub(in crate::editor) fn milestones(ui: &mut egui::Ui) {
    show_source_checkout(ui);
    ui.separator();
    ui.heading("Complete · Offline Variax 600 editing");
    ui.label(
        "Open a preset, bank or collection, or try the built-in example banks. Edit pickups, \
         controls or tuning, then choose Save or Save As….",
    );
    ui.separator();
    ui.heading("Offline PODxt Live tones");
    ui.label(
        "Open the PODxt Live tab. Bank Down and Bank Up move through the loaded banks, \
         then A, B, C, and D choose a channel. Amp, Stomp, Mod, and Delay open that block. \
         On/Bypass changes only the stored flag for the block you selected. Practice loads \
         Amp and Effects Tone or Stompbox Collection. Load tone or bank… opens your files. \
         Save As… writes a new file and leaves the original alone. Revert tells you what \
         it will put back before you discard it. Fetch from PODxt Live, Send current bank, \
         and Send all banks stay off.",
    );
    ui.separator();
    ui.heading("Current phase · tone checks are in Device check");
    ui.label(
        "Device check… opens in Simulation. No is the default. Step 2 is offered only after \
         a fresh step 1 pass for one PODxt Live with the POD driver on interface 0 and the \
         Variax driver on interface 1. The Yes button for that step says it sends one \
         Variax identity request. In Simulation the button rehearses the request and nothing \
         is sent. The rehearsal accepts only the Variax 600 answer, keeps listening for the \
         rest of the three seconds, and a second answer does not pass. That one request is \
         the app's. After the driver is given back, Linux's own snd-usb-variax startup can \
         send the same kind of question. That later traffic is Linux, not a second request \
         from this app. Close stops a step before it starts. Once Yes has started, this \
         window cannot cancel the exchange. This computer offers steps 1, 2, and 3. \
         A build without USB still offers only step 1. Step 3 reads the Variax and the \
         PODxt Live. Step 4 stays off.",
    );
    ui.label(
        "Build 20 is this copy. Auto-select control values is on: choosing a humbucker \
         sets the volume and tone to 500 kΩ and 47 nF, and other pickups set 250 kΩ and \
         22 nF. Turning it off leaves those knobs alone, and the choice is remembered. \
         A model name or author uses at most 18 letters, numbers, \
         spaces, and keyboard symbols. A saved name that is longer or uses other characters \
         stays as it is and is marked. Turning a pickup off keeps the other pickup on, and Front \
         draws that pickup on top of the other. On This computer, Device check can try a PODxt Live tone \
         change and put it back, and it can play a saved tone. The try changes treble, mid, \
         one EQ band, and the modulation, delay and reverb switches, then puts the original \
         values back. The stomp is not changed. Play a saved tone switches the PODxt the way \
         the footswitch does, and only when the current tone already matches a saved one. \
         Leaving a change on stays off. Nothing is saved. Back up the Variax still reads all \
         60 presets and tries to put the original preset back. Back up the PODxt Live still \
         reads the stored tones and does not change the current tone on purpose. Backup files \
         are ordinary files you can move or delete. Step 4 stays off. No is still the default.",
    );
    for (label, url) in [
        (
            "G1 procedure and authorization record (#131)",
            "https://github.com/guirossibrum/VariaxWorkbench/issues/131",
        ),
        (
            "Your G1 test (#125)",
            "https://github.com/guirossibrum/VariaxWorkbench/issues/125",
        ),
        (
            "Phase 2 exit review (#149)",
            "https://github.com/guirossibrum/VariaxWorkbench/issues/149",
        ),
        (
            "Phase 3 exit review (#150)",
            "https://github.com/guirossibrum/VariaxWorkbench/issues/150",
        ),
    ] {
        ui.hyperlink_to(label, url);
    }
    ui.separator();
    ui.heading("Later phases · before sound checks and device writes");
    ui.label(
        "These do not block USB recognition, identification or settings reads (delivery reset, \
         2026-11-20).",
    );
    for (label, url, status) in [
        (
            "Compare complete presets with original Workbench (#6)",
            "https://github.com/guirossibrum/VariaxWorkbench/issues/6",
            "Open. Broader matching across models, wiring and tuning is pending.",
        ),
        (
            "Expand tests for custom tunings, bends and sustain (#33)",
            "https://github.com/guirossibrum/VariaxWorkbench/issues/33",
            "Open. Audio checks need reference recordings tracked in #128.",
        ),
        (
            "Review the offline results and remaining gaps (#22)",
            "https://github.com/guirossibrum/VariaxWorkbench/issues/22",
            "Open. The write review waits for validation and backups.",
        ),
        (
            "Check the original Workbench calculation reference (#5)",
            "https://github.com/guirossibrum/VariaxWorkbench/issues/5",
            "Open. A rounding difference is documented. Further research follows the first rig milestone.",
        ),
    ] {
        ui.hyperlink_to(label, url);
        ui.label(status);
        ui.add_space(4.0);
    }
    ui.label(
        "Leave the guitar and PODxt Live unplugged except when you follow the owner card for \
         the one identity check. Any later hardware test needs your approval.",
    );
    ui.separator();
    ui.heading("Project status and issue closeout");
    ui.label(
        "Maintainers can use the GitHub command-line tool (gh) from a terminal that is already \
         logged in to GitHub to push branches and open pull requests. Close issues after all \
         work is merged and required tests pass. Link the merge and test results on the issue.",
    );
    ui.label(
        "Which work statuses can be verified from this checkout and its locally saved upstream \
         history, and which require the authoritative GitHub project board?",
    );
    ui.label(
        "The README and local Git history show what was tested or merged. GitHub tracks current \
         priorities, assignments, and issue status.",
    );
    ui.horizontal(|ui| {
        ui.hyperlink_to(
            "Project board",
            "https://github.com/users/guirossibrum/projects/17",
        );
        ui.hyperlink_to(
            "All issues",
            "https://github.com/guirossibrum/VariaxWorkbench/issues",
        );
        ui.hyperlink_to(
            "Pull requests",
            "https://github.com/guirossibrum/VariaxWorkbench/pulls",
        );
    });
}

fn show_source_checkout(ui: &mut egui::Ui) {
    let checkout = Path::new(env!("CARGO_MANIFEST_DIR"));
    ui.heading("Git repository used by this build");
    ui.label("Local folder");
    ui.monospace(checkout.display().to_string());
    if !checkout.join(".git").exists() {
        ui.label("The source checkout is unavailable on this computer.");
        return;
    }
    let status_id = ui.id().with("source-checkout-status");
    let refresh = ui.button("Refresh repository details").clicked();
    let status = ui
        .data(|data| data.get_temp::<Option<SourceCheckout>>(status_id))
        .filter(|_| !refresh)
        .unwrap_or_else(|| {
            let status = SourceCheckout::read(checkout);
            ui.data_mut(|data| data.insert_temp(status_id, status.clone()));
            status
        });
    let Some(status) = status else {
        ui.label("The source checkout status could not be read.");
        return;
    };
    ui.horizontal_wrapped(|ui| {
        ui.label("Current branch");
        ui.monospace(status.branch.as_str());
    });
    ui.label("Configured remote (origin)");
    if let Some(origin) = &status.origin {
        ui.monospace(origin.as_str());
    } else {
        ui.weak("No origin found for this folder.");
    }
    if status.commit == "(initial)" {
        ui.label("This checkout has no commits yet.");
    } else {
        ui.label("Last local commit");
        let short_commit = status.commit.get(..7).unwrap_or(&status.commit);
        ui.monospace(short_commit)
            .on_hover_text(status.commit.as_str());
    }
    let file_label = if status.changed_files == 1 {
        "file"
    } else {
        "files"
    };
    ui.label(format!("{} uncommitted {file_label}", status.changed_files));
}

#[derive(Clone)]
struct SourceCheckout {
    commit: String,
    branch: String,
    origin: Option<String>,
    changed_files: usize,
}

impl SourceCheckout {
    fn read(checkout: &Path) -> Option<Self> {
        let git = || {
            let mut command = Command::new("git");
            command.args(["--no-optional-locks", "-C"]).arg(checkout);
            command
        };
        let output = git()
            .args([
                "status",
                "--porcelain=v2",
                "--branch",
                "--no-ahead-behind",
                "--untracked-files=all",
            ])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let mut status = Self::parse_status(&String::from_utf8_lossy(&output.stdout))?;
        status.origin = git()
            .args(["config", "--local", "--get", "remote.origin.url"])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| readable_origin(String::from_utf8_lossy(&output.stdout).trim()))
            .filter(|origin| !origin.is_empty());
        Some(status)
    }

    fn parse_status(status: &str) -> Option<Self> {
        let commit = status
            .lines()
            .find_map(|line| line.strip_prefix("# branch.oid "))?
            .to_owned();
        let branch = status
            .lines()
            .find_map(|line| line.strip_prefix("# branch.head "))?
            .to_owned();
        // Porcelain v2 quotes filenames containing line breaks and emits one row per change.
        let changed_files = status.lines().filter(|line| !line.starts_with('#')).count();
        Some(Self {
            commit,
            branch,
            origin: None,
            changed_files,
        })
    }
}

fn readable_origin(origin: &str) -> String {
    if let Some((scheme, rest)) = origin.split_once("://") {
        // Remote URLs can contain credentials in the user info, query, or fragment.
        let rest = rest.split(['?', '#']).next().unwrap_or(rest);
        let (authority, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
        let host = authority.rsplit('@').next().unwrap_or(authority);
        return format!("{scheme}://{host}{path}");
    }
    origin.to_owned()
}

enum SlotCommand {
    Select,
    Rename,
    Copy,
    Paste,
    #[cfg(feature = "usb")]
    Device(super::device_sync::Action),
}

impl VariaxEditor {
    #[cfg(test)]
    pub(in crate::editor) fn open_example(&mut self) {
        match example_document() {
            Ok(document) => {
                self.adopt_document(document, None);
                self.drag_origin = None;
                self.clear_text_drafts();
                self.tab = Tab::Pickups;
                self.status = "Opened the built-in example banks.".into();
                self.error = None;
            }
            Err(error) => self.error = Some(format!("Example banks: {error}")),
        }
    }

    pub(super) fn guitar_header(&mut self, ui: &mut egui::Ui, available: bool) -> bool {
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(9, 4))
            .corner_radius(theme::RADIUS_PANEL)
            .fill(Color32::from_rgb(69, 43, 29))
            .stroke(egui::Stroke::new(1.0_f32, Color32::from_rgb(116, 78, 48)))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 3.0;
                let import = self.bank_strip(ui, available);
                ui.spacing_mut().interact_size.y = 22.0;
                ui.spacing_mut().button_padding.y = 2.0;
                self.model_details(ui, available);
                import
            })
            .inner
    }

    fn bank_strip(&mut self, ui: &mut egui::Ui, available: bool) -> bool {
        #[cfg(test)]
        self.bank_slots.clear();
        let artwork_paths = self
            .document
            .as_ref()
            .and_then(|document| {
                let (bank, _) = document.selection()?;
                Some(
                    document.banks()?[bank]
                        .presets()
                        .iter()
                        .map(|preset| preset.model(BODY).and_then(|body| body_thumbnail(body.id)))
                        .collect::<Vec<_>>(),
                )
            })
            .unwrap_or_default();
        let artwork: Vec<_> = artwork_paths
            .into_iter()
            .map(|path| path.and_then(|path| self.texture(ui.ctx(), &path)))
            .collect();
        #[cfg(feature = "usb")]
        let device_commands = {
            use super::device_sync::Action;
            [
                (
                    "Write to device",
                    Action::WriteSelected,
                    self.sync_ready(Action::WriteSelected),
                ),
                (
                    "Read unmodified from device",
                    Action::ReadUnmodified,
                    self.sync_ready(Action::ReadUnmodified),
                ),
            ]
        };
        let document = self
            .document
            .as_ref()
            .expect("guitar header requires a document");
        let guitar_library = self
            .library_origin
            .as_ref()
            .is_some_and(super::LibraryOrigin::is_guitar_library);
        let example = cfg!(test) && document.source.as_os_str().is_empty();
        let suffix = if example { " (example)" } else { "" };
        let mut next = document.selection();
        let mut import = false;
        let mut command = None;
        ui.horizontal_wrapped(|ui| {
            let unsaved = egui::Label::new(egui::RichText::new("Unsaved").color(FRET_MARKER));
            if let (Some(banks), Some((bank_index, selected))) = (document.banks(), next) {
                let bank = &banks[bank_index];
                ui.add_enabled_ui(available, |ui| {
                    let dropdown = egui::ComboBox::from_id_salt("editing-bank")
                        .selected_text(format!("Editing bank: {}{suffix}", bank.name))
                        .width(if example { 204.0 } else { 174.0 })
                        .show_ui(ui, |ui| {
                            #[cfg(test)]
                            self.bank_options.clear();
                            let mut names = HashSet::new();
                            let repeated_names: HashSet<_> = banks
                                .iter()
                                .map(|bank| bank.name.as_str())
                                .filter(|name| !names.insert(*name))
                                .collect();
                            if self.preferences_path.is_some() {
                                if ui.button("Open library backup…").clicked() {
                                    import = true;
                                    ui.close();
                                }
                                ui.separator();
                            }
                            for (index, entry) in banks.iter().enumerate() {
                                let count = entry.presets().len();
                                let title = if entry.name.is_empty() {
                                    format!("Bank {}", index + 1)
                                } else if repeated_names.contains(entry.name.as_str()) {
                                    format!("{} ({})", entry.name, index + 1)
                                } else {
                                    entry.name.clone()
                                };
                                let response = ui.add_enabled(
                                    count != 0,
                                    egui::Button::selectable(
                                        index == bank_index,
                                        format!("{title}{suffix}"),
                                    ),
                                );
                                #[cfg(test)]
                                self.bank_options.insert(index, response.rect);
                                if response.clicked() {
                                    if index != bank_index {
                                        next = Some((index, selected.min(count - 1)));
                                    }
                                    ui.close();
                                }
                            }
                        });
                    #[cfg(test)]
                    {
                        self.bank_dropdown = Some(dropdown.response.rect);
                    }
                    dropdown.response.on_hover_text(if example {
                        "These are built-in examples. Use Open to edit a saved bank file."
                    } else {
                        "Choose a bank or folder from this file"
                    });
                    for (index, (preset, thumbnail)) in
                        bank.presets().iter().zip(&artwork).enumerate()
                    {
                        let active = index == selected;
                        let changed = document.slot_is_dirty(bank_index, index);
                        let response = bank_slot(
                            ui,
                            index + 1,
                            preset.name(),
                            thumbnail.as_ref(),
                            active,
                            changed,
                            document.slot_is_unsent(bank_index, index),
                        );
                        #[cfg(test)]
                        self.bank_slots.insert(index, response.rect);
                        if response.clicked() && !active {
                            next = Some((bank_index, index));
                        }
                        response.context_menu(|ui| {
                            use variax_workbench::document::SlotBuffer;
                            for (label, action, enabled) in [
                                ("Edit", SlotCommand::Select, available),
                                ("Load", SlotCommand::Select, available),
                                ("Rename", SlotCommand::Rename, available),
                                ("", SlotCommand::Select, false),
                                ("Copy", SlotCommand::Copy, available),
                                (
                                    "Paste",
                                    SlotCommand::Paste,
                                    available
                                        && document
                                            .slot_buffer_label(SlotBuffer::ModelClipboard)
                                            .is_some(),
                                ),
                            ] {
                                if label.is_empty() {
                                    ui.separator();
                                } else if ui
                                    .add_enabled(enabled, egui::Button::new(label))
                                    .clicked()
                                {
                                    command = Some(((bank_index, index), action));
                                    ui.close();
                                }
                            }
                            #[cfg(feature = "usb")]
                            {
                                ui.separator();
                                for (label, action, ready) in device_commands {
                                    if ui
                                        .add_enabled(available && ready, egui::Button::new(label))
                                        .clicked()
                                    {
                                        command = Some((
                                            (bank_index, index),
                                            SlotCommand::Device(action),
                                        ));
                                        ui.close();
                                    }
                                }
                            }
                        });
                    }
                });
                let pending_store = guitar_library && document.slot_is_unsent(bank_index, selected);
                if document.is_dirty() || pending_store {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        if pending_store {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new("Modified · not stored on guitar").small(),
                                )
                                .extend(),
                            );
                        }
                        if document.is_dirty() {
                            ui.add(unsaved);
                        }
                    });
                }
            } else {
                if let Some(path) = &self.source_path
                    && let Some(filename) = path.file_name()
                {
                    ui.strong(format!(
                        "Editing “{}” [{}]",
                        document.preset().name(),
                        filename.to_string_lossy()
                    ));
                } else {
                    ui.strong("Individual preset");
                }
                if document.is_dirty() {
                    ui.add(unsaved);
                }
            }
        });

        if let Some((position, SlotCommand::Select | SlotCommand::Rename)) = &command {
            next = Some(*position);
        }
        let switched = if next != document.selection()
            && let Some((bank, slot)) = next
            && let Some(document) = &mut self.document
        {
            match document.select(bank, slot) {
                Ok(()) => {
                    let name = document.banks().expect("selected bank")[bank].name.clone();
                    Some(Ok(format!(
                        "Editing {name}{suffix}, slot {} offline.",
                        slot + 1
                    )))
                }
                Err(error) => Some(Err(error)),
            }
        } else {
            None
        };
        match switched {
            Some(Ok(status)) => {
                self.drag_origin = None;
                self.clear_text_drafts();
                self.error = None;
                self.status = status;
                #[cfg(feature = "usb")]
                if !matches!(command, Some((_, SlotCommand::Rename))) {
                    self.live_select_explicit();
                } else {
                    self.live_selection_followed();
                }
            }
            Some(Err(error)) => self.error = Some(error),
            None => {}
        }
        if let Some((position, command)) = command {
            use variax_workbench::document::SlotBuffer;
            match command {
                SlotCommand::Select => {}
                SlotCommand::Rename => {
                    ui.memory_mut(|memory| memory.request_focus(egui::Id::new("variax-model-name")))
                }
                SlotCommand::Copy | SlotCommand::Paste => {
                    let document = self.document.as_mut().expect("open library");
                    let (bank, slot) = document.selection().expect("selected model");
                    let result = document.select(position.0, position.1).and_then(|()| {
                        let operation = if matches!(command, SlotCommand::Copy) {
                            document.copy_to_slot_buffer(SlotBuffer::ModelClipboard)
                        } else if guitar_library {
                            document.paste_into_guitar_position()
                        } else {
                            document.paste_from_slot_buffer(SlotBuffer::ModelClipboard, false)
                        };
                        let restored = document.select(bank, slot);
                        operation.and(restored)
                    });
                    match result {
                        Ok(()) => {
                            self.error = None;
                            if matches!(command, SlotCommand::Copy) {
                                self.status = "Copied model.".into();
                            } else {
                                self.clear_text_drafts();
                                #[cfg(feature = "usb")]
                                if position == (bank, slot) {
                                    self.live_paste_completed();
                                }
                                self.status = "Pasted model.".into();
                            }
                        }
                        Err(error) => self.error = Some(error),
                    }
                }
                #[cfg(feature = "usb")]
                SlotCommand::Device(action) => self.confirm_position_action(action, position),
            }
        }
        import
    }

    fn model_details(&mut self, ui: &mut egui::Ui, available: bool) {
        const OUTPUT_TRIM: u32 = 0x200000;
        const TRIM_WIDTH: f32 = 240.0;
        const TRIM_RECOVERY_WIDTH: f32 = 300.0;
        let trim_width = |editor: &Self| {
            if editor
                .float(SYSTEM, OUTPUT_TRIM)
                .is_some_and(|value| !value.is_finite())
            {
                TRIM_RECOVERY_WIDTH
            } else {
                TRIM_WIDTH
            }
        };
        let (model_width, author_width) = if ui.available_width() < 1000.0 {
            (144.0, 100.0)
        } else {
            (168.0, 120.0)
        };
        let document = self
            .document
            .as_ref()
            .expect("model details require a document");
        let editable = available && matches!(document.preset().device_id(), 0x70000 | 0x70004);
        let can_undo = editable && document.can_undo();
        let can_redo = editable && document.can_redo();
        let stored_name = document.preset().name().to_owned();
        let mut name = self
            .name_draft
            .clone()
            .unwrap_or_else(|| stored_name.clone());
        #[cfg(test)]
        self.name_notice.clear();

        let (author, stored_author) = ui
            .horizontal_wrapped(|ui| {
                ui.horizontal(|ui| {
                    ui.add_sized([42.0, 17.0], egui::Label::new("Model"));
                    let response = ui.add_enabled(
                        editable,
                        egui::TextEdit::singleline(&mut name)
                            .id(egui::Id::new("variax-model-name"))
                            .desired_width(model_width)
                            .hint_text("Model name"),
                    );
                    self.limited_text(response, &stored_name, name.clone(), true);
                });
                ui.horizontal(|ui| {
                    ui.label("Author");
                    self.metadata_field(ui, editable, PresetText::Author, author_width);
                });
                let stored_author = self
                    .document
                    .as_ref()
                    .and_then(|document| document.preset().metadata(PresetText::Author).ok())
                    .unwrap_or_default();
                let author = self
                    .author_draft
                    .clone()
                    .unwrap_or_else(|| stored_author.clone());
                ui.horizontal(|ui| {
                    ui.label("Notes");
                    // Leave room after Notes for the history buttons and trim.
                    let notes_width =
                        (ui.available_width() - trim_width(self) - 160.0).clamp(96.0, 240.0);
                    self.metadata_field(ui, editable, PresetText::Notes, notes_width);
                });
                if ui
                    .add_enabled(can_undo, egui::Button::new("Undo"))
                    .clicked()
                {
                    self.undo();
                }
                if ui
                    .add_enabled(can_redo, egui::Button::new("Redo"))
                    .clicked()
                {
                    self.redo();
                }
                if self.can_edit(SYSTEM, Some(OUTPUT_TRIM)) {
                    // Keep the trim control together, including the wider recovery action.
                    ui.allocate_ui_with_layout(
                        egui::vec2(trim_width(self), ui.spacing().interact_size.y),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.add_enabled_ui(editable, |ui| {
                                ui.spacing_mut().slider_width = 70.0;
                                self.float_control(
                                    ui,
                                    SYSTEM,
                                    OUTPUT_TRIM,
                                    "Output trim",
                                    ValueFormat::TRIM,
                                );
                            });
                        },
                    );
                }
                (author, stored_author)
            })
            .inner;
        // A long notice gets the header width without widening either text field.
        self.name_flag(ui, "Model", &name, name == stored_name);
        self.name_flag(ui, "Author", &author, author == stored_author);
    }

    fn metadata_field(&mut self, ui: &mut egui::Ui, editable: bool, field: PresetText, width: f32) {
        let stored = match self
            .document
            .as_ref()
            .expect("metadata fields require a document")
            .preset()
            .metadata(field)
        {
            Ok(stored) => stored,
            Err(error) => {
                ui.weak("Unavailable").on_hover_text(error);
                return;
            }
        };
        let mut value = if field == PresetText::Author {
            self.author_draft.clone().unwrap_or_else(|| stored.clone())
        } else {
            stored.clone()
        };
        let widget = match field {
            PresetText::Author => egui::TextEdit::singleline(&mut value),
            PresetText::Notes => egui::TextEdit::multiline(&mut value).desired_rows(1),
        };
        let response = ui.add_enabled(editable, widget.desired_width(width));
        if field == PresetText::Author {
            self.limited_text(response, &stored, value, false);
        } else if response.changed() {
            self.change(|preset| preset.set_metadata(field, &value));
        }
    }

    fn name_flag(&mut self, ui: &mut egui::Ui, label: &str, value: &str, already_stored: bool) {
        let limits = self
            .document
            .as_ref()
            .is_some_and(|document| document.preset().limits_legacy_name());
        if !limits {
            return;
        }
        let Some(issue) = variax_workbench::preset::legacy_text_issue(value, already_stored) else {
            return;
        };
        let line = format!("{label}: {issue}");
        ui.add(
            egui::Label::new(
                egui::RichText::new(&line)
                    .small()
                    .color(Color32::from_rgb(214, 168, 72)),
            )
            .wrap(),
        );
        #[cfg(test)]
        self.name_notice.push_str(&line);
    }

    /// Keep an unfinished name or author on screen without writing the file.
    /// A finished 18-character keyboard name is one undo step.
    fn limited_text(&mut self, response: egui::Response, stored: &str, edited: String, name: bool) {
        let limits = self
            .document
            .as_ref()
            .is_some_and(|document| document.preset().limits_legacy_name());
        if !limits {
            if response.changed() && edited != stored {
                if name {
                    self.change(|preset| preset.rename(&edited));
                } else {
                    self.change(|preset| preset.set_metadata(PresetText::Author, &edited));
                }
            }
            return;
        }
        #[cfg(feature = "usb")]
        if self.refresh.busy() {
            return;
        }
        let invalid = variax_workbench::preset::legacy_text_issue(&edited, false).is_some();
        if !response.changed() {
            if response.lost_focus() {
                self.clear_one_draft(name);
            }
            return;
        }
        if self.is_reading() || self.pending.is_some() {
            return;
        }
        if !invalid {
            self.clear_one_draft(name);
            if edited != stored {
                if name {
                    self.change(|preset| preset.rename(&edited));
                } else {
                    self.change(|preset| preset.set_metadata(PresetText::Author, &edited));
                }
            }
        } else if response.lost_focus() {
            self.clear_one_draft(name);
        } else if name {
            self.name_draft = Some(edited);
        } else {
            self.author_draft = Some(edited);
        }
    }

    fn clear_one_draft(&mut self, name: bool) {
        if name {
            self.name_draft = None;
        } else {
            self.author_draft = None;
        }
    }

    pub(super) fn has_twelve_string(&self) -> bool {
        self.can_edit(TUNING, Some(MIX))
    }

    #[cfg(test)]
    pub(super) fn test_fretboard(&mut self, ui: &mut egui::Ui, can_edit: bool) {
        let width =
            variax_workbench::ui::editor_layout::twelve_string_column(self.has_twelve_string());
        self.fretboard(ui, can_edit, width);
    }

    pub(super) fn fretboard(&mut self, ui: &mut egui::Ui, can_edit: bool, twelve_width: f32) {
        let tuning = self
            .document
            .as_ref()
            .and_then(|d| d.preset().model(TUNING));
        let enabled = tuning.is_some_and(|model| model.enabled);
        let can_edit = can_edit && tuning.is_some() && self.can_edit(TUNING, None);
        let complete_pitch = can_edit && (PITCH..PITCH + 6).all(|id| self.tuning_parameter(id, 0));
        let twelve_string = twelve_width > 0.0;
        // Settle the panel's measured height and clipping on the first frame
        // and when a model adds or removes the secondary-string controls.
        let show_off = tuning.is_some() && !enabled;
        let layout_id = ui.id().with("tuning-panel-layout");
        let remeasure = ui.data_mut(|cache| {
            if cache.get_temp::<bool>(layout_id) == Some(twelve_string) {
                return false;
            }
            cache.insert_temp(layout_id, twelve_string);
            true
        });
        if remeasure {
            ui.ctx().request_discard("measure Variax tuning panel");
        }
        let complete_mix = can_edit && (MIX..MIX + 6).all(|id| self.tuning_parameter(id, 1));
        let offsets =
            std::array::from_fn::<_, 6, _>(|index| self.integer(TUNING, PITCH + index as u32));
        let labels = offsets.map(|offset| {
            offset.map_or("—".into(), |shift| {
                ValueFormat::SEMITONES.display(f64::from(shift))
            })
        });
        let rows = std::array::from_fn::<_, 6, _>(|row| {
            let id = 5 - row;
            TuningRow {
                id,
                name: STRINGS[id].0,
                shift: offsets[id].unwrap_or(0),
                shift_label: &labels[id],
                note: offsets[id]
                    .and_then(|offset| tuning_note(id, offset))
                    .unwrap_or("—"),
            }
        });
        let panel = TuningPanel::new(&rows)
            .enabled(enabled)
            .interactive(can_edit)
            .lock(complete_mix, self.link_tuning_mix)
            .presets_enabled(complete_pitch)
            .transpose_enabled(complete_pitch)
            .reset_enabled(complete_pitch);
        ui.add_visible(
            show_off,
            egui::Label::new("Alternate tuning is off: these offsets are saved but not played."),
        );
        #[cfg(feature = "usb")]
        {
            let label = self.follow.tuning_label();
            if self.follow.active() || label.is_some() {
                ui.add_visible(
                    label.is_some(),
                    egui::Label::new(
                        egui::RichText::new(label.unwrap_or("Guitar: alternate tuning off")).weak(),
                    ),
                );
            }
        }
        #[cfg(test)]
        self.tuning_widgets.clear();
        let output = ui
            .horizontal_top(|ui| {
                let left = ui.available_rect_before_wrap().left();
                if twelve_string {
                    ui.add_space(twelve_width);
                }
                let output = panel.show(ui);
                if twelve_string {
                    self.twelve_string_panel(ui, left, &output, can_edit, complete_mix);
                }
                output
            })
            .inner;
        if show_off {
            let [r, g, b, _] = theme::PANEL.to_array();
            let dim = Color32::from_rgba_unmultiplied(r, g, b, 96);
            if let (Some(first), Some(last)) = (output.rows.first(), output.rows.last()) {
                let band = Rect::from_min_max(
                    Pos2::new(output.enable.left(), first.lane.top()),
                    Pos2::new(output.neck.right(), last.lane.bottom()),
                );
                ui.painter().rect_filled(band, 0, dim);
            }
        }
        paint_missing_tuning_symbols(ui, output.flat, output.sharp, complete_pitch);
        #[cfg(test)]
        {
            self.tuning_toggle = Some(output.enable);
            self.tuning_widgets.insert("Neck", output.neck);
            for (label, rect) in [
                ("Presets", output.presets),
                ("Flat", output.flat),
                ("Sharp", output.sharp),
                ("Lock", output.lock),
                ("Reset", output.reset),
            ] {
                self.tuning_widgets.insert(label, rect);
            }
            if let Some(low) = output.rows.iter().find(|row| row.id == 0) {
                self.tuning_widgets.insert("Low E shift", low.shift_cell);
            }
        }
        let presets_clicked = output.events.contains(&TuningEvent::Presets);
        self.tuning_preset_menu(ui, output.presets, complete_pitch, presets_clicked);
        for event in output.events {
            match event {
                TuningEvent::Enable => self.change(|preset| preset.set_enabled(TUNING, !enabled)),
                TuningEvent::Lock(value) => self.link_tuning_mix = value,
                TuningEvent::Reset => self.set_tuning([0; 6], false),
                TuningEvent::FlattenAll => self.transpose_tuning(-1),
                TuningEvent::SharpenAll => self.transpose_tuning(1),
                TuningEvent::Shift { id, semitones } => self.set_string_shift(id, semitones),
                TuningEvent::Mix { .. } | TuningEvent::Detune { .. } => {}
                TuningEvent::DragStarted { id } => {
                    if self.tuning_parameter(PITCH + id as u32, 0)
                        && let Some(document) = &mut self.document
                    {
                        document.begin_gesture();
                    }
                }
                TuningEvent::DragFinished { id } => {
                    if can_edit
                        && let Some(pointer) = ui.input(|input| {
                            input
                                .pointer
                                .button_released(egui::PointerButton::Primary)
                                .then(|| input.pointer.interact_pos())
                                .flatten()
                        })
                    {
                        self.set_string_shift(id, semitone_at(output.neck, pointer.x));
                    }
                    if let Some(document) = &mut self.document {
                        document.end_gesture();
                    }
                }
                TuningEvent::Presets => {}
            }
        }
    }

    fn set_string_shift(&mut self, string: usize, semitones: i32) {
        let id = PITCH + string as u32;
        if self.tuning_parameter(id, 0) && self.integer(TUNING, id) != Some(semitones) {
            self.change(|preset| preset.set_parameter(TUNING, id, 0, semitones as u32));
        }
    }

    fn tuning_preset_menu(
        &mut self,
        ui: &mut egui::Ui,
        anchor: Rect,
        enabled: bool,
        clicked: bool,
    ) {
        use egui::containers::SetOpenCommand;
        let command = if !enabled {
            Some(SetOpenCommand::Bool(false))
        } else {
            clicked.then_some(SetOpenCommand::Toggle)
        };
        egui::Popup::new(
            ui.id().with("tuning-presets"),
            ui.ctx().clone(),
            anchor,
            ui.layer_id(),
        )
        .kind(egui::PopupKind::Menu)
        .open_memory(command)
        .show(|ui| {
            let manage = ui.button("Manage tuning presets…");
            #[cfg(test)]
            self.tuning_widgets
                .insert("Manage tuning presets…", manage.rect);
            if manage.clicked() {
                self.tuning_dialog = Some(TuningDialog::open(self.preferences_path.as_deref()));
                ui.close();
            }
            ui.separator();
            for (name, offsets) in TUNINGS {
                let option = ui.button(name);
                #[cfg(test)]
                self.tuning_widgets.insert(name, option.rect);
                if option.clicked() {
                    self.set_tuning(offsets, true);
                    ui.close();
                }
            }
        });
    }

    pub(super) fn tuning_parameter(&self, id: u32, kind: u32) -> bool {
        self.can_edit(TUNING, Some(id))
            && self
                .document
                .as_ref()
                .and_then(|document| document.preset().parameter(TUNING, id))
                .is_some_and(|value| value.value_type == kind)
    }

    pub(super) fn set_tuning(&mut self, offsets: [i32; 6], enable: bool) {
        self.change(|preset| {
            set_tuning_values(preset, PITCH, 0, offsets.map(|offset| offset as u32))?;
            if enable {
                preset.set_enabled(TUNING, true)?;
            }
            Ok(())
        });
    }

    fn transpose_tuning(&mut self, shift: i32) {
        self.change(|preset| {
            for i in 0..6 {
                let id = PITCH + i;
                let current = preset
                    .parameter(TUNING, id)
                    .ok_or("Missing tuning offset")?
                    .bits as i32;
                let next = current.saturating_add(shift).clamp(-12, 12);
                preset.set_parameter(TUNING, id, 0, next as u32)?;
            }
            Ok(())
        });
    }
}

// Egui's default fonts may omit the two accidental glyphs. Preserve the
// shared buttons and their keyboard targets, and draw the symbols in that case.
fn paint_missing_tuning_symbols(ui: &egui::Ui, flat: Rect, sharp: Rect, enabled: bool) {
    let font = egui::FontId::proportional(18.0);
    for (rect, glyph) in [(flat, '♭'), (sharp, '♯')] {
        if ui.fonts_mut(|fonts| fonts.has_glyph(&font, glyph)) {
            continue;
        }
        let widgets = &ui.visuals().widgets;
        let fill = if enabled && ui.rect_contains_pointer(rect) {
            widgets.hovered.weak_bg_fill
        } else {
            widgets.inactive.weak_bg_fill
        };
        let center = rect.center();
        let painter = ui.painter();
        painter.rect_filled(
            Rect::from_center_size(center, egui::vec2(14.0, 17.0)),
            0,
            fill,
        );
        let stroke = egui::Stroke::new(1.35_f32, if enabled { theme::TEXT } else { theme::DIM });
        let at = |x, y| center + egui::vec2(x, y);
        if glyph == '♭' {
            painter.line_segment([at(-2.0, -6.5), at(-2.0, 5.5)], stroke);
            painter.add(egui::epaint::CubicBezierShape::from_points_stroke(
                [at(-2.0, 0.0), at(4.0, -5.0), at(5.5, 0.5), at(-2.0, 5.5)],
                false,
                Color32::TRANSPARENT,
                stroke,
            ));
        } else {
            painter.line_segment([at(-2.0, -5.2), at(-2.0, 6.3)], stroke);
            painter.line_segment([at(2.0, -6.3), at(2.0, 5.2)], stroke);
            painter.line_segment([at(-4.7, -0.8), at(4.7, -3.0)], stroke);
            painter.line_segment([at(-4.7, 3.0), at(4.7, 0.8)], stroke);
        }
    }
}

fn body_thumbnail(body_id: u32) -> Option<String> {
    // The archived editor shares some body panels but assigns distinct slot artwork.
    let thumbnails = [
        "tele",
        "tele",
        "thinline",
        "strat",
        "lespaul",
        "lespaul",
        "lespaul",
        "juniorsinglecut",
        "lpspecial",
        "firebird",
        "gretsch6120",
        "sparklejet",
        "ric6",
        "ric12",
        "es335",
        "casino",
        "es175",
        "super400",
        "martind28",
        "martind12_28",
        "martin018",
        "guildf212",
        "j200",
        "dobro",
        "sitar",
        "danelectro",
        "banjo",
        "tricone",
    ];
    let index = body_id.checked_sub(0x0310_0000)? as usize;
    thumbnails
        .get(index)
        .map(|name| format!("bodies/buttons/{name}.png"))
}

fn bank_slot(
    ui: &mut egui::Ui,
    position: usize,
    name: &str,
    thumbnail: Option<&TextureHandle>,
    active: bool,
    changed: bool,
    unsent: bool,
) -> egui::Response {
    const ART_WIDTH: f32 = 24.0;
    const SLOT_WIDTH: f32 = 96.0;
    const NAME_PADDING: f32 = 1.0;
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(SLOT_WIDTH, theme::TARGET), Sense::click());
    let art = Rect::from_min_size(rect.min, egui::vec2(ART_WIDTH, rect.height()));
    let mut description = format!("Slot {position} · {name}");
    if changed {
        description.push_str(" · differs from the last saved file");
    }
    if unsent {
        description.push_str(" · modified since the library was opened");
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            response.enabled(),
            active,
            &description,
        )
    });
    let visuals = ui.style().interact(&response);
    let painter = ui.painter();
    let fill = if active {
        Color32::from_rgb(60, 39, 18)
    } else {
        Color32::from_rgb(34, 23, 14)
    };
    let stroke = if response.has_focus() || response.hovered() {
        visuals.fg_stroke
    } else if active {
        egui::Stroke::new(1.0_f32, theme::VARIAX)
    } else {
        visuals.bg_stroke
    };
    painter.rect(rect, 5, fill, stroke, egui::StrokeKind::Inside);
    if let Some(texture) = thumbnail {
        let icon = Rect::from_center_size(art.center(), egui::vec2(20.16, 24.0));
        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        let tint = if active || response.hovered() {
            Color32::WHITE
        } else {
            Color32::from_gray(195)
        };
        painter.image(texture.id(), icon, uv, tint);
        let digits = position.ilog10() + 1;
        let badge = Rect::from_min_size(
            art.min + egui::vec2(1.0, 1.0),
            egui::vec2(6.0 + 6.0 * digits as f32, 13.0),
        );
        painter.rect_filled(
            badge,
            3,
            if active {
                theme::VARIAX
            } else {
                Color32::from_rgb(220, 212, 198)
            },
        );
        painter.text(
            badge.center(),
            egui::Align2::CENTER_CENTER,
            position,
            egui::FontId::proportional(10.0),
            GUITAR_SURFACE,
        );
    } else {
        painter.text(
            art.center(),
            egui::Align2::CENTER_CENTER,
            position,
            egui::FontId::proportional(15.0),
            if active {
                theme::VARIAX
            } else {
                visuals.text_color()
            },
        );
    }
    if changed {
        painter.text(
            art.right_bottom() - egui::vec2(0.0, 2.0),
            egui::Align2::RIGHT_BOTTOM,
            "!",
            egui::FontId::proportional(15.0),
            FRET_MARKER,
        );
    }
    let color = visuals.text_color();
    let mut label = egui::text::LayoutJob::simple(
        name.to_owned(),
        egui::FontId::proportional(11.0),
        color,
        SLOT_WIDTH - ART_WIDTH - 2.0 * NAME_PADDING,
    );
    label.wrap.max_rows = 2;
    label.break_on_newline = false;
    let galley = painter.layout_job(label);
    let label_pos = Pos2::new(
        art.right() + NAME_PADDING,
        rect.center().y - galley.size().y / 2.0,
    );
    painter.galley(label_pos, galley, color);
    response.on_hover_text(description)
}

#[cfg(test)]
fn example_document() -> Result<EditorDocument, String> {
    fn chunk(output: &mut Vec<u8>, tag: &[u8; 4], data: &[u8]) {
        output.extend(tag);
        output.extend((data.len() as u32).to_be_bytes());
        output.extend(data);
        if !data.len().is_multiple_of(2) {
            output.push(0);
        }
    }

    let mut base = PresetFile::parse(
        include_bytes!("../../../../tests/fixtures/synthetic-host-preset.vwm").to_vec(),
    )?;
    base.select_model(PICKUP1, 0x03110004)?;
    base.select_model(PICKUP2, 0x03110003)?;
    for (slot, id, value) in [
        (PICKUP1, 0x200000, 1.8_f32),
        (PICKUP1, 0x200001, 8.7),
        (PICKUP1, 0x200003, 1.3),
        (PICKUP2, 0x200000, 3.88),
        (PICKUP2, 0x200003, 1.4),
        (WIRING, 0x200002, 470.0),
        (WIRING, 0x200003, 0.011),
        (SYSTEM, 0x200000, 0.0),
    ] {
        base.set_parameter(slot, id, 1, value.to_bits())?;
    }
    base.set_parameter(WIRING, 0x200001, 0, 1)?;
    set_tuning_values(&mut base, PITCH, 0, [0; 6])?;

    let mut bundle = b"L6BA".to_vec();
    chunk(&mut bundle, b"HEAD", &1u32.to_be_bytes());
    let bodies = [
        [
            ("T-Model example", 1),
            ("Spank example", 3),
            ("Lester example", 4),
            ("R-Billy example", 10),
            ("Twelve-string example", 13),
        ],
        [
            ("Spank neck example", 3),
            ("Lester Goldtop example", 5),
            ("T-Model Custom example", 0),
            ("Spank Drop D example", 3),
            ("Lester Custom example", 6),
        ],
    ];
    for (name, models) in ["Custom 1", "Custom 2"].into_iter().zip(bodies) {
        let mut bank = b"BANK".to_vec();
        let mut info = vec![0; 68];
        info[..4].copy_from_slice(&1u32.to_be_bytes());
        for (unit, dest) in name.encode_utf16().zip(info[4..].as_chunks_mut::<2>().0) {
            *dest = unit.to_be_bytes();
        }
        chunk(&mut bank, b"BINF", &info);
        for (model, body) in models {
            let mut preset = base.clone();
            preset.set_stored_name(model)?;
            preset.select_model(BODY, 0x03100000 + body)?;
            if body == 13 {
                set_tuning_values(&mut preset, DETUNE, 1, [0; 6])?;
                set_tuning_values(&mut preset, MIX, 1, [0.5_f32.to_bits(); 6])?;
                preset.set_parameter(TUNING, SPREAD, 1, 0)?;
            }
            if model == "Spank Drop D example" {
                set_tuning_values(&mut preset, PITCH, 0, [(-2_i32) as u32, 0, 0, 0, 0, 0])?;
                preset.set_enabled(TUNING, true)?;
            }
            bank.extend(preset.as_bytes());
        }
        chunk(&mut bundle, b"LIST", &bank);
    }
    let mut bytes = Vec::new();
    chunk(&mut bytes, b"FORM", &bundle);
    // The bundled examples have no source file.
    let mut document = EditorDocument::from_bundle(BundleFile::parse(bytes)?, Default::default())?;
    document.select(0, 1)?;
    Ok(document)
}

#[cfg(test)]
mod tests {
    use super::super::Workbench;
    use super::*;

    #[test]
    fn checkout_status_counts_renames_and_untracked_files_individually() {
        let status = SourceCheckout::parse_status(concat!(
            "# branch.oid e3e2fbbd1e55f493d1606a841b9e5a1acea646ff\n",
            "# branch.head master\n",
            "1 .M N... 100644 100644 100644 abcdef0 abcdef0 .gitignore\n",
            "2 R. N... 100644 100644 100644 abcdef0 abcdef0 R100 new.rs\told.rs\n",
            "? app/guitar.rs\n",
            "? app/main.rs\n",
            "? \"docs/file with a\\nline break.md\"\n",
        ))
        .unwrap();
        assert_eq!(status.commit, "e3e2fbbd1e55f493d1606a841b9e5a1acea646ff");
        assert_eq!(status.branch, "master");
        assert_eq!(status.changed_files, 5);

        let clean =
            SourceCheckout::parse_status("# branch.oid (initial)\n# branch.head main\n").unwrap();
        assert_eq!(clean.commit, "(initial)");
        assert_eq!(clean.branch, "main");
        assert_eq!(clean.changed_files, 0);
    }

    #[test]
    fn checkout_origin_omits_url_credentials_and_preserves_remote_paths() {
        assert_eq!(
            readable_origin("https://user:secret@example.com:8443/group/repo.git?token=secret#id"),
            "https://example.com:8443/group/repo.git"
        );
        assert_eq!(
            readable_origin("git@example.com:group/repo.git"),
            "git@example.com:group/repo.git"
        );
        assert_eq!(readable_origin("/tmp/repo.git"), "/tmp/repo.git");
    }

    fn contains_title(shape: &egui::Shape, expected: &str) -> bool {
        match shape {
            egui::Shape::Text(text) => text.galley.text() == expected,
            egui::Shape::Vec(shapes) => shapes.iter().any(|shape| contains_title(shape, expected)),
            _ => false,
        }
    }

    #[test]
    fn bank_selector_labels_built_in_examples_and_preserves_file_bank_names() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("example.vwb");
        let ctx = egui::Context::default();
        let mut app = Workbench::with_context(&ctx, None, Default::default(), None);
        app.variax_mut()
            .document
            .as_mut()
            .unwrap()
            .save_copy(&path)
            .unwrap();

        for suffix in [" (example)", ""] {
            if suffix.is_empty() {
                app.variax_mut().load(&path);
            }
            for (index, name) in ["Custom 1", "Custom 2"].into_iter().enumerate() {
                let document = app.variax_mut().document.as_mut().unwrap();
                document.select(index, 0).unwrap();
                assert_eq!(document.banks().unwrap()[index].name, name);
                let output = ctx.run(Default::default(), |ctx| {
                    egui::CentralPanel::default()
                        .show(ctx, |ui| app.variax_mut().guitar_header(ui, true));
                });
                let title = format!("Editing bank: {name}{suffix}");
                assert!(
                    output
                        .shapes
                        .iter()
                        .any(|shape| contains_title(&shape.shape, &title)),
                    "expected the bank selector to display {title:?}"
                );
            }
        }
    }

    #[test]
    fn detune_displays_whole_cents_until_the_two_decimal_field_is_focused() {
        let ctx = egui::Context::default();
        let mut app = Workbench::with_context(&ctx, None, Default::default(), None);
        app.variax_mut()
            .document
            .as_mut()
            .unwrap()
            .select(0, 4)
            .unwrap();
        let stored = 12.345_f32;
        app.variax_mut()
            .change(|preset| preset.set_parameter(TUNING, DETUNE, 1, stored.to_bits()));
        let before = app.variax().document.as_ref().unwrap().as_bytes().to_vec();

        let frame = |app: &mut Workbench, events| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(960.0, 680.0))),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default()
                        .show(ctx, |ui| app.variax_mut().test_fretboard(ui, true));
                },
            )
        };
        let contains = |output: egui::FullOutput, expected| {
            assert!(
                output
                    .shapes
                    .iter()
                    .any(|shape| contains_title(&shape.shape, expected))
            );
        };
        contains(frame(&mut app, vec![]), "+12 cents");
        let pos = app.variax().tuning_widgets["Low E detune"].center();
        let pointer = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let _ = frame(
            &mut app,
            vec![egui::Event::PointerMoved(pos), pointer(pos, true)],
        );
        let _ = frame(&mut app, vec![pointer(pos, false)]);
        contains(frame(&mut app, vec![]), "+12.35 cents");
        let _ = frame(
            &mut app,
            vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        contains(frame(&mut app, vec![]), "+12 cents");
        assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), before);
        let output = frame(&mut app, vec![]);
        assert!(
            !output
                .shapes
                .iter()
                .any(|shape| contains_title(&shape.shape, "Spread"))
        );
        let original_spread = app.variax().float(TUNING, SPREAD);
        let pos = app.variax().tuning_widgets["Zero detune"].center();
        let _ = frame(
            &mut app,
            vec![egui::Event::PointerMoved(pos), pointer(pos, true)],
        );
        let _ = frame(&mut app, vec![pointer(pos, false)]);
        assert!((DETUNE..DETUNE + 6).all(|id| app.variax().float(TUNING, id) == Some(0.0)));
        assert_eq!(app.variax().float(TUNING, SPREAD), original_spread);
        app.variax_mut().undo();
        assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), before);
    }

    #[test]
    fn tuning_presets_transpose_reset_and_linked_mix_edit_the_current_slot() {
        fn frame(ctx: &egui::Context, app: &mut Workbench, events: Vec<egui::Event>) {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(960.0, 680.0))),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default()
                        .show(ctx, |ui| app.variax_mut().test_fretboard(ui, true))
                        .inner
                },
            );
        }

        fn pointer(pos: Pos2, pressed: bool) -> egui::Event {
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            }
        }

        fn click(ctx: &egui::Context, app: &mut Workbench, name: &str) {
            let pos = app.variax().tuning_widgets[name].center();
            frame(
                ctx,
                app,
                vec![egui::Event::PointerMoved(pos), pointer(pos, true)],
            );
            frame(ctx, app, vec![pointer(pos, false)]);
            frame(ctx, app, vec![]);
        }

        let ctx = egui::Context::default();
        let mut app = Workbench::with_context(&ctx, None, Default::default(), Some("controls"));
        ctx.style_mut(|style| style.animation_time = 0.0);
        let offsets = |app: &Workbench| {
            std::array::from_fn(|i| app.variax().integer(TUNING, PITCH + i as u32).unwrap())
        };
        frame(&ctx, &mut app, vec![]);
        frame(&ctx, &mut app, vec![]);
        click(&ctx, &mut app, "Presets");
        click(&ctx, &mut app, "Drop D");
        assert_eq!(offsets(&app), [-2, 0, 0, 0, 0, 0]);
        assert!(
            app.variax()
                .document
                .as_ref()
                .unwrap()
                .preset()
                .model(TUNING)
                .unwrap()
                .enabled
        );
        click(&ctx, &mut app, "Flat");
        assert_eq!(offsets(&app), [-3, -1, -1, -1, -1, -1]);
        click(&ctx, &mut app, "Sharp");
        assert_eq!(offsets(&app), [-2, 0, 0, 0, 0, 0]);
        click(&ctx, &mut app, "Reset");
        assert_eq!(offsets(&app), [0; 6]);
        app.variax_mut().undo();
        assert_eq!(offsets(&app), [-2, 0, 0, 0, 0, 0]);
        click(&ctx, &mut app, "Lock");
        assert!(!app.variax().link_tuning_mix);
        click(&ctx, &mut app, "Low E shift");
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::Text("-4 semitones".into())],
        );
        assert_eq!(offsets(&app), [-2, 0, 0, 0, 0, 0]);
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert_eq!(offsets(&app), [-4, 0, 0, 0, 0, 0]);
        app.variax_mut().undo();
        assert_eq!(offsets(&app), [-2, 0, 0, 0, 0, 0]);

        let document = app.variax_mut().document.as_mut().unwrap();
        document.select(0, 4).unwrap();
        frame(&ctx, &mut app, vec![]);
        frame(&ctx, &mut app, vec![]);
        click(&ctx, &mut app, "Lock");
        assert!(app.variax().link_tuning_mix);
        let start = app.variax().tuning_widgets["Low E mix"].center();
        let end = start + egui::vec2(18.0, 0.0);
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(start), pointer(start, true)],
        );
        frame(&ctx, &mut app, vec![egui::Event::PointerMoved(end)]);
        frame(&ctx, &mut app, vec![pointer(end, false)]);
        let changed = app.variax().float(TUNING, MIX).unwrap();
        assert_ne!(changed, 0.5);
        assert!((MIX..MIX + 6).all(|id| app.variax().float(TUNING, id) == Some(changed)));
        let precise_mix = 0.627_f32;
        app.variax_mut()
            .change(|preset| preset.set_parameter(TUNING, MIX, 1, precise_mix.to_bits()));
        let before = app.variax().document.as_ref().unwrap().as_bytes().to_vec();
        frame(&ctx, &mut app, vec![]);
        let enter = || egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        };
        click(&ctx, &mut app, "Low E mix");
        frame(&ctx, &mut app, vec![enter()]);
        assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), before);

        click(&ctx, &mut app, "Low E mix");
        frame(&ctx, &mut app, vec![egui::Event::Text("75%".into())]);
        assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), before);
        frame(&ctx, &mut app, vec![enter()]);
        assert!((MIX..MIX + 6).all(|id| app.variax().float(TUNING, id) == Some(0.75)));
        app.variax_mut().undo();
        assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), before);
        assert_eq!(app.variax().tab, Tab::Controls);
    }
}
