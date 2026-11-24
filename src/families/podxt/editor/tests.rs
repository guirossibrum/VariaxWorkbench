use super::super::tests::{
    app, click_prompt, prepare_quit_take, prompt_frame, recording_frame, recording_frame_tall,
};
use super::*;
use variax_workbench::recording::ClockSelection;

fn click_pod(context: &egui::Context, app: &mut Workbench, id: &'static str) {
    recording_frame(context, app, vec![]);
    let pos = match id {
        "open-pod" => app.tab_buttons["podxt"].center(),
        "show-variax" => app.tab_buttons["variax"].center(),
        _ => app.podxt().pod_buttons[id].center(),
    };
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
    let pos = app.podxt().pod_model_enable_checks[&control].center();
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
    let pos = app.podxt().pod_model_enable_menu.unwrap().center();
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

pub(in crate::editor) fn sample_pod_tone() -> Vec<u8> {
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
    app.podxt_mut().pod_library.import_bytes(amp_tone).unwrap();
    app.podxt_mut().pod_selected = Some(0);
    app.select_test_editor::<PodxtEditor>();
    recording_frame(&context, &mut app, vec![]);
    open_pod_model_enable_menu(&context, &mut app);
    assert!(
        app.podxt()
            .pod_model_enable_checks
            .contains_key(&pod_preset::PodModelEnable::Amp)
    );
    assert!(
        !app.podxt()
            .pod_model_enable_checks
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
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        expected
    );
    assert!(app.podxt().pod_library.is_dirty(0));
    app.podxt_mut().pod_library.revert_unsaved(0).unwrap();
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        original
    );

    let mut short_minfo = original.clone();
    let minf_header = short_minfo
        .windows(4)
        .position(|window| window == b"MINF")
        .unwrap();
    short_minfo[minf_header + 4..minf_header + 8].copy_from_slice(&11u32.to_be_bytes());
    short_minfo[minf_header + 8 + 11] = 0;
    let short_original = short_minfo.clone();
    app.podxt_mut().pod_library.clear();
    app.podxt_mut()
        .pod_library
        .import_bytes(short_minfo)
        .unwrap();
    app.podxt_mut().pod_selected = Some(0);
    recording_frame(&context, &mut app, vec![]);
    open_pod_model_enable_menu(&context, &mut app);
    assert!(
        !app.podxt()
            .pod_model_enable_checks
            .contains_key(&pod_preset::PodModelEnable::Amp)
    );
    assert!(
        app.podxt()
            .pod_library
            .tone(0)
            .unwrap()
            .model_enable_state(pod_preset::PodModelEnable::Amp)
            .is_err()
    );
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        short_original
    );
    assert!(!app.podxt().pod_library.is_dirty(0));
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
    app.podxt_mut().pod_library.clear();
    app.podxt_mut()
        .pod_library
        .import_bytes(noncanonical.clone())
        .unwrap();
    app.podxt_mut().pod_selected = Some(0);
    recording_frame(&context, &mut app, vec![]);
    open_pod_model_enable_menu(&context, &mut app);
    assert!(
        !app.podxt()
            .pod_model_enable_checks
            .contains_key(&pod_preset::PodModelEnable::Amp)
    );
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        noncanonical
    );
    assert!(!app.podxt().pod_library.is_dirty(0));
    click_pod_model_enable_menu(&context, &mut app);

    let eq_tone = sample_pod_eq_tone();
    app.podxt_mut()
        .load_pod_bundle_bytes(sample_pod_bundle(std::slice::from_ref(&eq_tone)))
        .unwrap();
    app.select_test_editor::<PodxtEditor>();
    recording_frame(&context, &mut app, vec![]);
    open_pod_model_enable_menu(&context, &mut app);
    assert!(
        app.podxt()
            .pod_model_enable_checks
            .contains_key(&pod_preset::PodModelEnable::Eq)
    );
    click_pod_model_enable(&context, &mut app, pod_preset::PodModelEnable::Eq);
    assert!(app.podxt().pod_bundle.as_ref().unwrap().is_dirty());
    assert_eq!(
        app.podxt()
            .pod_bundle
            .as_ref()
            .unwrap()
            .tone(0, 0)
            .unwrap()
            .model_enable_state(pod_preset::PodModelEnable::Eq),
        Ok(pod_preset::ModelEnableState::Canonical(false))
    );
    assert_eq!(
        app.podxt()
            .pod_library
            .tone(0)
            .unwrap()
            .model_enable_state(pod_preset::PodModelEnable::Eq),
        Ok(pod_preset::ModelEnableState::Canonical(false))
    );
    assert!(app.podxt().pod_message.contains("Nothing was sent"));
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
    }

    let dir = tempfile::tempdir().unwrap();
    let collision = dir.path().join("existing.vwb");
    std::fs::write(&collision, b"keep existing bytes").unwrap();
    app.dialogs.queue_pod_save(Some(collision.clone()));
    app.force_test_quit::<PodxtEditor>();
    click_prompt(&context, &mut app, "cancel");
    assert!(app.test_pending().is_none());
    assert!(app.podxt().pod_bundle.as_ref().unwrap().is_dirty());

    app.force_test_quit::<PodxtEditor>();
    click_prompt(&context, &mut app, "save");
    assert!(
        app.test_pending().is_some(),
        "failed save must keep quit pending"
    );
    assert!(app.podxt().pod_bundle.as_ref().unwrap().is_dirty());
    assert_eq!(std::fs::read(&collision).unwrap(), b"keep existing bytes");

    let saved = dir.path().join("saved.vwb");
    app.dialogs.queue_pod_save(Some(saved.clone()));
    click_prompt(&context, &mut app, "save");
    assert!(app.test_pending().is_none());
    assert!(app.permit_close);
    assert!(!app.podxt().pod_bundle.as_ref().unwrap().is_dirty());
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
    value_record
        .extend_from_slice(&((0x3fu32 << 24) | pod_preset::STOMP_SYNTH_WAVE_PARAM).to_be_bytes());
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
    let combo = app
        .podxt()
        .pod_cab_combo
        .expect("CAB choice combo is rendered");
    click_pod_at(context, app, combo.center());
    recording_frame(context, app, vec![]);
    let option = app.podxt().pod_cab_options[&model_id].center();
    click_pod_at(context, app, option);
    recording_frame(context, app, vec![]);
}

fn choose_practice_tone(context: &egui::Context, app: &mut Workbench, sample: PracticeTone) {
    recording_frame(context, app, vec![]);
    let combo = app
        .podxt()
        .pod_practice_combo
        .expect("Practice tone combo is rendered");
    click_pod_at(context, app, combo.center());
    recording_frame(context, app, vec![]);
    assert_eq!(
        app.podxt().pod_practice_options.len(),
        PracticeTone::ALL.len()
    );
    let option = app.podxt().pod_practice_options[&sample].center();
    click_pod_at(context, app, option);
    recording_frame(context, app, vec![]);
}

#[test]
fn cab_popup_choice_persists_until_apply_and_isolated_per_tone() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    app.select_test_editor::<PodxtEditor>();
    let first = sample_cab_mic_pod_tone(0x0102_0000, 1);
    let second = sample_cab_mic_pod_tone(0x0102_0001, 2);
    app.podxt_mut()
        .pod_library
        .import_bytes(first.clone())
        .unwrap();
    app.podxt_mut()
        .pod_library
        .import_bytes(second.clone())
        .unwrap();
    app.podxt_mut().pod_selected = Some(0);

    recording_frame(&context, &mut app, vec![]);
    choose_cab_popup(&context, &mut app, 0x0102_0002);
    assert_eq!(app.podxt().pod_library.tone(0).unwrap().as_bytes(), first);
    assert!(!app.podxt().pod_library.is_dirty(0));
    assert_eq!(
        app.podxt().pod_cab_pending,
        Some((
            0,
            pod_preset::PodCabModelChoice::try_from(0x0102_0002).unwrap()
        ))
    );
    recording_frame(&context, &mut app, vec![]);
    assert_eq!(
        app.podxt().pod_cab_pending.unwrap().1.model_id(),
        0x0102_0002
    );

    click_pod(&context, &mut app, "pod-cab-apply");
    let applied = app.podxt().pod_library.tone(0).unwrap();
    assert_eq!(applied.cab_model_id(), Some(0x0102_0002));
    assert_eq!(applied.mic_choice().unwrap().value(), 1);
    assert!(app.podxt().pod_library.is_dirty(0));
    assert_eq!(app.podxt().pod_library.tone(1).unwrap().as_bytes(), second);

    choose_cab_popup(&context, &mut app, 0x0102_0003);
    assert_eq!(
        app.podxt().pod_cab_pending.unwrap().1.model_id(),
        0x0102_0003
    );
    recording_frame(&context, &mut app, vec![]);
    let second_row = app.podxt().pod_rows[&1].center();
    click_pod_at(&context, &mut app, second_row);
    recording_frame(&context, &mut app, vec![]);
    assert_eq!(app.podxt().pod_selected, Some(1));
    assert_eq!(app.podxt().pod_cab_pending.unwrap().0, 1);
    assert_eq!(
        app.podxt().pod_cab_pending.unwrap().1.model_id(),
        0x0102_0001
    );
    assert_eq!(app.podxt().pod_library.tone(1).unwrap().as_bytes(), second);
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
    }
}

#[test]
fn l6pa_cab_mic_controls_render_and_apply_only_after_explicit_choice() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    app.select_test_editor::<PodxtEditor>();
    let original =
        include_bytes!("../../../../tests/fixtures/pod-l6pa-cab-mic/cab.before.bin").to_vec();
    app.podxt_mut()
        .pod_library
        .import_bytes(original.clone())
        .unwrap();
    app.podxt_mut().pod_selected = Some(0);
    recording_frame(&context, &mut app, vec![]);
    assert!(app.podxt().pod_cab_combo.is_some());
    choose_cab_popup(&context, &mut app, 0x0102_0002);
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        original
    );
    click_pod(&context, &mut app, "pod-cab-apply");
    let changed = app.podxt().pod_library.tone(0).unwrap();
    assert_eq!(changed.cab_edit_model_id(), Some(0x0102_0002));
    assert_eq!(changed.mic_choice().unwrap().value(), 2);
    assert!(app.podxt().pod_library.is_dirty(0));

    click_pod(&context, &mut app, "pod-mic-3");
    assert_eq!(
        app.podxt()
            .pod_library
            .tone(0)
            .unwrap()
            .mic_choice()
            .unwrap()
            .value(),
        3
    );
    let staged = app.podxt().pod_cab_pending;
    click_pod(&context, &mut app, "pod-cab-discard");
    assert!(app.podxt().pod_cab_pending.is_none() || app.podxt().pod_cab_pending != staged);
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
    }
}

#[test]
fn l6pa_cab_edit_saves_reopens_and_stale_stages_do_not_cross_tones() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    app.select_test_editor::<PodxtEditor>();
    let original =
        include_bytes!("../../../../tests/fixtures/pod-l6pa-cab-mic/cab.before.bin").to_vec();
    let sibling =
        include_bytes!("../../../../tests/fixtures/pod-l6pa-cab-mic/mic-guitar.before.bin")
            .to_vec();
    app.podxt_mut()
        .pod_library
        .import_bytes(original.clone())
        .unwrap();
    app.podxt_mut()
        .pod_library
        .import_bytes(sibling.clone())
        .unwrap();
    app.podxt_mut().pod_selected = Some(0);
    recording_frame(&context, &mut app, vec![]);

    choose_cab_popup(&context, &mut app, 0x0102_0002);
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        original
    );
    click_pod(&context, &mut app, "pod-cab-apply");
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().cab_edit_model_id(),
        Some(0x0102_0002)
    );
    assert_eq!(app.podxt().pod_library.tone(1).unwrap().as_bytes(), sibling);
    assert!(app.podxt().pod_library.is_dirty(0));

    let directory = tempfile::tempdir().unwrap();
    let saved = directory.path().join("l6pa-copy.tone");
    app.dialogs = Box::new(FakeDialogs {
        pod_save: [Some(saved.clone())].into(),
        ..Default::default()
    });
    click_pod(&context, &mut app, "save-pod");
    assert!(!app.podxt().pod_library.is_dirty(0));
    let reopened = pod_preset::PodTone::parse(std::fs::read(saved).unwrap()).unwrap();
    assert_eq!(reopened.cab_edit_model_id(), Some(0x0102_0002));
    assert_eq!(reopened.mic_choice().unwrap().value(), 2);

    choose_cab_popup(&context, &mut app, 0x0102_0003);
    let staged = app.podxt().pod_cab_pending;
    let sibling_row = app.podxt().pod_rows[&1].center();
    click_pod_at(&context, &mut app, sibling_row);
    recording_frame(&context, &mut app, vec![]);
    assert_eq!(app.podxt().pod_selected, Some(1));
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().cab_edit_model_id(),
        Some(0x0102_0002)
    );
    assert!(app.podxt().pod_cab_pending.is_none() || app.podxt().pod_cab_pending != staged);
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
    }
}

#[test]
fn practice_amp_and_effects_tone_applies_and_reopens_edits() {
    let (context, mut app) = app();
    context.style_mut(|style| {
        style.animation_time = 0.0;
        style.spacing.slider_width = 220.0;
    });
    app.select_test_editor::<PodxtEditor>();
    recording_frame(&context, &mut app, vec![]);
    assert!(app.podxt().pod_buttons.contains_key("load-pod"));

    choose_practice_tone(&context, &mut app, PracticeTone::AmpAndEffects);
    assert_eq!(app.podxt().pod_selected, Some(0));
    assert!(app.podxt().pod_bundle.is_none());
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        sample_amp_effects_tone(b"L6PA")
    );
    let original_drive = app
        .podxt()
        .pod_library
        .tone(0)
        .unwrap()
        .amp_drive_fraction()
        .unwrap();
    let original_stomp = app
        .podxt()
        .pod_library
        .tone(0)
        .unwrap()
        .stomp_control_fraction(pod_preset::StompControl::Drive)
        .unwrap();
    drag_pod_drive_slider(&context, &mut app);
    drag_pod_stomp_slider(&context, &mut app, pod_preset::StompControl::Drive);
    let edited_drive = app
        .podxt()
        .pod_library
        .tone(0)
        .unwrap()
        .amp_drive_fraction()
        .unwrap();
    let edited_stomp = app
        .podxt()
        .pod_library
        .tone(0)
        .unwrap()
        .stomp_control_fraction(pod_preset::StompControl::Drive)
        .unwrap();
    assert_ne!(edited_drive.to_bits(), original_drive.to_bits());
    assert_ne!(edited_stomp.to_bits(), original_stomp.to_bits());
    assert!(app.podxt().pod_library.is_dirty(0));
    let delay = 0x0202_0003;
    let modulation = 0x0203_0008;
    choose_delay_model_popup(&context, &mut app, delay);
    click_pod_button_tall(&context, &mut app, "pod-delay-model-apply");
    assert_eq!(app.podxt().pod_delay_model_pending, None);
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().delay_model_id(),
        Some(delay)
    );
    choose_mod_model_popup(&context, &mut app, modulation);
    click_pod_button_tall(&context, &mut app, "pod-mod-model-apply");
    assert_eq!(app.podxt().pod_mod_model_pending, None);
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().mod_model_id(),
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
    assert!(!app.podxt().pod_library.is_dirty(0));
    click_pod(&context, &mut app, "load-pod");
    assert_eq!(app.podxt().pod_selected, Some(1));
    let reopened = app.podxt().pod_library.tone(1).unwrap();
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
    assert!(app.podxt().pod_buttons.contains_key("load-pod"));
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
    }
}

#[test]
fn practice_stompbox_collection_saves_the_changed_model() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    app.select_test_editor::<PodxtEditor>();
    recording_frame(&context, &mut app, vec![]);
    assert!(app.podxt().pod_buttons.contains_key("load-pod"));

    choose_practice_tone(&context, &mut app, PracticeTone::StompboxCollection);
    assert_eq!(app.podxt().pod_selected, Some(0));
    assert!(app.podxt().pod_bundle.is_some());
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().stomp_model_id(),
        Some(0x0205_0006)
    );
    assert!(app.podxt().pod_delay_model_combo.is_none());
    assert!(app.podxt().pod_mod_model_combo.is_none());
    assert!(
        !app.podxt()
            .pod_buttons
            .contains_key("pod-delay-model-apply")
    );
    assert!(!app.podxt().pod_buttons.contains_key("pod-mod-model-apply"));
    choose_stomp_model_popup(&context, &mut app, 0x0205_0002);
    click_pod_button_tall(&context, &mut app, "pod-stomp-model-apply");
    assert_eq!(
        app.podxt()
            .pod_bundle
            .as_ref()
            .unwrap()
            .tone(0, 0)
            .unwrap()
            .stomp_model_id(),
        Some(0x0205_0002)
    );
    assert!(app.podxt().pod_dirty());

    let directory = tempfile::tempdir().unwrap();
    let saved = directory.path().join("practice-stomp-collection.vwb");
    app.dialogs = Box::new(FakeDialogs {
        pod_open: [Some(saved.clone())].into(),
        pod_save: [Some(saved.clone())].into(),
        ..Default::default()
    });
    click_pod(&context, &mut app, "save-pod");
    assert!(!app.podxt().pod_dirty());
    click_pod(&context, &mut app, "load-pod");
    let reopened = app.podxt().pod_bundle.as_ref().unwrap();
    assert_eq!(
        reopened.tone(0, 0).unwrap().stomp_model_id(),
        Some(0x0205_0002)
    );
    assert!(app.podxt().pod_buttons.contains_key("load-pod"));
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
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
    app.podxt_mut()
        .load_pod_bundle_bytes(fixed_pod_banks(&[bank_one, bank_two]))
        .unwrap();
    app.select_test_editor::<PodxtEditor>();
    recording_frame(&context, &mut app, vec![]);
    assert!(app.podxt().pod_buttons.contains_key("pod-channel-a"));
    assert!(app.podxt().pod_buttons.contains_key("pod-channel-d"));
    assert!(app.podxt().pod_visible.contains("Fetch needs gate G5"));
    assert!(
        app.podxt()
            .pod_visible
            .contains("reviewed write and store workflow")
    );
    let before_fetch = app.podxt().pod_bundle.as_ref().unwrap().as_bytes().to_vec();
    click_pod(&context, &mut app, "fetch-pod");
    click_pod(&context, &mut app, "send-bank");
    click_pod(&context, &mut app, "send-all");
    assert_eq!(
        app.podxt().pod_bundle.as_ref().unwrap().as_bytes(),
        before_fetch
    );
    assert!(!app.podxt().pod_message.contains("sent to"));

    click_pod(&context, &mut app, "pod-channel-d");
    assert_eq!(app.podxt().pod_selected, Some(3));
    assert!(
        app.podxt()
            .pod_library
            .tone(3)
            .unwrap()
            .name()
            .contains('D')
    );
    let drive_before = app
        .podxt()
        .pod_library
        .tone(3)
        .unwrap()
        .amp_drive_fraction()
        .unwrap();
    drag_pod_drive_slider_tall(&context, &mut app);
    let drive_after = app
        .podxt()
        .pod_library
        .tone(3)
        .unwrap()
        .amp_drive_fraction()
        .unwrap();
    assert_ne!(drive_before.to_bits(), drive_after.to_bits());
    click_pod(&context, &mut app, "pod-block-stomp");
    assert_eq!(app.podxt().pod_block, PodBlock::Stomp);
    assert_eq!(app.podxt().pod_selected, Some(3));
    let amp_on = app
        .podxt()
        .pod_library
        .tone(3)
        .unwrap()
        .model_enable_state(pod_preset::PodModelEnable::Amp);
    let mod_on = app
        .podxt()
        .pod_library
        .tone(3)
        .unwrap()
        .model_enable_state(pod_preset::PodModelEnable::Mod);
    let delay_on = app
        .podxt()
        .pod_library
        .tone(3)
        .unwrap()
        .model_enable_state(pod_preset::PodModelEnable::Delay);
    let stomp_model = app.podxt().pod_library.tone(3).unwrap().stomp_model_id();
    click_pod(&context, &mut app, "pod-block-bypass");
    assert_eq!(
        app.podxt()
            .pod_library
            .tone(3)
            .unwrap()
            .model_enable_state(pod_preset::PodModelEnable::Stomp)
            .unwrap(),
        pod_preset::ModelEnableState::Canonical(false)
    );
    assert_eq!(
        app.podxt()
            .pod_library
            .tone(3)
            .unwrap()
            .model_enable_state(pod_preset::PodModelEnable::Amp)
            .unwrap(),
        amp_on.unwrap()
    );
    assert_eq!(
        app.podxt()
            .pod_library
            .tone(3)
            .unwrap()
            .model_enable_state(pod_preset::PodModelEnable::Mod)
            .unwrap(),
        mod_on.unwrap()
    );
    assert_eq!(
        app.podxt()
            .pod_library
            .tone(3)
            .unwrap()
            .model_enable_state(pod_preset::PodModelEnable::Delay)
            .unwrap(),
        delay_on.unwrap()
    );
    assert_eq!(
        app.podxt().pod_library.tone(3).unwrap().stomp_model_id(),
        stomp_model
    );
    assert_eq!(
        app.podxt()
            .pod_library
            .tone(3)
            .unwrap()
            .amp_drive_fraction()
            .unwrap()
            .to_bits(),
        drive_after.to_bits()
    );
    let edited_d = app.podxt().pod_library.tone(3).unwrap().as_bytes().to_vec();
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        original_a
    );

    click_pod(&context, &mut app, "pod-channel-a");
    assert_eq!(app.podxt().pod_selected, Some(0));
    assert_eq!(
        app.podxt().pod_library.tone(3).unwrap().as_bytes(),
        edited_d
    );
    click_pod(&context, &mut app, "show-variax");
    assert!(!app.test_editor_is_active::<PodxtEditor>());
    assert_eq!(
        app.podxt().pod_library.tone(3).unwrap().as_bytes(),
        edited_d
    );
    click_pod(&context, &mut app, "open-pod");
    assert!(app.test_editor_is_active::<PodxtEditor>());
    assert_eq!(
        app.podxt().pod_library.tone(3).unwrap().as_bytes(),
        edited_d
    );

    click_pod(&context, &mut app, "pod-bank-up");
    assert_eq!(app.podxt().pod_bank, 1);
    assert_eq!(app.podxt().pod_selected, Some(4));
    click_pod(&context, &mut app, "pod-channel-d");
    assert_eq!(
        app.podxt().pod_selected,
        Some(4),
        "empty channel D stays disabled"
    );
    click_pod(&context, &mut app, "pod-channel-b");
    assert_eq!(app.podxt().pod_selected, Some(5));
    click_pod(&context, &mut app, "pod-bank-down");
    assert_eq!(app.podxt().pod_bank, 0);
    assert_eq!(
        app.podxt().pod_library.tone(3).unwrap().as_bytes(),
        edited_d
    );
    assert_eq!(
        app.podxt()
            .pod_bundle
            .as_ref()
            .unwrap()
            .tone(1, 0)
            .unwrap()
            .as_bytes(),
        original_e
    );

    recording_frame(&context, &mut app, vec![]);
    assert!(app.podxt().pod_visible.contains("Revert restores"));
    click_pod(&context, &mut app, "revert-pod");
    click_pod(&context, &mut app, "revert-pod-cancel");
    assert_eq!(
        app.podxt().pod_library.tone(3).unwrap().as_bytes(),
        edited_d
    );
    click_pod(&context, &mut app, "revert-pod");
    click_pod(&context, &mut app, "revert-pod-confirm");
    assert_eq!(
        app.podxt().pod_library.tone(3).unwrap().as_bytes(),
        original_d
    );
    assert!(!app.podxt().pod_dirty());

    click_pod(&context, &mut app, "pod-channel-d");
    drag_pod_drive_slider_tall(&context, &mut app);
    click_pod(&context, &mut app, "pod-block-delay");
    click_pod(&context, &mut app, "pod-block-bypass");
    assert_eq!(
        app.podxt()
            .pod_library
            .tone(3)
            .unwrap()
            .model_enable_state(pod_preset::PodModelEnable::Delay)
            .unwrap(),
        pod_preset::ModelEnableState::Canonical(false)
    );
    assert_eq!(
        app.podxt()
            .pod_library
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
    assert!(!app.podxt().pod_dirty());
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
        app.podxt()
            .pod_bundle
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
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
        assert!(!hardware_gates::is_locked(Gate::FullBackupRead));
        assert!(!hardware_gates::is_locked(Gate::EditBufferWrite));
        assert!(!hardware_gates::is_locked(Gate::StoreRestore));
    }
}

#[test]
fn pod_tab_browses_thirty_two_banks_and_a_short_bank() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    app.select_test_editor::<PodxtEditor>();
    recording_frame(&context, &mut app, vec![]);
    choose_practice_tone(&context, &mut app, PracticeTone::AmpAndEffects);
    recording_frame(&context, &mut app, vec![]);
    click_pod(&context, &mut app, "pod-channel-b");
    assert_eq!(app.podxt().pod_selected, Some(0));
    click_pod(&context, &mut app, "pod-channel-d");
    assert_eq!(app.podxt().pod_selected, Some(0));
    assert!(app.podxt().pod_buttons.contains_key("pod-bank-up"));

    let tone = sample_stomp_model_tone(0x0205_0006);
    let banks: Vec<Vec<Vec<u8>>> = (0..32)
        .map(|_| (0..4).map(|_| tone.clone()).collect())
        .collect();
    app.podxt_mut()
        .load_pod_bundle_bytes(fixed_pod_banks(&banks))
        .unwrap();
    assert_eq!(app.podxt().pod_bundle.as_ref().unwrap().bank_count(), 32);
    recording_frame(&context, &mut app, vec![]);
    for bank in 0..32 {
        app.podxt_mut().show_pod_channel(bank, 3);
        recording_frame(&context, &mut app, vec![]);
        assert_eq!(app.podxt().pod_bank, bank);
        assert_eq!(app.podxt().pod_selected, Some(bank * 4 + 3));
        assert!(app.podxt().pod_buttons.contains_key("pod-channel-a"));
        assert!(app.podxt().pod_buttons.contains_key("pod-channel-d"));
    }
    click_pod(&context, &mut app, "pod-bank-down");
    assert_eq!(app.podxt().pod_bank, 30);
    click_pod(&context, &mut app, "pod-channel-a");
    assert_eq!(app.podxt().pod_selected, Some(30 * 4));

    let factory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("analysis/reports/podxt-live-3.01-factory/PODxt Live 3.01 Factory.vwb");
    if factory.is_file() {
        let bytes = std::fs::read(&factory).unwrap();
        app.podxt_mut().load_pod_bundle_bytes(bytes).unwrap();
        let bundle = app.podxt().pod_bundle.as_ref().unwrap();
        assert_eq!(bundle.bank_count(), 32);
        for bank in 0..32 {
            assert_eq!(bundle.preset_count(bank), Some(4));
        }
        app.select_test_editor::<PodxtEditor>();
        recording_frame(&context, &mut app, vec![]);
        click_pod(&context, &mut app, "pod-bank-up");
        click_pod(&context, &mut app, "pod-channel-d");
        assert_eq!(app.podxt().pod_bank, 1);
        assert_eq!(app.podxt().pod_selected, Some(7));
        app.podxt_mut().show_pod_channel(31, 0);
        recording_frame(&context, &mut app, vec![]);
        assert_eq!(app.podxt().pod_bank, 31);
        assert_eq!(
            app.podxt().pod_bundle.as_ref().unwrap().preset_count(31),
            Some(4)
        );
        click_pod(&context, &mut app, "fetch-pod");
        click_pod(&context, &mut app, "send-all");
        assert_eq!(app.podxt().pod_bank, 31);
    }
    const {
        assert!(!hardware_gates::is_locked(Gate::FullBackupRead));
    }
}

#[test]
fn l6pa_cab_stage_replacement_and_save_quit_failures_use_real_gui_paths() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    app.select_test_editor::<PodxtEditor>();
    let original =
        include_bytes!("../../../../tests/fixtures/pod-l6pa-cab-mic/cab.before.bin").to_vec();
    app.podxt_mut()
        .pod_library
        .import_bytes(original.clone())
        .unwrap();
    app.podxt_mut().pod_selected = Some(0);
    recording_frame(&context, &mut app, vec![]);

    choose_cab_popup(&context, &mut app, 0x0102_0002);
    assert_eq!(
        app.podxt().pod_cab_pending.unwrap().1.model_id(),
        0x0102_0002
    );
    let old_generation = app.podxt().pod_document_generation;
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
    app.podxt_mut()
        .load_pod_bundle_bytes(sample_pod_bundle(&[replacement.clone()]))
        .unwrap();
    assert_eq!(app.podxt().pod_document_generation, old_generation + 1);
    assert_eq!(app.podxt().pod_selected, Some(0));
    assert!(app.podxt().pod_cab_pending.is_none());
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        replacement
    );
    assert!(!app.podxt().pod_dirty());

    recording_frame(&context, &mut app, vec![]);
    choose_cab_popup(&context, &mut app, 0x0102_0003);
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        replacement
    );
    click_pod(&context, &mut app, "pod-cab-apply");
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().cab_edit_model_id(),
        Some(0x0102_0003)
    );
    assert!(app.podxt().pod_dirty());

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
    assert!(app.podxt().pod_dirty());
    assert!(!failed.exists());
    click_pod(&context, &mut app, "save-pod");
    assert!(app.podxt().pod_dirty());
    assert!(!failed.exists());
    click_pod(&context, &mut app, "save-pod");
    assert!(app.podxt().pod_dirty());
    assert_eq!(
        std::fs::read(&collision).unwrap(),
        b"keep original collision bytes"
    );
    click_pod(&context, &mut app, "save-pod");
    assert!(!app.podxt().pod_dirty());
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
    assert!(app.podxt().pod_dirty());
    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    click_prompt(&context, &mut app, "cancel");
    assert!(app.test_pending().is_none());
    assert!(app.podxt().pod_dirty());
    assert!(!app.permit_close);

    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    click_prompt(&context, &mut app, "discard");
    assert!(app.test_pending().is_none());
    assert!(!app.podxt().pod_dirty());
    assert!(app.permit_close);
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().cab_edit_model_id(),
        Some(0x0102_0003)
    );
    assert_eq!(
        app.podxt()
            .pod_library
            .tone(0)
            .unwrap()
            .mic_choice()
            .unwrap()
            .value(),
        2
    );
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
    }
}

#[test]
fn cab_mic_gui_apply_collision_save_reopen_revert_and_quit_cancel() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    app.select_test_editor::<PodxtEditor>();
    let original = sample_cab_mic_pod_tone(0x0102_0000, 1);
    app.podxt_mut().pod_library.import_bytes(original).unwrap();
    app.podxt_mut().pod_selected = Some(0);
    recording_frame(&context, &mut app, vec![]);

    choose_cab_popup(&context, &mut app, 0x0102_0002);
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().cab_model_id(),
        Some(0x0102_0000)
    );
    click_pod(&context, &mut app, "pod-cab-apply");
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().cab_model_id(),
        Some(0x0102_0002)
    );
    assert_eq!(
        app.podxt()
            .pod_library
            .tone(0)
            .unwrap()
            .mic_choice()
            .unwrap()
            .value(),
        1
    );
    assert!(app.podxt().pod_library.is_dirty(0));

    let directory = tempfile::tempdir().unwrap();
    let collision = directory.path().join("occupied.tone");
    std::fs::write(&collision, b"preserve existing destination").unwrap();
    let saved = directory.path().join("cab-copy.tone");
    app.dialogs = Box::new(FakeDialogs {
        pod_save: [Some(collision.clone()), Some(saved.clone())].into(),
        ..Default::default()
    });
    click_pod(&context, &mut app, "save-pod");
    assert!(app.podxt().pod_library.is_dirty(0));
    assert_eq!(
        std::fs::read(&collision).unwrap(),
        b"preserve existing destination"
    );
    click_pod(&context, &mut app, "save-pod");
    assert!(!app.podxt().pod_library.is_dirty(0));
    let reopened = pod_preset::PodTone::parse(std::fs::read(&saved).unwrap()).unwrap();
    assert_eq!(reopened.cab_model_id(), Some(0x0102_0002));
    assert_eq!(reopened.mic_choice().unwrap().value(), 1);

    choose_cab_popup(&context, &mut app, 0x0102_0003);
    click_pod(&context, &mut app, "pod-cab-apply");
    assert!(app.podxt().pod_library.is_dirty(0));
    app.podxt_mut().pod_library.revert_unsaved(0).unwrap();
    app.podxt_mut().refresh_selected_pod_name();
    assert_eq!(app.podxt().pod_cab_pending, None);
    recording_frame(&context, &mut app, vec![]);
    assert_eq!(
        app.podxt().pod_cab_pending.unwrap().1.model_id(),
        0x0102_0002
    );

    click_pod(&context, &mut app, "pod-mic-3");
    assert!(app.podxt().pod_library.is_dirty(0));
    assert_eq!(
        app.podxt()
            .pod_library
            .tone(0)
            .unwrap()
            .mic_choice()
            .unwrap()
            .value(),
        3
    );
    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    click_prompt(&context, &mut app, "cancel");
    assert!(app.podxt().pod_library.is_dirty(0));
    assert!(!app.permit_close);
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
    }
}

fn select_delay_heads_choice(
    context: &egui::Context,
    app: &mut Workbench,
    choice: pod_preset::DelayHeadsSelection,
) {
    recording_frame_tall(context, app, vec![]);
    let choice_rect = app.podxt().pod_delay_heads_choices[&choice];
    click_pod_at_tall(context, app, choice_rect.center());
    recording_frame_tall(context, app, vec![]);
}

fn select_delay_bits_choice(
    context: &egui::Context,
    app: &mut Workbench,
    choice: pod_preset::DelayBitsSelection,
) {
    recording_frame_tall(context, app, vec![]);
    let choice_rect = app.podxt().pod_delay_bits_choices[&choice];
    click_pod_at_tall(context, app, choice_rect.center());
    recording_frame_tall(context, app, vec![]);
}

fn select_stomp_wave_choice(
    context: &egui::Context,
    app: &mut Workbench,
    choice: pod_preset::StompWaveSelection,
) {
    recording_frame_tall(context, app, vec![]);
    let choice_rect = app.podxt().pod_stomp_wave_choices[&choice];
    click_pod_at_tall(context, app, choice_rect.center());
    recording_frame_tall(context, app, vec![]);
}

fn stage_stomp_harmony1_choice(
    context: &egui::Context,
    app: &mut Workbench,
    choice: pod_preset::StompHarmony1M335Selection,
) {
    recording_frame_tall(context, app, vec![]);
    let rect = app.podxt().pod_harmony1_choices[&choice];
    click_pod_at_tall(context, app, rect.center());
    recording_frame_tall(context, app, vec![]);
}

fn stage_delay_note_choice(
    context: &egui::Context,
    app: &mut Workbench,
    choice: pod_preset::DelayNoteChoice,
) {
    recording_frame_tall(context, app, vec![]);
    let option = app.podxt().pod_delay_note_options[&choice];
    click_pod_at_tall(context, app, option.center());
    recording_frame_tall(context, app, vec![]);
}

fn choose_delay_model_popup(context: &egui::Context, app: &mut Workbench, model_id: u32) {
    recording_frame_tall(context, app, vec![]);
    let combo = app
        .podxt()
        .pod_delay_model_combo
        .expect("Delay model selector is rendered");
    click_pod_at_tall(context, app, combo.center());
    recording_frame_tall(context, app, vec![]);
    let option = app.podxt().pod_delay_model_options[&model_id];
    click_pod_at_tall(context, app, option.center());
    recording_frame_tall(context, app, vec![]);
}

fn choose_mod_model_popup(context: &egui::Context, app: &mut Workbench, model_id: u32) {
    recording_frame_tall(context, app, vec![]);
    let combo = app
        .podxt()
        .pod_mod_model_combo
        .expect("Mod model selector is rendered");
    click_pod_at_tall(context, app, combo.center());
    recording_frame_tall(context, app, vec![]);
    let option = app.podxt().pod_mod_model_options[&model_id];
    click_pod_at_tall(context, app, option.center());
    recording_frame_tall(context, app, vec![]);
}

fn choose_stomp_model_popup(context: &egui::Context, app: &mut Workbench, model_id: u32) {
    recording_frame_tall(context, app, vec![]);
    let combo = app
        .podxt()
        .pod_stomp_model_combo
        .expect("Stomp model selector is rendered");
    click_pod_at_tall(context, app, combo.center());
    recording_frame_tall(context, app, vec![]);
    let option = app.podxt().pod_stomp_model_options[&model_id];
    click_pod_at_tall(context, app, option.center());
    recording_frame_tall(context, app, vec![]);
}

fn choose_reverb_model_popup(context: &egui::Context, app: &mut Workbench, model_id: u32) {
    recording_frame_tall(context, app, vec![]);
    let combo = app
        .podxt()
        .pod_reverb_model_combo
        .expect("Reverb model selector is rendered");
    click_pod_at_tall(context, app, combo.center());
    recording_frame_tall(context, app, vec![]);
    let option = app.podxt().pod_reverb_model_options[&model_id];
    click_pod_at_tall(context, app, option.center());
    recording_frame_tall(context, app, vec![]);
}

fn choose_amp_model_popup(context: &egui::Context, app: &mut Workbench, model_id: u32) {
    recording_frame_tall(context, app, vec![]);
    let combo = app
        .podxt()
        .pod_amp_model_combo
        .expect("AMP model selector is rendered");
    click_pod_at_tall(context, app, combo.center());
    recording_frame_tall(context, app, vec![]);
    assert_eq!(app.podxt().pod_amp_model_options.len(), 43);
    let option = app.podxt().pod_amp_model_options[&model_id];
    click_pod_at_tall(context, app, option.center());
    recording_frame_tall(context, app, vec![]);
}

fn click_pod_button_tall(context: &egui::Context, app: &mut Workbench, id: &'static str) {
    recording_frame_tall(context, app, vec![]);
    let position = app.podxt().pod_buttons[id].center();
    click_pod_at_tall(context, app, position);
}

fn stage_mod_note_choice(
    context: &egui::Context,
    app: &mut Workbench,
    choice: pod_preset::ModNoteChoice,
) {
    recording_frame_tall(context, app, vec![]);
    let option = app.podxt().pod_mod_note_options[&choice];
    click_pod_at_tall(context, app, option.center());
    recording_frame_tall(context, app, vec![]);
}

fn drag_delay_time_slider(context: &egui::Context, app: &mut Workbench, fraction: f32) {
    recording_frame_tall(context, app, vec![]);
    let rect = app
        .podxt()
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
    let rect = app
        .podxt()
        .pod_mod_speed_slider
        .expect("visible Mod SPEED slider");
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
    let rect = app.podxt().pod_harmony2_choices[&choice];
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
    let rect = app
        .podxt()
        .pod_drive_slider
        .expect("visible Amp Drive slider");
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
    let rect = app
        .podxt()
        .pod_drive_slider
        .expect("visible Amp Drive slider");
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
    let rect = app.podxt().pod_stomp_sliders[&control];
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
    let rect = app.podxt().pod_whammy_knob_sliders[&control];
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
    let rect = app.podxt().pod_eq_frequency_sliders[&band];
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
    let rect = app.podxt().pod_eq_gain_sliders[&band];
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
    let rect = app.podxt().pod_mod_sliders[&control];
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
    let rect = app.podxt().pod_reverb_sliders[&control];
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
    let rect = app.podxt().pod_delay_sliders[&control];
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
    let guitar = app.variax().document.as_ref().unwrap().as_bytes().to_vec();
    let directory = tempfile::tempdir().unwrap();
    let blocked = directory.path().join("not-a-tone");
    std::fs::create_dir(&blocked).unwrap();
    app.dialogs = Box::new(FakeDialogs {
        pod_open: [Some(blocked)].into(),
        ..Default::default()
    });
    click_pod(&context, &mut app, "open-pod");
    assert!(app.test_editor_is_active::<PodxtEditor>());
    assert!(app.podxt().pod_visible.contains(pod_preset::OFFLINE_BANNER));
    assert!(app.podxt().pod_visible.contains(pod_preset::NOT_GUITAR));
    assert!(app.podxt().pod_visible.contains(pod_preset::NOT_SENT));
    assert!(app.podxt().pod_visible.contains("remain raw"));
    assert!(app.variax().read_worker.is_none());
    assert!(!app.variax().is_reading());
    assert_eq!(app.podxt().pod_library.len(), 0);
    click_pod_button_tall(&context, &mut app, "load-pod");
    assert!(app.podxt().pod_message.contains("regular file"));
    assert!(app.podxt().pod_message.contains("not opened"));
    assert!(app.podxt().pod_library.is_empty());
    assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), guitar);

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
    assert!(app.podxt().pod_visible.contains("L6AS"));
    assert!(app.podxt().pod_visible.contains("11110002"));
    assert!(app.podxt().pod_visible.contains("22220003"));
    assert!(app.podxt().pod_visible.contains("remain raw"));
    assert!(
        app.podxt().pod_message.contains("Nothing is sent")
            || app.podxt().pod_visible.contains(pod_preset::NOT_SENT)
    );
    assert_eq!(app.podxt().pod_library.len(), 1);
    assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), guitar);
    assert!(app.variax().read_worker.is_none());
    app.podxt_mut().pod_name = "Stage".into();
    click_pod(&context, &mut app, "apply-pod-name");
    assert!(app.podxt().pod_message.contains("Nothing was sent"));
    assert!(app.podxt().pod_visible.contains("Stage"));
    assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), guitar);
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
    assert!(!app.podxt().pod_message.starts_with("Saved a new copy"));
    assert_eq!(std::fs::read(&saved).unwrap(), written);
    click_pod(&context, &mut app, "load-pod");
    assert!(app.podxt().pod_message.contains("Variax"));
    assert_eq!(app.podxt().pod_library.len(), 1);
    assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), guitar);
    app.variax_mut().start_read();
    assert!(app.variax().read_worker.is_none());
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
    }
}

#[test]
fn pod_amp_model_apply_preserves_save_revert_and_quit_guards() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    app.select_test_editor::<PodxtEditor>();
    let mut original = sample_drive_pod_tone();
    original[8..12].copy_from_slice(b"L6PA");
    app.podxt_mut()
        .pod_library
        .import_bytes(original.clone())
        .unwrap();
    app.podxt_mut().pod_selected = Some(0);
    recording_frame(&context, &mut app, vec![]);
    assert!(app.podxt().pod_drive_slider.is_some());

    let no_amp = pod_preset::PodAmpModelChoice::NoAmp;
    click_pod(&context, &mut app, "pod-amp-no-amp");
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().amp_model_id(),
        Some(no_amp.model_id())
    );
    assert!(app.podxt().pod_library.is_dirty(0));
    assert!(app.podxt().pod_drive_slider.is_none());
    assert!(app.podxt().pod_message.contains("Nothing was sent"));

    choose_amp_model_popup(&context, &mut app, 0x0002_002e);
    let pending = app.podxt().pod_amp_model_pending;
    assert!(pending.is_some());
    assert!(app.podxt().pod_library.is_dirty(0));

    let directory = tempfile::tempdir().unwrap();
    let collision = directory.path().join("existing.tone");
    std::fs::write(&collision, b"keep original").unwrap();
    app.dialogs = Box::new(FakeDialogs {
        pod_save: [Some(collision.clone()), None].into(),
        ..Default::default()
    });
    click_pod(&context, &mut app, "save-pod");
    assert!(app.podxt().pod_library.is_dirty(0));
    assert_eq!(std::fs::read(&collision).unwrap(), b"keep original");
    assert_eq!(app.podxt().pod_amp_model_pending, pending);
    click_pod(&context, &mut app, "save-pod");
    assert!(app.podxt().pod_library.is_dirty(0));
    assert!(app.podxt().pod_message.contains("cancelled"));
    assert_eq!(app.podxt().pod_amp_model_pending, pending);

    let saved = directory.path().join("saved.tone");
    app.dialogs.queue_pod_save(Some(saved.clone()));
    click_pod(&context, &mut app, "save-pod");
    assert!(!app.podxt().pod_library.is_dirty(0));
    assert_eq!(app.podxt().pod_amp_model_pending, pending);
    let reopened = pod_preset::PodTone::parse(std::fs::read(&saved).unwrap()).unwrap();
    assert_eq!(reopened.amp_model_id(), Some(no_amp.model_id()));
    click_pod(&context, &mut app, "pod-amp-no-amp");
    assert!(!app.podxt().pod_library.is_dirty(0));
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().amp_model_id(),
        Some(no_amp.model_id())
    );

    let clean = pod_preset::PodAmpModelChoice::Line6Clean;
    assert_eq!(reopened.amp_model_id(), Some(no_amp.model_id()));
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().amp_model_id(),
        Some(no_amp.model_id())
    );
    click_pod(&context, &mut app, "pod-amp-line6-clean");
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().amp_model_id(),
        Some(clean.model_id())
    );
    assert!(app.podxt().pod_library.is_dirty(0));
    choose_amp_model_popup(&context, &mut app, 0x0002_002e);
    let pending_before_close = app.podxt().pod_amp_model_pending;

    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    assert!(!app.permit_close);
    click_prompt(&context, &mut app, "cancel");
    assert!(app.podxt().pod_library.is_dirty(0));
    assert_eq!(app.podxt().pod_amp_model_pending, pending_before_close);
    assert!(!app.permit_close);
    app.request_close(&context);
    click_prompt(&context, &mut app, "discard");
    assert!(app.permit_close);
    assert_eq!(app.podxt().pod_amp_model_pending, None);
    assert!(!app.podxt().pod_library.is_dirty(0));
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().amp_model_id(),
        Some(no_amp.model_id())
    );
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
    }
}

#[test]
fn pod_amp_combo_stages_all_choices_and_applies_from_pack_gated_current_id() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    app.select_test_editor::<PodxtEditor>();
    let mut original = sample_drive_pod_tone();
    original[8..12].copy_from_slice(b"L6PA");
    let old_id = pod_preset::PLEXI_45_MODEL;
    let marker = [old_id.to_be_bytes(), pod_preset::AMP_SLOT.to_be_bytes()].concat();
    let amp = original
        .windows(marker.len())
        .position(|window| window == marker)
        .unwrap();
    original[amp..amp + 4].copy_from_slice(&0x0003_0001u32.to_be_bytes());
    app.podxt_mut()
        .pod_library
        .import_bytes(original.clone())
        .unwrap();
    app.podxt_mut().pod_selected = Some(0);
    recording_frame_tall(&context, &mut app, vec![]);
    assert!(!app.podxt().pod_library.is_dirty(0));
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().amp_model_id(),
        Some(0x0003_0001)
    );

    app.podxt_mut().pod_reverb_model_pending = Some((
        0,
        pod_preset::PodReverbModelChoice::from_model_id(pod_preset::REVERB_CONTROL_MODEL_IDS[1])
            .unwrap(),
    ));
    choose_amp_model_popup(&context, &mut app, 0x0002_002e);
    assert_eq!(
        app.podxt().pod_amp_model_pending.unwrap().2.model_id(),
        0x0002_002e
    );
    assert!(!app.podxt().pod_library.is_dirty(0));
    click_pod_button_tall(&context, &mut app, "pod-amp-model-apply");
    recording_frame_tall(&context, &mut app, vec![]);

    let changed = app.podxt().pod_library.tone(0).unwrap();
    assert_eq!(changed.amp_model_id(), Some(0x0002_002e));
    assert!(app.podxt().pod_library.is_dirty(0));
    assert_eq!(
        app.podxt().pod_reverb_model_pending.unwrap().1.model_id(),
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
    assert_eq!(app.podxt().pod_amp_model_pending, None);
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
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
        app.select_test_editor::<PodxtEditor>();
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
        app.podxt_mut().pod_library.import_bytes(original).unwrap();
        app.podxt_mut().pod_selected = Some(0);
        recording_frame_tall(&context, &mut app, vec![]);
        choose_amp_model_popup(&context, &mut app, 0x0002_0000);
        click_pod_button_tall(&context, &mut app, "pod-amp-model-apply");
        recording_frame_tall(&context, &mut app, vec![]);
        assert_eq!(
            app.podxt().pod_library.tone(0).unwrap().amp_model_id(),
            Some(0x0002_0000),
            "current {current_id:08x}"
        );
        assert!(
            app.podxt().pod_library.is_dirty(0),
            "current {current_id:08x}"
        );
    }
}

#[test]
fn pod_amp_failed_apply_retains_pending_choice_and_tone_bytes() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    app.select_test_editor::<PodxtEditor>();
    let original = as_fixed_bank_tone(sample_drive_pod_tone());
    app.podxt_mut()
        .pod_library
        .import_bytes(original.clone())
        .unwrap();
    let sibling = as_fixed_bank_tone(sample_drive_pod_tone());
    app.podxt_mut()
        .pod_library
        .import_bytes(sibling.clone())
        .unwrap();
    app.podxt_mut()
        .pod_library
        .set_amp_drive_fraction(0, 0.7)
        .unwrap();
    app.podxt_mut()
        .pod_library
        .set_amp_drive_fraction(1, 0.8)
        .unwrap();
    app.podxt_mut().pod_selected = Some(0);
    let choice = pod_preset::PodAmpModelChoice::Model2002e;
    recording_frame_tall(&context, &mut app, vec![]);
    choose_amp_model_popup(&context, &mut app, choice.model_id());
    let pending = Some((0, pod_preset::PLEXI_45_MODEL, choice));
    assert_eq!(app.podxt().pod_amp_model_pending, pending);
    app.podxt_mut().pod_reverb_model_pending = Some((
        0,
        pod_preset::PodReverbModelChoice::from_model_id(pod_preset::REVERB_CONTROL_MODEL_IDS[1])
            .unwrap(),
    ));

    let before_edit = app.podxt().pod_library.tone(0).unwrap().as_bytes().to_vec();
    let mut stale_profile = before_edit.clone();
    let pinf = stale_profile
        .windows(4)
        .position(|bytes| bytes == b"PINF")
        .unwrap();
    let version = pinf + 8 + 72;
    stale_profile[version..version + 4]
        .copy_from_slice(&(pod_preset::PODXT_LIVE_HOST_MIN_VERSION - 1).to_be_bytes());
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().amp_model_id(),
        Some(pod_preset::PLEXI_45_MODEL)
    );
    app.podxt_mut().replace_pod_tone(0, stale_profile).unwrap();
    let before = app.podxt().pod_library.tone(0).unwrap().as_bytes().to_vec();
    assert_eq!(app.podxt().pod_amp_model_pending, pending);
    assert!(app.podxt().pod_library.is_dirty(0));
    assert!(app.podxt().pod_library.is_dirty(1));

    click_pod_button_tall(&context, &mut app, "pod-amp-model-apply");
    assert_eq!(app.podxt().pod_library.tone(0).unwrap().as_bytes(), before);
    assert_eq!(
        pod_preset::PodTone::parse(before.clone()).unwrap(),
        *app.podxt().pod_library.tone(0).unwrap()
    );
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().amp_model_id(),
        Some(pod_preset::PLEXI_45_MODEL)
    );
    assert_eq!(app.podxt().pod_amp_model_pending, pending);
    assert_eq!(
        app.podxt().pod_reverb_model_pending.unwrap().1.model_id(),
        pod_preset::REVERB_CONTROL_MODEL_IDS[1]
    );
    assert!(app.podxt().pod_library.is_dirty(0));
    assert!(app.podxt().pod_library.is_dirty(1));
    assert_eq!(app.podxt().pod_library.tone(1).unwrap().as_bytes(), {
        let mut expected = sibling.clone();
        let mut dirty = pod_preset::PodTone::parse(expected.clone()).unwrap();
        dirty
            .set_amp_control_fraction(pod_preset::AmpControl::Drive, 0.8)
            .unwrap();
        expected = dirty.as_bytes().to_vec();
        expected
    });

    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    assert_eq!(app.podxt().pod_amp_model_pending, pending);
    click_prompt(&context, &mut app, "discard");
    assert!(!app.podxt().pod_library.is_dirty(0));
    assert!(app.podxt().pod_library.is_dirty(1));
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        original
    );
    assert_eq!(app.podxt().pod_amp_model_pending, pending);
    click_prompt(&context, &mut app, "discard");
    assert!(!app.podxt().pod_library.is_dirty(1));
    assert_eq!(app.podxt().pod_library.tone(1).unwrap().as_bytes(), sibling);
    assert_eq!(app.podxt().pod_amp_model_pending, pending);
    click_prompt(&context, &mut app, "amp-discard");
    assert_eq!(app.podxt().pod_amp_model_pending, None);
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
    }
}

#[test]
fn pod_amp_pending_survives_legacy_rejection_and_unrelated_edit() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    app.select_test_editor::<PodxtEditor>();
    let mut tone = sample_drive_pod_tone();
    tone[8..12].copy_from_slice(b"L6PA");
    let pinf = tone.windows(4).position(|bytes| bytes == b"PINF").unwrap();
    let version = pinf + 8 + 72;
    tone[version..version + 4]
        .copy_from_slice(&(pod_preset::PODXT_LIVE_HOST_MIN_VERSION - 1).to_be_bytes());
    app.podxt_mut()
        .pod_library
        .import_bytes(tone.clone())
        .unwrap();
    app.podxt_mut().pod_selected = Some(0);
    recording_frame_tall(&context, &mut app, vec![]);
    choose_amp_model_popup(&context, &mut app, 0x0002_002e);
    let pending = Some((
        0,
        pod_preset::PLEXI_45_MODEL,
        pod_preset::PodAmpModelChoice::Model2002e,
    ));
    assert_eq!(app.podxt().pod_amp_model_pending, pending);
    assert!(!app.podxt().pod_library.is_dirty(0));
    click_pod_button_tall(&context, &mut app, "pod-amp-no-amp");
    recording_frame_tall(&context, &mut app, vec![]);
    assert_eq!(app.podxt().pod_library.tone(0).unwrap().as_bytes(), tone);
    assert!(!app.podxt().pod_library.is_dirty(0));
    assert_eq!(app.podxt().pod_amp_model_pending, pending);

    let mut tone = sample_drive_pod_tone();
    tone[8..12].copy_from_slice(b"L6PA");
    app.podxt_mut().pod_library = pod_preset::Library::default();
    app.podxt_mut().pod_library.import_bytes(tone).unwrap();
    app.podxt_mut().pod_amp_model_pending = None;
    app.podxt_mut().pod_selected = Some(0);
    recording_frame_tall(&context, &mut app, vec![]);
    choose_amp_model_popup(&context, &mut app, 0x0002_002e);
    drag_pod_drive_slider(&context, &mut app);
    assert_eq!(app.podxt().pod_amp_model_pending, pending);
    assert!(app.podxt().pod_library.is_dirty(0));
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().amp_model_id(),
        Some(pod_preset::PLEXI_45_MODEL)
    );
}

#[test]
fn pod_amp_stage_only_quit_resolves_failed_apply_and_other_quit_prompts() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    app.select_test_editor::<PodxtEditor>();
    let mut original = as_fixed_bank_tone(sample_drive_pod_tone());
    let pinf = original
        .windows(4)
        .position(|bytes| bytes == b"PINF")
        .unwrap();
    original[pinf + 8 + 72..pinf + 8 + 76]
        .copy_from_slice(&(pod_preset::PODXT_LIVE_HOST_MIN_VERSION - 1).to_be_bytes());
    app.podxt_mut()
        .pod_library
        .import_bytes(original.clone())
        .unwrap();
    app.podxt_mut().pod_selected = Some(0);
    recording_frame_tall(&context, &mut app, vec![]);
    choose_amp_model_popup(&context, &mut app, 0x0002_002e);
    let pending = Some((
        0,
        pod_preset::PLEXI_45_MODEL,
        pod_preset::PodAmpModelChoice::Model2002e,
    ));
    assert_eq!(app.podxt().pod_amp_model_pending, pending);
    assert!(!app.podxt().pod_library.is_dirty(0));

    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    assert!(!app.permit_close);
    click_prompt(&context, &mut app, "amp-apply");
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    assert_eq!(app.podxt().pod_amp_model_pending, pending);
    assert!(!app.permit_close);
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        original
    );
    click_prompt(&context, &mut app, "amp-keep-editing");
    assert!(app.test_pending().is_none());
    assert_eq!(app.podxt().pod_amp_model_pending, pending);
    assert!(!app.permit_close);

    app.variax_mut()
        .change(|preset| preset.rename("Staged choice"));
    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::Quit)));
    click_prompt(&context, &mut app, "discard");
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    assert_eq!(app.podxt().pod_amp_model_pending, pending);
    click_prompt(&context, &mut app, "amp-discard");
    assert!(app.permit_close);
    assert!(app.test_pending().is_none());
    assert_eq!(app.podxt().pod_amp_model_pending, None);
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        original
    );
    assert!(!app.podxt().pod_library.is_dirty(0));

    let (context, mut app) = super::super::tests::app();
    context.style_mut(|style| style.animation_time = 0.0);
    app.select_test_editor::<PodxtEditor>();
    let original = as_fixed_bank_tone(sample_drive_pod_tone());
    app.podxt_mut()
        .pod_library
        .import_bytes(original.clone())
        .unwrap();
    app.podxt_mut().pod_selected = Some(0);
    recording_frame_tall(&context, &mut app, vec![]);
    choose_amp_model_popup(&context, &mut app, 0x0002_002e);
    let pending = app.podxt().pod_amp_model_pending;
    app.request_close(&context);
    click_prompt(&context, &mut app, "amp-apply");
    assert_eq!(app.podxt().pod_amp_model_pending, None);
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().amp_model_id(),
        Some(pod_preset::PodAmpModelChoice::Model2002e.model_id())
    );
    assert!(app.podxt().pod_library.is_dirty(0));
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    assert!(!app.permit_close);
    click_prompt(&context, &mut app, "discard");
    assert!(app.permit_close);
    assert_eq!(app.podxt().pod_amp_model_pending, None);
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        original
    );
    assert!(!app.podxt().pod_library.is_dirty(0));
    assert!(pending.is_some());

    let (context, mut app) = super::super::tests::app();
    context.style_mut(|style| style.animation_time = 0.0);
    app.select_test_editor::<PodxtEditor>();
    app.podxt_mut()
        .pod_library
        .import_bytes(original.clone())
        .unwrap();
    app.podxt_mut().pod_selected = Some(0);
    recording_frame_tall(&context, &mut app, vec![]);
    choose_amp_model_popup(&context, &mut app, 0x0002_002e);
    let staged_before_edit = app.podxt().pod_amp_model_pending;
    app.podxt_mut()
        .pod_library
        .set_amp_drive_fraction(0, 0.6)
        .unwrap();
    assert!(app.podxt().pod_library.is_dirty(0));
    app.request_close(&context);
    assert_eq!(app.podxt().pod_amp_model_pending, staged_before_edit);
    click_prompt(&context, &mut app, "discard");
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        original
    );
    assert!(!app.podxt().pod_library.is_dirty(0));
    assert_eq!(app.podxt().pod_amp_model_pending, staged_before_edit);
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    click_prompt(&context, &mut app, "amp-discard");
    assert_eq!(app.podxt().pod_amp_model_pending, None);
    assert!(app.permit_close);
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
    }
}

#[test]
fn pod_amp_stage_load_navigation_and_explicit_discard_are_preserving() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    app.select_test_editor::<PodxtEditor>();
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
    app.podxt_mut()
        .pod_library
        .import_bytes(original.clone())
        .unwrap();
    app.podxt_mut().pod_selected = Some(0);
    recording_frame_tall(&context, &mut app, vec![]);
    choose_amp_model_popup(&context, &mut app, 0x0002_002e);
    let pending = app.podxt().pod_amp_model_pending;
    click_pod_button_tall(&context, &mut app, "load-pod");
    assert_eq!(app.podxt().pod_amp_model_pending, pending);
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        original
    );
    assert!(!app.podxt().pod_library.is_dirty(0));

    click_pod_button_tall(&context, &mut app, "load-pod");
    assert_eq!(app.podxt().pod_amp_model_pending, None);
    assert_eq!(app.podxt().pod_library.len(), 2);
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        original
    );
    assert_eq!(
        app.podxt().pod_library.tone(1).unwrap().as_bytes(),
        reopened
    );
    assert!(!app.podxt().pod_library.is_dirty(0));
    assert!(!app.podxt().pod_library.is_dirty(1));

    app.podxt_mut().pod_selected = Some(0);
    choose_amp_model_popup(&context, &mut app, 0x0002_002e);
    let navigation_stage = Some((
        0,
        pod_preset::PLEXI_45_MODEL,
        pod_preset::PodAmpModelChoice::Model2002e,
    ));
    assert_eq!(app.podxt().pod_amp_model_pending, navigation_stage);
    let first_baseline = app.podxt().pod_library.tone(0).unwrap().as_bytes().to_vec();
    recording_frame_tall(&context, &mut app, vec![]);
    let row = app.podxt().pod_rows[&1].center();
    click_pod_at_tall(&context, &mut app, row);
    assert_eq!(app.podxt().pod_selected, Some(1));
    assert_eq!(app.podxt().pod_amp_model_pending, None);
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        first_baseline
    );
    assert_eq!(
        app.podxt().pod_library.tone(1).unwrap().as_bytes(),
        reopened
    );

    app.podxt_mut().pod_selected = Some(1);
    recording_frame_tall(&context, &mut app, vec![]);
    choose_amp_model_popup(&context, &mut app, 0x0002_002e);
    let explicit_pending = app.podxt().pod_amp_model_pending;
    app.podxt_mut().pod_reverb_model_pending = Some((
        1,
        pod_preset::PodReverbModelChoice::from_model_id(pod_preset::REVERB_CONTROL_MODEL_IDS[1])
            .unwrap(),
    ));
    click_pod_button_tall(&context, &mut app, "pod-amp-model-discard");
    assert_eq!(app.podxt().pod_amp_model_pending, None);
    assert_eq!(
        app.podxt().pod_library.tone(1).unwrap().as_bytes(),
        reopened
    );
    assert!(!app.podxt().pod_library.is_dirty(1));
    assert_eq!(
        app.podxt().pod_reverb_model_pending.unwrap().1.model_id(),
        pod_preset::REVERB_CONTROL_MODEL_IDS[1]
    );
    assert!(explicit_pending.is_some());

    let (context, mut app) = super::super::tests::app();
    context.style_mut(|style| style.animation_time = 0.0);
    app.select_test_editor::<PodxtEditor>();
    app.dialogs = Box::new(FakeDialogs {
        pod_open: [Some(bundle_path)].into(),
        ..Default::default()
    });
    app.podxt_mut().pod_library.import_bytes(original).unwrap();
    app.podxt_mut().pod_selected = Some(0);
    recording_frame_tall(&context, &mut app, vec![]);
    choose_amp_model_popup(&context, &mut app, 0x0002_002e);
    assert!(app.podxt().pod_amp_model_pending.is_some());
    click_pod_button_tall(&context, &mut app, "load-pod");
    assert_eq!(app.podxt().pod_amp_model_pending, None);
    assert_eq!(app.podxt().pod_library.len(), 1);
    assert_eq!(app.podxt().pod_library.tone(0).unwrap().as_bytes(), second);
    assert_eq!(
        app.podxt()
            .pod_bundle
            .as_ref()
            .unwrap()
            .tone(0, 0)
            .unwrap()
            .as_bytes(),
        second
    );
    assert!(!app.podxt().pod_library.is_dirty(0));
    assert!(!app.podxt().pod_bundle.as_ref().unwrap().is_dirty());
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
    }
}

#[test]
fn pod_amp_bundle_quit_discard_preserves_only_matching_stage_origin() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    app.select_test_editor::<PodxtEditor>();
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
    assert!(
        app.podxt().pod_bundle.is_some(),
        "{}",
        app.podxt().pod_message
    );
    assert_eq!(
        app.podxt().pod_bundle.as_ref().unwrap().as_bytes(),
        original_bundle.as_slice()
    );
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        original_tone
    );
    assert!(!app.podxt().pod_dirty());

    drag_pod_drive_slider_tall(&context, &mut app);
    assert!(app.podxt().pod_library.is_dirty(0));
    assert!(app.podxt().pod_bundle.as_ref().unwrap().is_dirty());
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().amp_model_id(),
        Some(pod_preset::PLEXI_45_MODEL)
    );
    choose_amp_model_popup(&context, &mut app, 0x0002_002e);
    let pending = Some((
        0,
        pod_preset::PLEXI_45_MODEL,
        pod_preset::PodAmpModelChoice::Model2002e,
    ));
    assert_eq!(app.podxt().pod_amp_model_pending, pending);
    let dirty_tone = app.podxt().pod_library.tone(0).unwrap().as_bytes().to_vec();
    let dirty_bundle = app.podxt().pod_bundle.as_ref().unwrap().as_bytes().to_vec();
    assert_ne!(dirty_tone, original_tone);
    assert_ne!(dirty_bundle, original_bundle);
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        dirty_tone
    );
    assert_eq!(
        app.podxt().pod_bundle.as_ref().unwrap().as_bytes(),
        dirty_bundle
    );

    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    assert!(!app.permit_close);
    click_prompt(&context, &mut app, "discard");
    assert_eq!(
        app.podxt().pod_bundle.as_ref().unwrap().as_bytes(),
        original_bundle
    );
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        original_tone
    );
    assert!(!app.podxt().pod_dirty());
    assert_eq!(app.podxt().pod_amp_model_pending, pending);
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    assert!(!app.permit_close);

    click_prompt(&context, &mut app, "amp-keep-editing");
    assert!(app.test_pending().is_none());
    assert_eq!(app.podxt().pod_amp_model_pending, pending);
    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    click_prompt(&context, &mut app, "amp-discard");
    assert_eq!(app.podxt().pod_amp_model_pending, None);
    assert!(app.permit_close);

    let (context, mut app) = super::super::tests::app();
    context.style_mut(|style| style.animation_time = 0.0);
    app.select_test_editor::<PodxtEditor>();
    app.dialogs = Box::new(FakeDialogs {
        pod_open: [Some(bundle_path)].into(),
        ..Default::default()
    });
    app.load_pod_tone();
    recording_frame_tall(&context, &mut app, vec![]);
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        original_tone
    );
    click_pod_button_tall(&context, &mut app, "pod-amp-line6-clean");
    assert!(app.podxt().pod_bundle.as_ref().unwrap().is_dirty());
    choose_amp_model_popup(&context, &mut app, 0x0002_002e);
    let invalidated_pending = Some((
        0,
        pod_preset::PodAmpModelChoice::Line6Clean.model_id(),
        pod_preset::PodAmpModelChoice::Model2002e,
    ));
    assert_eq!(app.podxt().pod_amp_model_pending, invalidated_pending);
    app.request_close(&context);
    click_prompt(&context, &mut app, "discard");
    assert_eq!(
        app.podxt().pod_bundle.as_ref().unwrap().as_bytes(),
        original_bundle
    );
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        original_tone
    );
    assert_eq!(app.podxt().pod_amp_model_pending, None);
    assert!(app.permit_close);
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
    }
}

#[test]
fn pod_amp_context_change_clears_stale_stage_and_bundle_rejection_is_atomic() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    app.select_test_editor::<PodxtEditor>();
    let first = as_fixed_bank_tone(sample_drive_pod_tone());
    let second = first.clone();
    app.podxt_mut()
        .pod_library
        .import_bytes(first.clone())
        .unwrap();
    app.podxt_mut()
        .pod_library
        .import_bytes(second.clone())
        .unwrap();
    app.podxt_mut()
        .pod_library
        .set_amp_drive_fraction(1, 0.65)
        .unwrap();
    app.podxt_mut().pod_selected = Some(1);
    let pending = Some((
        1,
        pod_preset::PLEXI_45_MODEL,
        pod_preset::PodAmpModelChoice::Model2002e,
    ));
    app.podxt_mut().pod_amp_model_pending = pending;
    app.podxt_mut().pod_bundle = Some(
        pod_preset::PodBundleLibrary::parse(sample_pod_bundle(std::slice::from_ref(&first)))
            .unwrap(),
    );
    let dirty_second = app.podxt().pod_library.tone(1).unwrap().as_bytes().to_vec();
    let replacement = Session::initial_rig()
        .set_pod_amp_model_choice(
            RoleKind::Pod,
            &dirty_second,
            pod_preset::PodAmpModelChoice::Model2002e,
        )
        .unwrap();
    let before_bundle = app.podxt().pod_bundle.as_ref().unwrap().as_bytes().to_vec();
    let before_tone = app.podxt().pod_library.tone(1).unwrap().as_bytes().to_vec();
    let baseline = second.clone();
    assert!(app.podxt_mut().replace_pod_tone(1, replacement).is_err());
    assert_eq!(app.podxt().pod_library.tone(0).unwrap().as_bytes(), first);
    assert_eq!(
        app.podxt().pod_library.tone(1).unwrap().as_bytes(),
        before_tone
    );
    assert_eq!(
        pod_preset::PodTone::parse(before_tone.clone()).unwrap(),
        *app.podxt().pod_library.tone(1).unwrap()
    );
    assert!(!app.podxt().pod_library.is_dirty(0));
    assert!(app.podxt().pod_library.is_dirty(1));
    assert_eq!(
        app.podxt().pod_bundle.as_ref().unwrap().as_bytes(),
        before_bundle
    );
    assert!(!app.podxt().pod_bundle.as_ref().unwrap().is_dirty());
    assert_eq!(app.podxt().pod_amp_model_pending, pending);

    app.podxt_mut().pod_selected = Some(1);
    app.podxt_mut()
        .pod_library
        .set_amp_model_choice(1, pod_preset::PodAmpModelChoice::Line6Clean)
        .unwrap();
    recording_frame_tall(&context, &mut app, vec![]);
    assert_eq!(app.podxt().pod_amp_model_pending, None);
    assert_eq!(app.podxt().pod_library.tone(0).unwrap().as_bytes(), first);
    let clean_model_id = app
        .podxt()
        .pod_library
        .tone(1)
        .unwrap()
        .amp_model_id()
        .unwrap();
    app.podxt_mut().pod_amp_model_pending =
        Some((1, clean_model_id, pod_preset::PodAmpModelChoice::Model2002e));
    app.podxt_mut().pod_library.revert_unsaved(1).unwrap();
    recording_frame_tall(&context, &mut app, vec![]);
    assert_eq!(app.podxt().pod_amp_model_pending, None);
    assert_eq!(
        app.podxt().pod_library.tone(1).unwrap().as_bytes(),
        baseline
    );
    assert!(!app.podxt().pod_library.is_dirty(1));
    assert_eq!(
        app.podxt().pod_bundle.as_ref().unwrap().as_bytes(),
        before_bundle
    );
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
    assert_eq!(app.podxt().pod_bundle.as_ref().unwrap().bank_count(), 2);
    assert_eq!(app.podxt().pod_library.len(), 2);
    assert_eq!(
        app.podxt().pod_bundle.as_ref().unwrap().bank_name(1),
        Some("B")
    );
    let second_row = app.podxt().pod_rows[&1].center();
    click_pod_at(&context, &mut app, second_row);
    assert_eq!(app.podxt().pod_selected, Some(1));
    assert_eq!(app.podxt().pod_library.tone(1).unwrap().as_bytes(), second);
    let first_row = app.podxt().pod_rows[&0].center();
    click_pod_at(&context, &mut app, first_row);
    drag_pod_drive_slider(&context, &mut app);
    let selected_value = app
        .podxt()
        .pod_library
        .tone(0)
        .unwrap()
        .amp_drive_fraction()
        .unwrap();
    assert!(app.podxt().pod_bundle.as_ref().unwrap().is_dirty());
    assert!(app.podxt().pod_library.is_dirty(0));
    assert!(!app.podxt().pod_library.is_dirty(1));

    click_pod(&context, &mut app, "save-pod");
    assert_eq!(std::fs::read(&blocked_path).unwrap(), b"preserve existing");
    assert!(app.podxt().pod_bundle.as_ref().unwrap().is_dirty());
    click_pod(&context, &mut app, "save-pod");
    let saved = std::fs::read(&saved_path).unwrap();
    let reopened = pod_preset::PodBundleLibrary::parse(saved.clone()).unwrap();
    assert_eq!(
        reopened.tone(0, 0).unwrap().amp_drive_fraction().unwrap(),
        selected_value
    );
    assert_eq!(reopened.tone(1, 0).unwrap().as_bytes(), second);
    assert_eq!(saved, app.podxt().pod_bundle.as_ref().unwrap().as_bytes());
    assert_eq!(std::fs::read(&original_path).unwrap(), original);
    assert!(!app.podxt().pod_bundle.as_ref().unwrap().is_dirty());
    assert!(!app.podxt().pod_library.any_dirty());

    app.podxt_mut()
        .edit_pod_tone(0, |tone| {
            tone.set_amp_control_fraction(pod_preset::AmpControl::Drive, 0.2)
        })
        .unwrap();
    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    click_prompt(&context, &mut app, "discard");
    assert!(!app.podxt().pod_bundle.as_ref().unwrap().is_dirty());
    assert!(!app.podxt().pod_library.any_dirty());
    assert_eq!(app.podxt().pod_bundle.as_ref().unwrap().as_bytes(), saved);
    assert!(app.variax().read_worker.is_none());
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
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
    app.select_test_editor::<PodxtEditor>();
    app.podxt_mut()
        .load_pod_bundle_bytes(original.clone())
        .unwrap();
    recording_frame(&context, &mut app, vec![]);
    assert!(!app.podxt().pod_buttons.contains_key("pod-duplicate-entry"));
    app.podxt_mut().pod_selected = Some(1);
    app.podxt_mut().refresh_selected_pod_name();
    assert_eq!(app.podxt().pod_selected, Some(1));
    recording_frame(&context, &mut app, vec![]);
    assert!(app.podxt().pod_buttons.contains_key("pod-duplicate-entry"));

    app.podxt_mut().pod_cab_pending = Some((
        1,
        pod_preset::PodCabModelChoice::from_model_id(0x0102_0016).unwrap(),
    ));
    click_pod(&context, &mut app, "pod-duplicate-entry");
    let bundle = app.podxt().pod_bundle.as_ref().unwrap();
    assert_eq!(bundle.entry_count(), 3);
    assert_eq!(app.podxt().pod_library.len(), 3);
    assert_eq!(app.podxt().pod_selected, Some(2));
    assert_eq!(
        app.podxt().pod_library.tone(2).unwrap().as_bytes(),
        folder_tone
    );
    assert!(bundle.is_dirty());
    assert_ne!(
        app.podxt().pod_cab_pending,
        Some((
            2,
            pod_preset::PodCabModelChoice::from_model_id(0x0102_0016).unwrap()
        ))
    );
    assert!(
        app.podxt()
            .pod_cab_pending
            .is_none_or(|(index, _)| index != 1)
    );
    assert_eq!(std::fs::read(&source_path).unwrap(), original);
    assert!(
        app.podxt_mut()
            .load_pod_bundle_bytes(original.clone())
            .is_err()
    );
    assert_eq!(app.podxt().pod_bundle.as_ref().unwrap().entry_count(), 3);

    click_pod(&context, &mut app, "save-pod");
    let saved = std::fs::read(&saved_path).unwrap();
    let reopened = pod_preset::PodBundleLibrary::parse(saved.clone()).unwrap();
    assert_eq!(reopened.entry_count(), 3);
    assert_eq!(reopened.tone(1, 1).unwrap().as_bytes(), folder_tone);
    assert_eq!(saved, app.podxt().pod_bundle.as_ref().unwrap().as_bytes());
    assert!(!app.podxt().pod_bundle.as_ref().unwrap().is_dirty());
    assert_eq!(std::fs::read(&source_path).unwrap(), original);

    click_pod(&context, &mut app, "pod-move-entry-up");
    assert_eq!(app.podxt().pod_selected, Some(1));
    assert_eq!(app.podxt().pod_bundle.as_ref().unwrap().as_bytes(), saved);
    assert!(!app.podxt().pod_bundle.as_ref().unwrap().is_dirty());

    click_pod(&context, &mut app, "pod-remove-entry");
    assert_eq!(app.podxt().pod_bundle.as_ref().unwrap().entry_count(), 2);
    assert_eq!(app.podxt().pod_selected, Some(1));
    assert!(app.podxt().pod_bundle.as_ref().unwrap().is_dirty());
    assert_ne!(
        app.podxt().pod_cab_pending,
        Some((
            1,
            pod_preset::PodCabModelChoice::from_model_id(0x0102_0016).unwrap()
        ))
    );
    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    click_prompt(&context, &mut app, "discard");
    assert!(!app.podxt().pod_bundle.as_ref().unwrap().is_dirty());
    assert_eq!(app.podxt().pod_bundle.as_ref().unwrap().as_bytes(), saved);
    assert_eq!(app.podxt().pod_selected, Some(1));
    assert_eq!(
        app.podxt().pod_name,
        pod_preset::PodTone::parse(folder_tone.clone())
            .unwrap()
            .name()
    );
    assert_eq!(std::fs::read(&source_path).unwrap(), original);
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
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

    app.select_test_editor::<PodxtEditor>();
    app.podxt_mut()
        .load_pod_bundle_bytes(original.clone())
        .unwrap();
    recording_frame(&context, &mut app, vec![]);
    assert!(app.podxt().pod_buttons.contains_key("pod-move-entry-up"));
    assert!(app.podxt().pod_buttons.contains_key("pod-move-entry-down"));
    assert!(!app.podxt().pod_buttons.contains_key("pod-duplicate-entry"));
    assert!(!app.podxt().pod_buttons.contains_key("pod-remove-entry"));

    app.podxt_mut().pod_selected = Some(1);
    app.podxt_mut().refresh_selected_pod_name();
    app.podxt_mut().pod_cab_pending = Some((
        1,
        pod_preset::PodCabModelChoice::from_model_id(0x0102_0016).unwrap(),
    ));
    let pending_before_rejected_action = app.podxt().pod_cab_pending;
    let original_bundle_bytes = app.podxt().pod_bundle.as_ref().unwrap().as_bytes().to_vec();
    let original_library_tones: Vec<_> = (0..app.podxt().pod_library.len())
        .map(|index| {
            app.podxt()
                .pod_library
                .tone(index)
                .unwrap()
                .as_bytes()
                .to_vec()
        })
        .collect();
    app.podxt_mut()
        .apply_pod_membership_action(PodMembershipAction::MoveDown, 99, 1);
    assert_eq!(
        app.podxt().pod_bundle.as_ref().unwrap().as_bytes(),
        original_bundle_bytes
    );
    assert_eq!(app.podxt().pod_selected, Some(1));
    assert_eq!(app.podxt().pod_cab_pending, pending_before_rejected_action);
    assert_eq!(
        (0..app.podxt().pod_library.len())
            .map(|index| {
                app.podxt()
                    .pod_library
                    .tone(index)
                    .unwrap()
                    .as_bytes()
                    .to_vec()
            })
            .collect::<Vec<_>>(),
        original_library_tones
    );

    click_pod(&context, &mut app, "pod-move-entry-up");
    let bundle = app.podxt().pod_bundle.as_ref().unwrap();
    assert_eq!(bundle.tone(0, 0).unwrap().as_bytes(), second);
    assert_eq!(bundle.tone(0, 1).unwrap().as_bytes(), first);
    assert_eq!(app.podxt().pod_library.tone(0).unwrap().as_bytes(), second);
    assert_eq!(app.podxt().pod_selected, Some(0));
    assert_eq!(
        app.podxt().pod_name,
        pod_preset::PodTone::parse(second.clone()).unwrap().name()
    );
    assert!(bundle.is_dirty());
    assert!(app.podxt().pod_cab_pending.is_none());
    assert!(app.podxt().pod_message.contains("fixed bank"));
    assert!(std::fs::read(&collision).unwrap() == b"retain this destination");

    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    click_prompt(&context, &mut app, "cancel");
    assert!(app.test_pending().is_none());
    assert!(app.podxt().pod_bundle.as_ref().unwrap().is_dirty());

    app.request_close(&context);
    app.dialogs.queue_pod_save(Some(collision.clone()));
    click_prompt(&context, &mut app, "save");
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    assert!(app.podxt().pod_bundle.as_ref().unwrap().is_dirty());
    assert_eq!(
        std::fs::read(&collision).unwrap(),
        b"retain this destination"
    );

    app.dialogs.queue_pod_save(Some(saved.clone()));
    click_prompt(&context, &mut app, "save");
    assert!(app.test_pending().is_none());
    assert!(!app.podxt().pod_bundle.as_ref().unwrap().is_dirty());
    assert_eq!(
        pod_preset::PodBundleLibrary::parse(std::fs::read(&saved).unwrap())
            .unwrap()
            .as_bytes(),
        app.podxt().pod_bundle.as_ref().unwrap().as_bytes()
    );

    click_pod(&context, &mut app, "pod-move-entry-down");
    assert!(app.podxt().pod_bundle.as_ref().unwrap().is_dirty());
    assert_eq!(app.podxt().pod_selected, Some(1));
    app.permit_close = false;
    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    click_prompt(&context, &mut app, "discard");
    assert!(app.test_pending().is_none());
    assert!(!app.podxt().pod_bundle.as_ref().unwrap().is_dirty());
    assert_eq!(
        app.podxt().pod_bundle.as_ref().unwrap().as_bytes(),
        std::fs::read(&saved).unwrap()
    );
    assert_eq!(app.podxt().pod_selected, Some(1));
    assert_eq!(app.podxt().pod_library.len(), 2);
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
    }
}

#[test]
fn reverting_pod_membership_before_recording_quit_cancel_keeps_selection_valid() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let fixed_tone = sample_drive_pod_tone();
    let folder_tone = sample_pod_tone();
    let original = sample_pod_bundle(&[fixed_tone, folder_tone.clone()]);
    app.select_test_editor::<PodxtEditor>();
    app.podxt_mut().load_pod_bundle_bytes(original).unwrap();
    app.podxt_mut().pod_selected = Some(1);
    app.podxt_mut().refresh_selected_pod_name();
    recording_frame(&context, &mut app, vec![]);

    click_pod(&context, &mut app, "pod-duplicate-entry");
    assert_eq!(app.podxt().pod_selected, Some(2));
    assert_eq!(app.podxt().pod_bundle.as_ref().unwrap().entry_count(), 3);

    prepare_quit_take(&mut app, ClockSelection::Integer(48_000));
    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    click_prompt(&context, &mut app, "discard");
    assert_eq!(app.podxt().pod_bundle.as_ref().unwrap().entry_count(), 2);
    assert!(!app.podxt().pod_bundle.as_ref().unwrap().is_dirty());
    assert_eq!(app.podxt().pod_selected, Some(1));
    assert_eq!(
        app.podxt().pod_name,
        pod_preset::PodTone::parse(folder_tone).unwrap().name()
    );
    assert!(matches!(app.test_pending(), Some(Pending::QuitRecording)));

    click_prompt(&context, &mut app, "cancel");
    assert!(app.test_pending().is_none());
    assert!(app.recorder.has_unsaved_take());
    assert_eq!(app.podxt().pod_selected, Some(1));
    recording_frame(&context, &mut app, vec![]);
    assert!(app.podxt().pod_rows.contains_key(&1));
    assert!(app.podxt().pod_buttons.contains_key("pod-duplicate-entry"));
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
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
    app.select_test_editor::<PodxtEditor>();
    app.load_pod_tone();
    recording_frame(&context, &mut app, vec![]);
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        original
    );
    assert!(!app.podxt().pod_library.is_dirty(0));
    let band = pod_preset::PodxtEqBand::One;
    drag_pod_eq_gain_slider(&context, &mut app, band);
    let (frequency, _) = app
        .podxt()
        .pod_library
        .tone(0)
        .unwrap()
        .podxt_eq_band_raw_values(band)
        .unwrap();
    assert_eq!(frequency.to_bits(), 0x3f80_0000);
    assert!(app.podxt().pod_library.is_dirty(0));
    app.podxt_mut().pod_library.revert_unsaved(0).unwrap();
    recording_frame(&context, &mut app, vec![]);
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        original
    );
    assert!(!app.podxt().pod_library.is_dirty(0));
    drag_pod_eq_frequency_slider(&context, &mut app, band);
    let (_, gain) = app
        .podxt()
        .pod_library
        .tone(0)
        .unwrap()
        .podxt_eq_band_raw_values(band)
        .unwrap();
    assert_eq!(gain.to_bits(), 0x3f80_0000);
    assert!(app.podxt().pod_library.is_dirty(0));
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
    }
}

#[test]
fn podxt_eq_reset_stages_applies_discards_and_resolves_quit_before_dirty_prompt() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let mut original = sample_pod_eq_tone();
    original[8..12].copy_from_slice(b"L6PA");
    let index = app
        .podxt_mut()
        .pod_library
        .import_bytes(original.clone())
        .unwrap();
    app.podxt_mut().pod_selected = Some(index);
    app.select_test_editor::<PodxtEditor>();

    recording_frame(&context, &mut app, vec![]);
    assert!(app.podxt().pod_buttons.contains_key("pod-eq-reset-stage"));
    click_pod_button_tall(&context, &mut app, "pod-eq-reset-stage");
    assert_eq!(
        app.podxt().pod_library.tone(index).unwrap().as_bytes(),
        original
    );
    assert!(!app.podxt().pod_library.is_dirty(index));
    assert!(app.podxt().pod_eq_reset_pending.is_some());

    recording_frame(&context, &mut app, vec![]);
    click_pod_button_tall(&context, &mut app, "pod-eq-reset-discard");
    assert_eq!(
        app.podxt().pod_library.tone(index).unwrap().as_bytes(),
        original
    );
    assert!(!app.podxt().pod_library.is_dirty(index));
    assert!(app.podxt().pod_eq_reset_pending.is_none());

    click_pod_button_tall(&context, &mut app, "pod-eq-reset-stage");
    recording_frame(&context, &mut app, vec![]);
    click_pod_button_tall(&context, &mut app, "pod-eq-reset-apply");
    let edited = app.podxt().pod_library.tone(index).unwrap().as_bytes();
    assert!(app.podxt().pod_library.is_dirty(index));
    assert!(app.podxt().pod_eq_reset_pending.is_none());
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
                .podxt()
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
    assert!(app.podxt().pod_message.contains("Nothing was sent"));

    app.podxt_mut().pod_library.revert_unsaved(index).unwrap();
    recording_frame(&context, &mut app, vec![]);
    click_pod_button_tall(&context, &mut app, "pod-eq-reset-stage");
    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    prompt_frame(&context, &mut app, vec![]);
    click_prompt(&context, &mut app, "eq-reset-quit-keep");
    assert!(app.test_pending().is_none());
    assert_eq!(
        app.podxt().pod_library.tone(index).unwrap().as_bytes(),
        original
    );
    assert!(!app.podxt().pod_library.is_dirty(index));

    recording_frame(&context, &mut app, vec![]);
    click_pod_button_tall(&context, &mut app, "pod-eq-reset-stage");
    app.request_close(&context);
    prompt_frame(&context, &mut app, vec![]);
    click_prompt(&context, &mut app, "eq-reset-quit-apply");
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    assert!(app.podxt().pod_library.is_dirty(index));
    assert!(app.podxt().pod_eq_reset_pending.is_none());
    click_prompt(&context, &mut app, "cancel");
    assert!(app.test_pending().is_none());
    assert!(app.podxt().pod_library.is_dirty(index));
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
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
    app.podxt_mut()
        .load_pod_bundle_bytes(bundle_bytes.clone())
        .unwrap();
    app.select_test_editor::<PodxtEditor>();

    recording_frame_tall(&context, &mut app, vec![]);
    click_pod_button_tall(&context, &mut app, "pod-eq-reset-stage");
    let pending = app.podxt().pod_eq_reset_pending.clone().unwrap();
    assert_eq!(
        pending.document_generation,
        app.podxt().pod_document_generation
    );
    assert_eq!(
        app.podxt().pod_bundle.as_ref().unwrap().as_bytes(),
        bundle_bytes
    );
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        original
    );
    assert!(!app.podxt().pod_library.is_dirty(0));

    // A cancelled dialog and a rejected file keep both the bundle and staged action.
    click_pod_button_tall(&context, &mut app, "load-pod");
    assert_eq!(app.podxt().pod_eq_reset_pending.as_ref(), Some(&pending));
    assert_eq!(
        app.podxt().pod_bundle.as_ref().unwrap().as_bytes(),
        bundle_bytes
    );
    click_pod_button_tall(&context, &mut app, "load-pod");
    assert_eq!(app.podxt().pod_eq_reset_pending.as_ref(), Some(&pending));
    assert_eq!(
        app.podxt().pod_bundle.as_ref().unwrap().as_bytes(),
        bundle_bytes
    );
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        original
    );

    // Replacing with a different Library document at the same index and EQ context
    // must invalidate the old stage. A later Apply/quit cannot touch this tone.
    click_pod_button_tall(&context, &mut app, "load-pod");
    assert!(app.podxt().pod_bundle.is_none());
    assert!(app.podxt().pod_eq_reset_pending.is_none());
    assert!(app.podxt().pod_document_generation > pending.document_generation);
    assert_eq!(app.podxt().pod_selected, Some(0));
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        replacement
    );
    assert!(!app.podxt().pod_library.is_dirty(0));

    app.podxt_mut().pod_buttons.clear();
    recording_frame_tall(&context, &mut app, vec![]);
    assert!(app.podxt().pod_buttons.contains_key("pod-eq-reset-stage"));
    assert!(!app.podxt().pod_buttons.contains_key("pod-eq-reset-apply"));
    app.request_close(&context);
    app.pending_buttons.clear();
    prompt_frame(&context, &mut app, vec![]);
    assert!(!app.pending_buttons.contains_key("eq-reset-quit-apply"));
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        replacement
    );
    assert!(!app.podxt().pod_library.is_dirty(0));
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
    }
}

#[test]
fn podxt_eq_and_amp_stages_resolve_against_a_loaded_bundle() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    app.select_test_editor::<PodxtEditor>();
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
    assert!(
        app.podxt().pod_bundle.is_some(),
        "{}",
        app.podxt().pod_message
    );
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        original
    );

    click_pod_button_tall(&context, &mut app, "pod-eq-reset-stage");
    choose_amp_model_popup(&context, &mut app, 0x0002_002e);
    let amp_stage = app.podxt().pod_amp_model_pending;
    assert!(amp_stage.is_some());
    assert!(app.podxt().pod_eq_reset_pending.is_some());
    assert_eq!(
        app.podxt().pod_bundle.as_ref().unwrap().as_bytes(),
        bundle_bytes
    );
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        original
    );
    assert!(!app.podxt().pod_dirty());

    // Keep then discard the EQ stage through the actual quit dialog. Discarding
    // EQ continues into the independently staged AMP choice.
    app.request_close(&context);
    click_prompt(&context, &mut app, "eq-reset-quit-keep");
    assert!(app.test_pending().is_none());
    assert!(app.podxt().pod_eq_reset_pending.is_some());
    assert_eq!(app.podxt().pod_amp_model_pending, amp_stage);
    app.request_close(&context);
    click_prompt(&context, &mut app, "eq-reset-quit-discard");
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    assert!(app.podxt().pod_eq_reset_pending.is_none());
    assert_eq!(app.podxt().pod_amp_model_pending, amp_stage);
    click_prompt(&context, &mut app, "amp-keep-editing");
    assert!(app.test_pending().is_none());
    assert_eq!(app.podxt().pod_amp_model_pending, amp_stage);
    assert_eq!(
        app.podxt().pod_bundle.as_ref().unwrap().as_bytes(),
        bundle_bytes
    );
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        original
    );
    assert!(!app.podxt().pod_dirty());

    // Apply EQ, then exercise the bundle cancel/discard path while the AMP
    // choice remains independently staged.
    click_pod_button_tall(&context, &mut app, "pod-eq-reset-stage");
    app.request_close(&context);
    click_prompt(&context, &mut app, "eq-reset-quit-apply");
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    assert!(app.podxt().pod_eq_reset_pending.is_none());
    assert_eq!(app.podxt().pod_amp_model_pending, amp_stage);
    assert_ne!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        original
    );
    assert!(app.podxt().pod_bundle.as_ref().unwrap().is_dirty());
    click_prompt(&context, &mut app, "cancel");
    assert!(app.test_pending().is_none());
    assert!(app.podxt().pod_amp_model_pending.is_some());
    assert!(app.podxt().pod_bundle.as_ref().unwrap().is_dirty());

    app.request_close(&context);
    click_prompt(&context, &mut app, "discard");
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        original
    );
    assert_eq!(
        app.podxt().pod_bundle.as_ref().unwrap().as_bytes(),
        bundle_bytes
    );
    assert_eq!(app.podxt().pod_amp_model_pending, amp_stage);
    click_prompt(&context, &mut app, "amp-keep-editing");
    assert!(app.test_pending().is_none());
    assert_eq!(app.podxt().pod_amp_model_pending, amp_stage);
    app.request_close(&context);
    click_prompt(&context, &mut app, "amp-discard");
    assert!(app.permit_close);
    assert_eq!(app.podxt().pod_amp_model_pending, None);

    // Exercise the Apply rejection path after a real bundle load. Removing
    // the selected bundle row behind the Library forces atomic publication
    // failure while the staged edit still has a valid source context.
    let (context, mut app) = super::super::tests::app();
    context.style_mut(|style| style.animation_time = 0.0);
    app.select_test_editor::<PodxtEditor>();
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
    app.podxt_mut().pod_selected = Some(1);
    assert_eq!(
        app.podxt().pod_library.tone(1).unwrap().as_bytes(),
        original
    );
    click_pod_button_tall(&context, &mut app, "pod-eq-reset-stage");
    choose_amp_model_popup(&context, &mut app, 0x0002_002e);
    let amp_stage = app.podxt().pod_amp_model_pending;
    app.podxt_mut()
        .pod_bundle
        .as_mut()
        .unwrap()
        .remove_entry(1, 0)
        .unwrap();
    let failed_publication_bundle = app.podxt().pod_bundle.as_ref().unwrap().as_bytes().to_vec();
    let parsed_before = app.podxt().pod_library.tone(1).unwrap().models().to_vec();
    app.request_close(&context);
    click_prompt(&context, &mut app, "eq-reset-quit-apply");
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    assert!(app.podxt().pod_eq_reset_pending.is_some());
    assert_eq!(app.podxt().pod_amp_model_pending, amp_stage);
    assert_eq!(
        app.podxt().pod_library.tone(1).unwrap().as_bytes(),
        original
    );
    assert_eq!(
        app.podxt().pod_library.tone(1).unwrap().models(),
        parsed_before
    );
    assert!(!app.podxt().pod_library.is_dirty(1));
    assert_eq!(
        app.podxt().pod_bundle.as_ref().unwrap().as_bytes(),
        failed_publication_bundle
    );
    assert!(app.podxt().pod_bundle.as_ref().unwrap().is_dirty());
    assert!(!app.permit_close);
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
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
    app.select_test_editor::<PodxtEditor>();
    app.load_pod_tone();
    recording_frame(&context, &mut app, vec![]);
    assert_eq!(app.podxt().pod_eq_frequency_sliders.len(), 4);
    assert_eq!(app.podxt().pod_eq_gain_sliders.len(), 4);
    let before = app.podxt().pod_library.tone(0).unwrap().as_bytes().to_vec();
    drag_pod_eq_frequency_slider(&context, &mut app, pod_preset::PodxtEqBand::One);
    assert!(app.podxt().pod_library.is_dirty(0));
    let after = app.podxt().pod_library.tone(0).unwrap().as_bytes();
    let changed: Vec<_> = before
        .iter()
        .zip(after)
        .enumerate()
        .filter_map(|(i, (a, b))| (a != b).then_some(i))
        .collect();
    assert!(!changed.is_empty());
    assert!(app.podxt().pod_message.contains("Nothing was sent"));

    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    prompt_frame(&context, &mut app, vec![]);
    click_prompt(&context, &mut app, "cancel");
    assert!(app.test_pending().is_none());
    assert!(
        app.podxt().pod_library.is_dirty(0),
        "keep editing retains the applied EQ edit"
    );

    click_pod(&context, &mut app, "save-pod");
    assert!(
        app.podxt().pod_library.is_dirty(0),
        "cancel keeps the applied EQ edit"
    );
    click_pod(&context, &mut app, "save-pod");
    assert_eq!(std::fs::read(&collision).unwrap(), b"keep this file");
    assert!(
        app.podxt().pod_library.is_dirty(0),
        "collision keeps the applied EQ edit"
    );
    click_pod(&context, &mut app, "save-pod");
    assert!(!app.podxt().pod_library.is_dirty(0));
    let reopened = pod_preset::PodTone::parse(std::fs::read(saved).unwrap()).unwrap();
    assert_ne!(reopened.as_bytes(), before);
}

fn click_pod_window_close(context: &egui::Context, app: &mut Workbench) {
    click_pod(context, app, "show-variax");
}

fn pod_tone_name(app: &Workbench, index: usize) -> String {
    app.podxt()
        .pod_library
        .tone(index)
        .expect("tone")
        .name()
        .to_owned()
}

#[test]
fn applied_pod_rename_blocks_quit_until_each_tone_is_saved_or_discarded() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let guitar = app.variax().document.as_ref().unwrap().as_bytes().to_vec();
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
    app.podxt_mut().pod_name = "Typed only".into();
    app.request_close(&context);
    assert!(app.test_pending().is_none());
    assert!(app.permit_close);
    assert!(!app.podxt().pod_library.any_dirty());
    // The headless fixture keeps running after the clean close command.
    app.permit_close = false;
    app.podxt_mut().pod_name = "Alpha".into();
    click_pod(&context, &mut app, "apply-pod-name");
    click_pod(&context, &mut app, "load-pod");
    app.podxt_mut().pod_name = "Beta".into();
    click_pod(&context, &mut app, "apply-pod-name");
    assert!(app.podxt().pod_library.is_dirty(0));
    assert!(app.podxt().pod_library.is_dirty(1));
    assert_eq!(pod_tone_name(&app, 0), "Alpha");
    assert_eq!(pod_tone_name(&app, 1), "Beta");
    assert!(!app.variax().dirty());
    click_pod_window_close(&context, &mut app);
    assert!(!app.test_editor_is_active::<PodxtEditor>());
    assert_eq!(pod_tone_name(&app, 0), "Alpha");
    assert_eq!(pod_tone_name(&app, 1), "Beta");
    assert!(app.podxt().pod_library.is_dirty(0));
    assert!(app.podxt().pod_library.is_dirty(1));

    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    assert!(!app.permit_close);
    prompt_frame(&context, &mut app, vec![]);
    click_prompt(&context, &mut app, "cancel");
    assert!(app.test_pending().is_none());
    assert!(!app.permit_close);
    assert_eq!(pod_tone_name(&app, 0), "Alpha");
    assert_eq!(pod_tone_name(&app, 1), "Beta");
    assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), guitar);

    app.request_close(&context);
    click_prompt(&context, &mut app, "save");
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    assert!(!app.permit_close);
    assert!(app.podxt().pod_message.contains("cancelled"));
    assert!(app.podxt().pod_library.is_dirty(0));
    assert!(app.podxt().pod_library.is_dirty(1));
    assert_eq!(pod_tone_name(&app, 0), "Alpha");
    click_prompt(&context, &mut app, "save");
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    assert!(!app.permit_close);
    assert!(!app.podxt().pod_message.starts_with("Saved a new copy"));
    assert_eq!(std::fs::read(&blocked).unwrap(), b"keep");
    assert!(app.podxt().pod_library.is_dirty(0));
    assert!(app.podxt().pod_library.is_dirty(1));
    assert_eq!(pod_tone_name(&app, 1), "Beta");
    click_prompt(&context, &mut app, "cancel");
    assert!(app.test_pending().is_none());

    app.variax_mut()
        .change(|preset| preset.rename("Guitar edit"));
    assert!(app.variax().dirty());
    let guitar_edited = app.variax().document.as_ref().unwrap().as_bytes().to_vec();
    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::Quit)));
    assert!(!app.permit_close);
    prompt_frame(&context, &mut app, vec![]);
    click_prompt(&context, &mut app, "save");
    assert!(!app.variax().dirty());
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    assert!(!app.permit_close);
    assert_eq!(std::fs::read(&guitar_copy).unwrap(), guitar_edited);
    assert_eq!(pod_tone_name(&app, 0), "Alpha");
    assert_eq!(pod_tone_name(&app, 1), "Beta");
    assert!(app.podxt().pod_library.is_dirty(0));
    assert!(app.podxt().pod_library.is_dirty(1));

    click_prompt(&context, &mut app, "save");
    assert!(!app.podxt().pod_library.is_dirty(0));
    assert!(app.podxt().pod_library.is_dirty(1));
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    assert!(!app.permit_close);
    assert_eq!(pod_tone_name(&app, 0), "Alpha");
    assert_eq!(pod_tone_name(&app, 1), "Beta");
    let saved_alpha = pod_preset::PodTone::parse(std::fs::read(&alpha).unwrap()).unwrap();
    assert_eq!(saved_alpha.name(), "Alpha");
    assert_eq!(saved_alpha.amp_model_id(), Some(0x1111_0002));
    assert!(!beta.exists());

    click_prompt(&context, &mut app, "save");
    assert!(!app.podxt().pod_library.any_dirty());
    assert!(app.permit_close);
    assert!(app.test_pending().is_none());
    assert_eq!(pod_tone_name(&app, 1), "Beta");
    let saved_beta = pod_preset::PodTone::parse(std::fs::read(&beta).unwrap()).unwrap();
    assert_eq!(saved_beta.name(), "Beta");
    assert_eq!(
        app.variax().document.as_ref().unwrap().as_bytes(),
        guitar_edited
    );

    app.permit_close = false;
    app.podxt_mut().pod_selected = Some(0);
    app.podxt_mut().pod_name = "Gamma".into();
    app.podxt_mut().apply_pod_name();
    assert!(app.podxt().pod_library.is_dirty(0));
    assert!(!app.podxt().pod_library.is_dirty(1));
    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    click_prompt(&context, &mut app, "discard");
    assert_eq!(pod_tone_name(&app, 0), "Alpha");
    assert_eq!(pod_tone_name(&app, 1), "Beta");
    assert!(!app.podxt().pod_library.any_dirty());
    assert!(app.permit_close);
    assert!(app.test_pending().is_none());
    assert!(app.podxt().pod_message.contains("Discarded"));
    assert_eq!(
        app.variax().document.as_ref().unwrap().as_bytes(),
        guitar_edited
    );
    assert!(app.variax().read_worker.is_none());
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
    }
}

#[test]
fn amp_stomp_mod_delay_and_reverb_sliders_edit_selected_tone_and_quit_guard_keeps_or_saves_it() {
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
    assert_eq!(
        app.podxt().pod_library.len(),
        2,
        "{}",
        app.podxt().pod_message
    );
    assert_eq!(app.podxt().pod_selected, Some(1));
    recording_frame(&context, &mut app, vec![]);
    assert!(app.podxt().pod_drive_slider.is_none());
    assert!(app.podxt().pod_stomp_sliders.is_empty());
    assert!(
        app.podxt()
            .pod_library
            .tone(1)
            .unwrap()
            .amp_drive_fraction()
            .is_err()
    );
    assert!(!app.podxt().pod_library.is_dirty(0));
    assert!(!app.podxt().pod_library.is_dirty(1));

    let first_row = app.podxt().pod_rows[&0].center();
    click_pod_at(&context, &mut app, first_row);
    assert_eq!(app.podxt().pod_selected, Some(0));
    recording_frame(&context, &mut app, vec![]);
    assert!(app.podxt().pod_drive_slider.is_some());
    let stomp_controls = [
        pod_preset::StompControl::Drive,
        pod_preset::StompControl::Gain,
        pod_preset::StompControl::Tone,
    ];
    assert_eq!(app.podxt().pod_stomp_sliders.len(), stomp_controls.len());
    assert_eq!(app.podxt().pod_mod_sliders.len(), 3);
    assert_eq!(app.podxt().pod_reverb_sliders.len(), 2);
    assert_eq!(app.podxt().pod_delay_sliders.len(), 3);
    let original_drive = app
        .podxt()
        .pod_library
        .tone(0)
        .unwrap()
        .amp_drive_fraction()
        .unwrap();
    let original_stomp: Vec<_> = stomp_controls
        .into_iter()
        .map(|control| {
            app.podxt()
                .pod_library
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
        .podxt()
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
            app.podxt()
                .pod_library
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
        .podxt()
        .pod_library
        .tone(0)
        .unwrap()
        .mod_control_fraction(pod_preset::ModControl::Depth)
        .unwrap();
    assert!((0.0..=1.0).contains(&edited_mod));
    assert_ne!(edited_mod.to_bits(), 0.2f32.to_bits());
    let edited_reverb = app
        .podxt()
        .pod_library
        .tone(0)
        .unwrap()
        .reverb_control_fraction(pod_preset::ReverbControl::Dwell)
        .unwrap();
    assert!((0.0..=1.0).contains(&edited_reverb));
    assert_ne!(edited_reverb.to_bits(), 0.3f32.to_bits());
    let edited_delay = app
        .podxt()
        .pod_library
        .tone(0)
        .unwrap()
        .delay_control_fraction(pod_preset::DelayControl::Feedback)
        .unwrap();
    assert!((0.0..=1.0).contains(&edited_delay));
    assert_ne!(edited_delay.to_bits(), 0.2f32.to_bits());
    assert!(app.podxt().pod_library.is_dirty(0));
    assert!(!app.podxt().pod_library.is_dirty(1));
    assert_eq!(
        app.podxt().pod_library.tone(1).unwrap().as_bytes(),
        unavailable_bytes
    );

    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    click_prompt(&context, &mut app, "cancel");
    assert!(app.test_pending().is_none());
    assert!(app.podxt().pod_library.is_dirty(0));
    assert!(!app.podxt().pod_library.is_dirty(1));
    assert_eq!(
        app.podxt()
            .pod_library
            .tone(0)
            .unwrap()
            .amp_drive_fraction()
            .unwrap()
            .to_bits(),
        edited_drive.to_bits()
    );
    for (control, edited) in stomp_controls.into_iter().zip(&edited_stomp) {
        assert_eq!(
            app.podxt()
                .pod_library
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
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    assert!(!app.permit_close);
    assert!(app.podxt().pod_library.is_dirty(0));
    assert!(!app.podxt().pod_library.is_dirty(1));
    assert_eq!(
        std::fs::read(&blocked_path).unwrap(),
        b"keep existing destination"
    );

    click_prompt(&context, &mut app, "save");
    assert!(app.test_pending().is_none());
    assert!(app.permit_close);
    assert!(!app.podxt().pod_library.any_dirty());
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
        app.podxt().pod_library.tone(1).unwrap().as_bytes(),
        unavailable_bytes
    );
    assert!(
        !app.variax()
            .document
            .as_ref()
            .unwrap()
            .as_bytes()
            .is_empty()
    );
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
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
    app.select_test_editor::<PodxtEditor>();
    app.podxt_mut().pod_selected = Some(
        app.podxt_mut()
            .pod_library
            .import_bytes(sample_stomp_harmony_pod_tone())
            .unwrap(),
    );
    app.podxt_mut()
        .pod_library
        .import_bytes(sample_stomp_harmony_pod_tone())
        .unwrap();
    recording_frame_tall(&context, &mut app, vec![]);
    let original = app.podxt().pod_library.tone(0).unwrap().as_bytes().to_vec();
    stage_stomp_harmony1_choice(
        &context,
        &mut app,
        pod_preset::StompHarmony1M335Selection::MinusOctave,
    );
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        original
    );
    assert_eq!(
        app.podxt().pod_harmony_pending,
        Some((
            0,
            pod_preset::StompHarmonyControl::Harmony1M335,
            PendingStompHarmony::Harmony1M335(pod_preset::StompHarmony1M335Selection::MinusOctave)
        ))
    );
    let second_row = app.podxt().pod_rows[&1].center();
    click_pod_at_tall(&context, &mut app, second_row);
    recording_frame_tall(&context, &mut app, vec![]);
    assert_eq!(app.podxt().pod_selected, Some(1));
    assert!(app.podxt().pod_harmony_pending.is_none());
    let first_row = app.podxt().pod_rows[&0].center();
    click_pod_at_tall(&context, &mut app, first_row);
    recording_frame_tall(&context, &mut app, vec![]);
    stage_stomp_harmony1_choice(
        &context,
        &mut app,
        pod_preset::StompHarmony1M335Selection::MinusOctave,
    );
    click_pod(&context, &mut app, "pod-harmony1-apply");
    let after_first = app.podxt().pod_library.tone(0).unwrap().as_bytes().to_vec();
    assert_ne!(after_first, original);
    assert!(app.podxt().pod_library.is_dirty(0));
    assert!(app.podxt().pod_harmony_pending.is_none());
    assert_eq!(
        app.podxt()
            .pod_library
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
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        after_first
    );
    click_pod(&context, &mut app, "pod-harmony2-apply");
    let final_bytes = app.podxt().pod_library.tone(0).unwrap().as_bytes();
    assert_ne!(final_bytes, after_first);
    assert_eq!(
        app.podxt()
            .pod_library
            .tone(0)
            .unwrap()
            .stomp_harmony_1457_selection()
            .unwrap(),
        pod_preset::StompHarmonyValue::Known(pod_preset::StompHarmony1457Selection::Fourth)
    );
    assert_eq!(
        app.podxt()
            .pod_library
            .tone(0)
            .unwrap()
            .stomp_harmony_1m335_selection()
            .unwrap(),
        pod_preset::StompHarmonyValue::Known(pod_preset::StompHarmony1M335Selection::MinusOctave)
    );
    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    click_prompt(&context, &mut app, "save");
    assert_eq!(
        std::fs::read(&blocked).unwrap(),
        b"keep existing destination"
    );
    assert!(app.podxt().pod_library.is_dirty(0));
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
        pod_preset::StompHarmonyValue::Known(pod_preset::StompHarmony1M335Selection::MinusOctave)
    );
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
    }
}

#[test]
fn mod_note_gui_stages_per_tone_and_applies_only_on_request() {
    let (context, mut app) = app();
    app.select_test_editor::<PodxtEditor>();
    app.podxt_mut().pod_selected = Some(
        app.podxt_mut()
            .pod_library
            .import_bytes(sample_delay_note_tone(0, 0))
            .unwrap(),
    );
    app.podxt_mut()
        .pod_library
        .import_bytes(sample_delay_note_tone(0, 0))
        .unwrap();
    recording_frame_tall(&context, &mut app, vec![]);
    let original = app.podxt().pod_library.tone(0).unwrap().as_bytes().to_vec();
    stage_mod_note_choice(&context, &mut app, pod_preset::ModNoteChoice::Quarter);
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        original
    );
    assert_eq!(
        app.podxt().pod_mod_pending,
        Some((0, PendingModEdit::Note(pod_preset::ModNoteChoice::Quarter)))
    );

    let second_row = app.podxt().pod_rows[&1].center();
    click_pod_at_tall(&context, &mut app, second_row);
    recording_frame_tall(&context, &mut app, vec![]);
    assert_eq!(app.podxt().pod_selected, Some(1));
    assert!(app.podxt().pod_mod_pending.is_none());
    let second_original = app.podxt().pod_library.tone(1).unwrap().as_bytes().to_vec();
    stage_mod_note_choice(&context, &mut app, pod_preset::ModNoteChoice::Quarter);
    click_pod(&context, &mut app, "pod-mod-discard");
    assert!(app.podxt().pod_mod_pending.is_none());
    assert_eq!(
        app.podxt().pod_library.tone(1).unwrap().as_bytes(),
        second_original
    );

    let first_row = app.podxt().pod_rows[&0].center();
    click_pod_at_tall(&context, &mut app, first_row);
    recording_frame_tall(&context, &mut app, vec![]);
    stage_mod_note_choice(&context, &mut app, pod_preset::ModNoteChoice::Quarter);
    click_pod(&context, &mut app, "pod-mod-apply");
    assert_eq!(
        app.podxt()
            .pod_library
            .tone(0)
            .unwrap()
            .mod_note_value()
            .unwrap(),
        pod_preset::ModNoteValue::Known(pod_preset::ModNoteChoice::Quarter)
    );
    assert_eq!(
        app.podxt()
            .pod_library
            .tone(0)
            .unwrap()
            .mod_speed_hz()
            .unwrap()
            .to_bits(),
        2.0_f32.to_bits()
    );
    assert!(app.podxt().pod_library.is_dirty(0));
    assert!(app.podxt().pod_mod_pending.is_none());
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
    }
}

#[test]
fn mod_speed_gui_stages_discards_applies_and_retains_failed_edits() {
    let (context, mut app) = app();
    app.select_test_editor::<PodxtEditor>();
    app.podxt_mut().pod_selected = Some(
        app.podxt_mut()
            .pod_library
            .import_bytes(sample_delay_note_tone(0, 6))
            .unwrap(),
    );
    app.podxt_mut()
        .pod_library
        .import_bytes(sample_delay_note_tone(0, 99))
        .unwrap();
    recording_frame_tall(&context, &mut app, vec![]);
    let original = app.podxt().pod_library.tone(0).unwrap().as_bytes().to_vec();
    drag_mod_speed_slider(&context, &mut app, 0.2);
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        original
    );
    assert!(matches!(
        app.podxt().pod_mod_pending,
        Some((0, PendingModEdit::SpeedHz(_)))
    ));
    click_pod(&context, &mut app, "pod-mod-discard");
    assert!(app.podxt().pod_mod_pending.is_none());
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        original
    );

    drag_mod_speed_slider(&context, &mut app, 0.2);
    click_pod(&context, &mut app, "pod-mod-apply");
    assert_eq!(
        app.podxt()
            .pod_library
            .tone(0)
            .unwrap()
            .mod_note_value()
            .unwrap(),
        pod_preset::ModNoteValue::Known(pod_preset::ModNoteChoice::Off)
    );
    assert!(app.podxt().pod_library.is_dirty(0));
    assert!(app.podxt().pod_library.tone(0).unwrap().as_bytes() != original);
    assert!(app.podxt().pod_mod_pending.is_none());

    let second_row = app.podxt().pod_rows[&1].center();
    click_pod_at_tall(&context, &mut app, second_row);
    recording_frame_tall(&context, &mut app, vec![]);
    let invalid_original = app.podxt().pod_library.tone(1).unwrap().as_bytes().to_vec();
    drag_mod_speed_slider(&context, &mut app, 0.2);
    assert_eq!(
        app.podxt().pod_library.tone(1).unwrap().as_bytes(),
        invalid_original
    );
    click_pod(&context, &mut app, "pod-mod-apply");
    assert_eq!(
        app.podxt().pod_library.tone(1).unwrap().as_bytes(),
        invalid_original
    );
    assert!(!app.podxt().pod_library.is_dirty(1));
    assert!(matches!(
        app.podxt().pod_mod_pending,
        Some((1, PendingModEdit::SpeedHz(_)))
    ));

    let first_row = app.podxt().pod_rows[&0].center();
    click_pod_at_tall(&context, &mut app, first_row);
    recording_frame_tall(&context, &mut app, vec![]);
    drag_mod_speed_slider(&context, &mut app, 0.3);
    assert!(app.podxt().pod_mod_pending.is_some());
    app.podxt_mut().pod_library.revert_unsaved(0).unwrap();
    app.podxt_mut().refresh_selected_pod_name();
    assert!(app.podxt().pod_mod_pending.is_none());
    assert!(!app.podxt().pod_library.is_dirty(0));
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
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
    app.select_test_editor::<PodxtEditor>();
    app.podxt_mut()
        .load_pod_bundle_bytes(original_bundle.clone())
        .unwrap();
    recording_frame_tall(&context, &mut app, vec![]);
    drag_mod_speed_slider(&context, &mut app, 0.2);
    assert_eq!(
        app.podxt().pod_bundle.as_ref().unwrap().as_bytes(),
        original_bundle.as_slice()
    );
    click_pod(&context, &mut app, "pod-mod-apply");
    assert!(app.podxt().pod_dirty());
    assert_eq!(
        app.podxt()
            .pod_library
            .tone(0)
            .unwrap()
            .mod_note_value()
            .unwrap(),
        pod_preset::ModNoteValue::Known(pod_preset::ModNoteChoice::Off)
    );

    click_pod(&context, &mut app, "save-pod");
    assert!(app.podxt().pod_dirty());
    assert_eq!(
        std::fs::read(&collision).unwrap(),
        b"preserve existing MOD destination"
    );
    click_pod(&context, &mut app, "save-pod");
    assert!(!app.podxt().pod_dirty());
    let saved_bytes = std::fs::read(&saved).unwrap();
    assert_ne!(saved_bytes, original_bundle);

    click_pod(&context, &mut app, "load-pod");
    recording_frame_tall(&context, &mut app, vec![]);
    assert_eq!(
        app.podxt()
            .pod_library
            .tone(0)
            .unwrap()
            .mod_note_value()
            .unwrap(),
        pod_preset::ModNoteValue::Known(pod_preset::ModNoteChoice::Off)
    );
    assert!(!app.podxt().pod_dirty());
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
    }
}

#[test]
fn delay_model_gui_stages_per_tone_applies_only_id_and_can_discard() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    app.select_test_editor::<PodxtEditor>();
    let mut original = sample_delay_note_tone(0, 0);
    original[8..12].copy_from_slice(b"L6PA");
    let index = app
        .podxt_mut()
        .pod_library
        .import_bytes(original.clone())
        .unwrap();
    app.podxt_mut()
        .pod_library
        .import_bytes(original.clone())
        .unwrap();
    app.podxt_mut().pod_selected = Some(index);
    recording_frame_tall(&context, &mut app, vec![]);
    stage_delay_note_choice(&context, &mut app, pod_preset::DelayNoteChoice::Quarter);
    assert!(app.podxt().pod_delay_pending.is_some());
    choose_delay_model_popup(&context, &mut app, 0x0202_0003);
    assert_eq!(
        app.podxt().pod_library.tone(index).unwrap().as_bytes(),
        original
    );
    assert_eq!(
        app.podxt().pod_delay_model_pending.unwrap().1.model_id(),
        0x0202_0003
    );
    click_pod_button_tall(&context, &mut app, "pod-delay-model-apply");
    let applied = app.podxt().pod_library.tone(index).unwrap();
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
    assert_eq!(app.podxt().pod_delay_model_pending, None);
    assert_eq!(app.podxt().pod_delay_pending, None);

    choose_delay_model_popup(&context, &mut app, 0x0202_0004);
    click_pod_button_tall(&context, &mut app, "pod-delay-model-discard");
    assert_eq!(app.podxt().pod_delay_model_pending, None);
    assert_eq!(
        app.podxt()
            .pod_library
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
    assert!(app.podxt().pod_delay_model_pending.is_some());
    let second_row = app.podxt().pod_rows[&1];
    click_pod_at_tall(&context, &mut app, second_row.center());
    recording_frame_tall(&context, &mut app, vec![]);
    assert_eq!(app.podxt().pod_selected, Some(1));
    assert_eq!(app.podxt().pod_delay_model_pending, None);

    // A failed model Apply preserves prior parameter staging and source bytes.
    let mut invalid = sample_delay_note_tone(0, 0);
    let model_id = 0x0202_0002u32.to_be_bytes();
    let offset = invalid
        .windows(4)
        .position(|word| word == model_id)
        .unwrap();
    invalid[offset..offset + 4].copy_from_slice(&0x3333_0001u32.to_be_bytes());
    invalid[8..12].copy_from_slice(b"L6PA");
    app.podxt_mut()
        .pod_library
        .replace(1, invalid.clone())
        .unwrap();
    app.podxt_mut().pod_delay_pending = Some((1, PendingDelayEdit::TimeMs(350.0)));
    app.podxt_mut().pod_delay_model_pending = Some((
        1,
        pod_preset::PodDelayModelChoice::try_from(0x0202_0003).unwrap(),
    ));
    let failed_choice = app.podxt().pod_delay_model_pending.unwrap().1;
    assert!(
        app.podxt_mut()
            .apply_pod_delay_model_choice(1, failed_choice)
            .is_err()
    );
    assert_eq!(
        app.podxt().pod_delay_pending,
        Some((1, PendingDelayEdit::TimeMs(350.0)))
    );
    assert_eq!(app.podxt().pod_library.tone(1).unwrap().as_bytes(), invalid);
    assert_eq!(
        app.podxt().pod_delay_model_pending,
        Some((1, failed_choice))
    );
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
    }
}

#[test]
fn mod_model_gui_directly_replaces_each_gated_current_id_and_invalidates_note_stage() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    app.select_test_editor::<PodxtEditor>();
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
        let index = app
            .podxt_mut()
            .pod_library
            .import_bytes(original.clone())
            .unwrap();
        app.podxt_mut().pod_selected = Some(index);
        recording_frame_tall(&context, &mut app, vec![]);
        stage_delay_note_choice(&context, &mut app, pod_preset::DelayNoteChoice::Quarter);
        stage_mod_note_choice(&context, &mut app, pod_preset::ModNoteChoice::Quarter);
        assert!(
            app.podxt().pod_mod_pending.is_some(),
            "Mod NOTE must be staged before model Apply"
        );
        assert!(
            app.podxt().pod_delay_pending.is_some(),
            "unrelated Delay NOTE must be staged"
        );
        choose_mod_model_popup(&context, &mut app, 0x0203_0004);
        assert_eq!(
            app.podxt().pod_mod_model_pending.unwrap().1.model_id(),
            0x0203_0004
        );
        click_pod_button_tall(&context, &mut app, "pod-mod-model-apply");
        let applied = app.podxt().pod_library.tone(index).unwrap();
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
        assert_eq!(app.podxt().pod_mod_pending, None);
        assert!(
            app.podxt().pod_delay_pending.is_some(),
            "unrelated Delay NOTE staging must survive"
        );
        assert_eq!(app.podxt().pod_mod_model_pending, None);
        app.podxt_mut().pod_delay_pending = None;
    }
}

#[test]
fn reverb_model_gui_apply_and_discard_preserve_other_stages_and_bundle_baseline() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    app.select_test_editor::<PodxtEditor>();
    let original = sample_reverb_model_tone(0x0204_0002, pod_preset::REVERB_FALLBACK_SLOT);
    let index = app
        .podxt_mut()
        .pod_library
        .import_bytes(original.clone())
        .unwrap();
    app.podxt_mut().pod_selected = Some(index);
    recording_frame_tall(&context, &mut app, vec![]);
    choose_reverb_model_popup(&context, &mut app, 0x0204_0003);
    assert_eq!(
        app.podxt().pod_reverb_model_pending.unwrap().1.model_id(),
        0x0204_0003
    );
    assert_eq!(
        app.podxt().pod_library.tone(index).unwrap().as_bytes(),
        original
    );
    app.podxt_mut().pod_delay_pending = Some((index, PendingDelayEdit::TimeMs(350.0)));
    app.podxt_mut().pod_mod_pending = Some((index, PendingModEdit::SpeedHz(2.0)));
    app.podxt_mut().pod_stomp_model_pending = Some((
        index,
        pod_preset::PodStompModelChoice::try_from(0x0205_0000).unwrap(),
    ));
    choose_reverb_model_popup(&context, &mut app, 0x0204_0002);
    assert_eq!(
        app.podxt().pod_reverb_model_pending.unwrap().1.model_id(),
        0x0204_0002
    );
    click_pod_button_tall(&context, &mut app, "pod-reverb-model-apply");
    assert_eq!(
        app.podxt().pod_reverb_model_pending.unwrap().1.model_id(),
        0x0204_0002
    );
    assert_eq!(
        app.podxt().pod_library.tone(index).unwrap().as_bytes(),
        original
    );
    assert!(app.podxt().pod_delay_pending.is_some() && app.podxt().pod_mod_pending.is_some());
    click_pod_button_tall(&context, &mut app, "pod-reverb-model-discard");
    assert_eq!(app.podxt().pod_reverb_model_pending, None);
    assert_eq!(
        app.podxt().pod_library.tone(index).unwrap().as_bytes(),
        original
    );

    choose_reverb_model_popup(&context, &mut app, 0x0204_0003);
    click_pod_button_tall(&context, &mut app, "pod-reverb-model-apply");
    assert_eq!(
        app.podxt()
            .pod_library
            .tone(index)
            .unwrap()
            .reverb_model_id(),
        Some(0x0204_0003)
    );
    assert_eq!(app.podxt().pod_reverb_model_pending, None);
    app.podxt_mut().pod_library.revert_unsaved(index).unwrap();
    app.podxt_mut().refresh_selected_pod_name();
    assert_eq!(
        app.podxt().pod_library.tone(index).unwrap().as_bytes(),
        original
    );

    app.podxt_mut().pod_delay_pending = Some((index, PendingDelayEdit::TimeMs(350.0)));
    app.podxt_mut().pod_mod_pending = Some((index, PendingModEdit::SpeedHz(2.0)));
    app.podxt_mut().pod_stomp_model_pending = Some((
        index,
        pod_preset::PodStompModelChoice::try_from(0x0205_0000).unwrap(),
    ));
    choose_reverb_model_popup(&context, &mut app, 0x0204_0003);
    let bundle =
        pod_preset::PodBundleLibrary::parse(sample_pod_bundle(std::slice::from_ref(&original)))
            .unwrap();
    let bundle_baseline = bundle.as_bytes().to_vec();
    app.podxt_mut().pod_bundle = Some(bundle);
    click_pod_button_tall(&context, &mut app, "pod-reverb-model-apply");
    let edited = app.podxt().pod_library.tone(index).unwrap();
    assert_eq!(edited.reverb_model_id(), Some(0x0204_0003));
    assert!(app.podxt().pod_library.is_dirty(index));
    assert_eq!(app.podxt().pod_reverb_model_pending, None);
    assert_eq!(
        app.podxt().pod_delay_pending,
        Some((index, PendingDelayEdit::TimeMs(350.0)))
    );
    assert_eq!(
        app.podxt().pod_mod_pending,
        Some((index, PendingModEdit::SpeedHz(2.0)))
    );
    assert_eq!(
        app.podxt().pod_stomp_model_pending.unwrap().1.model_id(),
        0x0205_0000
    );
    let bundle = app.podxt().pod_bundle.as_ref().unwrap();
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
    app.select_test_editor::<PodxtEditor>();
    let original = sample_reverb_model_tone(0x0204_0002, pod_preset::REVERB_FALLBACK_SLOT);
    let other = sample_reverb_model_tone(0x0204_0003, pod_preset::REVERB_FALLBACK_SLOT);
    let index = app
        .podxt_mut()
        .pod_library
        .import_bytes(original.clone())
        .unwrap();
    let second = app
        .podxt_mut()
        .pod_library
        .import_bytes(other.clone())
        .unwrap();
    app.podxt_mut().pod_selected = Some(index);
    recording_frame_tall(&context, &mut app, vec![]);

    // Failed Apply retains the staged choice and the pre-existing dirty baseline.
    choose_reverb_model_popup(&context, &mut app, 0x0204_0003);
    let short_minf =
        sample_reverb_model_tone_with_tail(0x0204_0002, pod_preset::REVERB_FALLBACK_SLOT, &[]);
    app.podxt_mut()
        .pod_library
        .replace(index, short_minf.clone())
        .unwrap();
    app.podxt_mut().pod_delay_pending = Some((index, PendingDelayEdit::TimeMs(350.0)));
    app.podxt_mut().pod_mod_pending = Some((index, PendingModEdit::SpeedHz(2.0)));
    app.podxt_mut().pod_stomp_model_pending = Some((
        index,
        pod_preset::PodStompModelChoice::try_from(0x0205_0000).unwrap(),
    ));
    assert!(app.podxt().pod_library.is_dirty(index));
    click_pod_button_tall(&context, &mut app, "pod-reverb-model-apply");
    assert_eq!(
        app.podxt().pod_library.tone(index).unwrap().as_bytes(),
        short_minf
    );
    assert_eq!(
        app.podxt()
            .pod_library
            .tone(index)
            .unwrap()
            .reverb_model_id(),
        Some(0x0204_0002)
    );
    assert!(app.podxt().pod_library.is_dirty(index));
    assert_eq!(
        app.podxt().pod_reverb_model_pending.unwrap().1.model_id(),
        0x0204_0003
    );
    assert_eq!(
        app.podxt().pod_delay_pending,
        Some((index, PendingDelayEdit::TimeMs(350.0)))
    );
    assert_eq!(
        app.podxt().pod_mod_pending,
        Some((index, PendingModEdit::SpeedHz(2.0)))
    );
    assert_eq!(
        app.podxt().pod_stomp_model_pending.unwrap().1.model_id(),
        0x0205_0000
    );
    app.podxt_mut().pod_library.revert_unsaved(index).unwrap();
    app.podxt_mut().refresh_selected_pod_name();
    assert_eq!(
        app.podxt().pod_library.tone(index).unwrap().as_bytes(),
        original
    );
    assert!(!app.podxt().pod_library.is_dirty(index));
    assert_eq!(app.podxt().pod_reverb_model_pending, None);

    // Changing the selected row clears staged Reverb state.
    choose_reverb_model_popup(&context, &mut app, 0x0204_0003);
    let second_row = app.podxt().pod_rows[&second].center();
    click_pod_at_tall(&context, &mut app, second_row);
    recording_frame_tall(&context, &mut app, vec![]);
    assert_eq!(app.podxt().pod_selected, Some(second));
    assert_eq!(app.podxt().pod_reverb_model_pending, None);
    app.podxt_mut().pod_selected = Some(index);
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
    assert_eq!(app.podxt().pod_reverb_model_pending, None);
    assert_eq!(
        app.podxt()
            .pod_library
            .tone(app.podxt().pod_selected.unwrap())
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
    assert!(app.podxt().pod_dirty());
    click_pod_button_tall(&context, &mut app, "save-pod");
    assert!(app.podxt().pod_dirty());
    assert_eq!(
        std::fs::read(&collision).unwrap(),
        b"keep existing destination"
    );
    click_pod_button_tall(&context, &mut app, "save-pod");
    assert!(!app.podxt().pod_dirty());
    click_pod_button_tall(&context, &mut app, "load-pod");
    let reopened = app.podxt().pod_selected.unwrap();
    assert_eq!(
        app.podxt()
            .pod_library
            .tone(reopened)
            .unwrap()
            .reverb_model_id(),
        Some(0x0204_0002)
    );
    choose_reverb_model_popup(&context, &mut app, 0x0204_0003);
    click_pod_button_tall(&context, &mut app, "pod-reverb-model-apply");
    assert!(app.podxt().pod_dirty());
    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    click_prompt(&context, &mut app, "cancel");
    assert!(app.podxt().pod_dirty());
    app.podxt_mut()
        .pod_library
        .revert_unsaved(reopened)
        .unwrap();
    app.podxt_mut().refresh_selected_pod_name();
    assert!(!app.podxt().pod_dirty());
    assert_eq!(
        app.podxt()
            .pod_library
            .tone(reopened)
            .unwrap()
            .reverb_model_id(),
        Some(0x0204_0002)
    );
    assert_eq!(app.podxt().pod_reverb_model_pending, None);
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
    }
}

#[test]
fn stomp_model_gui_directly_replaces_all_gated_current_ids_and_invalidates_only_stomp_stage() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    app.select_test_editor::<PodxtEditor>();
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
        let index = app
            .podxt_mut()
            .pod_library
            .import_bytes(original.clone())
            .unwrap();
        app.podxt_mut().pod_selected = Some(index);
        recording_frame_tall(&context, &mut app, vec![]);
        assert_eq!(
            app.podxt()
                .pod_library
                .tone(index)
                .unwrap()
                .stomp_model_id(),
            Some(current_id),
            "gated current id {current_id:08x} stays raw"
        );
        app.podxt_mut().pod_harmony_pending = Some((
            index,
            pod_preset::StompHarmonyControl::Harmony1M335,
            PendingStompHarmony::Harmony1M335(pod_preset::StompHarmony1M335Selection::Unison),
        ));
        app.podxt_mut().pod_delay_pending = Some((index, PendingDelayEdit::TimeMs(350.0)));
        app.podxt_mut().pod_mod_pending = Some((index, PendingModEdit::SpeedHz(2.0)));
        choose_stomp_model_popup(&context, &mut app, 0x0205_0000);
        assert_eq!(
            app.podxt().pod_stomp_model_pending.unwrap().1.model_id(),
            0x0205_0000
        );
        assert_eq!(
            app.podxt().pod_library.tone(index).unwrap().as_bytes(),
            original
        );
        click_pod_button_tall(&context, &mut app, "pod-stomp-model-apply");
        let applied = app.podxt().pod_library.tone(index).unwrap();
        assert_eq!(applied.stomp_model_id(), Some(0x0205_0000));
        assert!(app.podxt().pod_library.is_dirty(index));
        assert_eq!(app.podxt().pod_stomp_model_pending, None);
        assert_eq!(app.podxt().pod_harmony_pending, None);
        assert_eq!(
            app.podxt().pod_delay_pending,
            Some((index, PendingDelayEdit::TimeMs(350.0)))
        );
        assert_eq!(
            app.podxt().pod_mod_pending,
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
        app.podxt_mut().pod_delay_pending = None;
        app.podxt_mut().pod_mod_pending = None;
    }
}

#[test]
fn stomp_model_gui_discards_and_bundle_apply_reopens_preserving_copy() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    app.select_test_editor::<PodxtEditor>();
    let original = sample_stomp_model_tone(0x0205_0006);
    app.podxt_mut()
        .load_pod_bundle_bytes(sample_pod_bundle(std::slice::from_ref(&original)))
        .unwrap();
    recording_frame_tall(&context, &mut app, vec![]);
    choose_stomp_model_popup(&context, &mut app, 0x0205_0000);
    click_pod_button_tall(&context, &mut app, "pod-stomp-model-discard");
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        original
    );
    assert!(!app.podxt().pod_bundle.as_ref().unwrap().is_dirty());

    app.podxt_mut().pod_harmony_pending = Some((
        0,
        pod_preset::StompHarmonyControl::Harmony1M335,
        PendingStompHarmony::Harmony1M335(pod_preset::StompHarmony1M335Selection::Unison),
    ));
    app.podxt_mut().pod_delay_pending = Some((0, PendingDelayEdit::TimeMs(350.0)));
    app.podxt_mut().pod_mod_pending = Some((0, PendingModEdit::SpeedHz(2.0)));
    choose_stomp_model_popup(&context, &mut app, 0x0205_0002);
    assert_eq!(
        app.podxt()
            .pod_harmony_pending
            .as_ref()
            .map(|stage| stage.0),
        Some(0)
    );
    assert_eq!(
        app.podxt().pod_delay_pending,
        Some((0, PendingDelayEdit::TimeMs(350.0)))
    );
    assert_eq!(
        app.podxt().pod_mod_pending,
        Some((0, PendingModEdit::SpeedHz(2.0)))
    );
    click_pod_button_tall(&context, &mut app, "pod-stomp-model-apply");
    assert!(app.podxt().pod_bundle.as_ref().unwrap().is_dirty());
    assert_eq!(app.podxt().pod_harmony_pending, None);
    assert_eq!(
        app.podxt().pod_delay_pending,
        Some((0, PendingDelayEdit::TimeMs(350.0)))
    );
    assert_eq!(
        app.podxt().pod_mod_pending,
        Some((0, PendingModEdit::SpeedHz(2.0)))
    );
    let bundle_tone = app.podxt().pod_bundle.as_ref().unwrap().tone(0, 0).unwrap();
    assert_eq!(bundle_tone.stomp_model_id(), Some(0x0205_0002));
    let directory = tempfile::tempdir().unwrap();
    let copy = directory.path().join("stomp-bundle-copy.l6c");
    app.podxt()
        .pod_bundle
        .as_ref()
        .unwrap()
        .save_copy(&copy)
        .unwrap();
    assert!(
        app.podxt()
            .pod_bundle
            .as_ref()
            .unwrap()
            .save_copy(&copy)
            .is_err()
    );
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
    let index = app
        .podxt_mut()
        .pod_library
        .import_bytes(original.clone())
        .unwrap();
    app.podxt_mut().pod_harmony_pending = Some((
        index,
        pod_preset::StompHarmonyControl::Harmony1457,
        PendingStompHarmony::Harmony1457(pod_preset::StompHarmony1457Selection::Unison),
    ));
    app.podxt_mut().pod_delay_pending = Some((index, PendingDelayEdit::TimeMs(400.0)));
    let same = pod_preset::PodStompModelChoice::try_from(0x0205_0000).unwrap();
    assert!(
        !app.podxt_mut()
            .apply_pod_stomp_model_choice(index, same)
            .unwrap()
    );
    assert_eq!(
        app.podxt().pod_library.tone(index).unwrap().as_bytes(),
        original
    );
    assert!(app.podxt().pod_harmony_pending.is_some());
    assert!(app.podxt().pod_delay_pending.is_some());

    let mut unknown = original.clone();
    let id = 0x0205_0000u32.to_be_bytes();
    let offset = unknown.windows(4).position(|window| window == id).unwrap();
    unknown[offset..offset + 4].copy_from_slice(&0xdead_beefu32.to_be_bytes());
    app.podxt_mut()
        .pod_library
        .replace(index, unknown.clone())
        .unwrap();
    let requested = pod_preset::PodStompModelChoice::try_from(0x0205_0002).unwrap();
    assert!(
        app.podxt_mut()
            .apply_pod_stomp_model_choice(index, requested)
            .is_err()
    );
    assert_eq!(
        app.podxt().pod_library.tone(index).unwrap().as_bytes(),
        unknown
    );
    assert!(app.podxt().pod_harmony_pending.is_some());
    assert!(app.podxt().pod_delay_pending.is_some());
}

#[test]
fn mod_model_gui_applies_and_discards_within_the_selected_bundle_entry() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    app.select_test_editor::<PodxtEditor>();
    let mut original = sample_delay_note_tone(0, 0);
    original[8..12].copy_from_slice(b"L6PA");
    let old = 0x0203_0004u32.to_be_bytes();
    let offset = original
        .windows(4)
        .position(|word| word == old)
        .expect("fixture has a Mod model id");
    original[offset..offset + 4].copy_from_slice(&0x0203_0015u32.to_be_bytes());
    app.podxt_mut()
        .load_pod_bundle_bytes(sample_pod_bundle(&[original.clone()]))
        .unwrap();

    recording_frame_tall(&context, &mut app, vec![]);
    stage_mod_note_choice(&context, &mut app, pod_preset::ModNoteChoice::Quarter);
    assert!(app.podxt().pod_mod_pending.is_some());
    choose_mod_model_popup(&context, &mut app, 0x0203_0005);
    click_pod_button_tall(&context, &mut app, "pod-mod-model-discard");
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        original
    );
    assert!(!app.podxt().pod_bundle.as_ref().unwrap().is_dirty());

    choose_mod_model_popup(&context, &mut app, 0x0203_0005);
    click_pod_button_tall(&context, &mut app, "pod-mod-model-apply");
    assert!(app.podxt().pod_bundle.as_ref().unwrap().is_dirty());
    assert_eq!(app.podxt().pod_mod_pending, None);
    assert_eq!(
        app.podxt()
            .pod_bundle
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
    let index = app
        .podxt_mut()
        .pod_library
        .import_bytes(original.clone())
        .unwrap();
    app.podxt_mut().pod_mod_pending = Some((
        index,
        PendingModEdit::Note(pod_preset::ModNoteChoice::Quarter),
    ));
    let same = pod_preset::PodModModelChoice::try_from(0x0203_0004).unwrap();
    app.podxt_mut()
        .apply_pod_mod_model_choice(index, same)
        .unwrap();
    assert_eq!(
        app.podxt().pod_mod_pending,
        Some((
            index,
            PendingModEdit::Note(pod_preset::ModNoteChoice::Quarter)
        ))
    );
    assert_eq!(
        app.podxt().pod_library.tone(index).unwrap().as_bytes(),
        original
    );

    let mut invalid = original;
    let current = 0x0203_0004u32.to_be_bytes();
    let offset = invalid.windows(4).position(|word| word == current).unwrap();
    invalid[offset..offset + 4].copy_from_slice(&0x3333_0001u32.to_be_bytes());
    app.podxt_mut()
        .pod_library
        .replace(index, invalid.clone())
        .unwrap();
    let pending = Some((
        index,
        PendingModEdit::Note(pod_preset::ModNoteChoice::Quarter),
    ));
    app.podxt_mut().pod_mod_pending = pending;
    assert!(
        app.podxt_mut()
            .apply_pod_mod_model_choice(index, same)
            .is_err()
    );
    assert_eq!(app.podxt().pod_mod_pending, pending);
    assert_eq!(
        app.podxt().pod_library.tone(index).unwrap().as_bytes(),
        invalid
    );
}

#[test]
fn mod_model_apply_save_reopen_navigation_load_revert_and_quit_guard() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    app.select_test_editor::<PodxtEditor>();
    let directory = tempfile::tempdir().unwrap();
    let collision = directory.path().join("occupied-mod.tone");
    let saved = directory.path().join("mod-model-copy.tone");
    std::fs::write(&collision, b"keep existing destination").unwrap();
    let mut original = sample_delay_note_tone(0, 0);
    original[8..12].copy_from_slice(b"L6PA");
    app.podxt_mut()
        .pod_library
        .import_bytes(original.clone())
        .unwrap();
    app.podxt_mut()
        .pod_library
        .import_bytes(original.clone())
        .unwrap();
    app.podxt_mut().pod_selected = Some(0);
    app.dialogs = Box::new(FakeDialogs {
        pod_save: [Some(collision.clone()), Some(saved.clone())].into(),
        pod_open: [Some(saved.clone())].into(),
        ..Default::default()
    });

    recording_frame_tall(&context, &mut app, vec![]);
    choose_mod_model_popup(&context, &mut app, 0x0203_0005);
    click_pod_button_tall(&context, &mut app, "pod-mod-model-apply");
    assert!(app.podxt().pod_dirty());
    click_pod_button_tall(&context, &mut app, "save-pod");
    assert!(app.podxt().pod_dirty());
    assert_eq!(
        std::fs::read(&collision).unwrap(),
        b"keep existing destination"
    );
    click_pod_button_tall(&context, &mut app, "save-pod");
    assert!(!app.podxt().pod_dirty());
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
    assert!(app.podxt().pod_mod_model_pending.is_some());
    let second = app.podxt().pod_rows[&1].center();
    click_pod_at_tall(&context, &mut app, second);
    recording_frame_tall(&context, &mut app, vec![]);
    assert_eq!(app.podxt().pod_selected, Some(1));
    assert_eq!(app.podxt().pod_mod_model_pending, None);

    let first = app.podxt().pod_rows[&0].center();
    click_pod_at_tall(&context, &mut app, first);
    recording_frame_tall(&context, &mut app, vec![]);
    choose_mod_model_popup(&context, &mut app, 0x0203_0007);
    assert!(app.podxt().pod_mod_model_pending.is_some());
    click_pod_button_tall(&context, &mut app, "load-pod");
    assert_eq!(app.podxt().pod_mod_model_pending, None);
    assert_eq!(
        app.podxt()
            .pod_library
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
    assert!(app.podxt().pod_dirty());
    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    click_prompt(&context, &mut app, "cancel");
    assert!(app.podxt().pod_dirty());
    let selected = app.podxt().pod_selected.unwrap();
    app.podxt_mut()
        .pod_library
        .revert_unsaved(selected)
        .unwrap();
    app.podxt_mut().refresh_selected_pod_name();
    assert!(!app.podxt().pod_dirty());
    assert_eq!(app.podxt().pod_mod_model_pending, None);
    assert_eq!(
        app.podxt()
            .pod_library
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
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
    }
}

#[test]
fn delay_model_gui_can_directly_replace_each_pack_gated_current_model() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    app.select_test_editor::<PodxtEditor>();

    for (ordinal, current_id) in (0x0202_000bu32..=0x0202_000fu32).enumerate() {
        let mut original = sample_delay_note_tone(0, 0);
        original[8..12].copy_from_slice(b"L6PA");
        let old_id = 0x0202_0002u32.to_be_bytes();
        let old_id_offset = original
            .windows(4)
            .position(|word| word == old_id)
            .expect("fixture has a Delay model id");
        original[old_id_offset..old_id_offset + 4].copy_from_slice(&current_id.to_be_bytes());
        let index = app
            .podxt_mut()
            .pod_library
            .import_bytes(original.clone())
            .unwrap();
        app.podxt_mut().pod_selected = Some(index);

        recording_frame_tall(&context, &mut app, vec![]);
        stage_delay_note_choice(&context, &mut app, pod_preset::DelayNoteChoice::Quarter);
        assert!(app.podxt().pod_delay_pending.is_some());
        choose_delay_model_popup(&context, &mut app, 0x0202_0002);
        assert_eq!(
            app.podxt().pod_delay_model_pending.unwrap().1.model_id(),
            0x0202_0002,
            "pack-gated current model {current_id:08x} must allow direct first-choice staging"
        );

        if ordinal == 0 {
            click_pod_button_tall(&context, &mut app, "pod-delay-model-discard");
            assert_eq!(
                app.podxt().pod_library.tone(index).unwrap().as_bytes(),
                original
            );
            assert!(app.podxt().pod_delay_pending.is_some());
            choose_delay_model_popup(&context, &mut app, 0x0202_0002);
        }

        click_pod_button_tall(&context, &mut app, "pod-delay-model-apply");
        let applied = app.podxt().pod_library.tone(index).unwrap();
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
        assert_eq!(app.podxt().pod_delay_pending, None);
        assert_eq!(app.podxt().pod_delay_model_pending, None);
    }

    // A same-ID apply leaves a fresh NOTE/TIME stage available.
    let same_id = app.podxt().pod_selected.unwrap();
    app.podxt_mut().pod_delay_pending = Some((same_id, PendingDelayEdit::TimeMs(350.0)));
    let same_choice = pod_preset::PodDelayModelChoice::try_from(0x0202_0002).unwrap();
    app.podxt_mut()
        .apply_pod_delay_model_choice(same_id, same_choice)
        .unwrap();
    assert_eq!(
        app.podxt().pod_delay_pending,
        Some((same_id, PendingDelayEdit::TimeMs(350.0)))
    );
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
    }
}

#[test]
fn delay_model_apply_invalidates_stale_parameter_stage_in_bundle() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    app.select_test_editor::<PodxtEditor>();
    let mut original = sample_delay_note_tone(0, 0);
    original[8..12].copy_from_slice(b"L6PA");
    app.podxt_mut()
        .load_pod_bundle_bytes(sample_pod_bundle(&[original.clone()]))
        .unwrap();
    recording_frame_tall(&context, &mut app, vec![]);
    stage_delay_note_choice(&context, &mut app, pod_preset::DelayNoteChoice::Quarter);
    assert!(app.podxt().pod_delay_pending.is_some());
    choose_delay_model_popup(&context, &mut app, 0x0202_0003);
    click_pod_button_tall(&context, &mut app, "pod-delay-model-apply");
    assert_eq!(app.podxt().pod_delay_pending, None);
    let (bank, preset) = app
        .podxt()
        .pod_bundle
        .as_ref()
        .unwrap()
        .entry_location(0)
        .unwrap();
    assert_eq!(
        app.podxt()
            .pod_bundle
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
    assert!(app.podxt().pod_bundle.as_ref().unwrap().is_dirty());
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
    }
}

#[test]
fn delay_model_apply_save_collision_reopen_revert_and_quit_guard() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    app.select_test_editor::<PodxtEditor>();
    let directory = tempfile::tempdir().unwrap();
    let collision = directory.path().join("occupied.tone");
    let saved = directory.path().join("delay-model-copy.tone");
    std::fs::write(&collision, b"keep existing destination").unwrap();
    let mut original = sample_delay_note_tone(0, 0);
    original[8..12].copy_from_slice(b"L6PA");
    app.podxt_mut().pod_library.import_bytes(original).unwrap();
    app.podxt_mut().pod_selected = Some(0);
    app.dialogs = Box::new(FakeDialogs {
        pod_save: [Some(collision.clone()), Some(saved.clone())].into(),
        pod_open: [Some(saved.clone())].into(),
        ..Default::default()
    });
    recording_frame_tall(&context, &mut app, vec![]);
    choose_delay_model_popup(&context, &mut app, 0x0202_0003);
    click_pod_button_tall(&context, &mut app, "pod-delay-model-apply");
    assert!(app.podxt().pod_dirty());
    click_pod_button_tall(&context, &mut app, "save-pod");
    assert!(app.podxt().pod_dirty());
    assert_eq!(
        std::fs::read(&collision).unwrap(),
        b"keep existing destination"
    );
    click_pod_button_tall(&context, &mut app, "save-pod");
    assert!(!app.podxt().pod_dirty());
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
    assert!(app.podxt().pod_dirty());
    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    click_prompt(&context, &mut app, "cancel");
    assert!(app.podxt().pod_dirty());
    app.podxt_mut().pod_library.revert_unsaved(0).unwrap();
    app.podxt_mut().refresh_selected_pod_name();
    assert!(!app.podxt().pod_dirty());
    assert_eq!(
        app.podxt()
            .pod_library
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
        app.podxt()
            .pod_library
            .tone(0)
            .unwrap()
            .models()
            .iter()
            .find(|m| m.slot == pod_preset::DELAY_SLOT)
            .unwrap()
            .model_id,
        0x0202_0003
    );
    assert!(!app.podxt().pod_dirty());
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
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
    app.select_test_editor::<PodxtEditor>();
    app.podxt_mut()
        .pod_library
        .import_bytes(original.clone())
        .unwrap();
    app.podxt_mut()
        .pod_library
        .import_bytes(original.clone())
        .unwrap();
    app.podxt_mut().pod_selected = Some(0);
    recording_frame_tall(&context, &mut app, vec![]);

    stage_delay_note_choice(&context, &mut app, pod_preset::DelayNoteChoice::Quarter);
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        original
    );
    assert_eq!(
        app.podxt().pod_delay_pending,
        Some((
            0,
            PendingDelayEdit::Note(pod_preset::DelayNoteChoice::Quarter)
        ))
    );
    let second_row = app.podxt().pod_rows[&1].center();
    click_pod_at_tall(&context, &mut app, second_row);
    recording_frame_tall(&context, &mut app, vec![]);
    assert_eq!(app.podxt().pod_selected, Some(1));
    assert!(app.podxt().pod_delay_pending.is_none());

    let first_row = app.podxt().pod_rows[&0].center();
    click_pod_at_tall(&context, &mut app, first_row);
    recording_frame_tall(&context, &mut app, vec![]);
    stage_delay_note_choice(&context, &mut app, pod_preset::DelayNoteChoice::Quarter);
    click_pod(&context, &mut app, "pod-delay-apply");
    let applied = app.podxt().pod_library.tone(0).unwrap().as_bytes().to_vec();
    assert_ne!(applied, original);
    assert!(app.podxt().pod_library.is_dirty(0));
    assert!(app.podxt().pod_delay_pending.is_none());
    assert_eq!(
        app.podxt()
            .pod_library
            .tone(0)
            .unwrap()
            .delay_note_value()
            .unwrap(),
        pod_preset::DelayNoteValue::Known(pod_preset::DelayNoteChoice::Quarter)
    );
    click_pod(&context, &mut app, "save-pod");
    assert_eq!(
        std::fs::read(&blocked).unwrap(),
        b"preserve existing NOTE destination"
    );
    assert!(app.podxt().pod_library.is_dirty(0));
    click_pod(&context, &mut app, "save-pod");
    let reopened = pod_preset::PodTone::parse(std::fs::read(&saved).unwrap()).unwrap();
    assert_eq!(
        reopened.delay_note_value().unwrap(),
        pod_preset::DelayNoteValue::Known(pod_preset::DelayNoteChoice::Quarter)
    );
    assert!(!app.podxt().pod_library.is_dirty(0));

    let saved_note = app.podxt().pod_library.tone(0).unwrap().as_bytes().to_vec();
    drag_delay_time_slider(&context, &mut app, 0.75);
    let PendingDelayEdit::TimeMs(staged_time) = app.podxt().pod_delay_pending.unwrap().1 else {
        panic!("TIME slider should stage a TIME edit");
    };
    assert!(
        staged_time
            > app
                .podxt()
                .pod_library
                .tone(0)
                .unwrap()
                .delay_time_ms()
                .unwrap()
    );
    recording_frame_tall(&context, &mut app, vec![]);
    assert_eq!(
        app.podxt().pod_delay_pending,
        Some((0, PendingDelayEdit::TimeMs(staged_time))),
        "TIME staging must persist across frames"
    );
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        saved_note
    );
    let second_row = app.podxt().pod_rows[&1].center();
    click_pod_at_tall(&context, &mut app, second_row);
    recording_frame_tall(&context, &mut app, vec![]);
    assert!(
        app.podxt().pod_delay_pending.is_none(),
        "tone switch clears staged TIME"
    );
    let first_row = app.podxt().pod_rows[&0].center();
    click_pod_at_tall(&context, &mut app, first_row);
    recording_frame_tall(&context, &mut app, vec![]);

    drag_delay_time_slider(&context, &mut app, 0.75);
    click_pod(&context, &mut app, "pod-delay-discard");
    assert!(app.podxt().pod_delay_pending.is_none());
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        saved_note
    );
    assert!(!app.podxt().pod_library.is_dirty(0));

    drag_delay_time_slider(&context, &mut app, 0.75);
    click_pod(&context, &mut app, "pod-delay-apply");
    let time_applied = app.podxt().pod_library.tone(0).unwrap().as_bytes().to_vec();
    assert_ne!(time_applied, saved_note);
    assert_eq!(
        app.podxt()
            .pod_library
            .tone(0)
            .unwrap()
            .delay_note_value()
            .unwrap(),
        pod_preset::DelayNoteValue::Known(pod_preset::DelayNoteChoice::Off),
        "positive NOTE clears only when TIME Apply succeeds"
    );
    assert!(app.podxt().pod_library.is_dirty(0));
    click_pod(&context, &mut app, "save-pod");
    assert_eq!(
        std::fs::read(&blocked_time).unwrap(),
        b"preserve existing TIME destination"
    );
    assert!(app.podxt().pod_library.is_dirty(0));
    click_pod(&context, &mut app, "save-pod");
    let reopened_time = pod_preset::PodTone::parse(std::fs::read(&saved_time).unwrap()).unwrap();
    assert_eq!(reopened_time.as_bytes(), time_applied);
    assert!(!app.podxt().pod_library.is_dirty(0));

    stage_delay_note_choice(&context, &mut app, pod_preset::DelayNoteChoice::Quarter);
    click_pod(&context, &mut app, "pod-delay-apply");
    assert!(app.podxt().pod_library.is_dirty(0));
    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    assert!(!app.permit_close);
    click_prompt(&context, &mut app, "discard");
    assert!(app.permit_close);
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        time_applied
    );
    assert!(!app.podxt().pod_library.is_dirty(0));
}

#[test]
fn delay_note_gui_rejects_coupled_mod_state_without_mutating() {
    let (context, mut app) = app();
    app.select_test_editor::<PodxtEditor>();
    let original = sample_delay_note_tone(0, 6);
    app.podxt_mut()
        .pod_library
        .import_bytes(original.clone())
        .unwrap();
    app.podxt_mut().pod_selected = Some(0);
    recording_frame_tall(&context, &mut app, vec![]);
    stage_delay_note_choice(&context, &mut app, pod_preset::DelayNoteChoice::Quarter);
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        original
    );
    click_pod(&context, &mut app, "pod-delay-apply");
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        original
    );
    assert!(!app.podxt().pod_library.is_dirty(0));
    assert!(app.podxt().pod_message.contains("Mod NOTE Off"));
    assert_eq!(
        app.podxt().pod_delay_pending,
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
    app.select_test_editor::<PodxtEditor>();
    app.podxt_mut()
        .load_pod_bundle_bytes(original_bundle.clone())
        .unwrap();
    recording_frame_tall(&context, &mut app, vec![]);

    drag_delay_time_slider(&context, &mut app, 0.75);
    let pending = app.podxt().pod_delay_pending;
    assert!(matches!(pending, Some((0, PendingDelayEdit::TimeMs(_)))));
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        original
    );
    assert!(!app.podxt().pod_bundle.as_ref().unwrap().is_dirty());
    click_pod(&context, &mut app, "pod-delay-apply");
    assert!(app.podxt().pod_bundle.as_ref().unwrap().is_dirty());
    assert!(app.podxt().pod_library.is_dirty(0));
    assert_eq!(
        app.podxt()
            .pod_library
            .tone(0)
            .unwrap()
            .delay_note_value()
            .unwrap(),
        pod_preset::DelayNoteValue::Known(pod_preset::DelayNoteChoice::Off)
    );
    assert!(app.podxt().pod_delay_pending.is_none());

    stage_delay_note_choice(&context, &mut app, pod_preset::DelayNoteChoice::Quarter);
    assert!(app.podxt().pod_delay_pending.is_some());
    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    click_prompt(&context, &mut app, "discard");
    assert!(
        app.podxt().pod_delay_pending.is_none(),
        "bundle revert clears pending NOTE"
    );
    assert!(!app.podxt().pod_bundle.as_ref().unwrap().is_dirty());
    assert!(!app.podxt().pod_library.is_dirty(0));
    assert_eq!(
        app.podxt().pod_bundle.as_ref().unwrap().as_bytes(),
        original_bundle
    );
    assert_eq!(
        app.podxt().pod_library.tone(0).unwrap().as_bytes(),
        original
    );
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
    }
}

#[test]
fn stomp_harmony_gui_hides_a_selector_with_duplicate_parameter_records() {
    for target in [0x0010_0001u32, 0x0010_0002] {
        let (context, mut app) = app();
        app.select_test_editor::<PodxtEditor>();
        let duplicate_tone =
            sample_stomp_harmony_pod_tone_with_duplicate(Some((target, 0x3f, 0x3f00_0000)));
        let index = app
            .podxt_mut()
            .pod_library
            .import_bytes(duplicate_tone)
            .unwrap();
        app.podxt_mut().pod_selected = Some(index);
        let original = app
            .podxt()
            .pod_library
            .tone(index)
            .unwrap()
            .as_bytes()
            .to_vec();
        recording_frame_tall(&context, &mut app, vec![]);
        if target == 0x0010_0001 {
            assert!(app.podxt().pod_harmony1_choices.is_empty());
            assert!(!app.podxt().pod_buttons.contains_key("pod-harmony1-apply"));
            assert_eq!(app.podxt().pod_harmony2_choices.len(), 9);
        } else {
            assert!(app.podxt().pod_harmony2_choices.is_empty());
            assert!(!app.podxt().pod_buttons.contains_key("pod-harmony2-apply"));
            assert_eq!(app.podxt().pod_harmony1_choices.len(), 9);
        }
        assert_eq!(
            app.podxt().pod_library.tone(index).unwrap().as_bytes(),
            original
        );
        assert!(!app.podxt().pod_library.is_dirty(index));
        const {
            assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
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
    assert_eq!(
        app.podxt().pod_library.len(),
        1,
        "{}",
        app.podxt().pod_message
    );
    recording_frame_tall(&context, &mut app, vec![]);
    assert!(!app.podxt().pod_library.is_dirty(0));
    assert!(matches!(
        app.podxt()
            .pod_library
            .tone(0)
            .unwrap()
            .stomp_wave_selection(),
        Ok(pod_preset::StompWaveValue::Known(
            pod_preset::StompWaveSelection::Wave5
        ))
    ));

    select_stomp_wave_choice(&context, &mut app, pod_preset::StompWaveSelection::Wave1);
    assert!(app.podxt().pod_library.is_dirty(0));
    assert_eq!(
        app.podxt()
            .pod_library
            .tone(0)
            .unwrap()
            .stomp_wave_selection()
            .unwrap(),
        pod_preset::StompWaveValue::Known(pod_preset::StompWaveSelection::Wave1)
    );

    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    click_prompt(&context, &mut app, "save");
    assert_eq!(
        std::fs::read(&blocked).unwrap(),
        b"preserve existing destination"
    );
    assert!(app.podxt().pod_library.is_dirty(0));
    assert!(!app.permit_close);

    // A cancelled save leaves the dirty edit available for another choice/save.
    click_prompt(&context, &mut app, "save");
    assert!(app.podxt().pod_library.is_dirty(0));
    assert!(!app.permit_close);
    click_prompt(&context, &mut app, "cancel");
    assert!(app.podxt().pod_library.is_dirty(0));

    app.request_close(&context);
    click_prompt(&context, &mut app, "save");
    assert!(app.permit_close);
    assert!(app.test_pending().is_none());
    assert!(!app.podxt().pod_library.any_dirty());
    let reopened = pod_preset::PodTone::parse(std::fs::read(saved).unwrap()).unwrap();
    assert_eq!(
        reopened.stomp_wave_selection().unwrap(),
        pod_preset::StompWaveValue::Known(pod_preset::StompWaveSelection::Wave1)
    );
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
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
        assert_eq!(
            app.podxt().pod_library.tone(0).unwrap().as_bytes(),
            original
        );
        assert!(!app.podxt().pod_library.is_dirty(0));
        assert!(matches!(
            app.podxt().pod_library.tone(0).unwrap().stomp_wave_selection(),
            Ok(pod_preset::StompWaveValue::Unknown { raw_bits: found }) if found == raw_bits
        ));
        select_stomp_wave_choice(&context, &mut app, pod_preset::StompWaveSelection::Wave8);
        assert_eq!(
            app.podxt().pod_library.tone(0).unwrap().as_bytes(),
            original
        );
        assert!(matches!(
            app.podxt().pod_library.tone(0).unwrap().stomp_wave_selection(),
            Ok(pod_preset::StompWaveValue::Unknown { raw_bits: found }) if found == raw_bits
        ));
        assert!(!app.podxt().pod_library.is_dirty(0));
        assert!(app.podxt().pod_message.contains("non-finite"));
        const {
            assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
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
    assert_eq!(app.podxt().pod_whammy_knob_sliders.len(), 2);
    assert!(
        !app.podxt().pod_library.is_dirty(0),
        "opening and rendering preserve raw values"
    );
    let toe_before = app
        .podxt()
        .pod_library
        .tone(0)
        .unwrap()
        .stomp_whammy_knob_position(pod_preset::StompWhammyKnob::Toe)
        .unwrap()
        .to_bits();
    drag_pod_whammy_slider(&context, &mut app, pod_preset::StompWhammyKnob::Heel);
    assert!(app.podxt().pod_library.is_dirty(0));
    assert_eq!(
        app.podxt()
            .pod_library
            .tone(0)
            .unwrap()
            .stomp_whammy_knob_position(pod_preset::StompWhammyKnob::Toe)
            .unwrap()
            .to_bits(),
        toe_before
    );
    let edited_heel = app
        .podxt()
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
        app.podxt().pod_library.is_dirty(0),
        "collision preserves dirty edit"
    );
    click_pod(&context, &mut app, "save-pod");
    assert!(
        app.podxt().pod_library.is_dirty(0),
        "cancelled save preserves dirty edit"
    );
    click_pod(&context, &mut app, "save-pod");
    assert!(!app.podxt().pod_library.is_dirty(0));
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
    assert!(app.podxt().pod_library.is_dirty(0));
    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    click_prompt(&context, &mut app, "cancel");
    assert!(!app.permit_close);
    assert!(app.podxt().pod_library.is_dirty(0));
    app.request_close(&context);
    click_prompt(&context, &mut app, "discard");
    assert!(app.permit_close);
    assert!(app.test_pending().is_none());
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
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
    assert!(app.podxt().pod_bundle.is_some());
    assert!(!app.podxt().pod_bundle.as_ref().unwrap().is_dirty());
    recording_frame_tall(&context, &mut app, vec![]);
    select_stomp_wave_choice(&context, &mut app, pod_preset::StompWaveSelection::Wave8);
    assert!(app.podxt().pod_bundle.as_ref().unwrap().is_dirty());
    assert!(app.podxt().pod_library.is_dirty(0));
    assert_eq!(
        app.podxt()
            .pod_bundle
            .as_ref()
            .unwrap()
            .tone(0, 0)
            .unwrap()
            .stomp_wave_selection()
            .unwrap(),
        pod_preset::StompWaveValue::Known(pod_preset::StompWaveSelection::Wave8)
    );

    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    click_prompt(&context, &mut app, "save");
    assert!(app.permit_close);
    assert!(!app.podxt().pod_bundle.as_ref().unwrap().is_dirty());
    assert!(!app.podxt().pod_library.any_dirty());
    assert_eq!(std::fs::read(source).unwrap(), original_bundle);
    let reopened = pod_preset::PodBundleLibrary::parse(std::fs::read(saved).unwrap()).unwrap();
    assert_eq!(
        reopened.tone(0, 0).unwrap().stomp_wave_selection().unwrap(),
        pod_preset::StompWaveValue::Known(pod_preset::StompWaveSelection::Wave8)
    );
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
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
    assert!(app.podxt().pod_bundle.is_some());
    recording_frame_tall(&context, &mut app, vec![]);
    let heel = app
        .podxt()
        .pod_library
        .tone(0)
        .unwrap()
        .stomp_whammy_knob_position(pod_preset::StompWhammyKnob::Heel)
        .unwrap()
        .to_bits();
    drag_pod_whammy_slider(&context, &mut app, pod_preset::StompWhammyKnob::Toe);
    assert!(app.podxt().pod_bundle.as_ref().unwrap().is_dirty());
    assert!(app.podxt().pod_library.is_dirty(0));
    let toe = app
        .podxt()
        .pod_library
        .tone(0)
        .unwrap()
        .stomp_whammy_knob_position(pod_preset::StompWhammyKnob::Toe)
        .unwrap()
        .to_bits();
    app.request_close(&context);
    click_prompt(&context, &mut app, "save");
    assert!(app.permit_close);
    assert!(!app.podxt().pod_bundle.as_ref().unwrap().is_dirty());
    assert!(!app.podxt().pod_library.any_dirty());
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
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
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
        assert_eq!(
            app.podxt().pod_library.len(),
            1,
            "{}",
            app.podxt().pod_message
        );
        assert_eq!(app.podxt().pod_selected, Some(0));
        recording_frame(&context, &mut app, vec![]);
        assert!(!app.podxt().pod_library.is_dirty(0));

        if model_id == 0x0202_0005 {
            assert!(matches!(
                app.podxt()
                    .pod_library
                    .tone(0)
                    .unwrap()
                    .delay_heads_selection(),
                Ok(pod_preset::DelayAssociationValue::Unknown { .. })
            ));
            select_delay_heads_choice(
                &context,
                &mut app,
                pod_preset::DelayHeadsSelection::OneTwoThreeFour,
            );
            assert_eq!(
                app.podxt()
                    .pod_library
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
                app.podxt()
                    .pod_library
                    .tone(0)
                    .unwrap()
                    .delay_bits_selection(),
                Ok(pod_preset::DelayAssociationValue::Unknown { .. })
            ));
            select_delay_bits_choice(&context, &mut app, pod_preset::DelayBitsSelection::Four);
            assert_eq!(
                app.podxt()
                    .pod_library
                    .tone(0)
                    .unwrap()
                    .delay_bits_selection()
                    .unwrap(),
                pod_preset::DelayAssociationValue::Known(pod_preset::DelayBitsSelection::Four)
            );
        }
        assert!(app.podxt().pod_library.is_dirty(0));
        app.request_close(&context);
        assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
        click_prompt(&context, &mut app, "save");
        assert!(app.permit_close);
        assert!(app.test_pending().is_none());
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
            assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
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
        assert_eq!(
            app.podxt().pod_library.tone(0).unwrap().as_bytes(),
            original
        );
        assert!(!app.podxt().pod_library.is_dirty(0));

        if model_id == 0x0202_0005 {
            assert!(matches!(
                app.podxt().pod_library.tone(0).unwrap().delay_heads_selection(),
                Ok(pod_preset::DelayAssociationValue::Unknown { raw_bits: found }) if found == raw_bits
            ));
            select_delay_heads_choice(
                &context,
                &mut app,
                pod_preset::DelayHeadsSelection::OneTwoThreeFour,
            );
        } else {
            assert!(matches!(
                app.podxt().pod_library.tone(0).unwrap().delay_bits_selection(),
                Ok(pod_preset::DelayAssociationValue::Unknown { raw_bits: found }) if found == raw_bits
            ));
            select_delay_bits_choice(&context, &mut app, pod_preset::DelayBitsSelection::Four);
        }

        assert!(
            app.podxt().pod_message.contains("non-finite"),
            "{}",
            app.podxt().pod_message
        );
        assert_eq!(
            app.podxt().pod_library.tone(0).unwrap().as_bytes(),
            original
        );
        assert!(!app.podxt().pod_library.is_dirty(0));
        const {
            assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
        }
    }
}
