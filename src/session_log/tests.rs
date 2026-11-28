use super::*;
use serde_json::Value;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

fn export(log: &SessionLog, destination: &Path) -> std::io::Result<()> {
    log.export_current(destination.into())?
        .recv_timeout(Duration::from_secs(5))
        .expect("log worker responds")
}

#[test]
fn profiles_include_named_owner_on_optimized_builds() {
    use profile::default_level;
    assert_eq!(default_level(Some("owner"), true), Severity::Trace);
    assert_eq!(default_level(Some("development"), false), Severity::Trace);
    assert_eq!(default_level(None, false), Severity::Trace);
    assert_eq!(default_level(None, true), Severity::Warn);
    assert_eq!(default_level(Some("v1"), false), Severity::Warn);
    assert_eq!(default_level(Some("unrecognized"), false), Severity::Warn);
    assert_eq!(Severity::Trace.label(), "TRACE",);
}

#[test]
fn filtering_and_atomic_export_keep_structured_failure_and_preserve_existing_file() {
    let temp = tempfile::tempdir().unwrap();
    let folder = temp.path().join("logs");
    let (log, guard) = SessionLog::open(Some(folder.clone()), Severity::Warn);
    let op = Operation::with_log(&log, Endpoint::Hd500x);
    op.ui(Control::HdOpenFile, UiAction::Button, Disposition::Applied);
    op.finished(true);
    op.finished(false);
    log.gui_startup_failed();
    let copy = temp.path().join("export.jsonl");
    export(&log, &copy).unwrap();
    let status = log.status();
    assert_eq!(status.level, Severity::Warn);
    assert_eq!(status.folder.as_deref(), Some(folder.as_path()));
    assert_eq!(status.state, SinkState::Recording);
    let current = status.path.unwrap();
    assert!(current.starts_with(&folder));
    let contents = fs::read_to_string(&copy).unwrap();
    assert_eq!(contents, fs::read_to_string(&current).unwrap());
    let rows: Vec<Value> = contents
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["severity"], "ERROR");
    assert_eq!(rows[0]["operation"], op.id);
    assert_eq!(rows[1]["severity"], "CRITICAL");
    assert_eq!(rows[1]["category"], "gui_startup_failed");
    for row in rows {
        assert!(row["unix_ms"].as_u64().unwrap() > 0);
        assert!(!row["build"].as_str().unwrap().is_empty());
        assert!(!row["revision"].as_str().unwrap().is_empty());
    }
    assert_eq!(
        export(&log, &copy).unwrap_err().kind(),
        ErrorKind::AlreadyExists
    );
    assert_eq!(contents, fs::read_to_string(copy).unwrap());
    drop(guard);
    assert_eq!(log.status().state, SinkState::Closed);
}

#[test]
fn unavailable_log_folder_does_not_prevent_operation_events_from_returning() {
    let temp = tempfile::tempdir().unwrap();
    let obstacle = temp.path().join("regular-file");
    fs::write(&obstacle, "untouched").unwrap();
    let (log, guard) = SessionLog::open(Some(obstacle.join("logs")), Severity::Trace);
    let op = Operation::with_log(&log, Endpoint::Variax);
    op.finished(false);
    drop(guard);
    let status = log.status();
    assert_eq!(status.state, SinkState::Unavailable);
    assert!(status.path.is_none());
    assert!(log.export_current(temp.path().join("copy")).is_err());
    assert_eq!(fs::read_to_string(obstacle).unwrap(), "untouched");
}

#[test]
fn recall_facts_accept_only_bounded_control_differences() {
    let temp = tempfile::tempdir().unwrap();
    let (log, guard) = SessionLog::open(Some(temp.path().join("logs")), Severity::Trace);
    let op = Operation::with_log(&log, Endpoint::Podxt);
    let source = PodxtChannel::new(4);
    let target = PodxtChannel::new(7).unwrap();
    let cc = ControlDifference::cc(13, 50, 51, CcClass::Named).unwrap();
    let opaque = ControlDifference::opaque_bits(23, 4).unwrap();
    assert!(PodxtChannel::new(128).is_none());
    assert!(ControlDifference::cc(128, 0, 1, CcClass::Unknown).is_none());
    assert!(ControlDifference::cc(10, 0, 128, CcClass::Unknown).is_none());
    assert!(ControlDifference::cc(10, 10, 10, CcClass::Volatile).is_none());
    assert!(ControlDifference::opaque_bits(39, 1).is_none());
    assert!(ControlDifference::opaque_bits(23, 0).is_none());
    op.podxt_sync(
        source,
        target,
        PodxtSyncReason::Requested,
        PodxtSyncEvidence::None,
        None,
    );
    op.podxt_sync(
        source,
        target,
        PodxtSyncReason::ControlDifference,
        PodxtSyncEvidence::AddressedSlot,
        Some(cc),
    );
    op.podxt_sync(
        source,
        target,
        PodxtSyncReason::OpaqueDifference,
        PodxtSyncEvidence::AddressedSlot,
        Some(opaque),
    );
    op.podxt_retained_draft(source.unwrap(), target, PodxtDraftOrigin::DeviceBuffer);
    op.podxt_retained_draft(source.unwrap(), target, PodxtDraftOrigin::EditorEdit);
    drop(guard);
    let text = fs::read_to_string(log.status().path.unwrap()).unwrap();
    let rows: Vec<Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .filter(|row: &Value| row["category"] == "podxt_sync")
        .collect();
    assert_eq!(rows.len(), 5);
    assert!(
        rows.iter()
            .all(|row| row["operation"] == op.id && row["source"] == 4 && row["target"] == 7)
    );
    assert_eq!(
        rows[1]["difference"],
        serde_json::json!({"kind":"cc","number":13,"playing":50,"saved":51,"class":"named"})
    );
    assert_eq!(
        rows[2]["difference"],
        serde_json::json!({"kind":"opaque_bits","offset":23,"changed_bits":4})
    );
    assert!(rows[2].get("draft_origin").is_none());
    assert_eq!(rows[3]["reason"], "retained_draft");
    assert_eq!(rows[3]["evidence"], "checked_release");
    assert_eq!(rows[3]["draft_origin"], "device_buffer");
    assert_eq!(rows[4]["draft_origin"], "editor_edit");
}

use line6_devices::session::{Link, Path as LinkPath, Request};

pub(crate) struct ScriptedLink {
    path: LinkPath,
    script: std::collections::VecDeque<(Vec<u8>, Option<Vec<u8>>)>,
    reply: Option<Vec<u8>>,
    elapsed: Duration,
}

impl ScriptedLink {
    pub(crate) fn new<R: Request>(
        path: LinkPath,
        script: impl IntoIterator<Item = (R, Option<Vec<u8>>)>,
    ) -> Self {
        Self {
            path,
            script: script
                .into_iter()
                .map(|(request, reply)| (request.bytes(), reply))
                .collect(),
            reply: None,
            elapsed: Duration::ZERO,
        }
    }

    pub(crate) fn assert_finished(&self) {
        assert!(self.script.is_empty());
        assert!(self.reply.is_none());
    }
}

impl Link for ScriptedLink {
    fn path(&self) -> LinkPath {
        self.path
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        assert!(self.reply.is_none(), "previous reply must be consumed");
        let (expected, reply) = self.script.pop_front().expect("unscripted device call");
        assert_eq!(bytes, expected);
        self.reply = reply;
        Ok(())
    }

    fn read(&mut self, wait: Duration) -> Result<Option<Vec<u8>>, String> {
        self.elapsed += wait;
        Ok(self.reply.take())
    }

    fn elapsed(&self) -> Duration {
        self.elapsed
    }

    fn pause(&mut self, duration: Duration) {
        self.elapsed += duration;
    }
}

#[test]
fn dial_queue_fixture_coalesces_then_sends_and_verifies_under_one_operation() {
    use line6_devices::{
        family::podxt::{Control as PodxtControl, PodxtRequest, Value},
        gate::{GatePolicy, Operation as GateOperation},
        session::{Session, Wait},
    };

    struct FakePolicy;
    impl GatePolicy for FakePolicy {
        fn allows(&self, operation: GateOperation) -> bool {
            matches!(
                operation,
                GateOperation::PodxtEditBufferWrite | GateOperation::PodxtRead
            )
        }
    }

    let directory = tempfile::tempdir().unwrap();
    let (log, guard) = SessionLog::open(Some(directory.path().into()), Severity::Trace);
    let operation = Operation::with_log(&log, Endpoint::Podxt);

    // The future editor adapter reports the incoming values, the one replaced
    // before dispatch, and the result of the send/read using this same handle.
    let mut pending = None;
    for (old, new) in [(10, 20), (20, 30)] {
        let action = UiAction::Dial { old, new };
        operation.ui(Control::PodxtControl, action, Disposition::Queued);
        if let Some(previous) = pending.replace(action) {
            operation.ui(Control::PodxtControl, previous, Disposition::Coalesced);
        }
    }
    let action = pending.take().unwrap();
    let UiAction::Dial { new, .. } = action else {
        unreachable!()
    };
    let setting = PodxtRequest::Set(PodxtControl::Drive, Value::new(new as u8).unwrap());
    let expected_reply = vec![0xf0, 0x01, 30, 0xf7];
    let mut link = ScriptedLink::new(
        setting.path(),
        [
            (setting, None),
            (PodxtRequest::EditBuffer, Some(expected_reply.clone())),
        ],
    );
    let mut observer = operation.clone();
    let mut session = Session::new(&mut link, &FakePolicy, &mut observer);
    session.send(setting).unwrap();
    session.send(PodxtRequest::EditBuffer).unwrap();
    let reply = session
        .wait_for(
            Duration::from_millis(200),
            |data| data.starts_with(&[0xf0, 0x01]),
            false,
        )
        .unwrap();
    assert!(matches!(reply, Wait::Found(data) if data == expected_reply));
    link.assert_finished();
    operation.ui(Control::PodxtControl, action, Disposition::Applied);
    operation.checked_release(true, Duration::ZERO);
    drop(guard);
    let text = fs::read_to_string(log.status().path.unwrap()).unwrap();
    let rows: Vec<serde_json::Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .filter(|row: &serde_json::Value| row["operation"] == operation.id)
        .collect();
    assert_eq!(rows[2]["disposition"], "coalesced");
    assert_eq!(rows[2]["action"]["new"], 20);
    assert_eq!(rows[4]["observation"]["request"], "PodxtEditBufferWrite");
    assert_eq!(rows[4]["observation"]["outcome"], "Sent");
    let outcome = |index: usize| &rows[index]["observation"]["outcome"];
    assert_eq!(outcome(9), "Packet");
    assert_eq!(outcome(10), "Reply");
    assert_eq!(rows[11]["disposition"], "applied");
    assert_eq!(rows[11]["action"]["new"], 30);
    assert_eq!(rows[12]["category"], "checked_release");
}
