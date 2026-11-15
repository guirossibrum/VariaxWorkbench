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
        Some("render-preset") if matches!(args.len(), 4 | 5) => {
            let preset = read_preset(&args[1])?;
            let parameters = variax_workbench::host::HostParameters::from_preset(&preset)?;
            let settings = parameters.map(variax_workbench::host::HostOptions {
                muted: if args.len() == 5 {
                    match args[4].as_str() {
                        "muted" => true,
                        _ => return Err("optional mode must be muted".into()),
                    }
                } else {
                    false
                },
                ..Default::default()
            })?;
            let library = variax_workbench::host::ModelLibrary::load_manifest(
                std::path::Path::new(&args[2]),
            )?;
            let generated =
                variax_workbench::host::HostEngine::new(&library).generate(&settings)?;
            let output = parameters.finish(preset.name(), &settings, generated.packed());
            write_new(&args[3], &output)?;
        }
        Some("host-generate-batch") if args.len() == 4 => {
            let raw = fs::read(&args[1]).map_err(|e| e.to_string())?;
            if raw.is_empty() || !raw.len().is_multiple_of(192) || raw.len() > 192 * 4096 {
                return Err("expected 1..4096 complete 192-byte native input records".into());
            }
            let library = variax_workbench::host::ModelLibrary::load_manifest(
                std::path::Path::new(&args[2]),
            )?;
            let mut engine = variax_workbench::host::HostEngine::new(&library);
            let mut packed = Vec::new();
            for (i, native) in raw.as_chunks::<192>().0.iter().enumerate() {
                let output = engine
                    .generate(&variax_workbench::host::WorkbenchSettings {
                        native: *native,
                        block_mask: u32::MAX,
                    })
                    .map_err(|e| format!("host record {i}: {e}"))?;
                packed.extend_from_slice(&output.packed());
            }
            write_new(&args[3], &packed)?;
        }
        Some("host-generate") if args.len() == 4 => {
            let raw = fs::read(&args[1]).map_err(|e| e.to_string())?;
            let native = raw
                .try_into()
                .map_err(|_| "expected exactly 192 native input bytes")?;
            let library = variax_workbench::host::ModelLibrary::load_manifest(
                std::path::Path::new(&args[2]),
            )?;
            let output = variax_workbench::host::HostEngine::new(&library).generate(
                &variax_workbench::host::WorkbenchSettings {
                    native,
                    block_mask: u32::MAX,
                },
            )?;
            write_new(&args[3], &output.packed())?;
        }
        Some("inspect") if args.len() == 2 => {
            print!("{}", read_preset(&args[1])?.summary());
        }
        Some(
            command @ ("dsp-block" | "dsp-filter-block" | "dsp-resonance-block" | "dsp-eq-block"),
        ) if args.len() == 4 => {
            let mut seed = u32::from_str_radix(&args[1], 16).map_err(|e| e.to_string())?;
            let mut patch = variax_workbench::dsp::NativePatch::default();
            for word in &mut patch.words {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                *word = seed;
            }
            let bytes = fs::read(&args[3]).map_err(|e| e.to_string())?;
            if command == "dsp-block" {
                variax_workbench::dsp::write_simple_analysis(&mut patch, &args[2], &bytes)?;
            } else if command == "dsp-filter-block" {
                variax_workbench::dsp::write_filter_analysis(&mut patch, &args[2], &bytes)?;
            } else if command == "dsp-resonance-block" {
                variax_workbench::dsp::write_resonance_analysis(&mut patch, &args[2], &bytes)?;
            } else {
                variax_workbench::dsp::write_eq_analysis(&mut patch, &args[2], &bytes)?;
            }
            println!(
                "{}",
                patch
                    .words
                    .iter()
                    .map(|v| format!("{v:08x}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
        }
        Some("dsp-body-variant") if args.len() == 4 => {
            let body = variax_workbench::body::BodyDefinition::parse(
                &fs::read(&args[1]).map_err(|e| e.to_string())?,
            )?;
            let index = args[2].parse::<usize>().map_err(|e| e.to_string())?;
            let variant = body
                .variants
                .get(index)
                .ok_or("body variant index exceeds five")?;
            let patch = variax_workbench::dsp::NativePatch::from_body_variant(
                variant,
                variax_workbench::dsp::LEGACY_SAMPLE_RATE,
                Default::default(),
            )?;
            write_new(
                &args[3],
                &patch
                    .words
                    .iter()
                    .flat_map(|v| v.to_le_bytes())
                    .collect::<Vec<_>>(),
            )?;
        }
        Some("dsp-equalizer") if args.len() == 7 => {
            let values = args[1..]
                .iter()
                .map(|v| u32::from_str_radix(v, 16))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            let p = variax_workbench::dsp::EqParameters {
                kind: values[0],
                frequency: f32::from_bits(values[1]),
                q: f32::from_bits(values[2]),
                gain_db: f32::from_bits(values[3]),
                mix: f32::from_bits(values[4]),
            };
            let c = variax_workbench::dsp::equalizer_coefficients(p, f32::from_bits(values[5]))?;
            println!(
                "{}",
                c.iter()
                    .map(|v| format!("{:08x}", v.to_bits()))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
        }
        Some("dsp-biquad") if args.len() == 5 => {
            let values = args[1..]
                .iter()
                .map(|v| u32::from_str_radix(v, 16))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            let output = variax_workbench::dsp::biquad_coefficients(
                values[0],
                f32::from_bits(values[1]),
                f32::from_bits(values[2]),
                f32::from_bits(values[3]),
            )?;
            println!(
                "{}",
                output
                    .iter()
                    .map(|v| format!("{:08x}", v.to_bits()))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
        }
        Some("dsp-response") if args.len() == 3 => {
            let mut seed = u32::from_str_radix(&args[1], 16).map_err(|e| e.to_string())?;
            let (response, controls) = variax_workbench::dsp::response_from_analysis(
                &fs::read(&args[2]).map_err(|e| e.to_string())?,
            )?;
            let mut patch = variax_workbench::dsp::NativePatch::default();
            for word in &mut patch.words {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                *word = seed;
            }
            patch.write_pickup_response(&response, controls)?;
            println!(
                "{}",
                patch
                    .words
                    .iter()
                    .map(|v| format!("{v:08x}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
        }
        Some("dsp-geometry") if args.len() == 3 => {
            let mut seed = u32::from_str_radix(&args[1], 16).map_err(|e| e.to_string())?;
            let input = variax_workbench::pickup::GeometryInput::parse(
                &fs::read(&args[2]).map_err(|e| e.to_string())?,
            )?;
            let mut patch = variax_workbench::dsp::NativePatch::default();
            for word in &mut patch.words {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                *word = seed;
            }
            patch.write_pickup_geometry(&input.project()?)?;
            println!(
                "{}",
                patch
                    .words
                    .iter()
                    .map(|v| format!("{v:08x}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
        }
        Some(command @ ("dsp-scalar" | "dsp-coefficient")) if args.len() >= 3 => {
            let mut seed = u32::from_str_radix(&args[1], 16).map_err(|e| e.to_string())?;
            let mut patch = variax_workbench::dsp::NativePatch::default();
            for word in &mut patch.words {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                *word = seed;
            }
            let values = args[3..]
                .iter()
                .map(|v| u32::from_str_radix(v, 16))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            if command == "dsp-scalar" {
                patch.apply_scalar(&args[2], &values)?;
            } else {
                patch.apply_coefficient(&args[2], &values)?;
            }
            println!(
                "{}",
                patch
                    .words
                    .iter()
                    .map(|v| format!("{v:08x}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
        }
        Some("vx-float") if args.len() == 2 => {
            let bits = u32::from_str_radix(&args[1], 16).map_err(|e| e.to_string())?;
            let result = variax_workbench::dsp::vx_float_words(f32::from_bits(bits));
            println!("{:08x} {:08x}", result[0], result[1]);
        }
        Some("pickup-levels") if args.len() == 9 => {
            use variax_workbench::pickup::{BalancedLevels, PickupLevel};
            let value = |i: usize| -> Result<f32, String> {
                Ok(f32::from_bits(
                    u32::from_str_radix(&args[i], 16).map_err(|e| e.to_string())?,
                ))
            };
            let offset = |i: usize| -> Result<Option<f32>, String> {
                if args[i] == "none" {
                    Ok(None)
                } else {
                    Ok(Some(value(i)?))
                }
            };
            let swap = match args[8].as_str() {
                "0" => false,
                "1" => true,
                _ => return Err("swap must be 0 or 1".into()),
            };
            let r = BalancedLevels::electric(
                PickupLevel {
                    model_offset_db: offset(1)?,
                    level_db: value(3)?,
                },
                PickupLevel {
                    model_offset_db: offset(2)?,
                    level_db: value(4)?,
                },
                offset(5)?,
                value(6)?,
                value(7)?,
                swap,
            )?;
            println!(
                "{:08x} {:08x} {:08x}",
                r.pickup_db[0].to_bits(),
                r.pickup_db[1].to_bits(),
                r.trim_db.to_bits()
            );
        }
        Some("pickup-place") if args.len() == 7 => {
            let pickup = variax_workbench::models::PickupDefinition::parse(
                &fs::read(&args[1]).map_err(|e| e.to_string())?,
            )?;
            let body = variax_workbench::body::BodyDefinition::parse(
                &fs::read(&args[2]).map_err(|e| e.to_string())?,
            )?;
            let f = |i: usize| -> Result<f32, String> {
                Ok(f32::from_bits(
                    u32::from_str_radix(&args[i], 16).map_err(|e| e.to_string())?,
                ))
            };
            let phase = match args[5].as_str() {
                "0" => false,
                "1" => true,
                _ => return Err("phase must be 0 or 1".into()),
            };
            let single = match args[6].as_str() {
                "0" => false,
                "1" => true,
                _ => return Err("single must be 0 or 1".into()),
            };
            let position = f(3)?;
            let offsets = body.pole_offsets_at(body.normalized_pickup_position(position)?)?;
            let placed = pickup.place_coils(offsets, position, f(4)?, phase, single)?;
            let mut words = vec![placed.positions.len() as u32];
            for coil in placed.positions {
                words.extend(coil.map(f32::to_bits));
                words.extend([u32::from(placed.reverse_phase); 6]);
            }
            println!(
                "{}",
                words
                    .iter()
                    .map(|v| format!("{v:08x}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
        }
        Some("pickup-project") if args.len() == 2 => {
            let input = variax_workbench::pickup::GeometryInput::parse(
                &fs::read(&args[1]).map_err(|e| e.to_string())?,
            )?;
            let projection = input.project()?;
            println!(
                "{}",
                projection
                    .words()
                    .iter()
                    .map(|v| format!("{v:08x}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
        }
        Some("body-pole-offsets") if args.len() == 3 => {
            let body = variax_workbench::body::BodyDefinition::parse(
                &fs::read(&args[1]).map_err(|e| e.to_string())?,
            )?;
            let position =
                f32::from_bits(u32::from_str_radix(&args[2], 16).map_err(|e| e.to_string())?);
            println!(
                "{}",
                body.pole_offsets_at(position)?
                    .iter()
                    .map(|v| format!("{:08x}", v.to_bits()))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
        }
        Some(command @ ("pickup-tables" | "pickup-calculated"))
            if (command == "pickup-tables" && (6..=7).contains(&args.len()))
                || (command == "pickup-calculated" && (7..=8).contains(&args.len())) =>
        {
            use variax_workbench::{
                models::{PickupCircuit, PickupDefinition, PickupWiring},
                pickup::{TableSettings, VolumeTaper, WiringProperties},
            };
            let left = PickupCircuit::from(&PickupDefinition::parse(
                &fs::read(&args[1]).map_err(|e| e.to_string())?,
            )?);
            let right = PickupCircuit::from(&PickupDefinition::parse(
                &fs::read(&args[2]).map_err(|e| e.to_string())?,
            )?);
            let wiring_mode = match args[3].as_str() {
                "series" => PickupWiring::Series,
                "parallel" => PickupWiring::Parallel,
                _ => return Err("expected series or parallel".into()),
            };
            let taper = match args[4].as_str() {
                "linear" => VolumeTaper::Linear,
                "log" => VolumeTaper::Logarithmic,
                _ => return Err("expected linear or log taper".into()),
            };
            let sample_rate =
                f32::from_bits(u32::from_str_radix(&args[5], 16).map_err(|e| e.to_string())?);
            let wiring = WiringProperties {
                load_capacitance: 3.5e-10,
                load_resistance: 1e6,
                tone_resistance: 500000.,
                volume_resistance: 500000.,
                tone_capacitance: 2.2e-8,
            };
            let extra = if command == "pickup-calculated" { 7 } else { 6 };
            let wiring = if let Some(path) = args.get(extra) {
                let raw = fs::read(path).map_err(|e| e.to_string())?;
                if raw.len() != 40 {
                    return Err("expected 40-byte little-endian wiring analysis input".into());
                }
                let value = |i| f64::from_le_bytes(raw[i..i + 8].try_into().unwrap());
                WiringProperties {
                    load_capacitance: value(0),
                    load_resistance: value(8),
                    tone_resistance: value(16),
                    volume_resistance: value(24),
                    tone_capacitance: value(32),
                }
            } else {
                wiring
            };
            if command == "pickup-calculated" {
                let mut wiring = wiring;
                wiring.tone_capacitance =
                    f64::from_bits(u64::from_str_radix(&args[6], 16).map_err(|e| e.to_string())?);
                match left.calculated_response(
                    right,
                    wiring,
                    TableSettings {
                        sample_rate,
                        wiring: wiring_mode,
                        taper,
                    },
                )? {
                    Some(response) => println!(
                        "{}",
                        response
                            .fields()
                            .iter()
                            .map(|(offset, bits)| format!("{offset:x}:{bits:08x}"))
                            .collect::<Vec<_>>()
                            .join(" ")
                    ),
                    None => println!("inactive"),
                }
                return Ok(());
            }
            match left.response_tables(
                right,
                wiring,
                TableSettings {
                    sample_rate,
                    wiring: wiring_mode,
                    taper,
                },
            )? {
                Some(table) => println!(
                    "{}",
                    table
                        .0
                        .iter()
                        .map(|v| format!("{:016x}", v.to_bits()))
                        .collect::<Vec<_>>()
                        .join(" ")
                ),
                None => println!("inactive"),
            }
        }
        Some("pickup-filter2") if args.len() == 9 => {
            let definition = variax_workbench::models::PickupDefinition::parse(
                &fs::read(&args[1]).map_err(|e| e.to_string())?,
            )?;
            let rate =
                f32::from_bits(u32::from_str_radix(&args[2], 16).map_err(|e| e.to_string())?);
            let values = args[3..]
                .iter()
                .map(|v| u64::from_str_radix(v, 16).map(f64::from_bits))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            let primary = variax_workbench::pickup::FilterParameters {
                frequency: values[3],
                q: values[4],
                gain: values[5],
            };
            let settings = variax_workbench::pickup::SecondaryFit {
                sample_rate: rate,
                volume: values[0],
                tone: values[1],
                frequency_limit: values[2],
            };
            let wiring = variax_workbench::pickup::WiringProperties {
                load_capacitance: 3.5e-10,
                load_resistance: 1e6,
                tone_resistance: 500000.,
                volume_resistance: 500000.,
                tone_capacitance: 2.2e-8,
            };
            let result = variax_workbench::models::PickupCircuit::from(&definition)
                .fit_secondary_filter(wiring, primary, settings)?;
            println!(
                "{:016x} {:016x} {:016x}",
                result.frequency.to_bits(),
                result.q.to_bits(),
                result.gain.to_bits()
            );
        }
        Some("pickup-bandpass") if args.len() == 5 => {
            let rate =
                f32::from_bits(u32::from_str_radix(&args[1], 16).map_err(|e| e.to_string())?);
            let values = args[2..]
                .iter()
                .map(|v| u64::from_str_radix(v, 16).map(f64::from_bits))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            let result = variax_workbench::pickup::Biquad::bandpass(
                rate,
                variax_workbench::pickup::FilterParameters {
                    frequency: values[0],
                    q: values[1],
                    gain: values[2],
                },
            );
            println!(
                "{}",
                result
                    .0
                    .iter()
                    .map(|v| format!("{:016x}", v.to_bits()))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
        }
        Some("pickup-biquad-response") if args.len() == 8 => {
            let rate =
                f32::from_bits(u32::from_str_radix(&args[1], 16).map_err(|e| e.to_string())?);
            let values = args[2..]
                .iter()
                .map(|v| u64::from_str_radix(v, 16).map(f64::from_bits))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            let coefficients: [f64; 5] = values[1..]
                .try_into()
                .map_err(|e: std::array::TryFromSliceError| e.to_string())?;
            let response = variax_workbench::pickup::Biquad(coefficients).response(rate, values[0]);
            println!(
                "{:016x} {:016x}",
                response.real.to_bits(),
                response.imaginary.to_bits()
            );
        }
        Some(command @ ("pickup-response" | "pickup-filter1"))
            if (command == "pickup-response" && args.len() == 5)
                || (command == "pickup-filter1" && args.len() == 4) =>
        {
            use variax_workbench::{
                models::{PickupCircuit, PickupDefinition},
                pickup::WiringProperties,
            };
            let pickup = PickupCircuit::from(&PickupDefinition::parse(
                &fs::read(&args[1]).map_err(|e| e.to_string())?,
            )?);
            let value = |index: usize| -> Result<f64, String> {
                Ok(f64::from_bits(
                    u64::from_str_radix(&args[index], 16).map_err(|_| "expected float64 bits")?,
                ))
            };
            let wiring = WiringProperties {
                load_capacitance: 3.5e-10,
                load_resistance: 1e6,
                tone_resistance: 500000.0,
                volume_resistance: 500000.0,
                tone_capacitance: 2.2e-8,
            };
            if command == "pickup-filter1" {
                let filter = pickup.fit_primary_filter(wiring, value(2)?, value(3)?)?;
                println!(
                    "{:016x} {:016x} {:016x}",
                    filter.frequency.to_bits(),
                    filter.q.to_bits(),
                    filter.gain.to_bits()
                );
            } else {
                let response = pickup.response(wiring, value(2)?, value(3)?, value(4)?);
                println!(
                    "{:016x} {:016x}",
                    response.real.to_bits(),
                    response.imaginary.to_bits()
                );
            }
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
                "Usage: variax-workbench preview | decode FIRST.syx SECOND.syx | inspect PRESET.vwm | render-preset INPUT.vwm MODELS.tsv NEW.packed [muted] | pickup-response INPUT.bin FREQ_BITS VOL_BITS TONE_BITS | pickup-filter1 INPUT.bin VOL_BITS TONE_BITS | combine-pickups LEFT.bin RIGHT.bin series|parallel | interpolate-body INPUT.bin OUTPUT.bin X_BITS_HEX Y_BITS_HEX | blend-body INPUT.bin OUTPUT.bin LEFT_INDEX RIGHT_INDEX WEIGHT_BITS_HEX | body-roundtrip INPUT.bin OUTPUT.bin | normalize-body INPUT.bin POSITION_BITS_HEX | body-plan MODE X_BITS_HEX Y_BITS_HEX | inspect-bundle BANK.vwb | extract-preset BANK.vwb OUTPUT.vwm BANK_INDEX PRESET_INDEX | roundtrip INPUT OUTPUT | rename INPUT OUTPUT NAME | set INPUT OUTPUT SLOT_HEX PARAM_HEX int|float VALUE\nAll commands are offline. Output files must not exist. Parameter values are not checked against model-specific ranges.".into(),
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
