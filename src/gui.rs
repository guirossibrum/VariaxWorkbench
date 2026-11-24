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

#[path = "families/hd500x/editor/mod.rs"]
mod hd500x_panel;
use hd500x_panel::{
    PendingHdType8Choice, PendingHdType9Choice, PendingHdType10Choice, PendingHdType11Choice,
    PendingHdType12Choice, PendingHdType13Choice, PendingHdType19Choice,
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
