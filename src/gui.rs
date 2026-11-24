//! Offline Rust editor using recovered Workbench artwork and file semantics.
use eframe::egui::{self, Color32, Pos2, Rect, Sense, TextureHandle, Vec2};
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
use variax_workbench::pod_preset::{self, Library, PodPresetFile};
use variax_workbench::recording;
use variax_workbench::rig::{HdEditorLibrary, ProductProfile, Rejection, RoleKind, Session};
use variax_workbench::{document::EditorDocument, preset::PresetFile};

#[path = "families/variax/editor/mod.rs"]
mod variax_panel;
use variax_panel::{FRET_MARKER, ModelArt, PICKUP1, Tab, load_auto_select};

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

#[path = "tools/recording/panel.rs"]
mod recording_panel;

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
        let catalog = variax_panel::load_catalog();
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
        self.variax_pending_dialog(ctx);
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
                    variax_panel::milestones(ui);
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
    pub(super) use super::recording_panel::tests::prepare_quit_take;
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
}
