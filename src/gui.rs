//! Offline Rust editor using recovered Workbench artwork and file semantics.
use eframe::egui::{self, Color32, Pos2, Rect, Sense, TextureHandle, Vec2};
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use variax_workbench::device_probe::{
    self, Consent, HostRig, LinuxSysfs, ProbeMode, ProbeSession, RigBackend, SimulatedRig, Stage,
    StageStatus,
};
use variax_workbench::hardware_gates::{self, Gate};
use variax_workbench::pod_audio::RecordRoute;
use variax_workbench::pod_preset::{self, Library, PodPresetFile};
use variax_workbench::recording::{self, ClockSelection};
use variax_workbench::rig::{HdEditorLibrary, ProductProfile, Rejection, RoleKind, Session};
use variax_workbench::{document::EditorDocument, preset::PresetFile};

#[path = "../app/guitar.rs"]
mod guitar_ui;

#[path = "families/podxt/editor/mod.rs"]
mod podxt_panel;
#[cfg(test)]
use podxt_panel::PracticeTone;
use podxt_panel::{
    PendingDelayEdit, PendingModEdit, PendingPodEqReset, PendingStompHarmony, PodBlock,
    rejection_text,
};

const PICKUP1: u32 = 0x02010001;
const PICKUP2: u32 = 0x02010002;
const BODY: u32 = 0x02010003;
const WIRING: u32 = 0x02010004;
const TUNING: u32 = 0x02010005;
const SYSTEM: u32 = variax_workbench::edit::SYSTEM;
const GUITAR_SURFACE: Color32 = Color32::from_rgb(31, 31, 34);
const GUITAR_EDGE: Color32 = Color32::from_rgb(62, 60, 59);
const FRET_MARKER: Color32 = Color32::from_rgb(247, 189, 108);
const GUITAR_SELECTION: egui::style::Selection = egui::style::Selection {
    bg_fill: Color32::from_rgb(85, 59, 39),
    stroke: egui::Stroke {
        width: 1.0,
        color: Color32::from_rgb(255, 221, 173),
    },
};
const GUITAR_ART_SIZE: Vec2 = Vec2::new(426.0, 412.0);

fn guitar_panel() -> egui::Frame {
    egui::Frame::new()
        .inner_margin(12)
        .corner_radius(8)
        .fill(GUITAR_SURFACE)
        .stroke(egui::Stroke::new(1.0_f32, GUITAR_EDGE))
}

#[derive(Clone, Deserialize)]
struct ModelArt {
    kind: String,
    id: u32,
    name: String,
    image: String,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq)]
enum Tab {
    Body,
    Pickups,
    Controls,
}
enum Pending {
    Open(PathBuf),
    ReadCurrent,
    Quit,
    /// Quit while a PODxt Live tone has applied or staged unsaved work.
    QuitPod,
    QuitRecording,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PendingHdType9Choice {
    file_index: usize,
    slot: pod_preset::HdFxSlot,
    model_id: u32,
    choice: pod_preset::HdType9Choice,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PendingHdType8Choice {
    file_index: usize,
    slot: pod_preset::HdFxSlot,
    model_id: u32,
    choice: pod_preset::HdType8Choice,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PendingHdType10Choice {
    file_index: usize,
    slot: pod_preset::HdFxSlot,
    model_id: u32,
    choice: pod_preset::HdType10Choice,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PendingHdType11Choice {
    file_index: usize,
    slot: pod_preset::HdFxSlot,
    model_id: u32,
    control: pod_preset::HdType11Control,
    choice: pod_preset::HdType11Choice,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PendingHdType12Choice {
    file_index: usize,
    slot: pod_preset::HdFxSlot,
    model_id: u32,
    control: pod_preset::HdType12Control,
    choice: pod_preset::HdType12Choice,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PendingHdType13Choice {
    file_index: usize,
    slot: pod_preset::HdFxSlot,
    model_id: u32,
    control: pod_preset::HdType13Control,
    choice: pod_preset::HdType13Choice,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PendingHdType19Choice {
    file_index: usize,
    slot: pod_preset::HdFxSlot,
    model_id: u32,
    control: pod_preset::HdType19Control,
    choice: pod_preset::HdType19Choice,
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

trait FileDialogs {
    fn open(&mut self) -> Option<PathBuf>;
    fn save(&mut self, suggested: &str, extension: &str) -> Option<PathBuf>;
    fn artwork(&mut self) -> Option<PathBuf>;
    fn open_wav(&mut self) -> Option<PathBuf>;
    fn save_wav(&mut self, suggested: &str) -> Option<PathBuf>;
    fn save_aiff(&mut self, suggested: &str) -> Option<PathBuf>;
    fn open_pod_tone(&mut self) -> Option<PathBuf>;
    fn save_pod_tone(&mut self, suggested: &str) -> Option<PathBuf>;
    #[cfg_attr(test, allow(dead_code))]
    fn pick_folder(&mut self) -> Option<PathBuf>;
    #[cfg(test)]
    fn queue_pod_save(&mut self, _path: Option<PathBuf>) {
        panic!("POD save queue is available only on fake dialogs");
    }
}

#[cfg(not(test))]
struct NativeDialogs;
#[cfg(not(test))]
impl FileDialogs for NativeDialogs {
    fn open(&mut self) -> Option<PathBuf> {
        rfd::FileDialog::new()
            .add_filter(
                "Workbench model, bank or collection",
                &["vwm", "vwb", "l6c"],
            )
            .pick_file()
    }
    fn save(&mut self, suggested: &str, extension: &str) -> Option<PathBuf> {
        rfd::FileDialog::new()
            .add_filter("Workbench document", &[extension])
            .set_file_name(suggested)
            .save_file()
    }
    fn artwork(&mut self) -> Option<PathBuf> {
        rfd::FileDialog::new().pick_folder()
    }
    fn open_wav(&mut self) -> Option<PathBuf> {
        rfd::FileDialog::new()
            .add_filter("PCM WAV", &["wav"])
            .pick_file()
    }
    fn save_wav(&mut self, suggested: &str) -> Option<PathBuf> {
        rfd::FileDialog::new()
            .add_filter("PCM WAV", &["wav"])
            .set_file_name(suggested)
            .save_file()
    }
    fn save_aiff(&mut self, suggested: &str) -> Option<PathBuf> {
        rfd::FileDialog::new()
            .add_filter("Exact-clock AIFF", &["aiff", "aif"])
            .set_file_name(suggested)
            .save_file()
    }
    fn open_pod_tone(&mut self) -> Option<PathBuf> {
        rfd::FileDialog::new().pick_file()
    }
    fn save_pod_tone(&mut self, suggested: &str) -> Option<PathBuf> {
        rfd::FileDialog::new().set_file_name(suggested).save_file()
    }
    fn pick_folder(&mut self) -> Option<PathBuf> {
        rfd::FileDialog::new().pick_folder()
    }
}

#[cfg(test)]
#[derive(Default)]
struct FakeDialogs {
    open: std::collections::VecDeque<Option<PathBuf>>,
    save: std::collections::VecDeque<Option<PathBuf>>,
    artwork: std::collections::VecDeque<Option<PathBuf>>,
    wav: std::collections::VecDeque<Option<PathBuf>>,
    pod_open: std::collections::VecDeque<Option<PathBuf>>,
    pod_save: std::collections::VecDeque<Option<PathBuf>>,
    #[cfg_attr(test, allow(dead_code))]
    folder: std::collections::VecDeque<Option<PathBuf>>,
}
#[cfg(test)]
impl FileDialogs for FakeDialogs {
    fn open(&mut self) -> Option<PathBuf> {
        self.open.pop_front().expect("unexpected open dialog")
    }
    fn save(&mut self, _: &str, _: &str) -> Option<PathBuf> {
        self.save.pop_front().expect("unexpected save dialog")
    }
    fn artwork(&mut self) -> Option<PathBuf> {
        self.artwork.pop_front().expect("unexpected artwork dialog")
    }
    fn open_wav(&mut self) -> Option<PathBuf> {
        self.wav.pop_front().expect("unexpected wav dialog")
    }
    fn save_wav(&mut self, _: &str) -> Option<PathBuf> {
        self.save.pop_front().expect("unexpected audio save dialog")
    }
    fn save_aiff(&mut self, _: &str) -> Option<PathBuf> {
        self.save.pop_front().expect("unexpected AIFF save dialog")
    }
    fn open_pod_tone(&mut self) -> Option<PathBuf> {
        self.pod_open
            .pop_front()
            .expect("unexpected POD tone dialog")
    }
    fn save_pod_tone(&mut self, _: &str) -> Option<PathBuf> {
        self.pod_save
            .pop_front()
            .expect("unexpected POD save dialog")
    }
    fn pick_folder(&mut self) -> Option<PathBuf> {
        self.folder.pop_front().expect("unexpected folder dialog")
    }
    fn queue_pod_save(&mut self, path: Option<PathBuf>) {
        self.pod_save.push_back(path);
    }
}

fn route_button_id(route: RecordRoute) -> &'static str {
    match route {
        RecordRoute::SendProcessedGuitar => "route-processed",
        RecordRoute::SendCleanGuitar => "route-clean",
        RecordRoute::SendCleanGuitarReampPlayback => "route-reamp-clean",
        RecordRoute::SendReampPlayback => "route-reamp",
    }
}

fn clock_button_id(clock: ClockSelection) -> &'static str {
    match clock {
        ClockSelection::Unselected => "clock-none",
        ClockSelection::Integer(44_100) => "clock-44100",
        ClockSelection::Integer(48_000) => "clock-48000",
        ClockSelection::Integer(_) => "clock-other",
        ClockSelection::NativePodxt => "clock-native",
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BackupChoice {
    Variax,
    Podxt,
}

#[cfg_attr(test, allow(dead_code))]
enum BackupEvent {
    Progress(String),
    Finished(Result<String, String>),
}

fn backup_offered() -> bool {
    #[cfg(feature = "usb")]
    {
        variax_workbench::library_backup::offered()
    }
    #[cfg(not(feature = "usb"))]
    {
        false
    }
}

fn tone_offered() -> bool {
    #[cfg(feature = "usb")]
    {
        variax_workbench::podxt_live::offered()
    }
    #[cfg(not(feature = "usb"))]
    {
        false
    }
}

fn parse_tone_channel(text: &str) -> Result<String, String> {
    #[cfg(feature = "usb")]
    {
        variax_workbench::podxt_live::parse_channel(text).map(|location| location.label())
    }
    #[cfg(not(feature = "usb"))]
    {
        let _ = text;
        Err("this build does not open a device for a tone check; nothing was sent".into())
    }
}

fn channel_does(label: &str) -> String {
    format!(
        "Switches the PODxt Live to channel {label}, like the footswitch, and leaves it there. \
         The app reads the saved tones first. If the current tone has unsaved edits, nothing is \
         switched. Nothing is saved. This can take a few minutes. Leave this window open until \
         it finishes."
    )
}

const BACKUP_INTRO: &str = "Backup is separate from the steps above. Step 4 stays off. Nothing is written back to either device. No is the default. You choose a folder, and a new folder is made inside it.";
const BACKUP_VARIAX: &str = "Back up the Variax";
const BACKUP_PODXT: &str = "Back up the PODxt Live";
const BACKUP_VARIAX_DOES: &str = "Reads all 60 guitar presets. The guitar changes preset while this runs, then the app tries to put the original preset back. Leave this window open until it finishes.";
const BACKUP_PODXT_DOES: &str = "Reads the stored tones and setups. The current tone is not changed on purpose. It also asks which model packs are installed. Leave this window open until it finishes.";
const BACKUP_VARIAX_YES: &str = "Yes, back up the Variax";
const BACKUP_PODXT_YES: &str = "Yes, back up the PODxt Live";
const TONE_INTRO: &str = "Tone checks are separate from the steps above. Step 4 stays off. No is the default. Nothing is saved onto the PODxt.";
const TONE_TRY: &str = "Try a change and put it back";
const TONE_TRY_DOES: &str = "Changes treble, mid, one EQ band, and the modulation, delay and reverb switches, and moves modulation before or after the amp. It reads them back, then puts the original values back. The stomp is not changed. Nothing is saved. Leave this window open until it finishes.";
const TONE_TRY_YES: &str = "Yes, try it and put it back";
const TONE_CHANNEL: &str = "Play a saved tone";
const TONE_CHANNEL_YES: &str = "Yes, play that saved tone";
const TONE_KEEP: &str = "Leaving a change on the PODxt stays off.";

#[derive(Clone, Copy, PartialEq, Eq)]
enum ToneChoice {
    Try,
    Channel,
}

struct Workbench {
    dialogs: Box<dyn FileDialogs>,
    document: Option<EditorDocument>,
    catalog: Vec<ModelArt>,
    artwork: PathBuf,
    textures: HashMap<String, TextureHandle>,
    missing_textures: HashSet<String>,
    error: Option<String>,
    status: String,
    tab: Tab,
    help: bool,
    pending: Option<Pending>,
    permit_close: bool,
    drag_origin: Option<(u32, f32, f32, f32)>,
    /// Which pickup is painted last. Pickup 1 starts in front, matching the
    /// original panel's first child. This is view state, not a file field.
    pickup_front: u32,
    /// Unsaved model-name typing. It is not a file field and not an undo step.
    name_draft: Option<String>,
    /// Unsaved author typing. It is not a file field and not an undo step.
    author_draft: Option<String>,
    /// Original `variax_editor.auto_select_control_values`. Default on.
    /// This is an editor preference, not a field in the preset file.
    auto_select_controls: bool,
    /// Where the preference is remembered. Tests leave this empty.
    preferences_path: Option<PathBuf>,
    link_tuning_mix: bool,
    theme_polled: Instant,
    theme_text: String,
    screenshot: Option<PathBuf>,
    frame_count: u32,
    recording_open: bool,
    recorder: recording::Panel,
    device_check_open: bool,
    device_check_notice: String,
    /// Backup question waiting for Yes or No. Nothing is sent until Yes.
    backup_choice: Option<BackupChoice>,
    backup_rx: Option<std::sync::mpsc::Receiver<BackupEvent>>,
    /// Tone question waiting for Yes or No. Nothing is sent until Yes.
    tone_choice: Option<ToneChoice>,
    tone_channel: String,
    tone_rx: Option<std::sync::mpsc::Receiver<BackupEvent>>,
    /// True only after the step 2 report from this session was saved. A later
    /// answer of step 2 clears it. Restarting for the G1 check requires it.
    device_check_step_two_saved: bool,
    device_check: ProbeSession<RigBackend>,
    device_check_scenario: device_probe::Scenario,
    /// Root the "This computer" reader uses: `/` in the app; tests inject a
    /// temporary fake tree (an empty path makes `host_backend` refuse).
    device_check_root: PathBuf,
    #[cfg(test)]
    host_reader_builds: usize,
    pod_open: bool,
    pod_library: Library,
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
    pod_bundle: Option<pod_preset::PodBundleLibrary>,
    pod_selected: Option<usize>,
    /// Bank shown by Bank Down / Bank Up. Channel clicks keep this in sync.
    pod_bank: usize,
    /// Block whose settings the pedalboard is showing. Switching it does not
    /// discard edits on another block.
    pod_block: PodBlock,
    pod_revert_confirm: bool,
    pod_cab_pending: Option<(usize, pod_preset::PodCabModelChoice)>,
    pod_cab_pending_origin: Option<(u64, usize, Vec<u8>)>,
    pod_delay_model_pending: Option<(usize, pod_preset::PodDelayModelChoice)>,
    pod_mod_model_pending: Option<(usize, pod_preset::PodModModelChoice)>,
    pod_stomp_model_pending: Option<(usize, pod_preset::PodStompModelChoice)>,
    pod_reverb_model_pending: Option<(usize, pod_preset::PodReverbModelChoice)>,
    pod_amp_model_pending: Option<(usize, u32, pod_preset::PodAmpModelChoice)>,
    pod_eq_reset_pending: Option<PendingPodEqReset>,
    pod_document_generation: u64,
    pod_harmony_pending: Option<(usize, pod_preset::StompHarmonyControl, PendingStompHarmony)>,
    pod_delay_pending: Option<(usize, PendingDelayEdit)>,
    pod_mod_pending: Option<(usize, PendingModEdit)>,
    pod_name: String,
    pod_message: String,
    #[cfg(test)]
    bank_slots: HashMap<usize, Rect>,
    #[cfg(test)]
    bank_options: HashMap<usize, Rect>,
    #[cfg(test)]
    bank_dropdown: Option<Rect>,
    #[cfg(test)]
    pending_buttons: HashMap<&'static str, Rect>,
    #[cfg(test)]
    pickup_slider: Option<Rect>,
    #[cfg(test)]
    tuning_toggle: Option<Rect>,
    #[cfg(test)]
    pickup_controls: HashMap<(u32, &'static str), Rect>,
    #[cfg(test)]
    name_notice: String,
    #[cfg(test)]
    auto_select_checkbox: Option<Rect>,
    #[cfg(test)]
    tuning_widgets: HashMap<&'static str, Rect>,
    #[cfg(test)]
    recording_buttons: HashMap<&'static str, Rect>,
    #[cfg(test)]
    recording_visible: String,
    #[cfg(test)]
    device_check_buttons: HashMap<&'static str, Rect>,
    #[cfg(test)]
    device_check_button_ids: HashMap<&'static str, egui::Id>,
    #[cfg(test)]
    device_check_visible: String,
    #[cfg(test)]
    recording_capture_attempts: u32,
    #[cfg(test)]
    pod_buttons: HashMap<&'static str, Rect>,
    #[cfg(test)]
    pod_rows: HashMap<usize, Rect>,
    #[cfg(test)]
    pod_scroll_area: Option<(egui::Id, Rect, f32)>,
    #[cfg(test)]
    pod_cab_combo: Option<Rect>,
    #[cfg(test)]
    pod_cab_options: HashMap<u32, Rect>,
    #[cfg(test)]
    pod_practice_combo: Option<Rect>,
    #[cfg(test)]
    pod_practice_options: HashMap<PracticeTone, Rect>,
    #[cfg(test)]
    pod_drive_slider: Option<Rect>,
    #[cfg(test)]
    pod_stomp_sliders: HashMap<pod_preset::StompControl, Rect>,
    #[cfg(test)]
    pod_whammy_knob_sliders: HashMap<pod_preset::StompWhammyKnob, Rect>,
    #[cfg(test)]
    pod_eq_frequency_sliders: HashMap<pod_preset::PodxtEqBand, Rect>,
    #[cfg(test)]
    pod_eq_gain_sliders: HashMap<pod_preset::PodxtEqBand, Rect>,
    #[cfg(test)]
    pod_model_enable_checks: HashMap<pod_preset::PodModelEnable, Rect>,
    #[cfg(test)]
    pod_model_enable_menu: Option<Rect>,
    #[cfg(test)]
    pod_mod_sliders: HashMap<pod_preset::ModControl, Rect>,
    #[cfg(test)]
    pod_reverb_sliders: HashMap<pod_preset::ReverbControl, Rect>,
    #[cfg(test)]
    pod_delay_sliders: HashMap<pod_preset::DelayControl, Rect>,
    #[cfg(test)]
    pod_delay_note_options: HashMap<pod_preset::DelayNoteChoice, Rect>,
    #[cfg(test)]
    pod_delay_model_combo: Option<Rect>,
    #[cfg(test)]
    pod_delay_model_options: HashMap<u32, Rect>,
    #[cfg(test)]
    pod_mod_model_combo: Option<Rect>,
    #[cfg(test)]
    pod_mod_model_options: HashMap<u32, Rect>,
    #[cfg(test)]
    pod_stomp_model_combo: Option<Rect>,
    #[cfg(test)]
    pod_stomp_model_options: HashMap<u32, Rect>,
    #[cfg(test)]
    pod_reverb_model_combo: Option<Rect>,
    #[cfg(test)]
    pod_reverb_model_options: HashMap<u32, Rect>,
    #[cfg(test)]
    pod_amp_model_combo: Option<Rect>,
    #[cfg(test)]
    pod_amp_model_options: HashMap<u32, Rect>,
    #[cfg(test)]
    pod_mod_note_options: HashMap<pod_preset::ModNoteChoice, Rect>,
    #[cfg(test)]
    pod_delay_time_slider: Option<Rect>,
    #[cfg(test)]
    pod_mod_speed_slider: Option<Rect>,
    #[cfg(test)]
    pod_delay_heads_choices: HashMap<pod_preset::DelayHeadsSelection, Rect>,
    #[cfg(test)]
    pod_delay_bits_choices: HashMap<pod_preset::DelayBitsSelection, Rect>,
    #[cfg(test)]
    pod_stomp_wave_choices: HashMap<pod_preset::StompWaveSelection, Rect>,
    #[cfg(test)]
    pod_harmony1_choices: HashMap<pod_preset::StompHarmony1M335Selection, Rect>,
    #[cfg(test)]
    pod_harmony2_choices: HashMap<pod_preset::StompHarmony1457Selection, Rect>,
    #[cfg(test)]
    pod_visible: String,
    read_receiver: Option<
        std::sync::mpsc::Receiver<Result<variax_workbench::read_session::CurrentPreset, String>>,
    >,
    read_worker: Option<(
        std::sync::Arc<std::sync::atomic::AtomicBool>,
        std::thread::JoinHandle<()>,
    )>,
}

impl Workbench {
    fn with_context(
        ctx: &egui::Context,
        preset: Option<PathBuf>,
        artwork: PathBuf,
        tab: Option<Tab>,
    ) -> Self {
        ctx.set_visuals(egui::Visuals::dark());
        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = Vec2::new(10.0, 10.0);
        style.spacing.button_padding = Vec2::new(12.0, 7.0);
        style.visuals.window_corner_radius = 0.into();
        style.visuals.widgets.inactive.corner_radius = 0.into();
        style.visuals.widgets.hovered.corner_radius = 0.into();
        style.visuals.widgets.active.corner_radius = 0.into();
        ctx.set_style(style);
        let catalog = serde_json::from_str(include_str!("../assets/model-ui.json"))
            .expect("bundled model catalog");
        let mut app = Self {
            #[cfg(not(test))]
            dialogs: Box::new(NativeDialogs),
            #[cfg(test)]
            dialogs: Box::new(FakeDialogs::default()),
            document: None,
            catalog,
            artwork,
            textures: HashMap::new(),
            missing_textures: HashSet::new(),
            error: None,
            status: "Edit preset files offline. Your guitar is unchanged.".into(),
            tab: Tab::Body,
            help: false,
            pending: None,
            permit_close: false,
            drag_origin: None,
            pickup_front: PICKUP1,
            name_draft: None,
            author_draft: None,
            auto_select_controls: true,
            preferences_path: None,
            link_tuning_mix: false,
            theme_polled: Instant::now() - Duration::from_secs(2),
            theme_text: String::new(),
            screenshot: None,
            frame_count: 0,
            recording_open: false,
            device_check_open: false,
            device_check_notice: String::new(),
            backup_choice: None,
            backup_rx: None,
            tone_choice: None,
            tone_channel: "1A".into(),
            tone_rx: None,
            device_check_step_two_saved: false,
            device_check: ProbeSession::new(RigBackend::Simulated(SimulatedRig::new(
                device_probe::Scenario::Connected,
            ))),
            device_check_scenario: device_probe::Scenario::Connected,
            #[cfg(not(test))]
            device_check_root: PathBuf::from(device_probe::DEFAULT_ROOT),
            #[cfg(test)]
            device_check_root: PathBuf::new(),
            #[cfg(test)]
            host_reader_builds: 0,
            recorder: recording::Panel::new(),
            pod_open: false,
            pod_library: Library::default(),
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
            pod_bundle: None,
            pod_selected: None,
            pod_bank: 0,
            pod_block: PodBlock::Amp,
            pod_revert_confirm: false,
            pod_cab_pending: None,
            pod_cab_pending_origin: None,
            pod_delay_model_pending: None,
            pod_mod_model_pending: None,
            pod_stomp_model_pending: None,
            pod_reverb_model_pending: None,
            pod_amp_model_pending: None,
            pod_eq_reset_pending: None,
            pod_document_generation: 0,
            pod_harmony_pending: None,
            pod_delay_pending: None,
            pod_mod_pending: None,
            pod_name: String::new(),
            pod_message: String::new(),
            #[cfg(test)]
            bank_slots: HashMap::new(),
            #[cfg(test)]
            bank_options: HashMap::new(),
            #[cfg(test)]
            bank_dropdown: None,
            #[cfg(test)]
            pending_buttons: HashMap::new(),
            #[cfg(test)]
            pickup_slider: None,
            #[cfg(test)]
            tuning_toggle: None,
            #[cfg(test)]
            pickup_controls: HashMap::new(),
            #[cfg(test)]
            name_notice: String::new(),
            #[cfg(test)]
            auto_select_checkbox: None,
            #[cfg(test)]
            tuning_widgets: HashMap::new(),
            #[cfg(test)]
            recording_buttons: HashMap::new(),
            #[cfg(test)]
            recording_visible: String::new(),
            #[cfg(test)]
            device_check_buttons: HashMap::new(),
            #[cfg(test)]
            device_check_button_ids: HashMap::new(),
            #[cfg(test)]
            device_check_visible: String::new(),
            #[cfg(test)]
            recording_capture_attempts: 0,
            #[cfg(test)]
            pod_buttons: HashMap::new(),
            #[cfg(test)]
            pod_rows: HashMap::new(),
            #[cfg(test)]
            pod_scroll_area: None,
            #[cfg(test)]
            pod_cab_combo: None,
            #[cfg(test)]
            pod_cab_options: HashMap::new(),
            #[cfg(test)]
            pod_practice_combo: None,
            #[cfg(test)]
            pod_practice_options: HashMap::new(),
            #[cfg(test)]
            pod_drive_slider: None,
            #[cfg(test)]
            pod_stomp_sliders: HashMap::new(),
            #[cfg(test)]
            pod_whammy_knob_sliders: HashMap::new(),
            #[cfg(test)]
            pod_eq_frequency_sliders: HashMap::new(),
            #[cfg(test)]
            pod_eq_gain_sliders: HashMap::new(),
            #[cfg(test)]
            pod_model_enable_checks: HashMap::new(),
            #[cfg(test)]
            pod_model_enable_menu: None,
            #[cfg(test)]
            pod_mod_sliders: HashMap::new(),
            #[cfg(test)]
            pod_reverb_sliders: HashMap::new(),
            #[cfg(test)]
            pod_delay_sliders: HashMap::new(),
            #[cfg(test)]
            pod_delay_note_options: HashMap::new(),
            #[cfg(test)]
            pod_delay_model_combo: None,
            #[cfg(test)]
            pod_delay_model_options: HashMap::new(),
            #[cfg(test)]
            pod_mod_model_combo: None,
            #[cfg(test)]
            pod_mod_model_options: HashMap::new(),
            #[cfg(test)]
            pod_stomp_model_combo: None,
            #[cfg(test)]
            pod_stomp_model_options: HashMap::new(),
            #[cfg(test)]
            pod_reverb_model_combo: None,
            #[cfg(test)]
            pod_reverb_model_options: HashMap::new(),
            #[cfg(test)]
            pod_amp_model_combo: None,
            #[cfg(test)]
            pod_amp_model_options: HashMap::new(),
            #[cfg(test)]
            pod_mod_note_options: HashMap::new(),
            #[cfg(test)]
            pod_delay_time_slider: None,
            #[cfg(test)]
            pod_mod_speed_slider: None,
            #[cfg(test)]
            pod_delay_heads_choices: HashMap::new(),
            #[cfg(test)]
            pod_delay_bits_choices: HashMap::new(),
            #[cfg(test)]
            pod_stomp_wave_choices: HashMap::new(),
            #[cfg(test)]
            pod_harmony1_choices: HashMap::new(),
            #[cfg(test)]
            pod_harmony2_choices: HashMap::new(),
            #[cfg(test)]
            pod_visible: String::new(),
            read_receiver: None,
            read_worker: None,
        };
        if let Some(preset) = preset {
            app.load(&preset);
        } else {
            app.open_example();
        }
        if let Some(tab) = tab {
            app.tab = tab;
        }
        app
    }

    fn load(&mut self, path: &Path) {
        if self.is_reading() {
            return;
        }
        match EditorDocument::open(path) {
            Ok(document) => {
                self.document = Some(document);
                self.drag_origin = None;
                self.clear_text_drafts();
                self.error = None;
                self.status = format!("Opened {}", path.display());
            }
            Err(error) => self.error = Some(error),
        }
    }

    fn dirty(&self) -> bool {
        // Guitar open and read prompts use this. POD tones use the quit guard.
        self.document.as_ref().is_some_and(EditorDocument::is_dirty)
    }

    fn is_reading(&self) -> bool {
        self.read_receiver.is_some()
    }

    fn cancel_read(&mut self) {
        if let Some((cancel, _)) = &self.read_worker {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            self.status = "Cancelling guitar read…".into();
        }
    }

    fn clear_text_drafts(&mut self) {
        self.name_draft = None;
        self.author_draft = None;
    }

    fn undo(&mut self) {
        if self.is_reading() || self.pending.is_some() {
            return;
        }
        if let Some(document) = &mut self.document {
            document.undo();
        } else {
            return;
        }
        self.clear_text_drafts();
    }

    fn redo(&mut self) {
        if self.is_reading() || self.pending.is_some() {
            return;
        }
        if let Some(document) = &mut self.document {
            document.redo();
        } else {
            return;
        }
        self.clear_text_drafts();
    }

    fn start_read(&mut self) {
        // The user has locked all physical-device access during offline validation.
        if hardware_gates::is_locked(Gate::CurrentSettingsRead) {
            self.status = "Live access is locked while offline validation is in progress.".into();
            return;
        }
        if self.is_reading() {
            return;
        }
        if self.dirty() {
            self.pending = Some(Pending::ReadCurrent);
            return;
        }
        self.launch_read();
    }

    fn launch_read(&mut self) {
        if hardware_gates::is_locked(Gate::CurrentSettingsRead) {
            self.status = "Live access is locked while offline validation is in progress.".into();
            return;
        }
        // A test build never starts a device read, even with the gate open.
        if cfg!(test) {
            self.status = "Test builds do not open a USB device.".into();
            return;
        }
        let (sender, receiver) = std::sync::mpsc::channel();
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let worker = std::thread::spawn(move || {
            #[cfg(feature = "usb")]
            let result = variax_workbench::usb::read_current(&worker_cancel);
            #[cfg(not(feature = "usb"))]
            let result = {
                let _ = &worker_cancel;
                Err("this build has no live USB adapter (built without the `usb` feature)".into())
            };
            let _ = sender.send(result);
        });
        self.read_worker = Some((cancel, worker));
        self.read_receiver = Some(receiver);
        self.error = None;
        self.status = "Reading current model settings from the guitar…".into();
    }

    fn poll_read(&mut self) {
        let result = self
            .read_receiver
            .as_ref()
            .and_then(|receiver| match receiver.try_recv() {
                Ok(result) => Some(result),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    Some(Err("Guitar reader stopped unexpectedly".into()))
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
            });
        if let Some(result) = result {
            self.read_receiver = None;
            if let Some((_, worker)) = self.read_worker.take() {
                let _ = worker.join();
            }
            match result.and_then(|current| {
                let version = current.identity.version_string();
                let preset = PresetFile::from_metadata(
                    &current.metadata,
                    current.identity.product_id(),
                    current.identity.version.unwrap_or(0),
                )?;
                Ok((preset, version))
            }) {
                Ok((preset, version)) => {
                    self.document = Some(EditorDocument::from_preset(
                        preset,
                        PathBuf::from("Variax current model"),
                    ));
                    self.status = format!(
                        "Read current model settings from Variax, firmware {version}. Save a copy to keep them."
                    );
                }
                Err(error) => {
                    self.error = Some(error);
                    self.status =
                        "Guitar read did not complete. Your current document is retained.".into();
                }
            }
        }
    }

    fn artwork_dialog(&mut self) {
        if self.is_reading() || self.pending.is_some() {
            return;
        }
        if let Some(path) = self.dialogs.artwork() {
            self.artwork = path;
            self.textures.clear();
            self.missing_textures.clear();
            self.error = None;
        }
    }

    fn open_dialog(&mut self) {
        if self.is_reading() {
            return;
        }
        if self.pending.is_some() {
            return;
        }
        if let Some(path) = self.dialogs.open() {
            if self.dirty() {
                self.pending = Some(Pending::Open(path));
            } else {
                self.load(&path);
            }
        }
    }

    fn save_dialog(&mut self) -> bool {
        if self.is_reading() {
            return false;
        }
        let Some(document) = &mut self.document else {
            return false;
        };
        let name: String = document
            .preset()
            .name()
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == ' ' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let extension = document.extension();
        let name = format!("{name} - edited.{extension}");
        if let Some(path) = self.dialogs.save(&name, extension) {
            match document.save_copy(&path) {
                Ok(()) => {
                    self.error = None;
                    self.status = format!("Saved copy: {}", path.display());
                    return true;
                }
                Err(error) => self.error = Some(error),
            }
        }
        false
    }

    fn continue_pending(&mut self, ctx: &egui::Context) {
        if self.is_reading() {
            return;
        }
        match self.pending.take() {
            Some(Pending::Open(path)) => self.load(&path),
            Some(Pending::ReadCurrent) => self.launch_read(),
            Some(Pending::Quit | Pending::QuitPod | Pending::QuitRecording) => {
                // Guitar save or discard must not close while a POD tone is unsaved.
                if self.pod_dirty()
                    || self.pod_has_staged_edit()
                    || self.hd_library.any_dirty()
                    || self.hd_type9_pending.is_some()
                    || self.hd_type8_pending.is_some()
                    || self.hd_type10_pending.is_some()
                    || !self.hd_type11_pending.is_empty()
                    || !self.hd_type12_pending.is_empty()
                    || !self.hd_type13_pending.is_empty()
                    || !self.hd_type19_pending.is_empty()
                {
                    self.pending = Some(Pending::QuitPod);
                    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                } else if self.recorder.has_unsaved_take() {
                    self.pending = Some(Pending::QuitRecording);
                    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                } else {
                    self.permit_close = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            None => {}
        }
    }

    fn change(&mut self, change: impl FnOnce(&mut PresetFile) -> Result<(), String>) {
        if self.is_reading() || self.pending.is_some() {
            return;
        }
        if let Some(document) = &mut self.document
            && let Err(error) = document.apply(change)
        {
            self.error = Some(error);
        }
    }

    fn can_edit(&self, slot: u32, id: Option<u32>) -> bool {
        self.document
            .as_ref()
            .is_some_and(|d| d.preset().edit_availability(slot, id).is_ok())
    }

    fn model_id(&self, slot: u32) -> Option<u32> {
        self.document.as_ref()?.preset().model(slot).map(|m| m.id)
    }
    fn model_name(&self, slot: u32) -> String {
        self.model_id(slot)
            .and_then(|id| self.catalog.iter().find(|a| a.id == id))
            .map(|a| a.name.clone())
            .unwrap_or_else(|| "Unknown model".into())
    }
    fn float(&self, slot: u32, id: u32) -> Option<f32> {
        let p = self.document.as_ref()?.preset().parameter(slot, id)?;
        (p.value_type == 1).then(|| f32::from_bits(p.bits))
    }
    fn integer(&self, slot: u32, id: u32) -> Option<i32> {
        let p = self.document.as_ref()?.preset().parameter(slot, id)?;
        (p.value_type == 0).then_some(p.bits as i32)
    }

    fn float_control(&mut self, ui: &mut egui::Ui, slot: u32, id: u32, label: &str, suffix: &str) {
        if let Some(mut value) = self.float(slot, id) {
            let Some(variax_workbench::edit::Domain::Float { min, max, .. }) =
                variax_workbench::edit::domain(slot, id)
            else {
                return;
            };
            if !value.is_finite() {
                ui.horizontal(|ui| {
                    ui.label(format!("{label}: {value} (stored)"));
                    if ui
                        .add_enabled(
                            self.can_edit(slot, Some(id)),
                            egui::Button::new("Reset to minimum"),
                        )
                        .clicked()
                    {
                        self.change(|p| p.set_parameter(slot, id, 1, min.to_bits()));
                    }
                });
                return;
            }
            let response = ui.add_enabled(
                self.can_edit(slot, Some(id)),
                egui::Slider::new(&mut value, min..=max)
                    .clamping(egui::SliderClamping::Edits)
                    .text(label)
                    .suffix(suffix),
            );
            #[cfg(test)]
            if slot == PICKUP1 && id == 0x200000 {
                self.pickup_slider = Some(response.rect);
            }
            if response.drag_started()
                && let Some(document) = &mut self.document
            {
                document.begin_gesture();
            }
            if response.changed() {
                self.change(|p| p.set_parameter(slot, id, 1, value.to_bits()));
            }
            if response.drag_stopped()
                && let Some(document) = &mut self.document
            {
                document.end_gesture();
            }
        }
    }
    fn boolean_control(&mut self, ui: &mut egui::Ui, slot: u32, id: u32, label: &str) {
        if let Some(value) = self.integer(slot, id) {
            let mut checked = value != 0;
            if ui
                .add_enabled(
                    self.can_edit(slot, Some(id)),
                    egui::Checkbox::new(&mut checked, label),
                )
                .changed()
            {
                self.change(|p| p.set_parameter(slot, id, 0, u32::from(checked)));
            }
        }
    }

    fn float_choices(&mut self, ui: &mut egui::Ui, id: u32, label: &str, choices: &[(f32, &str)]) {
        let Some(value) = self.float(WIRING, id) else {
            return;
        };
        let mut selected = value.to_bits();
        let text = choices
            .iter()
            .find(|(candidate, _)| {
                // Values read from the guitar have only the high 24 IEEE bits.
                (candidate.to_bits() & 0xffff_ff00) == (value.to_bits() & 0xffff_ff00)
            })
            .map(|(_, text)| (*text).to_owned())
            .unwrap_or_else(|| format!("{value:.6} (stored)"));
        ui.vertical(|ui| {
            ui.label(label);
            egui::ComboBox::from_id_salt(("electrical", id))
                .selected_text(text)
                .width(ui.available_width())
                .truncate()
                .show_ui(ui, |ui| {
                    for &(candidate, text) in choices {
                        ui.selectable_value(&mut selected, candidate.to_bits(), text);
                    }
                });
        });
        if selected != value.to_bits() {
            self.change(|p| p.set_parameter(WIRING, id, 1, selected));
        }
    }

    fn taper_control(&mut self, ui: &mut egui::Ui) {
        let Some(mut value) = self.integer(WIRING, 0x200001) else {
            return;
        };
        let original = value;
        ui.vertical(|ui| {
            ui.label("Taper");
            egui::ComboBox::from_id_salt("volume-taper")
                .width(ui.available_width())
                .selected_text(match value {
                    0 => "Linear",
                    1 => "Audio",
                    _ => "Unknown (stored)",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut value, 0, "Linear");
                    ui.selectable_value(&mut value, 1, "Audio");
                });
        });
        if value != original {
            self.change(|p| p.set_parameter(WIRING, 0x200001, 0, value as u32));
        }
    }
    fn enabled_control(&mut self, ui: &mut egui::Ui, slot: u32, label: &str) {
        if let Some(model) = self.document.as_ref().and_then(|d| d.preset().model(slot)) {
            let mut enabled = model.enabled;
            let response = ui.add_enabled(
                self.can_edit(slot, None),
                egui::Checkbox::new(&mut enabled, label),
            );
            if response.changed() {
                if matches!(slot, PICKUP1 | PICKUP2) {
                    self.set_pickup_enabled(slot, enabled);
                } else {
                    self.change(|p| p.set_enabled(slot, enabled));
                }
            }
            #[cfg(test)]
            if matches!(slot, PICKUP1 | PICKUP2) {
                self.pickup_controls
                    .insert((slot, "enabled"), response.rect);
            }
        }
    }

    /// Original rule: turning a pickup on brings that pickup forward, and
    /// turning one off brings the other forward. The forced enable, if any,
    /// is inside `set_enabled`, so it shares this edit's one undo step.
    fn set_pickup_enabled(&mut self, slot: u32, enabled: bool) {
        if self.is_reading() || self.pending.is_some() {
            return;
        }
        let Some(document) = &mut self.document else {
            return;
        };
        if let Err(error) = document.apply(|preset| preset.set_enabled(slot, enabled)) {
            self.error = Some(error);
            return;
        }
        let other = if slot == PICKUP1 { PICKUP2 } else { PICKUP1 };
        self.pickup_front = if enabled { slot } else { other };
    }

    fn front_control(&mut self, ui: &mut egui::Ui, slot: u32) -> egui::Response {
        let which = if slot == PICKUP1 { "1" } else { "2" };
        let response = ui
            .add(
                egui::Button::new("Front")
                    .small()
                    .selected(self.pickup_front == slot),
            )
            .on_hover_text(format!("Move pickup {which} to the front"));
        #[cfg(test)]
        self.pickup_controls.insert((slot, "front"), response.rect);
        response
    }
    fn model_control(&mut self, ui: &mut egui::Ui, slot: u32, kind: &str) {
        let Some(mut id) = self.model_id(slot) else {
            return;
        };
        let previous = id;
        egui::ComboBox::from_id_salt(("model", slot))
            .selected_text(self.model_name(slot))
            .width(ui.available_width())
            .truncate()
            .show_ui(ui, |ui| {
                for model in self.catalog.iter().filter(|a| a.kind == kind) {
                    ui.selectable_value(&mut id, model.id, &model.name);
                }
            });
        if id != previous {
            if matches!(slot, PICKUP1 | PICKUP2) {
                self.select_pickup_model(slot, id);
            } else {
                self.change(|p| p.select_model(slot, id));
            }
        }
    }

    /// A pickup model change and the matching knob values are one undo step.
    /// The original does this from `OnParamChange` only while the preference is on.
    fn select_pickup_model(&mut self, slot: u32, id: u32) {
        let auto = self.auto_select_controls;
        self.change(|preset| {
            preset.select_model(slot, id)?;
            if auto {
                preset.apply_pickup_control_values()?;
            }
            Ok(())
        });
    }

    fn set_auto_select_controls(&mut self, enabled: bool) {
        self.auto_select_controls = enabled;
        if let Some(path) = self.preferences_path.clone()
            && let Err(error) = save_auto_select(&path, enabled)
        {
            self.error = Some(error);
        }
        if enabled {
            self.change(|preset| preset.apply_pickup_control_values());
        }
    }

    fn texture(&mut self, ctx: &egui::Context, relative: &str) -> Option<TextureHandle> {
        if let Some(texture) = self.textures.get(relative) {
            return Some(texture.clone());
        }
        if self.missing_textures.contains(relative) {
            return None;
        }
        let path = self.artwork.join(relative);
        match image::open(&path) {
            Ok(image) => {
                let rgba = image.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                let pixels = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
                let texture = ctx.load_texture(relative, pixels, egui::TextureOptions::LINEAR);
                self.textures.insert(relative.to_owned(), texture.clone());
                Some(texture)
            }
            Err(_) => {
                self.missing_textures.insert(relative.to_owned());
                None
            }
        }
    }

    fn guitar(&mut self, ui: &mut egui::Ui, height: f32) {
        let body = self
            .model_id(BODY)
            .and_then(|id| self.catalog.iter().find(|a| a.id == id))
            .map(|a| a.image.clone())
            .and_then(|path| self.texture(ui.ctx(), &path));
        let scale = (ui.available_width() / GUITAR_ART_SIZE.x).min(height / GUITAR_ART_SIZE.y);
        let (stage, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), GUITAR_ART_SIZE.y * scale),
            Sense::hover(),
        );
        let rect = Rect::from_center_size(stage.center(), GUITAR_ART_SIZE * scale);
        ui.painter()
            .rect_filled(rect, 5, Color32::from_rgb(20, 20, 22));
        if let Some(texture) = body.as_ref() {
            ui.painter().image(
                texture.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        } else {
            ui.painter().text(
                stage.center(),
                egui::Align2::CENTER_CENTER,
                "Guitar artwork unavailable",
                egui::FontId::proportional(15.0),
                ui.visuals().weak_text_color(),
            );
        }
        let back_to_front = if self.pickup_front == PICKUP2 {
            [PICKUP1, PICKUP2]
        } else {
            [PICKUP2, PICKUP1]
        };
        for slot in back_to_front {
            let Some(model) = self.document.as_ref().and_then(|d| d.preset().model(slot)) else {
                continue;
            };
            let enabled = model.enabled;
            let artwork = self
                .catalog
                .iter()
                .find(|a| a.id == model.id)
                .map(|a| a.image.clone())
                .unwrap_or_else(|| "pickups/unknown.png".into());
            let stored_position = self.float(slot, 0x200000);
            let position = stored_position.unwrap_or(2.0);
            let angle = self.float(slot, 0x200001).unwrap_or(0.0).to_radians();
            if !position.is_finite() || !angle.is_finite() {
                continue;
            }
            if let Some(texture) = self.texture(ui.ctx(), &artwork) {
                let center_at = |position: f32| {
                    rect.min
                        + Vec2::new(
                            68.0 + 318.0 * ((position.clamp(0.1, 9.5) - 0.1) / 9.4),
                            215.0,
                        ) * scale
                };
                let dimension = texture.size_vec2() * scale;
                let can_drag = ui.is_enabled()
                    && stored_position.is_some()
                    && self.can_edit(slot, Some(0x200000));
                let response = ui.interact(
                    Rect::from_center_size(center_at(position), dimension + Vec2::splat(12.0)),
                    egui::Id::new(("pickup-drag", slot)),
                    if can_drag {
                        Sense::drag()
                    } else {
                        Sense::hover()
                    },
                );
                if can_drag {
                    let response = response
                        .on_hover_and_drag_cursor(egui::CursorIcon::ResizeHorizontal)
                        .on_hover_text(
                            "Drag horizontally to move this pickup. Adjust rotation in Pickups.",
                        );
                    if let Some(pointer) = response.interact_pointer_pos() {
                        if response.drag_started_by(egui::PointerButton::Primary)
                            && let Some(delta) = response.total_drag_delta()
                            && let Some(document) = &mut self.document
                        {
                            self.drag_origin =
                                Some((slot, position.clamp(0.1, 9.5), pointer.x - delta.x, scale));
                            document.begin_gesture();
                        }
                        if (response.dragged() || response.drag_stopped())
                            && let Some((active, original, start_x, start_scale)) = self.drag_origin
                            && active == slot
                        {
                            let value = (original
                                + (pointer.x - start_x) / start_scale / 318.0 * 9.4)
                                .clamp(0.1, 9.5);
                            if value != position.clamp(0.1, 9.5) {
                                self.change(|p| {
                                    p.set_parameter(slot, 0x200000, 1, value.to_bits())
                                });
                            }
                        }
                    }
                }
                let center = center_at(self.float(slot, 0x200000).unwrap_or(position));
                let rotation = egui::emath::Rot2::from_angle(angle);
                let corners = [
                    Vec2::new(-0.5, -0.5),
                    Vec2::new(0.5, -0.5),
                    Vec2::new(0.5, 0.5),
                    Vec2::new(-0.5, 0.5),
                ];
                let uvs = [
                    Pos2::new(0.0, 0.0),
                    Pos2::new(1.0, 0.0),
                    Pos2::new(1.0, 1.0),
                    Pos2::new(0.0, 1.0),
                ];
                let mut mesh = egui::Mesh::with_texture(texture.id());
                for (corner, uv) in corners.into_iter().zip(uvs) {
                    mesh.vertices.push(egui::epaint::Vertex {
                        pos: center + rotation * (corner * dimension),
                        uv,
                        color: Color32::from_white_alpha(if enabled { 255 } else { 102 }),
                    });
                }
                mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
                ui.painter().add(egui::Shape::mesh(mesh));
            }
        }
        if self.drag_origin.is_some() && !ui.input(|input| input.pointer.primary_down()) {
            self.drag_origin = None;
            if let Some(document) = &mut self.document {
                document.end_gesture();
            }
        }
        if body.is_some() {
            ui.weak("Drag a pickup to move it");
        }
    }

    fn controls(&mut self, ui: &mut egui::Ui, height: f32) {
        let slider_width = (ui.available_width() * 0.45).min(190.0);
        let compact = ui.ctx().content_rect().height() < 720.0;
        let style = ui.style_mut();
        style.spacing.item_spacing.y = if compact { 1.0 } else { 3.0 };
        style.spacing.button_padding.y = if compact { 1.0 } else { 2.0 };
        style.spacing.interact_size.y = if compact { 15.0 } else { 17.0 };
        style.spacing.slider_width = slider_width;
        style.visuals.selection = GUITAR_SELECTION;
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                if self.can_edit(SYSTEM, Some(0x200000)) {
                    egui::TopBottomPanel::bottom("output-trim")
                        .frame(egui::Frame::NONE.inner_margin(egui::Margin {
                            top: 6,
                            ..egui::Margin::ZERO
                        }))
                        .show_inside(ui, |ui| {
                            self.float_control(ui, SYSTEM, 0x200000, "Output trim", " dB");
                        });
                }
                ui.horizontal_wrapped(|ui| {
                    for (tab, title) in [
                        (Tab::Body, "Body type"),
                        (Tab::Pickups, "Pickups"),
                        (Tab::Controls, "Controls"),
                    ] {
                        ui.selectable_value(&mut self.tab, tab, title);
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_salt(self.tab)
                    .show(ui, |ui| self.tab_controls(ui));
            },
        );
    }

    fn tab_controls(&mut self, ui: &mut egui::Ui) {
        let slot = match self.tab {
            Tab::Body => BODY,
            Tab::Pickups => PICKUP1,
            Tab::Controls => WIRING,
        };
        let availability = self
            .document
            .as_ref()
            .map(|d| d.preset().edit_availability(slot, None));
        if let Some(Err(reason)) = &availability {
            ui.label(reason);
        }
        #[cfg(test)]
        {
            self.pickup_controls.clear();
            self.auto_select_checkbox = None;
        }
        ui.add_enabled_ui(availability.is_some_and(|a| a.is_ok()), |ui| {
            match self.tab {
                Tab::Body => {
                    ui.strong("Guitar body");
                    self.model_control(ui, BODY, "body");
                }
                Tab::Pickups => {
                    for (slot, title) in [(PICKUP1, "Pickup 1"), (PICKUP2, "Pickup 2")] {
                        if slot == PICKUP2 {
                            ui.separator();
                        }
                        ui.horizontal(|ui| {
                            ui.strong(title);
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    self.enabled_control(ui, slot, "Enabled");
                                    if self.front_control(ui, slot).clicked() {
                                        self.pickup_front = slot;
                                    }
                                },
                            );
                        });
                        self.model_control(ui, slot, "pickup");
                        self.float_control(ui, slot, 0x200000, "Position", " in");
                        self.float_control(ui, slot, 0x200001, "Angle", "°");
                        self.float_control(ui, slot, 0x200003, "Level", " dB");
                        if slot == PICKUP1 {
                            ui.horizontal_wrapped(|ui| {
                                self.boolean_control(
                                    ui,
                                    PICKUP2,
                                    0x200002,
                                    "Reverse pickup 2 phase",
                                );
                                self.boolean_control(
                                    ui,
                                    PICKUP2,
                                    0x200004,
                                    "Wire pickups in series",
                                );
                            });
                        }
                    }
                }
                Tab::Controls => self.electrical_controls(ui),
            }
        });
    }

    fn electrical_controls(&mut self, ui: &mut egui::Ui) {
        let resistances = [
            (25.0, "25 kΩ"),
            (50.0, "50 kΩ"),
            (100.0, "100 kΩ"),
            (250.0, "250 kΩ"),
            (470.0, "470 kΩ"),
            (500.0, "500 kΩ"),
            (1000.0, "1 MΩ"),
        ];
        ui.strong("Volume knob");
        ui.columns(2, |columns| {
            self.float_choices(&mut columns[0], 0x200000, "Resistance", &resistances);
            self.taper_control(&mut columns[1]);
        });
        ui.separator();
        ui.strong("Tone knob");
        ui.columns(2, |columns| {
            self.float_choices(&mut columns[0], 0x200002, "Resistance", &resistances);
            self.float_choices(
                &mut columns[1],
                0x200003,
                "Capacitance",
                &[
                    (0.0011, "1.1 nF"),
                    (0.0022, "2.2 nF"),
                    (0.0047, "4.7 nF"),
                    (0.011, "11 nF"),
                    (0.022, "22 nF"),
                    (0.047, "47 nF"),
                    (0.1, "0.1 µF"),
                    (-0.047, "Filter"),
                ],
            );
        });
        self.float_control(ui, WIRING, 0x200004, "Tone position", "");
        self.boolean_control(ui, WIRING, 0x200005, "Save tone-knob position");
        let mut auto = self.auto_select_controls;
        let response = ui
            .checkbox(
                &mut auto,
                "Auto-select Tone/Volume defaults when selecting pickup types",
            )
            .on_hover_text(
                "If this is checked, the control component values will be selected based on the pickup types.",
            );
        #[cfg(test)]
        {
            self.auto_select_checkbox = Some(response.rect);
        }
        if response.changed() {
            self.set_auto_select_controls(auto);
        }
    }

    fn guitar_workspace(&mut self, ui: &mut egui::Ui) {
        if self.document.is_none() {
            guitar_panel().show(ui, |ui| {
                ui.heading("Open a Variax preset, bank or collection");
                ui.horizontal_wrapped(|ui| {
                    ui.add_enabled_ui(!self.is_reading() && self.pending.is_none(), |ui| {
                        if ui.button("Open file…").clicked() {
                            self.open_dialog();
                        }
                        if ui.button("Try example banks").clicked() && self.document.is_none() {
                            self.open_example();
                        }
                    });
                });
            });
            return;
        }
        let available = !self.is_reading() && self.pending.is_none();
        self.guitar_header(ui, available);
        let editable = self
            .document
            .as_ref()
            .is_some_and(|document| matches!(document.preset().device_id(), 0x70000 | 0x70004));
        let can_edit = editable && available;
        if !editable {
            ui.label("Editing supports legacy electric Variax presets.");
        }

        egui::TopBottomPanel::bottom("guitar-tuning")
            .default_height(132.0)
            .show_separator_line(false)
            .frame(guitar_panel().inner_margin(8).outer_margin(egui::Margin {
                top: 7,
                ..egui::Margin::ZERO
            }))
            .show_inside(ui, |ui| self.fretboard(ui, can_edit));
        let panel_height = (ui.available_height() - 26.0).max(62.0);
        ui.columns(2, |columns| {
            guitar_panel().show(&mut columns[0], |ui| {
                ui.set_min_height(panel_height);
                ui.horizontal(|ui| {
                    ui.heading(self.model_name(BODY));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_enabled(available, egui::Button::new("Artwork folder…").small())
                            .clicked()
                        {
                            self.artwork_dialog();
                        }
                    });
                });
                ui.add_enabled_ui(can_edit, |ui| {
                    self.guitar(ui, (panel_height - 56.0).max(9.0));
                });
            });
            guitar_panel().show(&mut columns[1], |ui| {
                ui.set_min_width(ui.available_width());
                ui.add_enabled_ui(can_edit, |ui| self.controls(ui, panel_height));
            });
        });
    }

    fn refresh_theme(&mut self, ctx: &egui::Context) {
        if self.theme_polled.elapsed() < Duration::from_secs(1) {
            return;
        }
        self.theme_polled = Instant::now();
        let text = std::env::var_os("HOME")
            .and_then(|home| {
                fs::read_to_string(
                    Path::new(&home).join(".local/state/omarchy/current/theme/colors.toml"),
                )
                .ok()
            })
            .unwrap_or_default();
        if text != self.theme_text {
            self.theme_text = text;
            let color = |key: &str, default| {
                self.theme_text
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once('=')?;
                        if name.trim() != key {
                            return None;
                        }
                        let value = value.trim().trim_matches('"').strip_prefix('#')?;
                        let value = u32::from_str_radix(value, 16).ok()?;
                        Some(Color32::from_rgb(
                            (value >> 16) as u8,
                            (value >> 8) as u8,
                            value as u8,
                        ))
                    })
                    .unwrap_or(default)
            };
            let accent = color("accent", Color32::from_rgb(255, 214, 10));
            let background = color("background", Color32::from_rgb(26, 27, 38));
            ctx.style_mut(|style| {
                style.visuals.selection.bg_fill = accent.gamma_multiply(0.35);
                style.visuals.hyperlink_color = accent;
                style.visuals.panel_fill = background;
            });
        }
        ctx.request_repaint_after(Duration::from_secs(1));
    }
}

impl eframe::App for Workbench {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.render(ctx);
    }
}

impl Workbench {
    fn pending_dialog(&mut self, ctx: &egui::Context) {
        #[cfg(test)]
        self.pending_buttons.clear();
        if matches!(self.pending, Some(Pending::QuitRecording)) {
            self.recording_quit_dialog(ctx);
            return;
        }
        if matches!(self.pending, Some(Pending::QuitPod)) {
            self.pod_quit_dialog(ctx);
            return;
        }
        if self.pending.is_some() {
            let mut discard = false;
            let mut save_continue = false;
            let mut cancel = false;
            egui::Window::new("Unsaved changes")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label("Save a copy to keep your changes before continuing.");
                    ui.horizontal(|ui| {
                        let response = ui.button("Save copy and continue…");
                        #[cfg(test)]
                        self.pending_buttons.insert("save", response.rect);
                        if response.clicked() {
                            save_continue = true;
                        }
                        let response = ui.button("Keep editing");
                        #[cfg(test)]
                        self.pending_buttons.insert("cancel", response.rect);
                        if response.clicked() {
                            cancel = true;
                        }
                        let response = ui.button("Discard changes");
                        #[cfg(test)]
                        self.pending_buttons.insert("discard", response.rect);
                        if response.clicked() {
                            discard = true;
                        }
                    });
                });
            if cancel {
                self.pending = None;
            }
            if save_continue {
                discard = self.save_dialog();
            }
            if discard {
                self.continue_pending(ctx);
            }
        }
    }

    fn request_close(&mut self, ctx: &egui::Context) {
        if self.is_reading() {
            self.cancel_read();
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        } else if self.pending.is_some() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        } else if self.dirty() && !self.permit_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Pending::Quit);
        } else if (self.pod_dirty()
            || self.pod_has_staged_edit()
            || self.hd_library.any_dirty()
            || self.hd_type9_pending.is_some()
            || self.hd_type8_pending.is_some()
            || self.hd_type10_pending.is_some()
            || !self.hd_type11_pending.is_empty()
            || !self.hd_type12_pending.is_empty()
            || !self.hd_type13_pending.is_empty()
            || !self.hd_type19_pending.is_empty())
            && !self.permit_close
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Pending::QuitPod);
        } else if self.recorder.has_unsaved_take() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Pending::QuitRecording);
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn recording_quit_dialog(&mut self, ctx: &egui::Context) {
        if !self.recorder.has_unsaved_take() {
            self.continue_pending(ctx);
            return;
        }
        let native = self.recorder.take_can_save_aiff();
        let can_save = self.recorder.take_can_save_wav() || native;
        let save_label = match (native, self.recorder.phase()) {
            (true, recording::Phase::Recording) => "Stop and save AIFF copy…",
            (true, _) => "Save AIFF copy…",
            (false, recording::Phase::Recording) => "Stop and save WAV copy…",
            (false, _) => "Save WAV copy…",
        };
        let mut save = false;
        let mut discard = false;
        let mut cancel = false;
        egui::Window::new("Unsaved offline test take")
            .default_width(560.0)
            .max_width(640.0)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label("This test take or its pending capture recovery data has not been fully saved or explicitly discarded.");
                if native {
                    ui.label("Exact-clock AIFF preserves 78125/2. Other software may round this rate; verify imports.");
                }
                ui.label(self.recorder.message());
                ui.horizontal(|ui| {
                    let response = ui.add_enabled(can_save, egui::Button::new(save_label));
                    #[cfg(test)]
                    self.pending_buttons.insert("save", response.rect);
                    save = response.clicked();
                    let response = ui.button("Keep editing");
                    #[cfg(test)]
                    self.pending_buttons.insert("cancel", response.rect);
                    cancel = response.clicked();
                    let response = ui.button("Discard this take");
                    #[cfg(test)]
                    self.pending_buttons.insert("discard", response.rect);
                    discard = response.clicked();
                });
            });
        if cancel {
            self.pending = None;
        } else if save {
            if self.recorder.phase() == recording::Phase::Recording
                && let Err(error) = self.recorder.stop()
            {
                self.recorder.note(error.to_string());
                return;
            }
            if self.save_recording() {
                self.continue_pending(ctx);
            }
        } else if discard {
            match self.recorder.cancel() {
                Ok(()) => self.continue_pending(ctx),
                Err(error) => self.recorder.note(error.to_string()),
            }
        }
    }

    fn render(&mut self, ctx: &egui::Context) {
        self.invalidate_stale_eq_reset_stage();
        self.refresh_theme(ctx);
        self.poll_read();
        if self.read_receiver.is_some() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
        self.frame_count += 1;
        if self.screenshot.is_some() && self.frame_count <= 5 {
            ctx.request_repaint();
            if self.frame_count == 5 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
        }
        for event in ctx.input(|input| input.events.clone()) {
            if let egui::Event::Screenshot { image: capture, .. } = event
                && let Some(path) = self.screenshot.take()
            {
                let pixels: Vec<u8> = capture
                    .pixels
                    .iter()
                    .flat_map(|pixel| pixel.to_array())
                    .collect();
                if let Err(error) = save_screenshot(
                    &path,
                    &pixels,
                    capture.size[0] as u32,
                    capture.size[1] as u32,
                ) {
                    self.error = Some(format!("Screenshot: {error}"));
                } else {
                    self.request_close(ctx);
                }
            }
        }
        let (open, save, undo, redo, quit, help) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::O) && i.modifiers.command,
                i.key_pressed(egui::Key::S) && i.modifiers.command,
                i.key_pressed(egui::Key::Z) && i.modifiers.command && !i.modifiers.shift,
                i.key_pressed(egui::Key::Z) && i.modifiers.command && i.modifiers.shift,
                i.key_pressed(egui::Key::Q) && !ctx.wants_keyboard_input(),
                i.key_pressed(egui::Key::Questionmark) && !ctx.wants_keyboard_input(),
            )
        });
        if open {
            self.open_dialog();
        }
        if save && self.pending.is_none() {
            self.save_dialog();
        }
        if undo {
            self.undo();
        }
        if redo {
            self.redo();
        }
        if help {
            self.help = !self.help;
        }
        if quit || ctx.input(|i| i.viewport().close_requested()) {
            self.request_close(ctx);
        }
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.heading("Variax Workbench");
                ui.weak("Offline validation");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Milestone & Help").clicked() {
                        self.help = true;
                    }
                    if ui.add_enabled(!self.is_reading() && self.pending.is_none(), egui::Button::new("Save copy…")).clicked() {
                        self.save_dialog();
                    }
                    if ui
                        .add_enabled(
                            false,
                            egui::Button::new("Live access locked"),
                        )
                        .on_disabled_hover_text("Hardware testing is locked during offline validation. Future testing starts with the lowest-risk operation and advances one explicitly approved step at a time.")
                        .clicked()
                    {
                        self.start_read();
                    }
                    if ui.add_enabled(!self.is_reading() && self.pending.is_none(), egui::Button::new("Open…")).clicked() {
                        self.open_dialog();
                    }
                    let response = ui.button("Device check…");
                    #[cfg(test)]
                    self.device_check_buttons.insert("open", response.rect);
                    if response.clicked() {
                        self.device_check_open = true;
                    }
                    let response = ui.button("Test recording…");
                    #[cfg(test)]
                    self.recording_buttons.insert("open-recording", response.rect);
                    if response.clicked() {
                        self.recording_open = true;
                    }
                    let pod_tab = ui.selectable_label(self.pod_open, "PODxt Live");
                    #[cfg(test)]
                    self.pod_buttons.insert("open-pod", pod_tab.rect);
                    if pod_tab.clicked() {
                        self.pod_open = true;
                    }
                    let variax_tab = ui.selectable_label(!self.pod_open, "Variax");
                    #[cfg(test)]
                    self.pod_buttons.insert("show-variax", variax_tab.rect);
                    if variax_tab.clicked() {
                        // Leaving the tab keeps the in-memory tones and unsaved edits.
                        self.pod_open = false;
                    }
                });
            });
        });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            if self.is_reading() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Reading model settings…");
                    if ui.button("Cancel read").clicked() {
                        self.cancel_read();
                    }
                });
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::BOTTOM), |ui| {
                ui.label(format!("build {}", crate::BUILD_NUMBER));
                ui.label(crate::APP_VERSION);
                ui.vertical(|ui| {
                    ui.weak(&self.status);
                    if let Some(error) = &self.error {
                        ui.colored_label(Color32::LIGHT_RED, error);
                    }
                });
            });
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.pod_open {
                self.pod_window(ui);
            } else {
                self.guitar_workspace(ui);
            }
        });
        if self.help {
            egui::Window::new(format!("Workbench Help · {}", crate::APP_VERSION))
                .open(&mut self.help)
                .collapsible(false)
                .default_width(400.0)
                .default_height(600.0)
                .vscroll(true)
                .show(ctx, |ui| {
                    guitar_ui::milestones(ui);
                    ui.separator();
                    ui.heading("Keyboard shortcuts");
                    for (key, action) in [
                        ("Ctrl+O", "Open preset"),
                        ("Ctrl+S", "Save a new copy"),
                        ("Ctrl+Z", "Undo"),
                        ("Ctrl+Shift+Z", "Redo"),
                        ("Q", "Quit"),
                        ("?", "Toggle Help"),
                    ] {
                        ui.horizontal(|ui| {
                            ui.monospace(key);
                            ui.label(action);
                        });
                    }
                });
        }
        self.pending_dialog(ctx);
        self.recording_window(ctx);
        self.device_check_window(ctx);
    }

    fn device_check_button(
        &mut self,
        ui: &mut egui::Ui,
        id: &'static str,
        label: &str,
    ) -> egui::Response {
        let response = ui.button(label);
        #[cfg(test)]
        {
            self.device_check_buttons.insert(id, response.rect);
            self.device_check_button_ids.insert(id, response.id);
        }
        #[cfg(not(test))]
        let _ = id;
        response
    }

    /// The "This computer (G1)" backend: the real step-1 reader on
    /// `device_check_root`. Built only when that mode is chosen.
    fn host_backend(&mut self) -> RigBackend {
        #[cfg(test)]
        {
            // Tests must inject a temporary fake tree; never the real root.
            assert!(
                self.device_check_root.starts_with(std::env::temp_dir())
                    && self.device_check_root != Path::new(device_probe::DEFAULT_ROOT),
                "tests must inject a temporary fake tree for This computer mode"
            );
            self.host_reader_builds += 1;
        }
        RigBackend::ThisComputer(HostRig::new(LinuxSysfs::with_root(
            self.device_check_root.clone(),
        )))
    }

    fn restart_device_check(&mut self, mode: ProbeMode, scenario: device_probe::Scenario) {
        self.device_check_scenario = scenario;
        let backend = match mode {
            ProbeMode::ThisComputer
                if device_probe::live_stage_available(Stage::UsbRecognition) =>
            {
                self.host_backend()
            }
            _ => RigBackend::Simulated(SimulatedRig::new(scenario)),
        };
        self.device_check = ProbeSession::new(backend);
        self.device_check_step_two_saved = false;
        self.device_check_notice.clear();
    }

    /// Staged device check (#157, #158). "Simulation" is the default. "This
    /// computer" is drawn only when a live step is open, and its label names
    /// the gates this build can run. Closing the window in the same moment as
    /// Yes cancels before the step starts. Once Yes has started, the exchange
    /// runs on this thread and the window cannot cancel it. A restart after
    /// step 2 saves that report first.
    /// Saves the sanitized report only. A failure is shown as a fixed phrase
    /// and does not answer or rerun a stage.
    fn note_sanitized_report_save(&mut self, path: &Path) {
        self.device_check_notice = match self.device_check.save_sanitized_report(path) {
            Ok(()) => {
                if self.device_check.identification_answered() {
                    self.device_check_step_two_saved = true;
                }
                "Steps 1–2 report saved.".into()
            }
            Err(error) => error.to_string(),
        };
    }

    fn poll_backup(&mut self, ctx: &egui::Context) {
        let Some(rx) = self.backup_rx.as_ref() else {
            return;
        };
        let mut finished = None;
        while let Ok(event) = rx.try_recv() {
            match event {
                BackupEvent::Progress(line) => self.device_check_notice = line,
                BackupEvent::Finished(result) => finished = Some(result),
            }
        }
        if let Some(result) = finished {
            self.backup_rx = None;
            self.device_check_notice = match result {
                Ok(text) => text,
                Err(text) => text,
            };
            self.status = "Backup finished. See Device check for the result.".into();
        } else {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
    }

    fn start_backup(&mut self, choice: BackupChoice) {
        self.backup_choice = None;
        #[cfg(test)]
        {
            let _ = choice;
            self.device_check_notice =
                "this build does not open a device for a backup; nothing was sent".into();
        }
        #[cfg(all(not(test), feature = "usb"))]
        {
            let Some(root) = self.dialogs.pick_folder() else {
                self.device_check_notice = "No folder chosen. Nothing was sent.".into();
                return;
            };
            let (tx, rx) = std::sync::mpsc::channel();
            self.backup_rx = Some(rx);
            self.device_check_notice = "Backup started. Leave this window open.".into();
            std::thread::spawn(move || {
                let tx_progress = tx.clone();
                let result = variax_workbench::library_backup::run(
                    match choice {
                        BackupChoice::Variax => variax_workbench::library_backup::Which::Variax,
                        BackupChoice::Podxt => variax_workbench::library_backup::Which::Podxt,
                    },
                    &root,
                    |line| {
                        let _ = tx_progress.send(BackupEvent::Progress(line.to_owned()));
                    },
                );
                let _ = tx.send(BackupEvent::Finished(result));
            });
        }
        #[cfg(all(not(test), not(feature = "usb")))]
        {
            let _ = choice;
            self.device_check_notice =
                "this build does not open a device for a backup; nothing was sent".into();
        }
    }

    fn poll_tone(&mut self, ctx: &egui::Context) {
        let Some(rx) = self.tone_rx.as_ref() else {
            return;
        };
        let mut finished = None;
        while let Ok(event) = rx.try_recv() {
            match event {
                BackupEvent::Progress(line) => self.device_check_notice = line,
                BackupEvent::Finished(result) => finished = Some(result),
            }
        }
        if let Some(result) = finished {
            self.tone_rx = None;
            self.device_check_notice = match result {
                Ok(text) => text,
                Err(text) => text,
            };
            self.status = "Tone check finished. See Device check for the result.".into();
        } else {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
    }

    fn start_tone(&mut self, choice: ToneChoice) {
        self.tone_choice = None;
        if self.backup_rx.is_some() || self.tone_rx.is_some() {
            self.device_check_notice = "Wait until the current check finishes.".into();
            return;
        }
        #[cfg(test)]
        {
            let _ = choice;
            self.device_check_notice =
                "this build does not open a device for a tone check; nothing was sent".into();
        }
        #[cfg(all(not(test), feature = "usb"))]
        {
            let channel = self.tone_channel.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            self.tone_rx = Some(rx);
            self.device_check_notice = "Tone check started. Leave this window open.".into();
            std::thread::spawn(move || {
                let tx_progress = tx.clone();
                let result = match choice {
                    ToneChoice::Try => variax_workbench::podxt_live::run_try(|line| {
                        let _ = tx_progress.send(BackupEvent::Progress(line.to_owned()));
                    }),
                    ToneChoice::Channel => {
                        variax_workbench::podxt_live::run_channel(&channel, |line| {
                            let _ = tx_progress.send(BackupEvent::Progress(line.to_owned()));
                        })
                    }
                };
                let _ = tx.send(BackupEvent::Finished(result));
            });
        }
        #[cfg(all(not(test), not(feature = "usb")))]
        {
            let _ = choice;
            self.device_check_notice =
                "this build does not open a device for a tone check; nothing was sent".into();
        }
    }

    fn device_check_window(&mut self, ctx: &egui::Context) {
        self.poll_backup(ctx);
        self.poll_tone(ctx);
        if !self.device_check_open {
            return;
        }
        #[cfg(test)]
        self.device_check_buttons.retain(|id, _| *id == "open");
        let mut open = true;
        let mut answer = None;
        let mut restart = None;
        let mut verify_g1 = false;
        let g1_available = device_probe::live_stage_available(Stage::UsbRecognition);
        let mode = self.device_check.mode();
        egui::Window::new("Device check")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(640.0)
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 56.0))
            .show(ctx, |ui| {
                if self.pending.is_some() {
                    ui.disable();
                }
                let mut visible = String::new();
                let banner = device_probe::availability_banner();
                ui.colored_label(FRET_MARKER, banner);
                visible.push_str(banner);
                ui.horizontal(|ui| {
                    ui.label("Mode:");
                    for (id, choice) in [
                        ("mode-simulation", ProbeMode::Simulation),
                        ("mode-this-computer", ProbeMode::ThisComputer),
                    ] {
                        // Omit the real-device mode entirely when this build
                        // cannot run it. A disabled radio would still offer it.
                        if choice == ProbeMode::ThisComputer && !g1_available {
                            continue;
                        }
                        let response = ui.add(egui::RadioButton::new(mode == choice, choice.label()));
                        #[cfg(test)]
                        {
                            self.device_check_buttons.insert(id, response.rect);
                            self.device_check_button_ids.insert(id, response.id);
                            visible.push_str(choice.label());
                            visible.push('\n');
                        }
                        #[cfg(not(test))]
                        let _ = id;
                        if response.clicked() && mode != choice {
                            restart = Some((choice, self.device_check_scenario));
                        }
                    }
                });
                let label = match mode {
                    ProbeMode::Simulation => device_probe::SIMULATION_LABEL,
                    ProbeMode::ThisComputer => device_probe::this_computer_label(),
                };
                ui.label(label);
                visible.push_str(&format!("\n{label}"));
                ui.weak(
                    "Each step asks first, the answer is No unless you choose Yes, and a passed \
                     step never starts the next one.",
                );
                ui.label(device_probe::STEP_TWO_PREFLIGHT);
                ui.label(device_probe::CANCEL_LIMIT);
                visible.push_str(&format!(
                    "\n{}\n{}",
                    device_probe::STEP_TWO_PREFLIGHT,
                    device_probe::CANCEL_LIMIT
                ));
                ui.horizontal(|ui| {
                    if mode == ProbeMode::Simulation {
                        ui.label("Pretend setup:");
                        egui::ComboBox::from_id_salt("device-check-scenario")
                            .width(300.0)
                            .selected_text(self.device_check_scenario.label())
                            .show_ui(ui, |ui| {
                                for scenario in device_probe::Scenario::ALL {
                                    if ui
                                        .selectable_label(
                                            self.device_check_scenario == scenario,
                                            scenario.label(),
                                        )
                                        .clicked()
                                    {
                                        restart = Some((mode, scenario));
                                    }
                                }
                            });
                    }
                    if self.device_check_button(ui, "restart", "Start over").clicked() {
                        restart = Some((mode, self.device_check_scenario));
                    }
                });
                ui.separator();
                for stage in Stage::LADDER {
                    let status = self.device_check.status(stage);
                    let waiting = self.device_check.awaiting() == Some(stage);
                    let locked = self.device_check.locked(stage);
                    let (mark, colour, state) = match status {
                        StageStatus::Passed => ("✔", Color32::from_rgb(167, 231, 133), status.label()),
                        StageStatus::Stopped => ("■", Color32::LIGHT_RED, status.label()),
                        StageStatus::NotRun if locked => {
                            ("○", Color32::GRAY, "locked in this build")
                        }
                        StageStatus::NotRun if waiting => {
                            ("▶", FRET_MARKER, "waiting for your answer")
                        }
                        StageStatus::NotRun => ("○", Color32::GRAY, status.label()),
                    };
                    ui.horizontal(|ui| {
                        ui.colored_label(colour, mark);
                        ui.strong(format!(
                            "Step {} · {} ({})",
                            stage.number(),
                            stage.title(),
                            stage.gate_label()
                        ));
                        ui.weak(state);
                        let lock = if device_probe::live_stage_available(stage) {
                            "real device: open"
                        } else {
                            "real device: locked"
                        };
                        ui.weak(format!("· {lock}"));
                        visible.push_str(&format!("\nstep {} {state} {lock}", stage.number()));
                    });
                    if let Some(detail) = self.device_check.detail(stage) {
                        ui.indent(("device-check-detail", stage.number()), |ui| {
                            ui.label(detail);
                        });
                        visible.push_str(&format!(": {detail}"));
                    }
                }
                ui.separator();
                if let Some(stage) = self.device_check.awaiting() {
                    ui.heading(format!("Step {}: {}", stage.number(), stage.title()));
                    ui.strong("What it does");
                    ui.label(stage.action());
                    ui.strong("Risk");
                    ui.label(stage.risk());
                    visible.push_str(&format!("\n{}\n{}", stage.action(), stage.risk()));
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        let no = self.device_check_button(ui, "no", "No, stop");
                        if ui.memory(|memory| memory.focused().is_none()) {
                            no.request_focus();
                        }
                        if no.clicked() {
                            answer = Some(Consent::No);
                        }
                        let yes = device_probe::consent_yes_label(mode, stage);
                        visible.push_str(yes);
                        if self.device_check_button(ui, "yes", yes).clicked() {
                            answer = Some(Consent::Yes);
                        }
                    });
                } else {
                    let outcome = match (self.device_check.stopped(), mode) {
                        (Some((stage, _)), _) => format!(
                            "Stopped at step {}. Nothing further runs; choose Start over to try again.",
                            stage.number()
                        ),
                        (None, ProbeMode::Simulation) => {
                            "All four steps passed in simulation. No real device was used.".into()
                        }
                        (None, ProbeMode::ThisComputer) => {
                            device_probe::this_computer_finished_message().into()
                        }
                    };
                    ui.label(&outcome);
                    visible.push_str(&format!("\n{outcome}"));
                }
                if self.device_check.mode() == ProbeMode::ThisComputer && backup_offered() {
                    ui.separator();
                    ui.heading("Backup");
                    ui.label(BACKUP_INTRO);
                    visible.push_str(BACKUP_INTRO);
                    if self.backup_rx.is_some() {
                        ui.label("Backup is running. Leave this window open.");
                        visible.push_str("\nBackup is running.");
                    } else if let Some(choice) = self.backup_choice {
                        let (does, yes) = match choice {
                            BackupChoice::Variax => (BACKUP_VARIAX_DOES, BACKUP_VARIAX_YES),
                            BackupChoice::Podxt => (BACKUP_PODXT_DOES, BACKUP_PODXT_YES),
                        };
                        ui.label(does);
                        visible.push_str(does);
                        ui.horizontal(|ui| {
                            let no = self.device_check_button(ui, "backup-no", "No, stop");
                            if no.clicked() {
                                self.backup_choice = None;
                                self.device_check_notice = "Nothing was read.".into();
                            }
                            if self.device_check_button(ui, "backup-yes", yes).clicked() {
                                self.start_backup(choice);
                            }
                        });
                        visible.push_str(yes);
                    } else {
                        ui.horizontal(|ui| {
                            if self
                                .device_check_button(ui, "backup-variax", BACKUP_VARIAX)
                                .clicked()
                            {
                                self.backup_choice = Some(BackupChoice::Variax);
                            }
                            if self
                                .device_check_button(ui, "backup-podxt", BACKUP_PODXT)
                                .clicked()
                            {
                                self.backup_choice = Some(BackupChoice::Podxt);
                            }
                        });
                        visible.push_str(BACKUP_VARIAX);
                        visible.push_str(BACKUP_PODXT);
                    }
                }
                if self.device_check.mode() == ProbeMode::ThisComputer && tone_offered() {
                    ui.separator();
                    ui.heading("PODxt Live tone");
                    ui.label(TONE_INTRO);
                    visible.push_str(TONE_INTRO);
                    ui.label(TONE_KEEP);
                    visible.push_str(TONE_KEEP);
                    if self.tone_rx.is_some() {
                        ui.label("The tone check is running. Leave this window open.");
                        visible.push_str("\nThe tone check is running.");
                    } else if let Some(choice) = self.tone_choice {
                        let (does, yes) = match choice {
                            ToneChoice::Try => (TONE_TRY_DOES.to_owned(), TONE_TRY_YES),
                            ToneChoice::Channel => {
                                (channel_does(self.tone_channel.trim()), TONE_CHANNEL_YES)
                            }
                        };
                        ui.label(&does);
                        visible.push_str(&does);
                        ui.horizontal(|ui| {
                            let no = self.device_check_button(ui, "tone-no", "No, stop");
                            if no.clicked() {
                                self.tone_choice = None;
                                self.device_check_notice = "Nothing was changed.".into();
                            }
                            if self.device_check_button(ui, "tone-yes", yes).clicked() {
                                self.start_tone(choice);
                            }
                        });
                        visible.push_str(yes);
                    } else {
                        ui.horizontal(|ui| {
                            if self.device_check_button(ui, "tone-try", TONE_TRY).clicked() {
                                self.tone_choice = Some(ToneChoice::Try);
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label("Channel");
                            ui.add(
                                egui::TextEdit::singleline(&mut self.tone_channel)
                                    .desired_width(80.0)
                                    .hint_text("1A"),
                            );
                            if self
                                .device_check_button(ui, "tone-channel", TONE_CHANNEL)
                                .clicked()
                            {
                                match parse_tone_channel(&self.tone_channel) {
                                    Ok(label) => {
                                        self.tone_channel = label;
                                        self.tone_choice = Some(ToneChoice::Channel);
                                    }
                                    Err(error) => self.device_check_notice = error,
                                }
                            }
                        });
                        visible.push_str(TONE_TRY);
                        visible.push_str(TONE_CHANNEL);
                        visible.push_str(&self.tone_channel);
                    }
                }
                let report = self.device_check.report();
                let sanitized = self.device_check.sanitized_report();
                ui.horizontal(|ui| {
                    if self
                        .device_check_button(ui, "copy", "Copy detailed report (#125)")
                        .clicked()
                    {
                        ui.ctx().copy_text(report.clone());
                    }
                });
                let reports_note = device_probe::reports_panel_note();
                ui.collapsing("Reports", |ui| {
                    ui.label("Detailed report (schema 1), for issue #125.");
                    ui.monospace(&report);
                    ui.label(reports_note);
                    ui.monospace(&sanitized);
                });
                visible.push_str(reports_note);
                if self
                    .device_check_button(ui, "save-report", "Save steps 1–2 report")
                    .clicked()
                    && let Some(path) = self.dialogs.save(&variax_workbench::probe_report::report_file_name_now(), "txt")
                {
                    self.note_sanitized_report_save(&path);
                }
                if self.device_check.identification_answered() {
                    visible.push_str(device_probe::SAVE_THEN_CHECK_AGAIN);
                    if self
                        .device_check_button(
                            ui,
                            "verify-g1",
                            device_probe::SAVE_THEN_CHECK_AGAIN,
                        )
                        .clicked()
                    {
                        verify_g1 = true;
                    }
                }
                if !self.device_check_notice.is_empty() {
                    ui.label(&self.device_check_notice);
                    visible.push_str(&format!("\n{}", self.device_check_notice));
                }
                #[cfg(test)]
                {
                    self.device_check_visible = visible;
                }
                #[cfg(not(test))]
                let _ = visible;
            });
        // Close wins over Yes and over Start over / mode change in this frame.
        // That only applies before the exchange starts. Once Yes has begun,
        // `answer` runs the exchange here, on this thread, and nothing in this
        // window can stop it.
        if !open {
            self.device_check.answer(Consent::Cancel);
            self.device_check_open = false;
        } else {
            if let Some(consent) = answer {
                let stage = self.device_check.awaiting();
                self.device_check.answer(consent);
                if stage == Some(Stage::Identification) {
                    self.device_check_step_two_saved = false;
                }
            }
            if verify_g1 {
                self.save_step_two_report_then_restart();
            } else if let Some((mode, scenario)) = restart {
                if self.device_check.identification_answered() && !self.device_check_step_two_saved
                {
                    self.device_check_notice = device_probe::SAVE_BEFORE_RESTART.into();
                } else {
                    self.restart_device_check(mode, scenario);
                }
            }
        }
    }

    /// Write the step 2 report, then start a fresh step 1. A failed or
    /// cancelled save leaves the current check in place.
    fn save_step_two_report_then_restart(&mut self) {
        let mode = self.device_check.mode();
        let scenario = self.device_check_scenario;
        let Some(path) = self.dialogs.save(
            &variax_workbench::probe_report::report_file_name_now(),
            "txt",
        ) else {
            return;
        };
        match self.device_check.save_sanitized_report(&path) {
            Ok(()) => {
                self.device_check_step_two_saved = true;
                self.restart_device_check(mode, scenario);
                self.device_check_notice = "Steps 1–2 report saved. Step 1 is ready again.".into();
            }
            Err(error) => self.device_check_notice = error.to_string(),
        }
    }

    fn recording_window(&mut self, ctx: &egui::Context) {
        if !self.recording_open {
            return;
        }
        let mut open = true;
        egui::Window::new("Offline test recording")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::LEFT_TOP, egui::vec2(24.0, 64.0))
            .show(ctx, |ui| {
                if self.pending.is_some() {
                    ui.disable();
                }
                ui.label(recording::OFFLINE_BANNER);
                ui.label(recording::DEVICE_CAPTURE_LOCKED);
                ui.label(recording::NO_AUTO_START);
                ui.label(self.recorder.phase_label());
                ui.label(self.recorder.meter_line());
                ui.label(self.recorder.guide_line());
                ui.label(self.recorder.message());
                ui.separator();
                for (route, label) in recording::ROUTE_CHOICES {
                    if self.record_choice(
                        ui,
                        route_button_id(route),
                        self.recorder.route() == Some(route),
                        label,
                    ) && let Err(error) = self.recorder.select_route(route)
                    {
                        self.recorder.note(error.to_string());
                    }
                }
                for (clock, label) in recording::CLOCK_CHOICES {
                    if self.record_choice(
                        ui,
                        clock_button_id(clock),
                        self.recorder.clock() == clock,
                        label,
                    ) && let Err(error) = self.recorder.select_clock(clock)
                    {
                        self.recorder.note(error.to_string());
                    }
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if self.record_button(ui, "inspect-native-plan", "Inspect PODxt Live plan…")
                        && let Err(error) = self.recorder.inspect_native_plan()
                    {
                        self.recorder.note(error.to_string());
                    }
                    if self.record_button(ui, "check-external-plan", "Check external interface…")
                        && let Err(error) = self.recorder.external_plan_status()
                    {
                        self.recorder.note(error.to_string());
                    }
                });
                if self.record_choice(
                    ui,
                    "input-synthetic",
                    self.recorder.input() == recording::InputSelection::Synthetic,
                    "Synthetic generator",
                ) && let Err(error) = self
                    .recorder
                    .select_input(recording::InputSelection::Synthetic)
                {
                    self.recorder.note(error.to_string());
                }
                if matches!(self.recorder.clock(), recording::ClockSelection::Integer(_))
                    && self.record_choice(
                        ui,
                        "input-simulated-callback",
                        self.recorder.input() == recording::InputSelection::SimulatedCallback,
                        "Simulated callback test data",
                    )
                    && let Err(error) = self
                        .recorder
                        .select_input(recording::InputSelection::SimulatedCallback)
                {
                    self.recorder.note(error.to_string());
                }
                #[cfg(target_os = "linux")]
                if self.recorder.clock() == recording::ClockSelection::NativePodxt
                    && self.record_choice(
                        ui,
                        "input-static-podxt-synthetic",
                        self.recorder.input() == recording::InputSelection::StaticPodxtSynthetic,
                        "PODxt static plan + generated test PCM",
                    )
                    && let Err(error) = self
                        .recorder
                        .select_input(recording::InputSelection::StaticPodxtSynthetic)
                {
                    self.recorder.note(error.to_string());
                }
                if self.record_choice(
                    ui,
                    "input-file",
                    self.recorder.input() == recording::InputSelection::File,
                    "WAV file",
                ) && let Err(error) = self.recorder.select_input(recording::InputSelection::File)
                {
                    self.recorder.note(error.to_string());
                }
                if self.record_button(ui, "load-wav", "Load test WAV…") {
                    self.load_recording_wav();
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if self.record_button(ui, "start", "Start") {
                        self.arm_recording();
                    }
                    if self.record_button(ui, "feed", "Feed chunk")
                        && let Err(error) = self.recorder.feed()
                    {
                        self.recorder.note(error.to_string());
                    }
                    if self.record_button(ui, "stop", "Stop")
                        && let Err(error) = self.recorder.stop()
                    {
                        self.recorder.note(error.to_string());
                    }
                    if self.record_button(ui, "cancel", "Cancel")
                        && let Err(error) = self.recorder.cancel()
                    {
                        self.recorder.note(error.to_string());
                    }
                });
                let label = if self.recorder.take_can_save_aiff() {
                    "Save exact-clock AIFF…"
                } else {
                    "Save WAV copy…"
                };
                if self.record_button(ui, "save", label) {
                    self.save_recording();
                }
                #[cfg(test)]
                {
                    self.recording_visible = format!(
                        "{}\n{}\n{}\n{}\n{}\n{}\n{}",
                        recording::OFFLINE_BANNER,
                        recording::DEVICE_CAPTURE_LOCKED,
                        recording::NO_AUTO_START,
                        self.recorder.phase_label(),
                        self.recorder.meter_line(),
                        self.recorder.guide_line(),
                        self.recorder.message(),
                    );
                }
            });
        self.recording_open = open;
    }

    fn record_choice(
        &mut self,
        ui: &mut egui::Ui,
        id: &'static str,
        selected: bool,
        label: &str,
    ) -> bool {
        let response = ui.selectable_label(selected, label);
        #[cfg(test)]
        self.recording_buttons.insert(id, response.rect);
        #[cfg(not(test))]
        let _ = id;
        response.clicked()
    }

    fn record_button(&mut self, ui: &mut egui::Ui, id: &'static str, label: &str) -> bool {
        let response = ui.button(label);
        #[cfg(test)]
        self.recording_buttons.insert(id, response.rect);
        #[cfg(not(test))]
        let _ = id;
        response.clicked()
    }

    fn arm_recording(&mut self) {
        let Some(route) = self.recorder.route() else {
            self.recorder
                .note("Select a route, an input, and a clock before starting.");
            return;
        };
        let mut attempts = 0;
        let gate = variax_workbench::rig::Session::initial_rig().gate_offline_recording(
            route,
            recording::OFFLINE_ROUTE_PERIOD_BYTES,
            recording::OFFLINE_ROUTE_PERIODS,
            || {
                attempts += 1;
                Ok(())
            },
        );
        #[cfg(test)]
        {
            self.recording_capture_attempts = attempts;
        }
        if attempts != 0 {
            self.recorder
                .note("Capture attempt ran. The take was not started.");
            return;
        }
        match gate {
            Err(error) => {
                self.recorder
                    .note(format!("Route gate refused the take: {error:?}"));
            }
            Ok(gate)
                if gate.stream_opened || gate.outbound_len != 0 || !gate.capture_stays_locked =>
            {
                self.recorder.note("Route plan opened a stream or produced outbound bytes. The take was not started.");
            }
            Ok(_) => {
                if let Err(error) = self.recorder.start() {
                    self.recorder.note(error.to_string());
                }
            }
        }
    }

    fn load_recording_wav(&mut self) {
        if self.recorder.phase() != recording::Phase::Idle {
            self.recorder
                .note("Cancel the current take before loading a file.");
            return;
        }
        let Some(path) = self.dialogs.open_wav() else {
            self.recorder.note("Load cancelled. No file was read.");
            return;
        };
        let label = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "audio.wav".into());
        match recording::read_regular_bytes(
            recording::path_facts(&path),
            recording::TAKE_BYTES_CEILING,
            || fs::read(&path),
        ) {
            Ok(bytes) => {
                if let Err(error) = self.recorder.load_wav(&label, &bytes) {
                    self.recorder.note(error.to_string());
                }
            }
            Err(recording::TakeError::CapExceeded) => {
                self.recorder
                    .note("That file is larger than the offline take ceiling. It was not loaded.");
            }
            Err(error) => self.recorder.note(error.to_string()),
        }
    }

    fn save_recording(&mut self) -> bool {
        if self.recorder.phase() != recording::Phase::Stopped {
            self.recorder.note("Stop the take before saving.");
            return false;
        }
        let native = self.recorder.take_can_save_aiff();
        let destination = if native {
            self.dialogs.save_aiff("exact-clock-test-take.aiff")
        } else {
            self.dialogs.save_wav("offline-test-take.wav")
        };
        let Some(path) = destination else {
            self.recorder.note("Save cancelled. No file was written.");
            return false;
        };
        let result = if native {
            self.recorder.save_aiff(&path).map(|_| ())
        } else {
            self.recorder.save(&path).map(|_| ())
        };
        match result {
            Ok(()) => true,
            Err(error) => {
                self.recorder.note(error.to_string());
                false
            }
        }
    }

    fn hd_quit_dialog(&mut self, ctx: &egui::Context) {
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
                        self.pending_buttons.insert("apply-type8", response.rect);
                        apply = response.clicked();
                        let response = ui.button("Keep editing");
                        #[cfg(test)]
                        self.pending_buttons.insert("cancel", response.rect);
                        cancel = response.clicked();
                        let response = ui.button("Discard staged choice");
                        #[cfg(test)]
                        self.pending_buttons.insert("discard", response.rect);
                        discard = response.clicked();
                    });
                });
            if cancel {
                self.pending = None;
            } else if apply {
                self.apply_hd_type8_choice(
                    staged.file_index,
                    staged.slot,
                    staged.model_id,
                    staged.choice,
                );
                if self.hd_type8_pending.is_none()
                    && !self.hd_library.any_dirty()
                    && !self.pod_library.any_dirty()
                {
                    self.continue_pending(ctx);
                }
            } else if discard {
                self.hd_type8_pending = None;
                self.pod_message = "Discarded the staged HD type-8 choice.".into();
                if !self.hd_library.any_dirty() && !self.pod_library.any_dirty() {
                    self.continue_pending(ctx);
                }
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
                        self.pending_buttons.insert("apply-type9", response.rect);
                        apply = response.clicked();
                        let response = ui.button("Keep editing");
                        #[cfg(test)]
                        self.pending_buttons.insert("cancel", response.rect);
                        cancel = response.clicked();
                        let response = ui.button("Discard staged choice");
                        #[cfg(test)]
                        self.pending_buttons.insert("discard", response.rect);
                        discard = response.clicked();
                    });
                });
            if cancel {
                self.pending = None;
            } else if apply {
                self.apply_hd_type9_choice(
                    staged.file_index,
                    staged.slot,
                    staged.model_id,
                    staged.choice,
                );
                if self.hd_type9_pending.is_none()
                    && !self.hd_library.any_dirty()
                    && !self.pod_library.any_dirty()
                {
                    self.continue_pending(ctx);
                }
            } else if discard {
                self.hd_type9_pending = None;
                self.pod_message = "Discarded the staged HD type-9 choice.".into();
                if !self.hd_library.any_dirty() && !self.pod_library.any_dirty() {
                    self.continue_pending(ctx);
                }
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
                        self.pending_buttons.insert("apply-type10", r.rect);
                        apply = r.clicked();
                        let r = ui.button("Keep editing");
                        #[cfg(test)]
                        self.pending_buttons.insert("cancel", r.rect);
                        cancel = r.clicked();
                        let r = ui.button("Discard staged choice");
                        #[cfg(test)]
                        self.pending_buttons.insert("discard", r.rect);
                        discard = r.clicked();
                    });
                });
            if cancel {
                self.pending = None;
            } else if apply {
                self.apply_hd_type10_choice(
                    staged.file_index,
                    staged.slot,
                    staged.model_id,
                    staged.choice,
                );
                if self.hd_type10_pending.is_none()
                    && !self.hd_library.any_dirty()
                    && !self.pod_library.any_dirty()
                {
                    self.continue_pending(ctx);
                }
            } else if discard {
                self.hd_type10_pending = None;
                self.pod_message = "Discarded the staged HD type-10 choice.".into();
                if !self.hd_library.any_dirty() && !self.pod_library.any_dirty() {
                    self.continue_pending(ctx);
                }
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
                        self.pending_buttons.insert("apply-type11", r.rect);
                        apply = r.clicked();
                        let r = ui.button("Keep editing");
                        #[cfg(test)]
                        self.pending_buttons.insert("cancel", r.rect);
                        cancel = r.clicked();
                        let r = ui.button("Discard staged choice");
                        #[cfg(test)]
                        self.pending_buttons.insert("discard", r.rect);
                        discard = r.clicked();
                    });
                });
            if cancel {
                self.pending = None;
            } else if apply {
                self.apply_hd_type11_choice(staged);
                if self.hd_type11_pending.is_empty()
                    && !self.hd_library.any_dirty()
                    && !self.pod_library.any_dirty()
                {
                    self.continue_pending(ctx);
                }
            } else if discard {
                self.hd_type11_pending
                    .remove(&(staged.file_index, staged.slot, staged.control));
                self.pod_message =
                    format!("Discarded the staged {} choice.", staged.control.label());
                if self.hd_type11_pending.is_empty()
                    && !self.hd_library.any_dirty()
                    && !self.pod_library.any_dirty()
                {
                    self.continue_pending(ctx);
                }
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
                        self.pending_buttons.insert("apply-type12", r.rect);
                        apply = r.clicked();
                        let r = ui.button("Keep editing");
                        #[cfg(test)]
                        self.pending_buttons.insert("cancel", r.rect);
                        cancel = r.clicked();
                        let r = ui.button("Discard staged choice");
                        #[cfg(test)]
                        self.pending_buttons.insert("discard", r.rect);
                        discard = r.clicked();
                    });
                });
            let key = (staged.file_index, staged.slot, staged.control);
            if cancel {
                self.pending = None;
            } else if apply {
                self.apply_hd_type12_choice(staged);
                if self.hd_type12_pending.is_empty()
                    && !self.hd_library.any_dirty()
                    && !self.pod_library.any_dirty()
                {
                    self.continue_pending(ctx);
                }
            } else if discard {
                self.hd_type12_pending.remove(&key);
                self.pod_message = "Discarded the staged HD type-12 choice.".into();
                if self.hd_type12_pending.is_empty()
                    && !self.hd_library.any_dirty()
                    && !self.pod_library.any_dirty()
                {
                    self.continue_pending(ctx);
                }
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
                        #[cfg(test)] self.pending_buttons.insert("apply-type13", r.rect);
                        apply = r.clicked();
                        let r = ui.button("Keep editing");
                        #[cfg(test)] self.pending_buttons.insert("cancel", r.rect);
                        cancel = r.clicked();
                        let r = ui.button("Discard staged choice");
                        #[cfg(test)] self.pending_buttons.insert("discard", r.rect);
                        discard = r.clicked();
                    });
                });
            let key = (staged.file_index, staged.slot, staged.control);
            if cancel {
                self.pending = None;
            } else if apply {
                self.apply_hd_type13_choice(staged);
                if self.hd_type13_pending.is_empty()
                    && !self.hd_library.any_dirty()
                    && !self.pod_library.any_dirty()
                {
                    self.continue_pending(ctx);
                }
            } else if discard {
                self.hd_type13_pending.remove(&key);
                self.pod_message = "Discarded the staged type-0x0D choice.".into();
                if self.hd_type13_pending.is_empty()
                    && !self.hd_library.any_dirty()
                    && !self.pod_library.any_dirty()
                {
                    self.continue_pending(ctx);
                }
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
                        #[cfg(test)] self.pending_buttons.insert("apply-type19", r.rect);
                        apply = r.clicked();
                        let r = ui.button("Keep editing");
                        #[cfg(test)] self.pending_buttons.insert("cancel", r.rect);
                        cancel = r.clicked();
                        let r = ui.button("Discard staged choice");
                        #[cfg(test)] self.pending_buttons.insert("discard", r.rect);
                        discard = r.clicked();
                    });
                });
            let key = (staged.file_index, staged.slot, staged.control);
            if cancel {
                self.pending = None;
            } else if apply {
                self.apply_hd_type19_choice(staged);
                if self.hd_type19_pending.is_empty()
                    && !self.hd_library.any_dirty()
                    && !self.pod_library.any_dirty()
                {
                    self.continue_pending(ctx);
                }
            } else if discard {
                self.hd_type19_pending.remove(&key);
                self.pod_message = "Discarded the staged type-0x13 choice.".into();
                if self.hd_type19_pending.is_empty()
                    && !self.hd_library.any_dirty()
                    && !self.pod_library.any_dirty()
                {
                    self.continue_pending(ctx);
                }
            }
            return;
        }
        let Some(index) = self.hd_library.first_dirty() else {
            self.continue_pending(ctx);
            return;
        };
        let kind = self
            .hd_library
            .file(index)
            .and_then(PodPresetFile::hd_container_kind)
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
                    self.pending_buttons.insert("save", response.rect);
                    save_continue = response.clicked();
                    let response = ui.button("Keep editing");
                    #[cfg(test)]
                    self.pending_buttons.insert("cancel", response.rect);
                    cancel = response.clicked();
                    let response = ui.button("Discard this edit");
                    #[cfg(test)]
                    self.pending_buttons.insert("discard", response.rect);
                    discard = response.clicked();
                });
            });
        if cancel {
            self.pending = None;
        } else if save_continue {
            if self.save_next_dirty_pod()
                && !self.pod_library.any_dirty()
                && !self.hd_library.any_dirty()
            {
                self.continue_pending(ctx);
            }
        } else if discard
            && self.discard_next_dirty_pod()
            && !self.pod_library.any_dirty()
            && !self.hd_library.any_dirty()
        {
            self.continue_pending(ctx);
        }
    }

    fn hd_editor_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("POD HD Edit files · offline");
        ui.label(pod_preset::HD_EDITOR_OFFLINE_NOTICE);
        ui.label("Fixed H5EP/H5ES/H5EB framing and selected source-traced type-1 and numeric controls are supported across nine effect models. Auxiliary widgets and all other rows remain opaque; file association does not prove physical target compatibility.");
        ui.label("No device, audio, firmware or driver operation is available here. Copy saves never replace an existing file.");

        let selected_octave_fuzz = self.hd_selected.and_then(|index| {
            self.hd_library.file(index).and_then(|file| match file {
                PodPresetFile::H5ep(preset) => preset.effect_model_id(self.hd_effect_slot),
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
                if self.pod_button(
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
                                self.pod_message = format!(
                                    "Replaced this offline effect with {} defaults. Nothing was sent; save a new copy to keep the change.",
                                    self.hd_reverb_default_target.label()
                                );
                            }
                            Err(error) => self.pod_message = rejection_text(&error),
                        },
                        Err(error) => self.pod_message = error.detail(),
                    }
                }
            });
            ui.label("Reset five values to Vintage Pre defaults.");
            if self.pod_button(
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
                            self.pod_message = "Replaced this offline effect with Vintage Pre defaults. Nothing was sent; save a new copy to keep the change.".into();
                        }
                        Err(error) => self.pod_message = rejection_text(&error),
                    },
                    Err(error) => self.pod_message = error.detail(),
                }
            }
            ui.label("Reset five values to Screamer defaults.");
            if self.pod_button(
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
                            self.pod_message = "Replaced this offline effect with Screamer defaults. Nothing was sent; save a new copy to keep the change.".into();
                        }
                        Err(error) => self.pod_message = rejection_text(&error),
                    },
                    Err(error) => self.pod_message = error.detail(),
                }
            }
            ui.label("Reset five values to Sub Octave Fuzz defaults.");
            if self.pod_button(
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
                            self.pod_message = "Replaced this offline effect with Sub Octave Fuzz defaults. Nothing was sent; save a new copy to keep the change.".into();
                        }
                        Err(error) => self.pod_message = rejection_text(&error),
                    },
                    Err(error) => self.pod_message = error.detail(),
                }
            }
            ui.label("Replace with Noise Gate defaults.");
            if self.pod_button(ui, "replace-octave-fuzz-noise-gate", "Noise Gate defaults") {
                let index = self.hd_selected.expect("selected HD file");
                match pod_preset::HdFxSlot::new(self.hd_effect_slot) {
                    Ok(slot) => match Session::initial_rig()
                        .replace_hd_octave_fuzz_with_noise_gate_defaults(
                            &mut self.hd_library,
                            index,
                            slot,
                        ) {
                        Ok(()) => {
                            self.pod_message = "Replaced this offline effect with Noise Gate defaults. Nothing was sent; save a new copy to keep the change.".into();
                        }
                        Err(error) => self.pod_message = rejection_text(&error),
                    },
                    Err(error) => self.pod_message = error.detail(),
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
            if self.pod_button(
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
                            self.pod_message = format!(
                                "Replaced this offline effect with {} defaults. Nothing was sent; save a new copy to keep the change.",
                                self.hd_default_target.label()
                            );
                        }
                        Err(error) => self.pod_message = rejection_text(&error),
                    },
                    Err(error) => self.pod_message = error.detail(),
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
            if self.pod_button(
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
                            self.pod_message = format!(
                                "Replaced this offline effect with {} defaults. Nothing was sent; save a new copy to keep the change.",
                                self.hd_dynamics_default_target.label()
                            );
                        }
                        Err(error) => self.pod_message = rejection_text(&error),
                    },
                    Err(error) => self.pod_message = error.detail(),
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
                        file.hd_container_kind()?,
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
            PodPresetFile::H5ep(preset) => {
                ui.label("Single preset. Source header and unmapped body bytes are preserved.");
                let slot_changed = ui
                    .add(egui::Slider::new(&mut self.hd_effect_slot, 0..=7).text("Effect slot"))
                    .changed();
                if slot_changed {
                    if self.hd_type9_pending.take().is_some() {
                        self.pod_message =
                            "Cleared the staged type-9 choice after changing FX slot.".into();
                    }
                    if self.hd_type8_pending.take().is_some() {
                        self.pod_message =
                            "Cleared the staged type-8 choice after changing FX slot.".into();
                    }
                    if self.hd_type10_pending.take().is_some() {
                        self.pod_message =
                            "Cleared the staged type-10 choice after changing FX slot.".into();
                    }
                    if self.hd_selected.is_some_and(|index| {
                        let before = self.hd_type11_pending.len();
                        self.hd_type11_pending
                            .retain(|(file_index, _, _), _| *file_index != index);
                        before != self.hd_type11_pending.len()
                    }) {
                        self.pod_message =
                            "Cleared staged type-11 choices after changing FX slot.".into();
                    }
                    if self.hd_selected.is_some_and(|index| {
                        let before = self.hd_type12_pending.len();
                        self.hd_type12_pending
                            .retain(|(file_index, _, _), _| *file_index != index);
                        before != self.hd_type12_pending.len()
                    }) {
                        self.pod_message =
                            "Cleared staged type-12 choice after changing FX slot.".into();
                    }
                    if self.hd_selected.is_some_and(|index| {
                        let before = self.hd_type13_pending.len();
                        self.hd_type13_pending
                            .retain(|(file_index, _, _), _| *file_index != index);
                        before != self.hd_type13_pending.len()
                    }) {
                        self.pod_message =
                            "Cleared the staged type-0x0D choice after changing FX slot.".into();
                    }
                    if self.hd_selected.is_some_and(|index| {
                        let before = self.hd_type19_pending.len();
                        self.hd_type19_pending
                            .retain(|(file_index, _, _), _| *file_index != index);
                        before != self.hd_type19_pending.len()
                    }) {
                        self.pod_message =
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
                    self.pod_message =
                        "Cleared a stale staged type-8 choice after the selected model changed."
                            .into();
                }
                if self.hd_type9_pending.is_some_and(|pending| {
                    pending.file_index == index
                        && (pending.slot.index() != self.hd_effect_slot
                            || pending.model_id != model_id)
                }) {
                    self.hd_type9_pending = None;
                    self.pod_message =
                        "Cleared a stale staged type-9 choice after the selected model changed."
                            .into();
                }
                if self.hd_type10_pending.is_some_and(|pending| {
                    pending.file_index == index
                        && (pending.slot.index() != self.hd_effect_slot
                            || pending.model_id != model_id)
                }) {
                    self.hd_type10_pending = None;
                    self.pod_message =
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
                                    self.pod_buttons.insert(
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
                    self.pod_buttons
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
                                    self.pod_buttons.insert(
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
                    self.pod_buttons
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
                                    self.pod_buttons.insert(
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
                    self.pod_buttons
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
                                    self.pod_buttons.insert(
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
                    self.pod_buttons
                        .insert(hd_type11_combo_button(control), _combo.response.rect);
                    ui.label(
                        "Choice labels are neutral. Each admitted key is staged independently.",
                    );
                    if matches!(value, pod_preset::HdType11ChoiceValue::NonFinite(_)) {
                        ui.label("A non-finite imported value cannot be replaced by this editor operation.");
                    }
                    if let Some(pending) = staged {
                        ui.horizontal(|ui| {
                            if self.pod_button(
                                ui,
                                hd_type11_apply_button(control),
                                "Apply offline choice",
                            ) {
                                self.apply_hd_type11_choice(pending);
                            }
                            if self.pod_button(
                                ui,
                                hd_type11_discard_button(control),
                                "Discard staged choice",
                            ) {
                                self.hd_type11_pending.remove(&pending_key);
                                self.pod_message =
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
                                    self.pod_buttons
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
                    self.pod_buttons
                        .insert(hd_type12_combo_button(), _combo.response.rect);
                    if matches!(value, pod_preset::HdType12ChoiceValue::NonFinite(_)) {
                        ui.label("A non-finite imported value cannot be replaced by this editor operation.");
                    }
                    if let Some(pending) = staged {
                        ui.horizontal(|ui| {
                            if self.pod_button(ui, hd_type12_apply_button(), "Apply offline choice")
                            {
                                self.apply_hd_type12_choice(pending);
                            }
                            if self.pod_button(
                                ui,
                                hd_type12_discard_button(),
                                "Discard staged choice",
                            ) {
                                self.hd_type12_pending.remove(&pending_key);
                                self.pod_message = "Discarded the staged type-12 choice.".into();
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
                                    self.pod_buttons
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
                    self.pod_buttons
                        .insert("hd-type13-choice-combo", _combo.response.rect);
                    ui.label("Values are the static-derived binary32 words; names, units, and physical mapping remain unknown.");
                    if matches!(value, pod_preset::HdType13ChoiceValue::NonFinite(_)) {
                        ui.label("A non-finite imported value cannot be replaced by this editor operation.");
                    }
                    if let Some(pending) = staged {
                        ui.horizontal(|ui| {
                            if self.pod_button(ui, hd_type13_apply_button(), "Apply offline choice")
                            {
                                self.apply_hd_type13_choice(pending);
                            }
                            if self.pod_button(
                                ui,
                                hd_type13_discard_button(),
                                "Discard staged choice",
                            ) {
                                self.hd_type13_pending.remove(&pending_key);
                                self.pod_message = "Discarded the staged type-0x0D choice.".into();
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
                                    self.pod_buttons
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
                    self.pod_buttons
                        .insert("hd-type19-choice-combo", _combo.response.rect);
                    ui.label("Eight static-derived binary32 candidates; labels, units, and physical mapping remain unknown.");
                    if matches!(value, pod_preset::HdType19ChoiceValue::NonFinite(_)) {
                        ui.label("A non-finite imported value cannot be replaced by this editor operation.");
                    }
                    if let Some(pending) = staged {
                        ui.horizontal(|ui| {
                            if self.pod_button(ui, hd_type19_apply_button(), "Apply offline choice")
                            {
                                self.apply_hd_type19_choice(pending);
                            }
                            if self.pod_button(
                                ui,
                                hd_type19_discard_button(),
                                "Discard staged choice",
                            ) {
                                self.hd_type19_pending.remove(&pending_key);
                                self.pod_message = "Discarded the staged type-0x13 choice.".into();
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
                    if self.pod_button(ui, "apply-hd-mapped-type1-control", "Apply offline control")
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
                        if self.pod_button(ui, "apply-hd-drive", "Apply offline control") {
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
                    if self.pod_button(
                        ui,
                        "apply-hd-numeric-control",
                        "Apply offline numeric control",
                    ) {
                        self.apply_hd_numeric_control(index);
                    }
                }
            }
            PodPresetFile::H5es(setlist) => {
                ui.label(format!("One setlist with {0} fixed preset entries. Extracted children become separate library items; the parent remains unchanged.", pod_preset::H5ES_PRESET_COUNT));
                ui.add(egui::Slider::new(&mut self.hd_preset_index, 0..=63).text("Preset index"));
                if self.pod_button(ui, "extract-hd-child", "Open child as separate preset") {
                    match Session::initial_rig().extract_hd_editor_preset(
                        &mut self.hd_library,
                        index,
                        0,
                        self.hd_preset_index,
                    ) {
                        Ok(child) => self.select_hd_file(child.index),
                        Err(error) => self.pod_message = rejection_text(&error),
                    }
                }
                let _ = setlist;
            }
            PodPresetFile::H5eb(bundle) => {
                ui.label(format!("Bundle with {0} fixed setlists × {1} preset entries. Extracted children become separate library items; the parent remains unchanged.", pod_preset::H5EB_SETLIST_COUNT, pod_preset::H5ES_PRESET_COUNT));
                ui.add(egui::Slider::new(&mut self.hd_setlist_index, 0..=7).text("Setlist index"));
                ui.add(egui::Slider::new(&mut self.hd_preset_index, 0..=63).text("Preset index"));
                if self.pod_button(ui, "extract-hd-child", "Open child as separate preset") {
                    match Session::initial_rig().extract_hd_editor_preset(
                        &mut self.hd_library,
                        index,
                        self.hd_setlist_index,
                        self.hd_preset_index,
                    ) {
                        Ok(child) => self.select_hd_file(child.index),
                        Err(error) => self.pod_message = rejection_text(&error),
                    }
                }
                let _ = bundle;
            }
            PodPresetFile::PodxtLive(_) => return,
        }
        ui.label(&self.pod_message);
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
            self.pod_message =
                "Cleared a staged type-9 choice after changing HD file selection.".into();
        }
        if self.hd_type8_pending.take().is_some() {
            self.pod_message =
                "Cleared a staged type-8 choice after changing HD file selection.".into();
        }
        if self.hd_type10_pending.take().is_some() {
            self.pod_message =
                "Cleared a staged type-10 choice after changing HD file selection.".into();
        }
        self.hd_selected = Some(index);
        self.hd_effect_slot = 0;
        self.hd_control = self
            .hd_library
            .file(index)
            .and_then(|file| match file {
                PodPresetFile::H5ep(preset) => {
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
                PodPresetFile::H5ep(preset) => preset.hd_type1_control_percent(0, self.hd_control),
                _ => None,
            })
            .unwrap_or(0);
        self.hd_mapped_control = self
            .hd_library
            .file(index)
            .and_then(|file| match file {
                PodPresetFile::H5ep(preset) => {
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
                PodPresetFile::H5ep(preset) => {
                    preset.hd_type1_mapped_control_percent(0, self.hd_mapped_control)
                }
                _ => None,
            })
            .unwrap_or(self.hd_control_percent);
        self.hd_numeric_control = self
            .hd_library
            .file(index)
            .and_then(|file| match file {
                PodPresetFile::H5ep(preset) => {
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
                PodPresetFile::H5ep(preset) => {
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
                self.pod_message =
                    "Updated only the traced in-memory value. Nothing was sent.".into();
            }
            Err(error) => self.pod_message = rejection_text(&error),
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
                self.pod_message =
                    "Updated only the traced in-memory value. Nothing was sent.".into();
            }
            Err(error) => self.pod_message = rejection_text(&error),
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
                self.pod_message =
                    "Updated only the traced in-memory value. Nothing was sent.".into();
            }
            Err(error) => self.pod_message = rejection_text(&error),
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
                pod_preset::PodPresetFile::H5ep(preset) => preset.effect_model_id(slot.index()),
                _ => None,
            });
        if current_model_id != Some(model_id) {
            if self
                .hd_type9_pending
                .is_some_and(|pending| pending.file_index == file_index && pending.slot == slot)
            {
                self.hd_type9_pending = None;
            }
            self.pod_message =
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
                self.pod_message = "Applied the type-9 choice to this offline H5EP entry. Nothing was sent; save a new copy to keep the change.".into();
            }
            Err(error) => self.pod_message = rejection_text(&error),
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
                pod_preset::PodPresetFile::H5ep(preset) => preset.effect_model_id(slot.index()),
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
            self.pod_message =
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
                self.pod_message = "Applied the type-8 two-choice value to this offline H5EP entry. Nothing was sent; save a new copy to keep the change.".into();
            }
            Err(error) => self.pod_message = rejection_text(&error),
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
            PodPresetFile::H5ep(p) => p.effect_model_id(slot.index()),
            _ => None,
        });
        if current != Some(model_id) || pod_preset::hd_type10_choice_key(model_id).is_none() {
            if self
                .hd_type10_pending
                .is_some_and(|p| p.file_index == file_index && p.slot == slot)
            {
                self.hd_type10_pending = None;
            }
            self.pod_message =
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
                self.pod_message = "Applied the type-10 choice to this offline H5EP entry. Nothing was sent; save a new copy to keep the change.".into();
            }
            Err(error) => self.pod_message = rejection_text(&error),
        }
    }

    fn apply_hd_type11_choice(&mut self, pending: PendingHdType11Choice) {
        let key = (pending.file_index, pending.slot, pending.control);
        let current_model = self
            .hd_library
            .file(pending.file_index)
            .and_then(|file| match file {
                PodPresetFile::H5ep(preset) => preset.effect_model_id(pending.slot.index()),
                _ => None,
            });
        if current_model != Some(pending.model_id)
            || pending.model_id != pending.control.model_id()
            || !pod_preset::HD_TYPE11_CHOICE_MODEL_KEYS
                .contains(&(pending.control.model_id(), pending.control.key()))
        {
            self.hd_type11_pending.remove(&key);
            self.pod_message = format!(
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
                self.pod_message = format!(
                    "Applied the {} menu value to this offline H5EP entry. Nothing was sent; save a new copy to keep the change.",
                    pending.control.label()
                );
            }
            Err(error) => self.pod_message = rejection_text(&error),
        }
    }

    fn apply_hd_type12_choice(&mut self, pending: PendingHdType12Choice) {
        let key = (pending.file_index, pending.slot, pending.control);
        let current_model = self
            .hd_library
            .file(pending.file_index)
            .and_then(|file| match file {
                PodPresetFile::H5ep(preset) => preset.effect_model_id(pending.slot.index()),
                _ => None,
            });
        if current_model != Some(pending.model_id) || pending.model_id != pending.control.model_id()
        {
            self.hd_type12_pending.remove(&key);
            self.pod_message =
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
                self.pod_message = "Applied the type-12 menu value to this offline H5EP entry. Nothing was sent; save a new copy to keep the change.".into();
            }
            Err(error) => self.pod_message = rejection_text(&error),
        }
    }

    fn apply_hd_type13_choice(&mut self, pending: PendingHdType13Choice) {
        let key = (pending.file_index, pending.slot, pending.control);
        let current_model = self
            .hd_library
            .file(pending.file_index)
            .and_then(|file| match file {
                PodPresetFile::H5ep(preset) => preset.effect_model_id(pending.slot.index()),
                _ => None,
            });
        if current_model != Some(pending.model_id) || pending.model_id != pending.control.model_id()
        {
            self.hd_type13_pending.remove(&key);
            self.pod_message =
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
                self.pod_message = "Applied the static-derived type-0x0D choice to this offline H5EP entry. Nothing was sent; save a new copy to keep the change.".into();
            }
            Err(error) => self.pod_message = rejection_text(&error),
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
                PodPresetFile::H5ep(preset) => preset.effect_model_id(pending.slot.index()),
                _ => None,
            });
        if current_model != Some(pending.model_id) || pending.model_id != pending.control.model_id()
        {
            self.hd_type19_pending.remove(&key);
            self.pod_message =
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
                self.pod_message = "Applied the static-derived type-0x13 choice to this offline H5EP entry. Nothing was sent; save a new copy to keep the change.".into();
            }
            Err(error) => self.pod_message = rejection_text(&error),
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

    fn load_hd_editor_file(&mut self) {
        let Some(path) = self.dialogs.open_pod_tone() else {
            self.pod_message = "Load cancelled. No file was read.".into();
            return;
        };
        let facts = pod_preset::path_facts(&path);
        let bytes = match facts {
            Ok(facts) if facts.len <= pod_preset::TONE_BYTES_CEILING => {
                pod_preset::read_regular_tone(Ok(facts), || pod_preset::open_regular_file(&path))
            }
            Ok(facts) => pod_preset::read_regular_hd_editor_file(Ok(facts), || {
                pod_preset::open_regular_file(&path)
            }),
            Err(error) => Err(pod_preset::ToneError::Format {
                detail: error.to_string(),
            }),
        };
        match bytes.and_then(|bytes| {
            let parsed = PodPresetFile::parse(bytes.clone())?;
            Ok((bytes, parsed))
        }) {
            Ok((bytes, PodPresetFile::PodxtLive(_)))
                if bytes.len() as u64 <= pod_preset::TONE_BYTES_CEILING =>
            {
                match Session::initial_rig().open_pod_preset(RoleKind::Pod, &bytes) {
                    Ok(summary) => match self.pod_library.import_bytes(bytes) {
                        Ok(index) => {
                            self.pod_selected = Some(index);
                            self.pod_name = summary.name;
                            self.pod_message = format!(
                                "Loaded {} for PODxt Live {:08x}. {}",
                                summary.container,
                                summary.device_id,
                                pod_preset::PAYLOADS_NOT_DECODED
                            );
                        }
                        Err(error) => self.pod_message = error.detail(),
                    },
                    Err(error) => self.pod_message = rejection_text(&error),
                }
            }
            Ok((bytes, _)) => {
                match Session::initial_rig().import_hd_editor_file(&mut self.hd_library, bytes) {
                    Ok(summary) => {
                        self.select_hd_file(summary.index);
                        self.pod_message = "Loaded an offline HD Edit file. Internal product target and physical compatibility remain unverified.".into();
                    }
                    Err(error) => self.pod_message = rejection_text(&error),
                }
            }
            Err(error) => self.pod_message = error.detail(),
        }
    }

    fn write_hd_copy(&mut self, index: usize) -> bool {
        let Some(file) = self.hd_library.file(index) else {
            self.pod_message = "Select an HD Edit file before saving a copy.".into();
            return false;
        };
        let suggested = match file.hd_container_kind() {
            Some("H5ES setlist") => "hd-edit-setlist.5xs",
            Some("H5EB bundle") => "hd-edit-bundle.5xb",
            _ => "hd-edit-preset.5xe",
        };
        let Some(path) = self.dialogs.save_pod_tone(suggested) else {
            self.pod_message = "Save cancelled. No file was written.".into();
            return false;
        };
        match Session::initial_rig().save_hd_editor_copy(&self.hd_library, &path, index) {
            Ok(()) => match self.hd_library.note_saved(index) {
                Ok(()) => {
                    self.pod_message = format!(
                        "Saved a new offline copy to {}. No device operation occurred.",
                        path.display()
                    );
                    true
                }
                Err(error) => {
                    self.pod_message = error.detail();
                    false
                }
            },
            Err(error) => {
                self.pod_message = rejection_text(&error);
                false
            }
        }
    }
}

impl Drop for Workbench {
    fn drop(&mut self) {
        if let Some((cancel, worker)) = self.read_worker.take() {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            let _ = worker.join();
        }
    }
}

const USAGE: &str = "Usage: variax-workbench-gui [FILE | --preset FILE] [--data-dir DIRECTORY] [--artwork EDITOR_IMAGE_FOLDER] [--tab body|pickups|controls] [--display auto|x11|wayland (Linux)] [--screenshot OUTPUT.png]\n       variax-workbench-gui --version";

fn save_screenshot(path: &Path, pixels: &[u8], width: u32, height: u32) -> Result<(), String> {
    use image::ImageEncoder;
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(pixels, width, height, image::ColorType::Rgba8.into())
        .map_err(|e| e.to_string())?;
    variax_workbench::document::save_new(path, &bytes)
}

struct GuiOptions {
    preset: Option<PathBuf>,
    artwork: PathBuf,
    preferences: PathBuf,
    screenshot: Option<PathBuf>,
    tab: Option<Tab>,
    display: String,
}

/// Original preference key `variax_editor.auto_select_control_values`.
/// A missing file stays on. `1` is on and `0` is off.
fn load_auto_select(path: &Path) -> bool {
    !matches!(std::fs::read_to_string(path), Ok(text) if text.trim() == "0")
}

fn save_auto_select(path: &Path, enabled: bool) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
    temporary
        .write_all(if enabled { b"1\n" } else { b"0\n" })
        .map_err(|error| error.to_string())?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    temporary
        .persist(path)
        .map_err(|error| error.error.to_string())?;
    Ok(())
}

fn editor_preferences_path(data_root: Option<&Path>) -> PathBuf {
    let absolute_env = |key: &str| {
        std::env::var_os(key)
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
    };
    let root = data_root.map(Path::to_path_buf).unwrap_or_else(|| {
        if let Some(path) = absolute_env("VARIAX_WORKBENCH_DATA_DIR") {
            return path;
        }
        platform_data_root(absolute_env)
    });
    root.join("editor-preferences.txt")
}

fn parse_options(
    arguments: impl IntoIterator<Item = String>,
    default_artwork: &Path,
) -> Result<GuiOptions, String> {
    let mut options = GuiOptions {
        preset: None,
        artwork: default_artwork.to_owned(),
        preferences: editor_preferences_path(None),
        screenshot: None,
        tab: None,
        display: "auto".into(),
    };
    let mut data_root = None;
    let mut artwork = None;
    let mut args = arguments.into_iter();
    while let Some(arg) = args.next() {
        if !arg.starts_with('-') {
            if options.preset.replace(PathBuf::from(arg)).is_some() {
                return Err("Open one file at a time".into());
            }
            continue;
        }
        if !matches!(
            arg.as_str(),
            "--preset" | "--data-dir" | "--artwork" | "--screenshot" | "--tab" | "--display"
        ) {
            return Err(format!("Unknown argument: {arg}"));
        }
        let value = args
            .next()
            .filter(|value| !value.starts_with("--"))
            .ok_or_else(|| format!("{arg} requires a value"))?;
        match arg.as_str() {
            "--preset" => {
                if options.preset.replace(PathBuf::from(value)).is_some() {
                    return Err("Open one file at a time".into());
                }
            }
            "--data-dir" => data_root = Some(PathBuf::from(value)),
            "--artwork" => artwork = Some(PathBuf::from(value)),
            "--display" => {
                if !matches!(value.as_str(), "auto" | "x11" | "wayland") {
                    return Err(format!("Unknown display backend: {value}"));
                }
                if !cfg!(target_os = "linux") && value != "auto" {
                    return Err("Explicit X11/Wayland selection is Linux-only".into());
                }
                options.display = value;
            }
            "--screenshot" => {
                let path = PathBuf::from(value);
                if path.extension().and_then(|e| e.to_str()) != Some("png") {
                    return Err("Screenshot output must have a .png extension".into());
                }
                options.screenshot = Some(path);
            }
            "--tab" => {
                options.tab = Some(match value.as_str() {
                    "body" => Tab::Body,
                    "pickups" => Tab::Pickups,
                    "tuning" => Tab::Pickups, // Tuning is always visible.
                    "controls" => Tab::Controls,
                    _ => return Err(format!("Unknown editor tab: {value}")),
                });
            }
            _ => unreachable!(),
        }
    }
    options.preferences = editor_preferences_path(data_root.as_deref());
    if let Some(artwork) = artwork.or_else(|| data_root.map(|root| root.join("artwork"))) {
        options.artwork = artwork;
    }
    Ok(options)
}

fn default_artwork() -> PathBuf {
    let absolute_env = |key: &str| {
        std::env::var_os(key)
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
    };
    if let Some(path) = absolute_env("VARIAX_WORKBENCH_DATA_DIR") {
        return path.join("artwork");
    }
    let installed = platform_data_root(absolute_env).join("artwork");
    default_artwork_from_checkout(installed, Path::new(env!("CARGO_MANIFEST_DIR")))
}

fn default_artwork_from_checkout(installed: PathBuf, checkout: &Path) -> PathBuf {
    // A launcher can start the binary from outside the source checkout.
    let extracted = checkout.join("assets/original-ui/modules/variax/editor/images");
    if !installed.is_dir() && extracted.is_dir() {
        return extracted;
    }
    installed
}

fn platform_data_root(absolute_env: impl Fn(&str) -> Option<PathBuf>) -> PathBuf {
    #[cfg(target_os = "windows")]
    if let Some(path) = absolute_env("LOCALAPPDATA") {
        return path.join("VariaxWorkbench");
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    if let Some(path) = absolute_env("XDG_DATA_HOME") {
        return path.join("variax-workbench");
    }
    if let Some(home) = absolute_env("HOME") {
        #[cfg(target_os = "macos")]
        return home.join("Library/Application Support/VariaxWorkbench");
        #[cfg(not(target_os = "macos"))]
        return home.join(".local/share/variax-workbench");
    }
    // This is only an optional artwork lookup; never create or write this path.
    std::env::temp_dir().join("variax-workbench")
}

pub(crate) fn run(title: &str) -> eframe::Result {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments == ["--version"] {
        println!("{title}");
        return Ok(());
    }
    if arguments.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{USAGE}");
        return Ok(());
    }
    let arguments = match parse_options(arguments, &default_artwork()) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}\n{USAGE}");
            std::process::exit(1);
        }
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_app_id("variax-workbench")
            .with_inner_size([1120.0, 760.0])
            .with_min_inner_size([960.0, 680.0]),
        ..Default::default()
    };
    #[cfg(target_os = "linux")]
    let options = {
        let mut options = options;
        let backend = arguments.display.clone();
        options.event_loop_builder = Some(Box::new(move |builder| match backend.as_str() {
            "x11" => {
                use winit::platform::x11::EventLoopBuilderExtX11;
                builder.with_x11();
            }
            "wayland" => {
                use winit::platform::wayland::EventLoopBuilderExtWayland;
                builder.with_wayland();
            }
            _ => {}
        }));
        options
    };
    #[cfg(not(target_os = "linux"))]
    let _ = &arguments.display;
    // Craft Anything saves the launcher's output in its play log.
    eprintln!("Opening {title}");
    eframe::run_native(
        title,
        options,
        Box::new(move |cc| {
            let mut app = Workbench::with_context(
                &cc.egui_ctx,
                arguments.preset,
                arguments.artwork,
                arguments.tab,
            );
            app.auto_select_controls = load_auto_select(&arguments.preferences);
            app.preferences_path = Some(arguments.preferences);
            app.screenshot = arguments.screenshot;
            Ok(Box::new(app))
        }),
    )
}

#[cfg(test)]
mod tests {
    pub(super) use super::podxt_panel::tests::sample_pod_tone;
    use super::*;

    fn hd_editor_frame(context: &egui::Context, app: &mut Workbench, events: Vec<egui::Event>) {
        // 820px leaves the HD scroll area shorter than its content, so the
        // idle-scroll fixture can move the type-9 control fully off screen.
        let _ = context.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 820.0))),
                events,
                ..Default::default()
            },
            |ctx| app.render(ctx),
        );
    }

    fn click_hd_editor(context: &egui::Context, app: &mut Workbench, id: &'static str) {
        hd_editor_frame(context, app, vec![]);
        if !matches!(
            id,
            "apply-hd-type9-choice"
                | "discard-hd-type9-choice"
                | "apply-hd-type10-choice"
                | "discard-hd-type10-choice"
                | "save-hd-copy"
                | "load-hd-file"
                | "hd-type9-choice-option-0"
                | "hd-type9-choice-option-1"
                | "hd-type9-choice-option-2"
                | "hd-type10-choice-option-0"
                | "hd-type10-choice-option-1"
                | "hd-type13-choice-option-0"
                | "hd-type13-choice-option-1"
                | "hd-type13-choice-option-2"
                | "hd-type13-choice-option-3"
                | "hd-type19-choice-option-0"
                | "hd-type19-choice-option-1"
                | "hd-type19-choice-option-2"
                | "hd-type19-choice-option-3"
                | "hd-type19-choice-option-4"
                | "hd-type19-choice-option-5"
                | "hd-type19-choice-option-6"
                | "hd-type19-choice-option-7"
        ) {
            scroll_hd_editor_control_into_view(context, app, id);
            hd_editor_frame(context, app, vec![]);
        }
        let pos = app
            .pod_buttons
            .get(id)
            .unwrap_or_else(|| {
                panic!(
                    "expected rendered POD button {id:?}; registered: {:?}",
                    app.pod_buttons.keys().collect::<Vec<_>>()
                )
            })
            .center();
        hd_editor_frame(
            context,
            app,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        hd_editor_frame(
            context,
            app,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
    }

    fn scroll_hd_editor_control_into_view(
        context: &egui::Context,
        app: &Workbench,
        id: &'static str,
    ) {
        let rect = app.pod_buttons[id];
        let (scroll_id, viewport, content_height) =
            app.pod_scroll_area.expect("rendered HD editor scroll area");
        let mut state = egui::scroll_area::State::load(context, scroll_id).unwrap_or_default();
        let max_offset = (content_height - viewport.height()).max(0.0);
        state.offset.y =
            (state.offset.y + rect.center().y - viewport.center().y).clamp(0.0, max_offset);
        state.store(context, scroll_id);
    }

    fn click_hd_type9_choice(context: &egui::Context, app: &mut Workbench, choice: usize) {
        click_hd_editor(context, app, "hd-type9-choice-combo");
        let option_id = match choice {
            0 => "hd-type9-choice-option-0",
            1 => "hd-type9-choice-option-1",
            2 => "hd-type9-choice-option-2",
            _ => panic!("invalid type-9 choice index"),
        };
        click_hd_editor(context, app, option_id);
    }

    fn click_hd_type8_choice(context: &egui::Context, app: &mut Workbench, choice: usize) {
        click_hd_editor(context, app, "hd-type8-choice-combo");
        let option_id = match choice {
            0 => "hd-type8-choice-option-0",
            1 => "hd-type8-choice-option-1",
            _ => panic!("invalid type-8 choice index"),
        };
        click_hd_editor(context, app, option_id);
    }

    fn click_hd_type10_choice(context: &egui::Context, app: &mut Workbench, choice: usize) {
        click_hd_editor(context, app, "hd-type10-choice-combo");
        let option_id = if choice == 0 {
            "hd-type10-choice-option-0"
        } else if choice == 1 {
            "hd-type10-choice-option-1"
        } else {
            panic!("invalid type-10 choice index")
        };
        click_hd_editor(context, app, option_id);
    }

    fn click_hd_type11_choice(
        context: &egui::Context,
        app: &mut Workbench,
        control: pod_preset::HdType11Control,
        choice: usize,
    ) {
        let choice = pod_preset::HdType11Choice::from_index(choice as u8).unwrap();
        click_hd_editor(context, app, hd_type11_combo_button(control));
        click_hd_editor(context, app, hd_type11_option_button(control, choice));
    }

    fn click_hd_type12_choice(context: &egui::Context, app: &mut Workbench, choice: usize) {
        let choice = pod_preset::HdType12Choice::from_index(choice as u8).unwrap();
        click_hd_editor(context, app, hd_type12_combo_button());
        click_hd_editor(context, app, hd_type12_option_button(choice));
    }

    fn click_hd_type13_choice(context: &egui::Context, app: &mut Workbench, choice: usize) {
        let choice = pod_preset::HdType13Choice::from_index(choice as u8).unwrap();
        click_hd_editor(context, app, "hd-type13-choice-combo");
        click_hd_editor(context, app, hd_type13_option_button(choice));
    }

    fn click_hd_type19_choice(context: &egui::Context, app: &mut Workbench, choice: usize) {
        let choice = pod_preset::HdType19Choice::from_index(choice as u8).unwrap();
        click_hd_editor(context, app, "hd-type19-choice-combo");
        click_hd_editor(context, app, hd_type19_option_button(choice));
    }

    fn assert_hd_bytes_eq(actual: &[u8], expected: &[u8], context: &str) {
        let differences = actual
            .iter()
            .zip(expected)
            .enumerate()
            .filter_map(|(offset, (actual, expected))| {
                (actual != expected).then_some((offset, *actual, *expected))
            })
            .take(8)
            .collect::<Vec<_>>();
        assert!(
            actual.len() == expected.len() && differences.is_empty(),
            "{context}: actual length {}, expected length {}, first differences (offset, actual, expected): {differences:?}",
            actual.len(),
            expected.len()
        );
    }

    fn sample_hd_editor_preset() -> Vec<u8> {
        let mut bytes = vec![0; pod_preset::H5EP_MIN_BYTES];
        bytes[..4].copy_from_slice(b"H5EP");
        bytes[4..8].copy_from_slice(&1u32.to_be_bytes());
        let body = &mut bytes[pod_preset::H5EP_HEADER_BYTES..];
        for offset in [0x228, 0x328] {
            body[offset..offset + 4].copy_from_slice(&0x0100_0000u32.to_be_bytes());
        }
        for index in 0..8 {
            let block = 0x428 + index * 0x100;
            body[block..block + 4].copy_from_slice(&0x0200_0000u32.to_be_bytes());
        }
        let block = 0x428;
        body[block..block + 4].copy_from_slice(&0x0205_000du32.to_be_bytes());
        let row = block + 0x10;
        body[row..row + 4].copy_from_slice(&0x3f10_0001u32.to_be_bytes());
        body[row + 4..row + 8].copy_from_slice(&0.5f32.to_bits().to_be_bytes());
        let bass_row = row + 0x14;
        body[bass_row..bass_row + 4].copy_from_slice(&0x3f10_0002u32.to_be_bytes());
        body[bass_row + 4..bass_row + 8].copy_from_slice(&0.25f32.to_bits().to_be_bytes());
        let mid_row = bass_row + 0x14;
        body[mid_row..mid_row + 4].copy_from_slice(&0x3f10_0003u32.to_be_bytes());
        body[mid_row + 4..mid_row + 8].copy_from_slice(&0.35f32.to_bits().to_be_bytes());
        bytes
    }

    fn sample_hd_type9_preset(slot: usize, row: usize, bits: u32) -> Vec<u8> {
        sample_hd_type9_preset_for(
            slot,
            row,
            bits,
            pod_preset::HD_TYPE9_CHOICE_MODEL_ID,
            0x3f10_0004,
        )
    }

    fn sample_hd_type9_preset_for(
        slot: usize,
        row: usize,
        bits: u32,
        model_id: u32,
        key: u32,
    ) -> Vec<u8> {
        let mut bytes = sample_hd_editor_preset();
        let block = pod_preset::H5EP_HEADER_BYTES + 0x428 + slot * 0x100;
        bytes[block..block + 4].copy_from_slice(&model_id.to_be_bytes());
        bytes[block + 8] = 0;
        bytes[block + 0x0b] = 1;
        bytes[block + 0x0f] = 12;
        let row_at = block + 0x10 + row * 0x14;
        bytes[row_at..row_at + 4].copy_from_slice(&key.to_be_bytes());
        bytes[row_at + 4..row_at + 8].copy_from_slice(&bits.to_be_bytes());
        bytes[row_at + 8..row_at + 0x10].fill(0x31);
        bytes[row_at + 0x10..row_at + 0x14].copy_from_slice(&[0, 0x42, 0x43, 0x44]);
        bytes.extend_from_slice(b"type9-tail");
        bytes
    }

    fn sample_hd_type8_preset(slot: usize, row: usize, bits: u32) -> Vec<u8> {
        sample_hd_type9_preset_for(
            slot,
            row,
            bits,
            pod_preset::HD_TYPE8_CHOICE_MODEL_ID,
            pod_preset::HD_TYPE8_CHOICE_KEY,
        )
    }

    fn sample_hd_type10_preset_for(
        slot: usize,
        row: usize,
        bits: u32,
        model_id: u32,
        key: u32,
    ) -> Vec<u8> {
        let mut bytes = sample_hd_type9_preset_for(slot, row, bits, model_id, key);
        let block = pod_preset::H5EP_HEADER_BYTES + 0x428 + slot * 0x100;
        let other_row = (row + 1) % 12;
        let at = block + 0x10 + other_row * 0x14;
        bytes[at..at + 4].copy_from_slice(&0x3f10_0004u32.to_be_bytes());
        bytes[at + 4..at + 8].copy_from_slice(&0x0000_0000u32.to_be_bytes());
        bytes
    }

    fn sample_hd_type11_preset(model_id: u32) -> Vec<u8> {
        let controls = pod_preset::HdType11Control::for_model(model_id);
        assert_eq!(controls.len(), 2);
        let mut bytes = sample_hd_type9_preset_for(5, 3, 0x3e12_3456, model_id, controls[0].key());
        let block = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100;
        if model_id == pod_preset::HD_TYPE12_CHOICE_MODEL_ID {
            let type12 = block + 0x10 + 4 * 0x14;
            bytes[type12..type12 + 4]
                .copy_from_slice(&pod_preset::HD_TYPE12_CHOICE_KEY.to_be_bytes());
            bytes[type12 + 4..type12 + 8].copy_from_slice(&0x3e65_9999u32.to_be_bytes());
        }
        if model_id == pod_preset::HD_TYPE13_CHOICE_MODEL_ID {
            let type13 = block + 0x10 + 4 * 0x14;
            bytes[type13..type13 + 4]
                .copy_from_slice(&pod_preset::HD_TYPE13_CHOICE_KEY.to_be_bytes());
            bytes[type13 + 4..type13 + 8].copy_from_slice(&0x3eaaaaaau32.to_be_bytes());
        }
        let second = block + 0x10 + 8 * 0x14;
        bytes[second..second + 4].copy_from_slice(&controls[1].key().to_be_bytes());
        bytes[second + 4..second + 8].copy_from_slice(&0x3e65_4321u32.to_be_bytes());
        bytes
    }

    #[test]
    fn hd_type11_gui_reaches_all_five_choices_on_all_four_explicit_controls() {
        for model_id in [0x020a_001e, 0x020a_001d] {
            let controls = pod_preset::HdType11Control::for_model(model_id);
            let (context, mut app) = app();
            context.style_mut(|style| style.animation_time = 0.0);
            let original = sample_hd_type11_preset(model_id);
            let index = app.hd_library.import_bytes(original.clone()).unwrap();
            app.select_hd_file(index);
            app.pod_open = true;
            app.hd_effect_slot = 5;
            let slot = pod_preset::HdFxSlot::new(5).unwrap();
            for control in controls.iter().copied() {
                for choice in pod_preset::HdType11Choice::ALL {
                    hd_editor_frame(&context, &mut app, vec![]);
                    click_hd_type11_choice(&context, &mut app, control, choice.index() as usize);
                    let expected = PendingHdType11Choice {
                        file_index: index,
                        slot,
                        model_id,
                        control,
                        choice,
                    };
                    assert_eq!(
                        app.hd_type11_pending.get(&(index, slot, control)),
                        Some(&expected),
                        "{model_id:#010x} / {:#010x} / {choice:?}",
                        control.key()
                    );
                    assert_eq!(app.hd_type11_pending.len(), 1);
                    assert_hd_bytes_eq(
                        app.hd_library.file(index).unwrap().as_bytes(),
                        &original,
                        "type-11 menu stages without changing bytes",
                    );
                    click_hd_editor(&context, &mut app, hd_type11_discard_button(control));
                    assert!(app.hd_type11_pending.is_empty());
                    assert_hd_bytes_eq(
                        app.hd_library.file(index).unwrap().as_bytes(),
                        &original,
                        "discarding one staged type-11 choice leaves source bytes",
                    );
                }
            }
        }
    }

    #[test]
    fn hd_type19_rendered_combo_stages_applies_and_discards_all_eight_exact_words() {
        let control = pod_preset::HdType19Control::ALL[0];
        let slot = pod_preset::HdFxSlot::new(5).unwrap();
        let model = pod_preset::HD_TYPE19_CHOICE_MODEL_ID;
        for choice in pod_preset::HdType19Choice::ALL {
            let (context, mut app) = app();
            context.style_mut(|style| style.animation_time = 0.0);
            let original = sample_hd_type9_preset_for(
                5,
                9,
                0x3e12_3456,
                model,
                pod_preset::HD_TYPE19_CHOICE_KEY,
            );
            let index = app.hd_library.import_bytes(original.clone()).unwrap();
            app.select_hd_file(index);
            app.pod_open = true;
            app.hd_effect_slot = 5;
            click_hd_type19_choice(&context, &mut app, choice.index() as usize);
            let staged = PendingHdType19Choice {
                file_index: index,
                slot,
                model_id: model,
                control,
                choice,
            };
            assert_eq!(
                app.hd_type19_pending.get(&(index, slot, control)),
                Some(&staged)
            );
            assert_hd_bytes_eq(
                app.hd_library.file(index).unwrap().as_bytes(),
                &original,
                "type-0x13 selection stages without changing bytes",
            );
            click_hd_editor(&context, &mut app, hd_type19_apply_button());
            let value = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + 9 * 0x14 + 4;
            let mut expected = original.clone();
            expected[value..value + 4].copy_from_slice(&choice.bits().to_be_bytes());
            assert_hd_bytes_eq(
                app.hd_library.file(index).unwrap().as_bytes(),
                &expected,
                "type-0x13 Apply changes only the selected word",
            );
            assert!(app.hd_type19_pending.is_empty());
            click_hd_type19_choice(&context, &mut app, ((choice.index() + 1) % 8) as usize);
            click_hd_editor(&context, &mut app, hd_type19_discard_button());
            assert!(app.hd_type19_pending.is_empty());
            assert_hd_bytes_eq(
                app.hd_library.file(index).unwrap().as_bytes(),
                &expected,
                "discard preserves applied bytes",
            );
        }
    }

    #[test]
    fn hd_type19_quit_handles_stage_only_apply_discard_keep_and_rejected_apply() {
        let model = pod_preset::HD_TYPE19_CHOICE_MODEL_ID;
        let control = pod_preset::HdType19Control::ALL[0];
        let slot = pod_preset::HdFxSlot::new(5).unwrap();
        let source = sample_hd_type9_preset_for(5, 9, 0x3e12_3456, model, control.key());
        let pending = |index| PendingHdType19Choice {
            file_index: index,
            slot,
            model_id: model,
            control,
            choice: pod_preset::HdType19Choice::Choice5,
        };

        for action in ["keep", "discard", "apply", "reject"] {
            let (context, mut app) = app();
            context.style_mut(|style| style.animation_time = 0.0);
            let index = app.hd_library.import_bytes(source.clone()).unwrap();
            app.select_hd_file(index);
            app.pod_open = true;
            app.hd_effect_slot = 5;
            click_hd_type19_choice(&context, &mut app, 5);
            let exact = pending(index);
            assert_eq!(
                app.hd_type19_pending.get(&(index, slot, control)),
                Some(&exact)
            );
            let mut before_quit = source.clone();
            if action == "reject" {
                let marker =
                    pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + 9 * 0x14 + 0x10;
                before_quit[marker] = 1;
                *app.hd_library.file_mut(index).unwrap() =
                    PodPresetFile::parse(before_quit.clone()).unwrap();
                assert!(
                    matches!(
                        app.hd_library.file(index),
                        Some(PodPresetFile::H5ep(preset)) if preset.effect_model_id(5) == Some(model)
                    ),
                    "reject case retains model identity"
                );
            }
            assert_hd_bytes_eq(
                app.hd_library.file(index).unwrap().as_bytes(),
                &before_quit,
                "stage-only quit setup bytes",
            );
            app.request_close(&context);
            assert!(matches!(app.pending, Some(Pending::QuitPod)));
            assert_eq!(
                app.hd_type19_pending.get(&(index, slot, control)),
                Some(&exact)
            );
            assert!(!app.permit_close);

            match action {
                "keep" => {
                    click_prompt(&context, &mut app, "cancel");
                    assert!(app.pending.is_none());
                    assert_eq!(
                        app.hd_type19_pending.get(&(index, slot, control)),
                        Some(&exact)
                    );
                    assert_hd_bytes_eq(
                        app.hd_library.file(index).unwrap().as_bytes(),
                        &source,
                        "Keep editing preserves stage-only bytes",
                    );
                    assert!(!app.permit_close);
                }
                "discard" => {
                    click_prompt(&context, &mut app, "discard");
                    assert!(app.hd_type19_pending.is_empty());
                    assert!(
                        app.permit_close,
                        "stage discard closes only after clearing the stage"
                    );
                    assert_hd_bytes_eq(
                        app.hd_library.file(index).unwrap().as_bytes(),
                        &source,
                        "stage-only discard leaves bytes unchanged",
                    );
                }
                "apply" => {
                    click_prompt(&context, &mut app, "apply-type19");
                    let value =
                        pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + 9 * 0x14 + 4;
                    let mut expected = source.clone();
                    expected[value..value + 4].copy_from_slice(&exact.choice.bits().to_be_bytes());
                    assert!(app.hd_type19_pending.is_empty());
                    assert!(app.hd_library.is_dirty(index));
                    assert!(!app.permit_close);
                    assert!(
                        matches!(app.pending, Some(Pending::QuitPod)),
                        "Apply transitions to the dirty-file prompt"
                    );
                    assert_hd_bytes_eq(
                        app.hd_library.file(index).unwrap().as_bytes(),
                        &expected,
                        "quit Apply changes only the target word",
                    );
                    click_prompt(&context, &mut app, "cancel");
                    assert!(app.pending.is_none());
                    assert!(!app.permit_close);
                    assert_hd_bytes_eq(
                        app.hd_library.file(index).unwrap().as_bytes(),
                        &expected,
                        "canceling dirty-file prompt retains applied bytes",
                    );
                }
                "reject" => {
                    click_prompt(&context, &mut app, "apply-type19");
                    assert_eq!(
                        app.hd_type19_pending.get(&(index, slot, control)),
                        Some(&exact)
                    );
                    assert!(matches!(app.pending, Some(Pending::QuitPod)));
                    assert!(app.hd_library.is_dirty(index));
                    assert!(
                        !app.permit_close,
                        "rejected quit Apply keeps the window open"
                    );
                    assert_hd_bytes_eq(
                        app.hd_library.file(index).unwrap().as_bytes(),
                        &before_quit,
                        "rejected quit Apply preserves complete bytes",
                    );
                    assert!(app.pod_message.contains("unassigned marker"));
                }
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn hd_type19_failed_apply_preserves_dirty_saved_baseline_and_reverts_exactly() {
        let model = pod_preset::HD_TYPE19_CHOICE_MODEL_ID;
        let control = pod_preset::HdType19Control::ALL[0];
        let slot = pod_preset::HdFxSlot::new(5).unwrap();
        let source = sample_hd_type9_preset_for(5, 9, 0x3e12_3456, model, control.key());
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let index = app.hd_library.import_bytes(source.clone()).unwrap();
        app.select_hd_file(index);
        app.pod_open = true;
        app.hd_effect_slot = 5;

        app.hd_library
            .set_effect_enabled(index, slot, true)
            .unwrap();
        app.hd_library.note_saved(index).unwrap();
        let saved_baseline = app.hd_library.file(index).unwrap().as_bytes().to_vec();
        assert!(!app.hd_library.is_dirty(index));
        app.hd_library
            .set_effect_enabled(index, slot, false)
            .unwrap();
        let independently_dirty = app.hd_library.file(index).unwrap().as_bytes().to_vec();
        assert!(app.hd_library.is_dirty(index));
        click_hd_type19_choice(&context, &mut app, 6);
        let exact = PendingHdType19Choice {
            file_index: index,
            slot,
            model_id: model,
            control,
            choice: pod_preset::HdType19Choice::Choice6,
        };
        assert_eq!(
            app.hd_type19_pending.get(&(index, slot, control)),
            Some(&exact)
        );
        assert_hd_bytes_eq(
            app.hd_library.file(index).unwrap().as_bytes(),
            &independently_dirty,
            "actual type-19 combo stages over the independent dirty enable edit",
        );

        let target = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + 9 * 0x14;
        let mut invalid = independently_dirty.clone();
        invalid[target + 0x10] = 1;
        *app.hd_library.file_mut(index).unwrap() = PodPresetFile::parse(invalid.clone()).unwrap();
        assert!(matches!(
            app.hd_library.file(index),
            Some(PodPresetFile::H5ep(preset)) if preset.effect_model_id(5) == Some(model)
        ));
        assert_eq!(app.hd_library.file(index).unwrap().as_bytes(), invalid);
        assert!(app.hd_library.is_dirty(index));
        assert_eq!(
            app.hd_type19_pending.get(&(index, slot, control)),
            Some(&exact)
        );
        click_hd_editor(&context, &mut app, hd_type19_apply_button());
        assert_eq!(
            app.hd_type19_pending.get(&(index, slot, control)),
            Some(&exact)
        );
        assert_eq!(app.hd_library.file(index).unwrap().as_bytes(), invalid);
        assert!(app.hd_library.is_dirty(index));
        assert!(app.pod_message.contains("unassigned marker"));
        assert!(app.discard_next_dirty_pod());
        assert!(app.hd_type19_pending.is_empty());
        assert_hd_bytes_eq(
            app.hd_library.file(index).unwrap().as_bytes(),
            &saved_baseline,
            "revert restores the complete saved baseline",
        );
        assert!(!app.hd_library.is_dirty(index));
    }

    #[test]
    fn hd_type19_stage_lifecycle_covers_context_open_save_collision_and_revert() {
        let model = pod_preset::HD_TYPE19_CHOICE_MODEL_ID;
        let control = pod_preset::HdType19Control::ALL[0];
        let slot = pod_preset::HdFxSlot::new(5).unwrap();
        let source = sample_hd_type9_preset_for(5, 9, 0x3e12_3456, model, control.key());
        let pending = |index, choice| PendingHdType19Choice {
            file_index: index,
            slot,
            model_id: model,
            control,
            choice,
        };

        let (context, mut by_slot) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let index = by_slot.hd_library.import_bytes(source.clone()).unwrap();
        by_slot.select_hd_file(index);
        by_slot.pod_open = true;
        by_slot.hd_effect_slot = 5;
        click_hd_type19_choice(&context, &mut by_slot, 2);
        let exact = pending(index, pod_preset::HdType19Choice::Choice2);
        assert_eq!(
            by_slot.hd_type19_pending.get(&(index, slot, control)),
            Some(&exact)
        );
        by_slot.hd_effect_slot = 4;
        hd_editor_frame(&context, &mut by_slot, vec![]);
        assert!(
            by_slot.hd_type19_pending.is_empty(),
            "slot change clears only stale type-19 stage"
        );
        assert_hd_bytes_eq(
            by_slot.hd_library.file(index).unwrap().as_bytes(),
            &source,
            "slot invalidation preserves full source",
        );

        let (context, mut by_model) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let index = by_model.hd_library.import_bytes(source.clone()).unwrap();
        by_model.select_hd_file(index);
        by_model.pod_open = true;
        by_model.hd_effect_slot = 5;
        click_hd_type19_choice(&context, &mut by_model, 1);
        let exact = pending(index, pod_preset::HdType19Choice::Choice1);
        assert_eq!(
            by_model.hd_type19_pending.get(&(index, slot, control)),
            Some(&exact)
        );
        let mut replacement = source.clone();
        let block = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100;
        replacement[block..block + 4].copy_from_slice(&0x020a_001e_u32.to_be_bytes());
        *by_model.hd_library.file_mut(index).unwrap() =
            PodPresetFile::parse(replacement.clone()).unwrap();
        hd_editor_frame(&context, &mut by_model, vec![]);
        assert!(
            by_model.hd_type19_pending.is_empty(),
            "model change clears stale type-19 stage"
        );
        assert_hd_bytes_eq(
            by_model.hd_library.file(index).unwrap().as_bytes(),
            &replacement,
            "model invalidation preserves complete replacement bytes",
        );

        let (context, mut by_entry) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let index = by_entry.hd_library.import_bytes(source.clone()).unwrap();
        by_entry.select_hd_file(index);
        by_entry.pod_open = true;
        by_entry.hd_effect_slot = 5;
        click_hd_type19_choice(&context, &mut by_entry, 0);
        let exact = pending(index, pod_preset::HdType19Choice::Choice0);
        assert_eq!(
            by_entry.hd_type19_pending.get(&(index, slot, control)),
            Some(&exact)
        );
        let other = by_entry.hd_library.import_bytes(source.clone()).unwrap();
        by_entry.select_hd_file(other);
        assert!(
            by_entry.hd_type19_pending.is_empty(),
            "selection replacement invalidates old-entry stage"
        );
        assert_hd_bytes_eq(
            by_entry.hd_library.file(index).unwrap().as_bytes(),
            &source,
            "entry invalidation preserves old complete bytes",
        );

        let (context, mut open) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let index = open.hd_library.import_bytes(source.clone()).unwrap();
        open.select_hd_file(index);
        open.pod_open = true;
        open.hd_effect_slot = 5;
        click_hd_type19_choice(&context, &mut open, 3);
        let exact = pending(index, pod_preset::HdType19Choice::Choice3);
        assert_eq!(
            open.hd_type19_pending.get(&(index, slot, control)),
            Some(&exact)
        );
        let replacement_dir = tempfile::tempdir().unwrap();
        let replacement_path = replacement_dir.path().join("type19-open.5xe");
        let replacement_bytes = sample_hd_type9_preset_for(5, 9, 0x3e00_0000, model, control.key());
        std::fs::write(&replacement_path, &replacement_bytes).unwrap();
        open.dialogs = Box::new(FakeDialogs {
            pod_open: [None, Some(replacement_path)].into(),
            ..Default::default()
        });
        click_hd_editor(&context, &mut open, "load-hd-file");
        assert_eq!(
            open.hd_type19_pending.get(&(index, slot, control)),
            Some(&exact),
            "canceled Open preserves exact stage"
        );
        assert_hd_bytes_eq(
            open.hd_library.file(index).unwrap().as_bytes(),
            &source,
            "canceled Open preserves all bytes",
        );
        click_hd_editor(&context, &mut open, "load-hd-file");
        let opened = open
            .hd_selected
            .expect("successful Open selects replacement");
        assert_ne!(opened, index);
        assert!(
            open.hd_type19_pending.is_empty(),
            "successful Open invalidates only the old entry stage"
        );
        assert_hd_bytes_eq(
            open.hd_library.file(index).unwrap().as_bytes(),
            &source,
            "successful Open preserves old entry bytes",
        );
        assert_hd_bytes_eq(
            open.hd_library.file(opened).unwrap().as_bytes(),
            &replacement_bytes,
            "successful Open preserves complete replacement bytes",
        );

        let (context, mut save) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let index = save.hd_library.import_bytes(source.clone()).unwrap();
        save.select_hd_file(index);
        save.pod_open = true;
        save.hd_effect_slot = 5;
        click_hd_type19_choice(&context, &mut save, 4);
        let exact = pending(index, pod_preset::HdType19Choice::Choice4);
        assert_eq!(
            save.hd_type19_pending.get(&(index, slot, control)),
            Some(&exact)
        );
        save.hd_library
            .set_effect_enabled(index, slot, true)
            .unwrap();
        let mut dirty_bytes = source.clone();
        dirty_bytes[pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 8] = 1;
        assert!(save.hd_library.is_dirty(index));
        assert_eq!(
            save.hd_type19_pending.get(&(index, slot, control)),
            Some(&exact),
            "independent edit retains exact stage"
        );
        assert_hd_bytes_eq(
            save.hd_library.file(index).unwrap().as_bytes(),
            &dirty_bytes,
            "independent edit preserves full type-19 source word",
        );
        let directory = tempfile::tempdir().unwrap();
        let collision = directory.path().join("occupied.5xe");
        let saved = directory.path().join("type19-lifecycle.5xe");
        std::fs::write(&collision, b"keep existing").unwrap();
        save.dialogs = Box::new(FakeDialogs {
            pod_save: [Some(collision.clone()), Some(saved.clone())].into(),
            ..Default::default()
        });
        assert!(!save.write_hd_copy(index));
        assert_eq!(std::fs::read(&collision).unwrap(), b"keep existing");
        assert_eq!(
            save.hd_type19_pending.get(&(index, slot, control)),
            Some(&exact),
            "collision preserves exact stage"
        );
        assert_hd_bytes_eq(
            save.hd_library.file(index).unwrap().as_bytes(),
            &dirty_bytes,
            "collision preserves every in-memory byte",
        );
        assert!(save.write_hd_copy(index));
        assert_eq!(
            std::fs::read(&saved).unwrap(),
            dirty_bytes,
            "successful save writes complete expected bytes"
        );
        assert!(!save.hd_library.is_dirty(index));
        assert_eq!(
            save.hd_type19_pending.get(&(index, slot, control)),
            Some(&exact),
            "successful save preserves exact stage"
        );
        let reopen_path = directory.path().join("reopen.5xe");
        std::fs::write(&reopen_path, std::fs::read(&saved).unwrap()).unwrap();
        save.dialogs = Box::new(FakeDialogs {
            pod_open: [Some(reopen_path)].into(),
            ..Default::default()
        });
        click_hd_editor(&context, &mut save, "load-hd-file");
        let reopened = save.hd_selected.expect("saved copy reopens");
        assert!(
            save.hd_type19_pending.is_empty(),
            "reopen invalidates stage belonging to prior entry"
        );
        assert_hd_bytes_eq(
            save.hd_library.file(reopened).unwrap().as_bytes(),
            &dirty_bytes,
            "reopened saved file matches complete saved bytes",
        );
        assert!(!save.hd_library.is_dirty(reopened));

        save.select_hd_file(index);
        save.hd_effect_slot = 5;
        click_hd_type19_choice(&context, &mut save, 7);
        let revert_stage = pending(index, pod_preset::HdType19Choice::Choice7);
        assert_eq!(
            save.hd_type19_pending.get(&(index, slot, control)),
            Some(&revert_stage)
        );
        save.hd_library
            .set_effect_enabled(index, slot, false)
            .unwrap();
        assert!(save.hd_library.is_dirty(index));
        assert_eq!(
            save.hd_type19_pending.get(&(index, slot, control)),
            Some(&revert_stage),
            "dirty independent edit retains the exact stage before revert"
        );
        assert!(save.discard_next_dirty_pod());
        assert!(save.hd_type19_pending.is_empty());
        assert_hd_bytes_eq(
            save.hd_library.file(index).unwrap().as_bytes(),
            &dirty_bytes,
            "revert returns to successful-save baseline",
        );
        assert!(!save.hd_library.is_dirty(index));
    }

    #[test]
    fn hd_type13_gui_applies_all_four_static_words_and_preserves_both_type11_controls() {
        let controls =
            pod_preset::HdType11Control::for_model(pod_preset::HD_TYPE13_CHOICE_MODEL_ID);
        let type13 = pod_preset::HdType13Control::ALL[0];
        let slot = pod_preset::HdFxSlot::new(5).unwrap();
        for choice in pod_preset::HdType13Choice::ALL {
            let (context, mut app) = app();
            context.style_mut(|style| style.animation_time = 0.0);
            let original = sample_hd_type11_preset(pod_preset::HD_TYPE13_CHOICE_MODEL_ID);
            let index = app.hd_library.import_bytes(original.clone()).unwrap();
            app.select_hd_file(index);
            app.pod_open = true;
            app.hd_effect_slot = 5;
            click_hd_type13_choice(&context, &mut app, choice.index() as usize);
            let staged = PendingHdType13Choice {
                file_index: index,
                slot,
                model_id: type13.model_id(),
                control: type13,
                choice,
            };
            assert_eq!(
                app.hd_type13_pending.get(&(index, slot, type13)),
                Some(&staged)
            );
            assert_hd_bytes_eq(
                app.hd_library.file(index).unwrap().as_bytes(),
                &original,
                "type-0x0D selection stages without changing bytes",
            );
            click_hd_editor(&context, &mut app, hd_type13_apply_button());
            let value = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + 4 * 0x14 + 4;
            let mut expected = original.clone();
            expected[value..value + 4].copy_from_slice(&choice.bits().to_be_bytes());
            assert_hd_bytes_eq(
                app.hd_library.file(index).unwrap().as_bytes(),
                &expected,
                &format!(
                    "type-0x0D {:?} Apply changes only its value word; status {}",
                    choice, app.pod_message
                ),
            );
            assert!(app.hd_type13_pending.is_empty());
            click_hd_type13_choice(&context, &mut app, ((choice.index() + 1) % 4) as usize);
            assert_hd_bytes_eq(
                app.hd_library.file(index).unwrap().as_bytes(),
                &expected,
                "staging another type-0x0D choice remains byte neutral",
            );
            click_hd_editor(&context, &mut app, hd_type13_discard_button());
            assert!(app.hd_type13_pending.is_empty());
            assert_hd_bytes_eq(
                app.hd_library.file(index).unwrap().as_bytes(),
                &expected,
                "discard changes no bytes",
            );
            for control in controls {
                let row = if control.key() == 0x3f10_0001 { 3 } else { 8 };
                let at = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + row * 0x14 + 4;
                assert_eq!(
                    &app.hd_library.file(index).unwrap().as_bytes()[at..at + 4],
                    &original[at..at + 4],
                    "type-11 sibling remains unchanged"
                );
            }
        }
        // Three independent stages coexist; applying the new control retains
        // both type-11 records and alters only its own word.
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let source = sample_hd_type11_preset(pod_preset::HD_TYPE13_CHOICE_MODEL_ID);
        let index = app.hd_library.import_bytes(source.clone()).unwrap();
        app.select_hd_file(index);
        app.pod_open = true;
        app.hd_effect_slot = 5;
        click_hd_type11_choice(&context, &mut app, controls[0], 1);
        click_hd_type11_choice(&context, &mut app, controls[1], 4);
        click_hd_type13_choice(&context, &mut app, 3);
        assert_eq!(app.hd_type11_pending.len(), 2);
        assert_eq!(app.hd_type13_pending.len(), 1);
        click_hd_editor(&context, &mut app, hd_type11_discard_button(controls[0]));
        assert_eq!(app.hd_type11_pending.len(), 1);
        assert!(
            app.hd_type11_pending
                .contains_key(&(index, slot, controls[1]))
        );
        assert_eq!(
            app.hd_type13_pending.len(),
            1,
            "type-11 Discard preserves type-0x0D stage"
        );
        click_hd_type11_choice(&context, &mut app, controls[0], 1);
        click_hd_editor(&context, &mut app, hd_type13_discard_button());
        assert_eq!(
            app.hd_type11_pending.len(),
            2,
            "type-0x0D Discard preserves both type-11 stages"
        );
        assert!(app.hd_type13_pending.is_empty());
        click_hd_type13_choice(&context, &mut app, 3);
        let pending_type13 =
            app.hd_type13_pending[&(index, slot, pod_preset::HdType13Control::ALL[0])];
        click_hd_editor(&context, &mut app, hd_type11_apply_button(controls[0]));
        assert_eq!(app.hd_type11_pending.len(), 1);
        assert!(
            app.hd_type11_pending
                .contains_key(&(index, slot, controls[1]))
        );
        assert_eq!(
            app.hd_type13_pending
                .get(&(index, slot, pod_preset::HdType13Control::ALL[0])),
            Some(&pending_type13),
            "type-11 Apply preserves type-0x0D pending state"
        );
        let type11_value = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + 3 * 0x14 + 4;
        let mut after_type11 = source.clone();
        after_type11[type11_value..type11_value + 4].copy_from_slice(&0x3e80_0000u32.to_be_bytes());
        assert_hd_bytes_eq(
            app.hd_library.file(index).unwrap().as_bytes(),
            &after_type11,
            "type-11 Apply changes only its own word",
        );
        let before = app.hd_type11_pending.clone();
        click_hd_editor(&context, &mut app, hd_type13_apply_button());
        assert_eq!(
            app.hd_type11_pending, before,
            "type-0x0D Apply preserves the remaining type-11 stage"
        );
        assert!(app.hd_type13_pending.is_empty());
        let at = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + 4 * 0x14 + 4;
        assert_eq!(
            &app.hd_library.file(index).unwrap().as_bytes()[at..at + 4],
            &0x3f80_0000u32.to_be_bytes()
        );
        after_type11[at..at + 4].copy_from_slice(&0x3f80_0000u32.to_be_bytes());
        assert_hd_bytes_eq(
            app.hd_library.file(index).unwrap().as_bytes(),
            &after_type11,
            "type-0x0D Apply preserves prior type-11 write",
        );
    }

    #[test]
    fn hd_type13_apply_and_discard_preserve_exact_simultaneous_sibling_stages() {
        let controls =
            pod_preset::HdType11Control::for_model(pod_preset::HD_TYPE13_CHOICE_MODEL_ID);
        let type13 = pod_preset::HdType13Control::ALL[0];
        let slot = pod_preset::HdFxSlot::new(5).unwrap();
        let source = sample_hd_type11_preset(pod_preset::HD_TYPE13_CHOICE_MODEL_ID);
        for apply in [true, false] {
            let (context, mut app) = app();
            context.style_mut(|style| style.animation_time = 0.0);
            let index = app.hd_library.import_bytes(source.clone()).unwrap();
            app.select_hd_file(index);
            app.pod_open = true;
            app.hd_effect_slot = 5;
            click_hd_type11_choice(&context, &mut app, controls[0], 1);
            click_hd_type11_choice(&context, &mut app, controls[1], 4);
            click_hd_type13_choice(&context, &mut app, 3);
            let first = PendingHdType11Choice {
                file_index: index,
                slot,
                model_id: pod_preset::HD_TYPE13_CHOICE_MODEL_ID,
                control: controls[0],
                choice: pod_preset::HdType11Choice::Choice1,
            };
            let second = PendingHdType11Choice {
                file_index: index,
                slot,
                model_id: pod_preset::HD_TYPE13_CHOICE_MODEL_ID,
                control: controls[1],
                choice: pod_preset::HdType11Choice::Choice4,
            };
            let type13_stage = PendingHdType13Choice {
                file_index: index,
                slot,
                model_id: pod_preset::HD_TYPE13_CHOICE_MODEL_ID,
                control: type13,
                choice: pod_preset::HdType13Choice::Choice3,
            };
            assert_eq!(
                app.hd_type11_pending.get(&(index, slot, controls[0])),
                Some(&first)
            );
            assert_eq!(
                app.hd_type11_pending.get(&(index, slot, controls[1])),
                Some(&second)
            );
            assert_eq!(
                app.hd_type13_pending.get(&(index, slot, type13)),
                Some(&type13_stage)
            );

            if apply {
                click_hd_editor(&context, &mut app, hd_type13_apply_button());
                let type13_at =
                    pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + 4 * 0x14 + 4;
                let mut expected = source.clone();
                expected[type13_at..type13_at + 4]
                    .copy_from_slice(&pod_preset::HdType13Choice::Choice3.bits().to_be_bytes());
                assert_hd_bytes_eq(
                    app.hd_library.file(index).unwrap().as_bytes(),
                    &expected,
                    "type-0x0D Apply changes only its own value with both siblings staged",
                );
                assert!(app.hd_type13_pending.is_empty());
                assert_eq!(
                    app.hd_type11_pending.get(&(index, slot, controls[0])),
                    Some(&first)
                );
                assert_eq!(
                    app.hd_type11_pending.get(&(index, slot, controls[1])),
                    Some(&second)
                );
            } else {
                click_hd_editor(&context, &mut app, hd_type13_discard_button());
                assert_hd_bytes_eq(
                    app.hd_library.file(index).unwrap().as_bytes(),
                    &source,
                    "type-0x0D Discard changes no bytes with both siblings staged",
                );
                assert_eq!(app.hd_type13_pending.len(), 0);
                assert_eq!(
                    app.hd_type11_pending.get(&(index, slot, controls[0])),
                    Some(&first)
                );
                assert_eq!(
                    app.hd_type11_pending.get(&(index, slot, controls[1])),
                    Some(&second)
                );
            }
        }

        for sibling_index in 0..2 {
            for apply in [true, false] {
                let (context, mut app) = app();
                context.style_mut(|style| style.animation_time = 0.0);
                let index = app.hd_library.import_bytes(source.clone()).unwrap();
                app.select_hd_file(index);
                app.pod_open = true;
                app.hd_effect_slot = 5;
                click_hd_type11_choice(&context, &mut app, controls[0], 1);
                click_hd_type11_choice(&context, &mut app, controls[1], 4);
                click_hd_type13_choice(&context, &mut app, 2);
                let kept_control = controls[1 - sibling_index];
                let kept_choice = if sibling_index == 0 {
                    pod_preset::HdType11Choice::Choice4
                } else {
                    pod_preset::HdType11Choice::Choice1
                };
                let kept = PendingHdType11Choice {
                    file_index: index,
                    slot,
                    model_id: pod_preset::HD_TYPE13_CHOICE_MODEL_ID,
                    control: kept_control,
                    choice: kept_choice,
                };
                let type13_stage = PendingHdType13Choice {
                    file_index: index,
                    slot,
                    model_id: pod_preset::HD_TYPE13_CHOICE_MODEL_ID,
                    control: type13,
                    choice: pod_preset::HdType13Choice::Choice2,
                };
                let acted_control = controls[sibling_index];
                if apply {
                    click_hd_editor(&context, &mut app, hd_type11_apply_button(acted_control));
                } else {
                    click_hd_editor(&context, &mut app, hd_type11_discard_button(acted_control));
                }
                assert_eq!(app.hd_type11_pending.len(), 1);
                assert_eq!(
                    app.hd_type11_pending.get(&(index, slot, kept_control)),
                    Some(&kept)
                );
                assert_eq!(
                    app.hd_type13_pending.get(&(index, slot, type13)),
                    Some(&type13_stage)
                );
                let mut expected = source.clone();
                if apply {
                    let row = if sibling_index == 0 { 3 } else { 8 };
                    let at =
                        pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + row * 0x14 + 4;
                    let choice = if sibling_index == 0 {
                        pod_preset::HdType11Choice::Choice1
                    } else {
                        pod_preset::HdType11Choice::Choice4
                    };
                    expected[at..at + 4].copy_from_slice(&choice.bits().to_be_bytes());
                }
                assert_hd_bytes_eq(
                    app.hd_library.file(index).unwrap().as_bytes(),
                    &expected,
                    "type-11 action changes only the acted-on control",
                );
            }
        }
    }

    #[test]
    fn hd_type13_stage_lifecycle_covers_context_open_save_collision_and_revert() {
        let control = pod_preset::HdType13Control::ALL[0];
        let slot = pod_preset::HdFxSlot::new(5).unwrap();
        let model_id = pod_preset::HD_TYPE13_CHOICE_MODEL_ID;
        let source = sample_hd_type11_preset(model_id);
        let pending = |index, choice| PendingHdType13Choice {
            file_index: index,
            slot,
            model_id,
            control,
            choice,
        };

        // Changing the selected FX slot clears only the stale type-0x0D stage.
        let (context, mut by_slot) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let index = by_slot.hd_library.import_bytes(source.clone()).unwrap();
        by_slot.select_hd_file(index);
        by_slot.pod_open = true;
        by_slot.hd_effect_slot = 5;
        click_hd_type13_choice(&context, &mut by_slot, 2);
        assert_eq!(
            by_slot.hd_type13_pending.get(&(index, slot, control)),
            Some(&pending(index, pod_preset::HdType13Choice::Choice2))
        );
        by_slot.hd_effect_slot = 4;
        hd_editor_frame(&context, &mut by_slot, vec![]);
        assert!(
            by_slot.hd_type13_pending.is_empty(),
            "slot change clears stale stage"
        );
        assert_hd_bytes_eq(
            by_slot.hd_library.file(index).unwrap().as_bytes(),
            &source,
            "slot invalidation leaves full source unchanged",
        );

        // Replacing the model in the staged slot invalidates the stage without
        // altering the replacement record or any unrelated bytes.
        let (context, mut by_model) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let index = by_model.hd_library.import_bytes(source.clone()).unwrap();
        by_model.select_hd_file(index);
        by_model.pod_open = true;
        by_model.hd_effect_slot = 5;
        click_hd_type13_choice(&context, &mut by_model, 1);
        let mut changed_model = source.clone();
        let model_at = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100;
        changed_model[model_at..model_at + 4].copy_from_slice(&0x020a_001e_u32.to_be_bytes());
        *by_model.hd_library.file_mut(index).unwrap() =
            PodPresetFile::parse(changed_model.clone()).unwrap();
        hd_editor_frame(&context, &mut by_model, vec![]);
        assert!(
            by_model.hd_type13_pending.is_empty(),
            "model change clears stale stage"
        );
        assert_hd_bytes_eq(
            by_model.hd_library.file(index).unwrap().as_bytes(),
            &changed_model,
            "model invalidation preserves all replacement bytes",
        );

        // Selecting another file clears the prior entry's exact stage.
        let (context, mut by_entry) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let index = by_entry.hd_library.import_bytes(source.clone()).unwrap();
        by_entry.select_hd_file(index);
        by_entry.pod_open = true;
        by_entry.hd_effect_slot = 5;
        click_hd_type13_choice(&context, &mut by_entry, 0);
        assert_eq!(
            by_entry.hd_type13_pending.get(&(index, slot, control)),
            Some(&pending(index, pod_preset::HdType13Choice::Choice0))
        );
        let other = by_entry.hd_library.import_bytes(source.clone()).unwrap();
        by_entry.select_hd_file(other);
        assert!(
            by_entry.hd_type13_pending.is_empty(),
            "selection change clears prior entry stage"
        );
        assert_hd_bytes_eq(
            by_entry.hd_library.file(index).unwrap().as_bytes(),
            &source,
            "entry invalidation preserves old bytes",
        );

        // Cancelled Open retains its stage; successful Open clears it and
        // leaves exact complete-file bytes in both the old and new entries.
        let (context, mut open) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let index = open.hd_library.import_bytes(source.clone()).unwrap();
        open.select_hd_file(index);
        open.pod_open = true;
        open.hd_effect_slot = 5;
        click_hd_type13_choice(&context, &mut open, 3);
        let staged = pending(index, pod_preset::HdType13Choice::Choice3);
        let replacement_dir = tempfile::tempdir().unwrap();
        let replacement_path = replacement_dir.path().join("type13-open.5xe");
        let replacement_bytes = sample_hd_type11_preset(0x020a_001e);
        std::fs::write(&replacement_path, &replacement_bytes).unwrap();
        open.dialogs = Box::new(FakeDialogs {
            pod_open: [None, Some(replacement_path)].into(),
            ..Default::default()
        });
        click_hd_editor(&context, &mut open, "load-hd-file");
        assert_eq!(
            open.hd_type13_pending.get(&(index, slot, control)),
            Some(&staged),
            "cancelled Open retains exact stage"
        );
        assert_hd_bytes_eq(
            open.hd_library.file(index).unwrap().as_bytes(),
            &source,
            "cancelled Open retains full bytes",
        );
        click_hd_editor(&context, &mut open, "load-hd-file");
        let opened = open
            .hd_selected
            .expect("successful Open selects replacement");
        assert_ne!(opened, index);
        assert!(
            open.hd_type13_pending.is_empty(),
            "successful Open clears old-entry stage"
        );
        assert_hd_bytes_eq(
            open.hd_library.file(index).unwrap().as_bytes(),
            &source,
            "successful Open preserves old-entry bytes",
        );
        assert_hd_bytes_eq(
            open.hd_library.file(opened).unwrap().as_bytes(),
            &replacement_bytes,
            "successful Open stores complete replacement bytes",
        );

        // An applied sibling makes the entry dirty while type-0x0D remains
        // staged. Both failed collision and successful save preserve the exact
        // pending record and current bytes; reopen intentionally invalidates it.
        let (context, mut save) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let index = save.hd_library.import_bytes(source.clone()).unwrap();
        save.select_hd_file(index);
        save.pod_open = true;
        save.hd_effect_slot = 5;
        click_hd_type13_choice(&context, &mut save, 2);
        let staged = pending(index, pod_preset::HdType13Choice::Choice2);
        let controls = pod_preset::HdType11Control::for_model(model_id);
        click_hd_type11_choice(&context, &mut save, controls[0], 1);
        click_hd_editor(&context, &mut save, hd_type11_apply_button(controls[0]));
        let type11_at = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + 3 * 0x14 + 4;
        let mut dirty_bytes = source.clone();
        dirty_bytes[type11_at..type11_at + 4]
            .copy_from_slice(&pod_preset::HdType11Choice::Choice1.bits().to_be_bytes());
        assert!(save.hd_library.is_dirty(index));
        assert_eq!(
            save.hd_type13_pending.get(&(index, slot, control)),
            Some(&staged)
        );
        assert_hd_bytes_eq(
            save.hd_library.file(index).unwrap().as_bytes(),
            &dirty_bytes,
            "sibling Apply preserves staged source bytes",
        );
        let directory = tempfile::tempdir().unwrap();
        let collision = directory.path().join("occupied.5xe");
        let saved = directory.path().join("type13-lifecycle.5xe");
        std::fs::write(&collision, b"keep existing").unwrap();
        save.dialogs = Box::new(FakeDialogs {
            pod_save: [Some(collision.clone()), Some(saved.clone())].into(),
            ..Default::default()
        });
        assert!(!save.write_hd_copy(index));
        assert_eq!(std::fs::read(&collision).unwrap(), b"keep existing");
        assert_eq!(
            save.hd_type13_pending.get(&(index, slot, control)),
            Some(&staged),
            "collision retains exact stage"
        );
        assert_hd_bytes_eq(
            save.hd_library.file(index).unwrap().as_bytes(),
            &dirty_bytes,
            "collision retains complete in-memory bytes",
        );
        assert!(save.write_hd_copy(index));
        assert_eq!(
            std::fs::read(&saved).unwrap(),
            dirty_bytes,
            "successful save writes complete expected bytes"
        );
        assert!(!save.hd_library.is_dirty(index));
        assert_eq!(
            save.hd_type13_pending.get(&(index, slot, control)),
            Some(&staged),
            "successful save preserves exact stage"
        );
        assert_hd_bytes_eq(
            save.hd_library.file(index).unwrap().as_bytes(),
            &dirty_bytes,
            "save preserves complete current bytes",
        );
        let reopened_bytes = std::fs::read(&saved).unwrap();
        let reopen_path = directory.path().join("reopen.5xe");
        std::fs::write(&reopen_path, &reopened_bytes).unwrap();
        save.dialogs = Box::new(FakeDialogs {
            pod_open: [Some(reopen_path)].into(),
            ..Default::default()
        });
        click_hd_editor(&context, &mut save, "load-hd-file");
        let reopened = save.hd_selected.expect("saved copy reopens");
        assert!(
            save.hd_type13_pending.is_empty(),
            "successful reopen clears old stage"
        );
        assert_hd_bytes_eq(
            save.hd_library.file(reopened).unwrap().as_bytes(),
            &dirty_bytes,
            "reopened saved copy has complete expected bytes",
        );
        assert!(!save.hd_library.is_dirty(reopened));

        save.select_hd_file(index);
        save.hd_effect_slot = 5;
        click_hd_type11_choice(&context, &mut save, controls[0], 4);
        click_hd_editor(&context, &mut save, hd_type11_apply_button(controls[0]));
        assert!(save.hd_library.is_dirty(index));
        assert!(save.discard_next_dirty_pod());
        assert_hd_bytes_eq(
            save.hd_library.file(index).unwrap().as_bytes(),
            &dirty_bytes,
            "revert restores the exact successful-save baseline",
        );
        assert!(!save.hd_library.is_dirty(index));
        assert!(save.hd_type13_pending.is_empty());
    }

    #[test]
    fn hd_type13_failed_apply_retains_dirty_baseline_and_all_three_stages() {
        let controls =
            pod_preset::HdType11Control::for_model(pod_preset::HD_TYPE13_CHOICE_MODEL_ID);
        let control = pod_preset::HdType13Control::ALL[0];
        let slot = pod_preset::HdFxSlot::new(5).unwrap();
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let original = sample_hd_type11_preset(pod_preset::HD_TYPE13_CHOICE_MODEL_ID);
        let index = app.hd_library.import_bytes(original.clone()).unwrap();
        app.select_hd_file(index);
        app.pod_open = true;
        app.hd_effect_slot = 5;
        click_hd_type13_choice(&context, &mut app, 2);
        let type13_stage = app.hd_type13_pending[&(index, slot, control)];
        click_hd_type11_choice(&context, &mut app, controls[0], 1);
        click_hd_editor(&context, &mut app, hd_type11_apply_button(controls[0]));
        assert!(app.hd_library.is_dirty(index));
        click_hd_type11_choice(&context, &mut app, controls[0], 3);
        click_hd_type11_choice(&context, &mut app, controls[1], 4);
        assert_eq!(app.hd_type11_pending.len(), 2);
        assert_eq!(
            app.hd_type13_pending.get(&(index, slot, control)),
            Some(&type13_stage)
        );

        let mut invalid = app.hd_library.file(index).unwrap().as_bytes().to_vec();
        let marker = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + 4 * 0x14 + 0x10;
        invalid[marker] = 1;
        match app.hd_library.file_mut(index).unwrap() {
            PodPresetFile::H5ep(preset) => {
                *preset = pod_preset::OpaqueH5epPreset::parse(invalid.clone()).unwrap()
            }
            _ => panic!("test fixture is H5EP"),
        }
        let type11_stages = app.hd_type11_pending.clone();
        assert!(app.hd_library.is_dirty(index));
        click_hd_editor(&context, &mut app, hd_type13_apply_button());
        assert_eq!(app.hd_library.file(index).unwrap().as_bytes(), invalid);
        assert!(app.hd_library.is_dirty(index));
        assert_eq!(
            app.hd_type13_pending.get(&(index, slot, control)),
            Some(&type13_stage)
        );
        assert_eq!(app.hd_type11_pending, type11_stages);
        assert!(app.pod_message.contains("unassigned marker"));
        assert!(app.discard_next_dirty_pod());
        assert_hd_bytes_eq(
            app.hd_library.file(index).unwrap().as_bytes(),
            &original,
            "revert restores saved baseline after failed target Apply",
        );
        assert!(!app.hd_library.is_dirty(index));
        assert!(app.hd_type11_pending.is_empty());
        assert!(app.hd_type13_pending.is_empty());
    }

    #[test]
    fn hd_type13_quit_keep_discard_and_apply_resolve_all_three_stages_first() {
        let controls =
            pod_preset::HdType11Control::for_model(pod_preset::HD_TYPE13_CHOICE_MODEL_ID);
        let source = sample_hd_type11_preset(pod_preset::HD_TYPE13_CHOICE_MODEL_ID);

        let (context, mut keep) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let keep_index = keep.hd_library.import_bytes(source.clone()).unwrap();
        keep.select_hd_file(keep_index);
        keep.pod_open = true;
        keep.hd_effect_slot = 5;
        click_hd_type11_choice(&context, &mut keep, controls[0], 1);
        click_hd_type11_choice(&context, &mut keep, controls[1], 4);
        click_hd_type13_choice(&context, &mut keep, 2);
        let keep_first = PendingHdType11Choice {
            file_index: keep_index,
            slot: pod_preset::HdFxSlot::new(5).unwrap(),
            model_id: pod_preset::HD_TYPE13_CHOICE_MODEL_ID,
            control: controls[0],
            choice: pod_preset::HdType11Choice::Choice1,
        };
        let keep_second = PendingHdType11Choice {
            file_index: keep_index,
            slot: pod_preset::HdFxSlot::new(5).unwrap(),
            model_id: pod_preset::HD_TYPE13_CHOICE_MODEL_ID,
            control: controls[1],
            choice: pod_preset::HdType11Choice::Choice4,
        };
        let keep_type13 = PendingHdType13Choice {
            file_index: keep_index,
            slot: pod_preset::HdFxSlot::new(5).unwrap(),
            model_id: pod_preset::HD_TYPE13_CHOICE_MODEL_ID,
            control: pod_preset::HdType13Control::ALL[0],
            choice: pod_preset::HdType13Choice::Choice2,
        };
        keep.request_close(&context);
        assert!(matches!(keep.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut keep, "cancel");
        assert!(keep.pending.is_none());
        assert_eq!(keep.hd_type11_pending.len(), 2);
        assert_eq!(
            keep.hd_type11_pending
                .get(&(keep_index, keep_type13.slot, controls[0])),
            Some(&keep_first)
        );
        assert_eq!(
            keep.hd_type11_pending
                .get(&(keep_index, keep_type13.slot, controls[1])),
            Some(&keep_second)
        );
        assert_eq!(
            keep.hd_type13_pending
                .get(&(keep_index, keep_type13.slot, keep_type13.control)),
            Some(&keep_type13)
        );
        assert!(!keep.permit_close, "Keep editing must not close the window");
        assert_hd_bytes_eq(
            keep.hd_library.file(keep_index).unwrap().as_bytes(),
            &source,
            "Keep editing preserves all three stages and bytes",
        );

        let (context, mut discard) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let discard_index = discard.hd_library.import_bytes(source.clone()).unwrap();
        discard.select_hd_file(discard_index);
        discard.pod_open = true;
        discard.hd_effect_slot = 5;
        click_hd_type11_choice(&context, &mut discard, controls[0], 1);
        click_hd_type11_choice(&context, &mut discard, controls[1], 4);
        click_hd_type13_choice(&context, &mut discard, 2);
        let discard_first = PendingHdType11Choice {
            file_index: discard_index,
            slot: pod_preset::HdFxSlot::new(5).unwrap(),
            model_id: pod_preset::HD_TYPE13_CHOICE_MODEL_ID,
            control: controls[0],
            choice: pod_preset::HdType11Choice::Choice1,
        };
        let discard_second = PendingHdType11Choice {
            file_index: discard_index,
            slot: pod_preset::HdFxSlot::new(5).unwrap(),
            model_id: pod_preset::HD_TYPE13_CHOICE_MODEL_ID,
            control: controls[1],
            choice: pod_preset::HdType11Choice::Choice4,
        };
        let discard_type13 = PendingHdType13Choice {
            file_index: discard_index,
            slot: pod_preset::HdFxSlot::new(5).unwrap(),
            model_id: pod_preset::HD_TYPE13_CHOICE_MODEL_ID,
            control: pod_preset::HdType13Control::ALL[0],
            choice: pod_preset::HdType13Choice::Choice2,
        };
        discard.request_close(&context);
        click_prompt(&context, &mut discard, "discard");
        assert_eq!(discard.hd_type11_pending.len(), 1);
        let remaining = discard.hd_type11_pending.values().next().copied().unwrap();
        assert!(remaining == discard_first || remaining == discard_second);
        assert_eq!(
            discard.hd_type13_pending.get(&(
                discard_index,
                discard_type13.slot,
                discard_type13.control
            )),
            Some(&discard_type13)
        );
        assert!(
            !discard.permit_close,
            "one remaining type-11 stage blocks close"
        );
        assert_hd_bytes_eq(
            discard.hd_library.file(discard_index).unwrap().as_bytes(),
            &source,
            "first quit Discard keeps complete source bytes",
        );
        click_prompt(&context, &mut discard, "discard");
        assert!(discard.hd_type11_pending.is_empty());
        assert_eq!(
            discard.hd_type13_pending.get(&(
                discard_index,
                discard_type13.slot,
                discard_type13.control
            )),
            Some(&discard_type13)
        );
        assert!(
            !discard.permit_close,
            "remaining type-0x0D stage blocks close"
        );
        assert_hd_bytes_eq(
            discard.hd_library.file(discard_index).unwrap().as_bytes(),
            &source,
            "second quit Discard keeps complete source bytes",
        );
        click_prompt(&context, &mut discard, "discard");
        assert!(discard.hd_type13_pending.is_empty());
        assert!(
            discard.permit_close,
            "quit discard closes only after all three stages are cleared"
        );
        assert_hd_bytes_eq(
            discard.hd_library.file(discard_index).unwrap().as_bytes(),
            &source,
            "quit discard leaves preset bytes unchanged",
        );

        let (context, mut apply) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let apply_index = apply.hd_library.import_bytes(source.clone()).unwrap();
        apply.select_hd_file(apply_index);
        apply.pod_open = true;
        apply.hd_effect_slot = 5;
        click_hd_type11_choice(&context, &mut apply, controls[0], 1);
        click_hd_type11_choice(&context, &mut apply, controls[1], 4);
        click_hd_type13_choice(&context, &mut apply, 3);
        let apply_type13 = PendingHdType13Choice {
            file_index: apply_index,
            slot: pod_preset::HdFxSlot::new(5).unwrap(),
            model_id: pod_preset::HD_TYPE13_CHOICE_MODEL_ID,
            control: pod_preset::HdType13Control::ALL[0],
            choice: pod_preset::HdType13Choice::Choice3,
        };
        let apply_first = PendingHdType11Choice {
            file_index: apply_index,
            slot: pod_preset::HdFxSlot::new(5).unwrap(),
            model_id: pod_preset::HD_TYPE13_CHOICE_MODEL_ID,
            control: controls[0],
            choice: pod_preset::HdType11Choice::Choice1,
        };
        let apply_second = PendingHdType11Choice {
            file_index: apply_index,
            slot: pod_preset::HdFxSlot::new(5).unwrap(),
            model_id: pod_preset::HD_TYPE13_CHOICE_MODEL_ID,
            control: controls[1],
            choice: pod_preset::HdType11Choice::Choice4,
        };
        assert_eq!(
            apply
                .hd_type11_pending
                .get(&(apply_index, apply_first.slot, controls[0])),
            Some(&apply_first)
        );
        assert_eq!(
            apply
                .hd_type11_pending
                .get(&(apply_index, apply_second.slot, controls[1])),
            Some(&apply_second)
        );
        assert_eq!(
            apply
                .hd_type13_pending
                .get(&(apply_index, apply_type13.slot, apply_type13.control)),
            Some(&apply_type13)
        );
        apply.request_close(&context);
        let before_first_apply = apply.hd_type11_pending.clone();
        click_prompt(&context, &mut apply, "apply-type11");
        assert_eq!(apply.hd_type11_pending.len(), 1);
        assert_eq!(
            apply
                .hd_type13_pending
                .get(&(apply_index, apply_type13.slot, apply_type13.control)),
            Some(&apply_type13)
        );
        let first_applied = before_first_apply
            .values()
            .find(|pending| {
                !apply
                    .hd_type11_pending
                    .contains_key(&(apply_index, pending.slot, pending.control))
            })
            .copied()
            .unwrap();
        let remaining_type11 = apply.hd_type11_pending.values().next().copied().unwrap();
        let mut expected_after_first = source.clone();
        let first_row = if first_applied.control == controls[0] {
            3
        } else {
            8
        };
        let first_at =
            pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + first_row * 0x14 + 4;
        expected_after_first[first_at..first_at + 4]
            .copy_from_slice(&first_applied.choice.bits().to_be_bytes());
        assert_eq!(
            apply.hd_type11_pending.get(&(
                apply_index,
                remaining_type11.slot,
                remaining_type11.control
            )),
            Some(&remaining_type11)
        );
        assert_hd_bytes_eq(
            apply.hd_library.file(apply_index).unwrap().as_bytes(),
            &expected_after_first,
            "first quit Apply changes only its exact type-11 stage",
        );
        let before_second_apply = apply.hd_type11_pending.values().next().copied().unwrap();
        click_prompt(&context, &mut apply, "apply-type11");
        assert!(apply.hd_type11_pending.is_empty());
        assert_eq!(
            apply
                .hd_type13_pending
                .get(&(apply_index, apply_type13.slot, apply_type13.control)),
            Some(&apply_type13)
        );
        let second_row = if before_second_apply.control == controls[0] {
            3
        } else {
            8
        };
        let second_at =
            pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + second_row * 0x14 + 4;
        let mut expected_after_second = expected_after_first.clone();
        expected_after_second[second_at..second_at + 4]
            .copy_from_slice(&before_second_apply.choice.bits().to_be_bytes());
        assert_hd_bytes_eq(
            apply.hd_library.file(apply_index).unwrap().as_bytes(),
            &expected_after_second,
            "second quit Apply changes only the remaining type-11 stage",
        );
        click_prompt(&context, &mut apply, "apply-type13");
        assert!(apply.hd_type13_pending.is_empty());
        let type13_at = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + 4 * 0x14 + 4;
        let mut expected_after_type13 = expected_after_second;
        expected_after_type13[type13_at..type13_at + 4]
            .copy_from_slice(&apply_type13.choice.bits().to_be_bytes());
        assert_hd_bytes_eq(
            apply.hd_library.file(apply_index).unwrap().as_bytes(),
            &expected_after_type13,
            "quit type-0x0D Apply follows both exact type-11 writes",
        );
        assert!(apply.hd_library.is_dirty(apply_index));
        assert!(
            matches!(apply.pending, Some(Pending::QuitPod)),
            "the unsaved file decision follows all staged controls"
        );
        assert!(
            !apply.permit_close,
            "unsaved file decision keeps the window open"
        );
        click_prompt(&context, &mut apply, "discard");
        assert!(apply.permit_close);
        assert_hd_bytes_eq(
            apply.hd_library.file(apply_index).unwrap().as_bytes(),
            &source,
            "file discard restores original saved baseline after all control Apply operations",
        );
    }

    #[test]
    fn hd_type11_same_model_controls_stage_apply_and_discard_independently() {
        for model_id in [0x020a_001e, 0x020a_001d] {
            let controls = pod_preset::HdType11Control::for_model(model_id);
            let first = controls[0];
            let second = controls[1];
            let (context, mut app) = app();
            context.style_mut(|style| style.animation_time = 0.0);
            let original = sample_hd_type11_preset(model_id);
            let index = app.hd_library.import_bytes(original.clone()).unwrap();
            app.select_hd_file(index);
            app.pod_open = true;
            app.hd_effect_slot = 5;
            let slot = pod_preset::HdFxSlot::new(5).unwrap();
            let first_at = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + 3 * 0x14 + 4;
            let second_at = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + 8 * 0x14 + 4;

            click_hd_type11_choice(&context, &mut app, first, 1);
            let first_pending = app.hd_type11_pending[&(index, slot, first)];
            assert_eq!(first_pending.choice, pod_preset::HdType11Choice::Choice1);
            assert!(!app.hd_type11_pending.contains_key(&(index, slot, second)));
            assert_hd_bytes_eq(
                app.hd_library.file(index).unwrap().as_bytes(),
                &original,
                "first stage",
            );
            click_hd_type11_choice(&context, &mut app, second, 3);
            let second_pending = app.hd_type11_pending[&(index, slot, second)];
            assert_eq!(second_pending.choice, pod_preset::HdType11Choice::Choice3);
            assert_eq!(app.hd_type11_pending[&(index, slot, first)], first_pending);
            assert_hd_bytes_eq(
                app.hd_library.file(index).unwrap().as_bytes(),
                &original,
                "both stages",
            );

            click_hd_editor(&context, &mut app, hd_type11_apply_button(first));
            let mut after_first = original.clone();
            after_first[first_at..first_at + 4].copy_from_slice(&0x3e80_0000u32.to_be_bytes());
            assert_hd_bytes_eq(
                app.hd_library.file(index).unwrap().as_bytes(),
                &after_first,
                "apply first key only",
            );
            assert!(!app.hd_type11_pending.contains_key(&(index, slot, first)));
            assert_eq!(
                app.hd_type11_pending.get(&(index, slot, second)),
                Some(&second_pending)
            );

            click_hd_type11_choice(&context, &mut app, first, 4);
            let first_pending = app.hd_type11_pending[&(index, slot, first)];
            assert_eq!(
                app.hd_type11_pending.get(&(index, slot, second)),
                Some(&second_pending)
            );
            click_hd_editor(&context, &mut app, hd_type11_apply_button(second));
            let mut after_second = after_first.clone();
            after_second[second_at..second_at + 4].copy_from_slice(&0x3f40_0000u32.to_be_bytes());
            assert_hd_bytes_eq(
                app.hd_library.file(index).unwrap().as_bytes(),
                &after_second,
                "apply second key while both stages exist",
            );
            assert_eq!(
                app.hd_type11_pending.get(&(index, slot, first)),
                Some(&first_pending),
                "reverse Apply preserves the first exact stage"
            );
            assert!(!app.hd_type11_pending.contains_key(&(index, slot, second)));

            click_hd_editor(&context, &mut app, hd_type11_apply_button(first));
            let mut after_both = after_second;
            after_both[first_at..first_at + 4].copy_from_slice(&0x3f80_0000u32.to_be_bytes());
            assert_hd_bytes_eq(
                app.hd_library.file(index).unwrap().as_bytes(),
                &after_both,
                "apply preserved first stage after reverse Apply",
            );
            assert!(app.hd_type11_pending.is_empty());

            click_hd_type11_choice(&context, &mut app, first, 2);
            let first_pending = app.hd_type11_pending[&(index, slot, first)];
            click_hd_type11_choice(&context, &mut app, second, 4);
            let second_pending = app.hd_type11_pending[&(index, slot, second)];
            assert_eq!(
                app.hd_type11_pending.get(&(index, slot, first)),
                Some(&first_pending)
            );
            click_hd_editor(&context, &mut app, hd_type11_discard_button(first));
            assert_eq!(
                app.hd_type11_pending.get(&(index, slot, second)),
                Some(&second_pending)
            );
            assert!(!app.hd_type11_pending.contains_key(&(index, slot, first)));
            assert_hd_bytes_eq(
                app.hd_library.file(index).unwrap().as_bytes(),
                &after_both,
                "discard first preserves second stage and applied bytes",
            );
            click_hd_editor(&context, &mut app, hd_type11_apply_button(second));
            let mut final_bytes = after_both;
            final_bytes[second_at..second_at + 4].copy_from_slice(&0x3f80_0000u32.to_be_bytes());
            assert_hd_bytes_eq(
                app.hd_library.file(index).unwrap().as_bytes(),
                &final_bytes,
                "apply second key only",
            );
            assert!(app.hd_type11_pending.is_empty());

            click_hd_type11_choice(&context, &mut app, first, 1);
            let first_pending = app.hd_type11_pending[&(index, slot, first)];
            click_hd_type11_choice(&context, &mut app, second, 2);
            assert_eq!(app.hd_type11_pending.len(), 2);
            click_hd_editor(&context, &mut app, hd_type11_discard_button(second));
            assert_eq!(
                app.hd_type11_pending.get(&(index, slot, first)),
                Some(&first_pending),
                "discarding second preserves the first exact stage"
            );
            assert!(!app.hd_type11_pending.contains_key(&(index, slot, second)));
            assert_hd_bytes_eq(
                app.hd_library.file(index).unwrap().as_bytes(),
                &final_bytes,
                "discard second leaves both stored words intact",
            );
            click_hd_editor(&context, &mut app, hd_type11_apply_button(first));
            let mut after_discard_second = final_bytes;
            after_discard_second[first_at..first_at + 4]
                .copy_from_slice(&0x3e80_0000u32.to_be_bytes());
            assert_hd_bytes_eq(
                app.hd_library.file(index).unwrap().as_bytes(),
                &after_discard_second,
                "apply first after discarding second",
            );
        }
    }

    #[test]
    fn hd_type11_failed_apply_retains_stage_and_file_state() {
        for model_id in [0x020a_001e, 0x020a_001d] {
            for control in pod_preset::HdType11Control::for_model(model_id)
                .iter()
                .copied()
            {
                hd_type11_failed_apply_retains_stage_and_file_state_for(model_id, control);
            }
        }
    }

    fn hd_type11_failed_apply_retains_stage_and_file_state_for(
        model_id: u32,
        control: pod_preset::HdType11Control,
    ) {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let sibling = pod_preset::HdType11Control::for_model(model_id)
            .iter()
            .copied()
            .find(|candidate| *candidate != control)
            .unwrap();
        let original = sample_hd_type11_preset(model_id);
        let index = app.hd_library.import_bytes(original.clone()).unwrap();
        app.select_hd_file(index);
        app.pod_open = true;
        app.hd_effect_slot = 5;
        let slot = pod_preset::HdFxSlot::new(5).unwrap();

        let directory = tempfile::tempdir().unwrap();
        let baseline_path = directory.path().join("type11-baseline.5xe");
        app.dialogs = Box::new(FakeDialogs {
            pod_save: [Some(baseline_path.clone())].into(),
            ..Default::default()
        });
        hd_editor_frame(&context, &mut app, vec![]);
        click_hd_editor(&context, &mut app, "save-hd-copy");
        assert_hd_bytes_eq(
            &std::fs::read(&baseline_path).unwrap(),
            &original,
            "saved baseline contains the original source bytes",
        );
        assert!(!app.hd_library.is_dirty(index));

        click_hd_type11_choice(&context, &mut app, control, 4);
        let target_stage = app.hd_type11_pending[&(index, slot, control)];
        assert_eq!(
            target_stage,
            PendingHdType11Choice {
                file_index: index,
                slot,
                model_id,
                control,
                choice: pod_preset::HdType11Choice::Choice4,
            },
            "target stage is created through the visible choice control"
        );

        click_hd_type11_choice(&context, &mut app, sibling, 1);
        let sibling_initial = app.hd_type11_pending[&(index, slot, sibling)];
        assert_eq!(sibling_initial.choice, pod_preset::HdType11Choice::Choice1);
        click_hd_editor(&context, &mut app, hd_type11_apply_button(sibling));
        let mut dirty_bytes = original.clone();
        let sibling_row = if sibling == pod_preset::HdType11Control::for_model(model_id)[0] {
            3
        } else {
            8
        };
        let sibling_value =
            pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + sibling_row * 0x14 + 4;
        dirty_bytes[sibling_value..sibling_value + 4]
            .copy_from_slice(&0x3e80_0000u32.to_be_bytes());
        assert_hd_bytes_eq(
            app.hd_library.file(index).unwrap().as_bytes(),
            &dirty_bytes,
            "applying sibling creates dirty state without altering target source word",
        );
        assert!(app.hd_library.is_dirty(index));
        assert_eq!(
            app.hd_type11_pending.get(&(index, slot, control)),
            Some(&target_stage),
            "sibling Apply preserves target stage"
        );

        click_hd_type11_choice(&context, &mut app, sibling, 3);
        let sibling_stage = app.hd_type11_pending[&(index, slot, sibling)];
        assert_eq!(sibling_stage.choice, pod_preset::HdType11Choice::Choice3);
        assert_eq!(
            app.hd_type11_pending.get(&(index, slot, control)),
            Some(&target_stage)
        );
        assert_eq!(app.hd_type11_pending.len(), 2);

        let target_marker = pod_preset::H5EP_HEADER_BYTES
            + 0x428
            + 5 * 0x100
            + 0x10
            + (if control == pod_preset::HdType11Control::for_model(model_id)[0] {
                3
            } else {
                8
            }) * 0x14
            + 0x10;
        let mut invalid_bytes = dirty_bytes;
        invalid_bytes[target_marker] = 1;
        let target_model_at = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100;
        *app.hd_library.file_mut(index).unwrap() =
            PodPresetFile::parse(invalid_bytes.clone()).unwrap();
        assert_eq!(
            u32::from_be_bytes(
                app.hd_library.file(index).unwrap().as_bytes()
                    [target_model_at..target_model_at + 4]
                    .try_into()
                    .unwrap(),
            ),
            model_id,
            "making the target marker invalid does not change its model identity"
        );
        assert!(app.hd_library.is_dirty(index));
        hd_editor_frame(&context, &mut app, vec![]);
        click_hd_editor(&context, &mut app, hd_type11_apply_button(control));
        assert_hd_bytes_eq(
            app.hd_library.file(index).unwrap().as_bytes(),
            &invalid_bytes,
            "failed GUI Apply preserves invalid target marker and all file bytes",
        );
        assert!(app.hd_library.is_dirty(index));
        assert_eq!(
            app.hd_type11_pending.get(&(index, slot, control)),
            Some(&target_stage),
            "failed Apply retains the exact target pending record"
        );
        assert_eq!(
            app.hd_type11_pending.get(&(index, slot, sibling)),
            Some(&sibling_stage),
            "failed Apply retains the exact sibling pending record"
        );

        assert!(app.discard_next_dirty_pod());
        assert_hd_bytes_eq(
            app.hd_library.file(index).unwrap().as_bytes(),
            &original,
            "revert restores the exact saved baseline",
        );
        assert!(!app.hd_library.is_dirty(index));
        assert!(
            !app.hd_type11_pending
                .keys()
                .any(|(file_index, _, _)| *file_index == index),
            "revert clears only stages belonging to the reverted file"
        );
        assert!(
            app.pod_message
                .contains("Discarded one unsaved HD Edit file change")
        );
    }

    #[test]
    fn hd_type11_cancelled_and_successful_open_handle_stages_and_model_context() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let controls = pod_preset::HdType11Control::for_model(0x020a_001e);
        let original = sample_hd_type11_preset(0x020a_001e);
        let index = app.hd_library.import_bytes(original.clone()).unwrap();
        app.select_hd_file(index);
        app.pod_open = true;
        app.hd_effect_slot = 5;
        click_hd_type11_choice(&context, &mut app, controls[0], 1);
        click_hd_type11_choice(&context, &mut app, controls[1], 3);
        assert_eq!(app.hd_type11_pending.len(), 2);
        click_hd_type12_choice(&context, &mut app, 1);
        assert_eq!(app.hd_type12_pending.len(), 1);

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("opened.5xe");
        let replacement_bytes = sample_hd_type11_preset(0x020a_001d);
        std::fs::write(&path, &replacement_bytes).unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_open: [None, Some(path)].into(),
            ..Default::default()
        });
        app.load_hd_editor_file();
        assert_eq!(
            app.hd_type11_pending.len(),
            2,
            "cancelled Open preserves stages"
        );
        assert_eq!(app.hd_type12_pending.len(), 1);
        assert_hd_bytes_eq(
            app.hd_library.file(index).unwrap().as_bytes(),
            &original,
            "cancelled Open does not mutate the existing entry",
        );
        app.load_hd_editor_file();
        assert!(
            app.hd_type11_pending.is_empty(),
            "successful Open invalidates prior entry stages"
        );
        assert!(
            app.hd_type12_pending.is_empty(),
            "successful Open invalidates prior type-12 stage"
        );
        let opened = app.hd_selected.unwrap();
        assert_ne!(opened, index);
        assert_hd_bytes_eq(
            app.hd_library.file(opened).unwrap().as_bytes(),
            &replacement_bytes,
            "successful Open retains the newly parsed source",
        );
        assert_hd_bytes_eq(
            app.hd_library.file(index).unwrap().as_bytes(),
            &original,
            "successful Open preserves the previous entry bytes",
        );

        app.select_hd_file(index);
        app.hd_effect_slot = 5;
        click_hd_type11_choice(&context, &mut app, controls[0], 4);
        assert_eq!(app.hd_type11_pending.len(), 1);
        click_hd_type12_choice(&context, &mut app, 0);
        assert_eq!(app.hd_type12_pending.len(), 1);
        let replacement = sample_hd_type11_preset(0x020a_001d);
        let parsed = pod_preset::OpaqueH5epPreset::parse(replacement).unwrap();
        *app.hd_library.file_mut(index).unwrap() = PodPresetFile::H5ep(parsed);
        hd_editor_frame(&context, &mut app, vec![]);
        assert!(
            app.hd_type11_pending.is_empty(),
            "same-entry model change invalidates old-model control stage"
        );
        assert!(
            app.hd_type12_pending.is_empty(),
            "same-entry model change invalidates type-12 stage"
        );
    }

    #[test]
    fn hd_type11_save_collision_reopen_and_revert_keep_stage_context_safe() {
        for model_id in [0x020a_001e, 0x020a_001d] {
            hd_type11_save_collision_reopen_and_revert_for_model(model_id);
        }
    }

    fn hd_type11_save_collision_reopen_and_revert_for_model(model_id: u32) {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let controls = pod_preset::HdType11Control::for_model(model_id);
        let type12_control = pod_preset::HdType12Control::for_model(model_id)
            .first()
            .copied();
        let original = sample_hd_type11_preset(model_id);
        let index = app.hd_library.import_bytes(original.clone()).unwrap();
        app.select_hd_file(index);
        app.pod_open = true;
        app.hd_effect_slot = 5;
        let slot = pod_preset::HdFxSlot::new(5).unwrap();
        click_hd_type11_choice(&context, &mut app, controls[0], 1);
        click_hd_type11_choice(&context, &mut app, controls[1], 3);
        if type12_control.is_some() {
            click_hd_type12_choice(&context, &mut app, 1);
        }
        let second_stage = app.hd_type11_pending[&(index, slot, controls[1])];
        let type12_stage =
            type12_control.map(|control| app.hd_type12_pending[&(index, slot, control)]);
        click_hd_editor(&context, &mut app, hd_type11_apply_button(controls[0]));
        let value = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + 3 * 0x14 + 4;
        let mut applied_first = original.clone();
        applied_first[value..value + 4].copy_from_slice(&0x3e80_0000u32.to_be_bytes());
        assert_hd_bytes_eq(
            app.hd_library.file(index).unwrap().as_bytes(),
            &applied_first,
            "first control applied before save",
        );
        assert_eq!(
            app.hd_type11_pending.get(&(index, slot, controls[1])),
            Some(&second_stage)
        );
        if let (Some(control), Some(stage)) = (type12_control, type12_stage) {
            assert_eq!(
                app.hd_type12_pending.get(&(index, slot, control)),
                Some(&stage)
            );
        } else {
            assert!(app.hd_type12_pending.is_empty());
        }

        let directory = tempfile::tempdir().unwrap();
        let collision = directory.path().join("occupied.5xe");
        let saved = directory.path().join("type11-copy.5xe");
        std::fs::write(&collision, b"preserve existing").unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_save: [Some(collision.clone()), Some(saved.clone())].into(),
            ..Default::default()
        });
        assert!(!app.write_hd_copy(index));
        assert_eq!(std::fs::read(&collision).unwrap(), b"preserve existing");
        assert!(app.hd_library.is_dirty(index));
        assert_eq!(
            app.hd_type11_pending.get(&(index, slot, controls[1])),
            Some(&second_stage)
        );
        if let (Some(control), Some(stage)) = (type12_control, type12_stage) {
            assert_eq!(
                app.hd_type12_pending.get(&(index, slot, control)),
                Some(&stage)
            );
        } else {
            assert!(app.hd_type12_pending.is_empty());
        }
        assert!(app.write_hd_copy(index));
        assert_eq!(std::fs::read(&saved).unwrap(), applied_first);
        assert!(!app.hd_library.is_dirty(index));
        assert_eq!(
            app.hd_type11_pending.get(&(index, slot, controls[1])),
            Some(&second_stage)
        );
        if let (Some(control), Some(stage)) = (type12_control, type12_stage) {
            assert_eq!(
                app.hd_type12_pending.get(&(index, slot, control)),
                Some(&stage)
            );
        } else {
            assert!(app.hd_type12_pending.is_empty());
        }

        let reopened_bytes = std::fs::read(&saved).unwrap();
        let reopened = app.hd_library.import_bytes(reopened_bytes.clone()).unwrap();
        app.select_hd_file(reopened);
        assert_eq!(
            app.hd_library.file(reopened).unwrap().as_bytes(),
            reopened_bytes
        );
        assert!(app.hd_type11_pending.is_empty());
        assert!(app.hd_type12_pending.is_empty());
        assert!(!app.hd_library.is_dirty(reopened));

        app.select_hd_file(index);
        app.hd_effect_slot = 5;
        click_hd_type11_choice(&context, &mut app, controls[1], 2);
        if type12_control.is_some() {
            click_hd_type12_choice(&context, &mut app, 1);
        }
        assert_eq!(app.hd_type11_pending.len(), 1);
        click_hd_editor(&context, &mut app, hd_type11_apply_button(controls[1]));
        assert!(app.hd_library.is_dirty(index));
        assert!(app.discard_next_dirty_pod());
        assert_hd_bytes_eq(
            app.hd_library.file(index).unwrap().as_bytes(),
            &applied_first,
            "revert restores the post-save baseline",
        );
        assert!(!app.hd_library.is_dirty(index));
        assert!(app.hd_type11_pending.is_empty());
        assert!(app.hd_type12_pending.is_empty());

        app.select_hd_file(index);
        app.hd_effect_slot = 5;
        click_hd_type11_choice(&context, &mut app, controls[0], 4);
        assert_eq!(app.hd_type11_pending.len(), 1);
        app.select_hd_file(reopened);
        assert!(
            app.hd_type11_pending.is_empty(),
            "selection invalidates stale entry stage"
        );
        app.hd_effect_slot = 5;
        click_hd_type11_choice(&context, &mut app, controls[0], 2);
        assert_eq!(app.hd_type11_pending.len(), 1);
        app.hd_effect_slot = 4;
        hd_editor_frame(&context, &mut app, vec![]);
        assert!(
            app.hd_type11_pending.is_empty(),
            "slot context change clears stale stages"
        );
    }

    #[test]
    fn hd_type12_gui_coexists_with_both_same_model_type11_stages() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let model_id = 0x020a_001e;
        let controls = pod_preset::HdType11Control::for_model(0x020a_001e);
        let control12 = pod_preset::HdType12Control::ALL[0];
        let source = sample_hd_type11_preset(model_id);
        let index = app.hd_library.import_bytes(source.clone()).unwrap();
        app.select_hd_file(index);
        app.pod_open = true;
        app.hd_effect_slot = 5;
        let slot = pod_preset::HdFxSlot::new(5).unwrap();
        let expected_type11 = |control, choice| PendingHdType11Choice {
            file_index: index,
            slot,
            model_id,
            control,
            choice,
        };
        let expected_type12 = |choice| PendingHdType12Choice {
            file_index: index,
            slot,
            model_id,
            control: control12,
            choice,
        };

        // Every independent discard starts with all three exact GUI stages.
        click_hd_type12_choice(&context, &mut app, 1);
        click_hd_type11_choice(&context, &mut app, controls[0], 1);
        click_hd_type11_choice(&context, &mut app, controls[1], 4);
        let type12_one = expected_type12(pod_preset::HdType12Choice::Choice1);
        let type11_first = expected_type11(controls[0], pod_preset::HdType11Choice::Choice1);
        let type11_second = expected_type11(controls[1], pod_preset::HdType11Choice::Choice4);
        assert_eq!(app.hd_type12_pending.len(), 1);
        assert_eq!(app.hd_type11_pending.len(), 2);
        assert_eq!(
            app.hd_type12_pending.get(&(index, slot, control12)),
            Some(&type12_one)
        );
        assert_eq!(
            app.hd_type11_pending.get(&(index, slot, controls[0])),
            Some(&type11_first)
        );
        assert_eq!(
            app.hd_type11_pending.get(&(index, slot, controls[1])),
            Some(&type11_second)
        );
        assert_hd_bytes_eq(
            app.hd_library.file(index).unwrap().as_bytes(),
            &source,
            "staging all controls is byte neutral",
        );

        click_hd_editor(&context, &mut app, hd_type12_discard_button());
        assert!(app.hd_type12_pending.is_empty());
        assert_eq!(
            app.hd_type11_pending.get(&(index, slot, controls[0])),
            Some(&type11_first)
        );
        assert_eq!(
            app.hd_type11_pending.get(&(index, slot, controls[1])),
            Some(&type11_second)
        );
        assert_hd_bytes_eq(
            app.hd_library.file(index).unwrap().as_bytes(),
            &source,
            "discarding type-12 leaves all preset bytes unchanged",
        );

        click_hd_type12_choice(&context, &mut app, 0);
        assert_eq!(
            app.hd_type12_pending.get(&(index, slot, control12)),
            Some(&expected_type12(pod_preset::HdType12Choice::Choice0))
        );
        assert_eq!(
            app.hd_type11_pending.get(&(index, slot, controls[0])),
            Some(&type11_first)
        );
        assert_eq!(
            app.hd_type11_pending.get(&(index, slot, controls[1])),
            Some(&type11_second)
        );
        click_hd_editor(&context, &mut app, hd_type11_discard_button(controls[0]));
        assert!(
            !app.hd_type11_pending
                .contains_key(&(index, slot, controls[0]))
        );
        assert_eq!(
            app.hd_type11_pending.get(&(index, slot, controls[1])),
            Some(&type11_second)
        );
        assert_eq!(
            app.hd_type12_pending.get(&(index, slot, control12)),
            Some(&expected_type12(pod_preset::HdType12Choice::Choice0))
        );
        assert_hd_bytes_eq(
            app.hd_library.file(index).unwrap().as_bytes(),
            &source,
            "discarding first type-11 leaves all preset bytes unchanged",
        );

        click_hd_type11_choice(&context, &mut app, controls[0], 1);
        assert_eq!(
            app.hd_type12_pending.get(&(index, slot, control12)),
            Some(&expected_type12(pod_preset::HdType12Choice::Choice0))
        );
        assert_eq!(
            app.hd_type11_pending.get(&(index, slot, controls[0])),
            Some(&type11_first)
        );
        assert_eq!(
            app.hd_type11_pending.get(&(index, slot, controls[1])),
            Some(&type11_second)
        );
        click_hd_editor(&context, &mut app, hd_type11_discard_button(controls[1]));
        assert!(
            !app.hd_type11_pending
                .contains_key(&(index, slot, controls[1]))
        );
        assert_eq!(
            app.hd_type11_pending.get(&(index, slot, controls[0])),
            Some(&type11_first)
        );
        assert_eq!(
            app.hd_type12_pending.get(&(index, slot, control12)),
            Some(&expected_type12(pod_preset::HdType12Choice::Choice0))
        );
        assert_hd_bytes_eq(
            app.hd_library.file(index).unwrap().as_bytes(),
            &source,
            "discarding second type-11 leaves all preset bytes unchanged",
        );

        // Each Apply starts with all three exact records pending. Expected full
        // buffers prove the two other rows and all unrelated bytes survive.
        let (context, mut app) = self::app();
        context.style_mut(|style| style.animation_time = 0.0);
        let index = app.hd_library.import_bytes(source.clone()).unwrap();
        app.select_hd_file(index);
        app.pod_open = true;
        app.hd_effect_slot = 5;
        let slot = pod_preset::HdFxSlot::new(5).unwrap();
        let row_value_offset =
            |row: usize| pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + row * 0x14 + 4;
        let expected_type11 = |control, choice| PendingHdType11Choice {
            file_index: index,
            slot,
            model_id,
            control,
            choice,
        };
        let expected_type12 = |choice| PendingHdType12Choice {
            file_index: index,
            slot,
            model_id,
            control: control12,
            choice,
        };
        let type11_first = expected_type11(controls[0], pod_preset::HdType11Choice::Choice1);
        let type11_second = expected_type11(controls[1], pod_preset::HdType11Choice::Choice4);
        let type12_one = expected_type12(pod_preset::HdType12Choice::Choice1);

        click_hd_type11_choice(&context, &mut app, controls[0], 1);
        click_hd_type11_choice(&context, &mut app, controls[1], 4);
        click_hd_type12_choice(&context, &mut app, 1);
        assert_eq!(
            app.hd_type12_pending.get(&(index, slot, control12)),
            Some(&type12_one)
        );
        assert_eq!(
            app.hd_type11_pending.get(&(index, slot, controls[0])),
            Some(&type11_first)
        );
        assert_eq!(
            app.hd_type11_pending.get(&(index, slot, controls[1])),
            Some(&type11_second)
        );
        let mut expected_bytes = source.clone();
        let row12 = row_value_offset(4);
        let row11a = row_value_offset(3);
        let row11b = row_value_offset(8);
        expected_bytes[row12..row12 + 4].copy_from_slice(&0x3f80_0000u32.to_be_bytes());
        click_hd_editor(&context, &mut app, hd_type12_apply_button());
        assert!(app.hd_type12_pending.is_empty());
        assert_eq!(
            app.hd_type11_pending.get(&(index, slot, controls[0])),
            Some(&type11_first)
        );
        assert_eq!(
            app.hd_type11_pending.get(&(index, slot, controls[1])),
            Some(&type11_second)
        );
        assert_hd_bytes_eq(
            app.hd_library.file(index).unwrap().as_bytes(),
            &expected_bytes,
            "type-12 Apply changes only its own value word",
        );

        click_hd_type12_choice(&context, &mut app, 0);
        let type12_zero = expected_type12(pod_preset::HdType12Choice::Choice0);
        assert_eq!(
            app.hd_type12_pending.get(&(index, slot, control12)),
            Some(&type12_zero)
        );
        assert_eq!(
            app.hd_type11_pending.get(&(index, slot, controls[0])),
            Some(&type11_first)
        );
        assert_eq!(
            app.hd_type11_pending.get(&(index, slot, controls[1])),
            Some(&type11_second)
        );
        expected_bytes[row11a..row11a + 4].copy_from_slice(&0x3e80_0000u32.to_be_bytes());
        click_hd_editor(&context, &mut app, hd_type11_apply_button(controls[0]));
        assert_eq!(
            app.hd_type12_pending.get(&(index, slot, control12)),
            Some(&type12_zero),
            "type-11 Apply preserves the exact type-12 record"
        );
        assert!(
            !app.hd_type11_pending
                .contains_key(&(index, slot, controls[0]))
        );
        assert_eq!(
            app.hd_type11_pending.get(&(index, slot, controls[1])),
            Some(&type11_second)
        );
        assert_hd_bytes_eq(
            app.hd_library.file(index).unwrap().as_bytes(),
            &expected_bytes,
            "first type-11 Apply changes only its own value word",
        );

        // Restage the already-applied first control so this Apply also begins
        // with all three pending records and verifies preservation exactly.
        click_hd_type11_choice(&context, &mut app, controls[0], 2);
        let type11_first_again = expected_type11(controls[0], pod_preset::HdType11Choice::Choice2);
        assert_eq!(
            app.hd_type12_pending.get(&(index, slot, control12)),
            Some(&type12_zero)
        );
        assert_eq!(
            app.hd_type11_pending.get(&(index, slot, controls[0])),
            Some(&type11_first_again)
        );
        assert_eq!(
            app.hd_type11_pending.get(&(index, slot, controls[1])),
            Some(&type11_second)
        );
        expected_bytes[row12..row12 + 4].copy_from_slice(&0u32.to_be_bytes());
        click_hd_editor(&context, &mut app, hd_type12_apply_button());
        assert!(app.hd_type12_pending.is_empty());
        assert_eq!(
            app.hd_type11_pending.get(&(index, slot, controls[0])),
            Some(&type11_first_again)
        );
        assert_eq!(
            app.hd_type11_pending.get(&(index, slot, controls[1])),
            Some(&type11_second)
        );
        assert_hd_bytes_eq(
            app.hd_library.file(index).unwrap().as_bytes(),
            &expected_bytes,
            "type-12 Choice 0 Apply changes only its own value word",
        );

        let type12_one_again = expected_type12(pod_preset::HdType12Choice::Choice1);
        click_hd_type12_choice(&context, &mut app, 1);
        assert_eq!(
            app.hd_type12_pending.get(&(index, slot, control12)),
            Some(&type12_one_again)
        );
        assert_eq!(
            app.hd_type11_pending.get(&(index, slot, controls[0])),
            Some(&type11_first_again)
        );
        assert_eq!(
            app.hd_type11_pending.get(&(index, slot, controls[1])),
            Some(&type11_second)
        );
        expected_bytes[row11b..row11b + 4].copy_from_slice(&0x3f80_0000u32.to_be_bytes());
        click_hd_editor(&context, &mut app, hd_type11_apply_button(controls[1]));
        assert_eq!(
            app.hd_type12_pending.get(&(index, slot, control12)),
            Some(&type12_one_again),
            "second type-11 Apply preserves the exact type-12 record"
        );
        assert_eq!(
            app.hd_type11_pending.get(&(index, slot, controls[0])),
            Some(&type11_first_again)
        );
        assert!(
            !app.hd_type11_pending
                .contains_key(&(index, slot, controls[1]))
        );
        assert_hd_bytes_eq(
            app.hd_library.file(index).unwrap().as_bytes(),
            &expected_bytes,
            "second type-11 Apply changes only its own value word",
        );

        // Rebuild all three records before the final type-12 discard.
        click_hd_type11_choice(&context, &mut app, controls[1], 3);
        let type11_second_again = expected_type11(controls[1], pod_preset::HdType11Choice::Choice3);
        assert_eq!(
            app.hd_type12_pending.get(&(index, slot, control12)),
            Some(&type12_one_again)
        );
        assert_eq!(
            app.hd_type11_pending.get(&(index, slot, controls[0])),
            Some(&type11_first_again)
        );
        assert_eq!(
            app.hd_type11_pending.get(&(index, slot, controls[1])),
            Some(&type11_second_again)
        );
        click_hd_editor(&context, &mut app, hd_type12_discard_button());
        assert!(app.hd_type12_pending.is_empty());
        assert_eq!(
            app.hd_type11_pending.get(&(index, slot, controls[0])),
            Some(&type11_first_again)
        );
        assert_eq!(
            app.hd_type11_pending.get(&(index, slot, controls[1])),
            Some(&type11_second_again)
        );
        assert_hd_bytes_eq(
            app.hd_library.file(index).unwrap().as_bytes(),
            &expected_bytes,
            "discarding type-12 leaves applied type-11 output unchanged",
        );
        assert!(app.hd_library.is_dirty(index));
        assert_eq!(
            control12,
            pod_preset::HdType12Control::Model020a001eKey3f100004
        );
    }

    #[test]
    fn hd_type12_failed_apply_retains_dirty_baseline_and_sibling_stages() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let controls = pod_preset::HdType11Control::for_model(0x020a_001e);
        let source = sample_hd_type11_preset(0x020a_001e);
        let index = app.hd_library.import_bytes(source.clone()).unwrap();
        app.select_hd_file(index);
        app.pod_open = true;
        app.hd_effect_slot = 5;
        let slot = pod_preset::HdFxSlot::new(5).unwrap();
        click_hd_type12_choice(&context, &mut app, 1);
        click_hd_type11_choice(&context, &mut app, controls[0], 1);
        click_hd_type11_choice(&context, &mut app, controls[1], 4);
        assert_eq!(app.hd_type12_pending.len(), 1);
        assert_eq!(app.hd_type11_pending.len(), 2);

        // An independent sibling-row edit creates real dirty state while the
        // target type-12 and other type-11 stages remain pending.
        click_hd_editor(&context, &mut app, hd_type11_apply_button(controls[0]));
        assert!(app.hd_library.is_dirty(index));
        assert_eq!(app.hd_type12_pending.len(), 1);
        assert_eq!(app.hd_type11_pending.len(), 1);
        let type12 = app.hd_type12_pending[&(index, slot, pod_preset::HdType12Control::ALL[0])];
        let sibling = app.hd_type11_pending[&(index, slot, controls[1])];

        let mut invalid = app.hd_library.file(index).unwrap().as_bytes().to_vec();
        let marker = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100 + 0x10 + 4 * 0x14 + 0x10;
        invalid[marker] = 1;
        *app.hd_library.file_mut(index).unwrap() =
            PodPresetFile::H5ep(pod_preset::OpaqueH5epPreset::parse(invalid).unwrap());
        let invalid_before = app.hd_library.file(index).unwrap().as_bytes().to_vec();
        assert_eq!(
            app.hd_type12_pending
                .get(&(index, slot, pod_preset::HdType12Control::ALL[0])),
            Some(&type12),
            "the exact staged type-12 record remains before invalidation and rejected Apply"
        );
        click_hd_editor(&context, &mut app, hd_type12_apply_button());
        assert_eq!(
            app.hd_library.file(index).unwrap().as_bytes(),
            invalid_before
        );
        assert!(app.hd_library.is_dirty(index));
        assert_eq!(
            app.hd_type12_pending
                .get(&(index, slot, pod_preset::HdType12Control::ALL[0])),
            Some(&type12),
            "rejected Apply retains the exact type-12 record"
        );
        assert_eq!(
            app.hd_type11_pending.get(&(index, slot, controls[1])),
            Some(&sibling)
        );

        assert!(app.discard_next_dirty_pod());
        assert_eq!(app.hd_library.file(index).unwrap().as_bytes(), source);
        assert!(!app.hd_library.is_dirty(index));
        assert!(app.hd_type12_pending.is_empty());
        assert!(app.hd_type11_pending.is_empty());
    }

    #[test]
    fn hd_type12_quit_resolves_all_three_stages_before_dirty_file_decision() {
        let controls = pod_preset::HdType11Control::for_model(0x020a_001e);
        let source = sample_hd_type11_preset(0x020a_001e);

        let (context, mut app1) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let index = app1.hd_library.import_bytes(source.clone()).unwrap();
        app1.select_hd_file(index);
        app1.pod_open = true;
        app1.hd_effect_slot = 5;
        click_hd_type12_choice(&context, &mut app1, 1);
        click_hd_type11_choice(&context, &mut app1, controls[0], 1);
        click_hd_type11_choice(&context, &mut app1, controls[1], 4);
        assert_eq!(app1.hd_type12_pending.len(), 1);
        assert_eq!(app1.hd_type11_pending.len(), 2);
        app1.request_close(&context);
        click_prompt(&context, &mut app1, "discard");
        click_prompt(&context, &mut app1, "discard");
        click_prompt(&context, &mut app1, "discard");
        assert!(app1.permit_close);
        assert!(app1.hd_type12_pending.is_empty());
        assert!(app1.hd_type11_pending.is_empty());
        assert_eq!(app1.hd_library.file(index).unwrap().as_bytes(), source);

        let (context2, mut app2) = app();
        context2.style_mut(|style| style.animation_time = 0.0);
        let index2 = app2.hd_library.import_bytes(source.clone()).unwrap();
        app2.select_hd_file(index2);
        app2.pod_open = true;
        app2.hd_effect_slot = 5;
        click_hd_type12_choice(&context2, &mut app2, 0);
        click_hd_type11_choice(&context2, &mut app2, controls[0], 1);
        click_hd_type11_choice(&context2, &mut app2, controls[1], 3);
        app2.request_close(&context2);
        click_prompt(&context2, &mut app2, "apply-type11");
        assert_eq!(app2.hd_type11_pending.len(), 1);
        click_prompt(&context2, &mut app2, "apply-type11");
        assert!(app2.hd_type11_pending.is_empty());
        assert_eq!(app2.hd_type12_pending.len(), 1);
        assert!(!app2.permit_close);
        click_prompt(&context2, &mut app2, "apply-type12");
        assert!(app2.hd_type12_pending.is_empty());
        assert!(app2.hd_library.is_dirty(index2));
        assert!(!app2.permit_close, "dirty-file decision still blocks close");
        click_prompt(&context2, &mut app2, "cancel");
        assert!(!app2.permit_close, "Keep editing must not close");
        app2.request_close(&context2);
        click_prompt(&context2, &mut app2, "discard");
        assert!(app2.permit_close);
        assert_eq!(app2.hd_library.file(index2).unwrap().as_bytes(), source);

        // Keep editing must dismiss the staged-control quit prompt before any
        // of the three records can be applied or discarded.
        let (context3, mut app3) = app();
        context3.style_mut(|style| style.animation_time = 0.0);
        let index3 = app3.hd_library.import_bytes(source.clone()).unwrap();
        app3.select_hd_file(index3);
        app3.pod_open = true;
        app3.hd_effect_slot = 5;
        let slot3 = pod_preset::HdFxSlot::new(5).unwrap();
        let control12 = pod_preset::HdType12Control::ALL[0];
        click_hd_type12_choice(&context3, &mut app3, 1);
        click_hd_type11_choice(&context3, &mut app3, controls[0], 1);
        click_hd_type11_choice(&context3, &mut app3, controls[1], 4);
        let expected_type12 = PendingHdType12Choice {
            file_index: index3,
            slot: slot3,
            model_id: 0x020a_001e,
            control: control12,
            choice: pod_preset::HdType12Choice::Choice1,
        };
        let expected_type11_first = PendingHdType11Choice {
            file_index: index3,
            slot: slot3,
            model_id: 0x020a_001e,
            control: controls[0],
            choice: pod_preset::HdType11Choice::Choice1,
        };
        let expected_type11_second = PendingHdType11Choice {
            file_index: index3,
            slot: slot3,
            model_id: 0x020a_001e,
            control: controls[1],
            choice: pod_preset::HdType11Choice::Choice4,
        };
        assert_eq!(app3.hd_type12_pending.len(), 1);
        assert_eq!(app3.hd_type11_pending.len(), 2);
        assert_eq!(
            app3.hd_type12_pending.get(&(index3, slot3, control12)),
            Some(&expected_type12)
        );
        assert_eq!(
            app3.hd_type11_pending.get(&(index3, slot3, controls[0])),
            Some(&expected_type11_first)
        );
        assert_eq!(
            app3.hd_type11_pending.get(&(index3, slot3, controls[1])),
            Some(&expected_type11_second)
        );
        app3.request_close(&context3);
        assert!(matches!(app3.pending, Some(Pending::QuitPod)));
        click_prompt(&context3, &mut app3, "cancel");
        assert!(
            app3.pending.is_none(),
            "Keep editing dismisses the staged-control prompt"
        );
        assert!(!app3.permit_close, "Keep editing never closes the app");
        assert_eq!(
            app3.hd_type12_pending.get(&(index3, slot3, control12)),
            Some(&expected_type12)
        );
        assert_eq!(
            app3.hd_type11_pending.get(&(index3, slot3, controls[0])),
            Some(&expected_type11_first)
        );
        assert_eq!(
            app3.hd_type11_pending.get(&(index3, slot3, controls[1])),
            Some(&expected_type11_second)
        );
        assert_hd_bytes_eq(
            app3.hd_library.file(index3).unwrap().as_bytes(),
            &source,
            "Keep editing with three staged controls leaves all bytes unchanged",
        );
    }

    #[test]
    fn hd_type11_quit_processes_every_staged_control_before_dirty_file_prompt() {
        for model_id in [0x020a_001e, 0x020a_001d] {
            hd_type11_quit_processes_every_staged_control_for(model_id);
        }
    }

    fn hd_type11_quit_processes_every_staged_control_for(model_id: u32) {
        let (context3, mut app3) = app();
        context3.style_mut(|style| style.animation_time = 0.0);
        let (context2, mut app2) = app();
        context2.style_mut(|style| style.animation_time = 0.0);
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let original = sample_hd_type11_preset(model_id);
        let controls = pod_preset::HdType11Control::for_model(model_id);
        let index = app.hd_library.import_bytes(original.clone()).unwrap();
        app.select_hd_file(index);
        app.pod_open = true;
        app.hd_effect_slot = 5;
        let slot = pod_preset::HdFxSlot::new(5).unwrap();
        click_hd_type11_choice(&context, &mut app, controls[0], 1);
        click_hd_type11_choice(&context, &mut app, controls[1], 4);
        assert_eq!(app.hd_type11_pending.len(), 2);
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "discard");
        assert_eq!(app.hd_type11_pending.len(), 1);
        assert!(!app.permit_close);
        click_prompt(&context, &mut app, "discard");
        assert!(app.hd_type11_pending.is_empty());
        assert_hd_bytes_eq(
            app.hd_library.file(index).unwrap().as_bytes(),
            &original,
            "quit discard processes both pending controls",
        );
        assert!(app.permit_close);

        let index2 = app2.hd_library.import_bytes(original.clone()).unwrap();
        app2.select_hd_file(index2);
        app2.pod_open = true;
        app2.hd_effect_slot = 5;
        click_hd_type11_choice(&context2, &mut app2, controls[0], 1);
        click_hd_type11_choice(&context2, &mut app2, controls[1], 3);
        assert_eq!(app2.hd_type11_pending.len(), 2);
        app2.request_close(&context2);
        click_prompt(&context2, &mut app2, "apply-type11");
        assert_eq!(app2.hd_type11_pending.len(), 1);
        assert!(!app2.permit_close);
        click_prompt(&context2, &mut app2, "apply-type11");
        assert!(app2.hd_type11_pending.is_empty());
        assert!(app2.hd_library.is_dirty(index2));
        assert!(!app2.permit_close);
        click_prompt(&context2, &mut app2, "discard");
        assert!(!app2.hd_library.is_dirty(index2));
        assert_hd_bytes_eq(
            app2.hd_library.file(index2).unwrap().as_bytes(),
            &original,
            "quit apply processes both pending controls before file revert",
        );
        assert!(app2.permit_close);
        assert_eq!(slot.index(), 5);

        let index3 = app3.hd_library.import_bytes(original.clone()).unwrap();
        app3.select_hd_file(index3);
        app3.pod_open = true;
        app3.hd_effect_slot = 5;
        click_hd_type11_choice(&context3, &mut app3, controls[0], 2);
        click_hd_type11_choice(&context3, &mut app3, controls[1], 4);
        let first = app3.hd_type11_pending[&(index3, slot, controls[0])];
        let second = app3.hd_type11_pending[&(index3, slot, controls[1])];
        assert_eq!(app3.hd_type11_pending.len(), 2);
        app3.request_close(&context3);
        assert!(matches!(app3.pending, Some(Pending::QuitPod)));
        click_prompt(&context3, &mut app3, "cancel");
        assert!(app3.pending.is_none());
        assert!(!app3.permit_close);
        assert_eq!(
            app3.hd_type11_pending.get(&(index3, slot, controls[0])),
            Some(&first),
            "Keep editing preserves the exact first stage"
        );
        assert_eq!(
            app3.hd_type11_pending.get(&(index3, slot, controls[1])),
            Some(&second),
            "Keep editing preserves the exact second stage"
        );
        assert_hd_bytes_eq(
            app3.hd_library.file(index3).unwrap().as_bytes(),
            &original,
            "Keep editing leaves source bytes unchanged",
        );
    }

    #[test]
    fn hd_editor_library_workflow_edits_and_saves_a_new_copy_offline() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let source = app.document.as_ref().unwrap().as_bytes().to_vec();
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("source.5xe");
        let output = directory.path().join("copy.5xe");
        let blocked_output = directory.path().join("blocked-output");
        std::fs::create_dir(&blocked_output).unwrap();
        let sentinel = blocked_output.join("keep.bin");
        std::fs::write(&sentinel, b"keep").unwrap();
        let original = sample_hd_editor_preset();
        std::fs::write(&input, &original).unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_open: [Some(input.clone())].into(),
            pod_save: [Some(blocked_output.clone()), Some(output.clone())].into(),
            ..Default::default()
        });

        app.load_hd_editor_file();
        assert_eq!(app.hd_library.len(), 1);
        assert_eq!(app.pod_library.len(), 0);
        assert_eq!(app.hd_selected, Some(0));
        assert_eq!(app.hd_control_percent, 50);
        app.hd_control_percent = 75;
        app.apply_hd_control(0);
        app.hd_control = pod_preset::HdType1Control::TubeDrive(pod_preset::TubeDriveControl::Mid);
        assert_eq!(
            app.hd_library.file(0).and_then(|file| match file {
                PodPresetFile::H5ep(preset) =>
                    preset.tube_drive_control_percent(0, pod_preset::TubeDriveControl::Mid),
                _ => None,
            }),
            Some(35)
        );
        app.hd_control_percent = 40;
        app.apply_hd_control(0);
        assert!(app.hd_library.is_dirty(0));
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), source);
        assert_eq!(std::fs::read(&input).unwrap(), original);

        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        prompt_frame(&context, &mut app, vec![]);
        click_prompt(&context, &mut app, "save");
        assert!(!app.permit_close);
        assert!(app.hd_library.is_dirty(0));
        assert_eq!(std::fs::read(&sentinel).unwrap(), b"keep");
        click_prompt(&context, &mut app, "save");
        assert!(app.permit_close);
        assert!(!app.hd_library.any_dirty());
        let written = std::fs::read(&output).unwrap();
        assert_eq!(written.len(), original.len());
        assert_eq!(
            &written[0x28 + 0x428 + 0x10 + 4..0x28 + 0x428 + 0x10 + 8],
            &0.75f32.to_bits().to_be_bytes()
        );
        assert_eq!(
            &written[0x28 + 0x428 + 0x10 + 2 * 0x14 + 4..0x28 + 0x428 + 0x10 + 2 * 0x14 + 8],
            &0.4f32.to_bits().to_be_bytes()
        );
        assert!(app.read_worker.is_none());
        assert!(!app.is_reading());
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
    }

    #[derive(Debug, Default)]
    struct HdExplicitReplacementTargets {
        distortion: Option<pod_preset::HdDistortionDefault>,
        dynamics: Option<pod_preset::HdDynamicsDefault>,
        reverb: Option<pod_preset::HdReverbDefault>,
    }

    fn exercise_hd_editor_explicit_replacement_lifecycle(
        target_model: u32,
        default_keys: &[u32],
        button_id: &'static str,
        message_label: &str,
        output_name: &str,
        targets: HdExplicitReplacementTargets,
    ) {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let source_document = app.document.as_ref().unwrap().as_bytes().to_vec();
        let mut original = sample_hd_editor_preset();
        let block = pod_preset::H5EP_HEADER_BYTES + 0x428;
        original[block..block + 4].copy_from_slice(&0x0205_000cu32.to_be_bytes());
        original[block + 4..block + 8].copy_from_slice(&0x1234_5678u32.to_be_bytes());
        original[block + 8..block + 0x0c].copy_from_slice(&[1, 0x5a, 0xa5, 1]);
        original[block + 0x0f] = 8;
        let keys = [
            0x3f10_0005u32,
            0x3f10_0001,
            0x3f10_0003,
            0x3f10_0002,
            0x3f10_0004,
        ];
        for (row, key) in keys.iter().copied().enumerate().chain((5..12).map(|row| {
            let key = if row == 6 {
                0x3f10_0005
            } else {
                0x6abc_0000 + row as u32
            };
            (row, key)
        })) {
            let row_at = block + 0x10 + row * 0x14;
            original[row_at..row_at + 4].copy_from_slice(&key.to_be_bytes());
            original[row_at + 4..row_at + 8]
                .copy_from_slice(&(0x3e80_0000u32 + row as u32).to_be_bytes());
            original[row_at + 8..row_at + 12]
                .copy_from_slice(&(0x7100_0000u32 + row as u32).to_be_bytes());
            original[row_at + 0x10] = (row % 3) as u8;
            original[row_at + 0x11..row_at + 0x14].copy_from_slice(&[
                0xc1 + row as u8,
                0xd1 + row as u8,
                0xe1 + row as u8,
            ]);
        }
        let index = app.hd_library.import_bytes(original.clone()).unwrap();
        app.select_hd_file(index);
        if let Some(target) = targets.distortion {
            app.hd_default_target = target;
        }
        if let Some(target) = targets.dynamics {
            app.hd_dynamics_default_target = target;
        }
        if let Some(target) = targets.reverb {
            app.hd_reverb_default_target = target;
        }
        app.pod_open = true;
        recording_frame(&context, &mut app, vec![]);
        assert_eq!(app.hd_library.file(index).unwrap().as_bytes(), original);

        let mut expected = original.clone();
        expected[block..block + 4].copy_from_slice(&target_model.to_be_bytes());
        for (row, key) in default_keys.iter().copied().enumerate() {
            let row_at = block + 0x10 + row * 0x14;
            expected[row_at..row_at + 4].copy_from_slice(&key.to_be_bytes());
            expected[row_at + 4..row_at + 8].copy_from_slice(&0u32.to_be_bytes());
        }
        expected[block + 0x0f] = default_keys.len() as u8;
        for row in 0..12 {
            expected[block + 0x10 + row * 0x14 + 0x10] = 0;
        }

        click_hd_editor(&context, &mut app, button_id);
        assert!(
            app.pod_message.contains(message_label),
            "button={button_id:?}, targets={targets:?}, message={:?}, button_rect={:?}, scroll={:?}",
            app.pod_message,
            app.pod_buttons.get(button_id),
            app.pod_scroll_area
        );
        assert_eq!(app.hd_library.file(index).unwrap().as_bytes(), expected);
        assert!(
            app.hd_library.is_dirty(index),
            "targets={targets:?}, source_matches_expected={}, button_rect={:?}",
            original == expected,
            app.pod_buttons.get(button_id)
        );
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), source_document);
        app.hd_library.revert_unsaved(index).unwrap();
        assert_eq!(app.hd_library.file(index).unwrap().as_bytes(), original);
        assert!(!app.hd_library.is_dirty(index));

        click_hd_editor(&context, &mut app, button_id);
        assert!(
            app.hd_library.is_dirty(index),
            "second apply left the file clean: message={:?}, button_rect={:?}, scroll={:?}",
            app.pod_message,
            app.pod_buttons.get(button_id),
            app.pod_scroll_area
        );
        let directory = tempfile::tempdir().unwrap();
        let blocked = directory.path().join("blocked-directory");
        std::fs::create_dir(&blocked).unwrap();
        let collision = directory.path().join("existing.5xe");
        let output = directory.path().join(output_name);
        std::fs::write(&collision, b"keep destination").unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_save: [
                None,
                Some(blocked.clone()),
                Some(collision.clone()),
                Some(output.clone()),
            ]
            .into(),
            ..Default::default()
        });
        assert!(!app.write_hd_copy(index)); // Save dialog cancellation.
        assert!(app.hd_library.is_dirty(index));
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pending.is_none());
        assert!(app.hd_library.is_dirty(index));

        app.request_close(&context);
        click_prompt(&context, &mut app, "save"); // A directory cannot be the destination.
        assert!(app.pending.is_some());
        assert!(app.hd_library.is_dirty(index));
        click_prompt(&context, &mut app, "save"); // Existing destination is preserved.
        assert!(app.pending.is_some());
        assert!(app.hd_library.is_dirty(index));
        assert_eq!(std::fs::read(&collision).unwrap(), b"keep destination");
        click_prompt(&context, &mut app, "save");
        assert!(app.permit_close);
        assert!(!app.hd_library.any_dirty());
        let written = std::fs::read(&output).unwrap();
        assert_eq!(written, expected);
        let reopened = pod_preset::OpaqueH5epPreset::parse(written).unwrap();
        assert_eq!(reopened.effect_model_id(0), Some(target_model));
        assert!(reopened.trailing_bytes().is_empty());
        assert!(app.read_worker.is_none());
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
    }

    #[test]
    fn hd_effect_enable_ui_edits_one_flag_and_guards_copy_save_and_quit() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let mut original = sample_hd_editor_preset();
        let block = pod_preset::H5EP_HEADER_BYTES + 0x428;
        original[block + 0x0b] = 1;
        original[block + 8] = 0;
        let index = app.hd_library.import_bytes(original.clone()).unwrap();
        app.select_hd_file(index);
        app.pod_open = true;
        hd_editor_frame(&context, &mut app, vec![]);
        assert!(app.pod_buttons.contains_key("enable-hd-effect"));

        click_hd_editor(&context, &mut app, "enable-hd-effect");
        let mut expected = original.clone();
        expected[block + 8] = 1;
        let actual = app.hd_library.file(index).unwrap().as_bytes();
        let differences = actual
            .iter()
            .zip(&expected)
            .enumerate()
            .filter_map(|(offset, (actual, expected))| (actual != expected).then_some(offset))
            .collect::<Vec<_>>();
        assert!(
            differences.is_empty(),
            "unexpected byte offsets: {differences:?}; actual={}, expected={}, button={:?}; message={}",
            actual[block + 8],
            expected[block + 8],
            app.pod_buttons.get("enable-hd-effect"),
            app.pod_message
        );
        assert!(app.hd_library.is_dirty(index));
        assert!(app.pod_message.contains("Nothing was sent"));

        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pending.is_none());
        assert!(app.hd_library.is_dirty(index));

        let directory = tempfile::tempdir().unwrap();
        let blocked = directory.path().join("blocked-directory");
        std::fs::create_dir(&blocked).unwrap();
        let collision = directory.path().join("existing.5xe");
        let output = directory.path().join("enabled-copy.5xe");
        std::fs::write(&collision, b"keep destination").unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_save: [
                Some(blocked.clone()),
                Some(collision.clone()),
                Some(output.clone()),
            ]
            .into(),
            ..Default::default()
        });

        app.request_close(&context);
        click_prompt(&context, &mut app, "save");
        assert!(app.pending.is_some());
        assert!(app.hd_library.is_dirty(index));
        click_prompt(&context, &mut app, "save");
        assert!(app.pending.is_some());
        assert!(app.hd_library.is_dirty(index));
        assert_eq!(std::fs::read(&collision).unwrap(), b"keep destination");
        click_prompt(&context, &mut app, "save");
        assert!(app.permit_close);
        assert!(!app.hd_library.any_dirty());
        let saved = std::fs::read(&output).unwrap();
        assert_eq!(saved, expected);
        let reopened = pod_preset::OpaqueH5epPreset::parse(saved).unwrap();
        assert_eq!(
            reopened.effect_enabled(pod_preset::HdFxSlot::new(0).unwrap()),
            Some(true)
        );
        assert!(app.read_worker.is_none());
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
    }

    #[test]
    fn hd_editor_explicit_default_replacements_stages_reverts_saves_and_guards_quit() {
        exercise_hd_editor_explicit_replacement_lifecycle(
            0x020c_000e,
            &[
                0x3f10_0002,
                0x3f10_0003,
                0x3f10_0004,
                0x3f10_0005,
                0x3f10_0001,
            ],
            "replace-octave-fuzz-vintage-pre",
            "Vintage Pre defaults",
            "vintage-pre-copy.5xe",
            HdExplicitReplacementTargets::default(),
        );
        exercise_hd_editor_explicit_replacement_lifecycle(
            0x0205_000e,
            &[
                0x3f10_0001,
                0x3f10_0002,
                0x3f10_0003,
                0x3f10_0004,
                0x3f10_0005,
            ],
            "replace-octave-fuzz-screamer",
            "Screamer defaults",
            "screamer-copy.5xe",
            HdExplicitReplacementTargets::default(),
        );
        exercise_hd_editor_explicit_replacement_lifecycle(
            0x0205_0014,
            &[
                0x3f10_0001,
                0x3f10_0002,
                0x3f10_0003,
                0x3f10_0004,
                0x3f10_0005,
            ],
            "replace-octave-fuzz-sub-octave-fuzz",
            "Sub Octave Fuzz defaults",
            "sub-octave-fuzz-copy.5xe",
            HdExplicitReplacementTargets::default(),
        );
        exercise_hd_editor_explicit_replacement_lifecycle(
            0x0200_0011,
            &[0x3f10_0001, 0x3f10_0002],
            "replace-octave-fuzz-noise-gate",
            "Noise Gate defaults",
            "noise-gate-copy.5xe",
            HdExplicitReplacementTargets::default(),
        );

        const BATCH: [(pod_preset::HdDistortionDefault, u32, &str, &str); 12] = [
            (
                pod_preset::HdDistortionDefault::JetFuzz,
                0x0205_000a,
                "Jet Fuzz defaults",
                "jet-fuzz-copy.5xe",
            ),
            (
                pod_preset::HdDistortionDefault::BuzzSaw,
                0x0205_0015,
                "Buzz Saw defaults",
                "buzz-saw-copy.5xe",
            ),
            (
                pod_preset::HdDistortionDefault::ClassicDistortion,
                0x0205_000b,
                "Classic Distortion defaults",
                "classic-distortion-copy.5xe",
            ),
            (
                pod_preset::HdDistortionDefault::ColorDrive,
                0x0205_0018,
                "Color Drive defaults",
                "color-drive-copy.5xe",
            ),
            (
                pod_preset::HdDistortionDefault::FacialFuzz,
                0x0205_0011,
                "Facial Fuzz defaults",
                "facial-fuzz-copy.5xe",
            ),
            (
                pod_preset::HdDistortionDefault::FuzzPi,
                0x0205_000f,
                "Fuzz Pi defaults",
                "fuzz-pi-copy.5xe",
            ),
            (
                pod_preset::HdDistortionDefault::HeavyDistortion,
                0x0205_0016,
                "Heavy Distortion defaults",
                "heavy-distortion-copy.5xe",
            ),
            (
                pod_preset::HdDistortionDefault::JumboFuzz,
                0x0205_0017,
                "Jumbo Fuzz defaults",
                "jumbo-fuzz-copy.5xe",
            ),
            (
                pod_preset::HdDistortionDefault::Line6Distortion,
                0x0205_0012,
                "Line 6 Distortion defaults",
                "line6-distortion-copy.5xe",
            ),
            (
                pod_preset::HdDistortionDefault::Line6Drive,
                0x0205_0013,
                "Line 6 Drive defaults",
                "line6-drive-copy.5xe",
            ),
            (
                pod_preset::HdDistortionDefault::Overdrive,
                0x0205_0010,
                "Overdrive defaults",
                "overdrive-copy.5xe",
            ),
            (
                pod_preset::HdDistortionDefault::TubeDrive,
                0x0205_000d,
                "Tube Drive defaults",
                "tube-drive-copy.5xe",
            ),
        ];
        for (target, model_id, label, output_name) in BATCH {
            exercise_hd_editor_explicit_replacement_lifecycle(
                model_id,
                &[
                    0x3f10_0001,
                    0x3f10_0002,
                    0x3f10_0003,
                    0x3f10_0004,
                    0x3f10_0005,
                ],
                "replace-octave-fuzz-admitted-default",
                label,
                output_name,
                HdExplicitReplacementTargets {
                    distortion: Some(target),
                    ..Default::default()
                },
            );
        }

        const DYNAMICS: [(pod_preset::HdDynamicsDefault, u32, &[u32], &str, &str); 8] = [
            (
                pod_preset::HdDynamicsDefault::HardGate,
                0x0200_0013,
                &[0x3f10_0000, 0x3f10_0001, 0x3f10_0002, 0x3f10_0003],
                "Hard Gate defaults",
                "hard-gate-copy.5xe",
            ),
            (
                pod_preset::HdDynamicsDefault::BoostComp,
                0x0200_0012,
                &[
                    0x3f10_0001,
                    0x3f10_0002,
                    0x3f10_0003,
                    0x3f10_0004,
                    0x3f10_0005,
                ],
                "Boost Comp defaults",
                "boost-comp-copy.5xe",
            ),
            (
                pod_preset::HdDynamicsDefault::RedComp,
                0x0200_000b,
                &[0x3f10_0001, 0x3f10_0002],
                "Red Comp defaults",
                "red-comp-copy.5xe",
            ),
            (
                pod_preset::HdDynamicsDefault::BlueComp,
                0x0200_000c,
                &[0x3f10_0001, 0x3f10_0002],
                "Blue Comp defaults",
                "blue-comp-copy.5xe",
            ),
            (
                pod_preset::HdDynamicsDefault::BlueCompTreb,
                0x0200_000d,
                &[0x3f10_0002, 0x3f10_0001],
                "Blue Comp Treb defaults",
                "blue-comp-treb-copy.5xe",
            ),
            (
                pod_preset::HdDynamicsDefault::VettaComp,
                0x0200_000f,
                &[0x3f10_0001, 0x3f10_0002],
                "Vetta Comp defaults",
                "vetta-comp-copy.5xe",
            ),
            (
                pod_preset::HdDynamicsDefault::VettaJuice,
                0x0200_0010,
                &[0x3f10_0001, 0x3f10_0002],
                "Vetta Juice defaults",
                "vetta-juice-copy.5xe",
            ),
            (
                pod_preset::HdDynamicsDefault::TubeComp,
                0x0200_000e,
                &[0x3f10_0001, 0x3f10_0002],
                "Tube Comp defaults",
                "tube-comp-copy.5xe",
            ),
        ];
        for (target, model_id, keys, label, output_name) in DYNAMICS {
            exercise_hd_editor_explicit_replacement_lifecycle(
                model_id,
                keys,
                "replace-octave-fuzz-admitted-dynamics-default",
                label,
                output_name,
                HdExplicitReplacementTargets {
                    dynamics: Some(target),
                    ..Default::default()
                },
            );
        }

        const REVERBS: [(pod_preset::HdReverbDefault, u32, &str, &str); 8] = [
            (
                pod_preset::HdReverbDefault::Octo,
                0x0204_0023,
                "Octo defaults",
                "octo-copy.5xe",
            ),
            (
                pod_preset::HdReverbDefault::Ducking,
                0x0204_0022,
                "Ducking defaults",
                "ducking-copy.5xe",
            ),
            (
                pod_preset::HdReverbDefault::Plate,
                0x0204_001e,
                "Plate defaults",
                "plate-copy.5xe",
            ),
            (
                pod_preset::HdReverbDefault::Room,
                0x0204_001f,
                "Room defaults",
                "room-copy.5xe",
            ),
            (
                pod_preset::HdReverbDefault::Chamber,
                0x0204_0020,
                "Chamber defaults",
                "chamber-copy.5xe",
            ),
            (
                pod_preset::HdReverbDefault::Hall,
                0x0204_0021,
                "Hall defaults",
                "hall-copy.5xe",
            ),
            (
                pod_preset::HdReverbDefault::Cave,
                0x0204_0024,
                "Cave defaults",
                "cave-copy.5xe",
            ),
            (
                pod_preset::HdReverbDefault::Tile,
                0x0204_0025,
                "Tile defaults",
                "tile-copy.5xe",
            ),
        ];
        for (target, model_id, label, output_name) in REVERBS {
            exercise_hd_editor_explicit_replacement_lifecycle(
                model_id,
                &[0x3f10_0001, 0x3f10_0002, 0x3f10_0003, 0x3f10_0005],
                "replace-octave-fuzz-admitted-reverb-default",
                label,
                output_name,
                HdExplicitReplacementTargets {
                    reverb: Some(target),
                    ..Default::default()
                },
            );
        }
    }

    #[test]
    fn hd_editor_numeric_control_updates_only_its_traced_value_offline() {
        let (_context, mut app) = app();
        let source_document = app.document.as_ref().unwrap().as_bytes().to_vec();
        let mut bytes = sample_hd_editor_preset();
        let body = pod_preset::H5EP_HEADER_BYTES;
        let block = body + 0x428;
        bytes[block..block + 4].copy_from_slice(&0x020c_0009u32.to_be_bytes());
        let controls = pod_preset::HD_NUMERIC_CONTROLS
            .into_iter()
            .filter(|control| control.model_id() == 0x020c_0009);
        for (index, control) in controls.enumerate() {
            let row = block + 0x10 + index * 0x14;
            bytes[row..row + 4].copy_from_slice(
                &match index {
                    0 => 0x3f10_0001u32,
                    1 => 0x3f10_0002u32,
                    2 => 0x3f10_0003u32,
                    3 => 0x3f10_0004u32,
                    _ => 0x3f10_0005u32,
                }
                .to_be_bytes(),
            );
            bytes[row + 4..row + 8].copy_from_slice(&0.5f32.to_bits().to_be_bytes());
            bytes[row + 0x10] = 0;
            assert_eq!(control.max(), 240);
        }
        let original = bytes.clone();
        let index = app.hd_library.import_bytes(bytes).unwrap();
        app.select_hd_file(index);
        let control = app.hd_numeric_control;
        assert_eq!(control.model_label(), "Graphic EQ");
        assert_eq!(control.label(), "80Hz");
        assert_eq!(app.hd_numeric_value, 120);
        app.hd_numeric_value = 180;
        app.apply_hd_numeric_control(index);
        assert!(app.hd_library.is_dirty(index));
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), source_document);
        let edited = app.hd_library.file(index).unwrap().as_bytes();
        let value_at = block + 0x10 + 4;
        assert_eq!(&edited[..value_at], &original[..value_at]);
        assert_eq!(
            &edited[value_at..value_at + 4],
            &0.75f32.to_bits().to_be_bytes()
        );
        assert_eq!(&edited[value_at + 4..], &original[value_at + 4..]);
        assert!(app.read_worker.is_none());
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
    }

    #[test]
    fn hd_editor_vintage_pre_type1b_value_patches_through_gui_save_and_quit_offline() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let source_document = app.document.as_ref().unwrap().as_bytes().to_vec();
        let control = pod_preset::HD_NUMERIC_CONTROLS
            .into_iter()
            .find(|control| control.raw_type() == 0x1b)
            .unwrap();
        let mut original = sample_hd_editor_preset();
        let block = pod_preset::H5EP_HEADER_BYTES + 0x428;
        original[block..block + 4].copy_from_slice(&control.model_id().to_be_bytes());
        let row = block + 0x10;
        original[row..row + 4].copy_from_slice(&0x3f10_0005u32.to_be_bytes());
        original[row + 4..row + 8].copy_from_slice(&0x3f00_0000u32.to_be_bytes());
        original[row + 0x10] = 0;
        let index = app.hd_library.import_bytes(original.clone()).unwrap();
        app.select_hd_file(index);

        assert_eq!(app.hd_numeric_control, control);
        assert_eq!(app.hd_numeric_control.raw_type(), 0x1b);
        assert_eq!(
            (app.hd_numeric_control.min(), app.hd_numeric_control.max()),
            (0, 15_000)
        );
        assert_eq!(app.hd_numeric_value, 7_500);

        // The existing library revert restores the exact loaded baseline.
        app.hd_numeric_value = 12_000;
        app.apply_hd_numeric_control(index);
        assert!(app.hd_library.is_dirty(index));
        app.hd_library.revert_unsaved(index).unwrap();
        assert!(!app.hd_library.is_dirty(index));
        assert_eq!(app.hd_library.file(index).unwrap().as_bytes(), original);
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), source_document);

        app.hd_numeric_value = 14_999;
        app.apply_hd_numeric_control(index);
        assert!(app.hd_library.is_dirty(index));
        let mut expected = original.clone();
        expected[row + 4..row + 8].copy_from_slice(&0x3f7f_fba2u32.to_be_bytes());
        assert_eq!(app.hd_library.file(index).unwrap().as_bytes(), expected);

        let directory = tempfile::tempdir().unwrap();
        let collision = directory.path().join("existing.h5p");
        let output = directory.path().join("vintage-pre-copy.h5p");
        std::fs::write(&collision, b"keep existing destination").unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_save: [Some(collision.clone()), Some(output.clone())].into(),
            ..Default::default()
        });
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "save");
        assert!(app.hd_library.is_dirty(index));
        assert!(!app.permit_close);
        assert_eq!(
            std::fs::read(&collision).unwrap(),
            b"keep existing destination"
        );
        click_prompt(&context, &mut app, "save");
        assert!(app.permit_close);
        assert!(!app.hd_library.any_dirty());
        assert!(app.read_worker.is_none());
        assert_eq!(std::fs::read(&output).unwrap(), expected);
        let reopened = pod_preset::OpaqueH5epPreset::parse(std::fs::read(output).unwrap())
            .expect("reopen saved type-0x1B copy");
        assert_eq!(reopened.hd_numeric_control_value(0, control), Some(14_999));
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
    }

    #[test]
    fn hd_editor_mapped_type1_control_updates_its_traced_value_offline() {
        let (_context, mut app) = app();
        let control = pod_preset::HD_TYPE1_MAPPED_CONTROLS[0];
        let mut bytes = sample_hd_editor_preset();
        let body = pod_preset::H5EP_HEADER_BYTES;
        let block = body + 0x428;
        bytes[block..block + 4].copy_from_slice(&control.model_id().to_be_bytes());
        let row = block + 0x10;
        bytes[row..row + 4].copy_from_slice(&control.key().to_be_bytes());
        bytes[row + 4..row + 8].copy_from_slice(&0.5f32.to_bits().to_be_bytes());
        bytes[row + 0x10] = 0;
        let original = bytes.clone();
        let index = app.hd_library.import_bytes(bytes).unwrap();
        app.select_hd_file(index);
        assert_eq!(app.hd_mapped_control, control);
        assert_eq!(app.hd_control_percent, 50);

        app.hd_control_percent = 75;
        app.apply_hd_mapped_control(index);
        assert_eq!(
            app.hd_library.file(index).and_then(|file| match file {
                PodPresetFile::H5ep(preset) => {
                    preset.hd_type1_mapped_control_percent(0, control)
                }
                _ => None,
            }),
            Some(75)
        );
        let mut expected = original;
        expected[row + 4..row + 8].copy_from_slice(&0.75f32.to_bits().to_be_bytes());
        assert_eq!(app.hd_library.file(index).unwrap().as_bytes(), expected);
        assert!(app.hd_library.is_dirty(index));
        assert!(app.read_worker.is_none());
    }

    #[test]
    fn hd_type9_choice_stages_per_entry_slot_then_applies_or_discards_explicitly() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let slot_index = 5;
        let row_index = 11;
        let original = sample_hd_type9_preset(slot_index, row_index, 0x3f40_0000);
        let index = app.hd_library.import_bytes(original.clone()).unwrap();
        app.select_hd_file(index);
        app.pod_open = true;
        app.hd_effect_slot = slot_index;
        let slot = pod_preset::HdFxSlot::new(slot_index).unwrap();

        assert!(matches!(
            app.hd_library.file(index),
            Some(PodPresetFile::H5ep(preset))
                if preset.hd_type9_choice_value(slot)
                    == pod_preset::HdType9ChoiceValue::UnknownFinite(0x3f40_0000)
        ));
        click_hd_type9_choice(&context, &mut app, 2);
        hd_editor_frame(&context, &mut app, vec![]);
        assert_hd_bytes_eq(
            app.hd_library.file(index).unwrap().as_bytes(),
            &original,
            "staging must not alter file bytes",
        );
        assert_eq!(
            app.hd_type9_pending,
            Some(PendingHdType9Choice {
                file_index: index,
                slot,
                model_id: pod_preset::HD_TYPE9_CHOICE_MODEL_ID,
                choice: pod_preset::HdType9Choice::Choice2,
            })
        );
        click_hd_editor(&context, &mut app, "apply-hd-type9-choice");
        let value_at = pod_preset::H5EP_HEADER_BYTES
            + 0x428
            + slot_index * 0x100
            + 0x10
            + row_index * 0x14
            + 4;
        let mut expected = original.clone();
        expected[value_at..value_at + 4].copy_from_slice(&[0x3f, 0x80, 0x00, 0x00]);
        assert_hd_bytes_eq(
            app.hd_library.file(index).unwrap().as_bytes(),
            &expected,
            "applying the staged GUI choice",
        );
        assert!(app.hd_library.is_dirty(index));
        assert!(app.hd_type9_pending.is_none());
        assert!(app.pod_message.contains("Nothing was sent"));

        let directory = tempfile::tempdir().unwrap();
        let saved_copy = directory.path().join("type9-copy.5xe");
        app.dialogs = Box::new(FakeDialogs {
            pod_save: [Some(saved_copy.clone())].into(),
            pod_open: [Some(saved_copy.clone())].into(),
            ..Default::default()
        });
        click_hd_editor(&context, &mut app, "save-hd-copy");
        assert_eq!(std::fs::read(&saved_copy).unwrap(), expected);
        assert!(!app.hd_library.is_dirty(index));
        click_hd_editor(&context, &mut app, "load-hd-file");
        let reopened_index = app.hd_selected.expect("GUI open selects the saved copy");
        assert_ne!(reopened_index, index);
        assert_hd_bytes_eq(
            app.hd_library.file(reopened_index).unwrap().as_bytes(),
            &expected,
            "GUI save and reopen",
        );
        app.select_hd_file(index);

        click_hd_type9_choice(&context, &mut app, 0);
        hd_editor_frame(&context, &mut app, vec![]);
        click_hd_editor(&context, &mut app, "discard-hd-type9-choice");
        assert_hd_bytes_eq(
            app.hd_library.file(index).unwrap().as_bytes(),
            &expected,
            "discarding a later staged GUI choice",
        );
        assert!(app.hd_type9_pending.is_none());

        let other_index = app
            .hd_library
            .import_bytes(sample_hd_editor_preset())
            .unwrap();
        app.stage_hd_type9_choice(
            index,
            slot,
            pod_preset::HD_TYPE9_CHOICE_MODEL_ID,
            pod_preset::HdType9Choice::Choice1,
        );
        app.select_hd_file(other_index);
        assert!(app.hd_type9_pending.is_none());
        app.select_hd_file(index);
        app.hd_effect_slot = slot_index;
        app.stage_hd_type9_choice(
            index,
            slot,
            pod_preset::HD_TYPE9_CHOICE_MODEL_ID,
            pod_preset::HdType9Choice::Choice1,
        );
        app.hd_effect_slot = 4;
        hd_editor_frame(&context, &mut app, vec![]);
        assert!(app.hd_type9_pending.is_none());
        app.hd_effect_slot = slot_index;

        click_hd_type9_choice(&context, &mut app, 0);
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pending.is_none());
        assert!(app.hd_type9_pending.is_some());
        assert!(!app.permit_close);

        app.request_close(&context);
        click_prompt(&context, &mut app, "apply-type9");
        assert!(app.hd_type9_pending.is_none());
        assert!(app.hd_library.is_dirty(index));
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pending.is_none());
        assert!(app.hd_library.is_dirty(index));
        assert!(!app.permit_close);

        click_hd_type9_choice(&context, &mut app, 1);
        app.request_close(&context);
        click_prompt(&context, &mut app, "discard");
        assert!(app.hd_type9_pending.is_none());
        assert!(app.hd_library.is_dirty(index));
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "discard");
        assert!(!app.hd_library.is_dirty(index));
        assert_hd_bytes_eq(
            app.hd_library.file(index).unwrap().as_bytes(),
            &expected,
            "discarding edited type-9 value restores saved baseline",
        );
        assert!(app.permit_close);
        assert!(app.read_worker.is_none());
    }

    #[test]
    fn hd_type9_catalog_additions_use_the_existing_gui_staging_flow() {
        let associations = [
            (0x020a_0010, 0x3f10_0004, 1),
            (0x020a_000f, 0x3f10_0004, 3),
            (0x020a_0012, 0x3f10_0003, 7),
        ];
        for (model_id, key, slot_index) in associations {
            let (context, mut app) = app();
            context.style_mut(|style| style.animation_time = 0.0);
            let row_index = 5;
            let original =
                sample_hd_type9_preset_for(slot_index, row_index, 0x3f40_0000, model_id, key);
            let index = app.hd_library.import_bytes(original.clone()).unwrap();
            app.select_hd_file(index);
            app.pod_open = true;
            app.hd_effect_slot = slot_index;

            click_hd_type9_choice(&context, &mut app, 2);
            hd_editor_frame(&context, &mut app, vec![]);
            assert_hd_bytes_eq(
                app.hd_library.file(index).unwrap().as_bytes(),
                &original,
                "new catalog selection stages without editing bytes",
            );
            assert_eq!(
                app.hd_type9_pending,
                Some(PendingHdType9Choice {
                    file_index: index,
                    slot: pod_preset::HdFxSlot::new(slot_index).unwrap(),
                    model_id,
                    choice: pod_preset::HdType9Choice::Choice2,
                })
            );

            app.hd_effect_slot = (slot_index + 1) % 8;
            hd_editor_frame(&context, &mut app, vec![]);
            assert!(app.hd_type9_pending.is_none());
            app.hd_effect_slot = slot_index;

            click_hd_type9_choice(&context, &mut app, 1);
            hd_editor_frame(&context, &mut app, vec![]);
            click_hd_editor(&context, &mut app, "apply-hd-type9-choice");
            let value_at = pod_preset::H5EP_HEADER_BYTES
                + 0x428
                + slot_index * 0x100
                + 0x10
                + row_index * 0x14
                + 4;
            let mut expected = original;
            expected[value_at..value_at + 4].copy_from_slice(&0x3f00_0000u32.to_be_bytes());
            assert_hd_bytes_eq(
                app.hd_library.file(index).unwrap().as_bytes(),
                &expected,
                "new catalog GUI apply changes only the selected value word",
            );
            assert!(app.hd_library.is_dirty(index));

            click_hd_type9_choice(&context, &mut app, 0);
            hd_editor_frame(&context, &mut app, vec![]);
            click_hd_editor(&context, &mut app, "discard-hd-type9-choice");
            assert_hd_bytes_eq(
                app.hd_library.file(index).unwrap().as_bytes(),
                &expected,
                "discarding a staged new catalog choice preserves applied bytes",
            );
            assert!(app.hd_type9_pending.is_none());
        }
    }

    #[test]
    fn hd_type9_gui_does_not_stage_nonfinite_or_unavailable_values() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        for (index, bits, invalid_marker) in [(0, 0x7fc0_0000, false), (1, 0x3f40_0000, true)] {
            let mut bytes = sample_hd_type9_preset(2, 3, bits);
            if invalid_marker {
                let row = pod_preset::H5EP_HEADER_BYTES + 0x428 + 2 * 0x100 + 0x10 + 3 * 0x14;
                bytes[row + 0x10] = 1;
            }
            let file_index = app.hd_library.import_bytes(bytes.clone()).unwrap();
            assert_eq!(file_index, index);
            app.select_hd_file(file_index);
            app.hd_effect_slot = 2;
            hd_editor_frame(&context, &mut app, vec![]);
            click_hd_type9_choice(&context, &mut app, 1);
            assert!(app.hd_type9_pending.is_none());
            assert_hd_bytes_eq(
                app.hd_library.file(file_index).unwrap().as_bytes(),
                &bytes,
                "refused non-finite or unavailable GUI value",
            );
        }

        let mut malformed = sample_hd_type9_preset(2, 3, 0x3f40_0000);
        malformed[7] = 2;
        assert!(app.hd_library.import_bytes(malformed).is_err());
    }

    #[test]
    fn hd_type9_idle_redraw_preserves_the_users_library_scroll_position() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let bytes = sample_hd_type9_preset(0, 3, 0x3f40_0000);
        let index = app.hd_library.import_bytes(bytes).unwrap();
        app.select_hd_file(index);
        app.pod_open = true;
        hd_editor_frame(&context, &mut app, vec![]);

        let (scroll_id, viewport, content_height) =
            app.pod_scroll_area.expect("rendered HD editor scroll area");
        let max_offset = (content_height - viewport.height()).max(0.0);
        assert!(
            max_offset > 0.0,
            "fixture must require vertical scrolling; content={content_height} viewport={viewport:?}"
        );
        let mut state = egui::scroll_area::State::load(&context, scroll_id).unwrap();
        state.offset.y = max_offset;
        state.store(&context, scroll_id);

        hd_editor_frame(&context, &mut app, vec![]);
        let (scroll_id, viewport, _) = app.pod_scroll_area.unwrap();
        let after_idle = egui::scroll_area::State::load(&context, scroll_id).unwrap();
        assert!((after_idle.offset.y - max_offset).abs() < 1.0);
        assert!(
            !viewport.intersects(app.pod_buttons["hd-type9-choice-combo"]),
            "the scrolled-away type-9 control must stay offscreen after redraw"
        );

        hd_editor_frame(&context, &mut app, vec![]);
        let after_second_idle = egui::scroll_area::State::load(&context, scroll_id).unwrap();
        assert!((after_second_idle.offset.y - after_idle.offset.y).abs() < 1.0);
    }

    #[test]
    fn hd_type8_choice_uses_distinct_two_option_ui_and_clears_cross_type_staging() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let slot_index = 5;
        let row_index = 11;
        let type8_original = sample_hd_type8_preset(slot_index, row_index, 0x3f40_0000);
        let type8 = app.hd_library.import_bytes(type8_original.clone()).unwrap();
        let type9 = app
            .hd_library
            .import_bytes(sample_hd_type9_preset(2, 3, 0x3f40_0000))
            .unwrap();
        app.select_hd_file(type8);
        app.pod_open = true;
        app.hd_effect_slot = slot_index;

        click_hd_type8_choice(&context, &mut app, 1);
        hd_editor_frame(&context, &mut app, vec![]);
        assert_hd_bytes_eq(
            app.hd_library.file(type8).unwrap().as_bytes(),
            &type8_original,
            "type-8 staging leaves source bytes untouched",
        );
        assert_eq!(
            app.hd_type8_pending,
            Some(PendingHdType8Choice {
                file_index: type8,
                slot: pod_preset::HdFxSlot::new(slot_index).unwrap(),
                model_id: pod_preset::HD_TYPE8_CHOICE_MODEL_ID,
                choice: pod_preset::HdType8Choice::Choice1,
            })
        );
        assert!(app.hd_type9_pending.is_none());
        click_hd_editor(&context, &mut app, "apply-hd-type8-choice");
        let value_at = pod_preset::H5EP_HEADER_BYTES
            + 0x428
            + slot_index * 0x100
            + 0x10
            + row_index * 0x14
            + 4;
        let mut expected = type8_original.clone();
        expected[value_at..value_at + 4].copy_from_slice(&[0x3f, 0x80, 0x00, 0x00]);
        assert_hd_bytes_eq(
            app.hd_library.file(type8).unwrap().as_bytes(),
            &expected,
            "type-8 apply patches only value word",
        );
        assert!(app.hd_type8_pending.is_none());
        assert!(app.hd_library.is_dirty(type8));

        let directory = tempfile::tempdir().unwrap();
        let saved_copy = directory.path().join("type8-copy.5xe");
        app.dialogs = Box::new(FakeDialogs {
            pod_save: [Some(saved_copy.clone())].into(),
            pod_open: [Some(saved_copy.clone())].into(),
            ..Default::default()
        });
        click_hd_editor(&context, &mut app, "save-hd-copy");
        assert_eq!(std::fs::read(&saved_copy).unwrap(), expected);
        assert!(!app.hd_library.is_dirty(type8));
        click_hd_editor(&context, &mut app, "load-hd-file");
        let reopened = app
            .hd_selected
            .expect("GUI open selects the saved type-8 copy");
        assert_ne!(reopened, type8);
        assert_hd_bytes_eq(
            app.hd_library.file(reopened).unwrap().as_bytes(),
            &expected,
            "GUI type-8 save and reopen",
        );
        app.select_hd_file(type8);

        click_hd_type8_choice(&context, &mut app, 0);
        hd_editor_frame(&context, &mut app, vec![]);
        click_hd_editor(&context, &mut app, "discard-hd-type8-choice");
        assert!(app.hd_type8_pending.is_none());
        assert_hd_bytes_eq(
            app.hd_library.file(type8).unwrap().as_bytes(),
            &expected,
            "discard clears staged type-8 choice only",
        );

        // Pending edits are tied to exact file, slot and model context.
        app.stage_hd_type8_choice(
            type8,
            pod_preset::HdFxSlot::new(slot_index).unwrap(),
            pod_preset::HD_TYPE8_CHOICE_MODEL_ID,
            pod_preset::HdType8Choice::Choice0,
        );
        app.select_hd_file(type9);
        assert!(app.hd_type8_pending.is_none());
        app.hd_effect_slot = 2;
        click_hd_type9_choice(&context, &mut app, 2);
        hd_editor_frame(&context, &mut app, vec![]);
        assert!(app.hd_type9_pending.is_some());
        app.select_hd_file(type8);
        assert!(app.hd_type9_pending.is_none());
        assert!(app.hd_type8_pending.is_none());

        // Same entry and same file slot can change type context while a choice
        // is staged. A real editor frame must discard each stale stage without
        // applying its previous value.
        app.hd_effect_slot = slot_index;
        click_hd_type8_choice(&context, &mut app, 0);
        hd_editor_frame(&context, &mut app, vec![]);
        assert!(app.hd_type8_pending.is_some());
        let type9_same_context = sample_hd_type9_preset(slot_index, row_index, 0x3f40_0000);
        *app.hd_library.file_mut(type8).unwrap() =
            PodPresetFile::parse(type9_same_context.clone()).unwrap();
        hd_editor_frame(&context, &mut app, vec![]);
        assert!(app.hd_type8_pending.is_none());
        assert!(app.hd_type9_pending.is_none());
        assert_hd_bytes_eq(
            app.hd_library.file(type8).unwrap().as_bytes(),
            &type9_same_context,
            "type-8 pending stage invalidated by same-file type-9 model",
        );

        click_hd_type9_choice(&context, &mut app, 2);
        hd_editor_frame(&context, &mut app, vec![]);
        assert!(app.hd_type9_pending.is_some());
        *app.hd_library.file_mut(type8).unwrap() = PodPresetFile::parse(expected.clone()).unwrap();
        hd_editor_frame(&context, &mut app, vec![]);
        assert!(app.hd_type9_pending.is_none());
        assert!(app.hd_type8_pending.is_none());
        assert_hd_bytes_eq(
            app.hd_library.file(type8).unwrap().as_bytes(),
            &expected,
            "type-9 pending stage invalidated by same-file type-8 model",
        );

        app.hd_effect_slot = slot_index;
        click_hd_type8_choice(&context, &mut app, 0);
        hd_editor_frame(&context, &mut app, vec![]);
        let staged = app
            .hd_type8_pending
            .expect("GUI type-8 combo stages a value");
        let mut bad_marker = expected.clone();
        bad_marker[0xa4c] = 1;
        *app.hd_library.file_mut(type8).unwrap() =
            PodPresetFile::parse(bad_marker.clone()).unwrap();
        assert!(app.hd_library.is_dirty(type8));
        hd_editor_frame(&context, &mut app, vec![]);
        click_hd_editor(&context, &mut app, "apply-hd-type8-choice");
        assert_hd_bytes_eq(
            app.hd_library.file(type8).unwrap().as_bytes(),
            &bad_marker,
            "invalid row marker makes actual Apply preserve current bytes",
        );
        assert!(app.hd_library.is_dirty(type8));
        assert_eq!(app.hd_type8_pending, Some(staged));
        assert!(app.pod_message.contains("marker"));

        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "discard");
        assert!(app.hd_type8_pending.is_none());
        assert!(app.hd_library.is_dirty(type8));
        click_prompt(&context, &mut app, "discard");
        assert_hd_bytes_eq(
            app.hd_library.file(type8).unwrap().as_bytes(),
            &expected,
            "quit discard restores the previously saved baseline",
        );
        assert!(!app.hd_library.is_dirty(type8));

        assert!(app.read_worker.is_none());
    }

    #[test]
    fn hd_type10_and_type9_choices_stage_and_apply_independently_on_same_model() {
        for model_id in [pod_preset::HD_TYPE10_CHOICE_MODEL_ID, 0x020a_000f] {
            hd_type10_and_type9_choices_stage_and_apply_independently_for(model_id);
        }
    }

    fn hd_type10_and_type9_choices_stage_and_apply_independently_for(model_id: u32) {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let slot_index = 5;
        let row_index = 8;
        let key = pod_preset::hd_type10_choice_key(model_id).unwrap();
        let original =
            sample_hd_type10_preset_for(slot_index, row_index, 0x3eaa_aaab, model_id, key);
        let index = app.hd_library.import_bytes(original.clone()).unwrap();
        app.select_hd_file(index);
        app.pod_open = true;
        app.hd_effect_slot = slot_index;
        click_hd_type10_choice(&context, &mut app, 1);
        hd_editor_frame(&context, &mut app, vec![]);
        assert!(app.hd_type10_pending.is_some());
        assert_hd_bytes_eq(
            app.hd_library.file(index).unwrap().as_bytes(),
            &original,
            "type-10 stage is display only",
        );
        click_hd_type9_choice(&context, &mut app, 1);
        hd_editor_frame(&context, &mut app, vec![]);
        assert_eq!(
            app.hd_type9_pending,
            Some(PendingHdType9Choice {
                file_index: index,
                slot: pod_preset::HdFxSlot::new(slot_index).unwrap(),
                model_id,
                choice: pod_preset::HdType9Choice::Choice1,
            })
        );
        assert!(
            app.hd_type10_pending.is_some(),
            "staging same-model type 9 must retain type-10 stage"
        );

        let type10_value = pod_preset::H5EP_HEADER_BYTES
            + 0x428
            + slot_index * 0x100
            + 0x10
            + row_index * 0x14
            + 4;
        let type9_row = (row_index + 1) % 12;
        let type9_value = pod_preset::H5EP_HEADER_BYTES
            + 0x428
            + slot_index * 0x100
            + 0x10
            + type9_row * 0x14
            + 4;
        let staged_type10 = app.hd_type10_pending;
        assert_eq!(
            staged_type10,
            Some(PendingHdType10Choice {
                file_index: index,
                slot: pod_preset::HdFxSlot::new(slot_index).unwrap(),
                model_id,
                choice: pod_preset::HdType10Choice::Choice1,
            })
        );
        click_hd_editor(&context, &mut app, "apply-hd-type9-choice");
        let after9 = app.hd_library.file(index).unwrap().as_bytes().to_vec();
        assert_eq!(&after9[type9_value..type9_value + 4], &[0x3f, 0, 0, 0]);
        assert_eq!(
            &after9[type10_value..type10_value + 4],
            &original[type10_value..type10_value + 4],
            "applying type 9 changes only its value word"
        );
        assert_eq!(
            app.hd_type10_pending, staged_type10,
            "applying type 9 preserves the exact staged type-10 value"
        );

        click_hd_editor(&context, &mut app, "apply-hd-type10-choice");
        let after10 = app.hd_library.file(index).unwrap().as_bytes().to_vec();
        assert_eq!(&after10[type9_value..type9_value + 4], &[0x3f, 0, 0, 0]);
        assert_eq!(
            &after10[type10_value..type10_value + 4],
            &[0x3f, 0x80, 0, 0]
        );
        assert!(app.hd_type10_pending.is_none());
        assert!(app.hd_type9_pending.is_none());

        click_hd_type10_choice(&context, &mut app, 0);
        hd_editor_frame(&context, &mut app, vec![]);
        click_hd_type9_choice(&context, &mut app, 2);
        hd_editor_frame(&context, &mut app, vec![]);
        let staged_type10 = app.hd_type10_pending;
        let staged_type9 = app.hd_type9_pending;
        assert!(staged_type10.is_some() && staged_type9.is_some());
        click_hd_editor(&context, &mut app, "discard-hd-type9-choice");
        assert!(app.hd_type9_pending.is_none());
        assert_eq!(
            app.hd_type10_pending, staged_type10,
            "discard type 9 preserves the exact type-10 stage"
        );
        assert_eq!(app.hd_library.file(index).unwrap().as_bytes(), after10);
        click_hd_editor(&context, &mut app, "discard-hd-type10-choice");
        assert!(app.hd_type10_pending.is_none());
        assert_eq!(app.hd_library.file(index).unwrap().as_bytes(), after10);

        click_hd_type10_choice(&context, &mut app, 0);
        hd_editor_frame(&context, &mut app, vec![]);
        click_hd_type9_choice(&context, &mut app, 2);
        hd_editor_frame(&context, &mut app, vec![]);
        let staged_type9 = app.hd_type9_pending;
        let staged_type10 = app.hd_type10_pending;
        assert!(staged_type10.is_some() && staged_type9.is_some());
        click_hd_editor(&context, &mut app, "discard-hd-type10-choice");
        assert!(app.hd_type10_pending.is_none());
        assert_eq!(
            app.hd_type9_pending, staged_type9,
            "discard type 10 preserves the exact type-9 stage"
        );
        assert_eq!(app.hd_library.file(index).unwrap().as_bytes(), after10);
        click_hd_editor(&context, &mut app, "discard-hd-type9-choice");
        assert!(app.hd_type9_pending.is_none());
        assert_eq!(app.hd_library.file(index).unwrap().as_bytes(), after10);

        let directory = tempfile::tempdir().unwrap();
        let collision = directory.path().join("type10-existing.5xe");
        std::fs::write(&collision, b"keep existing").unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_save: [Some(collision.clone())].into(),
            ..Default::default()
        });
        click_hd_editor(&context, &mut app, "save-hd-copy");
        assert_eq!(std::fs::read(&collision).unwrap(), b"keep existing");
        assert!(app.hd_library.is_dirty(index));
        let copy = directory.path().join("type10-copy.5xe");
        app.dialogs = Box::new(FakeDialogs {
            pod_save: [Some(copy.clone())].into(),
            pod_open: [Some(copy.clone())].into(),
            ..Default::default()
        });
        click_hd_editor(&context, &mut app, "save-hd-copy");
        assert_hd_bytes_eq(
            &std::fs::read(&copy).unwrap(),
            &after10,
            "type-10 copy save",
        );
        assert!(!app.hd_library.is_dirty(index));
        click_hd_editor(&context, &mut app, "load-hd-file");
        let reopened = app.hd_selected.expect("saved type-10 copy reopens");
        assert_hd_bytes_eq(
            app.hd_library.file(reopened).unwrap().as_bytes(),
            &after10,
            "type-10 copy reopen",
        );
        app.select_hd_file(index);
        app.hd_effect_slot = slot_index;

        click_hd_type10_choice(&context, &mut app, 0);
        hd_editor_frame(&context, &mut app, vec![]);
        let staged = app
            .hd_type10_pending
            .expect("second type-10 choice is staged");
        let mut bad_marker = after10.clone();
        let marker = pod_preset::H5EP_HEADER_BYTES
            + 0x428
            + slot_index * 0x100
            + 0x10
            + row_index * 0x14
            + 0x10;
        bad_marker[marker] = 1;
        *app.hd_library.file_mut(index).unwrap() =
            PodPresetFile::parse(bad_marker.clone()).unwrap();
        assert!(app.hd_library.is_dirty(index));
        hd_editor_frame(&context, &mut app, vec![]);
        click_hd_editor(&context, &mut app, "apply-hd-type10-choice");
        assert_hd_bytes_eq(
            app.hd_library.file(index).unwrap().as_bytes(),
            &bad_marker,
            "failed type-10 Apply preserves bytes",
        );
        assert_eq!(app.hd_type10_pending, Some(staged));
        assert!(app.hd_library.is_dirty(index));
        app.hd_library.revert_unsaved(index).unwrap();
        assert_hd_bytes_eq(
            app.hd_library.file(index).unwrap().as_bytes(),
            &after10,
            "type-10 revert returns to saved baseline",
        );
        assert!(
            app.hd_type10_pending.is_some(),
            "failed Apply keeps its intended stage"
        );
        click_hd_type9_choice(&context, &mut app, 0);
        assert!(app.hd_type9_pending.is_some());
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "discard");
        assert!(app.hd_type9_pending.is_none());
        assert!(
            app.hd_type10_pending.is_some(),
            "quit processes the remaining type-10 stage"
        );
        click_prompt(&context, &mut app, "discard");
        assert!(app.hd_type10_pending.is_none());
        assert!(
            app.permit_close,
            "quit closes only after all staged choices are resolved"
        );
    }

    #[test]
    fn hd_type10_pending_is_invalidated_on_same_entry_model_change_and_file_replacement() {
        for model_id in [pod_preset::HD_TYPE10_CHOICE_MODEL_ID, 0x020a_000f] {
            hd_type10_pending_is_invalidated_for(model_id);
        }
    }

    fn hd_type10_pending_is_invalidated_for(model_id: u32) {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let slot_index = 5;
        let key = pod_preset::hd_type10_choice_key(model_id).unwrap();
        let bytes = sample_hd_type10_preset_for(slot_index, 8, 0x3eaa_aaab, model_id, key);
        let index = app.hd_library.import_bytes(bytes.clone()).unwrap();
        app.select_hd_file(index);
        app.pod_open = true;
        app.hd_effect_slot = slot_index;

        click_hd_type10_choice(&context, &mut app, 1);
        hd_editor_frame(&context, &mut app, vec![]);
        app.hd_effect_slot = 4;
        hd_editor_frame(&context, &mut app, vec![]);
        assert!(
            app.hd_type10_pending.is_none(),
            "slot change clears stale stage"
        );

        let another_index = app
            .hd_library
            .import_bytes(sample_hd_type10_preset_for(
                3,
                4,
                0x3eaa_aaab,
                model_id,
                key,
            ))
            .unwrap();
        app.hd_effect_slot = slot_index;
        click_hd_type10_choice(&context, &mut app, 1);
        hd_editor_frame(&context, &mut app, vec![]);
        assert_eq!(
            app.hd_type10_pending,
            Some(PendingHdType10Choice {
                file_index: index,
                slot: pod_preset::HdFxSlot::new(slot_index).unwrap(),
                model_id,
                choice: pod_preset::HdType10Choice::Choice1,
            }),
            "combo establishes the exact stage before selection changes"
        );
        app.select_hd_file(another_index);
        assert!(app.hd_type10_pending.is_none(), "selection clears stage");
        app.select_hd_file(index);
        app.hd_effect_slot = slot_index;
        click_hd_type10_choice(&context, &mut app, 1);
        hd_editor_frame(&context, &mut app, vec![]);
        click_hd_type9_choice(&context, &mut app, 1);
        hd_editor_frame(&context, &mut app, vec![]);
        assert!(app.hd_type10_pending.is_some());
        assert!(app.hd_type9_pending.is_some());

        let model_offset = pod_preset::H5EP_HEADER_BYTES + 0x428 + slot_index * 0x100;
        let mut changed_model = bytes.clone();
        let other_admitted_model = if model_id == pod_preset::HD_TYPE10_CHOICE_MODEL_ID {
            0x020a_000f
        } else {
            pod_preset::HD_TYPE10_CHOICE_MODEL_ID
        };
        changed_model[model_offset..model_offset + 4]
            .copy_from_slice(&other_admitted_model.to_be_bytes());
        *app.hd_library.file_mut(index).unwrap() =
            PodPresetFile::parse(changed_model.clone()).unwrap();
        hd_editor_frame(&context, &mut app, vec![]);
        assert!(app.hd_type10_pending.is_none());
        assert!(app.hd_type9_pending.is_none());
        assert_hd_bytes_eq(
            app.hd_library.file(index).unwrap().as_bytes(),
            &changed_model,
            "same-entry model change clears both stale stages without editing bytes",
        );

        app.hd_library.revert_unsaved(index).unwrap();
        app.select_hd_file(index);
        app.hd_effect_slot = slot_index;
        hd_editor_frame(&context, &mut app, vec![]);
        click_hd_type10_choice(&context, &mut app, 0);
        hd_editor_frame(&context, &mut app, vec![]);
        let staged = app.hd_type10_pending;
        assert_eq!(
            staged,
            Some(PendingHdType10Choice {
                file_index: index,
                slot: pod_preset::HdFxSlot::new(slot_index).unwrap(),
                model_id,
                choice: pod_preset::HdType10Choice::Choice0,
            }),
            "actual combo interaction establishes the stage before Open"
        );

        let directory = tempfile::tempdir().unwrap();
        let replacement_path = directory.path().join("replacement.5xe");
        let replacement_bytes = sample_hd_type10_preset_for(2, 4, 0x3eaa_aaab, model_id, key);
        std::fs::write(&replacement_path, &replacement_bytes).unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_open: [None, Some(replacement_path)].into(),
            ..Default::default()
        });
        click_hd_editor(&context, &mut app, "load-hd-file");
        assert_eq!(
            app.hd_selected,
            Some(index),
            "cancel keeps the selected entry"
        );
        assert_eq!(
            app.hd_type10_pending, staged,
            "cancel keeps the established stage"
        );
        assert!(app.pod_message.contains("Load cancelled"));

        click_hd_editor(&context, &mut app, "load-hd-file");
        let replacement = app
            .hd_selected
            .expect("successful Open selects imported file");
        assert_ne!(replacement, index);
        assert!(
            app.hd_type10_pending.is_none(),
            "successful Open clears stale stage"
        );
        assert_hd_bytes_eq(
            app.hd_library.file(index).unwrap().as_bytes(),
            &bytes,
            "successful Open leaves the previously staged source bytes/baseline intact",
        );
        assert!(!app.hd_library.is_dirty(index));
        assert_hd_bytes_eq(
            app.hd_library.file(replacement).unwrap().as_bytes(),
            &replacement_bytes,
            "successful Open imports the selected offline replacement",
        );
    }

    #[test]
    fn hd_revert_clears_only_pending_type10_for_the_reverted_file() {
        for model_id in [pod_preset::HD_TYPE10_CHOICE_MODEL_ID, 0x020a_000f] {
            hd_revert_clears_type10_pending_for(model_id);
        }
    }

    fn hd_revert_clears_type10_pending_for(model_id: u32) {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let key = pod_preset::hd_type10_choice_key(model_id).unwrap();
        let bytes = sample_hd_type10_preset_for(5, 8, 0x3eaa_aaab, model_id, key);
        let reverted = app.hd_library.import_bytes(bytes.clone()).unwrap();
        let unrelated = app.hd_library.import_bytes(bytes.clone()).unwrap();
        app.select_hd_file(reverted);
        app.pod_open = true;
        app.hd_effect_slot = 5;
        click_hd_type10_choice(&context, &mut app, 1);
        hd_editor_frame(&context, &mut app, vec![]);
        let staged = app.hd_type10_pending;
        assert_eq!(
            staged,
            Some(PendingHdType10Choice {
                file_index: reverted,
                slot: pod_preset::HdFxSlot::new(5).unwrap(),
                model_id,
                choice: pod_preset::HdType10Choice::Choice1,
            }),
            "actual GUI interaction establishes stage before revert"
        );
        let model_offset = pod_preset::H5EP_HEADER_BYTES + 0x428 + 5 * 0x100;
        let mut dirty_bytes = bytes.clone();
        let other_admitted_model = if model_id == pod_preset::HD_TYPE10_CHOICE_MODEL_ID {
            0x020a_000f
        } else {
            pod_preset::HD_TYPE10_CHOICE_MODEL_ID
        };
        dirty_bytes[model_offset..model_offset + 4]
            .copy_from_slice(&other_admitted_model.to_be_bytes());
        *app.hd_library.file_mut(reverted).unwrap() = PodPresetFile::parse(dirty_bytes).unwrap();
        assert!(app.hd_library.is_dirty(reverted));
        let slot = pod_preset::HdFxSlot::new(5).unwrap();
        app.hd_type9_pending = Some(PendingHdType9Choice {
            file_index: unrelated,
            slot,
            model_id,
            choice: pod_preset::HdType9Choice::Choice1,
        });

        assert!(app.discard_next_dirty_pod());
        assert!(app.hd_type10_pending.is_none());
        assert!(
            app.hd_type9_pending
                .is_some_and(|pending| pending.file_index == unrelated)
        );
        assert_eq!(app.hd_library.file(reverted).unwrap().as_bytes(), bytes);
        assert!(!app.hd_library.is_dirty(reverted));
    }

    #[test]
    fn hd_type10_quit_apply_and_keep_editing_resolve_only_the_staged_choice() {
        for model_id in [pod_preset::HD_TYPE10_CHOICE_MODEL_ID, 0x020a_000f] {
            hd_type10_quit_apply_and_keep_editing_for(model_id);
        }
    }

    fn hd_type10_quit_apply_and_keep_editing_for(model_id: u32) {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let slot_index = 5;
        let key = pod_preset::hd_type10_choice_key(model_id).unwrap();
        let bytes = sample_hd_type10_preset_for(slot_index, 8, 0x3eaa_aaab, model_id, key);
        let index = app.hd_library.import_bytes(bytes.clone()).unwrap();
        app.select_hd_file(index);
        app.pod_open = true;
        app.hd_effect_slot = slot_index;
        click_hd_type10_choice(&context, &mut app, 1);
        hd_editor_frame(&context, &mut app, vec![]);

        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "cancel");
        assert!(
            app.pending.is_none(),
            "Keep editing dismisses the quit prompt"
        );
        assert!(
            app.hd_type10_pending.is_some(),
            "Keep editing retains the stage"
        );
        assert!(!app.permit_close);

        app.request_close(&context);
        click_prompt(&context, &mut app, "apply-type10");
        assert!(app.hd_type10_pending.is_none());
        assert!(app.hd_library.is_dirty(index));
        assert!(
            !app.permit_close,
            "applied edit still requires save or discard"
        );
        let value_at =
            pod_preset::H5EP_HEADER_BYTES + 0x428 + slot_index * 0x100 + 0x10 + 8 * 0x14 + 4;
        assert_eq!(
            &app.hd_library.file(index).unwrap().as_bytes()[value_at..value_at + 4],
            &[0x3f, 0x80, 0, 0]
        );
    }

    #[test]
    fn hd_type10_gui_refuses_nonfinite_and_unavailable_rows() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        for model_id in [pod_preset::HD_TYPE10_CHOICE_MODEL_ID, 0x020a_000f] {
            let key = pod_preset::hd_type10_choice_key(model_id).unwrap();
            for (bits, invalid_key) in [(0x7fc0_0001u32, false), (0x3f40_0000, true)] {
                let mut bytes = sample_hd_type10_preset_for(3, 4, bits, model_id, key);
                if invalid_key {
                    let row = pod_preset::H5EP_HEADER_BYTES + 0x428 + 3 * 0x100 + 0x10 + 4 * 0x14;
                    bytes[row..row + 4].fill(0);
                }
                let index = app.hd_library.import_bytes(bytes.clone()).unwrap();
                app.select_hd_file(index);
                app.hd_effect_slot = 3;
                hd_editor_frame(&context, &mut app, vec![]);
                click_hd_type10_choice(&context, &mut app, 1);
                assert!(app.hd_type10_pending.is_none());
                assert_hd_bytes_eq(
                    app.hd_library.file(index).unwrap().as_bytes(),
                    &bytes,
                    "uneditable type-10 value stays untouched",
                );
            }
        }
    }

    #[test]
    fn hd_type8_gui_does_not_stage_nonfinite_or_unavailable_values() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        for (bits, invalid_marker) in [(0x7fc0_0001u32, false), (0x3f40_0000, true)] {
            let mut bytes = sample_hd_type8_preset(3, 4, bits);
            if invalid_marker {
                let marker =
                    pod_preset::H5EP_HEADER_BYTES + 0x428 + 3 * 0x100 + 0x10 + 4 * 0x14 + 0x10;
                bytes[marker] = 1;
            }
            let index = app.hd_library.import_bytes(bytes.clone()).unwrap();
            app.select_hd_file(index);
            app.hd_effect_slot = 3;
            hd_editor_frame(&context, &mut app, vec![]);
            click_hd_type8_choice(&context, &mut app, 1);
            assert!(app.hd_type8_pending.is_none());
            assert!(app.hd_type9_pending.is_none());
            assert_hd_bytes_eq(
                app.hd_library.file(index).unwrap().as_bytes(),
                &bytes,
                "non-finite or unavailable input cannot stage a choice",
            );
        }
        assert!(app.read_worker.is_none());
    }

    #[test]
    fn second_type10_association_gui_stage_apply_and_discard() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let slot = 5;
        let row = 8;
        let original = sample_hd_type10_preset_for(
            slot,
            row,
            0x3eaa_aaab,
            0x020a_000f,
            pod_preset::HD_TYPE10_CHOICE_KEY,
        );
        let index = app.hd_library.import_bytes(original.clone()).unwrap();
        app.select_hd_file(index);
        app.pod_open = true;
        app.hd_effect_slot = slot;
        click_hd_type10_choice(&context, &mut app, 1);
        hd_editor_frame(&context, &mut app, vec![]);
        assert!(app.hd_type10_pending.is_some());
        assert_eq!(app.hd_library.file(index).unwrap().as_bytes(), original);
        click_hd_editor(&context, &mut app, "discard-hd-type10-choice");
        assert!(app.hd_type10_pending.is_none());
        assert_eq!(app.hd_library.file(index).unwrap().as_bytes(), original);

        click_hd_type10_choice(&context, &mut app, 1);
        hd_editor_frame(&context, &mut app, vec![]);
        click_hd_editor(&context, &mut app, "apply-hd-type10-choice");
        let changed = app.hd_library.file(index).unwrap().as_bytes();
        let value_at = pod_preset::H5EP_HEADER_BYTES + 0x428 + slot * 0x100 + 0x10 + row * 0x14 + 4;
        assert_eq!(&changed[value_at..value_at + 4], &[0x3f, 0x80, 0, 0]);
        assert!(app.hd_type10_pending.is_none());
        assert!(app.read_worker.is_none());
    }

    pub(super) fn app() -> (egui::Context, Workbench) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let context = egui::Context::default();
        let mut app = Workbench::with_context(
            &context,
            Some(root.join("tests/fixtures/editor-preset.vwm")),
            root.join("assets/original-ui/modules/variax/editor/images"),
            None,
        );
        // These tests exercise egui directly, without a display or theme polling.
        app.theme_polled = Instant::now();
        (context, app)
    }

    pub(super) fn prompt_frame(
        context: &egui::Context,
        app: &mut Workbench,
        events: Vec<egui::Event>,
    ) {
        let _ = context.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1120., 760.))),
                events,
                ..Default::default()
            },
            |ctx| app.pending_dialog(ctx),
        );
    }

    pub(super) fn click_prompt(context: &egui::Context, app: &mut Workbench, action: &'static str) {
        // Let a newly created or resized centered prompt settle before clicking.
        prompt_frame(context, app, vec![]);
        prompt_frame(context, app, vec![]);
        let pos = app.pending_buttons[action].center();
        prompt_frame(
            context,
            app,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        prompt_frame(
            context,
            app,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
    }

    #[test]
    fn dirty_open_cancel_failed_save_retry_and_discard_follow_actual_prompt_buttons() {
        let dir = tempfile::tempdir().unwrap();
        let next = dir.path().join("next.vwm");
        let saved = dir.path().join("saved.vwm");
        let mut preset =
            PresetFile::parse(include_bytes!("../tests/fixtures/editor-preset.vwm").to_vec())
                .unwrap();
        preset.rename("Next document").unwrap();
        std::fs::write(&next, preset.as_bytes()).unwrap();
        let (context, mut app) = app();
        context.style_mut(|s| s.animation_time = 0.);
        app.dialogs = Box::new(FakeDialogs {
            open: [None, Some(next.clone()), Some(next.clone())].into(),
            save: [None, Some(next.clone()), Some(saved.clone())].into(),
            ..Default::default()
        });
        app.change(|p| p.rename("Keep me"));
        let before = app.document.as_ref().unwrap().as_bytes().to_vec();
        app.open_dialog();
        assert!(app.pending.is_none());
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), before);
        app.open_dialog();
        assert!(matches!(app.pending, Some(Pending::Open(_))));
        app.change(|p| p.rename("Behind prompt"));
        app.undo();
        app.redo();
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), before);
        prompt_frame(&context, &mut app, vec![]);
        click_prompt(&context, &mut app, "save");
        assert!(app.pending.is_some());
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), before);
        click_prompt(&context, &mut app, "save"); // Existing destination is refused.
        assert!(app.pending.is_some());
        assert!(app.error.is_some());
        assert_eq!(std::fs::read(&next).unwrap(), preset.as_bytes());
        click_prompt(&context, &mut app, "save");
        assert!(app.pending.is_none());
        assert_eq!(std::fs::read(saved).unwrap(), before);
        assert_eq!(
            app.document.as_ref().unwrap().preset().name(),
            "Next document"
        );
        assert!(app.error.is_none());
        app.change(|p| p.rename("Discard me"));
        app.open_dialog();
        prompt_frame(&context, &mut app, vec![]);
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pending.is_none());
        assert_eq!(app.document.as_ref().unwrap().preset().name(), "Discard me");
        app.pending = Some(Pending::Open(next));
        prompt_frame(&context, &mut app, vec![]);
        click_prompt(&context, &mut app, "discard");
        assert_eq!(
            app.document.as_ref().unwrap().preset().name(),
            "Next document"
        );
    }

    #[test]
    fn clean_document_save_cancellation_does_not_continue_an_existing_prompt() {
        let (context, mut app) = app();
        context.style_mut(|s| s.animation_time = 0.);
        app.dialogs = Box::new(FakeDialogs {
            save: [None].into(),
            ..Default::default()
        });
        app.pending = Some(Pending::Quit);
        assert!(!app.dirty());
        prompt_frame(&context, &mut app, vec![]);
        click_prompt(&context, &mut app, "save");
        assert!(matches!(app.pending, Some(Pending::Quit)));
        assert!(!app.permit_close);
        click_prompt(&context, &mut app, "discard");
        assert!(app.permit_close);
        assert!(app.pending.is_none());
    }

    #[test]
    fn failed_open_and_missing_save_directory_preserve_dirty_document_and_history() {
        let (context, mut app) = app();
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("bad.vwb");
        std::fs::write(&bad, b"invalid container").unwrap();
        app.dialogs = Box::new(FakeDialogs {
            open: [Some(bad)].into(),
            save: [Some(dir.path().join("missing/copy.vwm"))].into(),
            ..Default::default()
        });
        app.change(|p| p.rename("Retained"));
        let before = app.document.as_ref().unwrap().as_bytes().to_vec();
        assert!(!app.save_dialog());
        assert!(app.error.is_some());
        app.open_dialog();
        app.continue_pending(&context);
        assert!(app.error.is_some());
        assert!(app.dirty());
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), before);
        app.undo();
        assert!(!app.dirty());
        app.redo();
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), before);
    }

    #[test]
    fn close_during_fake_operation_cancels_it_and_retains_document_after_disconnect() {
        let (context, mut app) = app();
        app.change(|p| p.rename("Retained"));
        let before = app.document.as_ref().unwrap().as_bytes().to_vec();
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (sender, receiver) = std::sync::mpsc::channel();
        app.read_receiver = Some(receiver);
        app.read_worker = Some((cancel.clone(), std::thread::spawn(|| {})));
        app.request_close(&context);
        assert!(cancel.load(std::sync::atomic::Ordering::Relaxed));
        assert!(!app.permit_close);
        assert!(app.pending.is_none());
        app.open_dialog();
        assert!(!app.save_dialog()); // Fake adapter panics if invoked.
        drop(sender);
        app.poll_read();
        assert!(!app.is_reading());
        assert!(app.read_worker.is_none());
        assert!(app.error.as_deref().unwrap().contains("unexpectedly"));
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), before);
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::Quit)));
        app.continue_pending(&context);
        assert!(app.permit_close);
    }

    #[test]
    fn actual_pickup_slider_drag_is_one_undo_group() {
        fn frame(context: &egui::Context, app: &mut Workbench, events: Vec<egui::Event>) {
            let _ = context.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        app.float_control(ui, PICKUP1, 0x200000, "Position", " in")
                    });
                },
            );
        }
        let (context, mut app) = app();
        context.style_mut(|style| style.spacing.slider_width = 200.);
        app.load(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/synthetic-host-preset.vwm"),
        );
        let before = app.document.as_ref().unwrap().as_bytes().to_vec();
        frame(&context, &mut app, vec![]);
        let rect = app.pickup_slider.unwrap();
        let start = Pos2::new(rect.left() + 30., rect.center().y);
        let end = start + Vec2::new(90., 0.);
        frame(
            &context,
            &mut app,
            vec![
                egui::Event::PointerMoved(start),
                egui::Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        for pos in [start + Vec2::new(40., 0.), end] {
            frame(&context, &mut app, vec![egui::Event::PointerMoved(pos)]);
        }
        frame(
            &context,
            &mut app,
            vec![egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
        let changed = app.document.as_ref().unwrap().as_bytes().to_vec();
        assert_ne!(changed, before);
        app.undo();
        let doc = app.document.as_ref().unwrap();
        assert_eq!(doc.as_bytes(), before);
        assert!(!doc.can_undo());
        app.redo();
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), changed);
    }

    #[test]
    fn dragging_either_pickup_follows_the_pointer_and_undoes_in_one_step() {
        fn frame(context: &egui::Context, app: &mut Workbench, events: Vec<egui::Event>) {
            let _ = context.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1120., 760.))),
                    events,
                    ..Default::default()
                },
                |ctx| app.render(ctx),
            );
        }
        fn button(pos: Pos2, pressed: bool) -> egui::Event {
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            }
        }

        for (slot, other_slot, direction) in [(PICKUP1, PICKUP2, -1.0), (PICKUP2, PICKUP1, 1.0)] {
            let context = egui::Context::default();
            let artwork = tempfile::tempdir().unwrap();
            let mut app = Workbench::with_context(&context, None, artwork.path().to_owned(), None);
            app.theme_polled = Instant::now();
            let texture = context.load_texture(
                "synthetic pickup",
                egui::ColorImage::filled([26, 92], Color32::WHITE),
                egui::TextureOptions::LINEAR,
            );
            app.textures.insert("pickups/strat.png".into(), texture);
            let before = app.document.as_ref().unwrap().as_bytes().to_vec();
            let initial = app.float(slot, 0x200000).unwrap();
            let other = app.float(other_slot, 0x200000).unwrap();
            frame(&context, &mut app, vec![]);
            frame(&context, &mut app, vec![]);
            let id = egui::Id::new(("pickup-drag", slot));
            let start = context.read_response(id).unwrap().rect.center();
            let step = Vec2::new(10.0 * direction, 0.0);
            frame(
                &context,
                &mut app,
                vec![egui::Event::PointerMoved(start), button(start, true)],
            );
            frame(
                &context,
                &mut app,
                vec![egui::Event::PointerMoved(start + step)],
            );
            let moved = app.float(slot, 0x200000).unwrap();
            assert!((moved - initial) * direction > 0.0);
            frame(&context, &mut app, vec![]);
            assert_eq!(app.float(slot, 0x200000), Some(moved));

            let second = start + step * 2.0 + Vec2::new(0.0, 10.0);
            frame(&context, &mut app, vec![egui::Event::PointerMoved(second)]);
            assert!((app.float(slot, 0x200000).unwrap() - (2.0 * moved - initial)).abs() < 1e-5);

            let end = second + step;
            frame(&context, &mut app, vec![button(end, false)]);
            assert!(
                (app.float(slot, 0x200000).unwrap() - (3.0 * moved - 2.0 * initial)).abs() < 1e-5
            );
            assert_eq!(app.float(other_slot, 0x200000), Some(other));
            let changed = app.document.as_ref().unwrap().as_bytes().to_vec();
            app.undo();
            let document = app.document.as_ref().unwrap();
            assert_eq!(document.as_bytes(), before);
            assert!(!document.can_undo());
            app.redo();
            assert_eq!(app.document.as_ref().unwrap().as_bytes(), changed);
        }
    }

    #[test]
    fn fretboard_enable_button_uses_preset_history_and_respects_pending_actions() {
        fn frame(context: &egui::Context, app: &mut Workbench, events: Vec<egui::Event>) {
            let _ = context.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 680.0))),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| app.guitar_workspace(ui));
                },
            );
        }

        let (context, mut app) = app();
        app.load(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/synthetic-host-preset.vwm"),
        );
        app.tab = Tab::Pickups;
        let original = app.document.as_ref().unwrap().as_bytes().to_vec();
        let tuning_enabled = |app: &Workbench| {
            app.document
                .as_ref()
                .unwrap()
                .preset()
                .model(TUNING)
                .unwrap()
                .enabled
        };
        let was_enabled = tuning_enabled(&app);

        let click_toggle = |app: &mut Workbench| {
            frame(&context, app, vec![]);
            frame(&context, app, vec![]);
            let pos = app.tuning_toggle.unwrap().center();
            for pressed in [true, false] {
                frame(
                    &context,
                    app,
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: Default::default(),
                        },
                    ],
                );
            }
        };

        app.pending = Some(Pending::Quit);
        click_toggle(&mut app);
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), original);

        app.pending = None;
        click_toggle(&mut app);
        assert_ne!(tuning_enabled(&app), was_enabled);
        assert_eq!(app.tab, Tab::Pickups);
        app.undo();
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), original);
        assert!(!app.document.as_ref().unwrap().can_undo());
        app.redo();
        assert_ne!(tuning_enabled(&app), was_enabled);

        app.tab = Tab::Controls;
        click_toggle(&mut app);
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), original);
        assert_eq!(app.tab, Tab::Controls);
    }

    #[test]
    fn pickup_enable_keeps_one_on_and_front_follows_the_original() {
        let (context, mut app) = app();
        app.load(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/synthetic-host-preset.vwm"),
        );
        app.tab = Tab::Pickups;
        let enabled = |app: &Workbench, slot: u32| {
            app.document
                .as_ref()
                .unwrap()
                .preset()
                .model(slot)
                .unwrap()
                .enabled
        };
        assert!(enabled(&app, PICKUP1));
        assert!(enabled(&app, PICKUP2));
        assert_eq!(app.pickup_front, PICKUP1);
        let original = app.document.as_ref().unwrap().as_bytes().to_vec();

        let click =
            |context: &egui::Context, app: &mut Workbench, slot: u32, kind: &'static str| {
                recording_frame(context, app, vec![]);
                let pos = app.pickup_controls[&(slot, kind)].center();
                for pressed in [true, false] {
                    recording_frame(
                        context,
                        app,
                        vec![
                            egui::Event::PointerMoved(pos),
                            egui::Event::PointerButton {
                                pos,
                                button: egui::PointerButton::Primary,
                                pressed,
                                modifiers: Default::default(),
                            },
                        ],
                    );
                }
            };

        click(&context, &mut app, PICKUP1, "enabled");
        assert!(!enabled(&app, PICKUP1));
        assert!(enabled(&app, PICKUP2));
        assert_eq!(app.pickup_front, PICKUP2);
        app.undo();
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), original);
        assert!(!app.document.as_ref().unwrap().can_undo());

        click(&context, &mut app, PICKUP2, "front");
        assert_eq!(app.pickup_front, PICKUP2);
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), original);
        assert!(!app.document.as_ref().unwrap().can_undo());

        click(&context, &mut app, PICKUP2, "enabled");
        assert!(enabled(&app, PICKUP1));
        assert!(!enabled(&app, PICKUP2));
        assert_eq!(app.pickup_front, PICKUP1);
        click(&context, &mut app, PICKUP1, "enabled");
        assert!(!enabled(&app, PICKUP1));
        assert!(enabled(&app, PICKUP2));
        assert_eq!(app.pickup_front, PICKUP2);
        app.undo();
        assert!(enabled(&app, PICKUP1));
        assert!(!enabled(&app, PICKUP2));
        assert!(app.document.as_ref().unwrap().can_undo());

        let mut both_off = original.clone();
        for slot in [PICKUP1, PICKUP2] {
            let pattern = slot.to_be_bytes();
            let at = both_off
                .windows(4)
                .enumerate()
                .find_map(|(index, window)| {
                    (window == pattern && index >= 4 && both_off[index - 4] == 0x03)
                        .then_some(index + 7)
                })
                .unwrap();
            both_off[at] = 0;
        }
        let preset = PresetFile::parse(both_off.clone()).unwrap();
        app.document = Some(EditorDocument::from_preset(
            preset,
            PathBuf::from("both-off.vwm"),
        ));
        recording_frame(&context, &mut app, vec![]);
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), both_off);
        assert!(!enabled(&app, PICKUP1));
        assert!(!enabled(&app, PICKUP2));
        assert!(!app.document.as_ref().unwrap().can_undo());
    }

    fn pinf_name_offset(bytes: &[u8]) -> usize {
        let mut cursor = 12;
        while cursor + 8 <= bytes.len() {
            let length =
                u32::from_be_bytes(bytes[cursor + 4..cursor + 8].try_into().unwrap()) as usize;
            if &bytes[cursor..cursor + 4] == b"PINF" {
                return cursor + 16;
            }
            cursor = cursor + 8 + length + (length & 1);
        }
        panic!("missing PINF");
    }

    #[test]
    fn auto_select_follows_pickup_models_and_is_remembered() {
        let directory = tempfile::tempdir().unwrap();
        let missing = directory.path().join("missing.txt");
        assert!(load_auto_select(&missing));
        let garbage = directory.path().join("garbage.txt");
        std::fs::write(&garbage, "maybe\n").unwrap();
        assert!(load_auto_select(&garbage));
        let path = directory.path().join("editor-preferences.txt");
        save_auto_select(&path, false).unwrap();
        assert!(!load_auto_select(&path));

        let context = egui::Context::default();
        let mut app = Workbench::with_context(&context, None, PathBuf::new(), Some(Tab::Controls));
        assert!(app.auto_select_controls);
        assert!(app.error.is_none());
        app.preferences_path = Some(path.clone());
        recording_frame(&context, &mut app, vec![]);
        assert!(app.auto_select_checkbox.is_some());

        let before = app.document.as_ref().unwrap().as_bytes().to_vec();
        let taper = app.integer(WIRING, 0x200001);
        app.select_pickup_model(PICKUP1, 0x03118000);
        assert!(app.error.is_none());
        assert_eq!(app.model_id(PICKUP1), Some(0x03118000));
        assert_eq!(app.float(WIRING, 0x200000), Some(500.0));
        assert_eq!(app.float(WIRING, 0x200002), Some(500.0));
        assert_eq!(app.float(WIRING, 0x200003), Some(0.047));
        assert_eq!(app.integer(WIRING, 0x200001), taper);
        assert!(app.document.as_ref().unwrap().can_undo());
        app.undo();
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), before);
        assert!(!app.document.as_ref().unwrap().can_undo());

        app.select_pickup_model(PICKUP2, 0x03110002);
        assert_eq!(app.float(WIRING, 0x200000), Some(250.0));
        assert_eq!(app.float(WIRING, 0x200002), Some(250.0));
        assert_eq!(app.float(WIRING, 0x200003), Some(0.022));
        app.undo();

        app.set_auto_select_controls(false);
        assert!(app.error.is_none());
        assert_eq!(std::fs::read_to_string(&path).unwrap().trim(), "0");
        app.select_pickup_model(PICKUP1, 0x03118007);
        assert_eq!(app.model_id(PICKUP1), Some(0x03118007));
        assert_eq!(app.float(WIRING, 0x200002), Some(470.0));
        assert_eq!(app.float(WIRING, 0x200003), Some(0.011));

        app.set_auto_select_controls(true);
        assert_eq!(std::fs::read_to_string(&path).unwrap().trim(), "1");
        assert_eq!(app.model_id(PICKUP1), Some(0x03118007));
        assert_eq!(app.float(WIRING, 0x200000), Some(500.0));
        assert_eq!(app.float(WIRING, 0x200002), Some(500.0));
        assert_eq!(app.float(WIRING, 0x200003), Some(0.047));
        app.undo();
        assert_eq!(app.model_id(PICKUP1), Some(0x03118007));
        assert_eq!(app.float(WIRING, 0x200002), Some(470.0));
        assert_eq!(app.float(WIRING, 0x200003), Some(0.011));
    }

    #[test]
    fn legacy_name_limit_keeps_a_saved_name_and_marks_it() {
        let (context, mut app) = app();
        let stored = app.document.as_ref().unwrap().preset().name().to_owned();
        assert!(app.document.as_ref().unwrap().preset().limits_legacy_name());
        assert!(variax_workbench::preset::legacy_text_issue(&stored, true).is_none());
        recording_frame(&context, &mut app, vec![]);
        assert!(app.name_notice.is_empty());

        let original = app.document.as_ref().unwrap().as_bytes().to_vec();
        let at = pinf_name_offset(&original);
        let needle: Vec<u8> = stored.encode_utf16().flat_map(u16::to_be_bytes).collect();
        assert_eq!(&original[at..at + needle.len()], needle.as_slice());
        let long = "Too long for guitar!";
        assert_eq!(long.len(), 20);
        let encoded: Vec<u8> = long.encode_utf16().flat_map(u16::to_be_bytes).collect();
        assert!(at + 64 <= original.len());
        let mut bytes = original.clone();
        bytes[at..at + 64].fill(0);
        bytes[at..at + encoded.len()].copy_from_slice(&encoded);
        assert_eq!(&bytes[..at], &original[..at]);
        assert_eq!(&bytes[at + 64..], &original[at + 64..]);
        let preset = PresetFile::parse(bytes.clone()).unwrap();
        assert_eq!(preset.name(), long);
        assert!(preset.limits_legacy_name());
        app.document = Some(EditorDocument::from_preset(
            preset,
            PathBuf::from("kept.vwm"),
        ));
        recording_frame(&context, &mut app, vec![]);
        assert!(app.name_notice.contains("Model: Kept as saved"));
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), bytes);
        assert!(!app.document.as_ref().unwrap().is_dirty());
        assert!(!app.document.as_ref().unwrap().can_undo());

        app.name_draft = Some("caf\u{e9}".into());
        recording_frame(&context, &mut app, vec![]);
        assert!(app.name_notice.contains("Model: Not saved"));
        assert_eq!(app.document.as_ref().unwrap().preset().name(), long);
        assert!(!app.document.as_ref().unwrap().can_undo());

        app.name_draft = None;
        app.change(|preset| preset.rename("1234567890123456789"));
        assert!(app.error.as_ref().unwrap().contains("18"));
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), bytes);
        app.error = None;
        app.change(|preset| preset.rename("Short name"));
        assert!(app.error.is_none());
        assert_eq!(app.document.as_ref().unwrap().preset().name(), "Short name");
        app.undo();
        assert_eq!(app.document.as_ref().unwrap().preset().name(), long);
        assert!(app.name_draft.is_none());
        assert!(!app.document.as_ref().unwrap().can_undo());
    }

    #[test]
    fn offline_example_opens_as_a_clean_editable_preset() {
        let context = egui::Context::default();
        let mut app = Workbench::with_context(&context, None, PathBuf::new(), None);

        assert!(app.error.is_none());
        assert_eq!(app.tab, Tab::Pickups);
        assert_eq!(app.model_name(BODY), "Spank");
        assert_eq!(app.float(SYSTEM, 0x200000), Some(0.0));
        let document = app.document.as_ref().unwrap();
        assert!(!document.is_dirty());
        assert!(!document.can_undo());
        let original = document.as_bytes().to_vec();

        app.change(|preset| preset.rename("My example"));
        assert!(app.dirty());
        app.undo();
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), original);
    }

    #[test]
    fn artwork_dialog_cancel_and_folder_recovery_preserve_document() {
        let (context, mut app) = app();
        let directory = tempfile::tempdir().unwrap();
        let before = app.document.as_ref().unwrap().as_bytes().to_vec();
        let old = app.artwork.clone();
        app.dialogs = Box::new(FakeDialogs {
            artwork: [None, Some(directory.path().to_owned())].into(),
            ..Default::default()
        });
        assert!(app.texture(&context, "missing-test.png").is_none());
        app.artwork_dialog();
        assert_eq!(app.artwork, old);
        assert!(app.error.is_none());
        assert_eq!(app.missing_textures.len(), 1);
        app.artwork_dialog();
        assert_eq!(app.artwork, directory.path());
        assert!(app.error.is_none());
        assert!(app.missing_textures.is_empty());
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), before);
        assert!(!app.dirty());
    }

    #[test]
    fn active_read_blocks_document_mutations_and_shortcuts() {
        let (context, mut app) = app();
        app.change(|preset| preset.rename("Before read"));
        let before = app.document.as_ref().unwrap().preset().as_bytes().to_vec();
        let (sender, receiver) = std::sync::mpsc::channel();
        app.read_receiver = Some(receiver);
        app.change(|preset| preset.rename("Must not replace it"));
        app.undo();
        app.redo();
        app.load(Path::new("this-file-must-not-be-opened.vwm"));
        app.open_dialog();
        app.save_dialog();
        let modifiers = egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        };
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1120.0, 760.0))),
            modifiers,
            events: vec![egui::Event::Key {
                key: egui::Key::Z,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            }],
            ..Default::default()
        };
        let output = context.run(input, |ctx| app.render(ctx));
        assert!(!output.shapes.is_empty());
        assert_eq!(app.document.as_ref().unwrap().preset().as_bytes(), before);
        assert!(app.is_reading());
        sender.send(Err("simulated read failure".into())).unwrap();
        app.poll_read();
        assert!(!app.is_reading());
        assert_eq!(app.document.as_ref().unwrap().preset().as_bytes(), before);
        assert_eq!(app.error.as_deref(), Some("simulated read failure"));
    }

    #[test]
    fn a_test_build_never_starts_a_live_read_worker() {
        // The settings-read gate is released (2026-11-23); the test-build
        // guard still stops the read before any worker or device access.
        let (_, mut app) = app();
        app.start_read();
        assert!(!app.is_reading());
        assert!(app.read_worker.is_none());
        assert!(app.pending.is_none());
        assert!(app.status.contains("Test builds"), "{}", app.status);
        app.launch_read();
        assert!(!app.is_reading());
        assert!(app.read_worker.is_none());
    }

    pub(super) fn recording_frame(
        context: &egui::Context,
        app: &mut Workbench,
        events: Vec<egui::Event>,
    ) {
        // Tall enough for the PODxt Live pedalboard plus the controls under it.
        let _ = context.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 8000.0))),
                events,
                ..Default::default()
            },
            |ctx| app.render(ctx),
        );
    }

    pub(super) fn recording_frame_tall(
        context: &egui::Context,
        app: &mut Workbench,
        events: Vec<egui::Event>,
    ) {
        let _ = context.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 8000.0))),
                events,
                ..Default::default()
            },
            |ctx| app.render(ctx),
        );
    }

    fn click_device_check(context: &egui::Context, app: &mut Workbench, id: &'static str) {
        recording_frame(context, app, vec![]);
        recording_frame(context, app, vec![]);
        let pos = app.device_check_buttons[id].center();
        recording_frame(
            context,
            app,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        recording_frame(
            context,
            app,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
        recording_frame(context, app, vec![]);
    }

    fn device_check_app() -> (egui::Context, Workbench) {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        click_device_check(&context, &mut app, "open");
        assert!(app.device_check_open);
        (context, app)
    }

    #[test]
    fn device_check_save_failure_is_shown_and_does_not_rerun() {
        let (context, mut app) = device_check_app();
        click_device_check(&context, &mut app, "yes");
        click_device_check(&context, &mut app, "yes");
        let calls = app.device_check.backend().calls();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("SECRET-USER-report.txt");
        app.note_sanitized_report_save(&path);
        app.note_sanitized_report_save(&path);
        recording_frame(&context, &mut app, vec![]);
        assert!(
            app.device_check_visible
                .contains("report with that name already exists")
        );
        assert!(!app.device_check_visible.contains("SECRET"));
        assert!(!app.device_check_notice.contains("SECRET"));
        assert_eq!(app.device_check.backend().calls(), calls);
    }

    #[test]
    fn device_check_window_defaults_to_simulation_and_declines_by_default() {
        let (context, mut app) = device_check_app();
        assert_eq!(app.device_check.mode(), ProbeMode::Simulation);
        assert!(
            app.device_check_visible
                .contains(device_probe::SIMULATION_LABEL)
        );
        assert!(app.device_check_buttons.contains_key("mode-simulation"));
        // "This computer (G1)" is drawn only on Linux while step 1 is open.
        // Elsewhere the radio is omitted, and the window does not say a real
        // reader is open. The Windows portable-math check does not compile
        // this GUI, so that branch is covered when this test is built there.
        if cfg!(target_os = "linux") {
            assert!(app.device_check_buttons.contains_key("mode-this-computer"));
            assert!(
                app.device_check_visible
                    .contains(device_probe::availability_banner())
            );
            assert!(
                app.device_check_visible
                    .contains(device_probe::ProbeMode::ThisComputer.label())
            );
            assert!(
                app.device_check_visible
                    .contains("step 1 waiting for your answer real device: open")
            );
        } else {
            assert!(!app.device_check_buttons.contains_key("mode-this-computer"));
            assert!(
                !app.device_check_visible
                    .contains(device_probe::G1_ONLY_BANNER)
            );
            assert!(
                !app.device_check_visible
                    .contains(device_probe::THIS_COMPUTER_LABEL)
            );
            assert!(!app.device_check_visible.contains("real device: open"));
            assert!(
                app.device_check_visible
                    .contains("Every hardware lock stays closed in this build.")
            );
            assert!(
                app.device_check_visible
                    .contains("step 1 waiting for your answer real device: locked")
            );
        }
        let step_two = if device_probe::live_stage_available(Stage::Identification) {
            "step 2 not run real device: open"
        } else {
            "step 2 not run real device: locked"
        };
        assert!(
            app.device_check_visible.contains(step_two),
            "{}",
            app.device_check_visible
        );
        let step_three = if device_probe::live_stage_available(Stage::SettingsRead) {
            "step 3 not run real device: open"
        } else {
            "step 3 not run real device: locked"
        };
        assert!(
            app.device_check_visible.contains(step_three),
            "step 3\n{}",
            app.device_check_visible
        );
        assert!(
            app.device_check_visible
                .contains("step 4 not run real device: locked"),
            "step 4"
        );
        assert!(
            app.device_check_visible
                .contains(Stage::UsbRecognition.action())
        );
        assert!(
            app.device_check_visible
                .contains(Stage::UsbRecognition.risk())
        );
        assert_eq!(app.host_reader_builds, 0);
        assert!(matches!(
            app.device_check.backend(),
            RigBackend::Simulated(_)
        ));
        // Rendering alone never runs a step.
        for _ in 0..5 {
            recording_frame(&context, &mut app, vec![]);
        }
        assert_eq!(app.device_check.backend().calls(), Default::default());
        // Enter on the focused default button answers No.
        recording_frame(
            &context,
            &mut app,
            vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        recording_frame(&context, &mut app, vec![]);
        assert_eq!(
            app.device_check.status(Stage::UsbRecognition),
            StageStatus::Stopped
        );
        assert_eq!(app.device_check.backend().calls(), Default::default());
        assert!(app.device_check_visible.contains("Stopped at step 1"));
        assert!(!app.device_check_buttons.is_empty());
        assert!(app.read_worker.is_none());
        assert!(!app.is_reading());
    }

    #[test]
    fn device_check_decline_button_stops_without_running() {
        let (context, mut app) = device_check_app();
        click_device_check(&context, &mut app, "no");
        assert_eq!(
            app.device_check.status(Stage::UsbRecognition),
            StageStatus::Stopped
        );
        assert!(app.device_check.finished());
        assert_eq!(app.device_check.backend().calls(), Default::default());
        assert!(app.device_check_visible.contains("You chose No"));
    }

    #[test]
    fn device_check_passed_step_does_not_advance_by_itself() {
        let (context, mut app) = device_check_app();
        click_device_check(&context, &mut app, "yes");
        assert_eq!(
            app.device_check.status(Stage::UsbRecognition),
            StageStatus::Passed
        );
        assert_eq!(app.device_check.awaiting(), Some(Stage::Identification));
        for _ in 0..5 {
            recording_frame(&context, &mut app, vec![]);
        }
        assert_eq!(
            app.device_check.status(Stage::Identification),
            StageStatus::NotRun
        );
        assert_eq!(app.device_check.backend().calls().identity_requests, 0);
        let marker = if device_probe::live_stage_available(Stage::Identification) {
            "open"
        } else {
            "locked"
        };
        assert!(app.device_check_visible.contains(&format!(
            "step 2 waiting for your answer real device: {marker}"
        )));
        assert!(app.device_check_visible.contains("0e41:4650"));
        for _ in 0..3 {
            click_device_check(&context, &mut app, "yes");
        }
        for stage in Stage::LADDER {
            assert_eq!(app.device_check.status(stage), StageStatus::Passed);
        }
        assert!(
            app.device_check_visible
                .contains("All four steps passed in simulation")
        );
        // Simulation never builds or calls the real reader.
        assert_eq!(app.host_reader_builds, 0);
        assert!(matches!(
            app.device_check.backend(),
            RigBackend::Simulated(_)
        ));
    }

    #[test]
    fn device_check_stops_on_failure_and_start_over_resets() {
        let (context, mut app) = device_check_app();
        app.device_check_scenario = device_probe::Scenario::TwoPods;
        click_device_check(&context, &mut app, "restart");
        click_device_check(&context, &mut app, "yes");
        assert_eq!(
            app.device_check.status(Stage::UsbRecognition),
            StageStatus::Stopped
        );
        assert!(app.device_check.finished());
        assert!(app.device_check_visible.contains("2 PODxt Live devices"));
        assert!(!app.device_check_visible.contains("waiting for your answer"));
        assert_eq!(app.device_check.backend().calls().identity_requests, 0);
        click_device_check(&context, &mut app, "restart");
        assert_eq!(app.device_check.awaiting(), Some(Stage::UsbRecognition));
        assert_eq!(app.device_check.backend().calls(), Default::default());
        assert_eq!(app.host_reader_builds, 0);
    }

    /// A temporary fake `/sys` + `/proc` tree with `units` PODxt Live devices.
    /// Tests never point the reader anywhere else.
    #[cfg(target_os = "linux")]
    fn fake_host_tree(units: &[&str]) -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        let write = |path: PathBuf, text: &str| {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        let devices = root.path().join("sys/bus/usb/devices");
        for name in units {
            let base = devices.join(name);
            write(base.join("idVendor"), "0e41\n");
            write(base.join("idProduct"), "4650\n");
            write(base.join("bcdDevice"), "0001\n");
            write(base.join("product"), "PODxt Live\n");
            write(base.join("serial"), "SECRET-SERIAL\n");
            for (number, driver) in [(0, "snd-usb-pod"), (1, "snd-usb-variax")] {
                let interface = devices.join(format!("{name}:1.{number}"));
                write(
                    interface.join("bInterfaceNumber"),
                    &format!("{number:02x}\n"),
                );
                std::fs::create_dir_all(root.path().join("sys/bus/usb/drivers").join(driver))
                    .unwrap();
                std::os::unix::fs::symlink(
                    format!("../../../bus/usb/drivers/{driver}"),
                    interface.join("driver"),
                )
                .unwrap();
            }
        }
        std::fs::create_dir_all(&devices).unwrap();
        write(
            root.path().join("proc/asound/cards"),
            " 0 [PODxtLive      ]: Line6-POD - PODxt Live\n                      Line 6 PODxt Live at USB 1-2:1.0\n",
        );
        root
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn device_check_this_computer_mode_runs_only_step_one_on_an_injected_tree() {
        assert!(device_probe::live_stage_available(Stage::UsbRecognition));
        let tree = fake_host_tree(&["1-2"]);
        let (context, mut app) = device_check_app();
        app.device_check_root = tree.path().to_owned();
        click_device_check(&context, &mut app, "mode-this-computer");
        assert_eq!(app.device_check.mode(), ProbeMode::ThisComputer);
        assert_eq!(app.host_reader_builds, 1);
        let RigBackend::ThisComputer(rig) = app.device_check.backend() else {
            panic!("expected This computer mode");
        };
        assert_eq!(rig.reader().root(), tree.path());
        // Choosing the mode reads nothing; step 1 waits at the default No.
        assert_eq!(app.device_check.backend().calls(), Default::default());
        assert_eq!(app.device_check.awaiting(), Some(Stage::UsbRecognition));
        assert!(
            app.device_check_visible
                .contains(device_probe::availability_banner())
        );
        assert!(
            app.device_check_visible
                .contains(device_probe::this_computer_label())
        );
        if device_probe::live_stage_available(Stage::Identification) {
            assert!(
                app.device_check_visible
                    .contains("step 2 not run real device: open")
            );
        } else {
            assert!(
                app.device_check_visible
                    .contains("step 2 locked in this build")
            );
        }
        if device_probe::live_stage_available(Stage::SettingsRead) {
            assert!(
                app.device_check_visible
                    .contains("step 3 not run real device: open"),
                "step 3: {}",
                app.device_check_visible
            );
        } else {
            assert!(
                app.device_check_visible
                    .contains("step 3 locked in this build"),
                "step 3: {}",
                app.device_check_visible
            );
        }
        assert!(
            app.device_check_visible
                .contains("step 4 locked in this build"),
            "step 4: {}",
            app.device_check_visible
        );
        click_device_check(&context, &mut app, "yes");
        assert_eq!(
            app.device_check.status(Stage::UsbRecognition),
            StageStatus::Passed
        );
        // A usb build offers step 2. Answer No. Do not send the identity request.
        if device_probe::live_stage_available(Stage::Identification) {
            assert_eq!(app.device_check.awaiting(), Some(Stage::Identification));
            assert!(app.device_check.g1_expected_binding());
            assert!(
                app.device_check_visible
                    .contains(device_probe::STEP_TWO_YES)
            );
            click_device_check(&context, &mut app, "no");
            assert_eq!(
                app.device_check.status(Stage::Identification),
                StageStatus::Stopped
            );
            assert!(app.device_check_visible.contains("Stopped at step 2"));
            assert!(device_probe::reports_panel_note().contains("gates G1–G3"));
        } else {
            assert_eq!(app.device_check.awaiting(), None);
            assert!(!app.device_check_buttons.contains_key("yes"));
            assert!(!app.device_check_buttons.contains_key("no"));
            assert_eq!(
                app.device_check.status(Stage::Identification),
                StageStatus::NotRun
            );
            assert!(app.device_check.locked(Stage::Identification));
            assert!(app.device_check_visible.contains("Step 1 finished"));
            assert!(
                app.device_check_visible
                    .contains(device_probe::this_computer_finished_message())
            );
            assert!(device_probe::reports_panel_note().contains("This build can run gate G1."));
        }
        assert_eq!(
            app.device_check.status(Stage::SettingsRead),
            StageStatus::NotRun
        );
        assert_eq!(
            app.device_check.locked(Stage::SettingsRead),
            !device_probe::live_stage_available(Stage::SettingsRead)
        );
        assert_eq!(
            app.device_check.status(Stage::ChangeCheck),
            StageStatus::NotRun
        );
        assert!(app.device_check.locked(Stage::ChangeCheck));
        let calls = app.device_check.backend().calls();
        assert_eq!(
            (
                calls.os_reads,
                calls.identity_requests,
                calls.settings_reads
            ),
            (1, 0, 0)
        );
        assert!(
            app.device_check_visible
                .contains(device_probe::reports_panel_note())
        );
        assert!(app.device_check_visible.contains("0e41:4650"));
        let report = app.device_check.report();
        assert!(report.contains(&format!(
            "mode=this-computer {}",
            device_probe::open_gates().token()
        )));
        assert!(report.contains("interface 1 driver snd-usb-variax"));
        assert!(!report.contains("SECRET"), "serial number leaked");
        assert!(
            !report.contains(&*tree.path().to_string_lossy()),
            "path leaked"
        );
        assert!(app.read_worker.is_none());
    }

    #[cfg(all(target_os = "linux", feature = "usb"))]
    #[test]
    fn device_check_backup_asks_before_it_reads() {
        let tree = fake_host_tree(&["1-2"]);
        let (context, mut app) = device_check_app();
        app.device_check_root = tree.path().to_owned();
        click_device_check(&context, &mut app, "mode-this-computer");
        assert!(app.device_check_visible.contains(BACKUP_VARIAX));
        assert!(app.device_check_visible.contains(BACKUP_PODXT));
        assert!(
            app.device_check_visible
                .contains("step 4 locked in this build")
        );
        click_device_check(&context, &mut app, "backup-variax");
        assert!(app.device_check_visible.contains(BACKUP_VARIAX_DOES));
        click_device_check(&context, &mut app, "backup-no");
        assert!(app.device_check_notice.contains("Nothing was read"));
        assert!(app.backup_rx.is_none());
        click_device_check(&context, &mut app, "backup-podxt");
        assert!(app.device_check_visible.contains(BACKUP_PODXT_DOES));
        click_device_check(&context, &mut app, "backup-yes");
        assert!(app.device_check_notice.contains("nothing was sent"));
        assert!(app.backup_rx.is_none());
        assert_eq!(app.device_check.backend().calls().identity_requests, 0);
        assert_eq!(app.device_check.backend().calls().settings_reads, 0);
    }

    #[cfg(all(target_os = "linux", feature = "usb"))]
    #[test]
    fn device_check_tone_actions_ask_before_they_send() {
        let tree = fake_host_tree(&["1-2"]);
        let (context, mut app) = device_check_app();
        app.device_check_root = tree.path().to_owned();
        click_device_check(&context, &mut app, "mode-this-computer");
        assert!(app.device_check_visible.contains(TONE_TRY));
        assert!(app.device_check_visible.contains(TONE_CHANNEL));
        assert!(app.device_check_visible.contains(TONE_KEEP));
        assert!(
            app.device_check_visible
                .contains("step 4 locked in this build")
        );
        click_device_check(&context, &mut app, "tone-try");
        assert!(app.device_check_visible.contains(TONE_TRY_DOES));
        click_device_check(&context, &mut app, "tone-no");
        assert!(app.device_check_notice.contains("Nothing was changed"));
        assert!(app.tone_rx.is_none());
        click_device_check(&context, &mut app, "tone-try");
        click_device_check(&context, &mut app, "tone-yes");
        assert!(app.device_check_notice.contains("nothing was sent"));
        assert!(app.tone_rx.is_none());
        app.tone_channel = "nope".into();
        click_device_check(&context, &mut app, "tone-channel");
        assert!(app.device_check_notice.contains("1A"));
        assert!(app.tone_choice.is_none());
        app.tone_channel = "2C".into();
        click_device_check(&context, &mut app, "tone-channel");
        assert!(app.device_check_visible.contains("2C"));
        click_device_check(&context, &mut app, "tone-yes");
        assert!(app.device_check_notice.contains("nothing was sent"));
        assert_eq!(app.device_check.backend().calls().identity_requests, 0);
        assert_eq!(app.device_check.backend().calls().settings_reads, 0);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn device_check_this_computer_mode_decline_and_two_units_stop() {
        let tree = fake_host_tree(&["1-2"]);
        let (context, mut app) = device_check_app();
        app.device_check_root = tree.path().to_owned();
        click_device_check(&context, &mut app, "mode-this-computer");
        click_device_check(&context, &mut app, "no");
        assert_eq!(
            app.device_check.status(Stage::UsbRecognition),
            StageStatus::Stopped
        );
        assert_eq!(app.device_check.backend().calls(), Default::default());
        let two = fake_host_tree(&["1-2", "1-3"]);
        app.device_check_root = two.path().to_owned();
        click_device_check(&context, &mut app, "restart");
        assert_eq!(app.device_check.mode(), ProbeMode::ThisComputer);
        click_device_check(&context, &mut app, "yes");
        assert_eq!(
            app.device_check.status(Stage::UsbRecognition),
            StageStatus::Stopped
        );
        assert!(app.device_check_visible.contains("2 PODxt Live devices"));
        assert_eq!(app.device_check.backend().calls().identity_requests, 0);
        // Back to simulation: the default backend again.
        click_device_check(&context, &mut app, "mode-simulation");
        assert!(matches!(
            app.device_check.backend(),
            RigBackend::Simulated(_)
        ));
    }

    /// Enter on the focused Yes button is a real Yes. The close regression
    /// below uses that same key, so a zero-read result means Close won.
    #[cfg(target_os = "linux")]
    #[test]
    fn device_check_enter_on_focused_yes_reads_the_injected_tree() {
        let tree = fake_host_tree(&["1-2"]);
        let (context, mut app) = device_check_app();
        app.device_check_root = tree.path().to_owned();
        click_device_check(&context, &mut app, "mode-this-computer");
        let yes_id = app.device_check_button_ids["yes"];
        context.memory_mut(|memory| memory.request_focus(yes_id));
        recording_frame(
            &context,
            &mut app,
            vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        assert_eq!(app.device_check.backend().calls().os_reads, 1);
        assert!(app.device_check.report().contains("decision=yes"));
    }

    /// Yes and Close in one frame go through `device_check_window`'s dispatch.
    /// Close is handled first, so the injected reader is never called.
    #[cfg(target_os = "linux")]
    #[test]
    fn device_check_close_in_the_same_frame_as_yes_reads_nothing() {
        let tree = fake_host_tree(&["1-2"]);
        let (context, mut app) = device_check_app();
        app.device_check_root = tree.path().to_owned();
        click_device_check(&context, &mut app, "mode-this-computer");
        assert_eq!(app.device_check.mode(), ProbeMode::ThisComputer);
        assert_eq!(app.device_check.awaiting(), Some(Stage::UsbRecognition));
        assert_eq!(app.device_check.backend().calls(), Default::default());
        let builds = app.host_reader_builds;
        let yes_id = app.device_check_button_ids["yes"];
        context.memory_mut(|memory| memory.request_focus(yes_id));
        let window = context
            .memory(|memory| memory.area_rect(egui::Id::new("Device check")))
            .expect("device check window");
        let close_at = Pos2::new(window.right() - 20.0, window.top() + 18.0);
        recording_frame(
            &context,
            &mut app,
            vec![
                egui::Event::Key {
                    key: egui::Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default(),
                },
                egui::Event::PointerMoved(close_at),
                egui::Event::PointerButton {
                    pos: close_at,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
                egui::Event::PointerButton {
                    pos: close_at,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                },
            ],
        );
        assert!(!app.device_check_open, "closing the window must win");
        assert_eq!(
            app.device_check.status(Stage::UsbRecognition),
            StageStatus::Stopped
        );
        assert_eq!(app.device_check.awaiting(), None);
        let report = app.device_check.report();
        assert!(report.contains("decision=cancel"), "{report}");
        assert!(!report.contains("decision=yes"), "{report}");
        assert_eq!(
            app.device_check.backend().calls(),
            Default::default(),
            "the injected reader must record zero calls"
        );
        assert_eq!(app.host_reader_builds, builds, "close must not restart");
        assert!(app.read_worker.is_none());
    }

    #[test]
    fn consent_preflight_step_two_screen_saves_before_g1() {
        let (context, mut app) = device_check_app();
        click_device_check(&context, &mut app, "yes");
        assert_eq!(app.device_check.awaiting(), Some(Stage::Identification));
        assert!(app.device_check.g1_expected_binding());
        assert!(
            app.device_check_visible
                .contains(device_probe::STEP_TWO_YES_SIMULATION),
            "{}",
            app.device_check_visible
        );
        assert!(
            app.device_check_visible
                .contains(device_probe::CANCEL_LIMIT)
        );
        assert!(
            app.device_check_visible
                .contains(device_probe::APP_REQUEST_VERSUS_LINUX)
        );
        assert!(
            app.device_check_visible
                .contains(device_probe::STEP_TWO_PREFLIGHT)
        );
        assert!(app.device_check_buttons.contains_key("no"));
        assert_eq!(device_probe::DEFAULT_CONSENT, device_probe::Consent::No);
        click_device_check(&context, &mut app, "yes");
        assert_eq!(
            app.device_check.status(Stage::Identification),
            StageStatus::Passed
        );
        assert!(app.device_check.identification_answered());
        assert!(
            app.device_check_visible
                .contains(device_probe::SAVE_THEN_CHECK_AGAIN)
        );
        let RigBackend::Simulated(rig) = app.device_check.backend() else {
            panic!("simulation");
        };
        assert_eq!(rig.identity_calls(), 8);
        click_device_check(&context, &mut app, "restart");
        assert_eq!(
            app.device_check.status(Stage::Identification),
            StageStatus::Passed
        );
        assert!(
            app.device_check_visible
                .contains(device_probe::SAVE_BEFORE_RESTART),
            "{}",
            app.device_check_visible
        );
        let RigBackend::Simulated(rig) = app.device_check.backend() else {
            panic!("simulation");
        };
        assert_eq!(rig.identity_calls(), 8);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("step2-report.txt");
        app.dialogs = Box::new(FakeDialogs {
            save: [Some(path.clone())].into(),
            ..FakeDialogs::default()
        });
        click_device_check(&context, &mut app, "verify-g1");
        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(
            saved.contains(device_probe::open_gates().token()),
            "{saved}"
        );
        assert!(saved.contains("guitar=variax-600-3.10"), "{saved}");
        assert!(saved.contains("stage=2"), "{saved}");
        assert!(!saved.contains("SECRET"));
        assert_eq!(app.device_check.awaiting(), Some(Stage::UsbRecognition));
        assert!(!app.device_check.g1_expected_binding());
        assert!(!app.device_check.identification_answered());
        assert!(app.device_check_visible.contains("Step 1 is ready again."));
        assert_eq!(app.device_check.backend().calls(), Default::default());
        let RigBackend::Simulated(rig) = app.device_check.backend() else {
            panic!("fresh simulation");
        };
        assert_eq!(rig.identity_calls(), 0);
    }

    fn click_recording(context: &egui::Context, app: &mut Workbench, id: &'static str) {
        recording_frame(context, app, vec![]);
        let pos = app.recording_buttons[id].center();
        recording_frame(
            context,
            app,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        recording_frame(
            context,
            app,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
    }

    #[test]
    fn offline_test_recording_window_does_not_start_a_device_or_a_worker() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("offline-test-take.wav");
        app.dialogs = Box::new(FakeDialogs {
            save: [Some(path.clone())].into(),
            ..Default::default()
        });
        click_recording(&context, &mut app, "open-recording");
        assert!(app.recording_open);
        assert!(app.recording_visible.contains(recording::OFFLINE_BANNER));
        assert!(
            app.recording_visible
                .contains(recording::DEVICE_CAPTURE_LOCKED)
        );
        assert!(app.recording_visible.contains(recording::NO_AUTO_START));
        assert!(
            app.recording_visible
                .contains("Native live locked; external input unsupported.")
        );
        assert!(app.recording_visible.contains(recording::PHASE_IDLE));
        assert!(app.recording_visible.contains("not a live route change"));
        assert!(app.read_worker.is_none());
        assert!(!app.is_reading());
        assert_eq!(app.recording_capture_attempts, 0);
        assert_eq!(app.recorder.phase(), recording::Phase::Idle);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
        click_recording(&context, &mut app, "start");
        assert_eq!(app.recorder.phase(), recording::Phase::Idle);
        assert!(app.read_worker.is_none());
        click_recording(&context, &mut app, "route-clean");
        click_recording(&context, &mut app, "inspect-native-plan");
        #[cfg(target_os = "linux")]
        {
            assert!(app.recording_visible.contains("Prepared declaration only"));
            assert!(app.recording_visible.contains("78125/2 Hz"));
            assert!(app.recording_visible.contains("2 channels mapped 0→L, 1→R"));
        }
        #[cfg(not(target_os = "linux"))]
        {
            assert!(app.recording_visible.contains("UnverifiedPlatform"));
        }
        assert_eq!(app.recorder.phase(), recording::Phase::Idle);
        assert_eq!(
            app.recorder.route(),
            Some(variax_workbench::pod_audio::RecordRoute::SendCleanGuitar)
        );
        assert!(!app.recorder.live_monitor_changed());
        assert!(!app.recorder.live_route_changed());
        assert_eq!(app.recording_capture_attempts, 0);
        click_recording(&context, &mut app, "check-external-plan");
        assert!(
            app.recording_visible
                .contains("no independently admitted interface record")
        );
        assert_eq!(app.recorder.phase(), recording::Phase::Idle);
        assert_eq!(app.recording_capture_attempts, 0);
        click_recording(&context, &mut app, "clock-44100");
        click_recording(&context, &mut app, "input-synthetic");
        assert!(app.recording_visible.contains("not a live route change"));
        assert!(!app.recorder.live_monitor_changed());
        assert!(!app.recorder.live_route_changed());
        click_recording(&context, &mut app, "start");
        assert_eq!(app.recorder.phase(), recording::Phase::Recording);
        assert_eq!(app.recording_capture_attempts, 0);
        assert!(app.read_worker.is_none());
        assert!(!app.recorder.device_opened());
        click_recording(&context, &mut app, "feed");
        assert!(app.recording_visible.contains("Left peak 1000"));
        assert!(app.recording_visible.contains("Right peak 2000"));
        click_recording(&context, &mut app, "stop");
        click_recording(&context, &mut app, "save");
        let wav = std::fs::read(&path).unwrap();
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 44_100);
        let sidecar =
            std::fs::read_to_string(format!("{}.recording.json", path.display())).unwrap();
        assert!(sidecar.contains("\"device_recording\": false"));
        assert!(sidecar.contains("\"sample_rate_hz\": 44100"));
        assert!(!sidecar.contains("39062"));
        assert!(!sidecar.contains("78125"));
        assert!(app.read_worker.is_none());
        app.start_read();
        assert!(app.read_worker.is_none());
        const {
            assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn rendered_simulated_callback_workflow_is_synthetic_and_offline() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("simulated-callback.wav");
        app.dialogs = Box::new(FakeDialogs {
            save: [Some(path.clone())].into(),
            ..Default::default()
        });
        click_recording(&context, &mut app, "open-recording");
        click_recording(&context, &mut app, "route-clean");
        click_recording(&context, &mut app, "clock-48000");
        click_recording(&context, &mut app, "input-simulated-callback");
        click_recording(&context, &mut app, "start");
        assert_eq!(app.recorder.phase(), recording::Phase::Recording);
        assert!(app.recording_visible.contains("No hardware connection"));
        assert!(
            app.recording_visible
                .contains("Generated software test data")
        );
        assert_eq!(app.recording_capture_attempts, 0);
        assert!(app.read_worker.is_none());
        assert!(!app.recorder.device_opened());
        click_recording(&context, &mut app, "feed");
        click_recording(&context, &mut app, "stop");
        click_recording(&context, &mut app, "save");
        let wav = std::fs::read(&path).unwrap();
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 48_000);
        assert_eq!(&wav[44..48], &[0xe8, 0x03, 0x30, 0xf8]);
        assert_eq!(app.recording_capture_attempts, 0);
        assert!(app.read_worker.is_none());
        assert!(!app.recorder.device_opened());
    }

    pub(super) fn prepare_quit_take(app: &mut Workbench, clock: ClockSelection) {
        app.recorder
            .select_route(RecordRoute::SendCleanGuitar)
            .unwrap();
        app.recorder.select_clock(clock).unwrap();
        app.recorder
            .select_input(recording::InputSelection::Synthetic)
            .unwrap();
        app.recorder.start().unwrap();
        app.recorder.feed().unwrap();
    }

    fn prepare_quit_callback_take(app: &mut Workbench, clock: ClockSelection) {
        app.recorder
            .select_route(RecordRoute::SendCleanGuitar)
            .unwrap();
        app.recorder.select_clock(clock).unwrap();
        app.recorder
            .select_input(recording::InputSelection::SimulatedCallback)
            .unwrap();
        app.recorder.start().unwrap();
        app.recorder.feed().unwrap();
    }

    #[test]
    fn recording_quit_cancel_and_failed_save_retain_audio_until_success() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let dir = tempfile::tempdir().unwrap();
        let blocked = dir.path().join("blocked.wav");
        let good = dir.path().join("saved.wav");
        std::fs::write(&blocked, b"keep").unwrap();
        app.dialogs = Box::new(FakeDialogs {
            save: [None, Some(blocked.clone()), Some(good.clone())].into(),
            ..Default::default()
        });
        prepare_quit_take(&mut app, ClockSelection::Integer(44_100));
        let levels = app.recorder.levels();
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitRecording)));
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pending.is_none());
        assert_eq!(app.recorder.phase(), recording::Phase::Recording);
        assert!(app.recorder.has_unsaved_take());
        assert!(!app.permit_close);
        app.request_close(&context);
        click_prompt(&context, &mut app, "save"); // explicit stop, cancelled dialog
        assert_eq!(app.recorder.phase(), recording::Phase::Stopped);
        assert!(matches!(app.pending, Some(Pending::QuitRecording)));
        assert!(app.recorder.has_unsaved_take());
        assert_eq!(app.recorder.levels(), levels);
        assert!(!app.permit_close);
        click_prompt(&context, &mut app, "save"); // refused overwrite
        assert!(matches!(app.pending, Some(Pending::QuitRecording)));
        assert!(app.recorder.has_unsaved_take());
        assert_eq!(std::fs::read(&blocked).unwrap(), b"keep");
        assert_eq!(app.recorder.levels(), levels);
        assert!(!app.permit_close);
        click_prompt(&context, &mut app, "save");
        assert!(!app.recorder.has_unsaved_take());
        assert!(app.pending.is_none());
        assert!(app.permit_close);
        let decoded = recording::parse_wav(&std::fs::read(&good).unwrap()).unwrap();
        assert!(!decoded.data.is_empty());
        assert!(
            decoded
                .data
                .as_chunks::<4>()
                .0
                .iter()
                .all(|frame| *frame == [0xe8, 3, 0x30, 0xf8])
        );
        assert_eq!(app.recording_capture_attempts, 0);
        assert!(app.read_worker.is_none());
    }

    #[test]
    fn native_clock_quit_cancelled_save_retains_the_take_until_explicit_discard() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        prepare_quit_take(&mut app, ClockSelection::NativePodxt);
        app.dialogs = Box::new(FakeDialogs {
            save: [None].into(),
            ..Default::default()
        });
        app.recorder.stop().unwrap();
        app.request_close(&context);
        click_prompt(&context, &mut app, "save"); // cancelled AIFF picker
        assert!(matches!(app.pending, Some(Pending::QuitRecording)));
        assert!(app.recorder.has_unsaved_take());
        assert!(!app.permit_close);
        click_prompt(&context, &mut app, "cancel");
        assert_eq!(app.recorder.phase(), recording::Phase::Stopped);
        assert!(app.recorder.has_unsaved_take());
        app.request_close(&context);
        click_prompt(&context, &mut app, "discard");
        assert!(!app.recorder.has_unsaved_take());
        assert_eq!(app.recorder.phase(), recording::Phase::Idle);
        assert!(app.pending.is_none());
        assert!(app.permit_close);
        assert!(!app.recorder.device_opened());
    }

    #[test]
    fn native_recording_quit_saves_an_exact_clock_file_and_retains_its_source() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("native.aiff");
        app.dialogs = Box::new(FakeDialogs {
            save: [Some(path.clone())].into(),
            ..Default::default()
        });
        prepare_quit_take(&mut app, ClockSelection::NativePodxt);
        app.request_close(&context);
        click_prompt(&context, &mut app, "save");
        assert!(app.pending.is_none());
        assert!(app.permit_close);
        assert!(!app.recorder.has_unsaved_take());
        assert_eq!(app.recorder.phase(), recording::Phase::Stopped);
        let bytes = std::fs::read(&path).unwrap();
        let source = recording::aiff::decode(&bytes).unwrap();
        assert_eq!(source.len(), 256 * 6);
        assert!(
            source
                .as_chunks::<6>()
                .0
                .iter()
                .all(|frame| *frame == [0xe8, 3, 0, 0x30, 0xf8, 0xff])
        );
        let metadata: serde_json::Value = serde_json::from_slice(
            &std::fs::read(recording::sidecar_path(&path).unwrap()).unwrap(),
        )
        .unwrap();
        assert_eq!(metadata["native_clock_exported"], true);
        assert_eq!(metadata["source_clock"]["denominator"], 2);
        app.permit_close = false;
        app.recorder.cancel().unwrap();
        prepare_quit_take(&mut app, ClockSelection::NativePodxt);
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitRecording)));
        assert!(!app.permit_close);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert!(app.read_worker.is_none());
        assert_eq!(app.recording_capture_attempts, 0);
    }

    #[test]
    fn guitar_and_pod_quit_continuations_cannot_skip_an_unsaved_take() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let dir = tempfile::tempdir().unwrap();
        let saved = dir.path().join("guitar.vwm");
        app.dialogs = Box::new(FakeDialogs {
            save: [Some(saved.clone())].into(),
            ..Default::default()
        });
        app.change(|preset| preset.rename("Guitar changed"));
        let index = app.pod_library.import_bytes(sample_pod_tone()).unwrap();
        app.pod_selected = Some(index);
        app.pod_name = "POD changed".into();
        app.apply_pod_name();
        prepare_quit_callback_take(&mut app, ClockSelection::Integer(48_000));
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::Quit)));
        click_prompt(&context, &mut app, "save");
        assert!(saved.exists());
        assert!(!app.dirty());
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        assert!(!app.permit_close);
        click_prompt(&context, &mut app, "discard");
        assert!(!app.pod_library.any_dirty());
        assert!(matches!(app.pending, Some(Pending::QuitRecording)));
        assert!(app.recorder.has_unsaved_take());
        assert!(!app.permit_close);
        // Background editors cannot alter the take while this decision is pending.
        app.recording_open = true;
        click_recording(&context, &mut app, "cancel");
        assert!(app.recorder.has_unsaved_take());
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pending.is_none());
        assert!(app.recorder.has_unsaved_take());
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitRecording)));
        click_prompt(&context, &mut app, "discard");
        assert!(app.permit_close);
        assert!(!app.recorder.has_unsaved_take());
        assert!(app.read_worker.is_none());
        const {
            assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn load_recording_wav_refuses_a_nonregular_path_without_opening_it() {
        let (_context, mut app) = app();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("not-a-regular-wav");
        std::fs::create_dir(&path).unwrap();
        app.dialogs = Box::new(FakeDialogs {
            wav: [Some(path)].into(),
            ..Default::default()
        });
        app.load_recording_wav();
        assert!(app.recorder.message().contains("regular file"));
        assert!(app.recorder.message().contains("not opened"));
        assert!(app.read_worker.is_none());
        assert!(!app.is_reading());
        assert_eq!(app.recording_capture_attempts, 0);
        assert!(!app.recorder.device_opened());
        const {
            assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn rendering_imported_out_of_range_values_does_not_edit_or_create_history() {
        let (context, mut app) = app();
        let mut bytes = include_bytes!("../tests/fixtures/synthetic-host-preset.vwm").to_vec();
        for (id, kind, old, new) in [
            (0x3f200000u32, 1u32, 6f32.to_bits(), f32::NAN.to_bits()),
            (0x3f200001, 1, 45f32.to_bits(), 95f32.to_bits()),
            (0x3f200000, 0, (-2i32) as u32, 1000),
            (0x3f200006, 1, 50f32.to_bits(), 200f32.to_bits()),
        ] {
            let pattern = [id.to_be_bytes(), kind.to_be_bytes(), old.to_be_bytes()].concat();
            let offset = bytes
                .windows(pattern.len())
                .position(|w| w == pattern)
                .unwrap();
            bytes[offset + 8..offset + 12].copy_from_slice(&new.to_be_bytes());
        }
        app.document = Some(EditorDocument::from_preset(
            PresetFile::parse(bytes.clone()).unwrap(),
            "imported.vwm".into(),
        ));
        for tab in [Tab::Body, Tab::Pickups, Tab::Controls] {
            app.tab = tab;
            for _ in 0..2 {
                let _ = context.run(egui::RawInput::default(), |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| app.guitar_workspace(ui));
                });
            }
            let doc = app.document.as_ref().unwrap();
            assert_eq!(doc.preset().as_bytes(), bytes);
            assert!(!doc.is_dirty());
            assert!(!doc.can_undo());
        }
    }

    #[test]
    fn missing_artwork_is_cached_without_altering_the_document() {
        let (context, mut app) = app();
        let before = app.document.as_ref().unwrap().preset().as_bytes().to_vec();
        let path = "definitely-not-an-original-asset.png";
        assert!(app.texture(&context, path).is_none());
        assert!(app.missing_textures.contains(path));
        assert!(app.texture(&context, path).is_none());
        assert_eq!(app.missing_textures.len(), 1);
        assert_eq!(app.document.as_ref().unwrap().preset().as_bytes(), before);
        assert!(app.error.is_none());
    }

    #[test]
    fn invalid_arguments_return_errors_instead_of_panicking() {
        let root = Path::new("project");
        for args in [
            vec!["--preset"],
            vec!["--artwork", "--preset"],
            vec!["--unknown"],
            vec!["--tab", "invalid"],
            vec!["--screenshot", "image.jpg"],
            vec!["--display", "unknown"],
        ] {
            assert!(parse_options(args.into_iter().map(str::to_owned), root).is_err());
        }
        let options = parse_options(
            [
                "--tab",
                "controls",
                "--preset",
                "model.vwm",
                "--screenshot",
                "preview.png",
            ]
            .into_iter()
            .map(str::to_owned),
            root,
        )
        .unwrap();
        assert_eq!(options.tab, Some(Tab::Controls));
        assert_eq!(options.preset.as_deref(), Some(Path::new("model.vwm")));
    }

    #[test]
    fn fresh_install_opens_the_example_without_assets_and_can_open_a_file() {
        let directory = tempfile::tempdir().unwrap();
        let options = parse_options(Vec::new(), &directory.path().join("artwork")).unwrap();
        assert!(options.preset.is_none());
        let context = egui::Context::default();
        let mut app =
            Workbench::with_context(&context, options.preset, options.artwork, options.tab);
        app.theme_polled = Instant::now();
        let _ = context.run(egui::RawInput::default(), |ctx| app.render(ctx));
        assert!(app.error.is_none());
        assert_eq!(app.tab, Tab::Pickups);
        assert_eq!(
            app.document.as_ref().unwrap().preset().name(),
            "Spank example"
        );
        assert!(app.read_worker.is_none());
        assert!(app.textures.is_empty());
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
        let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/editor-preset.vwm");
        app.dialogs = Box::new(FakeDialogs {
            open: [Some(file.clone())].into(),
            ..Default::default()
        });
        app.open_dialog();
        assert!(app.error.is_none());
        let before = fs::read(file).unwrap();
        let _ = context.run(egui::RawInput::default(), |ctx| app.render(ctx));
        assert!(app.error.is_none());
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), before);
        assert!(!app.dirty());
    }

    #[test]
    fn startup_honors_the_requested_file_and_tab() {
        let directory = tempfile::tempdir().unwrap();
        let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/editor-preset.vwm");
        let artwork = directory.path().join("artwork");
        let context = egui::Context::default();

        let example = Workbench::with_context(&context, None, artwork.clone(), Some(Tab::Controls));
        assert_eq!(example.tab, Tab::Controls);
        assert_eq!(
            example.document.as_ref().unwrap().preset().name(),
            "Spank example"
        );

        let supplied = Workbench::with_context(&context, Some(file.clone()), artwork.clone(), None);
        assert_eq!(supplied.tab, Tab::Body);
        assert_eq!(
            supplied.document.as_ref().unwrap().as_bytes(),
            fs::read(file).unwrap()
        );

        let missing = directory.path().join("missing.vwm");
        let failed = Workbench::with_context(&context, Some(missing), artwork, None);
        assert!(failed.document.is_none());
        assert!(failed.error.as_deref().unwrap().contains("missing.vwm"));
    }

    #[test]
    fn desktop_file_argument_and_explicit_artwork_override_data_directory() {
        let root = Path::new("user-data");
        let parse = |args: &[&str]| parse_options(args.iter().map(|s| (*s).to_owned()), root);
        let options = parse(&["a preset.vwm", "--data-dir", "chosen"]).unwrap();
        assert_eq!(options.preset.as_deref(), Some(Path::new("a preset.vwm")));
        assert_eq!(options.artwork, Path::new("chosen/artwork"));
        for args in [
            vec!["--artwork", "images", "--data-dir", "chosen"],
            vec!["--data-dir", "chosen", "--artwork", "images"],
        ] {
            assert_eq!(parse(&args).unwrap().artwork, Path::new("images"));
        }
        assert!(parse(&["one.vwm", "two.vwm"]).is_err());
        assert!(parse(&["one.vwm", "--preset", "two.vwm"]).is_err());
        assert!(parse(&["--data-dir"]).is_err());
    }

    #[test]
    fn artwork_lookup_uses_the_checkout_when_the_installed_folder_is_absent() {
        let directory = tempfile::tempdir().unwrap();
        let installed = directory.path().join("user-data/artwork");
        let checkout = directory.path().join("source checkout");
        let extracted = checkout.join("assets/original-ui/modules/variax/editor/images");
        let resolve = || default_artwork_from_checkout(installed.clone(), &checkout);

        assert_eq!(resolve(), installed);
        assert!(!installed.exists());
        fs::create_dir_all(&extracted).unwrap();
        assert_eq!(resolve(), extracted);
        fs::create_dir_all(&installed).unwrap();
        assert_eq!(resolve(), installed);
    }

    #[test]
    fn screenshot_output_creates_a_png_and_never_replaces_existing_data() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("preview.png");
        save_screenshot(&path, &[0, 0, 0, 255], 1, 1).unwrap();
        let original = fs::read(&path).unwrap();
        assert!(original.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(save_screenshot(&path, &[255, 0, 0, 255], 1, 1).is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
    }

    #[test]
    fn bank_dropdown_and_slots_preserve_edits_and_block_browsing_during_an_operation() {
        fn frame(context: &egui::Context, app: &mut Workbench, events: Vec<egui::Event>) {
            let _ = context.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1120., 760.))),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default()
                        .show(ctx, |ui| app.guitar_workspace(ui))
                        .inner
                },
            );
        }

        fn click(context: &egui::Context, app: &mut Workbench, pos: Pos2) {
            for pressed in [true, false] {
                frame(
                    context,
                    app,
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: Default::default(),
                        },
                    ],
                );
            }
            frame(context, app, vec![]);
        }

        fn bank(context: &egui::Context, app: &mut Workbench, bank: usize) {
            click(context, app, app.bank_dropdown.unwrap().center());
            let pos = app.bank_options[&bank].center();
            click(context, app, pos);
        }

        use variax_workbench::preset::PresetText;
        for name in ["synthetic-bank.vwb", "synthetic-collection.l6c"] {
            let (context, mut app) = app();
            context.style_mut(|style| style.animation_time = 0.);
            app.load(
                &Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures")
                    .join(name),
            );
            frame(&context, &mut app, vec![]);
            frame(&context, &mut app, vec![]);
            assert_eq!(app.document.as_ref().unwrap().selection(), Some((1, 0)));
            bank(&context, &mut app, 0); // The first bank is empty.
            assert_eq!(app.document.as_ref().unwrap().selection(), Some((1, 0)));
            click(&context, &mut app, Pos2::new(1050.0, 120.0));
            bank(&context, &mut app, 2);
            let slot = app.bank_slots[&1].center();
            click(&context, &mut app, slot);
            assert_eq!(app.document.as_ref().unwrap().selection(), Some((2, 1)));
            app.change(|p| p.set_metadata(PresetText::Author, "Guitarist"));
            app.change(|p| p.set_metadata(PresetText::Notes, "Drop D\nStudio take 🎸"));
            bank(&context, &mut app, 1);

            let doc = app.document.as_ref().unwrap();
            assert_eq!(doc.selection(), Some((1, 1)));
            assert!(doc.slot_is_dirty(2, 1));
            assert!(!doc.slot_is_dirty(1, 1));
            assert!(doc.is_dirty());

            let (sender, receiver) = std::sync::mpsc::channel();
            app.read_receiver = Some(receiver);
            frame(&context, &mut app, vec![]);
            let slot = app.bank_slots[&0].center();
            click(&context, &mut app, slot);
            assert_eq!(app.document.as_ref().unwrap().selection(), Some((1, 1)));
            sender
                .send(Err("offline simulated failure".into()))
                .unwrap();
            app.poll_read();

            let directory = tempfile::tempdir().unwrap();
            let destination = directory.path().join(name);
            let doc = app.document.as_mut().unwrap();
            doc.save_copy(&destination).unwrap();
            assert!(!doc.slot_is_dirty(2, 1));
            assert!(!doc.is_dirty());
            let mut reopened = EditorDocument::open(&destination).unwrap();
            reopened.select(2, 1).unwrap();
            assert_eq!(
                reopened.preset().metadata(PresetText::Author).unwrap(),
                "Guitarist"
            );
            assert_eq!(
                reopened.preset().metadata(PresetText::Notes).unwrap(),
                "Drop D\nStudio take 🎸"
            );

            app.undo();
            assert!(app.document.as_ref().unwrap().slot_is_dirty(2, 1));
            app.redo();
            assert!(!app.document.as_ref().unwrap().is_dirty());
        }
    }
}
