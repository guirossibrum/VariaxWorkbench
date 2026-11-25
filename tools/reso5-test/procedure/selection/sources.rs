use super::{Image, Mode, Prepared, index, slot_label, target};
use crate::procedure::{
    METADATA, STORED_HASH, expected_stored_image, matches_hash, read, set_firmware_checksum,
    variax_identity,
};
use line6_devices::{
    backup,
    family::variax::{ParameterValue, Preset, PresetMetadata, write},
    identity::Identity,
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, path::Path};
use variax_workbench::{
    factory_reference::FactoryReference,
    host::{HostEngine, HostParameters, ModelLibrary},
    preset::PresetFile,
};

// Reuse the #192 formatter directly. The owner binary also builds without GUI.
#[allow(dead_code)]
#[path = "../../../../src/families/variax/editor/value_format.rs"]
mod value_format;
use value_format::ValueFormat;

const ORIGINAL_RESO: &str = "3d8bae813991fa5996fb649ad80d116de8ef358c03724770183b89cdef5ad249";

pub(super) fn metadata(image: &Image) -> Result<PresetMetadata, String> {
    PresetMetadata::from_bytes(image[METADATA].try_into().expect("settings length"))
}

pub(super) fn digest(image: &Image) -> String {
    format!("{:x}", Sha256::digest(image))
}

pub(super) fn firmware_image(bytes: Vec<u8>) -> Result<Image, String> {
    let image: Image = bytes
        .try_into()
        .map_err(|_| "Expected a 3,276-byte Variax program.")?;
    let mut checked = image;
    set_firmware_checksum(&mut checked);
    if image != checked {
        return Err("Invalid firmware program checksum.".into());
    }
    metadata(&image)?;
    Ok(image)
}

fn program_file(preset: Preset) -> String {
    format!(
        "programs/{:02}-{}-{}.bin",
        index(preset),
        preset.bank_name().replace(' ', "_"),
        preset.slot() + 1
    )
}

fn library(dir: &Path) -> Result<Vec<Image>, String> {
    if dir
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with("-INCOMPLETE") || name.ends_with(".partial"))
    {
        return Err("Use the complete #236 byte-level backup.".into());
    }
    let manifest =
        std::fs::read_to_string(dir.join("MANIFEST.sha256")).map_err(|e| e.to_string())?;
    let files: BTreeSet<_> = manifest
        .lines()
        .filter_map(|line| line.split_once("  ").map(|(_, path)| path))
        .collect();
    backup::verify_manifest(dir)?;
    if !files.contains("identity.syx") {
        return Err("The backup manifest omits identity.syx.".into());
    }
    variax_identity(
        &Identity::parse(&read(&dir.join("identity.syx"))?).ok_or("Invalid backup identity.")?,
    )?;
    (0..60)
        .map(|i| {
            let file = program_file(target(i)?);
            if !files.contains(file.as_str()) {
                return Err(format!("The backup manifest omits {file}."));
            }
            firmware_image(read(&dir.join(&file))?).map_err(|error| format!("{file}: {error}"))
        })
        .collect()
}

fn archive_slot(factory: &FactoryReference, index: u8) -> Result<Image, String> {
    let mut image = factory.slots()[usize::from(index)].raw_program().to_vec();
    for word in &mut image.as_chunks_mut::<3>().0[..] {
        word.reverse();
    }
    firmware_image(image)
}

pub(super) fn models(path: &Path) -> Result<ModelLibrary, String> {
    if variax_workbench::math::PROFILE != "host-std-v1" {
        return Err("Build the owner tool without portable-math to render a .vwm.".into());
    }
    ModelLibrary::load_manifest(path).map_err(|error| format!("{}: {error}", path.display()))
}

pub(super) fn render_file(path: &Path, models: &ModelLibrary) -> Result<Image, String> {
    if !path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("vwm"))
    {
        return Err("Expected an editor .vwm.".into());
    }
    render(&PresetFile::parse(read(path)?)?, models)
}

pub(super) fn render(preset: &PresetFile, models: &ModelLibrary) -> Result<Image, String> {
    if !matches!(preset.device_id(), 0x70000 | 0x70004) || &preset.as_bytes()[8..12] != b"L6PA" {
        return Err("Expected a first-generation Variax FORM/L6PA .vwm preset.".into());
    }
    let parameters = HostParameters::from_preset(preset)?;
    let settings = parameters.map(Default::default())?;
    let generated = HostEngine::new(models).generate(&settings)?;
    let image = parameters.finish(preset.name(), &settings, generated.packed());
    write::Program::new(preset.name(), &image)?;
    Ok(image)
}

fn source(
    path: &Path,
    manifest: &Path,
    factory: &FactoryReference,
    slot: u8,
) -> Result<Image, String> {
    if path == Path::new("archive") {
        return archive_slot(factory, slot);
    }
    if let Some(number) = path.to_str().and_then(|text| text.strip_prefix("archive:")) {
        let index = number
            .parse::<u8>()
            .map_err(|_| "Invalid archive flash index.")?;
        target(index)?;
        return archive_slot(factory, index);
    }
    match path.extension().and_then(|extension| extension.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("bin") => firmware_image(read(path)?),
        Some(ext) if ext.eq_ignore_ascii_case("vwm") => render_file(path, &models(manifest)?),
        _ => Err("Source must be archive, a backup .bin, or an editor .vwm.".into()),
    }
}

fn renamed(mut image: Image) -> Image {
    image[METADATA.start..METADATA.start + 18].copy_from_slice(b"Reso-5 T          ");
    set_firmware_checksum(&mut image);
    image
}

pub(super) fn load_current_library(
    backup: &Path,
    archive: &Path,
) -> Result<(Vec<Image>, FactoryReference, Image), String> {
    let current = library(backup)?;
    let factory = FactoryReference::parse(&read(archive)?)?;
    let stamped_reso = expected_stored_image(&archive_slot(&factory, 50)?);
    if !matches_hash(&current[50], ORIGINAL_RESO) || !matches_hash(&stamped_reso, STORED_HASH) {
        return Err(
            "The slot 50 backup or the post-#177 factory baseline has the wrong hash.".into(),
        );
    }
    Ok((current, factory, stamped_reso))
}

pub fn prepare(
    mode: Mode,
    slot: u8,
    backup: &Path,
    archive: &Path,
    input: &Path,
    models: &Path,
) -> Result<Prepared, String> {
    let preset = target(slot)?;
    if matches!(mode, Mode::RenameReso | Mode::FactoryReso)
        && (slot != 50 || input != Path::new("archive"))
    {
        return Err("Rungs 3a and 3b require flash slot 50 and the archive source.".into());
    }
    let (mut current, factory, stamped_reso) = load_current_library(backup, archive)?;
    let factory_reso = archive_slot(&factory, 50)?;
    let i = usize::from(slot);
    let backed_up = current[i];
    current[50] = stamped_reso;
    if mode == Mode::FactoryReso {
        current[50] = expected_stored_image(&renamed(factory_reso));
    }
    let program = match mode {
        Mode::RenameReso => renamed(factory_reso),
        Mode::FactoryReso => factory_reso,
        Mode::Restore => {
            current[i] = expected_stored_image(&source(input, models, &factory, slot)?);
            backed_up
        }
        Mode::Listen | Mode::Store => source(input, models, &factory, slot)?,
    };
    finish(mode, preset, current, backed_up, program)
}

pub(super) fn finish(
    mode: Mode,
    target: Preset,
    current: Vec<Image>,
    backed_up: Image,
    program: Image,
) -> Result<Prepared, String> {
    write::Program::new(&metadata(&program)?.name, &program)?;
    let expected = if mode.listens() {
        program
    } else {
        expected_stored_image(&program)
    };
    if mode == Mode::Restore && expected != backed_up {
        return Err("The stamped restore would differ from the target backup file.".into());
    }
    let expected_current = &current[usize::from(index(target))];
    let mut description = vec![
        format!("{} {}", mode.label(), slot_label(target)),
        format!("Target preflight SHA-256: {}", digest(expected_current)),
        format!("Upload SHA-256: {}", digest(&program)),
        format!(
            "Expected {} image SHA-256: {}",
            if mode.listens() { "working" } else { "stored" },
            digest(&expected)
        ),
        "Settings changes versus the target's #236 backup:".into(),
    ];
    description.extend(diff(&backed_up, &program)?);
    if expected_current[METADATA] != backed_up[METADATA] {
        description.push("Settings changes versus the expected current target:".into());
        description.extend(diff(expected_current, &program)?);
    }
    Ok(Prepared {
        target,
        mode,
        current,
        program,
        expected,
        description,
    })
}

#[derive(serde::Deserialize)]
struct ModelName {
    id: i32,
    name: String,
}

fn format_value(key: &str, value: &ParameterValue, catalog: &[ModelName]) -> String {
    let ParameterValue::Float(value) = value else {
        return match value {
            ParameterValue::Boolean(on) => if *on { "On" } else { "Off" }.into(),
            ParameterValue::Integer(id) if key.ends_with("_model") => catalog
                .iter()
                .find(|model| model.id == *id)
                .map_or_else(|| format!("0x{id:08X}"), |model| model.name.clone()),
            ParameterValue::Integer(0) if key == "volume_taper" => "Linear".into(),
            ParameterValue::Integer(1) if key == "volume_taper" => "Audio".into(),
            ParameterValue::Integer(steps) if key.ends_with("_tuning") => {
                ValueFormat::SEMITONES.display(f64::from(*steps))
            }
            ParameterValue::Integer(value) => value.to_string(),
            _ => unreachable!(),
        };
    };
    let value = f64::from(*value);
    match key {
        "trim" => ValueFormat::TRIM.display(value),
        "tone_position" => ValueFormat::Percent(1.0).display(value),
        "string_spread" => ValueFormat::Percent(25.0).display(value),
        "tone_resistance" | "volume_resistance" => format!("{value} kΩ"),
        "tone_capacitance" if value < 0.0 => "Filter".into(),
        "tone_capacitance" if value >= 0.099 => format!("{value:.1} µF"),
        "tone_capacitance" => format!("{:.1} nF", value * 1000.0),
        _ if key.ends_with("_position") => ValueFormat::POSITION.display(value),
        _ if key.ends_with("_angle") => ValueFormat::ANGLE.display(value),
        _ if key.ends_with("_level") => ValueFormat::LEVEL.display(value),
        _ if key.ends_with("_detune") => ValueFormat::CENTS.display(value),
        _ if key.ends_with("_mix") => ValueFormat::Percent(1.0).display(value),
        _ => value.to_string(),
    }
}

fn catalog() -> Vec<ModelName> {
    serde_json::from_str(include_str!("../../../../assets/model-ui.json"))
        .expect("bundled model catalog")
}

pub(super) fn audition_sound(old: &Image, new: &Image) -> Result<String, String> {
    // Word 1 bit 4 controls the DSP pitch block independently of the settings.
    let (old_pitch, new_pitch) = (old[5] & 0x10 != 0, new[5] & 0x10 != 0);
    let (old, new) = (metadata(old)?, metadata(new)?);
    let catalog = catalog();
    let describe = |data: &PresetMetadata, pitch_enabled: bool| {
        let model = |key| format_value(key, &data.parameters[key], &catalog);
        let mut pickups = Vec::new();
        for (enabled, id) in [
            ("pickup1_enabled", "pickup1_model"),
            ("pickup2_enabled", "pickup2_model"),
        ] {
            if data.parameters[enabled] == ParameterValue::Boolean(true) {
                pickups.push(model(id));
            }
        }
        let ParameterValue::Integer(body) = data.parameters["body_model"] else {
            unreachable!("decoded body type");
        };
        let pickups = if variax_workbench::edit::is_acoustic(body as u32) {
            "acoustic body voice".into()
        } else if pickups.is_empty() {
            "no enabled pickups".into()
        } else {
            pickups.join(" and ")
        };
        let tuning = if pitch_enabled
            && data.parameters["alternate_tuning_enabled"] == ParameterValue::Boolean(true)
        {
            let steps = (1..=6)
                .map(
                    |i| match data.parameters[format!("string{i}_tuning").as_str()] {
                        ParameterValue::Integer(steps) => format!("{steps:+}"),
                        _ => unreachable!("decoded tuning type"),
                    },
                )
                .collect::<Vec<_>>()
                .join("/");
            if variax_workbench::edit::is_twelve_string(body as u32) {
                let formatted = |kind| {
                    (1..=6)
                        .map(|i| {
                            let key = format!("string{i}_{kind}");
                            format_value(&key, &data.parameters[key.as_str()], &catalog)
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                format!(
                    "alternate tuning on ({steps} semitones, strings 1–6), detune {}, mix {}",
                    formatted("detune"),
                    formatted("mix")
                )
            } else {
                format!("alternate tuning on ({steps} semitones, strings 1–6)")
            }
        } else if pitch_enabled {
            "alternate tuning off in the settings, pitch processing active".into()
        } else {
            "alternate tuning off".into()
        };
        format!("{} body, {pickups}, {tuning}", model("body_model"))
    };
    Ok(format!(
        "Expect the edited sound: {}. Your starting preset had {}.",
        describe(&new, new_pitch),
        describe(&old, old_pitch)
    ))
}

pub(super) fn diff(old: &Image, new: &Image) -> Result<Vec<String>, String> {
    let (old, new) = (metadata(old)?, metadata(new)?);
    let catalog = catalog();
    let mut changes = Vec::new();
    if old.name != new.name {
        changes.push(format!("  Preset name: {:?} → {:?}", old.name, new.name));
    }
    for (key, old) in &old.parameters {
        let new = &new.parameters[key];
        let same = match (old, new) {
            (ParameterValue::Float(a), ParameterValue::Float(b)) => a.to_bits() == b.to_bits(),
            _ => old == new,
        };
        if !same {
            let (before, after) = (
                format_value(key, old, &catalog),
                format_value(key, new, &catalog),
            );
            let detail = if before == after {
                format!(" (stored values {old:?} → {new:?})")
            } else {
                String::new()
            };
            changes.push(format!(
                "  {}: {before} → {after}{detail}",
                key.replace('_', " ")
            ));
        }
    }
    if changes.is_empty() {
        changes.push("  No decoded parameter changes.".into());
    }
    Ok(changes)
}
