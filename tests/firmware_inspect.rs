//! CLI coverage for the offline firmware inspector. No device files are opened.

use std::io::Write;
use std::process::Command;
use variax_workbench::firmware::synthetic::synthetic_container;
use variax_workbench::firmware::{PODXT_LIVE_PRODUCT_ID, PODXT_LIVE_VERSION};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_variax-firmware-inspect"))
}

#[test]
fn self_test_reports_rejection_counts_and_the_integrity_limit() {
    let output = bin().arg("--self-test").output().expect("run self-test");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).expect("utf-8");
    assert!(text.contains("truncated_prefixes: 390"));
    assert!(text.contains("damaged_structural_fields: 6"));
    assert!(text.contains("wrong_product: 2"));
    assert!(text.contains("wrong_version: 1"));
    assert!(text.contains("oversized: 2"));
    assert!(text.contains("overlapping: 2"));
    assert!(text.contains("duplicate_chunks: 2"));
    assert!(text.contains("corrupted_image: 4"));
    assert!(text.contains("integrity_is_not_authentication: true"));
    assert!(text.contains("no flashing API"));
}

#[test]
fn flash_and_other_unknown_flags_are_rejected() {
    for args in [
        &["--flash", "image.bin"][..],
        &["--write", "image.bin"][..],
        &["--install"][..],
    ] {
        let output = bin().args(args).output().expect("run inspector");
        assert!(!output.status.success(), "{args:?} should fail");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("unknown argument"), "{stderr}");
    }
}

#[test]
fn json_report_states_that_md5_is_not_authentication() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = directory.path().join("synthetic.l6ff");
    let bytes = synthetic_container(
        PODXT_LIVE_PRODUCT_ID,
        PODXT_LIVE_VERSION,
        b"cli-synthetic-image",
    );
    std::fs::File::create(&path)
        .expect("create")
        .write_all(&bytes)
        .expect("write synthetic container");
    let output = bin()
        .args([
            "--json",
            "--expect-product",
            "0003000a",
            "--expect-version",
            "03010000",
        ])
        .arg(&path)
        .output()
        .expect("inspect synthetic file");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).expect("utf-8");
    assert!(text.contains("\"integrity_is_not_authentication\": true"));
    assert!(text.contains("\"applicability\": \"recorded-declaration\""));
    assert!(text.contains("not a signature"));
    assert!(!text.contains("\"image_bytes_embedded\""));
}

#[test]
fn wrong_version_expectation_fails_without_writing_an_image() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = directory.path().join("synthetic.l6ff");
    let bytes = synthetic_container(
        PODXT_LIVE_PRODUCT_ID,
        PODXT_LIVE_VERSION,
        b"cli-synthetic-image",
    );
    std::fs::write(&path, bytes).expect("write synthetic container");
    let output = bin()
        .args([
            "--expect-product",
            "0x0003000a",
            "--expect-version",
            "0x03070000",
        ])
        .arg(&path)
        .output()
        .expect("inspect");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("wrong version"), "{stderr}");
    assert!(directory.path().read_dir().expect("dir").count() == 1);
}

#[test]
fn a_directory_is_not_opened_as_a_device_or_container() {
    let directory = tempfile::tempdir().expect("tempdir");
    let output = bin()
        .arg(directory.path())
        .output()
        .expect("inspect directory");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not a regular file"), "{stderr}");
}
