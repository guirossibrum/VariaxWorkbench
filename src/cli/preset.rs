use std::{io::Read, path::Path};
use variax_workbench::{
    document::save_new,
    firmware::{VARIAX_300_600_PRODUCT_ID, VARIAX_500_700_PRODUCT_ID},
    open_regular_file, path_facts,
    pod_preset::{PodTone, TONE_BYTES_CEILING, ToneError},
    preset::PresetFile,
    rig::ProductProfile,
};

enum OfflinePreset {
    Variax(PresetFile),
    PodxtLive(PodTone),
}

impl OfflinePreset {
    fn read(path: &Path) -> Result<Self, String> {
        let with_path = |detail: &str| format!("{}: {detail}", path.display());
        let facts = path_facts(path).map_err(|error| with_path(&error.to_string()))?;
        if !facts.regular_file {
            return Err(with_path("input must be a regular file"));
        }
        let too_large = || with_path("preset exceeds the 1 MiB offline file limit");
        if facts.len > TONE_BYTES_CEILING {
            return Err(too_large());
        }
        let mut bytes = Vec::new();
        open_regular_file(path)
            .map_err(|error| with_path(&error.to_string()))?
            .take(TONE_BYTES_CEILING + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| with_path(&error.to_string()))?;
        if bytes.len() as u64 > TONE_BYTES_CEILING {
            return Err(too_large());
        }
        Self::parse(bytes).map_err(|detail| with_path(&detail))
    }

    fn parse(bytes: Vec<u8>) -> Result<Self, String> {
        // The containers are shared. PodTone validates the framing and unique
        // PINF before reporting the target; only known Variax IDs allow fallback.
        match PodTone::parse(bytes.clone()) {
            Ok(tone) => Ok(Self::PodxtLive(tone)),
            Err(ToneError::WrongTarget {
                device_id: VARIAX_300_600_PRODUCT_ID | VARIAX_500_700_PRODUCT_ID,
            }) => PresetFile::parse(bytes).map(Self::Variax),
            Err(ToneError::WrongTarget { device_id }) => {
                Err(format!("unsupported preset target {device_id:08x}"))
            }
            Err(error) => Err(error.detail()),
        }
    }
}

pub(super) fn print_help() {
    println!(
        "Usage:\n  variax-workbench preset inspect FILE\n  variax-workbench preset rename INPUT OUTPUT NAME\n\n\
         Supported files use FORM/L6PA, FORM/L6FS or FORM/L6AS with a validated preset target.\n\
         Inputs must be regular files, up to 1 MiB. Rename writes to a new path.\n\n\
         Offline preset commands by product:"
    );
    for profile in ProductProfile::ALL {
        let support = match profile {
            ProductProfile::Variax600 => "inspect, rename (also Variax 300/500/700)",
            ProductProfile::PodxtLive => "inspect, rename",
            ProductProfile::WorkbenchHd | ProductProfile::Jtv69 | ProductProfile::PodHd500x => {
                "unavailable for this product"
            }
        };
        println!("  {:<25} {support}", profile.label());
    }
}

pub(super) fn inspect(path: &Path) -> Result<(), String> {
    let preset = OfflinePreset::read(path)?;
    let (family, name, container, device, version, models) = match &preset {
        OfflinePreset::Variax(preset) => {
            let bytes = preset.as_bytes();
            let tag = std::str::from_utf8(&bytes[8..12]).expect("validated FORM tag");
            (
                "Variax Workbench",
                preset.name(),
                tag,
                preset.device_id(),
                preset.device_version(),
                preset.models().len(),
            )
        }
        OfflinePreset::PodxtLive(tone) => (
            ProductProfile::PodxtLive.label(),
            tone.name(),
            tone.container().tag(),
            tone.device_id(),
            tone.device_version(),
            tone.models().len(),
        ),
    };
    println!(
        "family\t{family}\nname\t{name:?}\nformat\tFORM/{container}\ndevice\t{device:08x}\nversion\t{version:08x}\nmodels\t{models}"
    );
    Ok(())
}

pub(super) fn rename(input: &Path, output: &Path, name: &str) -> Result<(), String> {
    match OfflinePreset::read(input)? {
        OfflinePreset::Variax(mut preset) => {
            preset.rename(name)?;
            save_new(output, preset.as_bytes())
        }
        OfflinePreset::PodxtLive(mut tone) => {
            if name != tone.name() {
                tone.rename(name).map_err(|error| error.detail())?;
            }
            tone.save_copy(output).map_err(|error| error.detail())
        }
    }
}
