//! Offline verification. Default mode needs no private files.
use sha2::{Digest, Sha256};
use std::{env, fs, path::Path};
use variax_workbench::{
    ParameterValue::{Boolean, Float, Integer},
    PresetMetadata,
    document::save_new,
    dsp::PACKED_BYTES,
    host::ModelLibrary,
    preset::PresetFile,
    rig_check::engine_check::{AUDITION_SHA256, audition_preset, compare, render_preset},
};

const CASES: usize = 1148;
const ASSET_SHA256: &str = "635c726e8092c095532443c024437594c812ec35bf088eb82b29f74d301d35ec";
#[cfg(not(feature = "portable-math"))]
const SYNTHETIC_SHA256: &str = "debe7587ec72ea5475d71d4704d07ded715f21e266b0f21c0f9c151dc59189f0";
#[cfg(feature = "portable-math")]
const SYNTHETIC_SHA256: &str = "85bab888a4211a3b8f2c9e26440eae8cdf3425ac2bd8a84a43bcd67d3104214a";

fn main() -> Result<(), String> {
    // The debug engine and preset loader need extra stack on Windows.
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(run)
        .map_err(|error| error.to_string())?
        .join()
        .map_err(|_| "model verification panicked".to_owned())?
}

fn run() -> Result<(), String> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    let models = ModelLibrary::built_in()?;
    let asset = format!("{:x}", Sha256::digest(ModelLibrary::built_in_asset()));
    println!("App model bundle SHA-256 {asset}");
    if asset != ASSET_SHA256 {
        return Err("model bundle checksum changed".into());
    }
    match args.as_slice() {
        [] => {
            let audition = render_preset(models, &audition_preset()?)?;
            let audition_sha = format!("{:x}", Sha256::digest(audition));
            let mut digest = Sha256::new();
            for metadata in synthetic_settings(models)? {
                let preset = PresetFile::from_metadata(&metadata, 0x0007_0004, 0x0310_0000)?;
                digest.update(render_preset(models, &preset)?);
            }
            let sweep = format!("{:x}", digest.finalize());
            println!("Math profile {}", variax_workbench::math::PROFILE);
            println!("Rig Check audition SHA-256 {audition_sha}");
            println!("{CASES} generated validation programs SHA-256 {sweep}");
            if audition_sha != AUDITION_SHA256 || sweep != SYNTHETIC_SHA256 {
                return Err("generated model-check checksums changed".into());
            }
            Ok(())
        }
        [flag, directory] if flag == "--export-synthetic" => {
            let directory = Path::new(directory);
            fs::create_dir(directory).map_err(|error| error.to_string())?;
            let inputs = synthetic_settings(models)?;
            for (index, metadata) in inputs.iter().enumerate() {
                let preset = PresetFile::from_metadata(metadata, 0x0007_0004, 0x0310_0000)?;
                save_new(&directory.join(format!("{index}.vwm")), preset.as_bytes())?;
            }
            save_new(&directory.join("audition.vwm"), audition_preset()?.as_bytes())
        }
        [flag, directory, expected, offset] if flag == "--oracle" => {
            let offset = offset.to_str().ok_or("offset is not UTF-8")?
                .parse::<usize>().map_err(|error| error.to_string())?;
            verify_oracle(models, Path::new(directory), Path::new(expected), offset)
        }
        _ => Err("usage: verify_variax_models [--oracle PRESET_DIR ORACLE.packed FIRST_INDEX | --export-synthetic NEW_DIR]".into()),
    }
}

fn verify_oracle(
    models: &ModelLibrary,
    directory: &Path,
    oracle: &Path,
    offset: usize,
) -> Result<(), String> {
    let expected = fs::read(oracle).map_err(|error| error.to_string())?;
    let end = offset.checked_add(CASES).ok_or("invalid first index")?;
    if expected.len() / PACKED_BYTES != end || !expected.len().is_multiple_of(PACKED_BYTES) {
        return Err("expected 1,148 oracle records after the first index".into());
    }
    let mut hash = Sha256::new();
    let mut matches = 0;
    for (index, reference) in expected
        .as_chunks::<PACKED_BYTES>()
        .0
        .iter()
        .enumerate()
        .skip(offset)
    {
        let path = directory.join(format!("{index}.vwm"));
        let preset = PresetFile::parse(fs::read(&path).map_err(|error| error.to_string())?)?;
        let program = render_preset(models, &preset)?;
        hash.update(&program);
        if program == reference {
            matches += 1;
        } else {
            let diffs = compare(reference, &program);
            eprintln!(
                "Case {index}: {} differing words, first at {}",
                diffs.len(),
                diffs[0].word
            );
        }
    }
    println!(
        "{matches}/{CASES} programs byte-for-byte identical ({} words compared).",
        CASES * 1092
    );
    println!(
        "Oracle SHA-256 {:x}",
        Sha256::digest(&expected[offset * PACKED_BYTES..])
    );
    println!("Bundled render SHA-256 {:x}", hash.finalize());
    if matches == CASES {
        Ok(())
    } else {
        Err("Workbench oracle mismatch".into())
    }
}

fn synthetic_settings(models: &ModelLibrary) -> Result<Vec<PresetMetadata>, String> {
    let pickups: Vec<_> = models
        .pickups
        .keys()
        .copied()
        .filter(|&id| id < i32::MAX as u32)
        .collect();
    let mut cases = Vec::with_capacity(CASES);
    for (index, (&body, definition)) in models.bodies.iter().enumerate() {
        let [bridge, middle, neck] = &definition.pickup_defaults;
        let mut base = PresetMetadata::from_bytes([0; 192])?;
        base.name = format!("Generated {index:02}");
        base.parameters.extend([
            ("body_model", Integer(body as i32)),
            ("pickup1_model", Integer(0x0311_0003)),
            ("pickup2_model", Integer(0x0311_0004)),
            ("pickup1_enabled", Boolean(true)),
            ("pickup2_enabled", Boolean(true)),
            ("pickup1_position", Float(bridge.position)),
            ("pickup2_position", Float(neck.position)),
            ("tone_capacitance", Float(0.022)),
            ("tone_resistance", Float(250.0)),
            ("volume_resistance", Float(250.0)),
            ("tone_position", Float(1.0)),
            ("alternate_tuning_enabled", Boolean(true)),
        ]);
        let mut renamed = base.clone();
        renamed.name = format!("Alternate {index:02}");
        cases.extend([base.clone(), renamed]);
        for (field, alternate) in [
            (
                "pickup1_model",
                Integer(pickups[index % pickups.len()] as i32),
            ),
            (
                "pickup2_model",
                Integer(pickups[(index + 17) % pickups.len()] as i32),
            ),
            ("pickup1_enabled", Boolean(false)),
            ("pickup2_enabled", Boolean(false)),
            ("alternate_tuning_enabled", Boolean(false)),
            ("trim", Float(-6.0)),
            (
                "pickup1_position",
                Float((bridge.position + neck.position) * 0.5),
            ),
            ("pickup1_angle", Float(12.0)),
            ("pickup1_level", Float(-2.0)),
            (
                "pickup2_position",
                Float(if definition.has_middle {
                    middle.position
                } else {
                    (bridge.position + neck.position) * 0.5
                }),
            ),
            ("pickup2_angle", Float(-12.0)),
            ("pickup2_phase_reverse", Boolean(true)),
            ("pickup2_level", Float(2.0)),
            ("pickups_in_series", Boolean(true)),
            ("volume_resistance", Float(500.0)),
            ("volume_taper", Integer(1)),
            ("tone_resistance", Float(500.0)),
            ("tone_capacitance", Float(0.047)),
            ("tone_position", Float(0.5)),
            ("save_tone_position", Boolean(true)),
            ("string1_tuning", Integer(-2)),
            ("string2_tuning", Integer(1)),
            ("string3_tuning", Integer(2)),
            ("string4_tuning", Integer(-1)),
            ("string5_tuning", Integer(3)),
            ("string6_tuning", Integer(-3)),
            ("string1_detune", Float(2.0)),
            ("string2_detune", Float(3.0)),
            ("string3_detune", Float(4.0)),
            ("string4_detune", Float(5.0)),
            ("string5_detune", Float(6.0)),
            ("string6_detune", Float(7.0)),
            ("string1_mix", Float(0.125)),
            ("string2_mix", Float(0.25)),
            ("string3_mix", Float(0.375)),
            ("string4_mix", Float(0.5)),
            ("string5_mix", Float(0.625)),
            ("string6_mix", Float(0.75)),
            ("string_spread", Float(0.5)),
        ] {
            let mut modified = base.clone();
            modified.parameters.insert(field, alternate);
            cases.push(modified);
        }
    }
    assert_eq!(cases.len(), CASES);
    Ok(cases)
}
