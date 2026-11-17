use std::process::Command;
use variax_workbench::rig::audio::{ORIGINAL_SOURCE_SHA256, PROJECTION_SHA256};

#[test]
fn embedded_projection_preserves_byte_pinned_lf_storage_on_each_platform() {
    let source = variax_workbench::rig::audio::SNAPSHOT;
    assert!(!source.as_bytes().contains(&b'\r'));
    assert_eq!(
        source.as_bytes().iter().filter(|&&b| b == b'\n').count(),
        225
    );
    assert!(variax_workbench::rig::audio::Registry::from_snapshot(source).is_ok());
}

#[test]
fn cli_exports_224_source_rows_and_separate_static_route_declaration() {
    let executable = env!("CARGO_BIN_EXE_variax-workbench");
    let output = Command::new(executable)
        .arg("audio-capabilities")
        .output()
        .unwrap();
    assert!(output.status.success());
    let rows = String::from_utf8(output.stdout).unwrap();
    assert_eq!(rows.lines().count(), 226);
    assert!(rows.contains(ORIGINAL_SOURCE_SHA256));
    assert!(rows.contains(PROJECTION_SHA256));
    assert_eq!(
        rows.lines().filter(|l| l.contains("\tTonePort\t")).count(),
        2
    );
    let output = Command::new(executable)
        .arg("audio-route-declarations")
        .output()
        .unwrap();
    assert!(output.status.success());
    let routes = String::from_utf8(output.stdout).unwrap();
    assert_eq!(routes.lines().count(), 2);
    assert!(routes.contains("78125/2") && routes.contains("never live support"));
    assert!(
        !Command::new(executable)
            .args(["audio-capabilities", "--live"])
            .status()
            .unwrap()
            .success()
    );
}
