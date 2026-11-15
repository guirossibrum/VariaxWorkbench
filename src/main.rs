use std::{env, fs, path::Path, process::ExitCode};
use variax_workbench::preset::PresetFile;
use variax_workbench::{PODXT_ROUTING_ENABLE, PresetMetadata, metadata_requests, midi_enable};

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn read_preset(path: &str) -> Result<PresetFile, String> {
    PresetFile::parse(fs::read(path).map_err(|e| format!("{path}: {e}"))?)
}

fn write_new(path: &str, bytes: &[u8]) -> Result<(), String> {
    variax_workbench::document::save_new(Path::new(path), bytes)
}

fn parse_id(value: &str) -> Result<u32, String> {
    u32::from_str_radix(value.strip_prefix("0x").unwrap_or(value), 16)
        .map_err(|_| format!("invalid hexadecimal ID: {value}"))
}

fn run() -> Result<(), String> {
    let args: Vec<_> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("preview") if args.len() == 1 => {
            println!("Offline byte preview; no device opened. Live access is locked.");
            println!("PODxt routing enable: {}", hex(&PODXT_ROUTING_ENABLE));
            println!("Variax MIDI enable:   {}", hex(&midi_enable()));
            for (i, request) in metadata_requests().iter().enumerate() {
                println!("Metadata block {}:    {}", i + 1, hex(request));
            }
        }
        Some("decode") if args.len() == 3 => {
            let first = fs::read(&args[1]).map_err(|e| format!("{}: {e}", args[1]))?;
            let second = fs::read(&args[2]).map_err(|e| format!("{}: {e}", args[2]))?;
            println!("{:#?}", PresetMetadata::from_replies(&first, &second)?);
        }
        #[cfg(feature = "usb")]
        Some("read-current") if args.len() == 2 => {
            let current =
                variax_workbench::usb::read_current(&std::sync::atomic::AtomicBool::new(false))?;
            let preset = PresetFile::from_metadata(
                &current.metadata,
                current.identity.product_id(),
                current.identity.version.unwrap_or(0),
            )?;
            write_new(&args[1], preset.as_bytes())?;
            println!(
                "Read Variax {} firmware {} current preset {:?}; saved model/settings file to {}. No preset switched or written to the guitar.",
                current.identity.model,
                current.identity.version_string(),
                preset.name(),
                args[1]
            );
        }
        Some("inspect") if args.len() == 2 => {
            print!("{}", read_preset(&args[1])?.summary());
        }
        Some("combine-pickups") if args.len() == 4 => {
            use variax_workbench::models::{PickupCircuit, PickupDefinition, PickupWiring};
            let load = |path: &str| -> Result<PickupCircuit, String> {
                Ok(PickupCircuit::from(&PickupDefinition::parse(
                    &fs::read(path).map_err(|e| e.to_string())?,
                )?))
            };
            let wiring = match args[3].as_str() {
                "series" => PickupWiring::Series,
                "parallel" => PickupWiring::Parallel,
                _ => return Err("expected series or parallel".into()),
            };
            let result = load(&args[1])?.combine(load(&args[2])?, wiring);
            let c = result.circuit;
            println!(
                "{} {:016x} {:016x} {:016x} {:016x}",
                u8::from(result.active),
                c.dc_resistance.to_bits(),
                c.magnetic_resistance.to_bits(),
                c.inductance.to_bits(),
                c.capacitance.to_bits()
            );
        }
        Some("blend-body") if args.len() == 6 => {
            let body = variax_workbench::body::BodyDefinition::parse(
                &fs::read(&args[1]).map_err(|e| e.to_string())?,
            )?;
            let first = args[3]
                .parse::<usize>()
                .map_err(|_| "expected variant index")?;
            let second = args[4]
                .parse::<usize>()
                .map_err(|_| "expected variant index")?;
            let left = body
                .variants
                .get(first)
                .ok_or("body variant index out of range")?;
            let right = body
                .variants
                .get(second)
                .ok_or("body variant index out of range")?;
            let result = left.interpolate_with(right, f32::from_bits(parse_id(&args[5])?))?;
            write_new(&args[2], &result.canonical_bytes())?;
        }
        Some("interpolate-body") if args.len() == 5 => {
            let body = variax_workbench::body::BodyDefinition::parse(
                &fs::read(&args[1]).map_err(|e| e.to_string())?,
            )?;
            let variant = body.interpolate_normalized(
                f32::from_bits(parse_id(&args[3])?),
                f32::from_bits(parse_id(&args[4])?),
            )?;
            write_new(&args[2], &variant.canonical_bytes())?;
        }
        Some("body-roundtrip") if args.len() == 3 => {
            let body = variax_workbench::body::BodyDefinition::parse(
                &fs::read(&args[1]).map_err(|e| e.to_string())?,
            )?;
            write_new(&args[2], &body.canonical_bytes())?;
        }
        Some("normalize-body") if args.len() == 3 => {
            let body = variax_workbench::body::BodyDefinition::parse(
                &fs::read(&args[1]).map_err(|e| e.to_string())?,
            )?;
            let normalized =
                body.normalized_pickup_position(f32::from_bits(parse_id(&args[2])?))?;
            println!("{:08x}", normalized.to_bits());
        }
        Some("body-plan") if args.len() == 4 => {
            let mode = args[1]
                .parse::<u32>()
                .map_err(|_| "expected numeric interpolation mode")?;
            let plan = variax_workbench::body::BodyBlendPlan::from_normalized(
                mode,
                f32::from_bits(parse_id(&args[2])?),
                f32::from_bits(parse_id(&args[3])?),
            )?;
            println!("{plan}");
        }
        Some("inspect-bundle") if args.len() == 2 => {
            let bundle = variax_workbench::bundle::BundleFile::parse(
                fs::read(&args[1]).map_err(|e| e.to_string())?,
            )?;
            print!("{}", bundle.summary());
        }
        Some("extract-preset") if args.len() == 5 => {
            let bundle = variax_workbench::bundle::BundleFile::parse(
                fs::read(&args[1]).map_err(|e| e.to_string())?,
            )?;
            let bank = args[3]
                .parse::<usize>()
                .map_err(|_| "expected bank index")?;
            let preset = args[4]
                .parse::<usize>()
                .map_err(|_| "expected preset index")?;
            let preset = bundle
                .banks()
                .get(bank)
                .and_then(|b| b.presets().get(preset))
                .ok_or("bank/preset index is out of range")?;
            write_new(&args[2], preset.as_bytes())?;
        }
        Some("roundtrip") if args.len() == 3 => {
            let preset = read_preset(&args[1])?;
            write_new(&args[2], preset.as_bytes())?;
        }
        Some("rename") if args.len() == 4 => {
            let mut preset = read_preset(&args[1])?;
            preset.rename(&args[3])?;
            write_new(&args[2], preset.as_bytes())?;
        }
        Some("set") if args.len() == 7 => {
            let mut preset = read_preset(&args[1])?;
            let (kind, bits) = match args[5].as_str() {
                "int" => (
                    0,
                    args[6]
                        .parse::<i32>()
                        .map_err(|_| "expected a signed 32-bit integer")?
                        as u32,
                ),
                "float" => {
                    let value = args[6]
                        .parse::<f32>()
                        .map_err(|_| "expected a finite float")?;
                    if !value.is_finite() {
                        return Err("expected a finite float".into());
                    }
                    (1, value.to_bits())
                }
                _ => return Err("parameter type must be int or float".into()),
            };
            preset.set_parameter(parse_id(&args[3])?, parse_id(&args[4])?, kind, bits)?;
            write_new(&args[2], preset.as_bytes())?;
        }
        _ => {
            return Err(
                "Usage: variax-workbench preview | decode FIRST.syx SECOND.syx | inspect PRESET.vwm | combine-pickups LEFT.bin RIGHT.bin series|parallel | interpolate-body INPUT.bin OUTPUT.bin X_BITS_HEX Y_BITS_HEX | blend-body INPUT.bin OUTPUT.bin LEFT_INDEX RIGHT_INDEX WEIGHT_BITS_HEX | body-roundtrip INPUT.bin OUTPUT.bin | normalize-body INPUT.bin POSITION_BITS_HEX | body-plan MODE X_BITS_HEX Y_BITS_HEX | inspect-bundle BANK.vwb | extract-preset BANK.vwb OUTPUT.vwm BANK_INDEX PRESET_INDEX | roundtrip INPUT OUTPUT | rename INPUT OUTPUT NAME | set INPUT OUTPUT SLOT_HEX PARAM_HEX int|float VALUE\nAll commands are offline. Output files must not exist. Parameter values are not checked against model-specific ranges.".into(),
            );
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
