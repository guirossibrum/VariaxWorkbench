//! Offline complete preset rendering with synthetic resources only.
use std::process::Command;
fn resources(dir: &std::path::Path) -> std::path::PathBuf {
    let mut entries = String::new();
    for (i, (id, bytes)) in [
        0x3100003, 0x310000d, 0x3100012, 0x3100013, 0x3100015, 0x3100018, 0x310001b, 0x3100011,
    ]
    .into_iter()
    .zip(
        include_bytes!("fixtures/native-dsp-body-inputs.bin")
            .as_chunks::<17483>()
            .0,
    )
    .enumerate()
    {
        let name = format!("body-{i}.bin");
        std::fs::write(dir.join(&name), bytes).unwrap();
        entries.push_str(&format!("body\t{id:08x}\t{name}\n"));
    }
    for (i, (id, bytes)) in [0x3110001, 0x3110000, (-2i32) as u32, (-3i32) as u32]
        .into_iter()
        .zip(
            include_bytes!("fixtures/native-host-pickups.bin")
                .as_chunks::<85>()
                .0,
        )
        .enumerate()
    {
        let name = format!("pickup-{i}.bin");
        std::fs::write(dir.join(&name), bytes).unwrap();
        entries.push_str(&format!("pickup\t{id:08x}\t{name}\n"));
    }
    let path = dir.join("models.tsv");
    std::fs::write(&path, entries).unwrap();
    path
}
#[test]
fn complete_file_render_matches_original_java_native_and_refuses_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = resources(dir.path());
    let preset = dir.path().join("preset.vwm");
    std::fs::write(
        &preset,
        include_bytes!("fixtures/synthetic-host-preset.vwm"),
    )
    .unwrap();
    let output = dir.path().join("rendered.bin");
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_variax-workbench"))
            .arg("render-preset")
            .arg(&preset)
            .arg(&manifest)
            .arg(&output)
            .output()
            .unwrap()
    };
    let result = run();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let expected = include_bytes!("fixtures/synthetic-host-preset.packed");
    assert_eq!(std::fs::read(&output).unwrap(), expected);
    assert!(!run().status.success());
    assert_eq!(std::fs::read(&output).unwrap(), expected);
}
#[test]
fn invalid_preset_or_missing_resources_create_no_output() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = dir.path().join("models.tsv");
    std::fs::write(&manifest, "").unwrap();
    let preset = dir.path().join("preset.vwm");
    let output = dir.path().join("rendered.bin");
    for bytes in [
        &b"invalid"[..],
        include_bytes!("fixtures/synthetic-host-preset.vwm").as_slice(),
    ] {
        std::fs::write(&preset, bytes).unwrap();
        let result = Command::new(env!("CARGO_BIN_EXE_variax-workbench"))
            .arg("render-preset")
            .arg(&preset)
            .arg(&manifest)
            .arg(&output)
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(!output.exists());
        assert_eq!(std::fs::read(&preset).unwrap(), bytes);
    }
}

#[test]
fn adjacent_acoustic_trim_edits_cross_one_packed_q23_count() {
    use variax_workbench::host::{HostEngine, HostParameters, ModelLibrary};
    use variax_workbench::preset::PresetFile;

    let dir = tempfile::tempdir().unwrap();
    let library = ModelLibrary::load_manifest(&resources(dir.path())).unwrap();
    let mut preset =
        PresetFile::parse(include_bytes!("fixtures/synthetic-host-preset.vwm").to_vec()).unwrap();
    preset.set_enabled(0x0201_0001, false).unwrap();
    preset.set_enabled(0x0201_0002, false).unwrap();
    preset.select_model(0x0201_0003, 0x0310_0012).unwrap();

    // Exact binary32 input neighbors and original-instruction word 555 values.
    // See docs/math-quantization-threshold.md for the reference and widths.
    let outputs = [
        (0xc040_0099, [0x8b, 0xda, 0x34]),
        (0xc040_009a, [0x8b, 0xda, 0x34]),
        (0xc040_009b, [0x8b, 0xda, 0x33]),
        (0xc040_009c, [0x8b, 0xda, 0x33]),
    ]
    .map(|(bits, expected)| {
        preset
            .set_parameter(0x0201_0000, 0x20_0000, 1, bits)
            .unwrap();
        let reopened = PresetFile::parse(preset.as_bytes().to_vec()).unwrap();
        let params = HostParameters::from_preset(&reopened).unwrap();
        let settings = params.map(Default::default()).unwrap();
        assert_eq!(settings.word(0xbc), bits);

        let packed = HostEngine::new(&library)
            .generate(&settings)
            .unwrap()
            .packed();
        assert_eq!(packed[555 * 3..556 * 3], expected, "trim {bits:08x}");
        packed
    });

    assert_eq!(outputs[0], outputs[1]);
    assert_eq!(outputs[2], outputs[3]);
    let changed_words: Vec<_> = outputs[1]
        .as_chunks::<3>()
        .0
        .iter()
        .zip(outputs[2].as_chunks::<3>().0)
        .enumerate()
        .filter_map(|(i, (left, right))| (left != right).then_some(i))
        .collect();
    assert_eq!(changed_words, [555, 1091]);
}
