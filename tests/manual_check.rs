use std::{
    fs,
    io::Write,
    process::{Command, Stdio},
};

#[test]
fn rehearsal_saves_partial_evidence_without_overwriting_any_report() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("session.txt");
    let mut child = Command::new(env!("CARGO_BIN_EXE_variax-check"))
        .args(["rehearse", path.to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"ok\nquit\n")
        .unwrap();
    assert!(child.wait().unwrap().success());
    let original = fs::read(&path).unwrap();
    let report = String::from_utf8(original.clone()).unwrap();
    assert_eq!(report.matches("NOT RUN").count(), 4);
    assert!(report.contains("Hardware recognition: UNVERIFIED"));
    let retry = Command::new(env!("CARGO_BIN_EXE_variax-check"))
        .args(["rehearse", path.to_str().unwrap()])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!retry.status.success());
    assert_eq!(fs::read(path).unwrap(), original);
}

#[test]
fn live_mode_is_unavailable_and_plan_does_not_start_a_session() {
    let output = Command::new(env!("CARGO_BIN_EXE_variax-check"))
        .arg("plan")
        .output()
        .unwrap();
    assert!(output.status.success());
    let plan = String::from_utf8(output.stdout).unwrap();
    assert!(plan.contains("OFFLINE REHEARSAL"));
    assert!(plan.contains("18-note spot check"));
    assert!(plan.contains("Existing alternate tuning"));
    let live = Command::new(env!("CARGO_BIN_EXE_variax-check"))
        .arg("live")
        .output()
        .unwrap();
    assert!(!live.status.success());
    assert!(
        String::from_utf8(live.stderr)
            .unwrap()
            .contains("no live mode exists")
    );
}
