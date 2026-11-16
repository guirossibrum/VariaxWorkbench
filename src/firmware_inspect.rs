#![forbid(unsafe_code)]
//! Offline FORM/L6FF inspector.
//!
//! Reads one regular file, or runs the synthetic rejection suite. There is no
//! install, flash, reset, or device-access command.

use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;
use variax_workbench::firmware;
use variax_workbench::firmware::MAX_CONTAINER_BYTES;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        return Err(usage());
    }
    if args.iter().any(|arg| arg == "--self-test") {
        if args.len() != 1 {
            return Err("self-test takes no other arguments\n".to_owned() + &usage());
        }
        let counts = firmware::synthetic::run_rejection_suite()?;
        print!("{counts}");
        return Ok(());
    }
    let mut json = false;
    let mut reference_dir = None;
    let mut expect_product = None;
    let mut expect_version = None;
    let mut file = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => json = true,
            "--variax-reference" => {
                index += 1;
                reference_dir = Some(
                    args.get(index)
                        .ok_or("missing directory after --variax-reference")?
                        .clone(),
                );
            }
            "--expect-product" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or("missing value after --expect-product")?;
                expect_product = Some(parse_u32_hex(value)?);
            }
            "--expect-version" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or("missing value after --expect-version")?;
                expect_version = Some(parse_u32_hex(value)?);
            }
            "--help" | "-h" => {
                println!("{}", usage());
                return Ok(());
            }
            other if other.starts_with('-') => {
                return Err(format!("unknown argument {other}\n{}", usage()));
            }
            other => {
                if file.is_some() {
                    return Err("inspect exactly one file\n".to_owned() + &usage());
                }
                file = Some(other.to_owned());
            }
        }
        index += 1;
    }
    let path = file.ok_or_else(|| "missing file\n".to_owned() + &usage())?;
    match (expect_product, expect_version) {
        (None, None) | (Some(_), Some(_)) => {}
        _ => {
            return Err(
                "pass both --expect-product and --expect-version, or neither\n".to_owned()
                    + &usage(),
            );
        }
    }
    let bytes = read_regular_file(Path::new(&path))?;
    let inspection = firmware::inspect(&bytes).map_err(|error| error.to_string())?;
    let rendered = if json {
        inspection.to_json()
    } else {
        inspection.to_text()
    };
    if let (Some(product), Some(version)) = (expect_product, expect_version)
        && let Err(error) = inspection.require_target(product, version)
    {
        print!("{rendered}");
        return Err(error.to_string());
    }
    if let Some(directory) = reference_dir {
        let reference = variax_workbench::factory_reference::FactoryReference::parse(&bytes)?;
        reference.export_new(Path::new(&directory))?;
        eprintln!(
            "Saved local firmware-default reference to {directory}; not a current-unit/calibration backup or restoration procedure"
        );
    }
    print!("{rendered}");
    Ok(())
}

fn read_regular_file(path: &Path) -> Result<Vec<u8>, String> {
    let metadata =
        fs::metadata(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!(
            "{} is not a regular file; device nodes are not opened",
            path.display()
        ));
    }
    if metadata.len() > MAX_CONTAINER_BYTES as u64 {
        return Err(format!(
            "{} is {} bytes and the container limit is {MAX_CONTAINER_BYTES}",
            path.display(),
            metadata.len()
        ));
    }
    fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))
}

fn parse_u32_hex(text: &str) -> Result<u32, String> {
    let text = text
        .strip_prefix("0x")
        .or_else(|| text.strip_prefix("0X"))
        .unwrap_or(text);
    if text.is_empty()
        || text.len() > 8
        || !text.chars().all(|character| character.is_ascii_hexdigit())
    {
        return Err(format!("expected a hex u32, got {text}"));
    }
    u32::from_str_radix(text, 16).map_err(|_| format!("expected a hex u32, got {text}"))
}

fn usage() -> String {
    "Offline FORM/L6FF firmware container inspector.\n\
Reads one regular file and prints an integrity report.\n\
Does not write firmware, open a device, or prove authenticity.\n\
The reference option exports only the fingerprinted Variax 3.10 archive to a new local directory.\n\
\n\
Usage:\n  \
variax-firmware-inspect [--json] [--expect-product HEX --expect-version HEX] [--variax-reference NEW_DIRECTORY] FILE\n  \
variax-firmware-inspect --self-test\n"
        .to_owned()
}
