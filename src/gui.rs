//! Offline Rust editor using recovered Workbench artwork and file semantics.
use eframe::egui::{self, Color32, Pos2, Rect, Sense, TextureHandle, Vec2};
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    fs,
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

#[derive(Clone, Copy)]
enum PodMembershipAction {
    Duplicate,
    Remove,
    MoveUp,
    MoveDown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PendingStompHarmony {
    Harmony1M335(pod_preset::StompHarmony1M335Selection),
    Harmony1457(pod_preset::StompHarmony1457Selection),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum PendingDelayEdit {
    Note(pod_preset::DelayNoteChoice),
    TimeMs(f32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum PendingModEdit {
    Note(pod_preset::ModNoteChoice),
    SpeedHz(f32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PendingPodEqReset {
    document_generation: u64,
    index: usize,
    context: Vec<u8>,
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

fn suggested_tone_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|ch| if ch == '/' || ch == '\\' { '-' } else { ch })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "podxt-live-tone".into()
    } else {
        trimmed.to_owned()
    }
}

fn rejection_text(rejection: &Rejection) -> String {
    match rejection {
        Rejection::Format { detail } => detail.clone(),
        Rejection::CrossFamily { .. } => {
            "that file is a Variax tone, not a PODxt Live preset".into()
        }
        Rejection::Unsupported { .. } => "this profile cannot open a PODxt Live tone".into(),
        Rejection::Locked { .. } => "that operation stays locked".into(),
        other => format!("{other:?}"),
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

/// Built-in offline examples for the Practice menu. Both are the same
/// synthetic documents the existing GUI tests already build.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum PracticeTone {
    AmpAndEffects,
    StompboxCollection,
}

impl PracticeTone {
    const ALL: [PracticeTone; 2] = [
        PracticeTone::AmpAndEffects,
        PracticeTone::StompboxCollection,
    ];

    fn label(self) -> &'static str {
        match self {
            PracticeTone::AmpAndEffects => "Amp and Effects Tone",
            PracticeTone::StompboxCollection => "Stompbox Collection",
        }
    }

    fn bytes(self) -> Vec<u8> {
        match self {
            PracticeTone::AmpAndEffects => sample_amp_effects_tone(b"L6PA"),
            PracticeTone::StompboxCollection => {
                let tone = sample_stomp_model_tone(0x0205_0006);
                sample_pod_bundle(std::slice::from_ref(&tone))
            }
        }
    }
}

#[cfg(test)]
fn sample_drive_pod_tone() -> Vec<u8> {
    sample_amp_effects_tone(b"L6AS")
}

/// Pedalboard order for the PODxt Live tab. Amp, then Stomp, then Mod, then Delay.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum PodBlock {
    Amp,
    Stomp,
    Mod,
    Delay,
}

impl PodBlock {
    const ALL: [PodBlock; 4] = [
        PodBlock::Amp,
        PodBlock::Stomp,
        PodBlock::Mod,
        PodBlock::Delay,
    ];

    const fn label(self) -> &'static str {
        match self {
            PodBlock::Amp => "Amp",
            PodBlock::Stomp => "Stomp",
            PodBlock::Mod => "Mod",
            PodBlock::Delay => "Delay",
        }
    }

    const fn enable(self) -> pod_preset::PodModelEnable {
        match self {
            PodBlock::Amp => pod_preset::PodModelEnable::Amp,
            PodBlock::Stomp => pod_preset::PodModelEnable::Stomp,
            PodBlock::Mod => pod_preset::PodModelEnable::Mod,
            PodBlock::Delay => pod_preset::PodModelEnable::Delay,
        }
    }
}

fn sample_amp_effects_tone(container: &[u8; 4]) -> Vec<u8> {
    fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut output = id.to_vec();
        output.extend_from_slice(&(data.len() as u32).to_be_bytes());
        output.extend_from_slice(data);
        if !data.len().is_multiple_of(2) {
            output.push(0xa5);
        }
        output
    }
    fn parameter(id: u32, value: f32) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&((0x3fu32 << 24) | id).to_be_bytes());
        data.extend_from_slice(&1u32.to_be_bytes());
        data.extend_from_slice(&value.to_bits().to_be_bytes());
        chunk(b"PARM", &data)
    }
    fn model(slot: u32, model_id: u32, parameters: &[Vec<u8>]) -> Vec<u8> {
        let mut minf = Vec::new();
        minf.extend_from_slice(&model_id.to_be_bytes());
        minf.extend_from_slice(&slot.to_be_bytes());
        minf.extend_from_slice(&[0, 0, 0, 1]);
        let mut body = b"MODL".to_vec();
        body.extend(chunk(b"MINF", &minf));
        for parameter in parameters {
            body.extend_from_slice(parameter);
        }
        body.extend(chunk(b"XTRA", &[0xde, 0xad, 0xbe]));
        chunk(b"LIST", &body)
    }

    let mut info = vec![0; 76];
    info[..4].copy_from_slice(&1u32.to_be_bytes());
    info[4..8].copy_from_slice(&pod_preset::PODXT_LIVE_TONE_ID.to_be_bytes());
    info[8..10].copy_from_slice(&u16::from(b'A').to_be_bytes());
    info[72..76].copy_from_slice(&pod_preset::PODXT_LIVE_HOST_MIN_VERSION.to_be_bytes());
    let mut patch = b"PATC".to_vec();
    patch.extend(model(
        pod_preset::AMP_SLOT,
        pod_preset::PLEXI_45_MODEL,
        &[parameter(pod_preset::AMP_DRIVE_PARAM, 0.5)],
    ));
    patch.extend(model(
        pod_preset::STOMP_SLOT,
        pod_preset::STOMP_DRIVE_MODEL_ID,
        &[
            parameter(pod_preset::STOMP_DRIVE_PARAM, 0.25),
            parameter(pod_preset::STOMP_GAIN_PARAM, 0.5),
            parameter(pod_preset::STOMP_TONE_PARAM, 0.75),
        ],
    ));
    patch.extend(model(
        pod_preset::MOD_SLOT,
        pod_preset::MOD_CONTROL_MODEL_IDS[0],
        &[
            parameter(0x0010_0001, 0.2),
            parameter(0x0010_0002, 0.4),
            parameter(0x0010_0003, 0.6),
        ],
    ));
    patch.extend(model(
        pod_preset::REVERB_SLOT,
        pod_preset::REVERB_CONTROL_MODEL_IDS[0],
        &[parameter(0x0010_0000, 0.3), parameter(0x0010_0002, 0.6)],
    ));
    patch.extend(model(
        pod_preset::DELAY_SLOT,
        pod_preset::DELAY_CONTROL_MODEL_IDS[0],
        &[
            parameter(0x0010_0001, 0.2),
            parameter(0x0010_0002, 0.4),
            parameter(0x0010_0003, 0.6),
        ],
    ));
    let mut data = container.to_vec();
    data.extend(chunk(b"ODD!", &[0x17, 0x28, 0x39]));
    data.extend(chunk(b"PINF", &info));
    data.extend(chunk(b"LIST", &patch));
    chunk(b"FORM", &data)
}

fn sample_stomp_model_tone(model_id: u32) -> Vec<u8> {
    fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut bytes = id.to_vec();
        bytes.extend_from_slice(&(data.len() as u32).to_be_bytes());
        bytes.extend_from_slice(data);
        if !data.len().is_multiple_of(2) {
            bytes.push(0xa5);
        }
        bytes
    }
    let mut info = vec![0; 80];
    info[..4].copy_from_slice(&1u32.to_be_bytes());
    info[4..8].copy_from_slice(&pod_preset::PODXT_LIVE_TONE_ID.to_be_bytes());
    info[8..10].copy_from_slice(&u16::from(b'S').to_be_bytes());
    info[72..76].copy_from_slice(&pod_preset::PODXT_LIVE_HOST_MIN_VERSION.to_be_bytes());
    let mut minf = Vec::new();
    minf.extend_from_slice(&model_id.to_be_bytes());
    minf.extend_from_slice(&pod_preset::STOMP_SLOT.to_be_bytes());
    minf.extend_from_slice(&[0xfe, 0x12, 0x34, 0x01]);
    let mut model = b"MODL".to_vec();
    model.extend(chunk(b"MINF", &minf));
    model.extend(chunk(b"PARM", &[0xde, 0xad, 0xbe]));
    model.extend(chunk(b"XTRA", &[0x55, 0x66]));
    let mut patch = b"PATC".to_vec();
    patch.extend(chunk(b"LIST", &model));
    let mut data = b"L6PA".to_vec();
    data.extend(chunk(b"PINF", &info));
    data.extend(chunk(b"LIST", &patch));
    chunk(b"FORM", &data)
}

fn sample_pod_bundle(tones: &[Vec<u8>]) -> Vec<u8> {
    fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut output = id.to_vec();
        output.extend_from_slice(&(data.len() as u32).to_be_bytes());
        output.extend_from_slice(data);
        if !data.len().is_multiple_of(2) {
            output.push(0xa5);
        }
        output
    }
    let mut root = b"L6CO".to_vec();
    root.extend(chunk(b"JUNK", &[0x71, 0x72, 0x73]));
    for (index, tone) in tones.iter().enumerate() {
        let mut bank = if index == 0 {
            b"BANK".to_vec()
        } else {
            b"FLDR".to_vec()
        };
        let mut info = vec![0; 68];
        info[4..6].copy_from_slice(&u16::from(b'A' + index as u8).to_be_bytes());
        bank.extend(chunk(b"BINF", &info));
        bank.extend(chunk(b"NOTE", &[0x91, 0x92, 0x93]));
        bank.extend_from_slice(tone);
        root.extend(chunk(b"LIST", &bank));
    }
    chunk(b"FORM", &root)
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

    fn pod_dirty(&self) -> bool {
        self.pod_library.any_dirty()
            || self
                .pod_bundle
                .as_ref()
                .is_some_and(pod_preset::PodBundleLibrary::is_dirty)
    }

    fn pod_has_staged_amp_choice(&self) -> bool {
        self.pod_amp_model_pending.is_some()
    }

    fn pod_has_staged_edit(&self) -> bool {
        self.pod_has_staged_amp_choice() || self.pod_eq_reset_pending.is_some()
    }

    fn invalidate_stale_eq_reset_stage(&mut self) {
        let Some(pending) = self.pod_eq_reset_pending.as_ref() else {
            return;
        };
        let current = self
            .pod_library
            .tone(pending.index)
            .and_then(|tone| tone.podxt_eq_reset_context().ok());
        if pending.document_generation != self.pod_document_generation
            || current.as_ref() != Some(&pending.context)
        {
            self.pod_eq_reset_pending = None;
            self.pod_message =
                "Discarded a stale EQ reset because its entry or target EQ changed.".into();
        }
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

    fn undo(&mut self) {
        if !self.is_reading()
            && self.pending.is_none()
            && let Some(document) = &mut self.document
        {
            document.undo();
        }
    }

    fn redo(&mut self) {
        if !self.is_reading()
            && self.pending.is_none()
            && let Some(document) = &mut self.document
        {
            document.redo();
        }
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
                self.change(|p| p.set_enabled(slot, enabled));
            }
        }
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
            self.change(|p| p.select_model(slot, id));
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
        for slot in [PICKUP2, PICKUP1] {
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

    fn pod_quit_dialog(&mut self, ctx: &egui::Context) {
        if self.pod_eq_reset_pending.is_some() {
            self.pod_eq_reset_quit_dialog(ctx);
            return;
        }
        if self
            .pod_bundle
            .as_ref()
            .is_some_and(|bundle| bundle.is_dirty())
        {
            self.pod_bundle_quit_dialog(ctx);
            return;
        }
        let Some(index) = self.pod_library.first_dirty() else {
            if self.pod_has_staged_amp_choice() {
                self.pod_amp_quit_dialog(ctx);
                return;
            }
            if self.hd_library.any_dirty()
                || self.hd_type9_pending.is_some()
                || self.hd_type8_pending.is_some()
                || self.hd_type10_pending.is_some()
                || !self.hd_type11_pending.is_empty()
                || !self.hd_type12_pending.is_empty()
                || !self.hd_type13_pending.is_empty()
                || !self.hd_type19_pending.is_empty()
            {
                self.hd_quit_dialog(ctx);
                return;
            }
            self.continue_pending(ctx);
            return;
        };
        let Some(name) = self
            .pod_library
            .tone(index)
            .map(|tone| tone.name().to_owned())
        else {
            self.pending = None;
            return;
        };
        let remaining = self.pod_library.dirty_count();
        let still = if remaining == 1 {
            "This is the only unsaved tone.".to_owned()
        } else {
            format!(
                "{remaining} tones are unsaved. Saving or discarding this one leaves the others unsaved."
            )
        };
        let mut discard = false;
        let mut save_continue = false;
        let mut cancel = false;
        egui::Window::new("Unsaved PODxt Live tone")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(format!(
                    "\"{name}\" has an applied name edit that has not been saved. {still}"
                ));
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
                    let response = ui.button("Discard this tone");
                    #[cfg(test)]
                    self.pending_buttons.insert("discard", response.rect);
                    if response.clicked() {
                        discard = true;
                    }
                });
            });
        if cancel {
            self.pending = None;
        } else if save_continue {
            if self.save_next_dirty_pod() && !self.pod_library.any_dirty() {
                self.continue_pending(ctx);
            }
        } else if discard && self.discard_next_dirty_pod() && !self.pod_library.any_dirty() {
            self.continue_pending(ctx);
        }
    }

    fn pod_amp_quit_dialog(&mut self, ctx: &egui::Context) {
        let Some((index, origin_id, choice)) = self.pod_amp_model_pending else {
            self.continue_pending(ctx);
            return;
        };
        let current_id = self
            .pod_library
            .tone(index)
            .and_then(pod_preset::PodTone::amp_model_id);
        if current_id != Some(origin_id) {
            self.pod_amp_model_pending = None;
            self.continue_pending(ctx);
            return;
        }

        let mut apply = false;
        let mut discard = false;
        let mut keep_editing = false;
        egui::Window::new("Unapplied PODxt Live AMP choice")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(format!(
                    "AMP {:08x} is staged for tone {} but has not changed its bytes.",
                    choice.model_id(),
                    index + 1
                ));
                ui.horizontal(|ui| {
                    let response = ui.button("Apply and continue…");
                    #[cfg(test)]
                    self.pending_buttons.insert("amp-apply", response.rect);
                    apply = response.clicked();
                    let response = ui.button("Discard AMP choice");
                    #[cfg(test)]
                    self.pending_buttons.insert("amp-discard", response.rect);
                    discard = response.clicked();
                    let response = ui.button("Keep editing");
                    #[cfg(test)]
                    self.pending_buttons
                        .insert("amp-keep-editing", response.rect);
                    keep_editing = response.clicked();
                });
            });

        if apply {
            self.pod_selected = Some(index);
            if self.apply_pod_amp_model_choice(choice, Some(origin_id)) {
                self.pod_amp_model_pending = None;
                self.continue_staged_pod_quit(ctx);
            }
        } else if discard {
            self.pod_amp_model_pending = None;
            self.continue_staged_pod_quit(ctx);
        } else if keep_editing {
            self.pending = None;
        }
    }

    fn pod_eq_reset_quit_dialog(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.pod_eq_reset_pending.clone() else {
            self.continue_staged_pod_quit(ctx);
            return;
        };
        let current_context = self
            .pod_library
            .tone(pending.index)
            .and_then(|tone| tone.podxt_eq_reset_context().ok());
        if pending.document_generation != self.pod_document_generation
            || current_context.as_ref() != Some(&pending.context)
        {
            self.pod_eq_reset_pending = None;
            self.pod_message = "Discarded a stale EQ reset because its target EQ changed.".into();
            self.continue_staged_pod_quit(ctx);
            return;
        }
        let mut apply = false;
        let mut discard = false;
        let mut keep_editing = false;
        egui::Window::new("Unapplied PODxt Live EQ reset")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(format!(
                    "The four-band EQ reset is staged for tone {}. No bytes have changed yet.",
                    pending.index + 1
                ));
                ui.horizontal(|ui| {
                    let response = ui.button("Apply reset and continue…");
                    #[cfg(test)]
                    self.pending_buttons
                        .insert("eq-reset-quit-apply", response.rect);
                    apply = response.clicked();
                    let response = ui.button("Discard EQ reset");
                    #[cfg(test)]
                    self.pending_buttons
                        .insert("eq-reset-quit-discard", response.rect);
                    discard = response.clicked();
                    let response = ui.button("Keep editing");
                    #[cfg(test)]
                    self.pending_buttons
                        .insert("eq-reset-quit-keep", response.rect);
                    keep_editing = response.clicked();
                });
            });
        if apply {
            self.pod_selected = Some(pending.index);
            if self.apply_pod_eq_reset(Some(&pending)) {
                self.pod_eq_reset_pending = None;
                self.continue_staged_pod_quit(ctx);
            }
        } else if discard {
            self.pod_eq_reset_pending = None;
            self.continue_staged_pod_quit(ctx);
        } else if keep_editing {
            self.pending = None;
        }
    }

    fn continue_staged_pod_quit(&mut self, ctx: &egui::Context) {
        if self.pod_has_staged_edit() {
            self.pending = Some(Pending::QuitPod);
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        } else {
            self.continue_pending(ctx);
        }
    }

    fn pod_bundle_quit_dialog(&mut self, ctx: &egui::Context) {
        let mut discard = false;
        let mut save_continue = false;
        let mut cancel = false;
        egui::Window::new("Unsaved POD bank or collection")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label("This bank/collection has offline edits that have not been saved.");
                ui.horizontal(|ui| {
                    let response = ui.button("Save bank copy and continue…");
                    #[cfg(test)]
                    self.pending_buttons.insert("save", response.rect);
                    save_continue = response.clicked();
                    let response = ui.button("Keep editing");
                    #[cfg(test)]
                    self.pending_buttons.insert("cancel", response.rect);
                    cancel = response.clicked();
                    let response = ui.button("Discard bank edits");
                    #[cfg(test)]
                    self.pending_buttons.insert("discard", response.rect);
                    discard = response.clicked();
                });
            });
        if cancel {
            self.pending = None;
        } else if save_continue {
            let index = self.pod_library.first_dirty().unwrap_or(0);
            if self.write_pod_copy(index) && !self.pod_library.any_dirty() {
                self.continue_pending(ctx);
            }
        } else if discard {
            let staged_amp = self.pod_amp_model_pending;
            let result = self
                .pod_bundle
                .as_mut()
                .map_or(Ok(()), |bundle| bundle.revert());
            match result.and_then(|()| self.refresh_pod_library_from_bundle()) {
                Ok(()) => {
                    self.refresh_selected_pod_name();
                    self.restore_staged_amp_if_origin_matches(staged_amp);
                    self.pod_message = "Discarded offline bank edits. Nothing was sent.".into();
                    self.continue_pending(ctx);
                }
                Err(error) => self.pod_message = error.detail(),
            }
        }
    }

    fn save_next_dirty_pod(&mut self) -> bool {
        if let Some(index) = self.pod_library.first_dirty() {
            return self.write_pod_copy(index);
        }
        if let Some(index) = self.hd_library.first_dirty() {
            return self.write_hd_copy(index);
        }
        false
    }

    fn discard_next_dirty_pod(&mut self) -> bool {
        if let Some(bundle) = &mut self.pod_bundle {
            return match bundle
                .revert()
                .and_then(|()| self.refresh_pod_library_from_bundle())
            {
                Ok(()) => {
                    let staged_amp = self.pod_amp_model_pending;
                    self.refresh_selected_pod_name();
                    self.restore_staged_amp_if_origin_matches(staged_amp);
                    self.pod_message = "Discarded unsaved bank edits. Nothing was sent.".into();
                    true
                }
                Err(error) => {
                    self.pod_message = error.detail();
                    false
                }
            };
        }
        if let Some(index) = self.pod_library.first_dirty() {
            return match self.pod_library.revert_unsaved(index) {
                Ok(()) => {
                    let staged_amp = self.pod_amp_model_pending;
                    self.refresh_selected_pod_name();
                    self.restore_staged_amp_if_origin_matches(staged_amp);
                    self.pod_message =
                        "Discarded the unsaved name edit for one tone. Nothing was sent.".into();
                    true
                }
                Err(error) => {
                    self.pod_message = error.detail();
                    false
                }
            };
        }
        let Some(index) = self.hd_library.first_dirty() else {
            return false;
        };
        match self.hd_library.revert_unsaved(index) {
            Ok(()) => {
                if self
                    .hd_type9_pending
                    .is_some_and(|pending| pending.file_index == index)
                {
                    self.hd_type9_pending = None;
                }
                if self
                    .hd_type8_pending
                    .is_some_and(|pending| pending.file_index == index)
                {
                    self.hd_type8_pending = None;
                }
                if self
                    .hd_type10_pending
                    .is_some_and(|pending| pending.file_index == index)
                {
                    self.hd_type10_pending = None;
                }
                self.hd_type11_pending
                    .retain(|(file_index, _, _), _| *file_index != index);
                self.hd_type12_pending
                    .retain(|(file_index, _, _), _| *file_index != index);
                self.hd_type13_pending
                    .retain(|(file_index, _, _), _| *file_index != index);
                self.hd_type19_pending
                    .retain(|(file_index, _, _), _| *file_index != index);
                self.pod_message = "Discarded one unsaved HD Edit file change.".into();
                true
            }
            Err(error) => {
                self.pod_message = error.detail();
                false
            }
        }
    }

    fn restore_staged_amp_if_origin_matches(
        &mut self,
        staged_amp: Option<(usize, u32, pod_preset::PodAmpModelChoice)>,
    ) {
        let Some((index, origin_id, _)) = staged_amp else {
            return;
        };
        if self
            .pod_library
            .tone(index)
            .and_then(pod_preset::PodTone::amp_model_id)
            == Some(origin_id)
        {
            self.pod_amp_model_pending = staged_amp;
        }
    }

    fn refresh_selected_pod_name(&mut self) {
        self.pod_cab_pending = None;
        self.pod_cab_pending_origin = None;
        self.pod_delay_model_pending = None;
        self.pod_mod_model_pending = None;
        self.pod_stomp_model_pending = None;
        self.pod_reverb_model_pending = None;
        self.pod_amp_model_pending = None;
        self.pod_harmony_pending = None;
        self.pod_delay_pending = None;
        self.pod_mod_pending = None;
        match (self.pod_selected, self.pod_library.len()) {
            (Some(_), 0) => {
                self.pod_selected = None;
                self.pod_name.clear();
            }
            (Some(index), len) if index >= len => {
                let selected = len - 1;
                self.pod_selected = Some(selected);
                self.pod_name = self
                    .pod_library
                    .tone(selected)
                    .map_or_else(String::new, |tone| tone.name().to_owned());
            }
            (Some(index), _) => {
                self.pod_name = self
                    .pod_library
                    .tone(index)
                    .map_or_else(String::new, |tone| tone.name().to_owned());
            }
            (None, _) => self.pod_name.clear(),
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
                    && let Some(path) = self.dialogs.save("device-check-report.txt", "txt")
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
        let Some(path) = self.dialogs.save("device-check-report.txt", "txt") else {
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

    fn pod_window(&mut self, ui: &mut egui::Ui) {
        if !self.pod_open {
            return;
        }
        #[cfg(test)]
        {
            self.pod_rows.clear();
            self.pod_cab_combo = None;
            self.pod_cab_options.clear();
            self.pod_practice_combo = None;
            self.pod_practice_options.clear();
            self.pod_drive_slider = None;
            self.pod_stomp_sliders.clear();
            self.pod_whammy_knob_sliders.clear();
            self.pod_eq_frequency_sliders.clear();
            self.pod_eq_gain_sliders.clear();
            self.pod_model_enable_checks.clear();
            self.pod_model_enable_menu = None;
            self.pod_mod_sliders.clear();
            self.pod_reverb_sliders.clear();
            self.pod_delay_sliders.clear();
            self.pod_delay_note_options.clear();
            self.pod_delay_time_slider = None;
            self.pod_mod_note_options.clear();
            self.pod_mod_speed_slider = None;
            self.pod_delay_heads_choices.clear();
            self.pod_delay_bits_choices.clear();
            self.pod_stomp_wave_choices.clear();
            self.pod_harmony1_choices.clear();
            self.pod_harmony2_choices.clear();
            self.pod_stomp_model_combo = None;
            self.pod_stomp_model_options.clear();
            self.pod_reverb_model_combo = None;
            self.pod_reverb_model_options.clear();
            self.pod_amp_model_combo = None;
            self.pod_amp_model_options.clear();
        }
        ui.scope(|ui| {
                #[cfg(test)]
                {
                    self.pod_drive_slider = None;
                }
                if self.pending.is_some() {
                    ui.disable();
                }
                ui.label(pod_preset::OFFLINE_BANNER);
                ui.label(pod_preset::NOT_GUITAR);
                ui.label(pod_preset::NOT_SENT);
                ui.label(pod_preset::PAYLOADS_NOT_DECODED);
                // One line, with the full text on hover. A wrapping status
                // line pushes the HD replacement controls below the window.
                ui.add(egui::Label::new(&self.pod_message).truncate())
                    .on_hover_text(&self.pod_message);
                ui.separator();
                // Keep these controls on their own id stack. Buttons use the
                // parent auto-id counter, and a shift here sends later clicks
                // to the wrong POD or HD control.
                ui.push_id("pod-pedalboard", |ui| {
                    self.pod_pedalboard(ui);
                });
                ui.separator();
                let reverb_model_state = self.pod_selected.and_then(|index| {
                    self.pod_library.tone(index).and_then(|tone| {
                        (tone.container() == pod_preset::ContainerKind::L6pa).then(|| {
                            let ids: Vec<_> = tone.models().iter()
                                .filter(|model| matches!(model.slot, pod_preset::REVERB_SLOT | pod_preset::REVERB_FALLBACK_SLOT))
                                .map(|model| model.model_id).collect();
                            (!ids.is_empty()).then_some((index, ids.first().copied(), ids.len()))
                        }).flatten()
                    })
                });
                if let Some((index, current, slot_count)) = reverb_model_state {
                    if slot_count == 1 && current.and_then(pod_preset::PodReverbModelChoice::from_model_id).is_some() {
                        let mut selected = self.pod_reverb_model_pending.filter(|(i, _)| *i == index).map(|(_, choice)| Some(choice))
                            .or_else(|| current.and_then(pod_preset::PodReverbModelChoice::from_model_id).map(Some)).unwrap_or(None);
                        let selected_text = selected.map(pod_preset::PodReverbModelChoice::numeric_label)
                            .unwrap_or_else(|| "Select model".into());
                        let mut changed = None;
                        let combo = ui.horizontal(|ui| {
                            ui.label(format!("Reverb ID · current {:08x}", current.unwrap()));
                            let combo = egui::ComboBox::from_id_salt("pod-reverb-model-choice").selected_text(selected_text).show_ui(ui, |ui| {
                                for id in pod_preset::PodReverbModelChoice::MODEL_IDS {
                                    let choice = pod_preset::PodReverbModelChoice::from_model_id(id).expect("listed Reverb model id is admitted");
                                    let response = ui.selectable_value(&mut selected, Some(choice), format!("{id:08x}"));
                                    #[cfg(test)] self.pod_reverb_model_options.insert(id, response.rect);
                                    if response.changed() { changed = Some(choice); }
                                }
                            });
                            combo.response.rect
                        });
                        #[cfg(test)] { self.pod_reverb_model_combo = Some(combo.inner); }
                        #[cfg(not(test))] let _ = combo;
                        if let Some(choice) = changed { self.pod_reverb_model_pending = Some((index, choice)); }
                        if let Some((pending_index, choice)) = self.pod_reverb_model_pending.filter(|(i, _)| *i == index) {
                            ui.horizontal(|ui| {
                                ui.label(format!("Pending {} (not applied)", choice.numeric_label()));
                                let apply = ui.button("Apply Reverb model");
                                #[cfg(test)] self.pod_buttons.insert("pod-reverb-model-apply", apply.rect);
                                if apply.clicked() {
                                    match self.apply_pod_reverb_model_choice(pending_index, choice) {
                                        Ok(changed) => {
                                            if changed { self.pod_reverb_model_pending = None; }
                                            self.pod_message = if changed { "Applied the Reverb model ID offline; stored parameters were retained. Nothing was sent.".into() } else { "The Reverb model ID already matched; no bytes or staging changed.".into() };
                                        }
                                        Err(error) => self.pod_message = error.detail(),
                                    }
                                }
                                let discard = ui.button("Discard");
                                #[cfg(test)] self.pod_buttons.insert("pod-reverb-model-discard", discard.rect);
                                if discard.clicked() { self.pod_reverb_model_pending = None; }
                            });
                        }
                    } else {
                        ui.label(format!("Reverb ID · current {} (uneditable: requires one recognized primary or fallback model)", current.map_or_else(|| "unavailable".into(), |id| format!("{id:08x}"))));
                    }
                }
                let membership_selection = self.pod_selected.and_then(|index| {
                    self.pod_bundle.as_ref().and_then(|bundle| {
                        let (bank, preset) = bundle.entry_location(index)?;
                        let folder_editable = bundle.membership_editable(bank);
                        let fixed_bank_editable = bundle.fixed_bank_reorder_editable(bank);
                        (folder_editable || fixed_bank_editable).then_some((
                            bank,
                            preset,
                            bundle.preset_count(bank)?,
                            folder_editable,
                        ))
                    })
                });
                let mut membership_action = None;
                ui.horizontal(|ui| {
                    if self.pod_button(ui, "load-pod", "Load tone or bank…") {
                        self.load_pod_tone();
                    }
                    if let Some(sample) = self.practice_tone_menu(ui) {
                        self.import_pod_document(sample.bytes());
                    }
                    if self.pod_button(ui, "apply-pod-name", "Apply name") {
                        self.apply_pod_name();
                    }
                    if self.pod_button(ui, "save-pod", "Save copy…") {
                        self.save_pod_tone();
                    }
                    if self.pod_button_enabled(ui, "revert-pod", "Revert", self.pod_can_revert()) {
                        self.pod_revert_confirm = true;
                    }
                    if let Some((bank, preset, count, folder_editable)) = membership_selection {
                        if folder_editable {
                            if self.pod_button(ui, "pod-duplicate-entry", "Duplicate") {
                                membership_action =
                                    Some((PodMembershipAction::Duplicate, bank, preset));
                            }
                            if self.pod_button(ui, "pod-remove-entry", "Remove") {
                                membership_action =
                                    Some((PodMembershipAction::Remove, bank, preset));
                            }
                        }
                        if self.pod_button_enabled(
                            ui,
                            "pod-move-entry-up",
                            "Move up",
                            preset > 0,
                        ) {
                            membership_action = Some((PodMembershipAction::MoveUp, bank, preset));
                        }
                        if self.pod_button_enabled(
                            ui,
                            "pod-move-entry-down",
                            "Move down",
                            preset + 1 < count,
                        ) {
                            membership_action =
                                Some((PodMembershipAction::MoveDown, bank, preset));
                        }
                        let slot_kind = if folder_editable { "Folder" } else { "Fixed bank" };
                        ui.label(format!("{slot_kind} slot {} of {count}", preset + 1));
                    }
                    if let Some(index) = self.pod_selected {
                        let _response = ui.menu_button("Model flags…", |ui| {
                            self.model_enable_menu(ui, index);
                        });
                        #[cfg(test)]
                        {
                            self.pod_model_enable_menu = Some(_response.response.rect);
                        }
                    }
                });
                if let Some((action, bank, preset)) = membership_action {
                    self.apply_pod_membership_action(action, bank, preset);
                }
                let rows: Vec<_> = (0..self.pod_library.len())
                    .filter_map(|index| {
                        let tone = self.pod_library.tone(index)?;
                        let bank = self
                            .pod_bundle
                            .as_ref()
                            .and_then(|bundle| {
                                bundle.entry_location(index).and_then(|(bank, preset)| {
                                bundle
                                    .bank_name(bank)
                                    .map(|name| format!("bank {name} preset {preset} · "))
                                })
                            })
                            .unwrap_or_default();
                        Some((
                            index,
                            format!(
                                "{bank}{index} {} {} amp {} stomp {} cab {}",
                                tone.container().tag(),
                                tone.name(),
                                pod_preset::PodTone::model_label(tone.amp_model_id()),
                                pod_preset::PodTone::model_label(tone.stomp_model_id()),
                                pod_preset::PodTone::model_label(tone.cab_model_id()),
                            ),
                            tone.name().to_owned(),
                        ))
                    })
                    .collect();
                for (index, label, _) in rows {
                    let response = ui.selectable_label(self.pod_selected == Some(index), label);
                    #[cfg(test)]
                    self.pod_rows.insert(index, response.rect);
                    if response.clicked() {
                        self.select_pod_index(index);
                    }
                }
                let amp_choice_state = self.pod_selected.and_then(|index| {
                    self.pod_library.tone(index).and_then(|tone| {
                        if tone.container() != pod_preset::ContainerKind::L6pa {
                            return None;
                        }
                        Some((index, tone.amp_model_id()))
                    })
                });
                if let Some((index, current)) = amp_choice_state {
                    let current_id = current.unwrap_or_default();
                    if self
                        .pod_amp_model_pending
                        .is_some_and(|(pending_index, origin_id, _)| {
                            pending_index == index && current != Some(origin_id)
                        })
                    {
                        self.pod_amp_model_pending = None;
                    }
                    let known_current = current == Some(0x0002_0000)
                        || pod_preset::AMP_CONTROL_MODEL_IDS.contains(&current_id);
                    let mut selected = self.pod_amp_model_pending
                        .filter(|(pending_index, origin_id, _)| {
                            *pending_index == index && current == Some(*origin_id)
                        })
                        .map(|(_, _, choice)| Some(choice))
                        .or_else(|| current.and_then(pod_preset::PodAmpModelChoice::from_model_id).map(Some))
                        .unwrap_or(None);
                    let selected_text = selected.map_or_else(
                        || format!("{current_id:08x} (current only)"),
                        |choice| format!("{:08x}", choice.model_id()),
                    );
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!(
                            "AMP model ID (offline; parameters retained, no reset) · current {}",
                            current.map_or_else(|| "unavailable".to_owned(), |id| format!("0x{id:08x}"))
                        ));
                        for (id, choice, label) in [
                            ("pod-amp-no-amp", pod_preset::PodAmpModelChoice::NoAmp, "Apply No Amp · 00020000"),
                            ("pod-amp-line6-clean", pod_preset::PodAmpModelChoice::Line6Clean, "Apply Line 6 Clean · 00020001"),
                        ] {
                            if self.pod_button(ui, id, label) {
                                self.pod_selected = Some(index);
                                if self.apply_pod_amp_model_choice(choice, None) {
                                    self.pod_amp_model_pending = None;
                                }
                            }
                        }
                    });
                    if known_current {
                        let mut changed = None;
                        let combo = egui::ComboBox::from_id_salt("pod-amp-model-choice")
                            .selected_text(selected_text)
                            .show_ui(ui, |ui| {
                                for choice in pod_preset::PODXT_AMP_MODEL_CHOICES {
                                    let id = choice.model_id();
                                    let response = ui.selectable_value(&mut selected, Some(choice), format!("{id:08x}"));
                                    #[cfg(test)] self.pod_amp_model_options.insert(id, response.rect);
                                    if response.changed() { changed = Some(choice); }
                                }
                            });
                        #[cfg(test)] { self.pod_amp_model_combo = Some(combo.response.rect); }
                        #[cfg(not(test))] let _ = combo;
                        if let Some(choice) = changed
                            && let Some(origin_id) = current
                        {
                            self.pod_amp_model_pending = Some((index, origin_id, choice));
                        }
                        if let Some((pending_index, origin_id, choice)) = self.pod_amp_model_pending.filter(|(pending_index, origin_id, _)| *pending_index == index && current == Some(*origin_id)) {
                            ui.horizontal(|ui| {
                                ui.label(format!("Pending AMP {:08x} (not applied)", choice.model_id()));
                                let apply = ui.button("Apply AMP model");
                                #[cfg(test)] self.pod_buttons.insert("pod-amp-model-apply", apply.rect);
                                if apply.clicked() {
                                    self.pod_selected = Some(pending_index);
                                    if self.apply_pod_amp_model_choice(choice, Some(origin_id)) {
                                        self.pod_amp_model_pending = None;
                                    }
                                }
                                let discard = ui.button("Discard");
                                #[cfg(test)] self.pod_buttons.insert("pod-amp-model-discard", discard.rect);
                                if discard.clicked() { self.pod_amp_model_pending = None; }
                            });
                        }
                    }
                }
                let cab_state = self.pod_selected.and_then(|index| self.pod_library.tone(index).and_then(|tone| {
                    (matches!(tone.container(), pod_preset::ContainerKind::L6as | pod_preset::ContainerKind::L6pa)
                        && tone.cab_edit_model_id().is_some())
                        .then_some((index, tone.container(), tone.cab_edit_model_id(), tone.cab_edit_class(), tone.mic_choice(), tone.cab_mic_editable()))
                }));
                if let Some((index, container, cab_id, class, mic, editable)) = cab_state {
                    let fallback = pod_preset::PodCabModelChoice::from_model_id(
                        pod_preset::PODXT_CAB_MODEL_IDS[0],
                    )
                    .expect("source CAB allowlist is nonempty");
                    ui.label(format!("CAB (offline) · current {} · MIC class {}", cab_id.map_or_else(|| "unavailable".into(), |id| format!("{id:08x}")), match class { Some(pod_preset::PodCabClass::Bass) => "bass", Some(pod_preset::PodCabClass::Other) => "standard", None => "unknown" }));
                    if editable {
                    let mut selected_cab = self
                        .pod_cab_pending
                        .filter(|(pending_index, _)| *pending_index == index)
                        .map(|(_, choice)| choice)
                        .or_else(|| cab_id.and_then(pod_preset::PodCabModelChoice::from_model_id))
                        .unwrap_or(fallback);
                    let original_choice = selected_cab;
                    let mut choice_changed = false;
                    ui.horizontal(|ui| {
                        #[cfg(test)]
                        let mut option_rects = Vec::new();
                        let _combo = egui::ComboBox::from_id_salt("pod-cab-choice")
                            .selected_text(selected_cab.numeric_label())
                            .show_ui(ui, |ui| {
                                for id in pod_preset::PODXT_CAB_MODEL_IDS {
                                    if let Some(choice) = pod_preset::PodCabModelChoice::from_model_id(id) {
                                        let _response = ui.selectable_value(&mut selected_cab, choice, format!("CAB {id:08x}"));
                                        choice_changed |= _response.changed();
                                        #[cfg(test)]
                                        option_rects.push((id, _response.rect));
                                    }
                                }
                            });
                        #[cfg(test)]
                        {
                            self.pod_cab_combo = Some(_combo.response.rect);
                            self.pod_cab_options.extend(option_rects);
                        }
                        if choice_changed && selected_cab != original_choice {
                            self.pod_cab_pending = Some((index, selected_cab));
                            self.pod_cab_pending_origin = self.pod_library.tone(index).map(|tone| (
                                self.pod_document_generation,
                                index,
                                tone.as_bytes().to_vec(),
                            ));
                        }
                        if self.pod_cab_pending.is_none_or(|(pending_index, _)| pending_index != index) {
                            self.pod_cab_pending = Some((index, selected_cab));
                        }
                        if self.pod_cab_pending_origin.is_none() {
                            self.pod_cab_pending_origin = self.pod_library.tone(index).map(|tone| (
                                self.pod_document_generation,
                                index,
                                tone.as_bytes().to_vec(),
                            ));
                        }
                        if self.pod_button(ui, "pod-cab-apply", "Apply CAB") {
                            self.pod_selected = Some(index);
                            self.apply_pod_cab_model_choice(selected_cab);
                        }
                        let discard = self.pod_button(ui, "pod-cab-discard", "Discard");
                        if discard {
                            self.pod_cab_pending = None;
                            self.pod_cab_pending_origin = None;
                        }
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!("MIC current {}", mic.map_or_else(|| "unknown".into(), |m| m.value().to_string())));
                        for value in 0..=3 {
                            let id = ["pod-mic-0", "pod-mic-1", "pod-mic-2", "pod-mic-3"][value as usize];
                            if self.pod_button(ui, id, &format!("Apply MIC {value}")) {
                                self.pod_selected = Some(index);
                                if let Some(choice) = pod_preset::PodMicChoice::from_value(value) { self.apply_pod_mic_choice(choice); }
                            }
                        }
                    });
                    } else if container == pod_preset::ContainerKind::L6pa {
                        ui.label("CAB/MIC editing is unavailable because this L6PA tone fails strict offline admission.");
                    }
                }
                ui.horizontal(|ui| {
                    ui.label("PINF name");
                    ui.text_edit_singleline(&mut self.pod_name);
                });
                let controls = self
                    .pod_selected
                    .and_then(|index| self.pod_library.tone(index))
                    .map(|tone| {
                        pod_preset::AmpControl::ALL
                            .into_iter()
                            .filter_map(|control| {
                                tone.amp_control_fraction(control)
                                    .ok()
                                    .map(|value| (control, value))
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if !controls.is_empty() {
                    ui.small(pod_preset::AMP_CONTROL_SCALE_LABEL);
                }
                for (control, mut value) in controls {
                    ui.horizontal(|ui| {
                        ui.label(control.label());
                        let response = ui
                            .add(egui::Slider::new(&mut value, 0.0..=1.0).show_value(true))
                            .on_hover_text(pod_preset::AMP_CONTROL_SCALE_LABEL);
                        #[cfg(test)]
                        if control == pod_preset::AmpControl::Drive {
                            self.pod_drive_slider = Some(response.rect);
                        }
                        if response.changed()
                            && let Some(index) = self.pod_selected
                        {
                            match self.edit_pod_tone(index, |tone| {
                                tone.set_amp_control_fraction(control, value)
                            }) {
                                Ok(()) => {
                                    self.pod_message = format!(
                                        "Applied Amp {} host-scale value {value}. Nothing was sent.",
                                        control.label()
                                    )
                                }
                                Err(error) => self.pod_message = error.detail(),
                            }
                        }
                    });
                }
                let stomp_controls = self
                    .pod_selected
                    .and_then(|index| self.pod_library.tone(index))
                    .map(|tone| {
                        pod_preset::StompControl::ALL
                            .into_iter()
                            .filter_map(|control| {
                                tone.stomp_control_fraction(control)
                                    .ok()
                                    .map(|value| (control, value))
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if !stomp_controls.is_empty() {
                    ui.small(pod_preset::STOMP_CONTROL_SCALE_LABEL);
                }
                for (control, mut value) in stomp_controls {
                    ui.horizontal(|ui| {
                        ui.label(format!("Stomp {}", control.label()));
                        let response = ui
                            .add(egui::Slider::new(&mut value, 0.0..=1.0).show_value(true))
                            .on_hover_text(pod_preset::STOMP_CONTROL_SCALE_LABEL);
                        #[cfg(test)]
                        {
                            self.pod_stomp_sliders.insert(control, response.rect);
                        }
                        if response.changed()
                            && let Some(index) = self.pod_selected
                        {
                            match self.edit_pod_tone(index, |tone| {
                                tone.set_stomp_control_fraction(control, value)
                            }) {
                                Ok(()) => {
                                    self.pod_message = format!(
                                        "Applied Stomp {} host-scale value {value}. Nothing was sent.",
                                        control.label()
                                    )
                                }
                                Err(error) => self.pod_message = error.detail(),
                            }
                        }
                    });
                }
                let whammy_knob_positions = self
                    .pod_selected
                    .and_then(|index| self.pod_library.tone(index).map(|tone| (index, tone)))
                    .map(|(index, tone)| {
                        pod_preset::StompWhammyKnob::ALL
                            .into_iter()
                            .filter_map(|control| {
                                tone.stomp_whammy_knob_position(control)
                                    .ok()
                                    .map(|position| (index, control, position))
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if !whammy_knob_positions.is_empty() {
                    ui.small(
                        "Line6Edit normalized knob position 0–1; source-quantized, not device-validated",
                    );
                }
                for (index, control, mut position) in whammy_knob_positions {
                    ui.horizontal(|ui| {
                        ui.label(format!("Stomp Whammy {} knob position", control.label()));
                        let response = ui
                            .add(
                                egui::Slider::new(&mut position, 0.0..=1.0).show_value(true),
                            )
                            .on_hover_text(
                                "Normalized Line6Edit knob position; source quantization is applied. No physical units or device acceptance are claimed.",
                            );
                        #[cfg(test)]
                        self.pod_whammy_knob_sliders
                            .insert(control, response.rect);
                        if response.changed() {
                            match self.edit_pod_tone(index, |tone| {
                                tone.set_stomp_whammy_knob_position(control, position)
                            }) {
                                Ok(()) => {
                                    self.pod_message = format!(
                                        "Applied Stomp Whammy {} knob position. Source quantization applied. Nothing was sent.",
                                        control.label()
                                    )
                                }
                                Err(error) => self.pod_message = error.detail(),
                            }
                        }
                    });
                }
                let eq_bands = self
                    .pod_selected
                    .and_then(|index| self.pod_library.tone(index).map(|tone| (index, tone)))
                    .map(|(index, tone)| {
                        pod_preset::PodxtEqBand::ALL
                            .into_iter()
                            .filter_map(|band| {
                                tone.podxt_eq_band_display_values(band)
                                    .ok()
                                    .map(|(frequency, gain)| (index, band, frequency, gain))
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if !eq_bands.is_empty() {
                    ui.small("PODxt four-band EQ graph values from Line6Edit 3.06; offline edit only. No device acceptance or audio behavior is claimed.");
                }
                for (index, band, mut frequency, mut gain) in eq_bands {
                    let table = match band {
                        pod_preset::PodxtEqBand::One => (50.0, 690.0),
                        pod_preset::PodxtEqBand::Two => (50.0, 6050.0),
                        pod_preset::PodxtEqBand::Three => (100.0, 11300.0),
                        pod_preset::PodxtEqBand::Four => (500.0, 9300.0),
                    };
                    ui.label(format!("EQ band {}", match band { pod_preset::PodxtEqBand::One => 1, pod_preset::PodxtEqBand::Two => 2, pod_preset::PodxtEqBand::Three => 3, pod_preset::PodxtEqBand::Four => 4 }));
                    ui.horizontal(|ui| {
                        let frequency_response = ui.add(egui::Slider::new(&mut frequency, table.0..=table.1).text("Frequency").suffix(" Hz"));
                        #[cfg(test)]
                        self.pod_eq_frequency_sliders.insert(band, frequency_response.rect);
                        let gain_response = ui.add(egui::Slider::new(&mut gain, -12.8..=12.6).text("Gain").suffix(" dB"));
                        #[cfg(test)]
                        self.pod_eq_gain_sliders.insert(band, gain_response.rect);
                        if frequency_response.changed() || gain_response.changed() {
                            match self.edit_pod_tone(index, |tone| tone.set_podxt_eq_band(band, frequency, gain)) {
                                Ok(()) => self.pod_message = format!("Applied EQ band {} graph edit. Nothing was sent.", match band { pod_preset::PodxtEqBand::One => 1, pod_preset::PodxtEqBand::Two => 2, pod_preset::PodxtEqBand::Three => 3, pod_preset::PodxtEqBand::Four => 4 }),
                                Err(error) => self.pod_message = error.detail(),
                            }
                        }
                    });
                }
                let eq_reset_target = self.pod_selected.and_then(|index| {
                    self.pod_library
                        .tone(index)
                        .and_then(|tone| tone.podxt_eq_reset_context().ok())
                        .map(|context| (index, context))
                });
                if let Some((index, context)) = eq_reset_target {
                    if let Some(pending) = self.pod_eq_reset_pending.clone() {
                        if pending.document_generation == self.pod_document_generation
                            && pending.index == index
                            && pending.context == context
                        {
                            ui.horizontal(|ui| {
                                ui.label("Four-band EQ reset staged · offline only");
                                if self.pod_button(ui, "pod-eq-reset-apply", "Apply reset")
                                    && self.apply_pod_eq_reset(Some(&pending))
                                {
                                    self.pod_eq_reset_pending = None;
                                }
                                if self.pod_button(ui, "pod-eq-reset-discard", "Discard") {
                                    self.pod_eq_reset_pending = None;
                                    self.pod_message = "Discarded the staged EQ reset. No bytes changed.".into();
                                }
                            });
                        } else {
                            ui.label(format!(
                                "EQ reset staged for tone {}. Select it to apply or discard.",
                                pending.index + 1
                            ));
                        }
                    } else if self.pod_button(
                        ui,
                        "pod-eq-reset-stage",
                        "Stage four-band EQ reset",
                    ) {
                        self.pod_eq_reset_pending = Some(PendingPodEqReset {
                            document_generation: self.pod_document_generation,
                            index,
                            context,
                        });
                        self.pod_message = "Staged the four-band EQ reset. No bytes changed; nothing will be sent.".into();
                    }
                }
                if let Some(pending) = self.pod_eq_reset_pending.as_ref()
                    && self.pod_selected != Some(pending.index)
                {
                    ui.small(format!(
                        "An EQ reset is staged for tone {}. Select that tone to apply or discard it.",
                        pending.index + 1
                    ));
                }
                let stomp_wave = self
                    .pod_selected
                    .and_then(|index| self.pod_library.tone(index))
                    .and_then(|tone| tone.stomp_wave_selection().ok());
                if let Some(current) = stomp_wave {
                    let previous = match current {
                        pod_preset::StompWaveValue::Known(choice) => Some(choice),
                        pod_preset::StompWaveValue::Unknown { .. } => None,
                    };
                    let mut selected = previous;
                    let selected_text = match current {
                        pod_preset::StompWaveValue::Known(choice) => choice.label().to_owned(),
                        pod_preset::StompWaveValue::Unknown { raw_bits } => {
                            format!("Unknown float 0x{raw_bits:08x}")
                        }
                    };
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Stomp Wave");
                        if matches!(current, pod_preset::StompWaveValue::Unknown { .. }) {
                            ui.small(selected_text);
                        }
                        for choice in pod_preset::StompWaveSelection::ALL {
                            let _response =
                                ui.selectable_value(&mut selected, Some(choice), choice.label());
                            #[cfg(test)]
                            self.pod_stomp_wave_choices.insert(choice, _response.rect);
                        }
                    });
                    if selected != previous
                        && let (Some(index), Some(choice)) = (self.pod_selected, selected)
                    {
                        match self.edit_pod_tone(index, |tone| {
                            tone.set_stomp_wave_selection(choice)
                        }) {
                            Ok(()) => {
                                self.pod_message = format!(
                                    "Applied Stomp Wave {} selection. Nothing was sent.",
                                    choice.label()
                                )
                            }
                            Err(error) => self.pod_message = error.detail(),
                        }
                    }
                }
                let harmony1 = self.pod_selected.and_then(|index| self.pod_library.tone(index))
                    .and_then(|tone| tone.stomp_harmony_1m335_selection().ok());
                if let Some(current) = harmony1 {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Stomp Harmony (-1/Maj3/5)");
                        match current { pod_preset::StompHarmonyValue::Known(choice) => { ui.small(choice.label()); }, pod_preset::StompHarmonyValue::Unknown { raw_bits } => { ui.small(format!("Unknown float 0x{raw_bits:08x}")); } }
                        for choice in pod_preset::StompHarmony1M335Selection::ALL {
                            let previous = match self.pod_harmony_pending { Some((index, pod_preset::StompHarmonyControl::Harmony1M335, PendingStompHarmony::Harmony1M335(choice))) if Some(index) == self.pod_selected => Some(choice), _ => match current { pod_preset::StompHarmonyValue::Known(choice) => Some(choice), _ => None } };
                            let mut selected = previous;
                            let _response = ui.selectable_value(&mut selected, Some(choice), choice.label());
                            #[cfg(test)] self.pod_harmony1_choices.insert(choice, _response.rect);
                            if selected != previous && let Some(index) = self.pod_selected { self.pod_harmony_pending = Some((index, pod_preset::StompHarmonyControl::Harmony1M335, PendingStompHarmony::Harmony1M335(choice))); }
                        }
                    });
                    if let Some((index, pod_preset::StompHarmonyControl::Harmony1M335, PendingStompHarmony::Harmony1M335(choice))) = self.pod_harmony_pending.filter(|(index, _, _)| Some(*index) == self.pod_selected) {
                        ui.label(format!("Pending {} (not applied)", choice.label()));
                        if self.pod_button(ui, "pod-harmony1-apply", "Apply Harmony 1") {
                            match self.apply_pod_stomp_harmony(index, pod_preset::StompHarmonySelection::Harmony1M335(choice)) {
                                Ok(()) => { self.pod_harmony_pending = None; self.pod_message = format!("Applied Stomp HARMONY_1M335 {}. Nothing was sent.", choice.label()); }
                                Err(error) => self.pod_message = error.detail(),
                            }
                        }
                    }
                }
                let harmony2 = self.pod_selected.and_then(|index| self.pod_library.tone(index))
                    .and_then(|tone| tone.stomp_harmony_1457_selection().ok());
                if let Some(current) = harmony2 {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Stomp Harmony (-1/2/4/7)");
                        match current { pod_preset::StompHarmonyValue::Known(choice) => { ui.small(choice.label()); }, pod_preset::StompHarmonyValue::Unknown { raw_bits } => { ui.small(format!("Unknown float 0x{raw_bits:08x}")); } }
                        for choice in pod_preset::StompHarmony1457Selection::ALL {
                            let previous = match self.pod_harmony_pending { Some((index, pod_preset::StompHarmonyControl::Harmony1457, PendingStompHarmony::Harmony1457(choice))) if Some(index) == self.pod_selected => Some(choice), _ => match current { pod_preset::StompHarmonyValue::Known(choice) => Some(choice), _ => None } };
                            let mut selected = previous;
                            let _response = ui.selectable_value(&mut selected, Some(choice), choice.label());
                            #[cfg(test)] self.pod_harmony2_choices.insert(choice, _response.rect);
                            if selected != previous && let Some(index) = self.pod_selected { self.pod_harmony_pending = Some((index, pod_preset::StompHarmonyControl::Harmony1457, PendingStompHarmony::Harmony1457(choice))); }
                        }
                    });
                    if let Some((index, pod_preset::StompHarmonyControl::Harmony1457, PendingStompHarmony::Harmony1457(choice))) = self.pod_harmony_pending.filter(|(index, _, _)| Some(*index) == self.pod_selected) {
                        ui.label(format!("Pending {} (not applied)", choice.label()));
                        if self.pod_button(ui, "pod-harmony2-apply", "Apply Harmony 2") {
                            match self.apply_pod_stomp_harmony(index, pod_preset::StompHarmonySelection::Harmony1457(choice)) {
                                Ok(()) => { self.pod_harmony_pending = None; self.pod_message = format!("Applied Stomp HARMONY_1457 {}. Nothing was sent.", choice.label()); }
                                Err(error) => self.pod_message = error.detail(),
                            }
                        }
                    }
                }
                let mod_controls = self
                    .pod_selected
                    .and_then(|index| self.pod_library.tone(index))
                    .map(|tone| {
                        pod_preset::ModControl::ALL
                            .into_iter()
                            .filter_map(|control| {
                                tone.mod_control_fraction(control)
                                    .ok()
                                    .map(|value| (control, value))
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if !mod_controls.is_empty() {
                    ui.small(pod_preset::MOD_CONTROL_SCALE_LABEL);
                }
                for (control, mut value) in mod_controls {
                    ui.horizontal(|ui| {
                        ui.label(format!("Mod {}", control.label()));
                        let response = ui
                            .add(egui::Slider::new(&mut value, 0.0..=1.0).show_value(true))
                            .on_hover_text(pod_preset::MOD_CONTROL_SCALE_LABEL);
                        #[cfg(test)]
                        {
                            self.pod_mod_sliders.insert(control, response.rect);
                        }
                        if response.changed()
                            && let Some(index) = self.pod_selected
                        {
                            match self.edit_pod_tone(index, |tone| {
                                tone.set_mod_control_fraction(control, value)
                            }) {
                                Ok(()) => {
                                    self.pod_message = format!(
                                        "Applied Mod {} host-scale value {value}. Nothing was sent.",
                                        control.label()
                                    )
                                }
                                Err(error) => self.pod_message = error.detail(),
                            }
                        }
                    });
                }
                let reverb_controls = self
                    .pod_selected
                    .and_then(|index| self.pod_library.tone(index))
                    .map(|tone| {
                        pod_preset::ReverbControl::ALL
                            .into_iter()
                            .filter_map(|control| {
                                tone.reverb_control_fraction(control)
                                    .ok()
                                    .map(|value| (control, value))
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if !reverb_controls.is_empty() {
                    ui.small(pod_preset::REVERB_CONTROL_SCALE_LABEL);
                }
                for (control, mut value) in reverb_controls {
                    ui.horizontal(|ui| {
                        ui.label(format!("Reverb {}", control.label()));
                        let response = ui
                            .add(egui::Slider::new(&mut value, 0.0..=1.0).show_value(true))
                            .on_hover_text(pod_preset::REVERB_CONTROL_SCALE_LABEL);
                        #[cfg(test)]
                        {
                            self.pod_reverb_sliders.insert(control, response.rect);
                        }
                        if response.changed()
                            && let Some(index) = self.pod_selected
                        {
                            match self.edit_pod_tone(index, |tone| {
                                tone.set_reverb_control_fraction(control, value)
                            }) {
                                Ok(()) => {
                                    self.pod_message = format!(
                                        "Applied Reverb {} host-scale value {value}. Nothing was sent.",
                                        control.label()
                                    )
                                }
                                Err(error) => self.pod_message = error.detail(),
                            }
                        }
                    });
                }
                let delay_controls = self
                    .pod_selected
                    .and_then(|index| self.pod_library.tone(index))
                    .map(|tone| {
                        pod_preset::DelayControl::ALL
                            .into_iter()
                            .filter_map(|control| {
                                tone.delay_control_fraction(control)
                                    .ok()
                                    .map(|value| (control, value))
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if !delay_controls.is_empty() {
                    ui.small(pod_preset::DELAY_CONTROL_SCALE_LABEL);
                }
                for (control, mut value) in delay_controls {
                    ui.horizontal(|ui| {
                        ui.label(format!("Delay {}", control.label()));
                        let response = ui
                            .add(egui::Slider::new(&mut value, 0.0..=1.0).show_value(true))
                            .on_hover_text(pod_preset::DELAY_CONTROL_SCALE_LABEL);
                        #[cfg(test)]
                        {
                            self.pod_delay_sliders.insert(control, response.rect);
                        }
                        if response.changed()
                            && let Some(index) = self.pod_selected
                        {
                            match self.edit_pod_tone(index, |tone| {
                                tone.set_delay_control_fraction(control, value)
                            }) {
                                Ok(()) => {
                                    self.pod_message = format!(
                                        "Applied Delay {} host-scale value {value}. Nothing was sent.",
                                        control.label()
                                    )
                                }
                                Err(error) => self.pod_message = error.detail(),
                            }
                        }
                    });
                }
                let delay_note_state = self.pod_selected.and_then(|index| {
                    self.pod_library.tone(index).map(|tone| {
                        (index, tone.delay_note_value(), tone.delay_time_ms())
                    })
                });
                let delay_model_state = self.pod_selected.and_then(|index| {
                    let tone = self.pod_library.tone(index)?;
                    (tone.container() == pod_preset::ContainerKind::L6pa)
                        .then(|| (index, tone.editable_delay_model_id()))
                });
                let stomp_model_state = self.pod_selected.and_then(|index| {
                    self.pod_library.tone(index).and_then(|tone| {
                        (tone.container() == pod_preset::ContainerKind::L6pa).then(|| {
                            let matches: Vec<_> = tone.models().iter()
                                .filter(|model| model.slot == pod_preset::STOMP_SLOT)
                                .map(|model| model.model_id)
                                .collect();
                            (index, matches.first().copied(), matches.len())
                        })
                    })
                });
                if let Some((index, current, slot_count)) = stomp_model_state {
                    let mut selected = self.pod_stomp_model_pending
                        .filter(|(pending_index, _)| *pending_index == index)
                        .map(|(_, choice)| Some(choice))
                        .or_else(|| current.and_then(pod_preset::PodStompModelChoice::from_model_id).map(Some))
                        .unwrap_or(None);
                    let selected_text = selected
                        .map(pod_preset::PodStompModelChoice::numeric_label)
                        .unwrap_or_else(|| current.map_or_else(|| "Select model".into(), |id| format!("{id:08x} (current only)")));
                    ui.separator();
                    let current_text = current.map_or_else(|| "unavailable".into(), |id| {
                        if pod_preset::STOMP_CONTROL_MODEL_IDS.contains(&id) {
                            format!("{id:08x}{}", if pod_preset::PodStompModelChoice::from_model_id(id).is_some() { "" } else { " (Pack 4 gated; preserved)" })
                        } else { format!("{id:08x} (unknown; preserved)" ) }
                    });
                    ui.label(format!("Stomp model ID (offline; parameters retained, no reset) · current {current_text}"));
                    if slot_count == 1 {
                        let mut changed = None;
                        let combo = egui::ComboBox::from_id_salt("pod-stomp-model-choice")
                            .selected_text(selected_text)
                            .show_ui(ui, |ui| {
                                for id in pod_preset::PodStompModelChoice::MODEL_IDS {
                                    let choice = pod_preset::PodStompModelChoice::from_model_id(id)
                                        .expect("listed Stomp model id is admitted");
                                    let response = ui.selectable_value(&mut selected, Some(choice), format!("{id:08x}"));
                                    #[cfg(test)] self.pod_stomp_model_options.insert(id, response.rect);
                                    if response.changed() { changed = Some(choice); }
                                }
                            });
                        #[cfg(test)] { self.pod_stomp_model_combo = Some(combo.response.rect); }
                        #[cfg(not(test))] let _ = combo;
                        if let Some(choice) = changed { self.pod_stomp_model_pending = Some((index, choice)); }
                        if let Some((pending_index, choice)) = self.pod_stomp_model_pending.filter(|(pending_index, _)| *pending_index == index) {
                            ui.horizontal(|ui| {
                                ui.label(format!("Pending {} (not applied)", choice.numeric_label()));
                                let apply = ui.button("Apply Stomp model");
                                #[cfg(test)] self.pod_buttons.insert("pod-stomp-model-apply", apply.rect);
                                if apply.clicked() {
                                    match self.apply_pod_stomp_model_choice(pending_index, choice) {
                                        Ok(changed) => {
                                            if changed { self.pod_stomp_model_pending = None; }
                                            self.pod_message = if changed { "Applied the Stomp model ID offline; stored parameters were retained. Nothing was sent.".into() } else { "The Stomp model ID already matched; no bytes or staging changed.".into() };
                                        }
                                        Err(error) => self.pod_message = error.detail(),
                                    }
                                }
                                let discard = ui.button("Discard");
                                #[cfg(test)] self.pod_buttons.insert("pod-stomp-model-discard", discard.rect);
                                if discard.clicked() { self.pod_stomp_model_pending = None; }
                            });
                        }
                    } else {
                        ui.small("Stomp model selection requires exactly one existing Stomp slot; no slot will be created.");
                    }
                }
                if let Some((index, Ok(current))) = delay_model_state {
                    let mut selected = self.pod_delay_model_pending
                        .filter(|(i, _)| *i == index)
                        .map(|(_, choice)| choice)
                        .or_else(|| pod_preset::PodDelayModelChoice::from_model_id(current));
                    let selected_text = selected
                        .map(pod_preset::PodDelayModelChoice::numeric_label)
                        .unwrap_or_else(|| format!("{current:08x} (outside choices)"));
                    ui.separator();
                    ui.label(format!("Delay model ID (offline; stored parameters are retained, no reset) · current {current:08x}"));
                    let mut changed = None;
                    let _combo = egui::ComboBox::from_id_salt("pod-delay-model-choice")
                        .selected_text(selected_text)
                        .show_ui(ui, |ui| {
                            for id in pod_preset::PodDelayModelChoice::MODEL_IDS {
                                let choice = pod_preset::PodDelayModelChoice::from_model_id(id)
                                    .expect("listed Delay model id is admitted");
                                let response = ui.selectable_value(&mut selected, Some(choice), format!("{id:08x}"));
                                #[cfg(test)] { self.pod_delay_model_options.insert(id, response.rect); }
                                if response.changed() { changed = Some(choice); }
                            }
                        });
                    #[cfg(test)] { self.pod_delay_model_combo = Some(_combo.response.rect); }
                    if let Some(choice) = changed { self.pod_delay_model_pending = Some((index, choice)); }
                    if let Some((pending_index, choice)) = self.pod_delay_model_pending.filter(|(i, _)| *i == index) {
                        ui.horizontal(|ui| {
                            ui.label(format!("Pending {} (parameters retained)", choice.numeric_label()));
                            let response = ui.button("Apply Delay model");
                            #[cfg(test)] { self.pod_buttons.insert("pod-delay-model-apply", response.rect); }
                            if response.clicked() {
                                match self.apply_pod_delay_model_choice(pending_index, choice) {
                                    Ok(()) => { self.pod_delay_model_pending = None; self.pod_message = "Applied the Delay model ID offline; stored parameters were retained. Nothing was sent.".into(); }
                                    Err(error) => self.pod_message = error.detail(),
                                }
                            }
                            let discard = ui.button("Discard");
                            #[cfg(test)] { self.pod_buttons.insert("pod-delay-model-discard", discard.rect); }
                            if discard.clicked() { self.pod_delay_model_pending = None; }
                        });
                    }
                } else if let Some((_, Err(error))) = delay_model_state {
                    ui.separator();
                    ui.label(format!("Delay model unavailable. {}", error.detail()));
                }
                if let Some((index, note_result, time_result)) = delay_note_state {
                    ui.separator();
                    ui.label("Delay NOTE / TIME");
                    match note_result {
                        Ok(value) => {
                            let selected_current = match value {
                                pod_preset::DelayNoteValue::Known(choice) => Some(choice),
                                pod_preset::DelayNoteValue::Unknown { .. } => None,
                            };
                            let selected_pending = match self.pod_delay_pending {
                                Some((pending_index, PendingDelayEdit::Note(choice))) if pending_index == index => Some(choice),
                                _ => selected_current,
                            };
                            let selected_label = match value {
                                pod_preset::DelayNoteValue::Known(choice) => choice.label().to_owned(),
                                pod_preset::DelayNoteValue::Unknown { raw_bits } => format!("Unknown NOTE {raw_bits}"),
                            };
                            ui.label(format!("Stored: {selected_label}"));
                            let mut selected = selected_pending;
                            let mut changed = None;
                            ui.horizontal_wrapped(|ui| {
                                for choice in pod_preset::DelayNoteChoice::ALL {
                                    let response = ui.selectable_value(&mut selected, Some(choice), choice.label());
                                    #[cfg(test)]
                                    self.pod_delay_note_options.insert(choice, response.rect);
                                    if response.changed() { changed = selected; }
                                }
                            });
                            if let Some(choice) = changed {
                                self.pod_delay_pending = Some((index, PendingDelayEdit::Note(choice)));
                            }
                        }
                        Err(error) => { ui.small(format!("Delay NOTE unavailable: {}", error.detail())); }
                    }
                    match time_result {
                        Ok(current_ms) => {
                            let pending_ms = match self.pod_delay_pending {
                                Some((pending_index, PendingDelayEdit::TimeMs(value))) if pending_index == index => value,
                                _ => current_ms,
                            };
                            let mut value = pending_ms;
                            ui.horizontal(|ui| {
                                ui.label("TIME (ms)");
                                let response = ui.add(egui::Slider::new(&mut value, 20.0..=2000.0).show_value(true));
                                #[cfg(test)] { self.pod_delay_time_slider = Some(response.rect); }
                                if response.changed() {
                                    self.pod_delay_pending = Some((index, PendingDelayEdit::TimeMs(value)));
                                }
                            });
                        }
                        Err(error) => { ui.small(format!("Delay TIME unavailable: {}", error.detail())); }
                    }
                    if let Some((pending_index, edit)) = self.pod_delay_pending.filter(|(pending_index, _)| *pending_index == index) {
                        let (label, button) = match edit {
                            PendingDelayEdit::Note(choice) => (format!("{} (not applied)", choice.label()), "Apply Delay NOTE"),
                            PendingDelayEdit::TimeMs(value) => (format!("{value:.3} ms (not applied)"), "Apply Delay TIME"),
                        };
                        ui.horizontal(|ui| {
                            ui.label(format!("Pending {label}"));
                            let response = ui.button(button);
                            #[cfg(test)] { self.pod_buttons.insert("pod-delay-apply", response.rect); }
                            if response.clicked() {
                                match self.apply_pod_delay_edit(pending_index, edit) {
                                    Ok(()) => {
                                        self.pod_delay_pending = None;
                                        self.pod_message = "Applied the staged Delay NOTE/TIME edit offline. Nothing was sent.".into();
                                    }
                                    Err(error) => self.pod_message = error.detail(),
                                }
                            }
                            let discard = ui.button("Discard");
                            #[cfg(test)]
                            self.pod_buttons
                                .insert("pod-delay-discard", discard.rect);
                            if discard.clicked() { self.pod_delay_pending = None; }
                        });
                    }
                }
                let mod_model_state = self.pod_selected.and_then(|index| {
                    let tone = self.pod_library.tone(index)?;
                    (tone.container() == pod_preset::ContainerKind::L6pa)
                        .then(|| (index, tone.editable_mod_model_id()))
                });
                if let Some((index, Ok(current))) = mod_model_state {
                    let mut selected = self
                        .pod_mod_model_pending
                        .filter(|(pending_index, _)| *pending_index == index)
                        .map(|(_, choice)| choice)
                        .or_else(|| pod_preset::PodModModelChoice::from_model_id(current));
                    let selected_text = selected
                        .map(pod_preset::PodModModelChoice::numeric_label)
                        .unwrap_or_else(|| format!("{current:08x} (outside choices)"));
                    ui.separator();
                    ui.label(format!(
                        "Mod model ID (offline; parameters retained, no reset) · current {current:08x}"
                    ));
                    let mut changed = None;
                    let combo = egui::ComboBox::from_id_salt("pod-mod-model-choice")
                        .selected_text(selected_text)
                        .show_ui(ui, |ui| {
                            for id in pod_preset::PodModModelChoice::MODEL_IDS {
                                let choice = pod_preset::PodModModelChoice::from_model_id(id)
                                    .expect("listed Mod model id is admitted");
                                let response = ui.selectable_value(
                                    &mut selected,
                                    Some(choice),
                                    format!("{id:08x}"),
                                );
                                #[cfg(test)]
                                self.pod_mod_model_options.insert(id, response.rect);
                                if response.changed() {
                                    changed = Some(choice);
                                }
                            }
                        });
                    #[cfg(test)]
                    {
                        self.pod_mod_model_combo = Some(combo.response.rect);
                    }
                    #[cfg(not(test))]
                    let _ = combo;
                    if let Some(choice) = changed {
                        self.pod_mod_model_pending = Some((index, choice));
                    }
                    if let Some((pending_index, choice)) = self
                        .pod_mod_model_pending
                        .filter(|(pending_index, _)| *pending_index == index)
                    {
                        ui.horizontal(|ui| {
                            ui.label(format!(
                                "Pending {} (parameters retained)",
                                choice.numeric_label()
                            ));
                            let apply = ui.button("Apply Mod model");
                            #[cfg(test)]
                            self.pod_buttons.insert("pod-mod-model-apply", apply.rect);
                            if apply.clicked() {
                                match self.apply_pod_mod_model_choice(pending_index, choice) {
                                    Ok(()) => {
                                        self.pod_mod_model_pending = None;
                                        self.pod_message = "Applied the Mod model ID offline; stored parameters were retained. Nothing was sent.".into();
                                    }
                                    Err(error) => self.pod_message = error.detail(),
                                }
                            }
                            let discard = ui.button("Discard");
                            #[cfg(test)]
                            self.pod_buttons.insert("pod-mod-model-discard", discard.rect);
                            if discard.clicked() {
                                self.pod_mod_model_pending = None;
                            }
                        });
                    }
                } else if let Some((_, Err(error))) = mod_model_state {
                    ui.separator();
                    ui.label(format!("Mod model unavailable. {}", error.detail()));
                }
                let mod_state = self.pod_selected.and_then(|index| {
                    self.pod_library.tone(index).map(|tone| {
                        (index, tone.mod_note_value(), tone.mod_speed_hz())
                    })
                });
                if let Some((index, note_result, speed_result)) = mod_state {
                    ui.separator();
                    ui.label("Mod NOTE / SPEED");
                    match note_result {
                        Ok(value) => {
                            let current = match value {
                                pod_preset::ModNoteValue::Known(choice) => Some(choice),
                                pod_preset::ModNoteValue::Unknown { .. } => None,
                            };
                            let pending = match self.pod_mod_pending {
                                Some((pending_index, PendingModEdit::Note(choice))) if pending_index == index => Some(choice),
                                _ => current,
                            };
                            let label = match value {
                                pod_preset::ModNoteValue::Known(choice) => choice.label().to_owned(),
                                pod_preset::ModNoteValue::Unknown { raw_bits } => format!("Unknown NOTE {raw_bits}"),
                            };
                            ui.label(format!("Stored: {label}"));
                            let mut selected = pending;
                            let mut changed = None;
                            ui.horizontal_wrapped(|ui| {
                                for choice in pod_preset::DelayNoteChoice::ALL {
                                    let response = ui.selectable_value(&mut selected, Some(choice), choice.label());
                                    #[cfg(test)]
                                    self.pod_mod_note_options.insert(choice, response.rect);
                                    if response.changed() { changed = selected; }
                                }
                            });
                            if let Some(choice) = changed {
                                self.pod_mod_pending = Some((index, PendingModEdit::Note(choice)));
                            }
                        }
                        Err(error) => { ui.small(format!("Mod NOTE unavailable: {}", error.detail())); }
                    }
                    match speed_result {
                        Ok(current_hz) => {
                            let pending_hz = match self.pod_mod_pending {
                                Some((pending_index, PendingModEdit::SpeedHz(value))) if pending_index == index => value,
                                _ => current_hz,
                            };
                            let mut value = pending_hz;
                            ui.horizontal(|ui| {
                                ui.label("SPEED (Hz)");
                                let response = ui.add(egui::Slider::new(&mut value, 0.1..=15.0).show_value(true));
                                #[cfg(test)] { self.pod_mod_speed_slider = Some(response.rect); }
                                if response.changed() {
                                    self.pod_mod_pending = Some((index, PendingModEdit::SpeedHz(value)));
                                }
                            });
                        }
                        Err(error) => { ui.small(format!("Mod SPEED unavailable: {}", error.detail())); }
                    }
                    if let Some((pending_index, edit)) = self.pod_mod_pending.filter(|(pending_index, _)| *pending_index == index) {
                        let (label, button) = match edit {
                            PendingModEdit::Note(choice) => (format!("{} (not applied)", choice.label()), "Apply Mod NOTE"),
                            PendingModEdit::SpeedHz(value) => (format!("{value:.3} Hz (not applied)"), "Apply Mod SPEED"),
                        };
                        ui.horizontal(|ui| {
                            ui.label(format!("Pending {label}"));
                            let apply = ui.button(button);
                            #[cfg(test)]
                            self.pod_buttons.insert("pod-mod-apply", apply.rect);
                            if apply.clicked() {
                                match self.apply_pod_mod_edit(pending_index, edit) {
                                    Ok(()) => {
                                        self.pod_mod_pending = None;
                                        self.pod_message = "Applied the staged Mod NOTE/SPEED edit offline. Nothing was sent.".into();
                                    }
                                    Err(error) => self.pod_message = error.detail(),
                                }
                            }
                            let discard = ui.button("Discard");
                            #[cfg(test)]
                            self.pod_buttons.insert("pod-mod-discard", discard.rect);
                            if discard.clicked() { self.pod_mod_pending = None; }
                        });
                    }
                }
                let delay_heads = self
                    .pod_selected
                    .and_then(|index| self.pod_library.tone(index))
                    .and_then(|tone| tone.delay_heads_selection().ok());
                if let Some(current) = delay_heads {
                    let previous = match current {
                        pod_preset::DelayAssociationValue::Known(choice) => Some(choice),
                        pod_preset::DelayAssociationValue::Unknown { .. } => None,
                    };
                    let mut selected = previous;
                    let selected_text = match current {
                        pod_preset::DelayAssociationValue::Known(choice) => {
                            choice.label().to_owned()
                        }
                        pod_preset::DelayAssociationValue::Unknown { raw_bits } => {
                            format!("Unknown float 0x{raw_bits:08x}")
                        }
                    };
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Delay Heads");
                        if matches!(current, pod_preset::DelayAssociationValue::Unknown { .. }) {
                            ui.small(selected_text);
                        }
                        for choice in pod_preset::DelayHeadsSelection::ALL {
                            let _choice_response =
                                ui.selectable_value(&mut selected, Some(choice), choice.label());
                            #[cfg(test)]
                            self.pod_delay_heads_choices
                                .insert(choice, _choice_response.rect);
                        }
                    });
                    if selected != previous
                        && let (Some(index), Some(choice)) = (self.pod_selected, selected)
                    {
                        match self.edit_pod_tone(index, |tone| {
                            tone.set_delay_heads_selection(choice)
                        }) {
                            Ok(()) => {
                                self.pod_message = format!(
                                    "Applied Delay Heads {} selection. Nothing was sent.",
                                    choice.label()
                                )
                            }
                            Err(error) => self.pod_message = error.detail(),
                        }
                    }
                }
                let delay_bits = self
                    .pod_selected
                    .and_then(|index| self.pod_library.tone(index))
                    .and_then(|tone| tone.delay_bits_selection().ok());
                if let Some(current) = delay_bits {
                    let previous = match current {
                        pod_preset::DelayAssociationValue::Known(choice) => Some(choice),
                        pod_preset::DelayAssociationValue::Unknown { .. } => None,
                    };
                    let mut selected = previous;
                    let selected_text = match current {
                        pod_preset::DelayAssociationValue::Known(choice) => {
                            choice.label().to_owned()
                        }
                        pod_preset::DelayAssociationValue::Unknown { raw_bits } => {
                            format!("Unknown float 0x{raw_bits:08x}")
                        }
                    };
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Delay Bits");
                        if matches!(current, pod_preset::DelayAssociationValue::Unknown { .. }) {
                            ui.small(selected_text);
                        }
                        for choice in pod_preset::DelayBitsSelection::ALL {
                            let _choice_response =
                                ui.selectable_value(&mut selected, Some(choice), choice.label());
                            #[cfg(test)]
                            self.pod_delay_bits_choices.insert(choice, _choice_response.rect);
                        }
                    });
                    if selected != previous
                        && let (Some(index), Some(choice)) = (self.pod_selected, selected)
                    {
                        match self.edit_pod_tone(index, |tone| {
                            tone.set_delay_bits_selection(choice)
                        }) {
                            Ok(()) => {
                                self.pod_message = format!(
                                    "Applied Delay Bits {} selection. Nothing was sent.",
                                    choice.label()
                                )
                            }
                            Err(error) => self.pod_message = error.detail(),
                        }
                    }
                }
                let detail = self.pod_selected.and_then(|index| {
                    self.pod_library.tone(index).map(|tone| {
                        format!(
                            "Selected {} version {:08x}. {} slots have no GPXT role. {} parameter records have no known range. {} parameter chunks stay opaque.\n{}",
                            tone.container().tag(),
                            tone.device_version(),
                            tone.undecoded_slots(),
                            tone.recorded_parameter_chunks(),
                            tone.undecoded_parameter_chunks(),
                            tone.control_lines().join("\n"),
                        )
                    })
                });
                if let Some(detail) = detail {
                    ui.label(detail);
                }
                #[cfg(test)]
                {
                    let selected = self.pod_selected.and_then(|index| {
                        self.pod_library.tone(index).map(|tone| {
                            format!(
                                "\n{} {} amp {} cab {} undecoded {}\n{}",
                                tone.container().tag(),
                                tone.name(),
                                pod_preset::PodTone::model_label(tone.amp_model_id()),
                                pod_preset::PodTone::model_label(tone.cab_model_id()),
                                tone.undecoded_slots(),
                                tone.control_lines().join("\n"),
                            )
                        })
                    });
                    self.pod_visible = format!(
                        "{}\n{}\n{}\n{}\n{}\n{}",
                        pod_preset::OFFLINE_BANNER,
                        pod_preset::NOT_GUITAR,
                        pod_preset::NOT_SENT,
                        pod_preset::PAYLOADS_NOT_DECODED,
                        pod_preset::HD_EDITOR_OFFLINE_NOTICE,
                        self.pod_message,
                    );
                    if let Some(selected) = selected {
                        self.pod_visible.push_str(&selected);
                    }
                    let scope = self.pod_revert_scope();
                    self.pod_visible.push('\n');
                    self.pod_visible.push_str(&scope);
                    self.pod_visible.push_str(
                        "\nFetch needs gate G5. Sending a bank needs the reviewed write and store workflow.",
                    );
                }

                if self.pod_button(ui, "load-hd-file", "Open HD Edit file…") {
                    self.load_hd_editor_file();
                }
                let selected_enable = self.hd_selected.and_then(|index| {
                    let slot = pod_preset::HdFxSlot::new(self.hd_effect_slot).ok()?;
                    let PodPresetFile::H5ep(preset) = self.hd_library.file(index)? else {
                        return None;
                    };
                    Some((index, slot, preset.effect_enabled(slot)?))
                });
                if let Some((index, slot, enabled)) = selected_enable {
                    let (button_id, label) = if enabled {
                        ("disable-hd-effect", "Disable effect offline")
                    } else {
                        ("enable-hd-effect", "Enable effect offline")
                    };
                    if self.pod_button(ui, button_id, label) {
                        let result = Session::extension(ProductProfile::PodHd500x).and_then(
                            |session| {
                                session.set_hd_effect_enabled(
                                    &mut self.hd_library,
                                    index,
                                    slot,
                                    !enabled,
                                )
                            },
                        );
                        match result {
                            Ok(()) => {
                                self.pod_message = format!(
                                    "Effect {} updated in this offline preset. Nothing was sent; save a new copy to keep the change.",
                                    if enabled { "disabled" } else { "enabled" }
                                );
                            }
                            Err(error) => self.pod_message = rejection_text(&error),
                        }
                    }
                }
                if let Some(pending) = self.hd_type9_pending
                    && self.hd_selected == Some(pending.file_index)
                    && self.hd_effect_slot == pending.slot.index()
                {
                    ui.label(format!(
                        "{} for FX slot {} is staged.",
                        pending.choice.label(),
                        pending.slot.index()
                    ));
                    ui.horizontal(|ui| {
                        if self.pod_button(
                            ui,
                            "apply-hd-type9-choice",
                            "Apply offline choice",
                        ) {
                            self.apply_hd_type9_choice(
                                pending.file_index,
                                pending.slot,
                                pending.model_id,
                                pending.choice,
                            );
                        }
                        if self.pod_button(
                            ui,
                            "discard-hd-type9-choice",
                            "Discard staged choice",
                        ) {
                            self.hd_type9_pending = None;
                            self.pod_message = "Discarded the staged HD type-9 choice.".into();
                        }
                    });
                }
                if let Some(pending) = self.hd_type8_pending
                    && self.hd_selected == Some(pending.file_index)
                    && self.hd_effect_slot == pending.slot.index()
                {
                    ui.label(format!(
                        "{} for FX slot {} is staged.",
                        pending.choice.label(),
                        pending.slot.index()
                    ));
                    ui.horizontal(|ui| {
                        if self.pod_button(
                            ui,
                            "apply-hd-type8-choice",
                            "Apply offline choice",
                        ) {
                            self.apply_hd_type8_choice(
                                pending.file_index,
                                pending.slot,
                                pending.model_id,
                                pending.choice,
                            );
                        }
                        if self.pod_button(
                            ui,
                            "discard-hd-type8-choice",
                            "Discard staged choice",
                        ) {
                            self.hd_type8_pending = None;
                            self.pod_message = "Discarded the staged HD type-8 choice.".into();
                        }
                    });
                }
                if let Some(pending) = self.hd_type10_pending
                    && self.hd_selected == Some(pending.file_index)
                    && self.hd_effect_slot == pending.slot.index()
                {
                    ui.label(format!("{} for FX slot {} is staged.", pending.choice.label(), pending.slot.index()));
                    ui.horizontal(|ui| {
                        if self.pod_button(ui, "apply-hd-type10-choice", "Apply offline choice") { self.apply_hd_type10_choice(pending.file_index,pending.slot,pending.model_id,pending.choice); }
                        if self.pod_button(ui, "discard-hd-type10-choice", "Discard staged choice") { self.hd_type10_pending=None; self.pod_message="Discarded the staged HD type-10 choice.".into(); }
                    });
                }
                if self.hd_selected.is_some_and(|index| {
                    self.hd_library
                        .file(index)
                        .is_some_and(|file| file.hd_container_kind().is_some())
                }) && self.pod_button(ui, "save-hd-copy", "Save HD file copy…")
                {
                    self.write_hd_copy(self.hd_selected.expect("selected HD file"));
                }

                ui.separator();
                let scroll = egui::ScrollArea::vertical()
                    .max_height(560.0)
                    .id_salt("pod-hd-editor-panel")
                    .show(ui, |ui| self.hd_editor_panel(ui));
                #[cfg(test)]
                {
                    self.pod_scroll_area =
                        Some((scroll.id, scroll.inner_rect, scroll.content_size.y));
                }
                #[cfg(not(test))]
                let _ = scroll;
            });
    }

    fn pod_bank_count(&self) -> usize {
        if let Some(bundle) = &self.pod_bundle {
            bundle.bank_count()
        } else if !self.pod_library.is_empty() {
            1
        } else {
            0
        }
    }

    fn pod_channel_count(&self, bank: usize) -> usize {
        if let Some(bundle) = &self.pod_bundle {
            bundle.preset_count(bank).unwrap_or(0)
        } else if bank == 0 {
            self.pod_library.len()
        } else {
            0
        }
    }

    fn pod_flat_index(&self, bank: usize, channel: usize) -> Option<usize> {
        if let Some(bundle) = &self.pod_bundle {
            if channel >= bundle.preset_count(bank)? {
                return None;
            }
            let mut index = 0;
            for earlier in 0..bank {
                index += bundle.preset_count(earlier)?;
            }
            Some(index + channel)
        } else if bank == 0 && channel < self.pod_library.len() {
            Some(channel)
        } else {
            None
        }
    }

    fn pod_selected_channel(&self) -> Option<usize> {
        let index = self.pod_selected?;
        if let Some(bundle) = &self.pod_bundle {
            bundle.entry_location(index).map(|(_, preset)| preset)
        } else {
            Some(index)
        }
    }

    fn select_pod_index(&mut self, index: usize) {
        if self.pod_selected != Some(index) {
            self.pod_cab_pending = None;
            self.pod_delay_model_pending = None;
            self.pod_mod_model_pending = None;
            self.pod_stomp_model_pending = None;
            self.pod_reverb_model_pending = None;
            self.pod_amp_model_pending = None;
            self.pod_harmony_pending = None;
            self.pod_delay_pending = None;
            self.pod_mod_pending = None;
        }
        self.pod_selected = Some(index);
        if let Some(tone) = self.pod_library.tone(index) {
            self.pod_name = tone.name().to_owned();
        }
        if let Some((bank, _)) = self
            .pod_bundle
            .as_ref()
            .and_then(|bundle| bundle.entry_location(index))
        {
            self.pod_bank = bank;
        } else {
            self.pod_bank = 0;
        }
    }

    fn show_pod_channel(&mut self, bank: usize, channel: usize) {
        let Some(index) = self.pod_flat_index(bank, channel) else {
            return;
        };
        self.select_pod_index(index);
    }

    fn pod_has_staged_choice(&self) -> bool {
        self.pod_has_staged_edit()
            || self.pod_cab_pending.is_some()
            || self.pod_delay_model_pending.is_some()
            || self.pod_mod_model_pending.is_some()
            || self.pod_stomp_model_pending.is_some()
            || self.pod_reverb_model_pending.is_some()
            || self.pod_harmony_pending.is_some()
            || self.pod_delay_pending.is_some()
            || self.pod_mod_pending.is_some()
    }

    fn pod_can_revert(&self) -> bool {
        self.pod_dirty() || self.pod_has_staged_choice()
    }

    fn pod_revert_scope(&self) -> String {
        let stored = if self
            .pod_bundle
            .as_ref()
            .is_some_and(pod_preset::PodBundleLibrary::is_dirty)
        {
            let banks = self
                .pod_bundle
                .as_ref()
                .map(pod_preset::PodBundleLibrary::bank_count)
                .unwrap_or(0);
            format!("Revert restores all {banks} loaded banks to the imported copy.")
        } else if self.pod_library.any_dirty() {
            format!(
                "Revert restores {} unsaved tone(s). Other tones stay as they are.",
                self.pod_library.dirty_count()
            )
        } else {
            "No stored edits to revert.".to_owned()
        };
        let staged = if self.pod_has_staged_choice() {
            " Staged model choices that are not applied yet are dropped."
        } else {
            ""
        };
        format!("{stored}{staged} Save copy… writes a new file and leaves the original untouched.")
    }

    fn clear_pod_stages(&mut self) {
        self.pod_cab_pending = None;
        self.pod_cab_pending_origin = None;
        self.pod_delay_model_pending = None;
        self.pod_mod_model_pending = None;
        self.pod_stomp_model_pending = None;
        self.pod_reverb_model_pending = None;
        self.pod_amp_model_pending = None;
        self.pod_eq_reset_pending = None;
        self.pod_harmony_pending = None;
        self.pod_delay_pending = None;
        self.pod_mod_pending = None;
    }

    fn revert_pod_edits(&mut self) {
        self.pod_revert_confirm = false;
        let staged_amp = self.pod_amp_model_pending;
        if self.pod_bundle.is_some() {
            let result = self
                .pod_bundle
                .as_mut()
                .map_or(Ok(()), pod_preset::PodBundleLibrary::revert);
            if let Err(error) = result.and_then(|()| self.refresh_pod_library_from_bundle()) {
                self.pod_message = error.detail();
                return;
            }
        } else {
            let dirty: Vec<_> = (0..self.pod_library.len())
                .filter(|index| self.pod_library.is_dirty(*index))
                .collect();
            for index in dirty {
                if let Err(error) = self.pod_library.revert_unsaved(index) {
                    self.pod_message = error.detail();
                    return;
                }
            }
        }
        self.clear_pod_stages();
        self.refresh_selected_pod_name();
        self.restore_staged_amp_if_origin_matches(staged_amp);
        if self.pod_amp_model_pending.is_some() {
            self.pod_message =
                "Reverted stored edits. The staged Amp choice still matches this tone and was kept."
                    .into();
        } else {
            self.pod_message = "Reverted unsaved PODxt Live edits. Nothing was sent.".into();
        }
    }

    fn pod_block_state_label(&self, block: PodBlock) -> String {
        let Some(index) = self.pod_selected else {
            return format!("{} · no tone", block.label());
        };
        let Some(tone) = self.pod_library.tone(index) else {
            return format!("{} · no tone", block.label());
        };
        match tone.model_enable_state(block.enable()) {
            Ok(pod_preset::ModelEnableState::Canonical(true)) => {
                format!("{} · On", block.label())
            }
            Ok(pod_preset::ModelEnableState::Canonical(false)) => {
                format!("{} · Bypassed", block.label())
            }
            Ok(pod_preset::ModelEnableState::NonCanonical { raw, .. }) => {
                format!("{} · stored 0x{raw:02x}", block.label())
            }
            Err(_) => format!("{} · no model", block.label()),
        }
    }

    fn pod_block_model_line(&self, block: PodBlock) -> String {
        let Some(index) = self.pod_selected else {
            return "Load a tone to see this block.".to_owned();
        };
        let Some(tone) = self.pod_library.tone(index) else {
            return "The selected tone is no longer in the bank.".to_owned();
        };
        let model_id = match block {
            PodBlock::Amp => tone.amp_model_id(),
            PodBlock::Stomp => tone.stomp_model_id(),
            PodBlock::Mod => tone.mod_model_id(),
            PodBlock::Delay => tone.delay_model_id(),
        };
        let Some(model_id) = model_id else {
            return format!(
                "This tone has no {} model record. No model or parameters were added.",
                block.label()
            );
        };
        let pending = match block {
            PodBlock::Amp => self
                .pod_amp_model_pending
                .filter(|(pending_index, _, _)| *pending_index == index)
                .map(|(_, _, choice)| choice.model_id()),
            PodBlock::Stomp => self
                .pod_stomp_model_pending
                .filter(|(pending_index, _)| *pending_index == index)
                .map(|(_, choice)| choice.model_id()),
            PodBlock::Mod => self
                .pod_mod_model_pending
                .filter(|(pending_index, _)| *pending_index == index)
                .map(|(_, choice)| choice.model_id()),
            PodBlock::Delay => self
                .pod_delay_model_pending
                .filter(|(pending_index, _)| *pending_index == index)
                .map(|(_, choice)| choice.model_id()),
        };
        let staged = pending.map_or(String::new(), |id| {
            format!(" Staged model {id:08x} is not applied yet.")
        });
        format!("Model {model_id:08x}.{staged}")
    }

    fn pod_block_controls_line(&self, block: PodBlock) -> String {
        let Some(tone) = self
            .pod_selected
            .and_then(|index| self.pod_library.tone(index))
        else {
            return String::new();
        };
        let labels: Vec<&str> = match block {
            PodBlock::Amp => pod_preset::AmpControl::ALL
                .into_iter()
                .filter(|control| tone.amp_control_fraction(*control).is_ok())
                .map(|control| control.label())
                .collect(),
            PodBlock::Stomp => pod_preset::StompControl::ALL
                .into_iter()
                .filter(|control| tone.stomp_control_fraction(*control).is_ok())
                .map(|control| control.label())
                .collect(),
            PodBlock::Mod => pod_preset::ModControl::ALL
                .into_iter()
                .filter(|control| tone.mod_control_fraction(*control).is_ok())
                .map(|control| control.label())
                .collect(),
            PodBlock::Delay => pod_preset::DelayControl::ALL
                .into_iter()
                .filter(|control| tone.delay_control_fraction(*control).is_ok())
                .map(|control| control.label())
                .collect(),
        };
        if labels.is_empty() {
            format!(
                "No supported {} controls are stored in this tone.",
                block.label()
            )
        } else {
            format!("Controls in this tone: {}.", labels.join(", "))
        }
    }

    fn toggle_pod_block_bypass(&mut self) {
        let Some(index) = self.pod_selected else {
            self.pod_message = "Select a channel before changing On or Bypass.".into();
            return;
        };
        let block = self.pod_block;
        let control = block.enable();
        let state = self
            .pod_library
            .tone(index)
            .ok_or_else(|| pod_preset::ToneError::format("POD selection is out of range"))
            .and_then(|tone| tone.model_enable_state(control));
        let Ok(pod_preset::ModelEnableState::Canonical(enabled)) = state else {
            self.pod_message = format!(
                "{} On/Bypass stays unchanged. {}",
                block.label(),
                self.pod_block_model_line(block)
            );
            return;
        };
        match self.edit_pod_tone(index, |tone| tone.set_model_enabled(control, !enabled)) {
            Ok(()) => {
                self.pod_message = format!(
                    "{} is now {}. The model, its settings, and the other blocks were kept. Nothing was sent.",
                    block.label(),
                    if enabled { "bypassed" } else { "on" }
                );
            }
            Err(error) => self.pod_message = error.detail(),
        }
    }

    fn pod_pedalboard(&mut self, ui: &mut egui::Ui) {
        if let Some(index) = self.pod_selected
            && let Some((bank, _)) = self
                .pod_bundle
                .as_ref()
                .and_then(|bundle| bundle.entry_location(index))
        {
            self.pod_bank = bank;
        }
        let banks = self.pod_bank_count();
        if self.pod_bank >= banks {
            self.pod_bank = banks.saturating_sub(1);
        }
        let bank_name = self
            .pod_bundle
            .as_ref()
            .and_then(|bundle| bundle.bank_name(self.pod_bank))
            .unwrap_or("");
        let bank_label = if banks == 0 {
            "No bank loaded".to_owned()
        } else if self.pod_bundle.is_none() {
            "Current tone".to_owned()
        } else if bank_name.is_empty() {
            format!("Bank {} of {banks}", self.pod_bank + 1)
        } else {
            format!("Bank {} of {banks}: {bank_name}", self.pod_bank + 1)
        };
        ui.horizontal(|ui| {
            if self.pod_button_enabled(ui, "pod-bank-down", "Bank Down", self.pod_bank > 0) {
                let channel = self.pod_selected_channel().unwrap_or(0);
                let count = self.pod_channel_count(self.pod_bank - 1);
                let channel = channel.min(count.saturating_sub(1));
                self.show_pod_channel(self.pod_bank - 1, channel);
            }
            ui.label(bank_label);
            if self.pod_button_enabled(
                ui,
                "pod-bank-up",
                "Bank Up",
                banks > 0 && self.pod_bank + 1 < banks,
            ) {
                let channel = self.pod_selected_channel().unwrap_or(0);
                let count = self.pod_channel_count(self.pod_bank + 1);
                let channel = channel.min(count.saturating_sub(1));
                self.show_pod_channel(self.pod_bank + 1, channel);
            }
        });
        let channels = self.pod_channel_count(self.pod_bank);
        let selected_channel = self.pod_selected_channel();
        ui.horizontal(|ui| {
            for channel in 0..4 {
                let letter = ["A", "B", "C", "D"][channel];
                let available = channel < channels;
                let selected = available && selected_channel == Some(channel);
                let response =
                    ui.add_enabled(available, egui::Button::new(letter).selected(selected));
                #[cfg(test)]
                self.pod_buttons.insert(
                    match channel {
                        0 => "pod-channel-a",
                        1 => "pod-channel-b",
                        2 => "pod-channel-c",
                        _ => "pod-channel-d",
                    },
                    response.rect,
                );
                if response.clicked() {
                    self.show_pod_channel(self.pod_bank, channel);
                }
            }
            if let Some(index) = self.pod_selected
                && let Some(tone) = self.pod_library.tone(index)
            {
                let letter = selected_channel
                    .filter(|channel| *channel < 4)
                    .map(|channel| ["A", "B", "C", "D"][channel])
                    .unwrap_or("—");
                let dirty = if self.pod_library.is_dirty(index) {
                    " · unsaved changes"
                } else {
                    ""
                };
                ui.label(format!("Channel {letter} · {}{dirty}", tone.name()));
            } else {
                ui.label("No channel selected");
            }
        });
        ui.horizontal_wrapped(|ui| {
            for block in PodBlock::ALL {
                let selected = self.pod_block == block;
                let response =
                    ui.add(egui::Button::new(self.pod_block_state_label(block)).selected(selected));
                #[cfg(test)]
                self.pod_buttons.insert(
                    match block {
                        PodBlock::Amp => "pod-block-amp",
                        PodBlock::Stomp => "pod-block-stomp",
                        PodBlock::Mod => "pod-block-mod",
                        PodBlock::Delay => "pod-block-delay",
                    },
                    response.rect,
                );
                if response.clicked() {
                    self.pod_block = block;
                }
            }
            let can_toggle = self.pod_selected.is_some_and(|index| {
                self.pod_library.tone(index).is_some_and(|tone| {
                    matches!(
                        tone.model_enable_state(self.pod_block.enable()),
                        Ok(pod_preset::ModelEnableState::Canonical(_))
                    )
                })
            });
            if self.pod_button_enabled(ui, "pod-block-bypass", "On / Bypass", can_toggle) {
                self.toggle_pod_block_bypass();
            }
        });
        ui.small(self.pod_block_model_line(self.pod_block));
        ui.small(self.pod_block_controls_line(self.pod_block));
        let enable_note = self.pod_selected.and_then(|index| {
            self.pod_library.tone(index).and_then(|tone| {
                match tone.model_enable_state(self.pod_block.enable()) {
                    Ok(pod_preset::ModelEnableState::Canonical(_)) => None,
                    Ok(pod_preset::ModelEnableState::NonCanonical {
                        raw,
                        source_interpretation,
                    }) => Some(format!(
                        "On/Bypass is unavailable. Stored flag 0x{raw:02x} reads as {} and was left unchanged.",
                        if source_interpretation { "on" } else { "bypassed" }
                    )),
                    Err(error) => Some(format!("On/Bypass is unavailable. {}", error.detail())),
                }
            })
        });
        if let Some(note) = enable_note {
            ui.small(note);
        }
        if self.pod_revert_confirm {
            ui.small(self.pod_revert_scope());
            ui.horizontal(|ui| {
                ui.label("Discard those edits?");
                if self.pod_button(ui, "revert-pod-confirm", "Discard those edits") {
                    self.revert_pod_edits();
                }
                if self.pod_button(ui, "revert-pod-cancel", "Cancel") {
                    self.pod_revert_confirm = false;
                }
            });
        }
        ui.horizontal(|ui| {
            let _fetch = self.pod_button_enabled(ui, "fetch-pod", "Fetch from PODxt Live", false);
            let _send_bank = self.pod_button_enabled(ui, "send-bank", "Send current bank", false);
            let _send_all = self.pod_button_enabled(ui, "send-all", "Send all banks", false);
            ui.small(
                "Fetch needs gate G5. Sending a bank needs the reviewed write and store workflow.",
            );
        });
    }

    fn pod_button(&mut self, ui: &mut egui::Ui, id: &'static str, label: &str) -> bool {
        let response = ui.button(label);
        #[cfg(test)]
        self.pod_buttons.insert(id, response.rect);
        #[cfg(not(test))]
        let _ = id;
        response.clicked()
    }

    fn practice_tone_menu(&mut self, ui: &mut egui::Ui) -> Option<PracticeTone> {
        let mut chosen = None;
        #[cfg(test)]
        let mut option_rects = Vec::new();
        let combo = egui::ComboBox::from_id_salt("pod-practice")
            .selected_text("Practice")
            .show_ui(ui, |ui| {
                for sample in PracticeTone::ALL {
                    let response = ui.selectable_label(false, sample.label());
                    if response.clicked() {
                        chosen = Some(sample);
                    }
                    #[cfg(test)]
                    option_rects.push((sample, response.rect));
                }
            });
        #[cfg(test)]
        {
            self.pod_practice_combo = Some(combo.response.rect);
            self.pod_practice_options.extend(option_rects);
        }
        #[cfg(not(test))]
        let _ = combo;
        chosen
    }

    fn pod_button_enabled(
        &mut self,
        ui: &mut egui::Ui,
        id: &'static str,
        label: &str,
        enabled: bool,
    ) -> bool {
        let response = ui.add_enabled(enabled, egui::Button::new(label));
        #[cfg(test)]
        self.pod_buttons.insert(id, response.rect);
        #[cfg(not(test))]
        let _ = id;
        response.clicked()
    }

    fn model_enable_menu(&mut self, ui: &mut egui::Ui, index: usize) {
        ui.label("Stored model flags only; no bypass, audio, or device effect is claimed.");
        for control in [
            pod_preset::PodModelEnable::Amp,
            pod_preset::PodModelEnable::Eq,
        ] {
            let state = self
                .pod_library
                .tone(index)
                .ok_or_else(|| pod_preset::ToneError::format("POD selection is out of range"))
                .and_then(|tone| tone.model_enable_state(control));
            match state {
                Ok(pod_preset::ModelEnableState::Canonical(mut enabled)) => {
                    let label = format!("{} enabled (stored flag)", control.label());
                    let response = ui.checkbox(&mut enabled, label);
                    #[cfg(test)]
                    self.pod_model_enable_checks.insert(control, response.rect);
                    if response.changed() {
                        match self
                            .edit_pod_tone(index, |tone| tone.set_model_enabled(control, enabled))
                        {
                            Ok(()) => {
                                self.pod_message = format!(
                                    "Applied {} stored enable flag. Nothing was sent.",
                                    control.label()
                                )
                            }
                            Err(error) => self.pod_message = error.detail(),
                        }
                    }
                }
                Ok(pod_preset::ModelEnableState::NonCanonical {
                    raw,
                    source_interpretation,
                }) => {
                    ui.label(format!(
                        "{} edit unavailable: MINF byte 0x{raw:02x} is noncanonical (Line6Edit reads it as {}). Preserved unchanged.",
                        control.label(),
                        if source_interpretation { "enabled" } else { "disabled" }
                    ));
                }
                Err(error) => {
                    ui.label(format!(
                        "{} edit unavailable: {}",
                        control.label(),
                        error.detail()
                    ));
                }
            }
        }
    }

    fn edit_pod_tone(
        &mut self,
        index: usize,
        edit: impl FnOnce(&mut pod_preset::PodTone) -> Result<(), pod_preset::ToneError>,
    ) -> Result<(), pod_preset::ToneError> {
        if let Some(bundle) = &mut self.pod_bundle {
            bundle.edit_entry(index, edit)?;
            let (bank, preset) = bundle
                .entry_location(index)
                .ok_or_else(|| pod_preset::ToneError::format("POD selection is out of range"))?;
            let bytes = bundle.tone(bank, preset)?.as_bytes().to_vec();
            self.pod_library.replace(index, bytes)
        } else {
            let mut tone =
                self.pod_library.tone(index).cloned().ok_or_else(|| {
                    pod_preset::ToneError::format("POD selection is out of range")
                })?;
            edit(&mut tone)?;
            self.pod_library.replace(index, tone.into_bytes())
        }
    }

    fn replace_pod_tone(
        &mut self,
        index: usize,
        bytes: Vec<u8>,
    ) -> Result<(), pod_preset::ToneError> {
        let old_eq_context = self
            .pod_library
            .tone(index)
            .and_then(|tone| tone.podxt_eq_reset_context().ok());
        let old_amp_id = self
            .pod_library
            .tone(index)
            .and_then(pod_preset::PodTone::amp_model_id);
        let mut library = self.pod_library.clone();
        library.replace(index, bytes.clone())?;
        let new_amp_id = library
            .tone(index)
            .and_then(pod_preset::PodTone::amp_model_id);
        let mut bundle = self.pod_bundle.clone();
        if let Some(bundle) = &mut bundle {
            bundle.replace_entry(index, bytes)?;
        }
        self.pod_library = library;
        self.pod_bundle = bundle;
        self.pod_document_generation = self.pod_document_generation.wrapping_add(1);
        if self
            .pod_cab_pending
            .is_some_and(|(pending_index, _)| pending_index == index)
        {
            self.pod_cab_pending = None;
            self.pod_cab_pending_origin = None;
        }
        let new_eq_context = self
            .pod_library
            .tone(index)
            .and_then(|tone| tone.podxt_eq_reset_context().ok());
        if old_eq_context != new_eq_context
            && self
                .pod_eq_reset_pending
                .as_ref()
                .is_some_and(|pending| pending.index == index)
        {
            self.pod_eq_reset_pending = None;
            self.pod_message = "Discarded a staged EQ reset because its target EQ changed.".into();
        }
        if old_amp_id != new_amp_id
            && self
                .pod_amp_model_pending
                .is_some_and(|(pending_index, _, _)| pending_index == index)
        {
            self.pod_amp_model_pending = None;
        }
        Ok(())
    }

    fn apply_pod_eq_reset(&mut self, expected: Option<&PendingPodEqReset>) -> bool {
        let Some(index) = self.pod_selected else {
            self.pod_message = "Select a tone before applying an EQ reset.".into();
            return false;
        };
        let Some(current) = self
            .pod_library
            .tone(index)
            .map(|tone| tone.as_bytes().to_vec())
        else {
            self.pod_message = "Select a tone before applying an EQ reset.".into();
            return false;
        };
        let current_context = self
            .pod_library
            .tone(index)
            .and_then(|tone| tone.podxt_eq_reset_context().ok());
        if expected.is_some_and(|pending| {
            pending.document_generation != self.pod_document_generation
                || pending.index != index
                || current_context.as_deref() != Some(pending.context.as_slice())
        }) {
            self.pod_eq_reset_pending = None;
            self.pod_message = "Discarded a stale EQ reset because its target EQ changed.".into();
            return false;
        }
        let replacement = match Session::initial_rig().reset_pod_eq(RoleKind::Pod, &current) {
            Ok(bytes) => bytes,
            Err(error) => {
                self.pod_message = rejection_text(&error);
                return false;
            }
        };
        match self.replace_pod_tone(index, replacement) {
            Ok(()) => {
                self.pod_message =
                    "Applied the four-band EQ reset to the offline tone. Nothing was sent.".into();
                true
            }
            Err(error) => {
                self.pod_message = error.detail();
                false
            }
        }
    }

    fn apply_pod_stomp_harmony(
        &mut self,
        index: usize,
        selection: pod_preset::StompHarmonySelection,
    ) -> Result<(), pod_preset::ToneError> {
        let current = self
            .pod_library
            .tone(index)
            .ok_or_else(|| pod_preset::ToneError::format("POD selection is out of range"))?
            .as_bytes()
            .to_vec();
        let replacement = Session::initial_rig()
            .set_pod_stomp_harmony_selection(RoleKind::Pod, &current, selection)
            .map_err(|error| pod_preset::ToneError::format(rejection_text(&error)))?;
        self.replace_pod_tone(index, replacement)
    }

    fn apply_pod_delay_edit(
        &mut self,
        index: usize,
        edit: PendingDelayEdit,
    ) -> Result<(), pod_preset::ToneError> {
        let current = self
            .pod_library
            .tone(index)
            .ok_or_else(|| pod_preset::ToneError::format("POD selection is out of range"))?
            .as_bytes()
            .to_vec();
        let session = Session::initial_rig();
        let replacement = match edit {
            PendingDelayEdit::Note(choice) => {
                session.set_pod_delay_note_choice(RoleKind::Pod, &current, choice)
            }
            PendingDelayEdit::TimeMs(milliseconds) => {
                session.set_pod_delay_time_ms(RoleKind::Pod, &current, milliseconds)
            }
        }
        .map_err(|error| pod_preset::ToneError::format(rejection_text(&error)))?;
        self.replace_pod_tone(index, replacement)
    }

    fn apply_pod_mod_edit(
        &mut self,
        index: usize,
        edit: PendingModEdit,
    ) -> Result<(), pod_preset::ToneError> {
        let current = self
            .pod_library
            .tone(index)
            .ok_or_else(|| pod_preset::ToneError::format("POD selection is out of range"))?
            .as_bytes()
            .to_vec();
        let session = Session::initial_rig();
        let replacement = match edit {
            PendingModEdit::Note(choice) => {
                session.set_pod_mod_note_choice(RoleKind::Pod, &current, choice)
            }
            PendingModEdit::SpeedHz(hz) => {
                session.set_pod_mod_speed_hz(RoleKind::Pod, &current, hz)
            }
        }
        .map_err(|error| pod_preset::ToneError::format(rejection_text(&error)))?;
        self.replace_pod_tone(index, replacement)
    }

    fn load_pod_bundle_bytes(&mut self, bytes: Vec<u8>) -> Result<(), pod_preset::ToneError> {
        if self.pod_dirty() {
            return Err(pod_preset::ToneError::format(
                "save or discard POD edits before loading a bank",
            ));
        }
        let bundle = pod_preset::PodBundleLibrary::parse(bytes)?;
        let mut library = Library::default();
        for index in 0..bundle.entry_count() {
            let (bank_index, preset_index) = bundle
                .entry_location(index)
                .ok_or_else(|| pod_preset::ToneError::format("POD bundle index became invalid"))?;
            library.import_bytes(bundle.tone(bank_index, preset_index)?.into_bytes())?;
        }
        self.pod_library = library;
        self.pod_bundle = Some(bundle);
        self.pod_document_generation = self.pod_document_generation.wrapping_add(1);
        self.pod_selected = Some(0);
        self.pod_eq_reset_pending = None;
        self.pod_cab_pending = None;
        self.pod_harmony_pending = None;
        self.refresh_selected_pod_name();
        Ok(())
    }

    fn refresh_pod_library_from_bundle(&mut self) -> Result<(), pod_preset::ToneError> {
        let mut library = Library::default();
        if let Some(bundle) = &self.pod_bundle {
            for index in 0..bundle.entry_count() {
                let (bank_index, preset_index) = bundle.entry_location(index).ok_or_else(|| {
                    pod_preset::ToneError::format("POD bundle index became invalid")
                })?;
                library.import_bytes(bundle.tone(bank_index, preset_index)?.into_bytes())?;
            }
        }
        self.pod_library = library;
        self.pod_cab_pending = None;
        self.pod_harmony_pending = None;
        Ok(())
    }

    fn apply_pod_membership_action(
        &mut self,
        action: PodMembershipAction,
        bank: usize,
        preset: usize,
    ) {
        let Some(mut candidate) = self.pod_bundle.clone() else {
            return;
        };
        let folder_editable = candidate.membership_editable(bank);
        let result = match action {
            PodMembershipAction::Duplicate => candidate.duplicate_entry(bank, preset),
            PodMembershipAction::Remove => candidate.remove_entry(bank, preset),
            PodMembershipAction::MoveUp if preset > 0 => {
                if folder_editable {
                    candidate.reorder_entry(bank, preset, preset - 1)
                } else {
                    candidate.reorder_fixed_bank_entry(bank, preset, preset - 1)
                }
            }
            PodMembershipAction::MoveDown => {
                if folder_editable {
                    candidate.reorder_entry(bank, preset, preset + 1)
                } else {
                    candidate.reorder_fixed_bank_entry(bank, preset, preset + 1)
                }
            }
            PodMembershipAction::MoveUp => return,
        }
        .and_then(|selected| {
            let mut library = Library::default();
            for index in 0..candidate.entry_count() {
                let (bank_index, preset_index) =
                    candidate.entry_location(index).ok_or_else(|| {
                        pod_preset::ToneError::format("POD bundle index became invalid")
                    })?;
                library.import_bytes(candidate.tone(bank_index, preset_index)?.into_bytes())?;
            }
            Ok((selected, library))
        });
        match result {
            Ok((selected, library)) => {
                self.pod_bundle = Some(candidate);
                self.pod_library = library;
                self.pod_selected = Some(selected);
                self.pod_eq_reset_pending = None;
                self.refresh_selected_pod_name();
                self.pod_message = match action {
                    PodMembershipAction::Duplicate => {
                        "Duplicated the existing tone bytes in this folder. Nothing was sent."
                            .into()
                    }
                    PodMembershipAction::Remove => {
                        "Removed the selected tone from this folder. Nothing was sent.".into()
                    }
                    PodMembershipAction::MoveUp | PodMembershipAction::MoveDown => {
                        if folder_editable {
                            "Reordered tones in this folder. Nothing was sent.".into()
                        } else {
                            "Reordered tones in this fixed bank. Nothing was sent.".into()
                        }
                    }
                };
            }
            Err(error) => self.pod_message = error.detail(),
        }
    }

    fn load_pod_tone(&mut self) {
        let Some(path) = self.dialogs.open_pod_tone() else {
            self.pod_message = "Load cancelled. No file was read.".into();
            return;
        };
        match pod_preset::read_regular_bundle(pod_preset::path_facts(&path), || {
            let file = fs::File::open(&path)?;
            let metadata = file.metadata()?;
            Ok((
                pod_preset::PathFacts {
                    regular_file: metadata.is_file(),
                    len: metadata.len(),
                },
                file,
            ))
        }) {
            Ok(bytes) => self.import_pod_document(bytes),
            Err(error) => self.pod_message = error.detail(),
        }
    }

    fn import_pod_document(&mut self, bytes: Vec<u8>) {
        if matches!(bytes.get(8..12), Some(b"L6BA" | b"L6CO")) {
            match self.load_pod_bundle_bytes(bytes) {
                Ok(()) => {
                    let (banks, tones) = self
                        .pod_bundle
                        .as_ref()
                        .map_or((0, 0), |bundle| (bundle.bank_count(), bundle.entry_count()));
                    self.pod_message = format!(
                        "Loaded PODxt Live collection with {banks} banks and {tones} tones. Offline edits only; nothing was sent."
                    );
                }
                Err(error) => self.pod_message = error.detail(),
            }
        } else {
            self.import_individual_pod_tone(bytes);
        }
    }

    fn import_individual_pod_tone(&mut self, bytes: Vec<u8>) {
        if self.pod_bundle.is_some() && self.pod_dirty() {
            self.pod_message =
                "Save or discard bank edits before loading an individual tone.".into();
            return;
        }
        match Session::initial_rig().open_pod_preset(RoleKind::Pod, &bytes) {
            Ok(summary) => {
                let replaces_bundle_document = self.pod_bundle.is_some();
                let mut candidate_library = if replaces_bundle_document {
                    Library::default()
                } else {
                    self.pod_library.clone()
                };
                match candidate_library.import_bytes(bytes) {
                    Ok(index) => {
                        self.pod_library = candidate_library;
                        if replaces_bundle_document {
                            self.pod_bundle = None;
                            self.pod_document_generation =
                                self.pod_document_generation.wrapping_add(1);
                            self.pod_eq_reset_pending = None;
                            self.pod_cab_pending = None;
                            self.pod_harmony_pending = None;
                        }
                        self.pod_selected = Some(index);
                        self.pod_cab_pending = None;
                        self.pod_delay_model_pending = None;
                        self.pod_mod_model_pending = None;
                        self.pod_stomp_model_pending = None;
                        self.pod_reverb_model_pending = None;
                        self.pod_amp_model_pending = None;
                        self.pod_delay_pending = None;
                        self.pod_mod_pending = None;
                        self.pod_harmony_pending = None;
                        self.pod_name = summary.name;
                        self.pod_message = format!(
                            "Loaded {} for PODxt Live {:08x}. {}",
                            summary.container,
                            summary.device_id,
                            pod_preset::PAYLOADS_NOT_DECODED
                        );
                    }
                    Err(error) => self.pod_message = error.detail(),
                }
            }
            Err(error) => self.pod_message = rejection_text(&error),
        }
    }

    fn apply_pod_name(&mut self) {
        let Some(index) = self.pod_selected else {
            self.pod_message = "Select a tone before renaming it.".into();
            return;
        };
        let Some(current) = self
            .pod_library
            .tone(index)
            .map(|tone| tone.as_bytes().to_vec())
        else {
            self.pod_message = "Select a tone before renaming it.".into();
            return;
        };
        match Session::initial_rig().rename_pod_preset(RoleKind::Pod, &current, &self.pod_name) {
            Ok(bytes) => match self.replace_pod_tone(index, bytes) {
                Ok(()) => {
                    self.pod_message = format!(
                        "Renamed the in-memory tone to {}. Nothing was sent.",
                        self.pod_name
                    );
                }
                Err(error) => self.pod_message = error.detail(),
            },
            Err(error) => self.pod_message = rejection_text(&error),
        }
    }

    fn apply_pod_amp_model_choice(
        &mut self,
        choice: pod_preset::PodAmpModelChoice,
        expected_current_id: Option<u32>,
    ) -> bool {
        let Some(index) = self.pod_selected else {
            self.pod_message = "Select a tone before applying an AMP model.".into();
            return false;
        };
        let Some(current) = self
            .pod_library
            .tone(index)
            .map(|tone| tone.as_bytes().to_vec())
        else {
            self.pod_message = "Select a tone before applying an AMP model.".into();
            return false;
        };
        let current_id = self
            .pod_library
            .tone(index)
            .and_then(pod_preset::PodTone::amp_model_id);
        if expected_current_id.is_some_and(|expected| current_id != Some(expected)) {
            self.pod_amp_model_pending = None;
            self.pod_message =
                "Discarded a stale AMP choice because the stored model changed.".into();
            return false;
        }
        match Session::initial_rig().set_pod_amp_model_choice(RoleKind::Pod, &current, choice) {
            Ok(bytes) => match self.replace_pod_tone(index, bytes) {
                Ok(()) => {
                    self.pod_message = format!(
                        "Applied AMP model {} (0x{:08x}) to the offline tone. Nothing was sent.",
                        choice.label(),
                        choice.model_id()
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

    fn apply_pod_delay_model_choice(
        &mut self,
        index: usize,
        choice: pod_preset::PodDelayModelChoice,
    ) -> Result<(), pod_preset::ToneError> {
        let slot_ids = [pod_preset::DELAY_SLOT, 0x0005_0004];
        let before = self
            .pod_library
            .tone(index)
            .and_then(|tone| {
                tone.models()
                    .iter()
                    .find(|model| slot_ids.contains(&model.slot))
            })
            .map(|model| model.model_id);
        self.edit_pod_tone(index, |tone| tone.set_delay_model_choice(choice))?;
        let after = self
            .pod_library
            .tone(index)
            .and_then(|tone| {
                tone.models()
                    .iter()
                    .find(|model| slot_ids.contains(&model.slot))
            })
            .map(|model| model.model_id);
        if before != after {
            self.pod_delay_pending = None;
        }
        Ok(())
    }

    fn apply_pod_mod_model_choice(
        &mut self,
        index: usize,
        choice: pod_preset::PodModModelChoice,
    ) -> Result<(), pod_preset::ToneError> {
        let slots = [pod_preset::MOD_SLOT, 0x0005_0003];
        let before = self
            .pod_library
            .tone(index)
            .and_then(|tone| {
                tone.models()
                    .iter()
                    .find(|model| slots.contains(&model.slot))
            })
            .map(|model| model.model_id);
        let bytes = self
            .pod_library
            .tone(index)
            .ok_or_else(|| pod_preset::ToneError::format("POD selection is out of range"))?
            .as_bytes()
            .to_vec();
        let replacement = Session::initial_rig()
            .set_pod_mod_model_choice(RoleKind::Pod, &bytes, choice)
            .map_err(|error| pod_preset::ToneError::format(rejection_text(&error)))?;
        self.replace_pod_tone(index, replacement)?;
        let after = self
            .pod_library
            .tone(index)
            .and_then(|tone| {
                tone.models()
                    .iter()
                    .find(|model| slots.contains(&model.slot))
            })
            .map(|model| model.model_id);
        if before != after {
            self.pod_mod_pending = None;
        }
        Ok(())
    }

    fn apply_pod_stomp_model_choice(
        &mut self,
        index: usize,
        choice: pod_preset::PodStompModelChoice,
    ) -> Result<bool, pod_preset::ToneError> {
        let current = self
            .pod_library
            .tone(index)
            .ok_or_else(|| pod_preset::ToneError::format("POD selection is out of range"))?
            .as_bytes()
            .to_vec();
        let replacement = Session::initial_rig()
            .set_pod_stomp_model_choice(RoleKind::Pod, &current, choice)
            .map_err(|error| pod_preset::ToneError::format(rejection_text(&error)))?;
        if replacement == current {
            return Ok(false);
        }
        self.replace_pod_tone(index, replacement)?;
        if self
            .pod_harmony_pending
            .is_some_and(|(pending_index, _, _)| pending_index == index)
        {
            self.pod_harmony_pending = None;
        }
        Ok(true)
    }

    fn apply_pod_reverb_model_choice(
        &mut self,
        index: usize,
        choice: pod_preset::PodReverbModelChoice,
    ) -> Result<bool, pod_preset::ToneError> {
        let current = self
            .pod_library
            .tone(index)
            .ok_or_else(|| pod_preset::ToneError::format("POD selection is out of range"))?
            .as_bytes()
            .to_vec();
        let replacement = Session::initial_rig()
            .set_pod_reverb_model_choice(RoleKind::Pod, &current, choice)
            .map_err(|error| pod_preset::ToneError::format(rejection_text(&error)))?;
        if replacement == current {
            return Ok(false);
        }
        self.replace_pod_tone(index, replacement)?;
        Ok(true)
    }

    fn apply_pod_cab_model_choice(&mut self, choice: pod_preset::PodCabModelChoice) {
        let Some(index) = self.pod_selected else {
            return;
        };
        let Some(current) = self
            .pod_library
            .tone(index)
            .map(|tone| tone.as_bytes().to_vec())
        else {
            return;
        };
        let Some((generation, pending_index, original)) = self.pod_cab_pending_origin.take() else {
            self.pod_message =
                "Discarded the staged CAB choice because its original tone state is unavailable."
                    .into();
            self.pod_cab_pending = None;
            return;
        };
        if generation != self.pod_document_generation
            || pending_index != index
            || current != original
        {
            self.pod_message = "Discarded the staged CAB choice because its tone changed or selection became stale.".into();
            self.pod_cab_pending = None;
            return;
        }
        match Session::initial_rig().set_pod_cab_model_choice(RoleKind::Pod, &current, choice) {
            Ok(bytes) => match self.replace_pod_tone(index, bytes) {
                Ok(()) => {
                    self.pod_message = format!(
                        "Applied CAB {}. MIC value was preserved; nothing was sent.",
                        choice.numeric_label()
                    )
                }
                Err(error) => self.pod_message = error.detail(),
            },
            Err(error) => self.pod_message = rejection_text(&error),
        }
    }

    fn apply_pod_mic_choice(&mut self, choice: pod_preset::PodMicChoice) {
        let Some(index) = self.pod_selected else {
            return;
        };
        let Some(current) = self
            .pod_library
            .tone(index)
            .map(|tone| tone.as_bytes().to_vec())
        else {
            return;
        };
        match Session::initial_rig().set_pod_mic_choice(RoleKind::Pod, &current, choice) {
            Ok(bytes) => match self.replace_pod_tone(index, bytes) {
                Ok(()) => {
                    self.pod_message = format!("Applied MIC {}. Nothing was sent.", choice.value())
                }
                Err(error) => self.pod_message = error.detail(),
            },
            Err(error) => self.pod_message = rejection_text(&error),
        }
    }

    fn save_pod_tone(&mut self) {
        let Some(index) = self.pod_selected else {
            self.pod_message = "Select a tone before saving a copy.".into();
            return;
        };
        self.write_pod_copy(index);
    }

    fn write_pod_copy(&mut self, index: usize) -> bool {
        if self.pod_bundle.is_some() {
            let Some(path) = self.dialogs.save_pod_tone("podxt-live-bank.vwb") else {
                self.pod_message = "Save cancelled. No file was written.".into();
                return false;
            };
            let Some(bundle) = self.pod_bundle.as_ref() else {
                return false;
            };
            match bundle.save_copy(&path) {
                Ok(()) => {
                    if let Some(bundle) = &mut self.pod_bundle {
                        bundle.note_saved();
                    }
                    for tone_index in 0..self.pod_library.len() {
                        if self.pod_library.is_dirty(tone_index) {
                            let _ = self.pod_library.note_saved(tone_index);
                        }
                    }
                    self.pod_message = format!(
                        "Saved a new bank/collection copy at {}. Existing files were not replaced. Nothing was sent.",
                        path.display()
                    );
                    return true;
                }
                Err(error) => {
                    self.pod_message = error.detail();
                    return false;
                }
            }
        }
        let Some((bytes, suggested)) = self
            .pod_library
            .tone(index)
            .map(|tone| (tone.as_bytes().to_vec(), suggested_tone_name(tone.name())))
        else {
            self.pod_message = "Select a tone before saving a copy.".into();
            return false;
        };
        let Some(path) = self.dialogs.save_pod_tone(&suggested) else {
            self.pod_message = "Save cancelled. No file was written.".into();
            return false;
        };
        match Session::initial_rig().save_pod_copy(RoleKind::Pod, &path, &bytes) {
            Ok(()) => match self.pod_library.note_saved(index) {
                Ok(()) => {
                    self.pod_message = format!(
                        "Saved a new copy at {}. Existing files were not replaced. Nothing was sent.",
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
    screenshot: Option<PathBuf>,
    tab: Option<Tab>,
    display: String,
}

fn parse_options(
    arguments: impl IntoIterator<Item = String>,
    default_artwork: &Path,
) -> Result<GuiOptions, String> {
    let mut options = GuiOptions {
        preset: None,
        artwork: default_artwork.to_owned(),
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
            app.screenshot = arguments.screenshot;
            Ok(Box::new(app))
        }),
    )
}

#[cfg(test)]
mod tests {
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
        assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
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
        assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
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
        assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
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
        assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
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
        assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
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

    fn app() -> (egui::Context, Workbench) {
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

    fn prompt_frame(context: &egui::Context, app: &mut Workbench, events: Vec<egui::Event>) {
        let _ = context.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1120., 760.))),
                events,
                ..Default::default()
            },
            |ctx| app.pending_dialog(ctx),
        );
    }

    fn click_prompt(context: &egui::Context, app: &mut Workbench, action: &'static str) {
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
    fn live_access_lock_never_starts_a_worker() {
        let (_, mut app) = app();
        app.start_read();
        assert!(!app.is_reading());
        assert!(app.read_worker.is_none());
        assert!(app.pending.is_none());
        assert!(app.status.contains("locked"));
        app.launch_read();
        assert!(!app.is_reading());
        assert!(app.read_worker.is_none());
    }

    fn recording_frame(context: &egui::Context, app: &mut Workbench, events: Vec<egui::Event>) {
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

    fn recording_frame_tall(
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
        assert!(app.device_check_visible.contains("report already exists"));
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
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
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

    fn prepare_quit_take(app: &mut Workbench, clock: ClockSelection) {
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
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
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
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    fn click_pod(context: &egui::Context, app: &mut Workbench, id: &'static str) {
        recording_frame(context, app, vec![]);
        let pos = app.pod_buttons[id].center();
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

    fn click_pod_model_enable(
        context: &egui::Context,
        app: &mut Workbench,
        control: pod_preset::PodModelEnable,
    ) {
        recording_frame(context, app, vec![]);
        let pos = app.pod_model_enable_checks[&control].center();
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

    fn open_pod_model_enable_menu(context: &egui::Context, app: &mut Workbench) {
        recording_frame(context, app, vec![]);
        click_pod_model_enable_menu(context, app);
        recording_frame(context, app, vec![]);
    }

    fn click_pod_model_enable_menu(context: &egui::Context, app: &mut Workbench) {
        recording_frame(context, app, vec![]);
        let pos = app.pod_model_enable_menu.unwrap().center();
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

    fn sample_pod_tone() -> Vec<u8> {
        fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
            let mut output = id.to_vec();
            output.extend_from_slice(&(data.len() as u32).to_be_bytes());
            output.extend_from_slice(data);
            if !data.len().is_multiple_of(2) {
                output.push(0xa5);
            }
            output
        }
        let mut info = vec![0; 76];
        info[..4].copy_from_slice(&1u32.to_be_bytes());
        info[4..8].copy_from_slice(&pod_preset::PODXT_LIVE_TONE_ID.to_be_bytes());
        info[8..10].copy_from_slice(&u16::from(b'A').to_be_bytes());
        let model = |slot: u32, model_id: u32| {
            let mut minf = Vec::new();
            minf.extend_from_slice(&model_id.to_be_bytes());
            minf.extend_from_slice(&slot.to_be_bytes());
            minf.extend_from_slice(&[0, 0, 0, 1]);
            let mut body = b"MODL".to_vec();
            body.extend(chunk(b"MINF", &minf));
            body.extend(chunk(
                b"PARM",
                &[
                    0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc,
                ],
            ));
            chunk(b"LIST", &body)
        };
        let mut patch = b"PATC".to_vec();
        patch.extend(model(pod_preset::AMP_SLOT, 0x1111_0002));
        patch.extend(model(pod_preset::CAB_SLOT, 0x2222_0003));
        patch.extend(model(0x0003_00aa, 0x3333_0004));
        let mut data = b"L6AS".to_vec();
        data.extend(chunk(b"PINF", &info));
        data.extend(chunk(b"LIST", &patch));
        chunk(b"FORM", &data)
    }

    fn sample_delay_note_tone(delay_note: u32, mod_note: u32) -> Vec<u8> {
        fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
            let mut output = id.to_vec();
            output.extend_from_slice(&(data.len() as u32).to_be_bytes());
            output.extend_from_slice(data);
            if !data.len().is_multiple_of(2) {
                output.push(0xa5);
            }
            output
        }
        fn parameter(id: u32, kind: u32, bits: u32) -> Vec<u8> {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(&(0x3f00_0000 | id).to_be_bytes());
            bytes.extend_from_slice(&kind.to_be_bytes());
            bytes.extend_from_slice(&bits.to_be_bytes());
            chunk(b"PARM", &bytes)
        }
        fn model(slot: u32, model_id: u32, parameters: &[Vec<u8>]) -> Vec<u8> {
            let mut minf = Vec::new();
            minf.extend_from_slice(&model_id.to_be_bytes());
            minf.extend_from_slice(&slot.to_be_bytes());
            minf.extend_from_slice(&[0, 0, 0, 1]);
            let mut contents = b"MODL".to_vec();
            contents.extend(chunk(b"MINF", &minf));
            for parameter in parameters {
                contents.extend_from_slice(parameter);
            }
            chunk(b"LIST", &contents)
        }
        let mut info = vec![0; 80];
        info[..4].copy_from_slice(&1u32.to_be_bytes());
        info[4..8].copy_from_slice(&pod_preset::PODXT_LIVE_TONE_ID.to_be_bytes());
        info[8..10].copy_from_slice(&u16::from(b'D').to_be_bytes());
        info[72..76].copy_from_slice(&pod_preset::PODXT_LIVE_HOST_MIN_VERSION.to_be_bytes());
        let system = model(
            0x0001_0000,
            0x3e70_0000,
            &[parameter(0x0020_0001, 1, 120.0_f32.to_bits())],
        );
        let modulation = model(
            pod_preset::MOD_SLOT,
            pod_preset::MOD_CONTROL_MODEL_IDS[0],
            &[
                parameter(0x0010_0000, 1, 0x3e80_0000),
                parameter(0x0020_0000, 0, mod_note),
            ],
        );
        let delay = model(
            pod_preset::DELAY_SLOT,
            0x0202_0002,
            &[
                parameter(0x0010_0000, 1, 0x3e80_0000),
                parameter(0x0020_0000, 0, delay_note),
            ],
        );
        let mut patch = b"PATC".to_vec();
        patch.extend(system);
        patch.extend(modulation);
        patch.extend(delay);
        let mut data = b"L6AS".to_vec();
        data.extend(chunk(b"PINF", &info));
        data.extend(chunk(b"LIST", &patch));
        chunk(b"FORM", &data)
    }

    fn sample_reverb_model_tone_with_tail(model_id: u32, slot: u32, tail: &[u8]) -> Vec<u8> {
        fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
            let mut bytes = id.to_vec();
            bytes.extend_from_slice(&(data.len() as u32).to_be_bytes());
            bytes.extend_from_slice(data);
            if !data.len().is_multiple_of(2) {
                bytes.push(0xa5);
            }
            bytes
        }
        let mut info = vec![0; 80];
        info[..4].copy_from_slice(&1u32.to_be_bytes());
        info[4..8].copy_from_slice(&pod_preset::PODXT_LIVE_TONE_ID.to_be_bytes());
        info[8..10].copy_from_slice(&u16::from(b'R').to_be_bytes());
        info[72..76].copy_from_slice(&pod_preset::PODXT_LIVE_HOST_MIN_VERSION.to_be_bytes());
        let mut minf = Vec::new();
        minf.extend_from_slice(&model_id.to_be_bytes());
        minf.extend_from_slice(&slot.to_be_bytes());
        minf.extend_from_slice(tail);
        let mut model = b"MODL".to_vec();
        model.extend(chunk(b"MINF", &minf));
        model.extend(chunk(b"PARM", &[0xde, 0xad, 0xbe]));
        model.extend(chunk(b"XTRA", &[0x55, 0x66]));
        let mut patch = b"PATC".to_vec();
        patch.extend(chunk(b"LIST", &model));
        let mut data = b"L6PA".to_vec();
        data.extend(chunk(b"PINF", &info));
        data.extend(chunk(b"LIST", &patch));
        chunk(b"FORM", &data)
    }

    fn sample_reverb_model_tone(model_id: u32, slot: u32) -> Vec<u8> {
        sample_reverb_model_tone_with_tail(model_id, slot, &[0xfe, 0x12, 0x34, 0x01])
    }

    fn sample_fixed_pod_bank(tones: &[Vec<u8>]) -> Vec<u8> {
        fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
            let mut output = id.to_vec();
            output.extend_from_slice(&(data.len() as u32).to_be_bytes());
            output.extend_from_slice(data);
            if !data.len().is_multiple_of(2) {
                output.push(0xa5);
            }
            output
        }
        let mut info = vec![0; 68];
        info[4..6].copy_from_slice(&u16::from(b'A').to_be_bytes());
        let mut bank = b"BANK".to_vec();
        bank.extend(chunk(b"BINF", &info));
        bank.extend(chunk(b"BEFR", &[0x11, 0x22, 0x33]));
        for (index, tone) in tones.iter().enumerate() {
            bank.extend_from_slice(tone);
            if index + 1 < tones.len() {
                bank.extend(chunk(b"MID!", &[0x44, 0x55, 0x66]));
            }
        }
        bank.extend(chunk(b"AFTR", &[0x77, 0x88, 0x99]));
        let mut root = b"L6BA".to_vec();
        root.extend(chunk(b"ROOT", &[0xaa, 0xbb, 0xcc]));
        root.extend(chunk(b"LIST", &bank));
        root.extend(chunk(b"TAIL", &[0xdd, 0xee, 0xff]));
        chunk(b"FORM", &root)
    }

    fn as_fixed_bank_tone(mut tone: Vec<u8>) -> Vec<u8> {
        tone[8..12].copy_from_slice(b"L6PA");
        tone
    }

    fn sample_cab_mic_pod_tone(cab_id: u32, mic_value: u32) -> Vec<u8> {
        fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
            let mut out = id.to_vec();
            out.extend_from_slice(&(data.len() as u32).to_be_bytes());
            out.extend_from_slice(data);
            if !data.len().is_multiple_of(2) {
                out.push(0xa5);
            }
            out
        }
        let mut info = vec![0; 76];
        info[..4].copy_from_slice(&1u32.to_be_bytes());
        info[4..8].copy_from_slice(&pod_preset::PODXT_LIVE_TONE_ID.to_be_bytes());
        info[8..10].copy_from_slice(&u16::from(b'C').to_be_bytes());
        info[72..76].copy_from_slice(&pod_preset::PODXT_LIVE_HOST_MIN_VERSION.to_be_bytes());
        let mut minf = Vec::new();
        minf.extend_from_slice(&cab_id.to_be_bytes());
        minf.extend_from_slice(&pod_preset::CAB_SLOT.to_be_bytes());
        minf.extend_from_slice(&[0x80, 0xa1, 0xb2, 0xfe]);
        let mut mic = Vec::new();
        mic.extend_from_slice(&0x3f00_0000u32.to_be_bytes());
        mic.extend_from_slice(&0u32.to_be_bytes());
        mic.extend_from_slice(&mic_value.to_be_bytes());
        let mut model = b"MODL".to_vec();
        model.extend(chunk(b"MINF", &minf));
        model.extend(chunk(b"PARM", &mic));
        let mut patch = b"PATC".to_vec();
        patch.extend(chunk(b"LIST", &model));
        let mut data = b"L6AS".to_vec();
        data.extend(chunk(b"PINF", &info));
        data.extend(chunk(b"LIST", &patch));
        chunk(b"FORM", &data)
    }

    fn sample_pod_eq_tone() -> Vec<u8> {
        sample_pod_eq_tone_with_amp(false)
    }

    fn sample_pod_amp_eq_tone() -> Vec<u8> {
        sample_pod_eq_tone_with_amp(true)
    }

    fn sample_pod_eq_tone_with_amp(include_amp: bool) -> Vec<u8> {
        fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
            let mut bytes = id.to_vec();
            bytes.extend_from_slice(&(data.len() as u32).to_be_bytes());
            bytes.extend_from_slice(data);
            if !data.len().is_multiple_of(2) {
                bytes.push(0xa5);
            }
            bytes
        }
        let mut info = vec![0; 76];
        info[..4].copy_from_slice(&1u32.to_be_bytes());
        info[4..8].copy_from_slice(&pod_preset::PODXT_LIVE_TONE_ID.to_be_bytes());
        info[72..76].copy_from_slice(&pod_preset::PODXT_LIVE_HOST_MIN_VERSION.to_be_bytes());
        let mut model = b"MODL".to_vec();
        let mut minf = Vec::new();
        minf.extend_from_slice(&pod_preset::PODXT_EQ_MODEL_ID.to_be_bytes());
        minf.extend_from_slice(&pod_preset::PODXT_EQ_SLOT.to_be_bytes());
        minf.extend_from_slice(&[0, 0, 0, 1]);
        model.extend(chunk(b"MINF", &minf));
        for id in 0x0010_0000u32..=0x0010_0007 {
            let mut parm = Vec::new();
            parm.extend_from_slice(&(0x3f00_0000 | id).to_be_bytes());
            parm.extend_from_slice(&1u32.to_be_bytes());
            parm.extend_from_slice(&0x3f00_0000u32.to_be_bytes());
            model.extend(chunk(b"PARM", &parm));
        }
        let mut patch = b"PATC".to_vec();
        patch.extend(chunk(b"LIST", &model));
        if include_amp {
            let mut amp_model = b"MODL".to_vec();
            let mut amp_minf = Vec::new();
            amp_minf.extend_from_slice(&pod_preset::PLEXI_45_MODEL.to_be_bytes());
            amp_minf.extend_from_slice(&pod_preset::AMP_SLOT.to_be_bytes());
            amp_minf.extend_from_slice(&[0, 0, 0, 1]);
            amp_model.extend(chunk(b"MINF", &amp_minf));
            patch.extend(chunk(b"LIST", &amp_model));
        }
        let mut data = b"L6AS".to_vec();
        data.extend(chunk(b"PINF", &info));
        data.extend(chunk(b"LIST", &patch));
        chunk(b"FORM", &data)
    }

    #[test]
    fn pod_model_enable_gui_clicks_direct_tone_and_bundle_and_disables_noncanonical_state() {
        let (context, mut app) = app();
        let mut amp_tone = sample_pod_tone();
        let pinf_header = amp_tone
            .windows(4)
            .position(|window| window == b"PINF")
            .unwrap();
        let pinf_data = pinf_header + 8;
        amp_tone[pinf_data + 72..pinf_data + 76]
            .copy_from_slice(&pod_preset::PODXT_LIVE_HOST_MIN_VERSION.to_be_bytes());
        let original = amp_tone.clone();
        app.pod_library.import_bytes(amp_tone).unwrap();
        app.pod_selected = Some(0);
        app.pod_open = true;
        recording_frame(&context, &mut app, vec![]);
        open_pod_model_enable_menu(&context, &mut app);
        assert!(
            app.pod_model_enable_checks
                .contains_key(&pod_preset::PodModelEnable::Amp)
        );
        assert!(
            !app.pod_model_enable_checks
                .contains_key(&pod_preset::PodModelEnable::Eq)
        );
        let mut marker = 0x1111_0002u32.to_be_bytes().to_vec();
        marker.extend_from_slice(&pod_preset::AMP_SLOT.to_be_bytes());
        let minf_body = original
            .windows(marker.len())
            .position(|window| window == marker)
            .unwrap();
        let enabled_offset = minf_body + 11;
        let mut expected = original.clone();
        expected[enabled_offset] = 0;
        click_pod_model_enable(&context, &mut app, pod_preset::PodModelEnable::Amp);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), expected);
        assert!(app.pod_library.is_dirty(0));
        app.pod_library.revert_unsaved(0).unwrap();
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);

        let mut short_minfo = original.clone();
        let minf_header = short_minfo
            .windows(4)
            .position(|window| window == b"MINF")
            .unwrap();
        short_minfo[minf_header + 4..minf_header + 8].copy_from_slice(&11u32.to_be_bytes());
        short_minfo[minf_header + 8 + 11] = 0;
        let short_original = short_minfo.clone();
        app.pod_library.clear();
        app.pod_library.import_bytes(short_minfo).unwrap();
        app.pod_selected = Some(0);
        recording_frame(&context, &mut app, vec![]);
        open_pod_model_enable_menu(&context, &mut app);
        assert!(
            !app.pod_model_enable_checks
                .contains_key(&pod_preset::PodModelEnable::Amp)
        );
        assert!(
            app.pod_library
                .tone(0)
                .unwrap()
                .model_enable_state(pod_preset::PodModelEnable::Amp)
                .is_err()
        );
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), short_original);
        assert!(!app.pod_library.is_dirty(0));
        click_pod_model_enable_menu(&context, &mut app);

        let mut noncanonical = original;
        noncanonical[enabled_offset] = 0x80;
        let state = pod_preset::PodTone::parse(noncanonical.clone())
            .unwrap()
            .model_enable_state(pod_preset::PodModelEnable::Amp)
            .unwrap();
        assert_eq!(
            state,
            pod_preset::ModelEnableState::NonCanonical {
                raw: 0x80,
                source_interpretation: false,
            }
        );
        app.pod_library.clear();
        app.pod_library.import_bytes(noncanonical.clone()).unwrap();
        app.pod_selected = Some(0);
        recording_frame(&context, &mut app, vec![]);
        open_pod_model_enable_menu(&context, &mut app);
        assert!(
            !app.pod_model_enable_checks
                .contains_key(&pod_preset::PodModelEnable::Amp)
        );
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), noncanonical);
        assert!(!app.pod_library.is_dirty(0));
        click_pod_model_enable_menu(&context, &mut app);

        let eq_tone = sample_pod_eq_tone();
        app.load_pod_bundle_bytes(sample_pod_bundle(std::slice::from_ref(&eq_tone)))
            .unwrap();
        app.pod_open = true;
        recording_frame(&context, &mut app, vec![]);
        open_pod_model_enable_menu(&context, &mut app);
        assert!(
            app.pod_model_enable_checks
                .contains_key(&pod_preset::PodModelEnable::Eq)
        );
        click_pod_model_enable(&context, &mut app, pod_preset::PodModelEnable::Eq);
        assert!(app.pod_bundle.as_ref().unwrap().is_dirty());
        assert_eq!(
            app.pod_bundle
                .as_ref()
                .unwrap()
                .tone(0, 0)
                .unwrap()
                .model_enable_state(pod_preset::PodModelEnable::Eq),
            Ok(pod_preset::ModelEnableState::Canonical(false))
        );
        assert_eq!(
            app.pod_library
                .tone(0)
                .unwrap()
                .model_enable_state(pod_preset::PodModelEnable::Eq),
            Ok(pod_preset::ModelEnableState::Canonical(false))
        );
        assert!(app.pod_message.contains("Nothing was sent"));
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }

        let dir = tempfile::tempdir().unwrap();
        let collision = dir.path().join("existing.vwb");
        std::fs::write(&collision, b"keep existing bytes").unwrap();
        app.dialogs.queue_pod_save(Some(collision.clone()));
        app.pending = Some(Pending::QuitPod);
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pending.is_none());
        assert!(app.pod_bundle.as_ref().unwrap().is_dirty());

        app.pending = Some(Pending::QuitPod);
        click_prompt(&context, &mut app, "save");
        assert!(app.pending.is_some(), "failed save must keep quit pending");
        assert!(app.pod_bundle.as_ref().unwrap().is_dirty());
        assert_eq!(std::fs::read(&collision).unwrap(), b"keep existing bytes");

        let saved = dir.path().join("saved.vwb");
        app.dialogs.queue_pod_save(Some(saved.clone()));
        click_prompt(&context, &mut app, "save");
        assert!(app.pending.is_none());
        assert!(app.permit_close);
        assert!(!app.pod_bundle.as_ref().unwrap().is_dirty());
        assert_eq!(
            pod_preset::PodBundleLibrary::parse(std::fs::read(saved).unwrap())
                .unwrap()
                .tone(0, 0)
                .unwrap()
                .model_enable_state(pod_preset::PodModelEnable::Eq),
            Ok(pod_preset::ModelEnableState::Canonical(false))
        );
    }

    fn sample_pod_eq_tone_with_words(frequency: u32, gain: u32) -> Vec<u8> {
        let mut bytes = sample_pod_eq_tone();
        let marker = 0x3f00_0000u32.to_be_bytes();
        let mut offset = 0;
        let mut parameter = 0;
        while let Some(found) = bytes[offset..]
            .windows(4)
            .position(|window| window == marker)
        {
            let value_offset = offset + found;
            let value = if parameter % 2 == 0 { frequency } else { gain };
            bytes[value_offset..value_offset + 4].copy_from_slice(&value.to_be_bytes());
            offset = value_offset + 4;
            parameter += 1;
            if parameter == 8 {
                break;
            }
        }
        assert_eq!(parameter, 8);
        bytes
    }

    fn sample_stomp_wave_pod_tone() -> Vec<u8> {
        sample_stomp_wave_pod_tone_with_bits(0x3f00_0000)
    }

    fn sample_stomp_wave_pod_tone_with_bits(bits: u32) -> Vec<u8> {
        let mut bytes = sample_drive_pod_tone();
        let mut model_slot = pod_preset::STOMP_DRIVE_MODEL_ID.to_be_bytes().to_vec();
        model_slot.extend_from_slice(&pod_preset::STOMP_SLOT.to_be_bytes());
        let offset = bytes
            .windows(model_slot.len())
            .position(|window| window == model_slot)
            .expect("synthetic Stomp MINF");
        bytes[offset..offset + 4]
            .copy_from_slice(&pod_preset::STOMP_SYNTH_WAVE_MODEL_IDS[0].to_be_bytes());
        let mut value_record = Vec::new();
        value_record.extend_from_slice(
            &((0x3fu32 << 24) | pod_preset::STOMP_SYNTH_WAVE_PARAM).to_be_bytes(),
        );
        value_record.extend_from_slice(&1u32.to_be_bytes());
        value_record.extend_from_slice(&0x3e80_0000u32.to_be_bytes());
        let value_offset = bytes
            .windows(value_record.len())
            .rposition(|window| window == value_record)
            .expect("synthetic Stomp wave PARM")
            + 8;
        bytes[value_offset..value_offset + 4].copy_from_slice(&bits.to_be_bytes());
        bytes
    }

    fn sample_stomp_harmony_pod_tone() -> Vec<u8> {
        sample_stomp_harmony_pod_tone_with_duplicate(None)
    }

    fn sample_stomp_harmony_pod_tone_with_duplicate(duplicate: Option<(u32, u8, u32)>) -> Vec<u8> {
        fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
            let mut out = id.to_vec();
            out.extend_from_slice(&(data.len() as u32).to_be_bytes());
            out.extend_from_slice(data);
            if !data.len().is_multiple_of(2) {
                out.push(0xa5);
            }
            out
        }
        let mut info = vec![0; 80];
        info[..4].copy_from_slice(&1u32.to_be_bytes());
        info[4..8].copy_from_slice(&pod_preset::PODXT_LIVE_TONE_ID.to_be_bytes());
        info[8..10].copy_from_slice(&u16::from(b'A').to_be_bytes());
        info[72..76].copy_from_slice(&pod_preset::PODXT_LIVE_HOST_MIN_VERSION.to_be_bytes());
        let mut patch = b"PATC".to_vec();
        let mut model = b"MODL".to_vec();
        let mut minf = Vec::new();
        minf.extend_from_slice(&0x020a_0009u32.to_be_bytes());
        minf.extend_from_slice(&pod_preset::STOMP_SLOT.to_be_bytes());
        minf.extend_from_slice(&[1, 2, 3, 4]);
        model.extend(chunk(b"MINF", &minf));
        let mut records = vec![
            (0x0010_0001u32, 0x3fu8, 0x3f00_0000u32),
            (0x0010_0002, 0x3f, 0x3f40_8102),
        ];
        if let Some(duplicate) = duplicate {
            records.push(duplicate);
        }
        for (id, prefix, bits) in records {
            let mut parm = Vec::new();
            parm.extend_from_slice(&((u32::from(prefix) << 24) | id).to_be_bytes());
            parm.extend_from_slice(&1u32.to_be_bytes());
            parm.extend_from_slice(&bits.to_be_bytes());
            model.extend(chunk(b"PARM", &parm));
        }
        patch.extend(chunk(b"LIST", &model));
        let mut data = b"L6AS".to_vec();
        data.extend(chunk(b"PINF", &info));
        data.extend(chunk(b"LIST", &patch));
        chunk(b"FORM", &data)
    }

    fn sample_whammy_pod_tone() -> Vec<u8> {
        let mut bytes = sample_drive_pod_tone();
        let old = pod_preset::STOMP_DRIVE_MODEL_ID.to_be_bytes();
        let offset = bytes
            .windows(4)
            .position(|word| word == old)
            .expect("synthetic Stomp MINF");
        bytes[offset..offset + 4]
            .copy_from_slice(&pod_preset::STOMP_WHAMMY_INTERVAL_MODEL_ID.to_be_bytes());
        bytes
    }

    fn sample_delay_association_pod_tone(model_id: u32) -> Vec<u8> {
        let default_bits = if model_id == 0x0202_0005 {
            0.4f32.to_bits()
        } else {
            0.6f32.to_bits()
        };
        sample_delay_association_pod_tone_with_bits(model_id, default_bits)
    }

    fn sample_delay_association_pod_tone_with_bits(model_id: u32, value_bits: u32) -> Vec<u8> {
        let mut bytes = sample_drive_pod_tone();
        let old = pod_preset::DELAY_CONTROL_MODEL_IDS[0].to_be_bytes();
        let offset = bytes.windows(4).position(|word| word == old).unwrap();
        bytes[offset..offset + 4].copy_from_slice(&model_id.to_be_bytes());
        let parameter_id = if model_id == 0x0202_0005 {
            0x0010_0002
        } else {
            0x0010_0003
        };
        let mut marker = ((0x3fu32 << 24) | parameter_id).to_be_bytes().to_vec();
        marker.extend_from_slice(&1u32.to_be_bytes());
        let value_offset = bytes
            .windows(marker.len())
            .rposition(|window| window == marker)
            .unwrap()
            + 8;
        bytes[value_offset..value_offset + 4].copy_from_slice(&value_bits.to_be_bytes());
        bytes
    }

    fn click_pod_at(context: &egui::Context, app: &mut Workbench, pos: Pos2) {
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

    fn choose_cab_popup(context: &egui::Context, app: &mut Workbench, model_id: u32) {
        recording_frame(context, app, vec![]);
        let combo = app.pod_cab_combo.expect("CAB choice combo is rendered");
        click_pod_at(context, app, combo.center());
        recording_frame(context, app, vec![]);
        let option = app.pod_cab_options[&model_id].center();
        click_pod_at(context, app, option);
        recording_frame(context, app, vec![]);
    }

    fn choose_practice_tone(context: &egui::Context, app: &mut Workbench, sample: PracticeTone) {
        recording_frame(context, app, vec![]);
        let combo = app
            .pod_practice_combo
            .expect("Practice tone combo is rendered");
        click_pod_at(context, app, combo.center());
        recording_frame(context, app, vec![]);
        assert_eq!(app.pod_practice_options.len(), PracticeTone::ALL.len());
        let option = app.pod_practice_options[&sample].center();
        click_pod_at(context, app, option);
        recording_frame(context, app, vec![]);
    }

    #[test]
    fn cab_popup_choice_persists_until_apply_and_isolated_per_tone() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        let first = sample_cab_mic_pod_tone(0x0102_0000, 1);
        let second = sample_cab_mic_pod_tone(0x0102_0001, 2);
        app.pod_library.import_bytes(first.clone()).unwrap();
        app.pod_library.import_bytes(second.clone()).unwrap();
        app.pod_selected = Some(0);

        recording_frame(&context, &mut app, vec![]);
        choose_cab_popup(&context, &mut app, 0x0102_0002);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), first);
        assert!(!app.pod_library.is_dirty(0));
        assert_eq!(
            app.pod_cab_pending,
            Some((
                0,
                pod_preset::PodCabModelChoice::try_from(0x0102_0002).unwrap()
            ))
        );
        recording_frame(&context, &mut app, vec![]);
        assert_eq!(app.pod_cab_pending.unwrap().1.model_id(), 0x0102_0002);

        click_pod(&context, &mut app, "pod-cab-apply");
        let applied = app.pod_library.tone(0).unwrap();
        assert_eq!(applied.cab_model_id(), Some(0x0102_0002));
        assert_eq!(applied.mic_choice().unwrap().value(), 1);
        assert!(app.pod_library.is_dirty(0));
        assert_eq!(app.pod_library.tone(1).unwrap().as_bytes(), second);

        choose_cab_popup(&context, &mut app, 0x0102_0003);
        assert_eq!(app.pod_cab_pending.unwrap().1.model_id(), 0x0102_0003);
        recording_frame(&context, &mut app, vec![]);
        let second_row = app.pod_rows[&1].center();
        click_pod_at(&context, &mut app, second_row);
        recording_frame(&context, &mut app, vec![]);
        assert_eq!(app.pod_selected, Some(1));
        assert_eq!(app.pod_cab_pending.unwrap().0, 1);
        assert_eq!(app.pod_cab_pending.unwrap().1.model_id(), 0x0102_0001);
        assert_eq!(app.pod_library.tone(1).unwrap().as_bytes(), second);
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn l6pa_cab_mic_controls_render_and_apply_only_after_explicit_choice() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        let original = include_bytes!("../tests/fixtures/pod-l6pa-cab-mic/cab.before.bin").to_vec();
        app.pod_library.import_bytes(original.clone()).unwrap();
        app.pod_selected = Some(0);
        recording_frame(&context, &mut app, vec![]);
        assert!(app.pod_cab_combo.is_some());
        choose_cab_popup(&context, &mut app, 0x0102_0002);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
        click_pod(&context, &mut app, "pod-cab-apply");
        let changed = app.pod_library.tone(0).unwrap();
        assert_eq!(changed.cab_edit_model_id(), Some(0x0102_0002));
        assert_eq!(changed.mic_choice().unwrap().value(), 2);
        assert!(app.pod_library.is_dirty(0));

        click_pod(&context, &mut app, "pod-mic-3");
        assert_eq!(
            app.pod_library
                .tone(0)
                .unwrap()
                .mic_choice()
                .unwrap()
                .value(),
            3
        );
        let staged = app.pod_cab_pending;
        click_pod(&context, &mut app, "pod-cab-discard");
        assert!(app.pod_cab_pending.is_none() || app.pod_cab_pending != staged);
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn l6pa_cab_edit_saves_reopens_and_stale_stages_do_not_cross_tones() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        let original = include_bytes!("../tests/fixtures/pod-l6pa-cab-mic/cab.before.bin").to_vec();
        let sibling =
            include_bytes!("../tests/fixtures/pod-l6pa-cab-mic/mic-guitar.before.bin").to_vec();
        app.pod_library.import_bytes(original.clone()).unwrap();
        app.pod_library.import_bytes(sibling.clone()).unwrap();
        app.pod_selected = Some(0);
        recording_frame(&context, &mut app, vec![]);

        choose_cab_popup(&context, &mut app, 0x0102_0002);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
        click_pod(&context, &mut app, "pod-cab-apply");
        assert_eq!(
            app.pod_library.tone(0).unwrap().cab_edit_model_id(),
            Some(0x0102_0002)
        );
        assert_eq!(app.pod_library.tone(1).unwrap().as_bytes(), sibling);
        assert!(app.pod_library.is_dirty(0));

        let directory = tempfile::tempdir().unwrap();
        let saved = directory.path().join("l6pa-copy.tone");
        app.dialogs = Box::new(FakeDialogs {
            pod_save: [Some(saved.clone())].into(),
            ..Default::default()
        });
        click_pod(&context, &mut app, "save-pod");
        assert!(!app.pod_library.is_dirty(0));
        let reopened = pod_preset::PodTone::parse(std::fs::read(saved).unwrap()).unwrap();
        assert_eq!(reopened.cab_edit_model_id(), Some(0x0102_0002));
        assert_eq!(reopened.mic_choice().unwrap().value(), 2);

        choose_cab_popup(&context, &mut app, 0x0102_0003);
        let staged = app.pod_cab_pending;
        let sibling_row = app.pod_rows[&1].center();
        click_pod_at(&context, &mut app, sibling_row);
        recording_frame(&context, &mut app, vec![]);
        assert_eq!(app.pod_selected, Some(1));
        assert_eq!(
            app.pod_library.tone(0).unwrap().cab_edit_model_id(),
            Some(0x0102_0002)
        );
        assert!(app.pod_cab_pending.is_none() || app.pod_cab_pending != staged);
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn practice_amp_and_effects_tone_applies_and_reopens_edits() {
        let (context, mut app) = app();
        context.style_mut(|style| {
            style.animation_time = 0.0;
            style.spacing.slider_width = 220.0;
        });
        app.pod_open = true;
        recording_frame(&context, &mut app, vec![]);
        assert!(app.pod_buttons.contains_key("load-pod"));

        choose_practice_tone(&context, &mut app, PracticeTone::AmpAndEffects);
        assert_eq!(app.pod_selected, Some(0));
        assert!(app.pod_bundle.is_none());
        assert_eq!(
            app.pod_library.tone(0).unwrap().as_bytes(),
            sample_amp_effects_tone(b"L6PA")
        );
        let original_drive = app
            .pod_library
            .tone(0)
            .unwrap()
            .amp_drive_fraction()
            .unwrap();
        let original_stomp = app
            .pod_library
            .tone(0)
            .unwrap()
            .stomp_control_fraction(pod_preset::StompControl::Drive)
            .unwrap();
        drag_pod_drive_slider(&context, &mut app);
        drag_pod_stomp_slider(&context, &mut app, pod_preset::StompControl::Drive);
        let edited_drive = app
            .pod_library
            .tone(0)
            .unwrap()
            .amp_drive_fraction()
            .unwrap();
        let edited_stomp = app
            .pod_library
            .tone(0)
            .unwrap()
            .stomp_control_fraction(pod_preset::StompControl::Drive)
            .unwrap();
        assert_ne!(edited_drive.to_bits(), original_drive.to_bits());
        assert_ne!(edited_stomp.to_bits(), original_stomp.to_bits());
        assert!(app.pod_library.is_dirty(0));
        let delay = 0x0202_0003;
        let modulation = 0x0203_0008;
        choose_delay_model_popup(&context, &mut app, delay);
        click_pod_button_tall(&context, &mut app, "pod-delay-model-apply");
        assert_eq!(app.pod_delay_model_pending, None);
        assert_eq!(
            app.pod_library.tone(0).unwrap().delay_model_id(),
            Some(delay)
        );
        choose_mod_model_popup(&context, &mut app, modulation);
        click_pod_button_tall(&context, &mut app, "pod-mod-model-apply");
        assert_eq!(app.pod_mod_model_pending, None);
        assert_eq!(
            app.pod_library.tone(0).unwrap().mod_model_id(),
            Some(modulation)
        );

        let directory = tempfile::tempdir().unwrap();
        let saved = directory.path().join("practice-amp-effects.tone");
        app.dialogs = Box::new(FakeDialogs {
            pod_open: [Some(saved.clone())].into(),
            pod_save: [Some(saved.clone())].into(),
            ..Default::default()
        });
        click_pod(&context, &mut app, "save-pod");
        assert!(!app.pod_library.is_dirty(0));
        click_pod(&context, &mut app, "load-pod");
        assert_eq!(app.pod_selected, Some(1));
        let reopened = app.pod_library.tone(1).unwrap();
        assert_eq!(reopened.delay_model_id(), Some(delay));
        assert_eq!(reopened.mod_model_id(), Some(modulation));
        assert_eq!(
            reopened.amp_drive_fraction().unwrap().to_bits(),
            edited_drive.to_bits()
        );
        assert_eq!(
            reopened
                .stomp_control_fraction(pod_preset::StompControl::Drive)
                .unwrap()
                .to_bits(),
            edited_stomp.to_bits()
        );
        assert!(app.pod_buttons.contains_key("load-pod"));
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn practice_stompbox_collection_saves_the_changed_model() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        recording_frame(&context, &mut app, vec![]);
        assert!(app.pod_buttons.contains_key("load-pod"));

        choose_practice_tone(&context, &mut app, PracticeTone::StompboxCollection);
        assert_eq!(app.pod_selected, Some(0));
        assert!(app.pod_bundle.is_some());
        assert_eq!(
            app.pod_library.tone(0).unwrap().stomp_model_id(),
            Some(0x0205_0006)
        );
        assert!(app.pod_delay_model_combo.is_none());
        assert!(app.pod_mod_model_combo.is_none());
        assert!(!app.pod_buttons.contains_key("pod-delay-model-apply"));
        assert!(!app.pod_buttons.contains_key("pod-mod-model-apply"));
        choose_stomp_model_popup(&context, &mut app, 0x0205_0002);
        click_pod_button_tall(&context, &mut app, "pod-stomp-model-apply");
        assert_eq!(
            app.pod_bundle
                .as_ref()
                .unwrap()
                .tone(0, 0)
                .unwrap()
                .stomp_model_id(),
            Some(0x0205_0002)
        );
        assert!(app.pod_dirty());

        let directory = tempfile::tempdir().unwrap();
        let saved = directory.path().join("practice-stomp-collection.vwb");
        app.dialogs = Box::new(FakeDialogs {
            pod_open: [Some(saved.clone())].into(),
            pod_save: [Some(saved.clone())].into(),
            ..Default::default()
        });
        click_pod(&context, &mut app, "save-pod");
        assert!(!app.pod_dirty());
        click_pod(&context, &mut app, "load-pod");
        let reopened = app.pod_bundle.as_ref().unwrap();
        assert_eq!(
            reopened.tone(0, 0).unwrap().stomp_model_id(),
            Some(0x0205_0002)
        );
        assert!(app.pod_buttons.contains_key("load-pod"));
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    fn named_amp_effects_tone(letter: u8) -> Vec<u8> {
        let mut tone = sample_amp_effects_tone(b"L6PA");
        let marker = pod_preset::PODXT_LIVE_TONE_ID.to_be_bytes();
        let pos = tone
            .windows(4)
            .position(|word| word == marker)
            .expect("PINF device id");
        tone[pos + 4..pos + 6].copy_from_slice(&u16::from(letter).to_be_bytes());
        tone
    }

    fn fixed_pod_banks(banks: &[Vec<Vec<u8>>]) -> Vec<u8> {
        fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
            let mut output = id.to_vec();
            output.extend_from_slice(&(data.len() as u32).to_be_bytes());
            output.extend_from_slice(data);
            if !data.len().is_multiple_of(2) {
                output.push(0xa5);
            }
            output
        }
        let mut root = b"L6BA".to_vec();
        root.extend(chunk(b"ROOT", &[0xaa]));
        for (bank_index, tones) in banks.iter().enumerate() {
            let mut info = vec![0; 68];
            let name = format!("Bank{}", bank_index + 1);
            for (unit_index, unit) in name.encode_utf16().enumerate() {
                let at = 4 + unit_index * 2;
                if at + 2 > info.len() {
                    break;
                }
                info[at..at + 2].copy_from_slice(&unit.to_be_bytes());
            }
            let mut bank = b"BANK".to_vec();
            bank.extend(chunk(b"BINF", &info));
            for tone in tones {
                bank.extend_from_slice(tone);
            }
            root.extend(chunk(b"LIST", &bank));
        }
        chunk(b"FORM", &root)
    }

    #[test]
    fn pod_tab_navigates_banks_channels_and_preserves_block_edits() {
        let (context, mut app) = app();
        context.style_mut(|style| {
            style.animation_time = 0.0;
            style.spacing.slider_width = 220.0;
        });
        let bank_one = vec![
            named_amp_effects_tone(b'A'),
            named_amp_effects_tone(b'B'),
            named_amp_effects_tone(b'C'),
            named_amp_effects_tone(b'D'),
        ];
        let bank_two = vec![named_amp_effects_tone(b'E'), named_amp_effects_tone(b'F')];
        let original_d = bank_one[3].clone();
        let original_a = bank_one[0].clone();
        let original_e = bank_two[0].clone();
        app.load_pod_bundle_bytes(fixed_pod_banks(&[bank_one, bank_two]))
            .unwrap();
        app.pod_open = true;
        recording_frame(&context, &mut app, vec![]);
        assert!(app.pod_buttons.contains_key("pod-channel-a"));
        assert!(app.pod_buttons.contains_key("pod-channel-d"));
        assert!(app.pod_visible.contains("Fetch needs gate G5"));
        assert!(
            app.pod_visible
                .contains("reviewed write and store workflow")
        );
        let before_fetch = app.pod_bundle.as_ref().unwrap().as_bytes().to_vec();
        click_pod(&context, &mut app, "fetch-pod");
        click_pod(&context, &mut app, "send-bank");
        click_pod(&context, &mut app, "send-all");
        assert_eq!(app.pod_bundle.as_ref().unwrap().as_bytes(), before_fetch);
        assert!(!app.pod_message.contains("sent to"));

        click_pod(&context, &mut app, "pod-channel-d");
        assert_eq!(app.pod_selected, Some(3));
        assert!(app.pod_library.tone(3).unwrap().name().contains('D'));
        let drive_before = app
            .pod_library
            .tone(3)
            .unwrap()
            .amp_drive_fraction()
            .unwrap();
        drag_pod_drive_slider_tall(&context, &mut app);
        let drive_after = app
            .pod_library
            .tone(3)
            .unwrap()
            .amp_drive_fraction()
            .unwrap();
        assert_ne!(drive_before.to_bits(), drive_after.to_bits());
        click_pod(&context, &mut app, "pod-block-stomp");
        assert_eq!(app.pod_block, PodBlock::Stomp);
        assert_eq!(app.pod_selected, Some(3));
        let amp_on = app
            .pod_library
            .tone(3)
            .unwrap()
            .model_enable_state(pod_preset::PodModelEnable::Amp);
        let mod_on = app
            .pod_library
            .tone(3)
            .unwrap()
            .model_enable_state(pod_preset::PodModelEnable::Mod);
        let delay_on = app
            .pod_library
            .tone(3)
            .unwrap()
            .model_enable_state(pod_preset::PodModelEnable::Delay);
        let stomp_model = app.pod_library.tone(3).unwrap().stomp_model_id();
        click_pod(&context, &mut app, "pod-block-bypass");
        assert_eq!(
            app.pod_library
                .tone(3)
                .unwrap()
                .model_enable_state(pod_preset::PodModelEnable::Stomp)
                .unwrap(),
            pod_preset::ModelEnableState::Canonical(false)
        );
        assert_eq!(
            app.pod_library
                .tone(3)
                .unwrap()
                .model_enable_state(pod_preset::PodModelEnable::Amp)
                .unwrap(),
            amp_on.unwrap()
        );
        assert_eq!(
            app.pod_library
                .tone(3)
                .unwrap()
                .model_enable_state(pod_preset::PodModelEnable::Mod)
                .unwrap(),
            mod_on.unwrap()
        );
        assert_eq!(
            app.pod_library
                .tone(3)
                .unwrap()
                .model_enable_state(pod_preset::PodModelEnable::Delay)
                .unwrap(),
            delay_on.unwrap()
        );
        assert_eq!(
            app.pod_library.tone(3).unwrap().stomp_model_id(),
            stomp_model
        );
        assert_eq!(
            app.pod_library
                .tone(3)
                .unwrap()
                .amp_drive_fraction()
                .unwrap()
                .to_bits(),
            drive_after.to_bits()
        );
        let edited_d = app.pod_library.tone(3).unwrap().as_bytes().to_vec();
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original_a);

        click_pod(&context, &mut app, "pod-channel-a");
        assert_eq!(app.pod_selected, Some(0));
        assert_eq!(app.pod_library.tone(3).unwrap().as_bytes(), edited_d);
        click_pod(&context, &mut app, "show-variax");
        assert!(!app.pod_open);
        assert_eq!(app.pod_library.tone(3).unwrap().as_bytes(), edited_d);
        click_pod(&context, &mut app, "open-pod");
        assert!(app.pod_open);
        assert_eq!(app.pod_library.tone(3).unwrap().as_bytes(), edited_d);

        click_pod(&context, &mut app, "pod-bank-up");
        assert_eq!(app.pod_bank, 1);
        assert_eq!(app.pod_selected, Some(4));
        click_pod(&context, &mut app, "pod-channel-d");
        assert_eq!(app.pod_selected, Some(4), "empty channel D stays disabled");
        click_pod(&context, &mut app, "pod-channel-b");
        assert_eq!(app.pod_selected, Some(5));
        click_pod(&context, &mut app, "pod-bank-down");
        assert_eq!(app.pod_bank, 0);
        assert_eq!(app.pod_library.tone(3).unwrap().as_bytes(), edited_d);
        assert_eq!(
            app.pod_bundle
                .as_ref()
                .unwrap()
                .tone(1, 0)
                .unwrap()
                .as_bytes(),
            original_e
        );

        recording_frame(&context, &mut app, vec![]);
        assert!(app.pod_visible.contains("Revert restores"));
        click_pod(&context, &mut app, "revert-pod");
        click_pod(&context, &mut app, "revert-pod-cancel");
        assert_eq!(app.pod_library.tone(3).unwrap().as_bytes(), edited_d);
        click_pod(&context, &mut app, "revert-pod");
        click_pod(&context, &mut app, "revert-pod-confirm");
        assert_eq!(app.pod_library.tone(3).unwrap().as_bytes(), original_d);
        assert!(!app.pod_dirty());

        click_pod(&context, &mut app, "pod-channel-d");
        drag_pod_drive_slider_tall(&context, &mut app);
        click_pod(&context, &mut app, "pod-block-delay");
        click_pod(&context, &mut app, "pod-block-bypass");
        assert_eq!(
            app.pod_library
                .tone(3)
                .unwrap()
                .model_enable_state(pod_preset::PodModelEnable::Delay)
                .unwrap(),
            pod_preset::ModelEnableState::Canonical(false)
        );
        assert_eq!(
            app.pod_library
                .tone(0)
                .unwrap()
                .model_enable_state(pod_preset::PodModelEnable::Delay)
                .unwrap(),
            pod_preset::ModelEnableState::Canonical(true)
        );
        let directory = tempfile::tempdir().unwrap();
        let saved = directory.path().join("bank-copy.vwb");
        app.dialogs = Box::new(FakeDialogs {
            pod_save: [Some(saved.clone())].into(),
            pod_open: [Some(saved.clone())].into(),
            ..Default::default()
        });
        click_pod(&context, &mut app, "save-pod");
        assert!(!app.pod_dirty());
        let reopened = pod_preset::PodBundleLibrary::parse(std::fs::read(&saved).unwrap()).unwrap();
        assert_eq!(reopened.bank_count(), 2);
        assert_eq!(reopened.preset_count(0), Some(4));
        assert_eq!(reopened.preset_count(1), Some(2));
        assert_eq!(reopened.tone(0, 0).unwrap().as_bytes(), original_a);
        assert_eq!(reopened.tone(1, 0).unwrap().as_bytes(), original_e);
        assert_ne!(reopened.tone(0, 3).unwrap().as_bytes(), original_d);
        assert_eq!(
            reopened
                .tone(0, 3)
                .unwrap()
                .model_enable_state(pod_preset::PodModelEnable::Delay)
                .unwrap(),
            pod_preset::ModelEnableState::Canonical(false)
        );
        assert_eq!(
            reopened
                .tone(0, 3)
                .unwrap()
                .model_enable_state(pod_preset::PodModelEnable::Amp)
                .unwrap(),
            pod_preset::ModelEnableState::Canonical(true)
        );
        click_pod(&context, &mut app, "load-pod");
        assert_eq!(
            app.pod_bundle
                .as_ref()
                .unwrap()
                .tone(0, 3)
                .unwrap()
                .model_enable_state(pod_preset::PodModelEnable::Stomp)
                .unwrap(),
            pod_preset::ModelEnableState::Canonical(true)
        );
        const {
            assert!(!hardware_gates::is_locked(Gate::IdentityRequest));
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
            assert!(hardware_gates::is_locked(Gate::FullBackupRead));
            assert!(hardware_gates::is_locked(Gate::EditBufferWrite));
            assert!(hardware_gates::is_locked(Gate::StoreRestore));
        }
    }

    #[test]
    fn pod_tab_browses_thirty_two_banks_and_a_short_bank() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        recording_frame(&context, &mut app, vec![]);
        choose_practice_tone(&context, &mut app, PracticeTone::AmpAndEffects);
        recording_frame(&context, &mut app, vec![]);
        click_pod(&context, &mut app, "pod-channel-b");
        assert_eq!(app.pod_selected, Some(0));
        click_pod(&context, &mut app, "pod-channel-d");
        assert_eq!(app.pod_selected, Some(0));
        assert!(app.pod_buttons.contains_key("pod-bank-up"));

        let tone = sample_stomp_model_tone(0x0205_0006);
        let banks: Vec<Vec<Vec<u8>>> = (0..32)
            .map(|_| (0..4).map(|_| tone.clone()).collect())
            .collect();
        app.load_pod_bundle_bytes(fixed_pod_banks(&banks)).unwrap();
        assert_eq!(app.pod_bundle.as_ref().unwrap().bank_count(), 32);
        recording_frame(&context, &mut app, vec![]);
        for bank in 0..32 {
            app.show_pod_channel(bank, 3);
            recording_frame(&context, &mut app, vec![]);
            assert_eq!(app.pod_bank, bank);
            assert_eq!(app.pod_selected, Some(bank * 4 + 3));
            assert!(app.pod_buttons.contains_key("pod-channel-a"));
            assert!(app.pod_buttons.contains_key("pod-channel-d"));
        }
        click_pod(&context, &mut app, "pod-bank-down");
        assert_eq!(app.pod_bank, 30);
        click_pod(&context, &mut app, "pod-channel-a");
        assert_eq!(app.pod_selected, Some(30 * 4));

        let factory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("analysis/reports/podxt-live-3.01-factory/PODxt Live 3.01 Factory.vwb");
        if factory.is_file() {
            let bytes = std::fs::read(&factory).unwrap();
            app.load_pod_bundle_bytes(bytes).unwrap();
            let bundle = app.pod_bundle.as_ref().unwrap();
            assert_eq!(bundle.bank_count(), 32);
            for bank in 0..32 {
                assert_eq!(bundle.preset_count(bank), Some(4));
            }
            app.pod_open = true;
            recording_frame(&context, &mut app, vec![]);
            click_pod(&context, &mut app, "pod-bank-up");
            click_pod(&context, &mut app, "pod-channel-d");
            assert_eq!(app.pod_bank, 1);
            assert_eq!(app.pod_selected, Some(7));
            app.show_pod_channel(31, 0);
            recording_frame(&context, &mut app, vec![]);
            assert_eq!(app.pod_bank, 31);
            assert_eq!(app.pod_bundle.as_ref().unwrap().preset_count(31), Some(4));
            click_pod(&context, &mut app, "fetch-pod");
            click_pod(&context, &mut app, "send-all");
            assert_eq!(app.pod_bank, 31);
        }
        const {
            assert!(hardware_gates::is_locked(Gate::FullBackupRead));
        }
    }

    #[test]
    fn l6pa_cab_stage_replacement_and_save_quit_failures_use_real_gui_paths() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        let original = include_bytes!("../tests/fixtures/pod-l6pa-cab-mic/cab.before.bin").to_vec();
        app.pod_library.import_bytes(original.clone()).unwrap();
        app.pod_selected = Some(0);
        recording_frame(&context, &mut app, vec![]);

        choose_cab_popup(&context, &mut app, 0x0102_0002);
        assert_eq!(app.pod_cab_pending.unwrap().1.model_id(), 0x0102_0002);
        let old_generation = app.pod_document_generation;
        let mut replacement = original.clone();
        let pinf = replacement
            .windows(4)
            .position(|word| word == b"PINF")
            .unwrap()
            + 8;
        replacement[pinf + 8..pinf + 10].copy_from_slice(&[0, b'R']);
        let original_tone = pod_preset::PodTone::parse(original.clone()).unwrap();
        let replacement_tone = pod_preset::PodTone::parse(replacement.clone()).unwrap();
        assert_eq!(
            original_tone.cab_edit_model_id(),
            replacement_tone.cab_edit_model_id()
        );
        assert_eq!(original_tone.mic_choice(), replacement_tone.mic_choice());
        assert_ne!(original, replacement);

        // A successful clean collection replacement reuses index zero and keeps
        // CAB/MIC bytes identical while changing an unrelated PINF name byte.
        app.load_pod_bundle_bytes(sample_pod_bundle(&[replacement.clone()]))
            .unwrap();
        assert_eq!(app.pod_document_generation, old_generation + 1);
        assert_eq!(app.pod_selected, Some(0));
        assert!(app.pod_cab_pending.is_none());
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), replacement);
        assert!(!app.pod_dirty());

        recording_frame(&context, &mut app, vec![]);
        choose_cab_popup(&context, &mut app, 0x0102_0003);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), replacement);
        click_pod(&context, &mut app, "pod-cab-apply");
        assert_eq!(
            app.pod_library.tone(0).unwrap().cab_edit_model_id(),
            Some(0x0102_0003)
        );
        assert!(app.pod_dirty());

        let directory = tempfile::tempdir().unwrap();
        let failed = directory.path().join("missing-parent/copy.vwb");
        let collision = directory.path().join("occupied.vwb");
        std::fs::write(&collision, b"keep original collision bytes").unwrap();
        let saved = directory.path().join("saved.vwb");
        app.dialogs = Box::new(FakeDialogs {
            pod_save: [
                None,
                Some(failed.clone()),
                Some(collision.clone()),
                Some(saved.clone()),
            ]
            .into(),
            ..Default::default()
        });

        click_pod(&context, &mut app, "save-pod");
        assert!(app.pod_dirty());
        assert!(!failed.exists());
        click_pod(&context, &mut app, "save-pod");
        assert!(app.pod_dirty());
        assert!(!failed.exists());
        click_pod(&context, &mut app, "save-pod");
        assert!(app.pod_dirty());
        assert_eq!(
            std::fs::read(&collision).unwrap(),
            b"keep original collision bytes"
        );
        click_pod(&context, &mut app, "save-pod");
        assert!(!app.pod_dirty());
        let reopened = pod_preset::PodBundleLibrary::parse(std::fs::read(&saved).unwrap()).unwrap();
        assert_eq!(
            reopened.tone(0, 0).unwrap().cab_edit_model_id(),
            Some(0x0102_0003)
        );
        assert_eq!(
            reopened.tone(0, 0).unwrap().mic_choice().unwrap().value(),
            2
        );

        click_pod(&context, &mut app, "pod-mic-3");
        assert!(app.pod_dirty());
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pending.is_none());
        assert!(app.pod_dirty());
        assert!(!app.permit_close);

        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "discard");
        assert!(app.pending.is_none());
        assert!(!app.pod_dirty());
        assert!(app.permit_close);
        assert_eq!(
            app.pod_library.tone(0).unwrap().cab_edit_model_id(),
            Some(0x0102_0003)
        );
        assert_eq!(
            app.pod_library
                .tone(0)
                .unwrap()
                .mic_choice()
                .unwrap()
                .value(),
            2
        );
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn cab_mic_gui_apply_collision_save_reopen_revert_and_quit_cancel() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        let original = sample_cab_mic_pod_tone(0x0102_0000, 1);
        app.pod_library.import_bytes(original).unwrap();
        app.pod_selected = Some(0);
        recording_frame(&context, &mut app, vec![]);

        choose_cab_popup(&context, &mut app, 0x0102_0002);
        assert_eq!(
            app.pod_library.tone(0).unwrap().cab_model_id(),
            Some(0x0102_0000)
        );
        click_pod(&context, &mut app, "pod-cab-apply");
        assert_eq!(
            app.pod_library.tone(0).unwrap().cab_model_id(),
            Some(0x0102_0002)
        );
        assert_eq!(
            app.pod_library
                .tone(0)
                .unwrap()
                .mic_choice()
                .unwrap()
                .value(),
            1
        );
        assert!(app.pod_library.is_dirty(0));

        let directory = tempfile::tempdir().unwrap();
        let collision = directory.path().join("occupied.tone");
        std::fs::write(&collision, b"preserve existing destination").unwrap();
        let saved = directory.path().join("cab-copy.tone");
        app.dialogs = Box::new(FakeDialogs {
            pod_save: [Some(collision.clone()), Some(saved.clone())].into(),
            ..Default::default()
        });
        click_pod(&context, &mut app, "save-pod");
        assert!(app.pod_library.is_dirty(0));
        assert_eq!(
            std::fs::read(&collision).unwrap(),
            b"preserve existing destination"
        );
        click_pod(&context, &mut app, "save-pod");
        assert!(!app.pod_library.is_dirty(0));
        let reopened = pod_preset::PodTone::parse(std::fs::read(&saved).unwrap()).unwrap();
        assert_eq!(reopened.cab_model_id(), Some(0x0102_0002));
        assert_eq!(reopened.mic_choice().unwrap().value(), 1);

        choose_cab_popup(&context, &mut app, 0x0102_0003);
        click_pod(&context, &mut app, "pod-cab-apply");
        assert!(app.pod_library.is_dirty(0));
        app.pod_library.revert_unsaved(0).unwrap();
        app.refresh_selected_pod_name();
        assert_eq!(app.pod_cab_pending, None);
        recording_frame(&context, &mut app, vec![]);
        assert_eq!(app.pod_cab_pending.unwrap().1.model_id(), 0x0102_0002);

        click_pod(&context, &mut app, "pod-mic-3");
        assert!(app.pod_library.is_dirty(0));
        assert_eq!(
            app.pod_library
                .tone(0)
                .unwrap()
                .mic_choice()
                .unwrap()
                .value(),
            3
        );
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pod_library.is_dirty(0));
        assert!(!app.permit_close);
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    fn select_delay_heads_choice(
        context: &egui::Context,
        app: &mut Workbench,
        choice: pod_preset::DelayHeadsSelection,
    ) {
        recording_frame_tall(context, app, vec![]);
        let choice_rect = app.pod_delay_heads_choices[&choice];
        click_pod_at_tall(context, app, choice_rect.center());
        recording_frame_tall(context, app, vec![]);
    }

    fn select_delay_bits_choice(
        context: &egui::Context,
        app: &mut Workbench,
        choice: pod_preset::DelayBitsSelection,
    ) {
        recording_frame_tall(context, app, vec![]);
        let choice_rect = app.pod_delay_bits_choices[&choice];
        click_pod_at_tall(context, app, choice_rect.center());
        recording_frame_tall(context, app, vec![]);
    }

    fn select_stomp_wave_choice(
        context: &egui::Context,
        app: &mut Workbench,
        choice: pod_preset::StompWaveSelection,
    ) {
        recording_frame_tall(context, app, vec![]);
        let choice_rect = app.pod_stomp_wave_choices[&choice];
        click_pod_at_tall(context, app, choice_rect.center());
        recording_frame_tall(context, app, vec![]);
    }

    fn stage_stomp_harmony1_choice(
        context: &egui::Context,
        app: &mut Workbench,
        choice: pod_preset::StompHarmony1M335Selection,
    ) {
        recording_frame_tall(context, app, vec![]);
        let rect = app.pod_harmony1_choices[&choice];
        click_pod_at_tall(context, app, rect.center());
        recording_frame_tall(context, app, vec![]);
    }

    fn stage_delay_note_choice(
        context: &egui::Context,
        app: &mut Workbench,
        choice: pod_preset::DelayNoteChoice,
    ) {
        recording_frame_tall(context, app, vec![]);
        let option = app.pod_delay_note_options[&choice];
        click_pod_at_tall(context, app, option.center());
        recording_frame_tall(context, app, vec![]);
    }

    fn choose_delay_model_popup(context: &egui::Context, app: &mut Workbench, model_id: u32) {
        recording_frame_tall(context, app, vec![]);
        let combo = app
            .pod_delay_model_combo
            .expect("Delay model selector is rendered");
        click_pod_at_tall(context, app, combo.center());
        recording_frame_tall(context, app, vec![]);
        let option = app.pod_delay_model_options[&model_id];
        click_pod_at_tall(context, app, option.center());
        recording_frame_tall(context, app, vec![]);
    }

    fn choose_mod_model_popup(context: &egui::Context, app: &mut Workbench, model_id: u32) {
        recording_frame_tall(context, app, vec![]);
        let combo = app
            .pod_mod_model_combo
            .expect("Mod model selector is rendered");
        click_pod_at_tall(context, app, combo.center());
        recording_frame_tall(context, app, vec![]);
        let option = app.pod_mod_model_options[&model_id];
        click_pod_at_tall(context, app, option.center());
        recording_frame_tall(context, app, vec![]);
    }

    fn choose_stomp_model_popup(context: &egui::Context, app: &mut Workbench, model_id: u32) {
        recording_frame_tall(context, app, vec![]);
        let combo = app
            .pod_stomp_model_combo
            .expect("Stomp model selector is rendered");
        click_pod_at_tall(context, app, combo.center());
        recording_frame_tall(context, app, vec![]);
        let option = app.pod_stomp_model_options[&model_id];
        click_pod_at_tall(context, app, option.center());
        recording_frame_tall(context, app, vec![]);
    }

    fn choose_reverb_model_popup(context: &egui::Context, app: &mut Workbench, model_id: u32) {
        recording_frame_tall(context, app, vec![]);
        let combo = app
            .pod_reverb_model_combo
            .expect("Reverb model selector is rendered");
        click_pod_at_tall(context, app, combo.center());
        recording_frame_tall(context, app, vec![]);
        let option = app.pod_reverb_model_options[&model_id];
        click_pod_at_tall(context, app, option.center());
        recording_frame_tall(context, app, vec![]);
    }

    fn choose_amp_model_popup(context: &egui::Context, app: &mut Workbench, model_id: u32) {
        recording_frame_tall(context, app, vec![]);
        let combo = app
            .pod_amp_model_combo
            .expect("AMP model selector is rendered");
        click_pod_at_tall(context, app, combo.center());
        recording_frame_tall(context, app, vec![]);
        assert_eq!(app.pod_amp_model_options.len(), 43);
        let option = app.pod_amp_model_options[&model_id];
        click_pod_at_tall(context, app, option.center());
        recording_frame_tall(context, app, vec![]);
    }

    fn click_pod_button_tall(context: &egui::Context, app: &mut Workbench, id: &'static str) {
        recording_frame_tall(context, app, vec![]);
        let position = app.pod_buttons[id].center();
        click_pod_at_tall(context, app, position);
    }

    fn stage_mod_note_choice(
        context: &egui::Context,
        app: &mut Workbench,
        choice: pod_preset::ModNoteChoice,
    ) {
        recording_frame_tall(context, app, vec![]);
        let option = app.pod_mod_note_options[&choice];
        click_pod_at_tall(context, app, option.center());
        recording_frame_tall(context, app, vec![]);
    }

    fn drag_delay_time_slider(context: &egui::Context, app: &mut Workbench, fraction: f32) {
        recording_frame_tall(context, app, vec![]);
        let rect = app
            .pod_delay_time_slider
            .expect("visible Delay TIME slider");
        let start = Pos2::new(rect.center().x, rect.center().y);
        let end = Pos2::new(rect.left() + rect.width() * fraction, rect.center().y);
        recording_frame_tall(
            context,
            app,
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
        recording_frame_tall(context, app, vec![egui::Event::PointerMoved(end)]);
        recording_frame_tall(
            context,
            app,
            vec![egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
    }

    fn drag_mod_speed_slider(context: &egui::Context, app: &mut Workbench, fraction: f32) {
        recording_frame_tall(context, app, vec![]);
        let rect = app.pod_mod_speed_slider.expect("visible Mod SPEED slider");
        let start = Pos2::new(rect.center().x, rect.center().y);
        let end = Pos2::new(rect.left() + rect.width() * fraction, rect.center().y);
        recording_frame_tall(
            context,
            app,
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
        recording_frame_tall(context, app, vec![egui::Event::PointerMoved(end)]);
        recording_frame_tall(
            context,
            app,
            vec![egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
    }

    fn stage_stomp_harmony2_choice(
        context: &egui::Context,
        app: &mut Workbench,
        choice: pod_preset::StompHarmony1457Selection,
    ) {
        recording_frame_tall(context, app, vec![]);
        let rect = app.pod_harmony2_choices[&choice];
        click_pod_at_tall(context, app, rect.center());
        recording_frame_tall(context, app, vec![]);
    }

    fn click_pod_at_tall(context: &egui::Context, app: &mut Workbench, pos: Pos2) {
        recording_frame_tall(
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
        recording_frame_tall(
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

    fn drag_pod_drive_slider(context: &egui::Context, app: &mut Workbench) {
        recording_frame(context, app, vec![]);
        let rect = app.pod_drive_slider.expect("visible Amp Drive slider");
        let start = Pos2::new(rect.left() + rect.width() * 0.45, rect.center().y);
        let end = Pos2::new(rect.left() + rect.width() * 0.78, rect.center().y);
        recording_frame(
            context,
            app,
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
        recording_frame(context, app, vec![egui::Event::PointerMoved(end)]);
        recording_frame(
            context,
            app,
            vec![egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
    }

    fn drag_pod_drive_slider_tall(context: &egui::Context, app: &mut Workbench) {
        recording_frame_tall(context, app, vec![]);
        let rect = app.pod_drive_slider.expect("visible Amp Drive slider");
        let start = Pos2::new(rect.left() + rect.width() * 0.45, rect.center().y);
        let end = Pos2::new(rect.left() + rect.width() * 0.78, rect.center().y);
        recording_frame_tall(
            context,
            app,
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
        recording_frame_tall(context, app, vec![egui::Event::PointerMoved(end)]);
        recording_frame_tall(
            context,
            app,
            vec![egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
    }

    fn drag_pod_stomp_slider(
        context: &egui::Context,
        app: &mut Workbench,
        control: pod_preset::StompControl,
    ) {
        recording_frame(context, app, vec![]);
        let rect = app.pod_stomp_sliders[&control];
        let start = Pos2::new(rect.left() + rect.width() * 0.35, rect.center().y);
        let end = Pos2::new(rect.left() + rect.width() * 0.73, rect.center().y);
        recording_frame(
            context,
            app,
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
        recording_frame(context, app, vec![egui::Event::PointerMoved(end)]);
        recording_frame(
            context,
            app,
            vec![egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
    }

    fn drag_pod_whammy_slider(
        context: &egui::Context,
        app: &mut Workbench,
        control: pod_preset::StompWhammyKnob,
    ) {
        recording_frame(context, app, vec![]);
        let rect = app.pod_whammy_knob_sliders[&control];
        let start = Pos2::new(rect.left() + rect.width() * 0.35, rect.center().y);
        let end = Pos2::new(rect.left() + rect.width() * 0.73, rect.center().y);
        recording_frame(
            context,
            app,
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
        recording_frame(context, app, vec![egui::Event::PointerMoved(end)]);
        recording_frame(
            context,
            app,
            vec![egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
    }

    fn drag_pod_eq_frequency_slider(
        context: &egui::Context,
        app: &mut Workbench,
        band: pod_preset::PodxtEqBand,
    ) {
        recording_frame(context, app, vec![]);
        let rect = app.pod_eq_frequency_sliders[&band];
        let start = Pos2::new(rect.left() + rect.width() * 0.35, rect.center().y);
        let end = Pos2::new(rect.left() + rect.width() * 0.17, rect.center().y);
        recording_frame(
            context,
            app,
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
        recording_frame(context, app, vec![egui::Event::PointerMoved(end)]);
        recording_frame(
            context,
            app,
            vec![egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
    }

    fn drag_pod_eq_gain_slider(
        context: &egui::Context,
        app: &mut Workbench,
        band: pod_preset::PodxtEqBand,
    ) {
        recording_frame(context, app, vec![]);
        let rect = app.pod_eq_gain_sliders[&band];
        let start = Pos2::new(rect.left() + rect.width() * 0.35, rect.center().y);
        let end = Pos2::new(rect.left() + rect.width() * 0.17, rect.center().y);
        recording_frame(
            context,
            app,
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
        recording_frame(context, app, vec![egui::Event::PointerMoved(end)]);
        recording_frame(
            context,
            app,
            vec![egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
    }

    fn drag_pod_mod_slider(
        context: &egui::Context,
        app: &mut Workbench,
        control: pod_preset::ModControl,
    ) {
        recording_frame(context, app, vec![]);
        let rect = app.pod_mod_sliders[&control];
        let start = Pos2::new(rect.left() + rect.width() * 0.35, rect.center().y);
        let end = Pos2::new(rect.left() + rect.width() * 0.73, rect.center().y);
        recording_frame(
            context,
            app,
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
        recording_frame(context, app, vec![egui::Event::PointerMoved(end)]);
        recording_frame(
            context,
            app,
            vec![egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
    }

    fn drag_pod_reverb_slider(
        context: &egui::Context,
        app: &mut Workbench,
        control: pod_preset::ReverbControl,
    ) {
        recording_frame(context, app, vec![]);
        let rect = app.pod_reverb_sliders[&control];
        let start = Pos2::new(rect.left() + rect.width() * 0.35, rect.center().y);
        let end = Pos2::new(rect.left() + rect.width() * 0.73, rect.center().y);
        recording_frame(
            context,
            app,
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
        recording_frame(context, app, vec![egui::Event::PointerMoved(end)]);
        recording_frame(
            context,
            app,
            vec![egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
    }

    fn drag_pod_delay_slider(
        context: &egui::Context,
        app: &mut Workbench,
        control: pod_preset::DelayControl,
    ) {
        recording_frame(context, app, vec![]);
        let rect = app.pod_delay_sliders[&control];
        let start = Pos2::new(rect.left() + rect.width() * 0.35, rect.center().y);
        let end = Pos2::new(rect.left() + rect.width() * 0.73, rect.center().y);
        recording_frame(
            context,
            app,
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
        recording_frame(context, app, vec![egui::Event::PointerMoved(end)]);
        recording_frame(
            context,
            app,
            vec![egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
    }

    #[test]
    fn offline_pod_tone_window_does_not_open_a_device_or_edit_the_guitar() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let guitar = app.document.as_ref().unwrap().as_bytes().to_vec();
        let directory = tempfile::tempdir().unwrap();
        let blocked = directory.path().join("not-a-tone");
        std::fs::create_dir(&blocked).unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_open: [Some(blocked)].into(),
            ..Default::default()
        });
        click_pod(&context, &mut app, "open-pod");
        assert!(app.pod_open);
        assert!(app.pod_visible.contains(pod_preset::OFFLINE_BANNER));
        assert!(app.pod_visible.contains(pod_preset::NOT_GUITAR));
        assert!(app.pod_visible.contains(pod_preset::NOT_SENT));
        assert!(app.pod_visible.contains("remain raw"));
        assert!(app.read_worker.is_none());
        assert!(!app.is_reading());
        assert_eq!(app.pod_library.len(), 0);
        click_pod_button_tall(&context, &mut app, "load-pod");
        assert!(app.pod_message.contains("regular file"));
        assert!(app.pod_message.contains("not opened"));
        assert!(app.pod_library.is_empty());
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), guitar);

        let tone_path = directory.path().join("stage.tone");
        std::fs::write(&tone_path, sample_pod_tone()).unwrap();
        let guitar_path = directory.path().join("guitar.vwm");
        std::fs::write(&guitar_path, &guitar).unwrap();
        let saved = directory.path().join("copy.tone");
        app.dialogs = Box::new(FakeDialogs {
            pod_open: [Some(tone_path), Some(guitar_path)].into(),
            pod_save: [Some(saved.clone()), Some(saved.clone())].into(),
            ..Default::default()
        });
        click_pod(&context, &mut app, "load-pod");
        assert!(app.pod_visible.contains("L6AS"));
        assert!(app.pod_visible.contains("11110002"));
        assert!(app.pod_visible.contains("22220003"));
        assert!(app.pod_visible.contains("remain raw"));
        assert!(
            app.pod_message.contains("Nothing is sent")
                || app.pod_visible.contains(pod_preset::NOT_SENT)
        );
        assert_eq!(app.pod_library.len(), 1);
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), guitar);
        assert!(app.read_worker.is_none());
        app.pod_name = "Stage".into();
        click_pod(&context, &mut app, "apply-pod-name");
        assert!(app.pod_message.contains("Nothing was sent"));
        assert!(app.pod_visible.contains("Stage"));
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), guitar);
        click_pod(&context, &mut app, "save-pod");
        let written = std::fs::read(&saved).unwrap();
        let tone = pod_preset::PodTone::parse(written.clone()).unwrap();
        assert_eq!(tone.name(), "Stage");
        assert_eq!(tone.amp_model_id(), Some(0x1111_0002));
        assert_eq!(tone.cab_model_id(), Some(0x2222_0003));
        assert_eq!(tone.undecoded_slots(), 1);
        assert!(
            tone.as_bytes()
                .windows(4)
                .any(|bytes| bytes == [0x11, 0x22, 0x33, 0x44])
        );
        click_pod(&context, &mut app, "save-pod");
        assert!(!app.pod_message.starts_with("Saved a new copy"));
        assert_eq!(std::fs::read(&saved).unwrap(), written);
        click_pod(&context, &mut app, "load-pod");
        assert!(app.pod_message.contains("Variax"));
        assert_eq!(app.pod_library.len(), 1);
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), guitar);
        app.start_read();
        assert!(app.read_worker.is_none());
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn pod_amp_model_apply_preserves_save_revert_and_quit_guards() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        let mut original = sample_drive_pod_tone();
        original[8..12].copy_from_slice(b"L6PA");
        app.pod_library.import_bytes(original.clone()).unwrap();
        app.pod_selected = Some(0);
        recording_frame(&context, &mut app, vec![]);
        assert!(app.pod_drive_slider.is_some());

        let no_amp = pod_preset::PodAmpModelChoice::NoAmp;
        click_pod(&context, &mut app, "pod-amp-no-amp");
        assert_eq!(
            app.pod_library.tone(0).unwrap().amp_model_id(),
            Some(no_amp.model_id())
        );
        assert!(app.pod_library.is_dirty(0));
        assert!(app.pod_drive_slider.is_none());
        assert!(app.pod_message.contains("Nothing was sent"));

        choose_amp_model_popup(&context, &mut app, 0x0002_002e);
        let pending = app.pod_amp_model_pending;
        assert!(pending.is_some());
        assert!(app.pod_library.is_dirty(0));

        let directory = tempfile::tempdir().unwrap();
        let collision = directory.path().join("existing.tone");
        std::fs::write(&collision, b"keep original").unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_save: [Some(collision.clone()), None].into(),
            ..Default::default()
        });
        click_pod(&context, &mut app, "save-pod");
        assert!(app.pod_library.is_dirty(0));
        assert_eq!(std::fs::read(&collision).unwrap(), b"keep original");
        assert_eq!(app.pod_amp_model_pending, pending);
        click_pod(&context, &mut app, "save-pod");
        assert!(app.pod_library.is_dirty(0));
        assert!(app.pod_message.contains("cancelled"));
        assert_eq!(app.pod_amp_model_pending, pending);

        let saved = directory.path().join("saved.tone");
        app.dialogs.queue_pod_save(Some(saved.clone()));
        click_pod(&context, &mut app, "save-pod");
        assert!(!app.pod_library.is_dirty(0));
        assert_eq!(app.pod_amp_model_pending, pending);
        let reopened = pod_preset::PodTone::parse(std::fs::read(&saved).unwrap()).unwrap();
        assert_eq!(reopened.amp_model_id(), Some(no_amp.model_id()));
        click_pod(&context, &mut app, "pod-amp-no-amp");
        assert!(!app.pod_library.is_dirty(0));
        assert_eq!(
            app.pod_library.tone(0).unwrap().amp_model_id(),
            Some(no_amp.model_id())
        );

        let clean = pod_preset::PodAmpModelChoice::Line6Clean;
        assert_eq!(reopened.amp_model_id(), Some(no_amp.model_id()));
        assert_eq!(
            app.pod_library.tone(0).unwrap().amp_model_id(),
            Some(no_amp.model_id())
        );
        click_pod(&context, &mut app, "pod-amp-line6-clean");
        assert_eq!(
            app.pod_library.tone(0).unwrap().amp_model_id(),
            Some(clean.model_id())
        );
        assert!(app.pod_library.is_dirty(0));
        choose_amp_model_popup(&context, &mut app, 0x0002_002e);
        let pending_before_close = app.pod_amp_model_pending;

        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        assert!(!app.permit_close);
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pod_library.is_dirty(0));
        assert_eq!(app.pod_amp_model_pending, pending_before_close);
        assert!(!app.permit_close);
        app.request_close(&context);
        click_prompt(&context, &mut app, "discard");
        assert!(app.permit_close);
        assert_eq!(app.pod_amp_model_pending, None);
        assert!(!app.pod_library.is_dirty(0));
        assert_eq!(
            app.pod_library.tone(0).unwrap().amp_model_id(),
            Some(no_amp.model_id())
        );
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn pod_amp_combo_stages_all_choices_and_applies_from_pack_gated_current_id() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        let mut original = sample_drive_pod_tone();
        original[8..12].copy_from_slice(b"L6PA");
        let old_id = pod_preset::PLEXI_45_MODEL;
        let marker = [old_id.to_be_bytes(), pod_preset::AMP_SLOT.to_be_bytes()].concat();
        let amp = original
            .windows(marker.len())
            .position(|window| window == marker)
            .unwrap();
        original[amp..amp + 4].copy_from_slice(&0x0003_0001u32.to_be_bytes());
        app.pod_library.import_bytes(original.clone()).unwrap();
        app.pod_selected = Some(0);
        recording_frame_tall(&context, &mut app, vec![]);
        assert!(!app.pod_library.is_dirty(0));
        assert_eq!(
            app.pod_library.tone(0).unwrap().amp_model_id(),
            Some(0x0003_0001)
        );

        app.pod_reverb_model_pending = Some((
            0,
            pod_preset::PodReverbModelChoice::from_model_id(
                pod_preset::REVERB_CONTROL_MODEL_IDS[1],
            )
            .unwrap(),
        ));
        choose_amp_model_popup(&context, &mut app, 0x0002_002e);
        assert_eq!(app.pod_amp_model_pending.unwrap().2.model_id(), 0x0002_002e);
        assert!(!app.pod_library.is_dirty(0));
        click_pod_button_tall(&context, &mut app, "pod-amp-model-apply");
        recording_frame_tall(&context, &mut app, vec![]);

        let changed = app.pod_library.tone(0).unwrap();
        assert_eq!(changed.amp_model_id(), Some(0x0002_002e));
        assert!(app.pod_library.is_dirty(0));
        assert_eq!(
            app.pod_reverb_model_pending.unwrap().1.model_id(),
            pod_preset::REVERB_CONTROL_MODEL_IDS[1]
        );
        let parsed = pod_preset::PodTone::parse(changed.as_bytes().to_vec()).unwrap();
        let minf_offset = parsed
            .models()
            .iter()
            .find(|model| model.slot == pod_preset::AMP_SLOT)
            .unwrap()
            .model_id;
        assert_eq!(minf_offset, 0x0002_002e);
        assert_eq!(app.pod_amp_model_pending, None);
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn pod_amp_first_choice_is_reachable_from_every_pack_gated_current_id() {
        let gated: Vec<_> = pod_preset::AMP_CONTROL_MODEL_IDS
            .into_iter()
            .filter(|id| pod_preset::PodAmpModelChoice::from_model_id(*id).is_none())
            .collect();
        assert_eq!(gated.len(), 64);
        for current_id in gated {
            let (context, mut app) = app();
            context.style_mut(|style| style.animation_time = 0.0);
            app.pod_open = true;
            let mut original = sample_drive_pod_tone();
            original[8..12].copy_from_slice(b"L6PA");
            let marker = [
                pod_preset::PLEXI_45_MODEL.to_be_bytes(),
                pod_preset::AMP_SLOT.to_be_bytes(),
            ]
            .concat();
            let amp = original
                .windows(marker.len())
                .position(|window| window == marker)
                .unwrap();
            original[amp..amp + 4].copy_from_slice(&current_id.to_be_bytes());
            app.pod_library.import_bytes(original).unwrap();
            app.pod_selected = Some(0);
            recording_frame_tall(&context, &mut app, vec![]);
            choose_amp_model_popup(&context, &mut app, 0x0002_0000);
            click_pod_button_tall(&context, &mut app, "pod-amp-model-apply");
            recording_frame_tall(&context, &mut app, vec![]);
            assert_eq!(
                app.pod_library.tone(0).unwrap().amp_model_id(),
                Some(0x0002_0000),
                "current {current_id:08x}"
            );
            assert!(app.pod_library.is_dirty(0), "current {current_id:08x}");
        }
    }

    #[test]
    fn pod_amp_failed_apply_retains_pending_choice_and_tone_bytes() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        let original = as_fixed_bank_tone(sample_drive_pod_tone());
        app.pod_library.import_bytes(original.clone()).unwrap();
        let sibling = as_fixed_bank_tone(sample_drive_pod_tone());
        app.pod_library.import_bytes(sibling.clone()).unwrap();
        app.pod_library.set_amp_drive_fraction(0, 0.7).unwrap();
        app.pod_library.set_amp_drive_fraction(1, 0.8).unwrap();
        app.pod_selected = Some(0);
        let choice = pod_preset::PodAmpModelChoice::Model2002e;
        recording_frame_tall(&context, &mut app, vec![]);
        choose_amp_model_popup(&context, &mut app, choice.model_id());
        let pending = Some((0, pod_preset::PLEXI_45_MODEL, choice));
        assert_eq!(app.pod_amp_model_pending, pending);
        app.pod_reverb_model_pending = Some((
            0,
            pod_preset::PodReverbModelChoice::from_model_id(
                pod_preset::REVERB_CONTROL_MODEL_IDS[1],
            )
            .unwrap(),
        ));

        let before_edit = app.pod_library.tone(0).unwrap().as_bytes().to_vec();
        let mut stale_profile = before_edit.clone();
        let pinf = stale_profile
            .windows(4)
            .position(|bytes| bytes == b"PINF")
            .unwrap();
        let version = pinf + 8 + 72;
        stale_profile[version..version + 4]
            .copy_from_slice(&(pod_preset::PODXT_LIVE_HOST_MIN_VERSION - 1).to_be_bytes());
        assert_eq!(
            app.pod_library.tone(0).unwrap().amp_model_id(),
            Some(pod_preset::PLEXI_45_MODEL)
        );
        app.replace_pod_tone(0, stale_profile).unwrap();
        let before = app.pod_library.tone(0).unwrap().as_bytes().to_vec();
        assert_eq!(app.pod_amp_model_pending, pending);
        assert!(app.pod_library.is_dirty(0));
        assert!(app.pod_library.is_dirty(1));

        click_pod_button_tall(&context, &mut app, "pod-amp-model-apply");
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), before);
        assert_eq!(
            pod_preset::PodTone::parse(before.clone()).unwrap(),
            *app.pod_library.tone(0).unwrap()
        );
        assert_eq!(
            app.pod_library.tone(0).unwrap().amp_model_id(),
            Some(pod_preset::PLEXI_45_MODEL)
        );
        assert_eq!(app.pod_amp_model_pending, pending);
        assert_eq!(
            app.pod_reverb_model_pending.unwrap().1.model_id(),
            pod_preset::REVERB_CONTROL_MODEL_IDS[1]
        );
        assert!(app.pod_library.is_dirty(0));
        assert!(app.pod_library.is_dirty(1));
        assert_eq!(app.pod_library.tone(1).unwrap().as_bytes(), {
            let mut expected = sibling.clone();
            let mut dirty = pod_preset::PodTone::parse(expected.clone()).unwrap();
            dirty
                .set_amp_control_fraction(pod_preset::AmpControl::Drive, 0.8)
                .unwrap();
            expected = dirty.as_bytes().to_vec();
            expected
        });

        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        assert_eq!(app.pod_amp_model_pending, pending);
        click_prompt(&context, &mut app, "discard");
        assert!(!app.pod_library.is_dirty(0));
        assert!(app.pod_library.is_dirty(1));
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
        assert_eq!(app.pod_amp_model_pending, pending);
        click_prompt(&context, &mut app, "discard");
        assert!(!app.pod_library.is_dirty(1));
        assert_eq!(app.pod_library.tone(1).unwrap().as_bytes(), sibling);
        assert_eq!(app.pod_amp_model_pending, pending);
        click_prompt(&context, &mut app, "amp-discard");
        assert_eq!(app.pod_amp_model_pending, None);
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn pod_amp_pending_survives_legacy_rejection_and_unrelated_edit() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        let mut tone = sample_drive_pod_tone();
        tone[8..12].copy_from_slice(b"L6PA");
        let pinf = tone.windows(4).position(|bytes| bytes == b"PINF").unwrap();
        let version = pinf + 8 + 72;
        tone[version..version + 4]
            .copy_from_slice(&(pod_preset::PODXT_LIVE_HOST_MIN_VERSION - 1).to_be_bytes());
        app.pod_library.import_bytes(tone.clone()).unwrap();
        app.pod_selected = Some(0);
        recording_frame_tall(&context, &mut app, vec![]);
        choose_amp_model_popup(&context, &mut app, 0x0002_002e);
        let pending = Some((
            0,
            pod_preset::PLEXI_45_MODEL,
            pod_preset::PodAmpModelChoice::Model2002e,
        ));
        assert_eq!(app.pod_amp_model_pending, pending);
        assert!(!app.pod_library.is_dirty(0));
        click_pod_button_tall(&context, &mut app, "pod-amp-no-amp");
        recording_frame_tall(&context, &mut app, vec![]);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), tone);
        assert!(!app.pod_library.is_dirty(0));
        assert_eq!(app.pod_amp_model_pending, pending);

        let mut tone = sample_drive_pod_tone();
        tone[8..12].copy_from_slice(b"L6PA");
        app.pod_library = pod_preset::Library::default();
        app.pod_library.import_bytes(tone).unwrap();
        app.pod_amp_model_pending = None;
        app.pod_selected = Some(0);
        recording_frame_tall(&context, &mut app, vec![]);
        choose_amp_model_popup(&context, &mut app, 0x0002_002e);
        drag_pod_drive_slider(&context, &mut app);
        assert_eq!(app.pod_amp_model_pending, pending);
        assert!(app.pod_library.is_dirty(0));
        assert_eq!(
            app.pod_library.tone(0).unwrap().amp_model_id(),
            Some(pod_preset::PLEXI_45_MODEL)
        );
    }

    #[test]
    fn pod_amp_stage_only_quit_resolves_failed_apply_and_other_quit_prompts() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        let mut original = as_fixed_bank_tone(sample_drive_pod_tone());
        let pinf = original
            .windows(4)
            .position(|bytes| bytes == b"PINF")
            .unwrap();
        original[pinf + 8 + 72..pinf + 8 + 76]
            .copy_from_slice(&(pod_preset::PODXT_LIVE_HOST_MIN_VERSION - 1).to_be_bytes());
        app.pod_library.import_bytes(original.clone()).unwrap();
        app.pod_selected = Some(0);
        recording_frame_tall(&context, &mut app, vec![]);
        choose_amp_model_popup(&context, &mut app, 0x0002_002e);
        let pending = Some((
            0,
            pod_preset::PLEXI_45_MODEL,
            pod_preset::PodAmpModelChoice::Model2002e,
        ));
        assert_eq!(app.pod_amp_model_pending, pending);
        assert!(!app.pod_library.is_dirty(0));

        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        assert!(!app.permit_close);
        click_prompt(&context, &mut app, "amp-apply");
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        assert_eq!(app.pod_amp_model_pending, pending);
        assert!(!app.permit_close);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
        click_prompt(&context, &mut app, "amp-keep-editing");
        assert!(app.pending.is_none());
        assert_eq!(app.pod_amp_model_pending, pending);
        assert!(!app.permit_close);

        app.change(|preset| preset.rename("Continue to staged POD choice"));
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::Quit)));
        click_prompt(&context, &mut app, "discard");
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        assert_eq!(app.pod_amp_model_pending, pending);
        click_prompt(&context, &mut app, "amp-discard");
        assert!(app.permit_close);
        assert!(app.pending.is_none());
        assert_eq!(app.pod_amp_model_pending, None);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
        assert!(!app.pod_library.is_dirty(0));

        let (context, mut app) = self::app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        let original = as_fixed_bank_tone(sample_drive_pod_tone());
        app.pod_library.import_bytes(original.clone()).unwrap();
        app.pod_selected = Some(0);
        recording_frame_tall(&context, &mut app, vec![]);
        choose_amp_model_popup(&context, &mut app, 0x0002_002e);
        let pending = app.pod_amp_model_pending;
        app.request_close(&context);
        click_prompt(&context, &mut app, "amp-apply");
        assert_eq!(app.pod_amp_model_pending, None);
        assert_eq!(
            app.pod_library.tone(0).unwrap().amp_model_id(),
            Some(pod_preset::PodAmpModelChoice::Model2002e.model_id())
        );
        assert!(app.pod_library.is_dirty(0));
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        assert!(!app.permit_close);
        click_prompt(&context, &mut app, "discard");
        assert!(app.permit_close);
        assert_eq!(app.pod_amp_model_pending, None);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
        assert!(!app.pod_library.is_dirty(0));
        assert!(pending.is_some());

        let (context, mut app) = self::app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        app.pod_library.import_bytes(original.clone()).unwrap();
        app.pod_selected = Some(0);
        recording_frame_tall(&context, &mut app, vec![]);
        choose_amp_model_popup(&context, &mut app, 0x0002_002e);
        let staged_before_edit = app.pod_amp_model_pending;
        app.pod_library.set_amp_drive_fraction(0, 0.6).unwrap();
        assert!(app.pod_library.is_dirty(0));
        app.request_close(&context);
        assert_eq!(app.pod_amp_model_pending, staged_before_edit);
        click_prompt(&context, &mut app, "discard");
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
        assert!(!app.pod_library.is_dirty(0));
        assert_eq!(app.pod_amp_model_pending, staged_before_edit);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "amp-discard");
        assert_eq!(app.pod_amp_model_pending, None);
        assert!(app.permit_close);
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn pod_amp_stage_load_navigation_and_explicit_discard_are_preserving() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        let original = as_fixed_bank_tone(sample_drive_pod_tone());
        let reopened = original.clone();
        let second = sample_pod_tone();
        let directory = tempfile::tempdir().unwrap();
        let tone_path = directory.path().join("reopened.tone");
        let bundle_path = directory.path().join("replacement.vwb");
        std::fs::write(&tone_path, &reopened).unwrap();
        std::fs::write(
            &bundle_path,
            sample_pod_bundle(std::slice::from_ref(&second)),
        )
        .unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_open: [None, Some(tone_path.clone())].into(),
            ..Default::default()
        });
        app.pod_library.import_bytes(original.clone()).unwrap();
        app.pod_selected = Some(0);
        recording_frame_tall(&context, &mut app, vec![]);
        choose_amp_model_popup(&context, &mut app, 0x0002_002e);
        let pending = app.pod_amp_model_pending;
        click_pod_button_tall(&context, &mut app, "load-pod");
        assert_eq!(app.pod_amp_model_pending, pending);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
        assert!(!app.pod_library.is_dirty(0));

        click_pod_button_tall(&context, &mut app, "load-pod");
        assert_eq!(app.pod_amp_model_pending, None);
        assert_eq!(app.pod_library.len(), 2);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
        assert_eq!(app.pod_library.tone(1).unwrap().as_bytes(), reopened);
        assert!(!app.pod_library.is_dirty(0));
        assert!(!app.pod_library.is_dirty(1));

        app.pod_selected = Some(0);
        choose_amp_model_popup(&context, &mut app, 0x0002_002e);
        let navigation_stage = Some((
            0,
            pod_preset::PLEXI_45_MODEL,
            pod_preset::PodAmpModelChoice::Model2002e,
        ));
        assert_eq!(app.pod_amp_model_pending, navigation_stage);
        let first_baseline = app.pod_library.tone(0).unwrap().as_bytes().to_vec();
        recording_frame_tall(&context, &mut app, vec![]);
        let row = app.pod_rows[&1].center();
        click_pod_at_tall(&context, &mut app, row);
        assert_eq!(app.pod_selected, Some(1));
        assert_eq!(app.pod_amp_model_pending, None);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), first_baseline);
        assert_eq!(app.pod_library.tone(1).unwrap().as_bytes(), reopened);

        app.pod_selected = Some(1);
        recording_frame_tall(&context, &mut app, vec![]);
        choose_amp_model_popup(&context, &mut app, 0x0002_002e);
        let explicit_pending = app.pod_amp_model_pending;
        app.pod_reverb_model_pending = Some((
            1,
            pod_preset::PodReverbModelChoice::from_model_id(
                pod_preset::REVERB_CONTROL_MODEL_IDS[1],
            )
            .unwrap(),
        ));
        click_pod_button_tall(&context, &mut app, "pod-amp-model-discard");
        assert_eq!(app.pod_amp_model_pending, None);
        assert_eq!(app.pod_library.tone(1).unwrap().as_bytes(), reopened);
        assert!(!app.pod_library.is_dirty(1));
        assert_eq!(
            app.pod_reverb_model_pending.unwrap().1.model_id(),
            pod_preset::REVERB_CONTROL_MODEL_IDS[1]
        );
        assert!(explicit_pending.is_some());

        let (context, mut app) = self::app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        app.dialogs = Box::new(FakeDialogs {
            pod_open: [Some(bundle_path)].into(),
            ..Default::default()
        });
        app.pod_library.import_bytes(original).unwrap();
        app.pod_selected = Some(0);
        recording_frame_tall(&context, &mut app, vec![]);
        choose_amp_model_popup(&context, &mut app, 0x0002_002e);
        assert!(app.pod_amp_model_pending.is_some());
        click_pod_button_tall(&context, &mut app, "load-pod");
        assert_eq!(app.pod_amp_model_pending, None);
        assert_eq!(app.pod_library.len(), 1);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), second);
        assert_eq!(
            app.pod_bundle
                .as_ref()
                .unwrap()
                .tone(0, 0)
                .unwrap()
                .as_bytes(),
            second
        );
        assert!(!app.pod_library.is_dirty(0));
        assert!(!app.pod_bundle.as_ref().unwrap().is_dirty());
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn pod_amp_bundle_quit_discard_preserves_only_matching_stage_origin() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        let directory = tempfile::tempdir().unwrap();
        let bundle_path = directory.path().join("amp-stage.vwb");
        let original_tone = as_fixed_bank_tone(sample_drive_pod_tone());
        let original_bundle = sample_pod_bundle(std::slice::from_ref(&original_tone));
        std::fs::write(&bundle_path, &original_bundle).unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_open: [Some(bundle_path.clone())].into(),
            ..Default::default()
        });
        app.load_pod_tone();
        recording_frame_tall(&context, &mut app, vec![]);
        assert!(app.pod_bundle.is_some(), "{}", app.pod_message);
        assert_eq!(
            app.pod_bundle.as_ref().unwrap().as_bytes(),
            original_bundle.as_slice()
        );
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original_tone);
        assert!(!app.pod_dirty());

        drag_pod_drive_slider_tall(&context, &mut app);
        assert!(app.pod_library.is_dirty(0));
        assert!(app.pod_bundle.as_ref().unwrap().is_dirty());
        assert_eq!(
            app.pod_library.tone(0).unwrap().amp_model_id(),
            Some(pod_preset::PLEXI_45_MODEL)
        );
        choose_amp_model_popup(&context, &mut app, 0x0002_002e);
        let pending = Some((
            0,
            pod_preset::PLEXI_45_MODEL,
            pod_preset::PodAmpModelChoice::Model2002e,
        ));
        assert_eq!(app.pod_amp_model_pending, pending);
        let dirty_tone = app.pod_library.tone(0).unwrap().as_bytes().to_vec();
        let dirty_bundle = app.pod_bundle.as_ref().unwrap().as_bytes().to_vec();
        assert_ne!(dirty_tone, original_tone);
        assert_ne!(dirty_bundle, original_bundle);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), dirty_tone);
        assert_eq!(app.pod_bundle.as_ref().unwrap().as_bytes(), dirty_bundle);

        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        assert!(!app.permit_close);
        click_prompt(&context, &mut app, "discard");
        assert_eq!(app.pod_bundle.as_ref().unwrap().as_bytes(), original_bundle);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original_tone);
        assert!(!app.pod_dirty());
        assert_eq!(app.pod_amp_model_pending, pending);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        assert!(!app.permit_close);

        click_prompt(&context, &mut app, "amp-keep-editing");
        assert!(app.pending.is_none());
        assert_eq!(app.pod_amp_model_pending, pending);
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "amp-discard");
        assert_eq!(app.pod_amp_model_pending, None);
        assert!(app.permit_close);

        let (context, mut app) = self::app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        app.dialogs = Box::new(FakeDialogs {
            pod_open: [Some(bundle_path)].into(),
            ..Default::default()
        });
        app.load_pod_tone();
        recording_frame_tall(&context, &mut app, vec![]);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original_tone);
        click_pod_button_tall(&context, &mut app, "pod-amp-line6-clean");
        assert!(app.pod_bundle.as_ref().unwrap().is_dirty());
        choose_amp_model_popup(&context, &mut app, 0x0002_002e);
        let invalidated_pending = Some((
            0,
            pod_preset::PodAmpModelChoice::Line6Clean.model_id(),
            pod_preset::PodAmpModelChoice::Model2002e,
        ));
        assert_eq!(app.pod_amp_model_pending, invalidated_pending);
        app.request_close(&context);
        click_prompt(&context, &mut app, "discard");
        assert_eq!(app.pod_bundle.as_ref().unwrap().as_bytes(), original_bundle);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original_tone);
        assert_eq!(app.pod_amp_model_pending, None);
        assert!(app.permit_close);
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn pod_amp_context_change_clears_stale_stage_and_bundle_rejection_is_atomic() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        let first = as_fixed_bank_tone(sample_drive_pod_tone());
        let second = first.clone();
        app.pod_library.import_bytes(first.clone()).unwrap();
        app.pod_library.import_bytes(second.clone()).unwrap();
        app.pod_library.set_amp_drive_fraction(1, 0.65).unwrap();
        app.pod_selected = Some(1);
        let pending = Some((
            1,
            pod_preset::PLEXI_45_MODEL,
            pod_preset::PodAmpModelChoice::Model2002e,
        ));
        app.pod_amp_model_pending = pending;
        app.pod_bundle = Some(
            pod_preset::PodBundleLibrary::parse(sample_pod_bundle(std::slice::from_ref(&first)))
                .unwrap(),
        );
        let dirty_second = app.pod_library.tone(1).unwrap().as_bytes().to_vec();
        let replacement = Session::initial_rig()
            .set_pod_amp_model_choice(
                RoleKind::Pod,
                &dirty_second,
                pod_preset::PodAmpModelChoice::Model2002e,
            )
            .unwrap();
        let before_bundle = app.pod_bundle.as_ref().unwrap().as_bytes().to_vec();
        let before_tone = app.pod_library.tone(1).unwrap().as_bytes().to_vec();
        let baseline = second.clone();
        assert!(app.replace_pod_tone(1, replacement).is_err());
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), first);
        assert_eq!(app.pod_library.tone(1).unwrap().as_bytes(), before_tone);
        assert_eq!(
            pod_preset::PodTone::parse(before_tone.clone()).unwrap(),
            *app.pod_library.tone(1).unwrap()
        );
        assert!(!app.pod_library.is_dirty(0));
        assert!(app.pod_library.is_dirty(1));
        assert_eq!(app.pod_bundle.as_ref().unwrap().as_bytes(), before_bundle);
        assert!(!app.pod_bundle.as_ref().unwrap().is_dirty());
        assert_eq!(app.pod_amp_model_pending, pending);

        app.pod_selected = Some(1);
        app.pod_library
            .set_amp_model_choice(1, pod_preset::PodAmpModelChoice::Line6Clean)
            .unwrap();
        recording_frame_tall(&context, &mut app, vec![]);
        assert_eq!(app.pod_amp_model_pending, None);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), first);
        let clean_model_id = app.pod_library.tone(1).unwrap().amp_model_id().unwrap();
        app.pod_amp_model_pending =
            Some((1, clean_model_id, pod_preset::PodAmpModelChoice::Model2002e));
        app.pod_library.revert_unsaved(1).unwrap();
        recording_frame_tall(&context, &mut app, vec![]);
        assert_eq!(app.pod_amp_model_pending, None);
        assert_eq!(app.pod_library.tone(1).unwrap().as_bytes(), baseline);
        assert!(!app.pod_library.is_dirty(1));
        assert_eq!(app.pod_bundle.as_ref().unwrap().as_bytes(), before_bundle);
    }

    #[test]
    fn offline_pod_bundle_window_navigates_edits_saves_and_guards_quit() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let directory = tempfile::tempdir().unwrap();
        let original_path = directory.path().join("source.vwb");
        let blocked_path = directory.path().join("occupied.vwb");
        let saved_path = directory.path().join("saved.vwb");
        let first = sample_drive_pod_tone();
        let second = sample_pod_tone();
        let original = sample_pod_bundle(&[first.clone(), second.clone()]);
        std::fs::write(&original_path, &original).unwrap();
        std::fs::write(&blocked_path, b"preserve existing").unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_open: [Some(original_path.clone())].into(),
            pod_save: [Some(blocked_path.clone()), Some(saved_path.clone())].into(),
            ..Default::default()
        });

        click_pod(&context, &mut app, "open-pod");
        click_pod(&context, &mut app, "load-pod");
        assert_eq!(app.pod_bundle.as_ref().unwrap().bank_count(), 2);
        assert_eq!(app.pod_library.len(), 2);
        assert_eq!(app.pod_bundle.as_ref().unwrap().bank_name(1), Some("B"));
        let second_row = app.pod_rows[&1].center();
        click_pod_at(&context, &mut app, second_row);
        assert_eq!(app.pod_selected, Some(1));
        assert_eq!(app.pod_library.tone(1).unwrap().as_bytes(), second);
        let first_row = app.pod_rows[&0].center();
        click_pod_at(&context, &mut app, first_row);
        drag_pod_drive_slider(&context, &mut app);
        let selected_value = app
            .pod_library
            .tone(0)
            .unwrap()
            .amp_drive_fraction()
            .unwrap();
        assert!(app.pod_bundle.as_ref().unwrap().is_dirty());
        assert!(app.pod_library.is_dirty(0));
        assert!(!app.pod_library.is_dirty(1));

        click_pod(&context, &mut app, "save-pod");
        assert_eq!(std::fs::read(&blocked_path).unwrap(), b"preserve existing");
        assert!(app.pod_bundle.as_ref().unwrap().is_dirty());
        click_pod(&context, &mut app, "save-pod");
        let saved = std::fs::read(&saved_path).unwrap();
        let reopened = pod_preset::PodBundleLibrary::parse(saved.clone()).unwrap();
        assert_eq!(
            reopened.tone(0, 0).unwrap().amp_drive_fraction().unwrap(),
            selected_value
        );
        assert_eq!(reopened.tone(1, 0).unwrap().as_bytes(), second);
        assert_eq!(saved, app.pod_bundle.as_ref().unwrap().as_bytes());
        assert_eq!(std::fs::read(&original_path).unwrap(), original);
        assert!(!app.pod_bundle.as_ref().unwrap().is_dirty());
        assert!(!app.pod_library.any_dirty());

        app.edit_pod_tone(0, |tone| {
            tone.set_amp_control_fraction(pod_preset::AmpControl::Drive, 0.2)
        })
        .unwrap();
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "discard");
        assert!(!app.pod_bundle.as_ref().unwrap().is_dirty());
        assert!(!app.pod_library.any_dirty());
        assert_eq!(app.pod_bundle.as_ref().unwrap().as_bytes(), saved);
        assert!(app.read_worker.is_none());
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn pod_membership_controls_are_folder_only_and_refresh_selection_and_pending_choices() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let directory = tempfile::tempdir().unwrap();
        let source_path = directory.path().join("source.l6c");
        let saved_path = directory.path().join("saved.l6c");
        let fixed_tone = sample_drive_pod_tone();
        let folder_tone = sample_pod_tone();
        let original = sample_pod_bundle(&[fixed_tone.clone(), folder_tone.clone()]);
        std::fs::write(&source_path, &original).unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_save: [Some(saved_path.clone())].into(),
            ..Default::default()
        });
        app.pod_open = true;
        app.load_pod_bundle_bytes(original.clone()).unwrap();
        recording_frame(&context, &mut app, vec![]);
        assert!(!app.pod_buttons.contains_key("pod-duplicate-entry"));
        app.pod_selected = Some(1);
        app.refresh_selected_pod_name();
        assert_eq!(app.pod_selected, Some(1));
        recording_frame(&context, &mut app, vec![]);
        assert!(app.pod_buttons.contains_key("pod-duplicate-entry"));

        app.pod_cab_pending = Some((
            1,
            pod_preset::PodCabModelChoice::from_model_id(0x0102_0016).unwrap(),
        ));
        click_pod(&context, &mut app, "pod-duplicate-entry");
        let bundle = app.pod_bundle.as_ref().unwrap();
        assert_eq!(bundle.entry_count(), 3);
        assert_eq!(app.pod_library.len(), 3);
        assert_eq!(app.pod_selected, Some(2));
        assert_eq!(app.pod_library.tone(2).unwrap().as_bytes(), folder_tone);
        assert!(bundle.is_dirty());
        assert_ne!(
            app.pod_cab_pending,
            Some((
                2,
                pod_preset::PodCabModelChoice::from_model_id(0x0102_0016).unwrap()
            ))
        );
        assert!(app.pod_cab_pending.is_none_or(|(index, _)| index != 1));
        assert_eq!(std::fs::read(&source_path).unwrap(), original);
        assert!(app.load_pod_bundle_bytes(original.clone()).is_err());
        assert_eq!(app.pod_bundle.as_ref().unwrap().entry_count(), 3);

        click_pod(&context, &mut app, "save-pod");
        let saved = std::fs::read(&saved_path).unwrap();
        let reopened = pod_preset::PodBundleLibrary::parse(saved.clone()).unwrap();
        assert_eq!(reopened.entry_count(), 3);
        assert_eq!(reopened.tone(1, 1).unwrap().as_bytes(), folder_tone);
        assert_eq!(saved, app.pod_bundle.as_ref().unwrap().as_bytes());
        assert!(!app.pod_bundle.as_ref().unwrap().is_dirty());
        assert_eq!(std::fs::read(&source_path).unwrap(), original);

        click_pod(&context, &mut app, "pod-move-entry-up");
        assert_eq!(app.pod_selected, Some(1));
        assert_eq!(app.pod_bundle.as_ref().unwrap().as_bytes(), saved);
        assert!(!app.pod_bundle.as_ref().unwrap().is_dirty());

        click_pod(&context, &mut app, "pod-remove-entry");
        assert_eq!(app.pod_bundle.as_ref().unwrap().entry_count(), 2);
        assert_eq!(app.pod_selected, Some(1));
        assert!(app.pod_bundle.as_ref().unwrap().is_dirty());
        assert_ne!(
            app.pod_cab_pending,
            Some((
                1,
                pod_preset::PodCabModelChoice::from_model_id(0x0102_0016).unwrap()
            ))
        );
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "discard");
        assert!(!app.pod_bundle.as_ref().unwrap().is_dirty());
        assert_eq!(app.pod_bundle.as_ref().unwrap().as_bytes(), saved);
        assert_eq!(app.pod_selected, Some(1));
        assert_eq!(
            app.pod_name,
            pod_preset::PodTone::parse(folder_tone.clone())
                .unwrap()
                .name()
        );
        assert_eq!(std::fs::read(&source_path).unwrap(), original);
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn fixed_bank_reorder_controls_keep_selection_save_and_quit_state_offline() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let first = as_fixed_bank_tone(sample_drive_pod_tone());
        let second = as_fixed_bank_tone(sample_pod_tone());
        let original = sample_fixed_pod_bank(&[first.clone(), second.clone()]);
        let directory = tempfile::tempdir().unwrap();
        let collision = directory.path().join("occupied.vwb");
        let saved = directory.path().join("saved.vwb");
        std::fs::write(&collision, b"retain this destination").unwrap();

        app.pod_open = true;
        app.load_pod_bundle_bytes(original.clone()).unwrap();
        recording_frame(&context, &mut app, vec![]);
        assert!(app.pod_buttons.contains_key("pod-move-entry-up"));
        assert!(app.pod_buttons.contains_key("pod-move-entry-down"));
        assert!(!app.pod_buttons.contains_key("pod-duplicate-entry"));
        assert!(!app.pod_buttons.contains_key("pod-remove-entry"));

        app.pod_selected = Some(1);
        app.refresh_selected_pod_name();
        app.pod_cab_pending = Some((
            1,
            pod_preset::PodCabModelChoice::from_model_id(0x0102_0016).unwrap(),
        ));
        let pending_before_rejected_action = app.pod_cab_pending;
        let original_bundle_bytes = app.pod_bundle.as_ref().unwrap().as_bytes().to_vec();
        let original_library_tones: Vec<_> = (0..app.pod_library.len())
            .map(|index| app.pod_library.tone(index).unwrap().as_bytes().to_vec())
            .collect();
        app.apply_pod_membership_action(PodMembershipAction::MoveDown, 99, 1);
        assert_eq!(
            app.pod_bundle.as_ref().unwrap().as_bytes(),
            original_bundle_bytes
        );
        assert_eq!(app.pod_selected, Some(1));
        assert_eq!(app.pod_cab_pending, pending_before_rejected_action);
        assert_eq!(
            (0..app.pod_library.len())
                .map(|index| { app.pod_library.tone(index).unwrap().as_bytes().to_vec() })
                .collect::<Vec<_>>(),
            original_library_tones
        );

        click_pod(&context, &mut app, "pod-move-entry-up");
        let bundle = app.pod_bundle.as_ref().unwrap();
        assert_eq!(bundle.tone(0, 0).unwrap().as_bytes(), second);
        assert_eq!(bundle.tone(0, 1).unwrap().as_bytes(), first);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), second);
        assert_eq!(app.pod_selected, Some(0));
        assert_eq!(
            app.pod_name,
            pod_preset::PodTone::parse(second.clone()).unwrap().name()
        );
        assert!(bundle.is_dirty());
        assert!(app.pod_cab_pending.is_none());
        assert!(app.pod_message.contains("fixed bank"));
        assert!(std::fs::read(&collision).unwrap() == b"retain this destination");

        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pending.is_none());
        assert!(app.pod_bundle.as_ref().unwrap().is_dirty());

        app.request_close(&context);
        app.dialogs.queue_pod_save(Some(collision.clone()));
        click_prompt(&context, &mut app, "save");
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        assert!(app.pod_bundle.as_ref().unwrap().is_dirty());
        assert_eq!(
            std::fs::read(&collision).unwrap(),
            b"retain this destination"
        );

        app.dialogs.queue_pod_save(Some(saved.clone()));
        click_prompt(&context, &mut app, "save");
        assert!(app.pending.is_none());
        assert!(!app.pod_bundle.as_ref().unwrap().is_dirty());
        assert_eq!(
            pod_preset::PodBundleLibrary::parse(std::fs::read(&saved).unwrap())
                .unwrap()
                .as_bytes(),
            app.pod_bundle.as_ref().unwrap().as_bytes()
        );

        click_pod(&context, &mut app, "pod-move-entry-down");
        assert!(app.pod_bundle.as_ref().unwrap().is_dirty());
        assert_eq!(app.pod_selected, Some(1));
        app.permit_close = false;
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "discard");
        assert!(app.pending.is_none());
        assert!(!app.pod_bundle.as_ref().unwrap().is_dirty());
        assert_eq!(
            app.pod_bundle.as_ref().unwrap().as_bytes(),
            std::fs::read(&saved).unwrap()
        );
        assert_eq!(app.pod_selected, Some(1));
        assert_eq!(app.pod_library.len(), 2);
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn reverting_pod_membership_before_recording_quit_cancel_keeps_selection_valid() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let fixed_tone = sample_drive_pod_tone();
        let folder_tone = sample_pod_tone();
        let original = sample_pod_bundle(&[fixed_tone, folder_tone.clone()]);
        app.pod_open = true;
        app.load_pod_bundle_bytes(original).unwrap();
        app.pod_selected = Some(1);
        app.refresh_selected_pod_name();
        recording_frame(&context, &mut app, vec![]);

        click_pod(&context, &mut app, "pod-duplicate-entry");
        assert_eq!(app.pod_selected, Some(2));
        assert_eq!(app.pod_bundle.as_ref().unwrap().entry_count(), 3);

        prepare_quit_take(&mut app, ClockSelection::Integer(48_000));
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "discard");
        assert_eq!(app.pod_bundle.as_ref().unwrap().entry_count(), 2);
        assert!(!app.pod_bundle.as_ref().unwrap().is_dirty());
        assert_eq!(app.pod_selected, Some(1));
        assert_eq!(
            app.pod_name,
            pod_preset::PodTone::parse(folder_tone).unwrap().name()
        );
        assert!(matches!(app.pending, Some(Pending::QuitRecording)));

        click_prompt(&context, &mut app, "cancel");
        assert!(app.pending.is_none());
        assert!(app.recorder.has_unsaved_take());
        assert_eq!(app.pod_selected, Some(1));
        recording_frame(&context, &mut app, vec![]);
        assert!(app.pod_rows.contains_key(&1));
        assert!(app.pod_buttons.contains_key("pod-duplicate-entry"));
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn podxt_eq_gui_edits_preserve_maximum_frequency_and_gain_on_render() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("eq-endpoints.l6t");
        let original = sample_pod_eq_tone_with_words(0x3f80_0000, 0x3f80_0000);
        std::fs::write(&source, &original).unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_open: [Some(source)].into(),
            ..Default::default()
        });
        app.pod_open = true;
        app.load_pod_tone();
        recording_frame(&context, &mut app, vec![]);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
        assert!(!app.pod_library.is_dirty(0));
        let band = pod_preset::PodxtEqBand::One;
        drag_pod_eq_gain_slider(&context, &mut app, band);
        let (frequency, _) = app
            .pod_library
            .tone(0)
            .unwrap()
            .podxt_eq_band_raw_values(band)
            .unwrap();
        assert_eq!(frequency.to_bits(), 0x3f80_0000);
        assert!(app.pod_library.is_dirty(0));
        app.pod_library.revert_unsaved(0).unwrap();
        recording_frame(&context, &mut app, vec![]);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
        assert!(!app.pod_library.is_dirty(0));
        drag_pod_eq_frequency_slider(&context, &mut app, band);
        let (_, gain) = app
            .pod_library
            .tone(0)
            .unwrap()
            .podxt_eq_band_raw_values(band)
            .unwrap();
        assert_eq!(gain.to_bits(), 0x3f80_0000);
        assert!(app.pod_library.is_dirty(0));
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn podxt_eq_reset_stages_applies_discards_and_resolves_quit_before_dirty_prompt() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let mut original = sample_pod_eq_tone();
        original[8..12].copy_from_slice(b"L6PA");
        let index = app.pod_library.import_bytes(original.clone()).unwrap();
        app.pod_selected = Some(index);
        app.pod_open = true;

        recording_frame(&context, &mut app, vec![]);
        assert!(app.pod_buttons.contains_key("pod-eq-reset-stage"));
        click_pod_button_tall(&context, &mut app, "pod-eq-reset-stage");
        assert_eq!(app.pod_library.tone(index).unwrap().as_bytes(), original);
        assert!(!app.pod_library.is_dirty(index));
        assert!(app.pod_eq_reset_pending.is_some());

        recording_frame(&context, &mut app, vec![]);
        click_pod_button_tall(&context, &mut app, "pod-eq-reset-discard");
        assert_eq!(app.pod_library.tone(index).unwrap().as_bytes(), original);
        assert!(!app.pod_library.is_dirty(index));
        assert!(app.pod_eq_reset_pending.is_none());

        click_pod_button_tall(&context, &mut app, "pod-eq-reset-stage");
        recording_frame(&context, &mut app, vec![]);
        click_pod_button_tall(&context, &mut app, "pod-eq-reset-apply");
        let edited = app.pod_library.tone(index).unwrap().as_bytes();
        assert!(app.pod_library.is_dirty(index));
        assert!(app.pod_eq_reset_pending.is_none());
        let changed: Vec<_> = original
            .iter()
            .zip(edited)
            .enumerate()
            .filter_map(|(offset, (before, after))| (before != after).then_some(offset))
            .collect();
        assert!(!changed.is_empty());
        assert!(changed.len() <= 8 * 4);
        assert_eq!(
            pod_preset::PodxtEqBand::ALL.map(|band| {
                let (frequency, gain) = app
                    .pod_library
                    .tone(index)
                    .unwrap()
                    .podxt_eq_band_raw_values(band)
                    .unwrap();
                [frequency.to_bits(), gain.to_bits()]
            }),
            [
                [0x3e00_0000, 0x3f01_0204],
                [0x3eec_0000, 0x3f01_0204],
                [0x3ef0_0000, 0x3f01_0204],
                [0x3f5c_0000, 0x3f01_0204],
            ]
        );
        assert!(app.pod_message.contains("Nothing was sent"));

        app.pod_library.revert_unsaved(index).unwrap();
        recording_frame(&context, &mut app, vec![]);
        click_pod_button_tall(&context, &mut app, "pod-eq-reset-stage");
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        prompt_frame(&context, &mut app, vec![]);
        click_prompt(&context, &mut app, "eq-reset-quit-keep");
        assert!(app.pending.is_none());
        assert_eq!(app.pod_library.tone(index).unwrap().as_bytes(), original);
        assert!(!app.pod_library.is_dirty(index));

        recording_frame(&context, &mut app, vec![]);
        click_pod_button_tall(&context, &mut app, "pod-eq-reset-stage");
        app.request_close(&context);
        prompt_frame(&context, &mut app, vec![]);
        click_prompt(&context, &mut app, "eq-reset-quit-apply");
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        assert!(app.pod_library.is_dirty(index));
        assert!(app.pod_eq_reset_pending.is_none());
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pending.is_none());
        assert!(app.pod_library.is_dirty(index));
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn podxt_eq_staged_in_bundle_cannot_cross_cancelled_or_successful_document_replacement() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let directory = tempfile::tempdir().unwrap();
        let rejected_path = directory.path().join("rejected.l6t");
        let replacement_path = directory.path().join("replacement.l6t");
        std::fs::write(&rejected_path, b"not a tone").unwrap();

        let mut original = sample_pod_eq_tone();
        original[8..12].copy_from_slice(b"L6PA");
        let mut replacement = original.clone();
        replacement[28..32].copy_from_slice(b"NEW!");
        assert_ne!(original, replacement);
        assert_eq!(
            &original[32..],
            &replacement[32..],
            "EQ bytes are identical"
        );
        std::fs::write(&replacement_path, &replacement).unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_open: [None, Some(rejected_path), Some(replacement_path)].into(),
            ..Default::default()
        });
        let bundle_bytes = sample_pod_bundle(std::slice::from_ref(&original));
        app.load_pod_bundle_bytes(bundle_bytes.clone()).unwrap();
        app.pod_open = true;

        recording_frame_tall(&context, &mut app, vec![]);
        click_pod_button_tall(&context, &mut app, "pod-eq-reset-stage");
        let pending = app.pod_eq_reset_pending.clone().unwrap();
        assert_eq!(pending.document_generation, app.pod_document_generation);
        assert_eq!(app.pod_bundle.as_ref().unwrap().as_bytes(), bundle_bytes);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
        assert!(!app.pod_library.is_dirty(0));

        // A cancelled dialog and a rejected file keep both the bundle and staged action.
        click_pod_button_tall(&context, &mut app, "load-pod");
        assert_eq!(app.pod_eq_reset_pending.as_ref(), Some(&pending));
        assert_eq!(app.pod_bundle.as_ref().unwrap().as_bytes(), bundle_bytes);
        click_pod_button_tall(&context, &mut app, "load-pod");
        assert_eq!(app.pod_eq_reset_pending.as_ref(), Some(&pending));
        assert_eq!(app.pod_bundle.as_ref().unwrap().as_bytes(), bundle_bytes);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);

        // Replacing with a different Library document at the same index and EQ context
        // must invalidate the old stage. A later Apply/quit cannot touch this tone.
        click_pod_button_tall(&context, &mut app, "load-pod");
        assert!(app.pod_bundle.is_none());
        assert!(app.pod_eq_reset_pending.is_none());
        assert!(app.pod_document_generation > pending.document_generation);
        assert_eq!(app.pod_selected, Some(0));
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), replacement);
        assert!(!app.pod_library.is_dirty(0));

        app.pod_buttons.clear();
        recording_frame_tall(&context, &mut app, vec![]);
        assert!(app.pod_buttons.contains_key("pod-eq-reset-stage"));
        assert!(!app.pod_buttons.contains_key("pod-eq-reset-apply"));
        app.request_close(&context);
        app.pending_buttons.clear();
        prompt_frame(&context, &mut app, vec![]);
        assert!(!app.pending_buttons.contains_key("eq-reset-quit-apply"));
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), replacement);
        assert!(!app.pod_library.is_dirty(0));
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn podxt_eq_and_amp_stages_resolve_against_a_loaded_bundle() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("eq-amp.vwb");
        let mut original = sample_pod_amp_eq_tone();
        original[8..12].copy_from_slice(b"L6PA");
        let bundle_bytes = sample_pod_bundle(std::slice::from_ref(&original));
        std::fs::write(&path, &bundle_bytes).unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_open: [Some(path)].into(),
            ..Default::default()
        });
        app.load_pod_tone();
        recording_frame_tall(&context, &mut app, vec![]);
        assert!(app.pod_bundle.is_some(), "{}", app.pod_message);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);

        click_pod_button_tall(&context, &mut app, "pod-eq-reset-stage");
        choose_amp_model_popup(&context, &mut app, 0x0002_002e);
        let amp_stage = app.pod_amp_model_pending;
        assert!(amp_stage.is_some());
        assert!(app.pod_eq_reset_pending.is_some());
        assert_eq!(app.pod_bundle.as_ref().unwrap().as_bytes(), bundle_bytes);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
        assert!(!app.pod_dirty());

        // Keep then discard the EQ stage through the actual quit dialog. Discarding
        // EQ continues into the independently staged AMP choice.
        app.request_close(&context);
        click_prompt(&context, &mut app, "eq-reset-quit-keep");
        assert!(app.pending.is_none());
        assert!(app.pod_eq_reset_pending.is_some());
        assert_eq!(app.pod_amp_model_pending, amp_stage);
        app.request_close(&context);
        click_prompt(&context, &mut app, "eq-reset-quit-discard");
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        assert!(app.pod_eq_reset_pending.is_none());
        assert_eq!(app.pod_amp_model_pending, amp_stage);
        click_prompt(&context, &mut app, "amp-keep-editing");
        assert!(app.pending.is_none());
        assert_eq!(app.pod_amp_model_pending, amp_stage);
        assert_eq!(app.pod_bundle.as_ref().unwrap().as_bytes(), bundle_bytes);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
        assert!(!app.pod_dirty());

        // Apply EQ, then exercise the bundle cancel/discard path while the AMP
        // choice remains independently staged.
        click_pod_button_tall(&context, &mut app, "pod-eq-reset-stage");
        app.request_close(&context);
        click_prompt(&context, &mut app, "eq-reset-quit-apply");
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        assert!(app.pod_eq_reset_pending.is_none());
        assert_eq!(app.pod_amp_model_pending, amp_stage);
        assert_ne!(app.pod_library.tone(0).unwrap().as_bytes(), original);
        assert!(app.pod_bundle.as_ref().unwrap().is_dirty());
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pending.is_none());
        assert!(app.pod_amp_model_pending.is_some());
        assert!(app.pod_bundle.as_ref().unwrap().is_dirty());

        app.request_close(&context);
        click_prompt(&context, &mut app, "discard");
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
        assert_eq!(app.pod_bundle.as_ref().unwrap().as_bytes(), bundle_bytes);
        assert_eq!(app.pod_amp_model_pending, amp_stage);
        click_prompt(&context, &mut app, "amp-keep-editing");
        assert!(app.pending.is_none());
        assert_eq!(app.pod_amp_model_pending, amp_stage);
        app.request_close(&context);
        click_prompt(&context, &mut app, "amp-discard");
        assert!(app.permit_close);
        assert_eq!(app.pod_amp_model_pending, None);

        // Exercise the Apply rejection path after a real bundle load. Removing
        // the selected bundle row behind the Library forces atomic publication
        // failure while the staged edit still has a valid source context.
        let (context, mut app) = self::app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("eq-amp-failed-publication.vwb");
        let mut original = sample_pod_amp_eq_tone();
        original[8..12].copy_from_slice(b"L6PA");
        let bundle_bytes = sample_pod_bundle(&[original.clone(), original.clone()]);
        std::fs::write(&path, &bundle_bytes).unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_open: [Some(path)].into(),
            ..Default::default()
        });
        app.load_pod_tone();
        recording_frame_tall(&context, &mut app, vec![]);
        app.pod_selected = Some(1);
        assert_eq!(app.pod_library.tone(1).unwrap().as_bytes(), original);
        click_pod_button_tall(&context, &mut app, "pod-eq-reset-stage");
        choose_amp_model_popup(&context, &mut app, 0x0002_002e);
        let amp_stage = app.pod_amp_model_pending;
        app.pod_bundle.as_mut().unwrap().remove_entry(1, 0).unwrap();
        let failed_publication_bundle = app.pod_bundle.as_ref().unwrap().as_bytes().to_vec();
        let parsed_before = app.pod_library.tone(1).unwrap().models().to_vec();
        app.request_close(&context);
        click_prompt(&context, &mut app, "eq-reset-quit-apply");
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        assert!(app.pod_eq_reset_pending.is_some());
        assert_eq!(app.pod_amp_model_pending, amp_stage);
        assert_eq!(app.pod_library.tone(1).unwrap().as_bytes(), original);
        assert_eq!(app.pod_library.tone(1).unwrap().models(), parsed_before);
        assert!(!app.pod_library.is_dirty(1));
        assert_eq!(
            app.pod_bundle.as_ref().unwrap().as_bytes(),
            failed_publication_bundle
        );
        assert!(app.pod_bundle.as_ref().unwrap().is_dirty());
        assert!(!app.permit_close);
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn podxt_eq_graph_slider_edits_two_words_and_preserves_failed_save_state() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("eq.l6t");
        let collision = directory.path().join("occupied.l6t");
        let saved = directory.path().join("saved.l6t");
        let original = sample_pod_eq_tone();
        std::fs::write(&source, &original).unwrap();
        std::fs::write(&collision, b"keep this file").unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_open: [Some(source)].into(),
            pod_save: [None, Some(collision.clone()), Some(saved.clone())].into(),
            ..Default::default()
        });
        app.pod_open = true;
        app.load_pod_tone();
        recording_frame(&context, &mut app, vec![]);
        assert_eq!(app.pod_eq_frequency_sliders.len(), 4);
        assert_eq!(app.pod_eq_gain_sliders.len(), 4);
        let before = app.pod_library.tone(0).unwrap().as_bytes().to_vec();
        drag_pod_eq_frequency_slider(&context, &mut app, pod_preset::PodxtEqBand::One);
        assert!(app.pod_library.is_dirty(0));
        let after = app.pod_library.tone(0).unwrap().as_bytes();
        let changed: Vec<_> = before
            .iter()
            .zip(after)
            .enumerate()
            .filter_map(|(i, (a, b))| (a != b).then_some(i))
            .collect();
        assert!(!changed.is_empty());
        assert!(app.pod_message.contains("Nothing was sent"));

        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        prompt_frame(&context, &mut app, vec![]);
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pending.is_none());
        assert!(
            app.pod_library.is_dirty(0),
            "keep editing retains the applied EQ edit"
        );

        click_pod(&context, &mut app, "save-pod");
        assert!(
            app.pod_library.is_dirty(0),
            "cancel keeps the applied EQ edit"
        );
        click_pod(&context, &mut app, "save-pod");
        assert_eq!(std::fs::read(&collision).unwrap(), b"keep this file");
        assert!(
            app.pod_library.is_dirty(0),
            "collision keeps the applied EQ edit"
        );
        click_pod(&context, &mut app, "save-pod");
        assert!(!app.pod_library.is_dirty(0));
        let reopened = pod_preset::PodTone::parse(std::fs::read(saved).unwrap()).unwrap();
        assert_ne!(reopened.as_bytes(), before);
    }

    fn click_pod_window_close(context: &egui::Context, app: &mut Workbench) {
        click_pod(context, app, "show-variax");
    }

    fn pod_tone_name(app: &Workbench, index: usize) -> String {
        app.pod_library.tone(index).expect("tone").name().to_owned()
    }

    #[test]
    fn applied_pod_rename_blocks_quit_until_each_tone_is_saved_or_discarded() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let guitar = app.document.as_ref().unwrap().as_bytes().to_vec();
        let directory = tempfile::tempdir().unwrap();
        let tone_a = directory.path().join("a.tone");
        let tone_b = directory.path().join("b.tone");
        std::fs::write(&tone_a, sample_pod_tone()).unwrap();
        std::fs::write(&tone_b, sample_pod_tone()).unwrap();
        let blocked = directory.path().join("blocked.tone");
        std::fs::write(&blocked, b"keep").unwrap();
        let alpha = directory.path().join("alpha.tone");
        let beta = directory.path().join("beta.tone");
        let guitar_copy = directory.path().join("guitar-copy.vwm");
        app.dialogs = Box::new(FakeDialogs {
            pod_open: [Some(tone_a), Some(tone_b)].into(),
            pod_save: [
                None,
                Some(blocked.clone()),
                Some(alpha.clone()),
                Some(beta.clone()),
            ]
            .into(),
            save: [Some(guitar_copy.clone())].into(),
            ..Default::default()
        });
        click_pod(&context, &mut app, "open-pod");
        click_pod(&context, &mut app, "load-pod");
        app.pod_name = "Typed only".into();
        app.request_close(&context);
        assert!(app.pending.is_none());
        assert!(!app.permit_close);
        assert!(!app.pod_library.any_dirty());
        app.pod_name = "Alpha".into();
        click_pod(&context, &mut app, "apply-pod-name");
        click_pod(&context, &mut app, "load-pod");
        app.pod_name = "Beta".into();
        click_pod(&context, &mut app, "apply-pod-name");
        assert!(app.pod_library.is_dirty(0));
        assert!(app.pod_library.is_dirty(1));
        assert_eq!(pod_tone_name(&app, 0), "Alpha");
        assert_eq!(pod_tone_name(&app, 1), "Beta");
        assert!(!app.dirty());
        click_pod_window_close(&context, &mut app);
        assert!(!app.pod_open);
        assert_eq!(pod_tone_name(&app, 0), "Alpha");
        assert_eq!(pod_tone_name(&app, 1), "Beta");
        assert!(app.pod_library.is_dirty(0));
        assert!(app.pod_library.is_dirty(1));

        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        assert!(!app.permit_close);
        prompt_frame(&context, &mut app, vec![]);
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pending.is_none());
        assert!(!app.permit_close);
        assert_eq!(pod_tone_name(&app, 0), "Alpha");
        assert_eq!(pod_tone_name(&app, 1), "Beta");
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), guitar);

        app.request_close(&context);
        click_prompt(&context, &mut app, "save");
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        assert!(!app.permit_close);
        assert!(app.pod_message.contains("cancelled"));
        assert!(app.pod_library.is_dirty(0));
        assert!(app.pod_library.is_dirty(1));
        assert_eq!(pod_tone_name(&app, 0), "Alpha");
        click_prompt(&context, &mut app, "save");
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        assert!(!app.permit_close);
        assert!(!app.pod_message.starts_with("Saved a new copy"));
        assert_eq!(std::fs::read(&blocked).unwrap(), b"keep");
        assert!(app.pod_library.is_dirty(0));
        assert!(app.pod_library.is_dirty(1));
        assert_eq!(pod_tone_name(&app, 1), "Beta");
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pending.is_none());

        app.change(|preset| preset.rename("Guitar edit"));
        assert!(app.dirty());
        let guitar_edited = app.document.as_ref().unwrap().as_bytes().to_vec();
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::Quit)));
        assert!(!app.permit_close);
        prompt_frame(&context, &mut app, vec![]);
        click_prompt(&context, &mut app, "save");
        assert!(!app.dirty());
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        assert!(!app.permit_close);
        assert_eq!(std::fs::read(&guitar_copy).unwrap(), guitar_edited);
        assert_eq!(pod_tone_name(&app, 0), "Alpha");
        assert_eq!(pod_tone_name(&app, 1), "Beta");
        assert!(app.pod_library.is_dirty(0));
        assert!(app.pod_library.is_dirty(1));

        click_prompt(&context, &mut app, "save");
        assert!(!app.pod_library.is_dirty(0));
        assert!(app.pod_library.is_dirty(1));
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        assert!(!app.permit_close);
        assert_eq!(pod_tone_name(&app, 0), "Alpha");
        assert_eq!(pod_tone_name(&app, 1), "Beta");
        let saved_alpha = pod_preset::PodTone::parse(std::fs::read(&alpha).unwrap()).unwrap();
        assert_eq!(saved_alpha.name(), "Alpha");
        assert_eq!(saved_alpha.amp_model_id(), Some(0x1111_0002));
        assert!(!beta.exists());

        click_prompt(&context, &mut app, "save");
        assert!(!app.pod_library.any_dirty());
        assert!(app.permit_close);
        assert!(app.pending.is_none());
        assert_eq!(pod_tone_name(&app, 1), "Beta");
        let saved_beta = pod_preset::PodTone::parse(std::fs::read(&beta).unwrap()).unwrap();
        assert_eq!(saved_beta.name(), "Beta");
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), guitar_edited);

        app.permit_close = false;
        app.pod_selected = Some(0);
        app.pod_name = "Gamma".into();
        app.apply_pod_name();
        assert!(app.pod_library.is_dirty(0));
        assert!(!app.pod_library.is_dirty(1));
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "discard");
        assert_eq!(pod_tone_name(&app, 0), "Alpha");
        assert_eq!(pod_tone_name(&app, 1), "Beta");
        assert!(!app.pod_library.any_dirty());
        assert!(app.permit_close);
        assert!(app.pending.is_none());
        assert!(app.pod_message.contains("Discarded"));
        assert_eq!(app.document.as_ref().unwrap().as_bytes(), guitar_edited);
        assert!(app.read_worker.is_none());
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn amp_stomp_mod_delay_and_reverb_sliders_edit_selected_tone_and_quit_guard_keeps_or_saves_it()
    {
        let (context, mut app) = app();
        context.style_mut(|style| {
            style.animation_time = 0.0;
            style.spacing.slider_width = 220.0;
        });
        let directory = tempfile::tempdir().unwrap();
        let admitted_path = directory.path().join("admitted-drive.tone");
        let unavailable_path = directory.path().join("unavailable.tone");
        let blocked_path = directory.path().join("blocked-copy.tone");
        let saved_path = directory.path().join("drive-copy.tone");
        let admitted_bytes = sample_drive_pod_tone();
        let unavailable_bytes = sample_pod_tone();
        std::fs::write(&admitted_path, &admitted_bytes).unwrap();
        std::fs::write(&unavailable_path, &unavailable_bytes).unwrap();
        std::fs::write(&blocked_path, b"keep existing destination").unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_open: [Some(admitted_path), Some(unavailable_path)].into(),
            pod_save: [Some(blocked_path.clone()), Some(saved_path.clone())].into(),
            ..Default::default()
        });

        click_pod(&context, &mut app, "open-pod");
        click_pod(&context, &mut app, "load-pod");
        click_pod(&context, &mut app, "load-pod");
        assert_eq!(app.pod_library.len(), 2, "{}", app.pod_message);
        assert_eq!(app.pod_selected, Some(1));
        recording_frame(&context, &mut app, vec![]);
        assert!(app.pod_drive_slider.is_none());
        assert!(app.pod_stomp_sliders.is_empty());
        assert!(
            app.pod_library
                .tone(1)
                .unwrap()
                .amp_drive_fraction()
                .is_err()
        );
        assert!(!app.pod_library.is_dirty(0));
        assert!(!app.pod_library.is_dirty(1));

        let first_row = app.pod_rows[&0].center();
        click_pod_at(&context, &mut app, first_row);
        assert_eq!(app.pod_selected, Some(0));
        recording_frame(&context, &mut app, vec![]);
        assert!(app.pod_drive_slider.is_some());
        let stomp_controls = [
            pod_preset::StompControl::Drive,
            pod_preset::StompControl::Gain,
            pod_preset::StompControl::Tone,
        ];
        assert_eq!(app.pod_stomp_sliders.len(), stomp_controls.len());
        assert_eq!(app.pod_mod_sliders.len(), 3);
        assert_eq!(app.pod_reverb_sliders.len(), 2);
        assert_eq!(app.pod_delay_sliders.len(), 3);
        let original_drive = app
            .pod_library
            .tone(0)
            .unwrap()
            .amp_drive_fraction()
            .unwrap();
        let original_stomp: Vec<_> = stomp_controls
            .into_iter()
            .map(|control| {
                app.pod_library
                    .tone(0)
                    .unwrap()
                    .stomp_control_fraction(control)
                    .unwrap()
            })
            .collect();
        drag_pod_drive_slider(&context, &mut app);
        for control in stomp_controls {
            drag_pod_stomp_slider(&context, &mut app, control);
        }
        drag_pod_mod_slider(&context, &mut app, pod_preset::ModControl::Depth);
        drag_pod_reverb_slider(&context, &mut app, pod_preset::ReverbControl::Dwell);
        drag_pod_delay_slider(&context, &mut app, pod_preset::DelayControl::Feedback);
        let edited_drive = app
            .pod_library
            .tone(0)
            .unwrap()
            .amp_drive_fraction()
            .unwrap();
        assert_ne!(edited_drive.to_bits(), original_drive.to_bits());
        assert!((0.0..=1.0).contains(&edited_drive));
        let edited_stomp: Vec<_> = stomp_controls
            .into_iter()
            .map(|control| {
                app.pod_library
                    .tone(0)
                    .unwrap()
                    .stomp_control_fraction(control)
                    .unwrap()
            })
            .collect();
        for (edited, original) in edited_stomp.iter().zip(&original_stomp) {
            assert_ne!(edited.to_bits(), original.to_bits());
            assert!((0.0..=1.0).contains(edited));
        }
        let edited_mod = app
            .pod_library
            .tone(0)
            .unwrap()
            .mod_control_fraction(pod_preset::ModControl::Depth)
            .unwrap();
        assert!((0.0..=1.0).contains(&edited_mod));
        assert_ne!(edited_mod.to_bits(), 0.2f32.to_bits());
        let edited_reverb = app
            .pod_library
            .tone(0)
            .unwrap()
            .reverb_control_fraction(pod_preset::ReverbControl::Dwell)
            .unwrap();
        assert!((0.0..=1.0).contains(&edited_reverb));
        assert_ne!(edited_reverb.to_bits(), 0.3f32.to_bits());
        let edited_delay = app
            .pod_library
            .tone(0)
            .unwrap()
            .delay_control_fraction(pod_preset::DelayControl::Feedback)
            .unwrap();
        assert!((0.0..=1.0).contains(&edited_delay));
        assert_ne!(edited_delay.to_bits(), 0.2f32.to_bits());
        assert!(app.pod_library.is_dirty(0));
        assert!(!app.pod_library.is_dirty(1));
        assert_eq!(
            app.pod_library.tone(1).unwrap().as_bytes(),
            unavailable_bytes
        );

        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pending.is_none());
        assert!(app.pod_library.is_dirty(0));
        assert!(!app.pod_library.is_dirty(1));
        assert_eq!(
            app.pod_library
                .tone(0)
                .unwrap()
                .amp_drive_fraction()
                .unwrap()
                .to_bits(),
            edited_drive.to_bits()
        );
        for (control, edited) in stomp_controls.into_iter().zip(&edited_stomp) {
            assert_eq!(
                app.pod_library
                    .tone(0)
                    .unwrap()
                    .stomp_control_fraction(control)
                    .unwrap()
                    .to_bits(),
                edited.to_bits()
            );
        }

        app.request_close(&context);
        click_prompt(&context, &mut app, "save");
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        assert!(!app.permit_close);
        assert!(app.pod_library.is_dirty(0));
        assert!(!app.pod_library.is_dirty(1));
        assert_eq!(
            std::fs::read(&blocked_path).unwrap(),
            b"keep existing destination"
        );

        click_prompt(&context, &mut app, "save");
        assert!(app.pending.is_none());
        assert!(app.permit_close);
        assert!(!app.pod_library.any_dirty());
        let saved_bytes = std::fs::read(saved_path).unwrap();
        let reopened = pod_preset::PodTone::parse(saved_bytes).unwrap();
        assert_eq!(
            reopened.amp_drive_fraction().unwrap().to_bits(),
            edited_drive.to_bits()
        );
        for (control, edited) in stomp_controls.into_iter().zip(&edited_stomp) {
            assert_eq!(
                reopened.stomp_control_fraction(control).unwrap().to_bits(),
                edited.to_bits()
            );
        }
        assert_eq!(
            reopened
                .mod_control_fraction(pod_preset::ModControl::Depth)
                .unwrap()
                .to_bits(),
            edited_mod.to_bits()
        );
        assert_eq!(
            reopened
                .reverb_control_fraction(pod_preset::ReverbControl::Dwell)
                .unwrap()
                .to_bits(),
            edited_reverb.to_bits()
        );
        assert_eq!(
            reopened
                .delay_control_fraction(pod_preset::DelayControl::Feedback)
                .unwrap()
                .to_bits(),
            edited_delay.to_bits()
        );
        assert_eq!(
            app.pod_library.tone(1).unwrap().as_bytes(),
            unavailable_bytes
        );
        assert!(!app.document.as_ref().unwrap().as_bytes().is_empty());
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn stomp_harmony_gui_stages_until_apply_and_keeps_the_other_selector_independent() {
        let (context, mut app) = app();
        let directory = tempfile::tempdir().unwrap();
        let blocked = directory.path().join("existing-harmony-copy.l6t");
        let saved = directory.path().join("harmony-copy.l6t");
        std::fs::write(&blocked, b"keep existing destination").unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_save: [Some(blocked.clone()), Some(saved.clone())].into(),
            ..Default::default()
        });
        app.pod_open = true;
        app.pod_selected = Some(
            app.pod_library
                .import_bytes(sample_stomp_harmony_pod_tone())
                .unwrap(),
        );
        app.pod_library
            .import_bytes(sample_stomp_harmony_pod_tone())
            .unwrap();
        recording_frame_tall(&context, &mut app, vec![]);
        let original = app.pod_library.tone(0).unwrap().as_bytes().to_vec();
        stage_stomp_harmony1_choice(
            &context,
            &mut app,
            pod_preset::StompHarmony1M335Selection::MinusOctave,
        );
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
        assert_eq!(
            app.pod_harmony_pending,
            Some((
                0,
                pod_preset::StompHarmonyControl::Harmony1M335,
                PendingStompHarmony::Harmony1M335(
                    pod_preset::StompHarmony1M335Selection::MinusOctave
                )
            ))
        );
        let second_row = app.pod_rows[&1].center();
        click_pod_at_tall(&context, &mut app, second_row);
        recording_frame_tall(&context, &mut app, vec![]);
        assert_eq!(app.pod_selected, Some(1));
        assert!(app.pod_harmony_pending.is_none());
        let first_row = app.pod_rows[&0].center();
        click_pod_at_tall(&context, &mut app, first_row);
        recording_frame_tall(&context, &mut app, vec![]);
        stage_stomp_harmony1_choice(
            &context,
            &mut app,
            pod_preset::StompHarmony1M335Selection::MinusOctave,
        );
        click_pod(&context, &mut app, "pod-harmony1-apply");
        let after_first = app.pod_library.tone(0).unwrap().as_bytes().to_vec();
        assert_ne!(after_first, original);
        assert!(app.pod_library.is_dirty(0));
        assert!(app.pod_harmony_pending.is_none());
        assert_eq!(
            app.pod_library
                .tone(0)
                .unwrap()
                .stomp_harmony_1457_selection()
                .unwrap(),
            pod_preset::StompHarmonyValue::Known(pod_preset::StompHarmony1457Selection::Fifth)
        );

        stage_stomp_harmony2_choice(
            &context,
            &mut app,
            pod_preset::StompHarmony1457Selection::Fourth,
        );
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), after_first);
        click_pod(&context, &mut app, "pod-harmony2-apply");
        let final_bytes = app.pod_library.tone(0).unwrap().as_bytes();
        assert_ne!(final_bytes, after_first);
        assert_eq!(
            app.pod_library
                .tone(0)
                .unwrap()
                .stomp_harmony_1457_selection()
                .unwrap(),
            pod_preset::StompHarmonyValue::Known(pod_preset::StompHarmony1457Selection::Fourth)
        );
        assert_eq!(
            app.pod_library
                .tone(0)
                .unwrap()
                .stomp_harmony_1m335_selection()
                .unwrap(),
            pod_preset::StompHarmonyValue::Known(
                pod_preset::StompHarmony1M335Selection::MinusOctave
            )
        );
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "save");
        assert_eq!(
            std::fs::read(&blocked).unwrap(),
            b"keep existing destination"
        );
        assert!(app.pod_library.is_dirty(0));
        assert!(!app.permit_close);
        click_prompt(&context, &mut app, "save");
        assert!(app.permit_close);
        let reopened = pod_preset::PodTone::parse(std::fs::read(saved).unwrap()).unwrap();
        assert_eq!(
            reopened.stomp_harmony_1457_selection().unwrap(),
            pod_preset::StompHarmonyValue::Known(pod_preset::StompHarmony1457Selection::Fourth)
        );
        assert_eq!(
            reopened.stomp_harmony_1m335_selection().unwrap(),
            pod_preset::StompHarmonyValue::Known(
                pod_preset::StompHarmony1M335Selection::MinusOctave
            )
        );
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn mod_note_gui_stages_per_tone_and_applies_only_on_request() {
        let (context, mut app) = app();
        app.pod_open = true;
        app.pod_selected = Some(
            app.pod_library
                .import_bytes(sample_delay_note_tone(0, 0))
                .unwrap(),
        );
        app.pod_library
            .import_bytes(sample_delay_note_tone(0, 0))
            .unwrap();
        recording_frame_tall(&context, &mut app, vec![]);
        let original = app.pod_library.tone(0).unwrap().as_bytes().to_vec();
        stage_mod_note_choice(&context, &mut app, pod_preset::ModNoteChoice::Quarter);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
        assert_eq!(
            app.pod_mod_pending,
            Some((0, PendingModEdit::Note(pod_preset::ModNoteChoice::Quarter)))
        );

        let second_row = app.pod_rows[&1].center();
        click_pod_at_tall(&context, &mut app, second_row);
        recording_frame_tall(&context, &mut app, vec![]);
        assert_eq!(app.pod_selected, Some(1));
        assert!(app.pod_mod_pending.is_none());
        let second_original = app.pod_library.tone(1).unwrap().as_bytes().to_vec();
        stage_mod_note_choice(&context, &mut app, pod_preset::ModNoteChoice::Quarter);
        click_pod(&context, &mut app, "pod-mod-discard");
        assert!(app.pod_mod_pending.is_none());
        assert_eq!(app.pod_library.tone(1).unwrap().as_bytes(), second_original);

        let first_row = app.pod_rows[&0].center();
        click_pod_at_tall(&context, &mut app, first_row);
        recording_frame_tall(&context, &mut app, vec![]);
        stage_mod_note_choice(&context, &mut app, pod_preset::ModNoteChoice::Quarter);
        click_pod(&context, &mut app, "pod-mod-apply");
        assert_eq!(
            app.pod_library.tone(0).unwrap().mod_note_value().unwrap(),
            pod_preset::ModNoteValue::Known(pod_preset::ModNoteChoice::Quarter)
        );
        assert_eq!(
            app.pod_library
                .tone(0)
                .unwrap()
                .mod_speed_hz()
                .unwrap()
                .to_bits(),
            2.0_f32.to_bits()
        );
        assert!(app.pod_library.is_dirty(0));
        assert!(app.pod_mod_pending.is_none());
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn mod_speed_gui_stages_discards_applies_and_retains_failed_edits() {
        let (context, mut app) = app();
        app.pod_open = true;
        app.pod_selected = Some(
            app.pod_library
                .import_bytes(sample_delay_note_tone(0, 6))
                .unwrap(),
        );
        app.pod_library
            .import_bytes(sample_delay_note_tone(0, 99))
            .unwrap();
        recording_frame_tall(&context, &mut app, vec![]);
        let original = app.pod_library.tone(0).unwrap().as_bytes().to_vec();
        drag_mod_speed_slider(&context, &mut app, 0.2);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
        assert!(matches!(
            app.pod_mod_pending,
            Some((0, PendingModEdit::SpeedHz(_)))
        ));
        click_pod(&context, &mut app, "pod-mod-discard");
        assert!(app.pod_mod_pending.is_none());
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);

        drag_mod_speed_slider(&context, &mut app, 0.2);
        click_pod(&context, &mut app, "pod-mod-apply");
        assert_eq!(
            app.pod_library.tone(0).unwrap().mod_note_value().unwrap(),
            pod_preset::ModNoteValue::Known(pod_preset::ModNoteChoice::Off)
        );
        assert!(app.pod_library.is_dirty(0));
        assert!(app.pod_library.tone(0).unwrap().as_bytes() != original);
        assert!(app.pod_mod_pending.is_none());

        let second_row = app.pod_rows[&1].center();
        click_pod_at_tall(&context, &mut app, second_row);
        recording_frame_tall(&context, &mut app, vec![]);
        let invalid_original = app.pod_library.tone(1).unwrap().as_bytes().to_vec();
        drag_mod_speed_slider(&context, &mut app, 0.2);
        assert_eq!(
            app.pod_library.tone(1).unwrap().as_bytes(),
            invalid_original
        );
        click_pod(&context, &mut app, "pod-mod-apply");
        assert_eq!(
            app.pod_library.tone(1).unwrap().as_bytes(),
            invalid_original
        );
        assert!(!app.pod_library.is_dirty(1));
        assert!(matches!(
            app.pod_mod_pending,
            Some((1, PendingModEdit::SpeedHz(_)))
        ));

        let first_row = app.pod_rows[&0].center();
        click_pod_at_tall(&context, &mut app, first_row);
        recording_frame_tall(&context, &mut app, vec![]);
        drag_mod_speed_slider(&context, &mut app, 0.3);
        assert!(app.pod_mod_pending.is_some());
        app.pod_library.revert_unsaved(0).unwrap();
        app.refresh_selected_pod_name();
        assert!(app.pod_mod_pending.is_none());
        assert!(!app.pod_library.is_dirty(0));
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn mod_speed_bundle_gui_saves_without_overwrite_and_reopens() {
        let (context, mut app) = app();
        let directory = tempfile::tempdir().unwrap();
        let collision = directory.path().join("existing-mod-bank.l6c");
        let saved = directory.path().join("mod-bank-copy.l6c");
        std::fs::write(&collision, b"preserve existing MOD destination").unwrap();
        let original_tone = sample_delay_note_tone(0, 6);
        let original_bundle = sample_pod_bundle(std::slice::from_ref(&original_tone));
        app.dialogs = Box::new(FakeDialogs {
            pod_save: [Some(collision.clone()), Some(saved.clone())].into(),
            pod_open: [Some(saved.clone())].into(),
            ..Default::default()
        });
        app.pod_open = true;
        app.load_pod_bundle_bytes(original_bundle.clone()).unwrap();
        recording_frame_tall(&context, &mut app, vec![]);
        drag_mod_speed_slider(&context, &mut app, 0.2);
        assert_eq!(
            app.pod_bundle.as_ref().unwrap().as_bytes(),
            original_bundle.as_slice()
        );
        click_pod(&context, &mut app, "pod-mod-apply");
        assert!(app.pod_dirty());
        assert_eq!(
            app.pod_library.tone(0).unwrap().mod_note_value().unwrap(),
            pod_preset::ModNoteValue::Known(pod_preset::ModNoteChoice::Off)
        );

        click_pod(&context, &mut app, "save-pod");
        assert!(app.pod_dirty());
        assert_eq!(
            std::fs::read(&collision).unwrap(),
            b"preserve existing MOD destination"
        );
        click_pod(&context, &mut app, "save-pod");
        assert!(!app.pod_dirty());
        let saved_bytes = std::fs::read(&saved).unwrap();
        assert_ne!(saved_bytes, original_bundle);

        click_pod(&context, &mut app, "load-pod");
        recording_frame_tall(&context, &mut app, vec![]);
        assert_eq!(
            app.pod_library.tone(0).unwrap().mod_note_value().unwrap(),
            pod_preset::ModNoteValue::Known(pod_preset::ModNoteChoice::Off)
        );
        assert!(!app.pod_dirty());
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn delay_model_gui_stages_per_tone_applies_only_id_and_can_discard() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        let mut original = sample_delay_note_tone(0, 0);
        original[8..12].copy_from_slice(b"L6PA");
        let index = app.pod_library.import_bytes(original.clone()).unwrap();
        app.pod_library.import_bytes(original.clone()).unwrap();
        app.pod_selected = Some(index);
        recording_frame_tall(&context, &mut app, vec![]);
        stage_delay_note_choice(&context, &mut app, pod_preset::DelayNoteChoice::Quarter);
        assert!(app.pod_delay_pending.is_some());
        choose_delay_model_popup(&context, &mut app, 0x0202_0003);
        assert_eq!(app.pod_library.tone(index).unwrap().as_bytes(), original);
        assert_eq!(
            app.pod_delay_model_pending.unwrap().1.model_id(),
            0x0202_0003
        );
        click_pod_button_tall(&context, &mut app, "pod-delay-model-apply");
        let applied = app.pod_library.tone(index).unwrap();
        assert_eq!(
            applied
                .models()
                .iter()
                .find(|m| m.slot == pod_preset::DELAY_SLOT)
                .unwrap()
                .model_id,
            0x0202_0003
        );
        let changed: Vec<_> = original
            .iter()
            .zip(applied.as_bytes())
            .enumerate()
            .filter_map(|(i, (a, b))| (a != b).then_some(i))
            .collect();
        assert_eq!(changed.len(), 1);
        assert_eq!(app.pod_delay_model_pending, None);
        assert_eq!(app.pod_delay_pending, None);

        choose_delay_model_popup(&context, &mut app, 0x0202_0004);
        click_pod_button_tall(&context, &mut app, "pod-delay-model-discard");
        assert_eq!(app.pod_delay_model_pending, None);
        assert_eq!(
            app.pod_library
                .tone(index)
                .unwrap()
                .models()
                .iter()
                .find(|model| model.slot == pod_preset::DELAY_SLOT)
                .unwrap()
                .model_id,
            0x0202_0003
        );

        // A staged selector is isolated to its tone and discarded on navigation.
        choose_delay_model_popup(&context, &mut app, 0x0202_0004);
        assert!(app.pod_delay_model_pending.is_some());
        let second_row = app.pod_rows[&1];
        click_pod_at_tall(&context, &mut app, second_row.center());
        recording_frame_tall(&context, &mut app, vec![]);
        assert_eq!(app.pod_selected, Some(1));
        assert_eq!(app.pod_delay_model_pending, None);

        // A failed model Apply preserves prior parameter staging and source bytes.
        let mut invalid = sample_delay_note_tone(0, 0);
        let model_id = 0x0202_0002u32.to_be_bytes();
        let offset = invalid
            .windows(4)
            .position(|word| word == model_id)
            .unwrap();
        invalid[offset..offset + 4].copy_from_slice(&0x3333_0001u32.to_be_bytes());
        invalid[8..12].copy_from_slice(b"L6PA");
        app.pod_library.replace(1, invalid.clone()).unwrap();
        app.pod_delay_pending = Some((1, PendingDelayEdit::TimeMs(350.0)));
        app.pod_delay_model_pending = Some((
            1,
            pod_preset::PodDelayModelChoice::try_from(0x0202_0003).unwrap(),
        ));
        let failed_choice = app.pod_delay_model_pending.unwrap().1;
        assert!(app.apply_pod_delay_model_choice(1, failed_choice).is_err());
        assert_eq!(
            app.pod_delay_pending,
            Some((1, PendingDelayEdit::TimeMs(350.0)))
        );
        assert_eq!(app.pod_library.tone(1).unwrap().as_bytes(), invalid);
        assert_eq!(app.pod_delay_model_pending, Some((1, failed_choice)));
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn mod_model_gui_directly_replaces_each_gated_current_id_and_invalidates_note_stage() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        let gated: [u32; 13] = [
            0x0203_0015,
            0x0203_0012,
            0x0203_001e,
            0x0203_0014,
            0x0203_001f,
            0x0203_0013,
            0x0203_0011,
            0x0203_001a,
            0x0203_001b,
            0x0203_0017,
            0x0203_0018,
            0x0203_0019,
            0x0203_001c,
        ];
        for current_id in gated {
            let mut original = sample_delay_note_tone(0, 0);
            original[8..12].copy_from_slice(b"L6PA");
            let old_id = 0x0203_0004u32.to_be_bytes();
            let offset = original
                .windows(4)
                .position(|word| word == old_id)
                .expect("fixture has a Mod model id");
            original[offset..offset + 4].copy_from_slice(&current_id.to_be_bytes());
            let index = app.pod_library.import_bytes(original.clone()).unwrap();
            app.pod_selected = Some(index);
            recording_frame_tall(&context, &mut app, vec![]);
            stage_delay_note_choice(&context, &mut app, pod_preset::DelayNoteChoice::Quarter);
            stage_mod_note_choice(&context, &mut app, pod_preset::ModNoteChoice::Quarter);
            assert!(
                app.pod_mod_pending.is_some(),
                "Mod NOTE must be staged before model Apply"
            );
            assert!(
                app.pod_delay_pending.is_some(),
                "unrelated Delay NOTE must be staged"
            );
            choose_mod_model_popup(&context, &mut app, 0x0203_0004);
            assert_eq!(app.pod_mod_model_pending.unwrap().1.model_id(), 0x0203_0004);
            click_pod_button_tall(&context, &mut app, "pod-mod-model-apply");
            let applied = app.pod_library.tone(index).unwrap();
            let changed: Vec<_> = original
                .iter()
                .zip(applied.as_bytes())
                .enumerate()
                .filter_map(|(offset, (before, after))| (before != after).then_some(offset))
                .collect();
            assert!(!changed.is_empty());
            assert!(
                changed
                    .iter()
                    .all(|byte| (offset..offset + 4).contains(byte))
            );
            assert_eq!(app.pod_mod_pending, None);
            assert!(
                app.pod_delay_pending.is_some(),
                "unrelated Delay NOTE staging must survive"
            );
            assert_eq!(app.pod_mod_model_pending, None);
            app.pod_delay_pending = None;
        }
    }

    #[test]
    fn reverb_model_gui_apply_and_discard_preserve_other_stages_and_bundle_baseline() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        let original = sample_reverb_model_tone(0x0204_0002, pod_preset::REVERB_FALLBACK_SLOT);
        let index = app.pod_library.import_bytes(original.clone()).unwrap();
        app.pod_selected = Some(index);
        recording_frame_tall(&context, &mut app, vec![]);
        choose_reverb_model_popup(&context, &mut app, 0x0204_0003);
        assert_eq!(
            app.pod_reverb_model_pending.unwrap().1.model_id(),
            0x0204_0003
        );
        assert_eq!(app.pod_library.tone(index).unwrap().as_bytes(), original);
        app.pod_delay_pending = Some((index, PendingDelayEdit::TimeMs(350.0)));
        app.pod_mod_pending = Some((index, PendingModEdit::SpeedHz(2.0)));
        app.pod_stomp_model_pending = Some((
            index,
            pod_preset::PodStompModelChoice::try_from(0x0205_0000).unwrap(),
        ));
        choose_reverb_model_popup(&context, &mut app, 0x0204_0002);
        assert_eq!(
            app.pod_reverb_model_pending.unwrap().1.model_id(),
            0x0204_0002
        );
        click_pod_button_tall(&context, &mut app, "pod-reverb-model-apply");
        assert_eq!(
            app.pod_reverb_model_pending.unwrap().1.model_id(),
            0x0204_0002
        );
        assert_eq!(app.pod_library.tone(index).unwrap().as_bytes(), original);
        assert!(app.pod_delay_pending.is_some() && app.pod_mod_pending.is_some());
        click_pod_button_tall(&context, &mut app, "pod-reverb-model-discard");
        assert_eq!(app.pod_reverb_model_pending, None);
        assert_eq!(app.pod_library.tone(index).unwrap().as_bytes(), original);

        choose_reverb_model_popup(&context, &mut app, 0x0204_0003);
        click_pod_button_tall(&context, &mut app, "pod-reverb-model-apply");
        assert_eq!(
            app.pod_library.tone(index).unwrap().reverb_model_id(),
            Some(0x0204_0003)
        );
        assert_eq!(app.pod_reverb_model_pending, None);
        app.pod_library.revert_unsaved(index).unwrap();
        app.refresh_selected_pod_name();
        assert_eq!(app.pod_library.tone(index).unwrap().as_bytes(), original);

        app.pod_delay_pending = Some((index, PendingDelayEdit::TimeMs(350.0)));
        app.pod_mod_pending = Some((index, PendingModEdit::SpeedHz(2.0)));
        app.pod_stomp_model_pending = Some((
            index,
            pod_preset::PodStompModelChoice::try_from(0x0205_0000).unwrap(),
        ));
        choose_reverb_model_popup(&context, &mut app, 0x0204_0003);
        let bundle =
            pod_preset::PodBundleLibrary::parse(sample_pod_bundle(std::slice::from_ref(&original)))
                .unwrap();
        let bundle_baseline = bundle.as_bytes().to_vec();
        app.pod_bundle = Some(bundle);
        click_pod_button_tall(&context, &mut app, "pod-reverb-model-apply");
        let edited = app.pod_library.tone(index).unwrap();
        assert_eq!(edited.reverb_model_id(), Some(0x0204_0003));
        assert!(app.pod_library.is_dirty(index));
        assert_eq!(app.pod_reverb_model_pending, None);
        assert_eq!(
            app.pod_delay_pending,
            Some((index, PendingDelayEdit::TimeMs(350.0)))
        );
        assert_eq!(
            app.pod_mod_pending,
            Some((index, PendingModEdit::SpeedHz(2.0)))
        );
        assert_eq!(
            app.pod_stomp_model_pending.unwrap().1.model_id(),
            0x0205_0000
        );
        let bundle = app.pod_bundle.as_ref().unwrap();
        assert_ne!(bundle.as_bytes(), bundle_baseline);
        let (bank, preset) = bundle.entry_location(index).unwrap();
        assert_eq!(
            bundle.tone(bank, preset).unwrap().reverb_model_id(),
            Some(0x0204_0003)
        );
    }

    #[test]
    fn reverb_model_gui_failed_apply_navigation_load_save_reopen_and_quit_guard() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        let original = sample_reverb_model_tone(0x0204_0002, pod_preset::REVERB_FALLBACK_SLOT);
        let other = sample_reverb_model_tone(0x0204_0003, pod_preset::REVERB_FALLBACK_SLOT);
        let index = app.pod_library.import_bytes(original.clone()).unwrap();
        let second = app.pod_library.import_bytes(other.clone()).unwrap();
        app.pod_selected = Some(index);
        recording_frame_tall(&context, &mut app, vec![]);

        // Failed Apply retains the staged choice and the pre-existing dirty baseline.
        choose_reverb_model_popup(&context, &mut app, 0x0204_0003);
        let short_minf =
            sample_reverb_model_tone_with_tail(0x0204_0002, pod_preset::REVERB_FALLBACK_SLOT, &[]);
        app.pod_library.replace(index, short_minf.clone()).unwrap();
        app.pod_delay_pending = Some((index, PendingDelayEdit::TimeMs(350.0)));
        app.pod_mod_pending = Some((index, PendingModEdit::SpeedHz(2.0)));
        app.pod_stomp_model_pending = Some((
            index,
            pod_preset::PodStompModelChoice::try_from(0x0205_0000).unwrap(),
        ));
        assert!(app.pod_library.is_dirty(index));
        click_pod_button_tall(&context, &mut app, "pod-reverb-model-apply");
        assert_eq!(app.pod_library.tone(index).unwrap().as_bytes(), short_minf);
        assert_eq!(
            app.pod_library.tone(index).unwrap().reverb_model_id(),
            Some(0x0204_0002)
        );
        assert!(app.pod_library.is_dirty(index));
        assert_eq!(
            app.pod_reverb_model_pending.unwrap().1.model_id(),
            0x0204_0003
        );
        assert_eq!(
            app.pod_delay_pending,
            Some((index, PendingDelayEdit::TimeMs(350.0)))
        );
        assert_eq!(
            app.pod_mod_pending,
            Some((index, PendingModEdit::SpeedHz(2.0)))
        );
        assert_eq!(
            app.pod_stomp_model_pending.unwrap().1.model_id(),
            0x0205_0000
        );
        app.pod_library.revert_unsaved(index).unwrap();
        app.refresh_selected_pod_name();
        assert_eq!(app.pod_library.tone(index).unwrap().as_bytes(), original);
        assert!(!app.pod_library.is_dirty(index));
        assert_eq!(app.pod_reverb_model_pending, None);

        // Changing the selected row clears staged Reverb state.
        choose_reverb_model_popup(&context, &mut app, 0x0204_0003);
        let second_row = app.pod_rows[&second].center();
        click_pod_at_tall(&context, &mut app, second_row);
        recording_frame_tall(&context, &mut app, vec![]);
        assert_eq!(app.pod_selected, Some(second));
        assert_eq!(app.pod_reverb_model_pending, None);
        app.pod_selected = Some(index);
        recording_frame_tall(&context, &mut app, vec![]);

        // Loading through the actual button clears staged Reverb state.
        let directory = tempfile::tempdir().unwrap();
        let load_path = directory.path().join("loaded-reverb.tone");
        std::fs::write(&load_path, &other).unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_open: [Some(load_path.clone())].into(),
            ..Default::default()
        });
        choose_reverb_model_popup(&context, &mut app, 0x0204_0003);
        click_pod_button_tall(&context, &mut app, "load-pod");
        assert_eq!(app.pod_reverb_model_pending, None);
        assert_eq!(
            app.pod_library
                .tone(app.pod_selected.unwrap())
                .unwrap()
                .as_bytes(),
            other
        );

        // Collision, save/reopen, and quit confirmation preserve the edit guards.
        let collision = directory.path().join("occupied.tone");
        let saved = directory.path().join("reverb-model-copy.tone");
        std::fs::write(&collision, b"keep existing destination").unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_save: [Some(collision.clone()), Some(saved.clone())].into(),
            pod_open: [Some(saved.clone())].into(),
            ..Default::default()
        });
        choose_reverb_model_popup(&context, &mut app, 0x0204_0002);
        click_pod_button_tall(&context, &mut app, "pod-reverb-model-apply");
        assert!(app.pod_dirty());
        click_pod_button_tall(&context, &mut app, "save-pod");
        assert!(app.pod_dirty());
        assert_eq!(
            std::fs::read(&collision).unwrap(),
            b"keep existing destination"
        );
        click_pod_button_tall(&context, &mut app, "save-pod");
        assert!(!app.pod_dirty());
        click_pod_button_tall(&context, &mut app, "load-pod");
        let reopened = app.pod_selected.unwrap();
        assert_eq!(
            app.pod_library.tone(reopened).unwrap().reverb_model_id(),
            Some(0x0204_0002)
        );
        choose_reverb_model_popup(&context, &mut app, 0x0204_0003);
        click_pod_button_tall(&context, &mut app, "pod-reverb-model-apply");
        assert!(app.pod_dirty());
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pod_dirty());
        app.pod_library.revert_unsaved(reopened).unwrap();
        app.refresh_selected_pod_name();
        assert!(!app.pod_dirty());
        assert_eq!(
            app.pod_library.tone(reopened).unwrap().reverb_model_id(),
            Some(0x0204_0002)
        );
        assert_eq!(app.pod_reverb_model_pending, None);
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn stomp_model_gui_directly_replaces_all_gated_current_ids_and_invalidates_only_stomp_stage() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        let gated = [
            0x0205_0006,
            0x0205_0001,
            0x0200_0006,
            0x0205_0007,
            0x0200_0003,
            0x020a_000a,
            0x020a_000b,
            0x020a_000c,
            0x020a_0008,
            0x020a_0005,
            0x020a_0006,
            0x020a_0007,
            0x020a_0003,
            0x020a_0004,
            0x020a_0009,
            0x020a_0001,
            0x020a_0002,
        ];
        for current_id in gated {
            let original = sample_stomp_model_tone(current_id);
            let index = app.pod_library.import_bytes(original.clone()).unwrap();
            app.pod_selected = Some(index);
            recording_frame_tall(&context, &mut app, vec![]);
            assert_eq!(
                app.pod_library.tone(index).unwrap().stomp_model_id(),
                Some(current_id),
                "gated current id {current_id:08x} stays raw"
            );
            app.pod_harmony_pending = Some((
                index,
                pod_preset::StompHarmonyControl::Harmony1M335,
                PendingStompHarmony::Harmony1M335(pod_preset::StompHarmony1M335Selection::Unison),
            ));
            app.pod_delay_pending = Some((index, PendingDelayEdit::TimeMs(350.0)));
            app.pod_mod_pending = Some((index, PendingModEdit::SpeedHz(2.0)));
            choose_stomp_model_popup(&context, &mut app, 0x0205_0000);
            assert_eq!(
                app.pod_stomp_model_pending.unwrap().1.model_id(),
                0x0205_0000
            );
            assert_eq!(app.pod_library.tone(index).unwrap().as_bytes(), original);
            click_pod_button_tall(&context, &mut app, "pod-stomp-model-apply");
            let applied = app.pod_library.tone(index).unwrap();
            assert_eq!(applied.stomp_model_id(), Some(0x0205_0000));
            assert!(app.pod_library.is_dirty(index));
            assert_eq!(app.pod_stomp_model_pending, None);
            assert_eq!(app.pod_harmony_pending, None);
            assert_eq!(
                app.pod_delay_pending,
                Some((index, PendingDelayEdit::TimeMs(350.0)))
            );
            assert_eq!(
                app.pod_mod_pending,
                Some((index, PendingModEdit::SpeedHz(2.0)))
            );
            let changed: Vec<_> = original
                .iter()
                .zip(applied.as_bytes())
                .enumerate()
                .filter_map(|(offset, (before, after))| (before != after).then_some(offset))
                .collect();
            let id_offset = original
                .windows(4)
                .position(|window| window == current_id.to_be_bytes())
                .unwrap();
            assert!(!changed.is_empty());
            assert!(
                changed
                    .iter()
                    .all(|offset| (id_offset..id_offset + 4).contains(offset))
            );
            app.pod_delay_pending = None;
            app.pod_mod_pending = None;
        }
    }

    #[test]
    fn stomp_model_gui_discards_and_bundle_apply_reopens_preserving_copy() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        let original = sample_stomp_model_tone(0x0205_0006);
        app.load_pod_bundle_bytes(sample_pod_bundle(std::slice::from_ref(&original)))
            .unwrap();
        recording_frame_tall(&context, &mut app, vec![]);
        choose_stomp_model_popup(&context, &mut app, 0x0205_0000);
        click_pod_button_tall(&context, &mut app, "pod-stomp-model-discard");
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
        assert!(!app.pod_bundle.as_ref().unwrap().is_dirty());

        app.pod_harmony_pending = Some((
            0,
            pod_preset::StompHarmonyControl::Harmony1M335,
            PendingStompHarmony::Harmony1M335(pod_preset::StompHarmony1M335Selection::Unison),
        ));
        app.pod_delay_pending = Some((0, PendingDelayEdit::TimeMs(350.0)));
        app.pod_mod_pending = Some((0, PendingModEdit::SpeedHz(2.0)));
        choose_stomp_model_popup(&context, &mut app, 0x0205_0002);
        assert_eq!(
            app.pod_harmony_pending.as_ref().map(|stage| stage.0),
            Some(0)
        );
        assert_eq!(
            app.pod_delay_pending,
            Some((0, PendingDelayEdit::TimeMs(350.0)))
        );
        assert_eq!(app.pod_mod_pending, Some((0, PendingModEdit::SpeedHz(2.0))));
        click_pod_button_tall(&context, &mut app, "pod-stomp-model-apply");
        assert!(app.pod_bundle.as_ref().unwrap().is_dirty());
        assert_eq!(app.pod_harmony_pending, None);
        assert_eq!(
            app.pod_delay_pending,
            Some((0, PendingDelayEdit::TimeMs(350.0)))
        );
        assert_eq!(app.pod_mod_pending, Some((0, PendingModEdit::SpeedHz(2.0))));
        let bundle_tone = app.pod_bundle.as_ref().unwrap().tone(0, 0).unwrap();
        assert_eq!(bundle_tone.stomp_model_id(), Some(0x0205_0002));
        let directory = tempfile::tempdir().unwrap();
        let copy = directory.path().join("stomp-bundle-copy.l6c");
        app.pod_bundle.as_ref().unwrap().save_copy(&copy).unwrap();
        assert!(app.pod_bundle.as_ref().unwrap().save_copy(&copy).is_err());
        let reopened = pod_preset::PodBundleLibrary::parse(std::fs::read(copy).unwrap()).unwrap();
        assert_eq!(
            reopened.tone(0, 0).unwrap().stomp_model_id(),
            Some(0x0205_0002)
        );
    }

    #[test]
    fn stomp_model_gui_failed_apply_and_same_noop_retain_all_stages() {
        let (_context, mut app) = app();
        let original = sample_stomp_model_tone(0x0205_0000);
        let index = app.pod_library.import_bytes(original.clone()).unwrap();
        app.pod_harmony_pending = Some((
            index,
            pod_preset::StompHarmonyControl::Harmony1457,
            PendingStompHarmony::Harmony1457(pod_preset::StompHarmony1457Selection::Unison),
        ));
        app.pod_delay_pending = Some((index, PendingDelayEdit::TimeMs(400.0)));
        let same = pod_preset::PodStompModelChoice::try_from(0x0205_0000).unwrap();
        assert!(!app.apply_pod_stomp_model_choice(index, same).unwrap());
        assert_eq!(app.pod_library.tone(index).unwrap().as_bytes(), original);
        assert!(app.pod_harmony_pending.is_some());
        assert!(app.pod_delay_pending.is_some());

        let mut unknown = original.clone();
        let id = 0x0205_0000u32.to_be_bytes();
        let offset = unknown.windows(4).position(|window| window == id).unwrap();
        unknown[offset..offset + 4].copy_from_slice(&0xdead_beefu32.to_be_bytes());
        app.pod_library.replace(index, unknown.clone()).unwrap();
        let requested = pod_preset::PodStompModelChoice::try_from(0x0205_0002).unwrap();
        assert!(app.apply_pod_stomp_model_choice(index, requested).is_err());
        assert_eq!(app.pod_library.tone(index).unwrap().as_bytes(), unknown);
        assert!(app.pod_harmony_pending.is_some());
        assert!(app.pod_delay_pending.is_some());
    }

    #[test]
    fn mod_model_gui_applies_and_discards_within_the_selected_bundle_entry() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        let mut original = sample_delay_note_tone(0, 0);
        original[8..12].copy_from_slice(b"L6PA");
        let old = 0x0203_0004u32.to_be_bytes();
        let offset = original
            .windows(4)
            .position(|word| word == old)
            .expect("fixture has a Mod model id");
        original[offset..offset + 4].copy_from_slice(&0x0203_0015u32.to_be_bytes());
        app.load_pod_bundle_bytes(sample_pod_bundle(&[original.clone()]))
            .unwrap();

        recording_frame_tall(&context, &mut app, vec![]);
        stage_mod_note_choice(&context, &mut app, pod_preset::ModNoteChoice::Quarter);
        assert!(app.pod_mod_pending.is_some());
        choose_mod_model_popup(&context, &mut app, 0x0203_0005);
        click_pod_button_tall(&context, &mut app, "pod-mod-model-discard");
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
        assert!(!app.pod_bundle.as_ref().unwrap().is_dirty());

        choose_mod_model_popup(&context, &mut app, 0x0203_0005);
        click_pod_button_tall(&context, &mut app, "pod-mod-model-apply");
        assert!(app.pod_bundle.as_ref().unwrap().is_dirty());
        assert_eq!(app.pod_mod_pending, None);
        assert_eq!(
            app.pod_bundle
                .as_ref()
                .unwrap()
                .tone(0, 0)
                .unwrap()
                .models()
                .iter()
                .find(|model| model.slot == pod_preset::MOD_SLOT)
                .unwrap()
                .model_id,
            0x0203_0005
        );
    }

    #[test]
    fn mod_model_same_id_and_rejected_apply_preserve_staged_note() {
        let (_context, mut app) = app();
        let mut original = sample_delay_note_tone(0, 0);
        original[8..12].copy_from_slice(b"L6PA");
        let index = app.pod_library.import_bytes(original.clone()).unwrap();
        app.pod_mod_pending = Some((
            index,
            PendingModEdit::Note(pod_preset::ModNoteChoice::Quarter),
        ));
        let same = pod_preset::PodModModelChoice::try_from(0x0203_0004).unwrap();
        app.apply_pod_mod_model_choice(index, same).unwrap();
        assert_eq!(
            app.pod_mod_pending,
            Some((
                index,
                PendingModEdit::Note(pod_preset::ModNoteChoice::Quarter)
            ))
        );
        assert_eq!(app.pod_library.tone(index).unwrap().as_bytes(), original);

        let mut invalid = original;
        let current = 0x0203_0004u32.to_be_bytes();
        let offset = invalid.windows(4).position(|word| word == current).unwrap();
        invalid[offset..offset + 4].copy_from_slice(&0x3333_0001u32.to_be_bytes());
        app.pod_library.replace(index, invalid.clone()).unwrap();
        let pending = Some((
            index,
            PendingModEdit::Note(pod_preset::ModNoteChoice::Quarter),
        ));
        app.pod_mod_pending = pending;
        assert!(app.apply_pod_mod_model_choice(index, same).is_err());
        assert_eq!(app.pod_mod_pending, pending);
        assert_eq!(app.pod_library.tone(index).unwrap().as_bytes(), invalid);
    }

    #[test]
    fn mod_model_apply_save_reopen_navigation_load_revert_and_quit_guard() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        let directory = tempfile::tempdir().unwrap();
        let collision = directory.path().join("occupied-mod.tone");
        let saved = directory.path().join("mod-model-copy.tone");
        std::fs::write(&collision, b"keep existing destination").unwrap();
        let mut original = sample_delay_note_tone(0, 0);
        original[8..12].copy_from_slice(b"L6PA");
        app.pod_library.import_bytes(original.clone()).unwrap();
        app.pod_library.import_bytes(original.clone()).unwrap();
        app.pod_selected = Some(0);
        app.dialogs = Box::new(FakeDialogs {
            pod_save: [Some(collision.clone()), Some(saved.clone())].into(),
            pod_open: [Some(saved.clone())].into(),
            ..Default::default()
        });

        recording_frame_tall(&context, &mut app, vec![]);
        choose_mod_model_popup(&context, &mut app, 0x0203_0005);
        click_pod_button_tall(&context, &mut app, "pod-mod-model-apply");
        assert!(app.pod_dirty());
        click_pod_button_tall(&context, &mut app, "save-pod");
        assert!(app.pod_dirty());
        assert_eq!(
            std::fs::read(&collision).unwrap(),
            b"keep existing destination"
        );
        click_pod_button_tall(&context, &mut app, "save-pod");
        assert!(!app.pod_dirty());
        let saved_tone = pod_preset::PodTone::parse(std::fs::read(&saved).unwrap()).unwrap();
        assert_eq!(
            saved_tone
                .models()
                .iter()
                .find(|model| model.slot == pod_preset::MOD_SLOT)
                .unwrap()
                .model_id,
            0x0203_0005
        );

        choose_mod_model_popup(&context, &mut app, 0x0203_0006);
        assert!(app.pod_mod_model_pending.is_some());
        let second = app.pod_rows[&1].center();
        click_pod_at_tall(&context, &mut app, second);
        recording_frame_tall(&context, &mut app, vec![]);
        assert_eq!(app.pod_selected, Some(1));
        assert_eq!(app.pod_mod_model_pending, None);

        let first = app.pod_rows[&0].center();
        click_pod_at_tall(&context, &mut app, first);
        recording_frame_tall(&context, &mut app, vec![]);
        choose_mod_model_popup(&context, &mut app, 0x0203_0007);
        assert!(app.pod_mod_model_pending.is_some());
        click_pod_button_tall(&context, &mut app, "load-pod");
        assert_eq!(app.pod_mod_model_pending, None);
        assert_eq!(
            app.pod_library
                .tone(0)
                .unwrap()
                .models()
                .iter()
                .find(|model| model.slot == pod_preset::MOD_SLOT)
                .unwrap()
                .model_id,
            0x0203_0005
        );

        choose_mod_model_popup(&context, &mut app, 0x0203_0006);
        click_pod_button_tall(&context, &mut app, "pod-mod-model-apply");
        assert!(app.pod_dirty());
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pod_dirty());
        let selected = app.pod_selected.unwrap();
        app.pod_library.revert_unsaved(selected).unwrap();
        app.refresh_selected_pod_name();
        assert!(!app.pod_dirty());
        assert_eq!(app.pod_mod_model_pending, None);
        assert_eq!(
            app.pod_library
                .tone(selected)
                .unwrap()
                .models()
                .iter()
                .find(|model| model.slot == pod_preset::MOD_SLOT)
                .unwrap()
                .model_id,
            0x0203_0005
        );
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn delay_model_gui_can_directly_replace_each_pack_gated_current_model() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;

        for (ordinal, current_id) in (0x0202_000bu32..=0x0202_000fu32).enumerate() {
            let mut original = sample_delay_note_tone(0, 0);
            original[8..12].copy_from_slice(b"L6PA");
            let old_id = 0x0202_0002u32.to_be_bytes();
            let old_id_offset = original
                .windows(4)
                .position(|word| word == old_id)
                .expect("fixture has a Delay model id");
            original[old_id_offset..old_id_offset + 4].copy_from_slice(&current_id.to_be_bytes());
            let index = app.pod_library.import_bytes(original.clone()).unwrap();
            app.pod_selected = Some(index);

            recording_frame_tall(&context, &mut app, vec![]);
            stage_delay_note_choice(&context, &mut app, pod_preset::DelayNoteChoice::Quarter);
            assert!(app.pod_delay_pending.is_some());
            choose_delay_model_popup(&context, &mut app, 0x0202_0002);
            assert_eq!(
                app.pod_delay_model_pending.unwrap().1.model_id(),
                0x0202_0002,
                "pack-gated current model {current_id:08x} must allow direct first-choice staging"
            );

            if ordinal == 0 {
                click_pod_button_tall(&context, &mut app, "pod-delay-model-discard");
                assert_eq!(app.pod_library.tone(index).unwrap().as_bytes(), original);
                assert!(app.pod_delay_pending.is_some());
                choose_delay_model_popup(&context, &mut app, 0x0202_0002);
            }

            click_pod_button_tall(&context, &mut app, "pod-delay-model-apply");
            let applied = app.pod_library.tone(index).unwrap();
            let changed: Vec<_> = original
                .iter()
                .zip(applied.as_bytes())
                .enumerate()
                .filter_map(|(offset, (before, after))| (before != after).then_some(offset))
                .collect();
            let id_offset = original
                .windows(4)
                .position(|word| word == current_id.to_be_bytes())
                .unwrap();
            assert!(!changed.is_empty());
            assert!(
                changed
                    .iter()
                    .all(|offset| (id_offset..id_offset + 4).contains(offset)),
                "only the Delay MINF model-id word may change"
            );
            assert_eq!(
                applied
                    .models()
                    .iter()
                    .find(|model| model.slot == pod_preset::DELAY_SLOT)
                    .unwrap()
                    .model_id,
                0x0202_0002
            );
            assert_eq!(app.pod_delay_pending, None);
            assert_eq!(app.pod_delay_model_pending, None);
        }

        // A same-ID apply leaves a fresh NOTE/TIME stage available.
        let same_id = app.pod_selected.unwrap();
        app.pod_delay_pending = Some((same_id, PendingDelayEdit::TimeMs(350.0)));
        let same_choice = pod_preset::PodDelayModelChoice::try_from(0x0202_0002).unwrap();
        app.apply_pod_delay_model_choice(same_id, same_choice)
            .unwrap();
        assert_eq!(
            app.pod_delay_pending,
            Some((same_id, PendingDelayEdit::TimeMs(350.0)))
        );
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn delay_model_apply_invalidates_stale_parameter_stage_in_bundle() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        let mut original = sample_delay_note_tone(0, 0);
        original[8..12].copy_from_slice(b"L6PA");
        app.load_pod_bundle_bytes(sample_pod_bundle(&[original.clone()]))
            .unwrap();
        recording_frame_tall(&context, &mut app, vec![]);
        stage_delay_note_choice(&context, &mut app, pod_preset::DelayNoteChoice::Quarter);
        assert!(app.pod_delay_pending.is_some());
        choose_delay_model_popup(&context, &mut app, 0x0202_0003);
        click_pod_button_tall(&context, &mut app, "pod-delay-model-apply");
        assert_eq!(app.pod_delay_pending, None);
        let (bank, preset) = app.pod_bundle.as_ref().unwrap().entry_location(0).unwrap();
        assert_eq!(
            app.pod_bundle
                .as_ref()
                .unwrap()
                .tone(bank, preset)
                .unwrap()
                .models()
                .iter()
                .find(|model| model.slot == pod_preset::DELAY_SLOT)
                .unwrap()
                .model_id,
            0x0202_0003
        );
        assert!(app.pod_bundle.as_ref().unwrap().is_dirty());
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn delay_model_apply_save_collision_reopen_revert_and_quit_guard() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        app.pod_open = true;
        let directory = tempfile::tempdir().unwrap();
        let collision = directory.path().join("occupied.tone");
        let saved = directory.path().join("delay-model-copy.tone");
        std::fs::write(&collision, b"keep existing destination").unwrap();
        let mut original = sample_delay_note_tone(0, 0);
        original[8..12].copy_from_slice(b"L6PA");
        app.pod_library.import_bytes(original).unwrap();
        app.pod_selected = Some(0);
        app.dialogs = Box::new(FakeDialogs {
            pod_save: [Some(collision.clone()), Some(saved.clone())].into(),
            pod_open: [Some(saved.clone())].into(),
            ..Default::default()
        });
        recording_frame_tall(&context, &mut app, vec![]);
        choose_delay_model_popup(&context, &mut app, 0x0202_0003);
        click_pod_button_tall(&context, &mut app, "pod-delay-model-apply");
        assert!(app.pod_dirty());
        click_pod_button_tall(&context, &mut app, "save-pod");
        assert!(app.pod_dirty());
        assert_eq!(
            std::fs::read(&collision).unwrap(),
            b"keep existing destination"
        );
        click_pod_button_tall(&context, &mut app, "save-pod");
        assert!(!app.pod_dirty());
        let saved_tone = pod_preset::PodTone::parse(std::fs::read(&saved).unwrap()).unwrap();
        assert_eq!(
            saved_tone
                .models()
                .iter()
                .find(|m| m.slot == pod_preset::DELAY_SLOT)
                .unwrap()
                .model_id,
            0x0202_0003
        );

        choose_delay_model_popup(&context, &mut app, 0x0202_0004);
        click_pod_button_tall(&context, &mut app, "pod-delay-model-apply");
        assert!(app.pod_dirty());
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pod_dirty());
        app.pod_library.revert_unsaved(0).unwrap();
        app.refresh_selected_pod_name();
        assert!(!app.pod_dirty());
        assert_eq!(
            app.pod_library
                .tone(0)
                .unwrap()
                .models()
                .iter()
                .find(|m| m.slot == pod_preset::DELAY_SLOT)
                .unwrap()
                .model_id,
            0x0202_0003
        );
        click_pod(&context, &mut app, "load-pod");
        assert_eq!(
            app.pod_library
                .tone(0)
                .unwrap()
                .models()
                .iter()
                .find(|m| m.slot == pod_preset::DELAY_SLOT)
                .unwrap()
                .model_id,
            0x0202_0003
        );
        assert!(!app.pod_dirty());
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn delay_note_gui_stages_per_tone_applies_offline_and_keeps_unsaved_guards() {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.0);
        let directory = tempfile::tempdir().unwrap();
        let saved = directory.path().join("delay-note-copy.l6c");
        let blocked = directory.path().join("occupied-delay-copy.l6c");
        let saved_time = directory.path().join("delay-time-copy.l6c");
        let blocked_time = directory.path().join("occupied-delay-time-copy.l6c");
        std::fs::write(&blocked, b"preserve existing NOTE destination").unwrap();
        std::fs::write(&blocked_time, b"preserve existing TIME destination").unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_save: [
                Some(blocked.clone()),
                Some(saved.clone()),
                Some(blocked_time.clone()),
                Some(saved_time.clone()),
            ]
            .into(),
            ..Default::default()
        });
        let original = sample_delay_note_tone(0, 0);
        app.pod_open = true;
        app.pod_library.import_bytes(original.clone()).unwrap();
        app.pod_library.import_bytes(original.clone()).unwrap();
        app.pod_selected = Some(0);
        recording_frame_tall(&context, &mut app, vec![]);

        stage_delay_note_choice(&context, &mut app, pod_preset::DelayNoteChoice::Quarter);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
        assert_eq!(
            app.pod_delay_pending,
            Some((
                0,
                PendingDelayEdit::Note(pod_preset::DelayNoteChoice::Quarter)
            ))
        );
        let second_row = app.pod_rows[&1].center();
        click_pod_at_tall(&context, &mut app, second_row);
        recording_frame_tall(&context, &mut app, vec![]);
        assert_eq!(app.pod_selected, Some(1));
        assert!(app.pod_delay_pending.is_none());

        let first_row = app.pod_rows[&0].center();
        click_pod_at_tall(&context, &mut app, first_row);
        recording_frame_tall(&context, &mut app, vec![]);
        stage_delay_note_choice(&context, &mut app, pod_preset::DelayNoteChoice::Quarter);
        click_pod(&context, &mut app, "pod-delay-apply");
        let applied = app.pod_library.tone(0).unwrap().as_bytes().to_vec();
        assert_ne!(applied, original);
        assert!(app.pod_library.is_dirty(0));
        assert!(app.pod_delay_pending.is_none());
        assert_eq!(
            app.pod_library.tone(0).unwrap().delay_note_value().unwrap(),
            pod_preset::DelayNoteValue::Known(pod_preset::DelayNoteChoice::Quarter)
        );
        click_pod(&context, &mut app, "save-pod");
        assert_eq!(
            std::fs::read(&blocked).unwrap(),
            b"preserve existing NOTE destination"
        );
        assert!(app.pod_library.is_dirty(0));
        click_pod(&context, &mut app, "save-pod");
        let reopened = pod_preset::PodTone::parse(std::fs::read(&saved).unwrap()).unwrap();
        assert_eq!(
            reopened.delay_note_value().unwrap(),
            pod_preset::DelayNoteValue::Known(pod_preset::DelayNoteChoice::Quarter)
        );
        assert!(!app.pod_library.is_dirty(0));

        let saved_note = app.pod_library.tone(0).unwrap().as_bytes().to_vec();
        drag_delay_time_slider(&context, &mut app, 0.75);
        let PendingDelayEdit::TimeMs(staged_time) = app.pod_delay_pending.unwrap().1 else {
            panic!("TIME slider should stage a TIME edit");
        };
        assert!(staged_time > app.pod_library.tone(0).unwrap().delay_time_ms().unwrap());
        recording_frame_tall(&context, &mut app, vec![]);
        assert_eq!(
            app.pod_delay_pending,
            Some((0, PendingDelayEdit::TimeMs(staged_time))),
            "TIME staging must persist across frames"
        );
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), saved_note);
        let second_row = app.pod_rows[&1].center();
        click_pod_at_tall(&context, &mut app, second_row);
        recording_frame_tall(&context, &mut app, vec![]);
        assert!(
            app.pod_delay_pending.is_none(),
            "tone switch clears staged TIME"
        );
        let first_row = app.pod_rows[&0].center();
        click_pod_at_tall(&context, &mut app, first_row);
        recording_frame_tall(&context, &mut app, vec![]);

        drag_delay_time_slider(&context, &mut app, 0.75);
        click_pod(&context, &mut app, "pod-delay-discard");
        assert!(app.pod_delay_pending.is_none());
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), saved_note);
        assert!(!app.pod_library.is_dirty(0));

        drag_delay_time_slider(&context, &mut app, 0.75);
        click_pod(&context, &mut app, "pod-delay-apply");
        let time_applied = app.pod_library.tone(0).unwrap().as_bytes().to_vec();
        assert_ne!(time_applied, saved_note);
        assert_eq!(
            app.pod_library.tone(0).unwrap().delay_note_value().unwrap(),
            pod_preset::DelayNoteValue::Known(pod_preset::DelayNoteChoice::Off),
            "positive NOTE clears only when TIME Apply succeeds"
        );
        assert!(app.pod_library.is_dirty(0));
        click_pod(&context, &mut app, "save-pod");
        assert_eq!(
            std::fs::read(&blocked_time).unwrap(),
            b"preserve existing TIME destination"
        );
        assert!(app.pod_library.is_dirty(0));
        click_pod(&context, &mut app, "save-pod");
        let reopened_time =
            pod_preset::PodTone::parse(std::fs::read(&saved_time).unwrap()).unwrap();
        assert_eq!(reopened_time.as_bytes(), time_applied);
        assert!(!app.pod_library.is_dirty(0));

        stage_delay_note_choice(&context, &mut app, pod_preset::DelayNoteChoice::Quarter);
        click_pod(&context, &mut app, "pod-delay-apply");
        assert!(app.pod_library.is_dirty(0));
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        assert!(!app.permit_close);
        click_prompt(&context, &mut app, "discard");
        assert!(app.permit_close);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), time_applied);
        assert!(!app.pod_library.is_dirty(0));
    }

    #[test]
    fn delay_note_gui_rejects_coupled_mod_state_without_mutating() {
        let (context, mut app) = app();
        app.pod_open = true;
        let original = sample_delay_note_tone(0, 6);
        app.pod_library.import_bytes(original.clone()).unwrap();
        app.pod_selected = Some(0);
        recording_frame_tall(&context, &mut app, vec![]);
        stage_delay_note_choice(&context, &mut app, pod_preset::DelayNoteChoice::Quarter);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
        click_pod(&context, &mut app, "pod-delay-apply");
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
        assert!(!app.pod_library.is_dirty(0));
        assert!(app.pod_message.contains("Mod NOTE Off"));
        assert_eq!(
            app.pod_delay_pending,
            Some((
                0,
                PendingDelayEdit::Note(pod_preset::DelayNoteChoice::Quarter)
            ))
        );
    }

    #[test]
    fn delay_time_gui_bundle_apply_reverts_through_shared_bundle_path() {
        let (context, mut app) = app();
        let original = sample_delay_note_tone(6, 0);
        let original_bundle = sample_pod_bundle(std::slice::from_ref(&original));
        app.pod_open = true;
        app.load_pod_bundle_bytes(original_bundle.clone()).unwrap();
        recording_frame_tall(&context, &mut app, vec![]);

        drag_delay_time_slider(&context, &mut app, 0.75);
        let pending = app.pod_delay_pending;
        assert!(matches!(pending, Some((0, PendingDelayEdit::TimeMs(_)))));
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
        assert!(!app.pod_bundle.as_ref().unwrap().is_dirty());
        click_pod(&context, &mut app, "pod-delay-apply");
        assert!(app.pod_bundle.as_ref().unwrap().is_dirty());
        assert!(app.pod_library.is_dirty(0));
        assert_eq!(
            app.pod_library.tone(0).unwrap().delay_note_value().unwrap(),
            pod_preset::DelayNoteValue::Known(pod_preset::DelayNoteChoice::Off)
        );
        assert!(app.pod_delay_pending.is_none());

        stage_delay_note_choice(&context, &mut app, pod_preset::DelayNoteChoice::Quarter);
        assert!(app.pod_delay_pending.is_some());
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "discard");
        assert!(
            app.pod_delay_pending.is_none(),
            "bundle revert clears pending NOTE"
        );
        assert!(!app.pod_bundle.as_ref().unwrap().is_dirty());
        assert!(!app.pod_library.is_dirty(0));
        assert_eq!(app.pod_bundle.as_ref().unwrap().as_bytes(), original_bundle);
        assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn stomp_harmony_gui_hides_a_selector_with_duplicate_parameter_records() {
        for target in [0x0010_0001u32, 0x0010_0002] {
            let (context, mut app) = app();
            app.pod_open = true;
            let duplicate_tone =
                sample_stomp_harmony_pod_tone_with_duplicate(Some((target, 0x3f, 0x3f00_0000)));
            let index = app.pod_library.import_bytes(duplicate_tone).unwrap();
            app.pod_selected = Some(index);
            let original = app.pod_library.tone(index).unwrap().as_bytes().to_vec();
            recording_frame_tall(&context, &mut app, vec![]);
            if target == 0x0010_0001 {
                assert!(app.pod_harmony1_choices.is_empty());
                assert!(!app.pod_buttons.contains_key("pod-harmony1-apply"));
                assert_eq!(app.pod_harmony2_choices.len(), 9);
            } else {
                assert!(app.pod_harmony2_choices.is_empty());
                assert!(!app.pod_buttons.contains_key("pod-harmony2-apply"));
                assert_eq!(app.pod_harmony1_choices.len(), 9);
            }
            assert_eq!(app.pod_library.tone(index).unwrap().as_bytes(), original);
            assert!(!app.pod_library.is_dirty(index));
            const {
                assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
            }
        }
    }

    #[test]
    fn stomp_wave_choices_edit_dirty_save_quit_and_reopen_in_gui() {
        let (context, mut app) = app();
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("stomp-wave-source.l6t");
        let blocked = directory.path().join("existing-copy.l6t");
        let saved = directory.path().join("stomp-wave-copy.l6t");
        let original = sample_stomp_wave_pod_tone();
        std::fs::write(&source, &original).unwrap();
        std::fs::write(&blocked, b"preserve existing destination").unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_open: [Some(source.clone())].into(),
            pod_save: [Some(blocked.clone()), None, Some(saved.clone())].into(),
            ..Default::default()
        });
        click_pod(&context, &mut app, "open-pod");
        click_pod(&context, &mut app, "load-pod");
        assert_eq!(app.pod_library.len(), 1, "{}", app.pod_message);
        recording_frame_tall(&context, &mut app, vec![]);
        assert!(!app.pod_library.is_dirty(0));
        assert!(matches!(
            app.pod_library.tone(0).unwrap().stomp_wave_selection(),
            Ok(pod_preset::StompWaveValue::Known(
                pod_preset::StompWaveSelection::Wave5
            ))
        ));

        select_stomp_wave_choice(&context, &mut app, pod_preset::StompWaveSelection::Wave1);
        assert!(app.pod_library.is_dirty(0));
        assert_eq!(
            app.pod_library
                .tone(0)
                .unwrap()
                .stomp_wave_selection()
                .unwrap(),
            pod_preset::StompWaveValue::Known(pod_preset::StompWaveSelection::Wave1)
        );

        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "save");
        assert_eq!(
            std::fs::read(&blocked).unwrap(),
            b"preserve existing destination"
        );
        assert!(app.pod_library.is_dirty(0));
        assert!(!app.permit_close);

        // A cancelled save leaves the dirty edit available for another choice/save.
        click_prompt(&context, &mut app, "save");
        assert!(app.pod_library.is_dirty(0));
        assert!(!app.permit_close);
        click_prompt(&context, &mut app, "cancel");
        assert!(app.pod_library.is_dirty(0));

        app.request_close(&context);
        click_prompt(&context, &mut app, "save");
        assert!(app.permit_close);
        assert!(app.pending.is_none());
        assert!(!app.pod_library.any_dirty());
        let reopened = pod_preset::PodTone::parse(std::fs::read(saved).unwrap()).unwrap();
        assert_eq!(
            reopened.stomp_wave_selection().unwrap(),
            pod_preset::StompWaveValue::Known(pod_preset::StompWaveSelection::Wave1)
        );
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn stomp_wave_gui_refuses_nonfinite_values_without_dirtying_the_library() {
        for raw_bits in [
            0x7fc0_1234,
            f32::INFINITY.to_bits(),
            f32::NEG_INFINITY.to_bits(),
        ] {
            let (context, mut app) = app();
            let directory = tempfile::tempdir().unwrap();
            let source = directory.path().join("nonfinite-stomp-wave.l6t");
            let original = sample_stomp_wave_pod_tone_with_bits(raw_bits);
            std::fs::write(&source, &original).unwrap();
            app.dialogs = Box::new(FakeDialogs {
                pod_open: [Some(source)].into(),
                ..Default::default()
            });
            click_pod(&context, &mut app, "open-pod");
            click_pod(&context, &mut app, "load-pod");
            assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
            assert!(!app.pod_library.is_dirty(0));
            assert!(matches!(
                app.pod_library.tone(0).unwrap().stomp_wave_selection(),
                Ok(pod_preset::StompWaveValue::Unknown { raw_bits: found }) if found == raw_bits
            ));
            select_stomp_wave_choice(&context, &mut app, pod_preset::StompWaveSelection::Wave8);
            assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
            assert!(matches!(
                app.pod_library.tone(0).unwrap().stomp_wave_selection(),
                Ok(pod_preset::StompWaveValue::Unknown { raw_bits: found }) if found == raw_bits
            ));
            assert!(!app.pod_library.is_dirty(0));
            assert!(app.pod_message.contains("non-finite"));
            const {
                assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
            }
        }
    }

    #[test]
    fn whammy_knob_gui_edit_preserves_other_control_and_guards_save_and_quit() {
        let (context, mut app) = app();
        context.style_mut(|style| {
            style.animation_time = 0.0;
            style.spacing.slider_width = 220.0;
        });
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("whammy-source.l6t");
        let blocked = directory.path().join("existing-copy.l6t");
        let saved = directory.path().join("whammy-copy.l6t");
        let original = sample_whammy_pod_tone();
        std::fs::write(&source, &original).unwrap();
        std::fs::write(&blocked, b"keep existing destination").unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_open: [Some(source)].into(),
            pod_save: [Some(blocked.clone()), None, Some(saved.clone())].into(),
            ..Default::default()
        });
        click_pod(&context, &mut app, "open-pod");
        click_pod(&context, &mut app, "load-pod");
        recording_frame_tall(&context, &mut app, vec![]);
        assert_eq!(app.pod_whammy_knob_sliders.len(), 2);
        assert!(
            !app.pod_library.is_dirty(0),
            "opening and rendering preserve raw values"
        );
        let toe_before = app
            .pod_library
            .tone(0)
            .unwrap()
            .stomp_whammy_knob_position(pod_preset::StompWhammyKnob::Toe)
            .unwrap()
            .to_bits();
        drag_pod_whammy_slider(&context, &mut app, pod_preset::StompWhammyKnob::Heel);
        assert!(app.pod_library.is_dirty(0));
        assert_eq!(
            app.pod_library
                .tone(0)
                .unwrap()
                .stomp_whammy_knob_position(pod_preset::StompWhammyKnob::Toe)
                .unwrap()
                .to_bits(),
            toe_before
        );
        let edited_heel = app
            .pod_library
            .tone(0)
            .unwrap()
            .stomp_whammy_knob_position(pod_preset::StompWhammyKnob::Heel)
            .unwrap()
            .to_bits();
        assert_ne!(edited_heel, 0x3e80_0000);
        click_pod(&context, &mut app, "save-pod");
        assert_eq!(
            std::fs::read(&blocked).unwrap(),
            b"keep existing destination"
        );
        assert!(
            app.pod_library.is_dirty(0),
            "collision preserves dirty edit"
        );
        click_pod(&context, &mut app, "save-pod");
        assert!(
            app.pod_library.is_dirty(0),
            "cancelled save preserves dirty edit"
        );
        click_pod(&context, &mut app, "save-pod");
        assert!(!app.pod_library.is_dirty(0));
        let reopened = pod_preset::PodTone::parse(std::fs::read(saved).unwrap()).unwrap();
        assert_eq!(
            reopened
                .stomp_whammy_knob_position(pod_preset::StompWhammyKnob::Heel)
                .unwrap()
                .to_bits(),
            edited_heel
        );
        assert_eq!(
            reopened
                .stomp_whammy_knob_position(pod_preset::StompWhammyKnob::Toe)
                .unwrap()
                .to_bits(),
            toe_before
        );

        drag_pod_whammy_slider(&context, &mut app, pod_preset::StompWhammyKnob::Toe);
        assert!(app.pod_library.is_dirty(0));
        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "cancel");
        assert!(!app.permit_close);
        assert!(app.pod_library.is_dirty(0));
        app.request_close(&context);
        click_prompt(&context, &mut app, "discard");
        assert!(app.permit_close);
        assert!(app.pending.is_none());
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn stomp_wave_bundle_gui_edit_is_dirty_and_saves_a_preserving_copy() {
        let (context, mut app) = app();
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("stomp-wave-bank.vwb");
        let saved = directory.path().join("stomp-wave-bank-copy.vwb");
        let tone = sample_stomp_wave_pod_tone();
        let original_bundle = sample_pod_bundle(std::slice::from_ref(&tone));
        std::fs::write(&source, &original_bundle).unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_open: [Some(source.clone())].into(),
            pod_save: [Some(saved.clone())].into(),
            ..Default::default()
        });
        click_pod(&context, &mut app, "open-pod");
        click_pod(&context, &mut app, "load-pod");
        assert!(app.pod_bundle.is_some());
        assert!(!app.pod_bundle.as_ref().unwrap().is_dirty());
        recording_frame_tall(&context, &mut app, vec![]);
        select_stomp_wave_choice(&context, &mut app, pod_preset::StompWaveSelection::Wave8);
        assert!(app.pod_bundle.as_ref().unwrap().is_dirty());
        assert!(app.pod_library.is_dirty(0));
        assert_eq!(
            app.pod_bundle
                .as_ref()
                .unwrap()
                .tone(0, 0)
                .unwrap()
                .stomp_wave_selection()
                .unwrap(),
            pod_preset::StompWaveValue::Known(pod_preset::StompWaveSelection::Wave8)
        );

        app.request_close(&context);
        assert!(matches!(app.pending, Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "save");
        assert!(app.permit_close);
        assert!(!app.pod_bundle.as_ref().unwrap().is_dirty());
        assert!(!app.pod_library.any_dirty());
        assert_eq!(std::fs::read(source).unwrap(), original_bundle);
        let reopened = pod_preset::PodBundleLibrary::parse(std::fs::read(saved).unwrap()).unwrap();
        assert_eq!(
            reopened.tone(0, 0).unwrap().stomp_wave_selection().unwrap(),
            pod_preset::StompWaveValue::Known(pod_preset::StompWaveSelection::Wave8)
        );
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn whammy_bundle_gui_routes_typed_knob_edit_through_collection_save() {
        let (context, mut app) = app();
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("whammy-bank.vwb");
        let saved = directory.path().join("whammy-bank-copy.vwb");
        let tone = sample_whammy_pod_tone();
        let original_bundle = sample_pod_bundle(std::slice::from_ref(&tone));
        std::fs::write(&source, &original_bundle).unwrap();
        app.dialogs = Box::new(FakeDialogs {
            pod_open: [Some(source.clone())].into(),
            pod_save: [Some(saved.clone())].into(),
            ..Default::default()
        });
        click_pod(&context, &mut app, "open-pod");
        click_pod(&context, &mut app, "load-pod");
        assert!(app.pod_bundle.is_some());
        recording_frame_tall(&context, &mut app, vec![]);
        let heel = app
            .pod_library
            .tone(0)
            .unwrap()
            .stomp_whammy_knob_position(pod_preset::StompWhammyKnob::Heel)
            .unwrap()
            .to_bits();
        drag_pod_whammy_slider(&context, &mut app, pod_preset::StompWhammyKnob::Toe);
        assert!(app.pod_bundle.as_ref().unwrap().is_dirty());
        assert!(app.pod_library.is_dirty(0));
        let toe = app
            .pod_library
            .tone(0)
            .unwrap()
            .stomp_whammy_knob_position(pod_preset::StompWhammyKnob::Toe)
            .unwrap()
            .to_bits();
        app.request_close(&context);
        click_prompt(&context, &mut app, "save");
        assert!(app.permit_close);
        assert!(!app.pod_bundle.as_ref().unwrap().is_dirty());
        assert!(!app.pod_library.any_dirty());
        assert_eq!(std::fs::read(source).unwrap(), original_bundle);
        let reopened = pod_preset::PodBundleLibrary::parse(std::fs::read(saved).unwrap()).unwrap();
        assert_eq!(
            reopened
                .stomp_whammy_knob_position(0, pod_preset::StompWhammyKnob::Heel)
                .unwrap()
                .to_bits(),
            heel
        );
        assert_eq!(
            reopened
                .stomp_whammy_knob_position(0, pod_preset::StompWhammyKnob::Toe)
                .unwrap()
                .to_bits(),
            toe
        );
        const {
            assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }

    #[test]
    fn delay_association_combos_keep_unknown_values_until_explicit_choice_and_save_reopen() {
        for model_id in [0x0202_0005, 0x0202_000d] {
            let (context, mut app) = app();
            let directory = tempfile::tempdir().unwrap();
            let source = directory.path().join("association-source.l6t");
            let saved = directory.path().join("association-copy.l6t");
            std::fs::write(&source, sample_delay_association_pod_tone(model_id)).unwrap();
            app.dialogs = Box::new(FakeDialogs {
                pod_open: [Some(source)].into(),
                pod_save: [Some(saved.clone())].into(),
                ..Default::default()
            });
            click_pod(&context, &mut app, "open-pod");
            click_pod(&context, &mut app, "load-pod");
            assert_eq!(app.pod_library.len(), 1, "{}", app.pod_message);
            assert_eq!(app.pod_selected, Some(0));
            recording_frame(&context, &mut app, vec![]);
            assert!(!app.pod_library.is_dirty(0));

            if model_id == 0x0202_0005 {
                assert!(matches!(
                    app.pod_library.tone(0).unwrap().delay_heads_selection(),
                    Ok(pod_preset::DelayAssociationValue::Unknown { .. })
                ));
                select_delay_heads_choice(
                    &context,
                    &mut app,
                    pod_preset::DelayHeadsSelection::OneTwoThreeFour,
                );
                assert_eq!(
                    app.pod_library
                        .tone(0)
                        .unwrap()
                        .delay_heads_selection()
                        .unwrap(),
                    pod_preset::DelayAssociationValue::Known(
                        pod_preset::DelayHeadsSelection::OneTwoThreeFour
                    )
                );
            } else {
                assert!(matches!(
                    app.pod_library.tone(0).unwrap().delay_bits_selection(),
                    Ok(pod_preset::DelayAssociationValue::Unknown { .. })
                ));
                select_delay_bits_choice(&context, &mut app, pod_preset::DelayBitsSelection::Four);
                assert_eq!(
                    app.pod_library
                        .tone(0)
                        .unwrap()
                        .delay_bits_selection()
                        .unwrap(),
                    pod_preset::DelayAssociationValue::Known(pod_preset::DelayBitsSelection::Four)
                );
            }
            assert!(app.pod_library.is_dirty(0));
            app.request_close(&context);
            assert!(matches!(app.pending, Some(Pending::QuitPod)));
            click_prompt(&context, &mut app, "save");
            assert!(app.permit_close);
            assert!(app.pending.is_none());
            let reopened = pod_preset::PodTone::parse(std::fs::read(saved).unwrap()).unwrap();
            if model_id == 0x0202_0005 {
                assert_eq!(
                    reopened.delay_heads_selection().unwrap(),
                    pod_preset::DelayAssociationValue::Known(
                        pod_preset::DelayHeadsSelection::OneTwoThreeFour
                    )
                );
            } else {
                assert_eq!(
                    reopened.delay_bits_selection().unwrap(),
                    pod_preset::DelayAssociationValue::Known(pod_preset::DelayBitsSelection::Four)
                );
            }
            const {
                assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
            }
        }
    }

    #[test]
    fn delay_association_gui_refuses_nonfinite_values_without_dirtying_the_library() {
        for (model_id, raw_bits) in [
            (0x0202_0005, 0x7fc0_0001),
            (0x0202_0005, 0x7f80_0000),
            (0x0202_0005, 0xff80_0000),
            (0x0202_000d, 0x7fc0_0001),
            (0x0202_000d, 0x7f80_0000),
            (0x0202_000d, 0xff80_0000),
        ] {
            let (context, mut app) = app();
            let directory = tempfile::tempdir().unwrap();
            let source = directory.path().join("nonfinite-association.l6t");
            let original = sample_delay_association_pod_tone_with_bits(model_id, raw_bits);
            std::fs::write(&source, &original).unwrap();
            app.dialogs = Box::new(FakeDialogs {
                pod_open: [Some(source)].into(),
                ..Default::default()
            });
            click_pod(&context, &mut app, "open-pod");
            click_pod(&context, &mut app, "load-pod");
            assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
            assert!(!app.pod_library.is_dirty(0));

            if model_id == 0x0202_0005 {
                assert!(matches!(
                    app.pod_library.tone(0).unwrap().delay_heads_selection(),
                    Ok(pod_preset::DelayAssociationValue::Unknown { raw_bits: found }) if found == raw_bits
                ));
                select_delay_heads_choice(
                    &context,
                    &mut app,
                    pod_preset::DelayHeadsSelection::OneTwoThreeFour,
                );
            } else {
                assert!(matches!(
                    app.pod_library.tone(0).unwrap().delay_bits_selection(),
                    Ok(pod_preset::DelayAssociationValue::Unknown { raw_bits: found }) if found == raw_bits
                ));
                select_delay_bits_choice(&context, &mut app, pod_preset::DelayBitsSelection::Four);
            }

            assert!(
                app.pod_message.contains("non-finite"),
                "{}",
                app.pod_message
            );
            assert_eq!(app.pod_library.tone(0).unwrap().as_bytes(), original);
            assert!(!app.pod_library.is_dirty(0));
            const {
                assert!(hardware_gates::is_locked(Gate::CurrentSettingsRead));
            }
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
            app.change(|p| p.set_metadata(PresetText::Author, "Guitarist 🎸"));
            app.change(|p| p.set_metadata(PresetText::Notes, "Drop D\nStudio take"));
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
                "Guitarist 🎸"
            );
            assert_eq!(
                reopened.preset().metadata(PresetText::Notes).unwrap(),
                "Drop D\nStudio take"
            );

            app.undo();
            assert!(app.document.as_ref().unwrap().slot_is_dirty(2, 1));
            app.redo();
            assert!(!app.document.as_ref().unwrap().is_dirty());
        }
    }
}
