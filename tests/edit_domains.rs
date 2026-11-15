use variax_workbench::{document::EditorDocument, edit::*, preset::PresetFile};

fn preset() -> PresetFile {
    PresetFile::parse(include_bytes!("fixtures/synthetic-host-preset.vwm").to_vec()).unwrap()
}

#[test]
fn every_exposed_parameter_accepts_boundaries_and_rejects_invalid_bits_atomically() {
    let mut base = preset();
    base.select_model(BODY, 0x0310000d).unwrap();
    base.set_parameter(WIRING, 0x200005, 0, 1).unwrap();
    let mut checked = 0;
    for model in base.models() {
        for parameter in &model.parameters {
            let domain = domain(model.slot, parameter.id).unwrap();
            let (valid, invalid): (Vec<u32>, Vec<u32>) = match domain {
                Domain::Float { min, max, .. } => (
                    vec![min.to_bits(), max.to_bits()],
                    vec![
                        min.next_down().to_bits(),
                        max.next_up().to_bits(),
                        f32::NAN.to_bits(),
                        f32::INFINITY.to_bits(),
                        f32::NEG_INFINITY.to_bits(),
                    ],
                ),
                Domain::Integer { min, max } => (
                    vec![min as u32, max as u32],
                    vec![(min - 1) as u32, (max + 1) as u32],
                ),
                Domain::Choices(values) => (
                    values.iter().map(|v| v.to_bits()).collect(),
                    vec![0, f32::NAN.to_bits(), f32::INFINITY.to_bits()],
                ),
            };
            for bits in valid {
                let mut patch = base.clone();
                patch
                    .set_parameter(model.slot, parameter.id, parameter.value_type, bits)
                    .unwrap();
                let reread = PresetFile::parse(patch.as_bytes().to_vec()).unwrap();
                assert_eq!(
                    reread.parameter(model.slot, parameter.id).unwrap().bits,
                    bits
                );
            }
            for bits in invalid {
                let mut patch = base.clone();
                assert!(
                    patch
                        .set_parameter(model.slot, parameter.id, parameter.value_type, bits)
                        .is_err(),
                    "{:08x}/{:06x} {bits:08x}",
                    model.slot,
                    parameter.id
                );
                assert_eq!(patch.as_bytes(), base.as_bytes());
            }
            let mut patch = base.clone();
            assert!(
                patch
                    .set_parameter(
                        model.slot,
                        parameter.id,
                        1 - parameter.value_type,
                        parameter.bits
                    )
                    .is_err()
            );
            assert_eq!(patch.as_bytes(), base.as_bytes());
            checked += 1;
        }
    }
    assert_eq!(checked, 34);
}

#[test]
fn body_and_toggle_restrictions_preserve_dormant_settings_and_failed_history() {
    let original = preset();
    let mut patch = original.clone();
    for body in 0x03100012..=0x0310001b {
        patch.select_model(BODY, body).unwrap();
        if is_acoustic(body) {
            let before = patch.as_bytes().to_vec();
            assert!(patch.set_enabled(PICKUP1, false).is_err());
            assert!(patch.select_model(PICKUP1, 0x03110003).is_err());
            assert!(
                patch
                    .set_parameter(WIRING, 0x200003, 1, 0.1f32.to_bits())
                    .is_err()
            );
            assert_eq!(patch.as_bytes(), before);
        }
    }
    patch.select_model(BODY, 0x03100018).unwrap();
    assert!(patch.set_enabled(TUNING, true).is_err());
    assert!(patch.set_parameter(TUNING, 0x200000, 0, 1).is_err());
    patch.select_model(BODY, 0x03100003).unwrap();
    assert_eq!(patch.as_bytes(), original.as_bytes());
    assert!(
        patch
            .set_parameter(TUNING, 0x20000c, 1, 0.5f32.to_bits())
            .is_err()
    );
    assert!(
        patch
            .set_parameter(WIRING, 0x200004, 1, 0.5f32.to_bits())
            .is_err()
    );
    assert!(
        patch
            .set_parameter(TUNING, 0x200006, 1, 0.5f32.to_bits())
            .is_err()
    );
    let mut doc = EditorDocument::from_preset(patch, "synthetic.vwm".into());
    doc.apply(|p| p.rename("Undoable")).unwrap();
    doc.undo();
    assert!(doc.can_redo());
    assert!(
        doc.apply(|p| {
            p.rename("Partial")?;
            p.set_parameter(PICKUP1, 0x200000, 1, f32::NAN.to_bits())
        })
        .is_err()
    );
    assert_eq!(doc.preset().as_bytes(), original.as_bytes());
    assert!(!doc.is_dirty());
    assert!(!doc.can_undo());
    assert!(doc.can_redo());
}

#[test]
fn original_ui_model_catalog_matches_allowed_choices_and_unknown_ids_cannot_be_selected() {
    for (slot, first, last) in [
        (BODY, 0x03100000, 0x0310001b),
        (PICKUP1, 0x03110000, 0x03110015),
        (PICKUP2, 0x03118000, 0x03118009),
    ] {
        for id in first..=last {
            let mut p = preset();
            p.select_model(slot, id).unwrap();
        }
    }
    for (slot, id) in [
        (BODY, 0x0310001c),
        (PICKUP1, 0x03110016),
        (PICKUP2, 0x0311800a),
        (BODY, 0x03110000),
        (WIRING, 0),
    ] {
        let mut p = preset();
        let before = p.as_bytes().to_vec();
        assert!(p.select_model(slot, id).is_err());
        assert_eq!(p.as_bytes(), before);
    }
}

#[test]
fn cli_rejects_invalid_edits_before_creating_an_output_file() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.vwm");
    let output = dir.path().join("output.vwm");
    let original = preset();
    std::fs::write(&input, original.as_bytes()).unwrap();
    for (slot, id, kind, value) in [
        ("02010001", "200000", "float", "9.6"),
        ("02010002", "200002", "int", "2"),
        ("02010004", "200000", "float", "123"),
        ("02010005", "200000", "int", "-13"),
    ] {
        let result = std::process::Command::new(env!("CARGO_BIN_EXE_variax-workbench"))
            .arg("set")
            .arg(&input)
            .arg(&output)
            .args([slot, id, kind, value])
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(!output.exists());
        assert_eq!(std::fs::read(&input).unwrap(), original.as_bytes());
    }
}

#[test]
fn every_visible_body_and_pickup_pair_obeys_original_panel_restrictions() {
    let acoustic = [0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x1a, 0x1b];
    let pickups = (0x03110000..=0x03110015)
        .chain(0x03118000..=0x03118009)
        .collect::<Vec<_>>();
    let mut checked = 0;
    for body in 0x03100000..=0x0310001b {
        for slot in [PICKUP1, PICKUP2] {
            for &pickup in &pickups {
                let mut p = preset();
                p.select_model(BODY, body).unwrap();
                let before = p.as_bytes().to_vec();
                let result = p.select_model(slot, pickup);
                if acoustic.contains(&(body & 0xffff)) {
                    assert!(result.is_err());
                    assert_eq!(p.as_bytes(), before);
                } else {
                    result.unwrap();
                    assert_eq!(p.model(slot).unwrap().id, pickup);
                }
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 1792);
}
