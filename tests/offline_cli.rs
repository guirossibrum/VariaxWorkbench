use std::{
    ffi::OsStr,
    fs,
    path::Path,
    process::{Command, Output},
};

const VARIAX: &[u8] = include_bytes!("fixtures/editor-preset.vwm");

fn cli<I, S>(args: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Command::new(env!("CARGO_BIN_EXE_variax-workbench"))
        .args(args)
        .output()
        .unwrap()
}

fn success(output: Output) -> String {
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    String::from_utf8(output.stdout).unwrap()
}

fn failure(output: Output, message: &str) {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains(message), "{stderr}");
}

fn inspect(path: &Path) -> Output {
    cli(["preset".as_ref(), "inspect".as_ref(), path.as_os_str()])
}

fn rename(input: &Path, output: &Path, name: &str) -> Output {
    cli([
        "preset".as_ref(),
        "rename".as_ref(),
        input.as_os_str(),
        output.as_os_str(),
        name.as_ref(),
    ])
}

fn chunk(tag: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut bytes = tag.to_vec();
    bytes.extend((data.len() as u32).to_be_bytes());
    bytes.extend(data);
    if !data.len().is_multiple_of(2) {
        bytes.push(0xa5);
    }
    bytes
}

fn podxt() -> Vec<u8> {
    // Synthetic tone, including opaque PINF tail, PARM and odd chunk padding.
    let mut pinf = [0; 80];
    pinf[..4].copy_from_slice(&1u32.to_be_bytes());
    pinf[4..8].copy_from_slice(&0x0003_000au32.to_be_bytes());
    pinf[8..14].copy_from_slice(&[0, b'P', 0, b'O', 0, b'D']);
    pinf[16..18].copy_from_slice(&[0xa7, 0xcc]);
    pinf[72..].copy_from_slice(&[3, 1, 0, 0, 4, 5, 6, 7]);
    let mut model = b"MODL".to_vec();
    model.extend(chunk(b"MINF", &[0, 3, 0, 7, 0, 3, 0, 0, 1, 9, 8, 1]));
    model.extend(chunk(b"PARM", &[1, 2, 3, 4, 5]));
    let mut patch = b"PATC".to_vec();
    patch.extend(chunk(b"LIST", &model));
    let mut form = b"L6AS".to_vec();
    form.extend(chunk(b"JUNK", &[0xf1, 0xf2, 0xf3]));
    form.extend(chunk(b"PINF", &pinf));
    form.extend(chunk(b"LIST", &patch));
    chunk(b"FORM", &form)
}

fn append_form_chunk(bytes: &mut Vec<u8>, raw_chunk: &[u8]) {
    bytes.extend(raw_chunk);
    let len = (bytes.len() - 8) as u32;
    bytes[4..8].copy_from_slice(&len.to_be_bytes());
}

#[test]
fn structured_help_describes_offline_commands_and_build_capabilities() {
    let help = success(cli(["--help"]));
    assert_eq!(help, success(cli(["-h"])));
    assert_eq!(help, success(cli(["help"])));
    for command in [
        "gui [OPTIONS]",
        "preset inspect",
        "preset rename",
        "preview",
    ] {
        assert!(help.contains(command), "{help}");
    }
    assert_eq!(help.contains("read-current"), cfg!(feature = "usb"));
    let presets = success(cli(["preset", "--help"]));
    assert!(presets.contains("FORM/L6PA, FORM/L6FS or FORM/L6AS"));
    assert!(presets.contains("Variax 600                inspect, rename"));
    assert!(presets.contains("PODxt Live                inspect, rename"));
    assert!(presets.contains("POD HD500X                unavailable"));
    failure(cli(["preset", "inspect"]), "preset inspect FILE");
}

#[test]
fn gui_dispatch_preserves_launcher_help_version_and_argument_errors() {
    #[cfg(feature = "gui")]
    {
        let direct_gui = |arg| {
            Command::new(env!("CARGO_BIN_EXE_variax-workbench-gui"))
                .arg(arg)
                .output()
                .unwrap()
        };
        for arg in ["--help", "--version", "--preset"] {
            let direct = direct_gui(arg);
            let nested = cli(["gui", arg]);
            assert_eq!(direct.status, nested.status);
            assert_eq!(direct.stdout, nested.stdout);
            assert_eq!(direct.stderr, nested.stderr);
            if arg == "--preset" {
                failure(nested, "--preset requires a value");
            } else {
                assert!(!success(nested).is_empty());
            }
        }
    }
    #[cfg(not(feature = "gui"))]
    failure(cli(["gui", "--help"]), "GUI unavailable in this build");
}

#[test]
fn inspect_and_rename_by_content_preserve_every_byte_outside_the_name() {
    let dir = tempfile::tempdir().unwrap();
    let mut variax = VARIAX.to_vec();
    append_form_chunk(&mut variax, &chunk(b"JUNK", &[0xf1, 0xf2, 0xf3]));
    for (input_file, bytes, name_at, family, kind, device, version, old, model_count, new) in [
        (
            "guitar.l6t",
            variax,
            28,
            "Variax Workbench",
            "L6PA",
            "00070004",
            "00000000",
            "Synthetic",
            0,
            "New Guitar",
        ),
        (
            "pod.vwm",
            podxt(),
            40,
            "PODxt Live",
            "L6AS",
            "0003000a",
            "03010000",
            "POD",
            1,
            "POD 🎸",
        ),
    ] {
        let input = dir.path().join(input_file);
        let output = dir.path().join(format!("{input_file}.renamed"));
        fs::write(&input, &bytes).unwrap();
        let summary = |name: &str| {
            format!(
                "family\t{family}\nname\t{name:?}\nformat\tFORM/{kind}\ndevice\t{device}\nversion\t{version}\nmodels\t{model_count}\n"
            )
        };
        assert_eq!(success(inspect(&input)), summary(old));
        assert!(success(rename(&input, &output, new)).is_empty());
        assert_eq!(fs::read(&input).unwrap(), bytes);
        let mut expected = bytes.clone();
        expected[name_at..name_at + 64].fill(0);
        for (word, dest) in new
            .encode_utf16()
            .zip(expected[name_at..name_at + 64].as_chunks_mut::<2>().0)
        {
            dest.copy_from_slice(&word.to_be_bytes());
        }
        assert_eq!(fs::read(&output).unwrap(), expected);
        assert_eq!(success(inspect(&output)), summary(new));

        let unchanged = dir.path().join(format!("{input_file}.unchanged"));
        success(rename(&input, &unchanged, old));
        assert_eq!(fs::read(&unchanged).unwrap(), bytes);
    }
}

#[test]
fn legacy_electric_targets_and_shared_form_kinds_are_supported() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("preset");
    let mut variax_500_700 = VARIAX.to_vec();
    variax_500_700[24..28].copy_from_slice(&0x0007_0000u32.to_be_bytes());
    for (mut bytes, device) in [
        (VARIAX.to_vec(), "00070004"),
        (variax_500_700, "00070000"),
        (podxt(), "0003000a"),
    ] {
        for kind in [b"L6PA", b"L6FS", b"L6AS"] {
            bytes[8..12].copy_from_slice(kind);
            fs::write(&input, &bytes).unwrap();
            let summary = success(inspect(&input));
            let kind = std::str::from_utf8(kind).unwrap();
            assert!(summary.contains(&format!("format\tFORM/{kind}\ndevice\t{device}\n")));
        }
    }
}

#[test]
fn conflicting_target_metadata_and_patch_containers_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("ambiguous.vwm");
    let output = dir.path().join("copy.vwm");
    let pod = podxt();
    let mut conflicting_pinf = VARIAX.to_vec();
    append_form_chunk(&mut conflicting_pinf, &pod[24..112]);
    let duplicate = chunk(b"LIST", b"PATC");
    let mut duplicate_pod_patch = pod;
    append_form_chunk(&mut duplicate_pod_patch, &duplicate);
    let mut duplicate_variax_patch = VARIAX.to_vec();
    append_form_chunk(&mut duplicate_variax_patch, &duplicate);
    for (bytes, message) in [
        (conflicting_pinf, "ambiguous duplicate PINF"),
        (duplicate_pod_patch, "expected exactly one PATC container"),
        (
            duplicate_variax_patch,
            "expected exactly one PATC container",
        ),
    ] {
        fs::write(&input, bytes).unwrap();
        failure(inspect(&input), message);
        failure(rename(&input, &output, "Renamed"), message);
        assert!(!output.exists());
    }
}

#[test]
fn malformed_unsupported_and_unbounded_inputs_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("test.vwm");
    let output = dir.path().join("copy");
    let mut wrong_target = VARIAX.to_vec();
    wrong_target[24..28].copy_from_slice(&0x0007_0001u32.to_be_bytes());
    let mut malformed_pod = podxt();
    malformed_pod.pop();
    let mut malformed_variax = VARIAX.to_vec();
    malformed_variax[12..16].copy_from_slice(b"JUNK");
    for (bytes, message) in [
        (
            wrong_target.as_slice(),
            "unsupported preset target 00070001",
        ),
        (&malformed_pod, "FORM length does not match"),
        (&malformed_variax, "missing PINF chunk"),
        (
            include_bytes!("fixtures/synthetic-bank.vwb"),
            "expected FORM/L6PA",
        ),
        (
            include_bytes!("fixtures/type12-source.h5ep"),
            "expected an individual FORM tone",
        ),
        (b"", "expected an individual FORM tone"),
    ] {
        fs::write(&input, bytes).unwrap();
        failure(inspect(&input), message);
        failure(rename(&input, &output, "Renamed"), message);
        assert!(!output.exists());
    }
    fs::File::create(&input)
        .unwrap()
        .set_len(1_048_577)
        .unwrap();
    failure(inspect(&input), "exceeds the 1 MiB");
    failure(inspect(dir.path()), "input must be a regular file");
}

#[test]
fn rename_checks_family_name_limits_and_output_collisions() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("original");
    let existing = dir.path().join("existing");
    let missing = dir.path().join("new");
    fs::write(&existing, b"keep me").unwrap();
    for (original, invalid, message) in [
        (VARIAX.to_vec(), "🎸".to_owned(), "at most 18"),
        (podxt(), "🎸".repeat(17), "at most 32 UTF-16 code units"),
    ] {
        fs::write(&input, &original).unwrap();
        failure(rename(&input, &missing, &invalid), message);
        assert!(!missing.exists());
        failure(rename(&input, &existing, "Renamed"), "existing");
        failure(rename(&input, &input, "Renamed"), "original");
        assert_eq!(fs::read(&existing).unwrap(), b"keep me");
        assert_eq!(fs::read(&input).unwrap(), original);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }
}

#[test]
fn legacy_commands_keep_output_exit_status_and_saved_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("original.vwm");
    let output = dir.path().join("renamed.vwm");
    let new_output = dir.path().join("new.vwm");
    fs::write(&input, VARIAX).unwrap();
    assert_eq!(
        success(cli(["inspect".as_ref(), input.as_os_str()])),
        "name\tSynthetic\ndevice\t00070004\nversion\t00000000\n"
    );
    assert!(
        success(cli([
            "rename".as_ref(),
            input.as_os_str(),
            output.as_os_str(),
            "Copy".as_ref()
        ]))
        .is_empty()
    );
    success(rename(&input, &new_output, "Copy"));
    assert_eq!(fs::read(&output).unwrap(), fs::read(&new_output).unwrap());
    assert_eq!(
        success(cli(["preview"])),
        "Offline byte preview; no device opened. Live access is locked.\n\
         PODxt routing enable: f0 00 01 0c 03 23 01 f7\n\
         Variax MIDI enable:   f0 00 01 0c 07 00 2a 01 f7\n\
         Metadata block 1:    f0 00 01 0c 07 00 3c 00 02 00 00 00 00 03 05 03 00 00 00 f7\n\
         Metadata block 2:    f0 00 01 0c 07 00 3c 00 02 00 00 00 00 03 07 03 00 00 00 f7\n"
    );
    failure(
        cli(["inspect"]),
        "Usage: variax-workbench audio-capabilities",
    );
    failure(
        cli([] as [&str; 0]),
        "Usage: variax-workbench audio-capabilities",
    );
}

#[cfg(unix)]
#[test]
fn preset_subcommands_accept_non_unicode_paths() {
    use std::os::unix::ffi::OsStrExt;

    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join(OsStr::from_bytes(b"test-\xff"));
    let output = dir.path().join(OsStr::from_bytes(b"copy-\xff"));
    fs::write(&input, VARIAX).unwrap();
    assert!(success(inspect(&input)).contains("Synthetic"));
    success(rename(&input, &output, "Renamed"));
    assert!(success(inspect(&output)).contains("Renamed"));
}
