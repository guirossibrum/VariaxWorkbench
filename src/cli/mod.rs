mod legacy;
mod preset;

use std::{env, ffi::OsString, path::Path};

const HELP: &str = "Usage: variax-workbench <COMMAND>

Workbench commands:
  gui [OPTIONS]                  Launch the desktop app (requires the gui feature)
  preset inspect FILE            Show an offline preset's family, name and metadata
  preset rename INPUT OUTPUT NAME
                                 Save a renamed preset copy to a new file
  preset --help                  Show preset formats and supported families
  help, -h, --help                Show this help

Legacy file commands:
  inspect PRESET.vwm | inspect-bundle BANK.vwb | inspect-workbench-asset FILE.ast
  roundtrip INPUT OUTPUT | rename INPUT OUTPUT NAME
  set INPUT OUTPUT SLOT_HEX PARAM_HEX int|float VALUE
  extract-preset BANK OUTPUT BANK_INDEX PRESET_INDEX
  replace-preset BANK PRESET NEW_BANK BANK_INDEX PRESET_INDEX
  render-preset INPUT.vwm MODELS.tsv NEW.packed [muted]

Legacy offline analysis commands:
  preview, decode, math-profile, audio-capabilities, audio-route-declarations
  host-generate, host-generate-batch, vx-float
  dsp-block, dsp-filter-block, dsp-resonance-block, dsp-eq-block, dsp-body-variant
  dsp-equalizer, dsp-biquad, dsp-response, dsp-geometry, dsp-scalar, dsp-coefficient
  pickup-levels, pickup-place, pickup-project, pickup-tables, pickup-calculated
  pickup-response, pickup-filter1, pickup-filter2, pickup-bandpass
  pickup-biquad-response, combine-pickups
  body-pole-offsets, blend-body, interpolate-body, body-roundtrip, normalize-body, body-plan";

pub(super) fn run() -> Result<(), String> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    match args.first().and_then(|arg| arg.to_str()) {
        Some("help" | "-h" | "--help") if args.len() == 1 => {
            println!("{HELP}");
            #[cfg(feature = "usb")]
            println!("\nLegacy hardware command (USB builds):\n  read-current OUTPUT.vwm");
            Ok(())
        }
        Some("preset") => run_preset(&args[1..]),
        _ => legacy::run(),
    }
}

fn run_preset(args: &[OsString]) -> Result<(), String> {
    match args {
        [help] if help == "--help" || help == "-h" => {
            preset::print_help();
            Ok(())
        }
        [command, input] if command == "inspect" => {
            preset::inspect(Path::new(input))
        }
        [command, input, output, name] if command == "rename" => preset::rename(
            Path::new(input),
            Path::new(output),
            name.to_str().ok_or("preset name must be valid Unicode")?,
        ),
        _ => Err("Usage: variax-workbench preset inspect FILE | preset rename INPUT OUTPUT NAME\nRun 'variax-workbench preset --help' for supported formats.".into()),
    }
}
