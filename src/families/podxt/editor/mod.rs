use super::*;
use variax_workbench::pod_preset::Library;

#[cfg(feature = "usb")]
mod live_panel;
mod locker;
mod parameter_panel;
mod stage;
use parameter_panel::ParameterPanel;

#[cfg(test)]
pub(super) mod tests;

#[derive(Clone, Copy)]
enum PodMembershipAction {
    Duplicate,
    Remove,
    MoveUp,
    MoveDown,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PendingStompHarmony {
    Harmony1M335(pod_preset::StompHarmony1M335Selection),
    Harmony1457(pod_preset::StompHarmony1457Selection),
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum PendingDelayEdit {
    Note(pod_preset::DelayNoteChoice),
    TimeMs(f32),
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum PendingModEdit {
    Note(pod_preset::ModNoteChoice),
    SpeedHz(f32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PendingPodEqReset {
    document_generation: u64,
    index: usize,
    context: Vec<u8>,
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

/// Built-in offline examples for the Practice menu. Both are the same
/// synthetic documents the existing GUI tests already build.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(super) enum PracticeTone {
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
pub(super) enum PodBlock {
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

pub(super) struct PodxtEditor {
    #[cfg(feature = "usb")]
    live: live_panel::LivePanel,
    locker: locker::LockerWindow,
    parameter_panel: ParameterPanel,
    stage: stage::Stage,
    #[cfg(test)]
    table_view_in_test: bool,
    pub(super) pod_library: Library,
    pub(super) pod_bundle: Option<pod_preset::PodBundleLibrary>,
    pub(super) pod_selected: Option<usize>,
    /// Bank shown by Bank Down / Bank Up. Channel clicks keep this in sync.
    pod_bank: usize,
    /// Block whose settings the pedalboard is showing. Switching it does not
    /// discard edits on another block.
    pod_block: PodBlock,
    pod_revert_confirm: bool,
    pod_cab_pending: Option<(usize, pod_preset::PodCabModelChoice)>,
    pod_cab_pending_origin: Option<(u64, usize, Vec<u8>)>,
    #[cfg(test)]
    pod_delay_model_pending: Option<(usize, pod_preset::PodDelayModelChoice)>,
    #[cfg(test)]
    pod_mod_model_pending: Option<(usize, pod_preset::PodModModelChoice)>,
    #[cfg(test)]
    pod_stomp_model_pending: Option<(usize, pod_preset::PodStompModelChoice)>,
    #[cfg(test)]
    pod_reverb_model_pending: Option<(usize, pod_preset::PodReverbModelChoice)>,
    pod_amp_model_pending: Option<(usize, u32, pod_preset::PodAmpModelChoice)>,
    pod_eq_reset_pending: Option<PendingPodEqReset>,
    pod_document_generation: u64,
    #[cfg(test)]
    pod_harmony_pending: Option<(usize, pod_preset::StompHarmonyControl, PendingStompHarmony)>,
    #[cfg(test)]
    pod_delay_pending: Option<(usize, PendingDelayEdit)>,
    #[cfg(test)]
    pod_mod_pending: Option<(usize, PendingModEdit)>,
    pub(super) pod_name: String,
    pod_message: String,
    #[cfg(test)]
    pod_buttons: HashMap<&'static str, Rect>,
    #[cfg(test)]
    pod_rows: HashMap<usize, Rect>,
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
}

impl PodxtEditor {
    fn new() -> Self {
        Self {
            #[cfg(feature = "usb")]
            live: live_panel::LivePanel::default(),
            locker: locker::LockerWindow::default(),
            parameter_panel: ParameterPanel::default(),
            stage: stage::Stage::default(),
            #[cfg(test)]
            table_view_in_test: false,
            pod_library: Library::default(),
            pod_bundle: None,
            pod_selected: None,
            pod_bank: 0,
            pod_block: PodBlock::Amp,
            pod_revert_confirm: false,
            pod_cab_pending: None,
            pod_cab_pending_origin: None,
            #[cfg(test)]
            pod_delay_model_pending: None,
            #[cfg(test)]
            pod_mod_model_pending: None,
            #[cfg(test)]
            pod_stomp_model_pending: None,
            #[cfg(test)]
            pod_reverb_model_pending: None,
            pod_amp_model_pending: None,
            pod_eq_reset_pending: None,
            pod_document_generation: 0,
            #[cfg(test)]
            pod_harmony_pending: None,
            #[cfg(test)]
            pod_delay_pending: None,
            #[cfg(test)]
            pod_mod_pending: None,
            pod_name: String::new(),
            pod_message: String::new(),
            #[cfg(test)]
            pod_buttons: HashMap::new(),
            #[cfg(test)]
            pod_rows: HashMap::new(),
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
        }
    }

    pub(super) fn pod_dirty(&self) -> bool {
        self.pod_library.any_dirty()
            || self
                .pod_bundle
                .as_ref()
                .is_some_and(pod_preset::PodBundleLibrary::is_dirty)
    }

    fn pod_has_staged_amp_choice(&self) -> bool {
        self.pod_amp_model_pending.is_some()
    }

    pub(super) fn pod_has_staged_edit(&self) -> bool {
        self.pod_has_staged_amp_choice() || self.pod_eq_reset_pending.is_some()
    }

    pub(super) fn invalidate_stale_eq_reset_stage(&mut self) {
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

    pub(super) fn pod_quit_dialog(
        &mut self,
        services: &mut EditorServices<'_>,
        ctx: &egui::Context,
    ) {
        #[cfg(feature = "usb")]
        if self.live.locker_dirty() {
            egui::Window::new("Staged PODxt device changes")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Closing discards the staged Tone Locker changes.");
                    ui.horizontal(|ui| {
                        if ui.button("Keep working").clicked() {
                            services.close = CloseResult::Cancelled;
                        }
                        if ui.button("Discard staged changes").clicked() {
                            self.live.discard_locker();
                            services.finish_close_if(!self.needs_close());
                        }
                    });
                });
            return;
        }
        #[cfg(feature = "usb")]
        if self.live.has_staged_edits() {
            egui::Window::new("Staged PODxt tone edits")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Closing discards the staged edits to the PODxt tone.");
                    ui.horizontal(|ui| {
                        if ui.button("Keep editing").clicked() {
                            services.close = CloseResult::Cancelled;
                        }
                        if ui.button("Discard staged edits").clicked() {
                            self.live.discard_staged();
                            services.finish_close_if(!self.needs_close());
                        }
                    });
                });
            return;
        }
        if self.pod_eq_reset_pending.is_some() {
            self.pod_eq_reset_quit_dialog(services, ctx);
            return;
        }
        if self
            .pod_bundle
            .as_ref()
            .is_some_and(|bundle| bundle.is_dirty())
        {
            self.pod_bundle_quit_dialog(services, ctx);
            return;
        }
        let Some(index) = self.pod_library.first_dirty() else {
            if self.pod_has_staged_amp_choice() {
                self.pod_amp_quit_dialog(services, ctx);
                return;
            }
            services.finish_close_if(!self.needs_close());
            return;
        };
        let Some(name) = self
            .pod_library
            .tone(index)
            .map(|tone| tone.name().to_owned())
        else {
            services.close = CloseResult::Cancelled;
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
                    "\"{name}\" has applied edits that have not been saved. {still}"
                ));
                ui.horizontal(|ui| {
                    let response = ui.button("Save copy and continue…");
                    #[cfg(test)]
                    services.prompt_buttons.insert("save", response.rect);
                    if response.clicked() {
                        save_continue = true;
                    }
                    let response = ui.button("Keep editing");
                    #[cfg(test)]
                    services.prompt_buttons.insert("cancel", response.rect);
                    if response.clicked() {
                        cancel = true;
                    }
                    let response = ui.button("Discard this tone");
                    #[cfg(test)]
                    services.prompt_buttons.insert("discard", response.rect);
                    if response.clicked() {
                        discard = true;
                    }
                });
            });
        if cancel {
            services.close = CloseResult::Cancelled;
        } else if save_continue {
            if self.save_next_dirty_pod(services) {
                services.finish_close_if(!self.needs_close());
            }
        } else if discard && self.discard_next_dirty_pod() {
            services.finish_close_if(!self.needs_close());
        }
    }

    fn pod_amp_quit_dialog(&mut self, services: &mut EditorServices<'_>, ctx: &egui::Context) {
        let Some((index, origin_id, choice)) = self.pod_amp_model_pending else {
            services.finish_close_if(!self.needs_close());
            return;
        };
        let current_id = self
            .pod_library
            .tone(index)
            .and_then(pod_preset::PodTone::amp_model_id);
        if current_id != Some(origin_id) {
            self.pod_amp_model_pending = None;
            services.finish_close_if(!self.needs_close());
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
                    services.prompt_buttons.insert("amp-apply", response.rect);
                    apply = response.clicked();
                    let response = ui.button("Discard AMP choice");
                    #[cfg(test)]
                    services.prompt_buttons.insert("amp-discard", response.rect);
                    discard = response.clicked();
                    let response = ui.button("Keep editing");
                    #[cfg(test)]
                    services
                        .prompt_buttons
                        .insert("amp-keep-editing", response.rect);
                    keep_editing = response.clicked();
                });
            });

        if apply {
            self.pod_selected = Some(index);
            if self.apply_pod_amp_model_choice(choice, Some(origin_id)) {
                self.pod_amp_model_pending = None;
                self.continue_staged_pod_quit(services);
            }
        } else if discard {
            self.pod_amp_model_pending = None;
            self.continue_staged_pod_quit(services);
        } else if keep_editing {
            services.close = CloseResult::Cancelled;
        }
    }

    fn pod_eq_reset_quit_dialog(&mut self, services: &mut EditorServices<'_>, ctx: &egui::Context) {
        let Some(pending) = self.pod_eq_reset_pending.clone() else {
            self.continue_staged_pod_quit(services);
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
            self.continue_staged_pod_quit(services);
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
                    services
                        .prompt_buttons
                        .insert("eq-reset-quit-apply", response.rect);
                    apply = response.clicked();
                    let response = ui.button("Discard EQ reset");
                    #[cfg(test)]
                    services
                        .prompt_buttons
                        .insert("eq-reset-quit-discard", response.rect);
                    discard = response.clicked();
                    let response = ui.button("Keep editing");
                    #[cfg(test)]
                    services
                        .prompt_buttons
                        .insert("eq-reset-quit-keep", response.rect);
                    keep_editing = response.clicked();
                });
            });
        if apply {
            self.pod_selected = Some(pending.index);
            if self.apply_pod_eq_reset(Some(&pending)) {
                self.pod_eq_reset_pending = None;
                self.continue_staged_pod_quit(services);
            }
        } else if discard {
            self.pod_eq_reset_pending = None;
            self.continue_staged_pod_quit(services);
        } else if keep_editing {
            services.close = CloseResult::Cancelled;
        }
    }

    fn continue_staged_pod_quit(&mut self, services: &mut EditorServices<'_>) {
        services.finish_close_if(!self.needs_close());
    }

    fn pod_bundle_quit_dialog(&mut self, services: &mut EditorServices<'_>, ctx: &egui::Context) {
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
                    services.prompt_buttons.insert("save", response.rect);
                    save_continue = response.clicked();
                    let response = ui.button("Keep editing");
                    #[cfg(test)]
                    services.prompt_buttons.insert("cancel", response.rect);
                    cancel = response.clicked();
                    let response = ui.button("Discard bank edits");
                    #[cfg(test)]
                    services.prompt_buttons.insert("discard", response.rect);
                    discard = response.clicked();
                });
            });
        if cancel {
            services.close = CloseResult::Cancelled;
        } else if save_continue {
            let index = self.pod_library.first_dirty().unwrap_or(0);
            if self.write_pod_copy(services, index) {
                services.finish_close_if(!self.needs_close());
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
                    services.finish_close_if(!self.needs_close());
                }
                Err(error) => self.pod_message = error.detail(),
            }
        }
    }

    pub(super) fn save_next_dirty_pod(&mut self, services: &mut EditorServices<'_>) -> bool {
        if let Some(index) = self.pod_library.first_dirty() {
            return self.write_pod_copy(services, index);
        }
        false
    }

    pub(super) fn discard_next_dirty_pod(&mut self) -> bool {
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
                    self.parameter_panel.clear_tone_history(index);
                    let staged_amp = self.pod_amp_model_pending;
                    self.refresh_selected_pod_name();
                    self.restore_staged_amp_if_origin_matches(staged_amp);
                    self.pod_message =
                        "Discarded the unsaved edits for one tone. Nothing was sent.".into();
                    true
                }
                Err(error) => {
                    self.pod_message = error.detail();
                    false
                }
            };
        }
        false
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
        #[cfg(test)]
        {
            self.pod_delay_model_pending = None;
        }
        #[cfg(test)]
        {
            self.pod_mod_model_pending = None;
        }
        #[cfg(test)]
        {
            self.pod_stomp_model_pending = None;
        }
        #[cfg(test)]
        {
            self.pod_reverb_model_pending = None;
        }
        self.pod_amp_model_pending = None;
        #[cfg(test)]
        {
            self.pod_harmony_pending = None;
        }
        #[cfg(test)]
        {
            self.pod_delay_pending = None;
        }
        #[cfg(test)]
        {
            self.pod_mod_pending = None;
        }
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

    // Retain the former widgets for the existing compatibility regression suite.
    fn legacy_control_widgets(&self) -> bool {
        #[cfg(test)]
        {
            !self.table_view_in_test
        }
        #[cfg(not(test))]
        {
            false
        }
    }

    pub(super) fn pod_window(&mut self, services: &mut EditorServices<'_>, ui: &mut egui::Ui) {
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
                ui.label(pod_preset::OFFLINE_BANNER);
                ui.label(pod_preset::NOT_GUITAR);
                ui.label(pod_preset::NOT_SENT);
                if self.legacy_control_widgets() { ui.label(pod_preset::PAYLOADS_NOT_DECODED); }
                // Keep the editor status on one line, with the full text on hover.
                ui.add(egui::Label::new(&self.pod_message).truncate())
                    .on_hover_text(&self.pod_message);
                ui.separator();
                // Keep the pedalboard widget IDs independent of the editor controls.
                ui.push_id("pod-pedalboard", |ui| {
                    self.pod_pedalboard(ui);
                });
                ui.separator();
                #[cfg(test)]
                if self.legacy_control_widgets() {
                    self.legacy_reverb_widgets(ui);
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
                        self.load_pod_tone(services);
                    }
                    if let Some(sample) = self.practice_tone_menu(ui) {
                        self.import_pod_document(sample.bytes());
                    }
                    if self.pod_button(ui, "apply-pod-name", "Apply name") {
                        self.apply_pod_name();
                    }
                    if self.pod_button(ui, "save-pod", "Save copy…") {
                        self.save_pod_tone(services);
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
                    if self.legacy_control_widgets() && let Some(index) = self.pod_selected {
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
                            if self.legacy_control_widgets() { format!(
                                "{bank}{index} {} {} amp {} stomp {} cab {}",
                                tone.container().tag(),
                                tone.name(),
                                pod_preset::PodTone::model_label(tone.amp_model_id()),
                                pod_preset::PodTone::model_label(tone.stomp_model_id()),
                                pod_preset::PodTone::model_label(tone.cab_model_id()),
                            ) } else { format!("{bank}{}", tone.name()) },
                            tone.name().to_owned(),
                        ))
                    })
                    .collect();
                let legacy = self.legacy_control_widgets();
                let mut render_rows = |ui: &mut egui::Ui| {
                    for (index, label, _) in rows.iter() {
                        let response = ui.selectable_label(self.pod_selected == Some(*index), label);
                        #[cfg(test)]
                        self.pod_rows.insert(*index, response.rect);
                        if response.clicked() {
                            self.select_pod_index(*index);
                        }
                    }
                };
                if legacy {
                    render_rows(ui);
                } else if rows.len() > 1 {
                    ui.collapsing("Tones", render_rows);
                }
                #[cfg(test)]
                if self.legacy_control_widgets() {
                    self.legacy_parameter_widgets(ui);
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
                        "{}\n{}\n{}\n{}\n{}",
                        pod_preset::OFFLINE_BANNER,
                        pod_preset::NOT_GUITAR,
                        pod_preset::NOT_SENT,
                        pod_preset::PAYLOADS_NOT_DECODED,
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

            });
    }

    #[cfg(test)]
    fn legacy_reverb_widgets(&mut self, ui: &mut egui::Ui) {
        ui.scope(|ui| {
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
        });
    }

    #[cfg(test)]
    fn legacy_parameter_widgets(&mut self, ui: &mut egui::Ui) {
        ui.scope(|ui| {
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
                                        #[cfg(test)]
                                        {
                                            self.pod_delay_pending = None;
                                        }
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
                                        #[cfg(test)]
                                        {
                                            self.pod_mod_model_pending = None;
                                        }
                                        self.pod_message = "Applied the Mod model ID offline; stored parameters were retained. Nothing was sent.".into();
                                    }
                                    Err(error) => self.pod_message = error.detail(),
                                }
                            }
                            let discard = ui.button("Discard");
                            #[cfg(test)]
                            self.pod_buttons.insert("pod-mod-model-discard", discard.rect);
                            if discard.clicked() {
                                #[cfg(test)]
                                {
                                    self.pod_mod_model_pending = None;
                                }
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
                                        #[cfg(test)]
                                        {
                                            self.pod_mod_pending = None;
                                        }
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
            #[cfg(test)]
            {
                self.pod_delay_model_pending = None;
            }
            #[cfg(test)]
            {
                self.pod_mod_model_pending = None;
            }
            #[cfg(test)]
            {
                self.pod_stomp_model_pending = None;
            }
            #[cfg(test)]
            {
                self.pod_reverb_model_pending = None;
            }
            self.pod_amp_model_pending = None;
            #[cfg(test)]
            {
                self.pod_harmony_pending = None;
            }
            #[cfg(test)]
            {
                self.pod_delay_pending = None;
            }
            #[cfg(test)]
            {
                self.pod_mod_pending = None;
            }
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
        let staged = self.pod_cab_pending.is_some()
            || self.pod_amp_model_pending.is_some()
            || self.pod_eq_reset_pending.is_some();
        #[cfg(not(test))]
        {
            staged
        }
        #[cfg(test)]
        {
            staged
                || self.pod_delay_model_pending.is_some()
                || self.pod_mod_model_pending.is_some()
                || self.pod_stomp_model_pending.is_some()
                || self.pod_reverb_model_pending.is_some()
                || self.pod_harmony_pending.is_some()
                || self.pod_delay_pending.is_some()
                || self.pod_mod_pending.is_some()
        }
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
        #[cfg(test)]
        {
            self.pod_delay_model_pending = None;
        }
        #[cfg(test)]
        {
            self.pod_mod_model_pending = None;
        }
        #[cfg(test)]
        {
            self.pod_stomp_model_pending = None;
        }
        #[cfg(test)]
        {
            self.pod_reverb_model_pending = None;
        }
        self.pod_amp_model_pending = None;
        self.pod_eq_reset_pending = None;
        #[cfg(test)]
        {
            self.pod_harmony_pending = None;
        }
        #[cfg(test)]
        {
            self.pod_delay_pending = None;
        }
        #[cfg(test)]
        {
            self.pod_mod_pending = None;
        }
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
        self.parameter_panel.clear_history();
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
            Ok(pod_preset::ModelEnableState::NonCanonical { .. }) => {
                format!("{} · unavailable", block.label())
            }
            Err(_) => format!("{} · no model", block.label()),
        }
    }

    #[cfg(test)]
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

    #[cfg(test)]
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
            self.pod_message = format!("{} On/Bypass is unavailable for this tone.", block.label());
            return;
        };
        let result = if self.legacy_control_widgets() {
            self.edit_pod_tone(index, |tone| tone.set_model_enabled(control, !enabled))
        } else {
            let key = match block {
                PodBlock::Amp => "AMP_ENABLE",
                PodBlock::Stomp => "STOMP_ENABLE",
                PodBlock::Mod => "MOD_ENABLE",
                PodBlock::Delay => "DELAY_ENABLE",
            };
            self.table_edit(index, |tone| {
                tone.set_table_parameter(key, u32::from(!enabled))?;
                Ok(Default::default())
            })
        };
        match result {
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
        #[cfg(test)]
        if self.legacy_control_widgets() {
            ui.small(self.pod_block_model_line(self.pod_block));
            ui.small(self.pod_block_controls_line(self.pod_block));
        }
        let enable_note =
            self.pod_selected.and_then(|index| {
                self.pod_library.tone(index).and_then(|tone| {
                match tone.model_enable_state(self.pod_block.enable()) {
                    Ok(pod_preset::ModelEnableState::Canonical(_)) => None,
                    Ok(pod_preset::ModelEnableState::NonCanonical {
                        raw: _,
                        source_interpretation,
                    }) => Some(format!(
                        "On/Bypass is unavailable. Stored flag reads as {} and cannot be edited.",
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
            #[cfg(feature = "usb")]
            let show_offline_fetch = !self.live.visible();
            #[cfg(not(feature = "usb"))]
            let show_offline_fetch = true;
            if show_offline_fetch {
                let _fetch =
                    self.pod_button_enabled(ui, "fetch-pod", "Fetch from PODxt Live", false);
            }
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

    #[cfg(test)]
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

    #[cfg(test)]
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

    #[cfg(test)]
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
        self.parameter_panel.clear_history();
        self.pod_document_generation = self.pod_document_generation.wrapping_add(1);
        self.pod_selected = Some(0);
        self.pod_eq_reset_pending = None;
        self.pod_cab_pending = None;
        #[cfg(test)]
        {
            self.pod_harmony_pending = None;
        }
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
        self.parameter_panel.clear_history();
        self.pod_cab_pending = None;
        #[cfg(test)]
        {
            self.pod_harmony_pending = None;
        }
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
                self.parameter_panel.clear_history();
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

    pub(super) fn load_pod_tone(&mut self, services: &mut EditorServices<'_>) {
        let Some(path) = services.dialogs.open_pod_tone() else {
            self.pod_message = "Load cancelled. No file was read.".into();
            return;
        };
        self.open_stage_file(&path);
    }

    fn open_stage_file(&mut self, path: &Path) {
        match pod_preset::read_regular_bundle(pod_preset::path_facts(path), || {
            let file = fs::File::open(path)?;
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
                            self.parameter_panel.clear_history();
                            self.pod_bundle = None;
                            self.pod_document_generation =
                                self.pod_document_generation.wrapping_add(1);
                            self.pod_eq_reset_pending = None;
                            self.pod_cab_pending = None;
                            #[cfg(test)]
                            {
                                self.pod_harmony_pending = None;
                            }
                        }
                        self.pod_selected = Some(index);
                        self.pod_cab_pending = None;
                        #[cfg(test)]
                        {
                            self.pod_delay_model_pending = None;
                        }
                        #[cfg(test)]
                        {
                            self.pod_mod_model_pending = None;
                        }
                        #[cfg(test)]
                        {
                            self.pod_stomp_model_pending = None;
                        }
                        #[cfg(test)]
                        {
                            self.pod_reverb_model_pending = None;
                        }
                        self.pod_amp_model_pending = None;
                        #[cfg(test)]
                        {
                            self.pod_delay_pending = None;
                        }
                        #[cfg(test)]
                        {
                            self.pod_mod_pending = None;
                        }
                        #[cfg(test)]
                        {
                            self.pod_harmony_pending = None;
                        }
                        self.pod_name = summary.name.clone();
                        self.pod_message = if self.legacy_control_widgets() {
                            format!(
                                "Loaded {} for PODxt Live {:08x}. {}",
                                summary.container,
                                summary.device_id,
                                pod_preset::PAYLOADS_NOT_DECODED
                            )
                        } else {
                            format!("Loaded {} for PODxt Live.", summary.name)
                        };
                    }
                    Err(error) => self.pod_message = error.detail(),
                }
            }
            Err(error) => self.pod_message = rejection_text(&error),
        }
    }

    pub(super) fn apply_pod_name(&mut self) {
        let Some(index) = self.pod_selected else {
            self.pod_message = "Select a tone before renaming it.".into();
            return;
        };
        if !self.legacy_control_widgets() {
            let name = self.pod_name.clone();
            if let Err(error) = self.table_edit(index, |tone| {
                tone.rename(&name)?;
                Ok(Default::default())
            }) {
                self.pod_message = error.detail();
            }
            return;
        }
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

    #[cfg(test)]
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
            #[cfg(test)]
            {
                self.pod_delay_pending = None;
            }
        }
        Ok(())
    }

    #[cfg(test)]
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
            #[cfg(test)]
            {
                self.pod_mod_pending = None;
            }
        }
        Ok(())
    }

    #[cfg(test)]
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
            #[cfg(test)]
            {
                self.pod_harmony_pending = None;
            }
        }
        Ok(true)
    }

    #[cfg(test)]
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

    #[cfg(test)]
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

    #[cfg(test)]
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

    pub(super) fn save_pod_tone(&mut self, services: &mut EditorServices<'_>) {
        let Some(index) = self.pod_selected else {
            self.pod_message = "Select a tone before saving a copy.".into();
            return;
        };
        self.write_pod_copy(services, index);
    }

    fn write_pod_copy(&mut self, services: &mut EditorServices<'_>, index: usize) -> bool {
        if let Some(bundle) = &self.pod_bundle {
            let name = if bundle.as_bytes().get(8..12) == Some(b"L6CO") {
                "PODxt Live Collection.l6c"
            } else {
                "PODxt Live Bundle.l6b"
            };
            let Some(path) = services.dialogs.save_pod_tone(name) else {
                self.pod_message = "Save cancelled. No file was written.".into();
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
        let Some(path) = services.dialogs.save_pod_tone(&suggested) else {
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
}

impl PodxtEditor {
    fn show_new_documents(&mut self, ui: &mut egui::Ui, services: &mut EditorServices<'_>) {
        ui.horizontal_wrapped(|ui| {
            if ui.button("New tone").clicked() {
                match pod_preset::PodTone::new_podxt_tone("New Tone") {
                    Ok(tone) => self.open_stage_document(tone.into_bytes()),
                    Err(error) => self.pod_message = error.detail(),
                }
            }
            for (label, fixed) in [("New bundle", true), ("New collection", false)] {
                if ui.button(label).clicked() {
                    match pod_preset::new_podxt_bundle(fixed) {
                        Ok(bundle) => self.open_stage_document(bundle.as_bytes().to_vec()),
                        Err(error) => self.pod_message = error.detail(),
                    }
                }
            }
            if ui
                .add_enabled(
                    !self.pod_library.is_empty(),
                    egui::Button::new("Export file tone list"),
                )
                .clicked()
            {
                let entries: Vec<_> = (0..self.pod_library.len())
                    .filter_map(|index| {
                        let tone = self.pod_library.tone(index)?;
                        let location = self
                            .pod_bundle
                            .as_ref()
                            .and_then(|bundle| {
                                let (bank, slot) = bundle.entry_location(index)?;
                                Some(format!("{} {}", bundle.bank_name(bank)?, slot + 1))
                            })
                            .unwrap_or_else(|| (index + 1).to_string());
                        Some((location, tone.name().to_owned()))
                    })
                    .collect();
                services.request_picker::<Self>(
                    ui.ctx(),
                    PickRequest::SaveTone {
                        name: "PODxt Tones.html".into(),
                    },
                    move |editor, path, _| {
                        if let Some(path) = path {
                            let result = pod_preset::save_podxt_html(
                                &path,
                                entries
                                    .iter()
                                    .map(|(location, name)| (location.as_str(), name.as_str())),
                            );
                            editor.pod_message = match result {
                                Ok(()) => "Exported tone list.".into(),
                                Err(error) => error.detail(),
                            };
                        }
                    },
                );
            }
        });
    }

    #[cfg(feature = "usb")]
    pub(in crate::editor) fn device_check_panel(
        &mut self,
        ui: &mut egui::Ui,
        services: &mut EditorServices<'_>,
    ) {
        self.live.device_check_panel(ui, services);
    }

    fn needs_close(&self) -> bool {
        #[cfg(feature = "usb")]
        if self.live.locker_dirty() || self.live.has_staged_edits() {
            return true;
        }
        self.pod_dirty() || self.pod_has_staged_edit()
    }
}

impl FamilyTab for PodxtEditor {
    fn show(&mut self, ui: &mut egui::Ui, services: &mut EditorServices<'_>) {
        if self.legacy_control_widgets() {
            self.pod_window(services, ui);
        } else {
            egui::ScrollArea::vertical()
                .id_salt("podxt-stage")
                .show(ui, |ui| {
                    self.show_stage(ui, services);
                    #[cfg(feature = "usb")]
                    self.live.show_store_panel(ui, services);
                });
            let document_pending = self.stage_has_pending_document();
            if let Some(document) = self.locker.show(
                ui,
                services,
                #[cfg(feature = "usb")]
                &mut self.live,
                self.stage.backup.as_ref(),
                #[cfg(feature = "usb")]
                self.pod_selected
                    .and_then(|index| self.pod_library.tone(index)),
                document_pending,
            ) {
                self.open_locker_document(document);
                if self.stage_has_pending_document() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Focus);
                }
            }
        }
    }
    fn source_path(&self) -> Option<&Path> {
        self.stage.path.as_deref()
    }
    fn local_prompt(&mut self, ctx: &egui::Context, services: &mut EditorServices<'_>) {
        self.stage_document_prompt(ctx, services);
    }
    fn blocking_prompt(&self) -> bool {
        self.stage_has_pending_document()
    }
    fn undo(&mut self) {
        if !self.stage_is_current() {
            self.table_history(false);
        }
    }
    fn redo(&mut self) {
        if !self.stage_is_current() {
            self.table_history(true);
        }
    }
    fn dirty(&self) -> bool {
        self.needs_close()
    }
    fn close_prompt(&mut self, ctx: &egui::Context, services: &mut EditorServices<'_>) {
        self.pod_quit_dialog(services, ctx);
    }
    fn status(&self) -> (&str, Option<&str>) {
        #[cfg(feature = "usb")]
        if self.stage_is_current() {
            return (self.live.stage_footer(), None);
        }
        (&self.pod_message, None)
    }
    fn save_copy(&mut self, _ctx: &egui::Context, services: &mut EditorServices<'_>) {
        self.save_pod_tone(services);
    }
    fn observe_device(&mut self, device: Option<&tabs::DeviceTab>) {
        self.stage.observe(device);
        #[cfg(feature = "usb")]
        {
            if self.live.set_device(device) {
                if self.pod_dirty()
                    || self.pod_has_staged_edit()
                    || self.stage_has_pending_document()
                {
                    self.live.enter_edit();
                } else {
                    self.stage.view_current = true;
                }
            } else if !self.live.connected() && !self.live.has_staged_edits() {
                self.stage.view_current = false;
            }
        }
    }
    #[cfg(feature = "usb")]
    fn busy(&self) -> bool {
        self.live.busy()
    }
    #[cfg(feature = "usb")]
    fn cancel_operation(&mut self) {
        self.live.cancel();
    }
    #[cfg(feature = "usb")]
    fn connection_sync_admission(&self, _consent: bool) -> tabs::SyncAdmission {
        self.live.connection_sync_admission()
    }
    #[cfg(feature = "usb")]
    fn start_connection_sync(&mut self, ctx: &egui::Context) -> Result<(), String> {
        self.live.start_connection_sync(ctx)
    }
    #[cfg(feature = "usb")]
    fn connection_sync_update(&mut self) -> Option<tabs::SyncUpdate> {
        self.live.connection_sync_update()
    }
    #[cfg(feature = "usb")]
    fn cancel_connection_sync(&mut self) {
        self.live.cancel_connection_sync();
    }
    fn poll(&mut self, _ctx: &egui::Context) {
        #[cfg(feature = "usb")]
        {
            let was_live = self.live.live_editing;
            let before = self.live.playing_channel();
            self.live.poll(_ctx);
            let reveal = self.live.take_playing_reveal();
            if self.live.live_editing
                && (reveal || !was_live || before != self.live.playing_channel())
            {
                self.stage.reveal_playing_channel();
            }
        }
        self.invalidate_stale_eq_reset_stage();
    }
}

pub(super) struct Family;

pub(super) const FAMILY: Family = Family;

impl ProductFamily for Family {
    fn info(&self) -> FamilyInfo {
        FamilyInfo {
            id: "podxt",
            title: "PODxt Live",
            new_label: "PODxt Live",
            extensions: &["l6t", "tone", "l6a", "l6s", "vwb", "l6c"],
            ceiling: pod_preset::BUNDLE_BYTES_CEILING,
        }
    }

    fn claims(&self, bytes: &[u8]) -> bool {
        if matches!(bytes.get(8..12), Some(b"L6BA" | b"L6CO")) {
            pod_preset::PodBundleLibrary::parse(bytes.to_vec()).is_ok()
        } else {
            pod_preset::PodTone::parse(bytes.to_vec()).is_ok()
        }
    }

    fn open(
        &self,
        defaults: &EditorDefaults,
        source: TabSource,
    ) -> Result<Box<dyn FamilyTab>, String> {
        let mut editor = PodxtEditor::new();
        editor.initialize_stage(defaults.preferences.clone());
        if let TabSource::File(path, bytes) = source {
            editor.import_pod_document(bytes);
            editor.stage.path = Some(path);
            editor
                .locker
                .files
                .use_document_directory(editor.stage.path.as_deref());
            if editor.pod_library.is_empty() {
                return Err(editor.pod_message);
            }
        }
        Ok(Box::new(editor))
    }
}
