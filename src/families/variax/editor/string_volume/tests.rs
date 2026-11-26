use super::*;
use line6_devices::{
    backup::BackupFolder,
    gate::{GatePolicy, Operation},
    session::{Link, Path as LinkPath, Quiet, Session},
};
use std::time::Instant;

fn original() -> Levels {
    Levels::from_wire_words([0x800000, 0x123abc, 0xeeeeee, 0x345678, 0xffffff, 0x7fffff]).unwrap()
}

#[derive(Debug, PartialEq, Eq)]
enum Traffic {
    ReadRam(Levels),
    SetRam(Levels),
    Store,
}

struct FakeLink {
    ram: Levels,
    trace: Sender<Traffic>,
}

impl FakeLink {
    fn read_ram(&self) -> Levels {
        self.trace.send(Traffic::ReadRam(self.ram)).unwrap();
        self.ram
    }

    fn set_ram(&mut self, levels: Levels) -> Levels {
        self.trace.send(Traffic::SetRam(levels)).unwrap();
        self.ram = levels;
        self.read_ram()
    }
}

struct FakeEdit {
    link: FakeLink,
    original: Levels,
    backup: PathBuf,
}

impl EditSession for FakeEdit {
    fn original(&self) -> Levels {
        self.original
    }
    fn backup_folder(&self) -> &Path {
        &self.backup
    }
    fn set_ram(&mut self, levels: Levels) -> Result<Levels, String> {
        Ok(self.link.set_ram(levels))
    }
    fn restore_ram(&mut self) -> Result<(), String> {
        if self.link.ram != self.original {
            assert_eq!(self.link.set_ram(self.original), self.original);
        }
        Ok(())
    }
    fn request_store(&mut self) -> Result<StoreRequestSent, String> {
        let ram_before_request = self.link.read_ram();
        self.link.trace.send(Traffic::Store).unwrap();
        Ok(StoreRequestSent { ram_before_request })
    }
}

fn fake_worker(root: &Path, can_store: bool) -> (Worker, Receiver<Traffic>) {
    let root = root.to_owned();
    let (trace, traffic) = mpsc::channel();
    let worker = Worker::spawn(can_store, move |io| {
        let link = FakeLink {
            ram: original(),
            trace,
        };
        let original = link.read_ram();
        let mut folder = BackupFolder::create(&root, "levels")?;
        folder.file(string_levels::BACKUP_FILE, &original.backup_bytes())?;
        let backup = folder.finish(true)?;
        let mut edit = FakeEdit {
            link,
            original,
            backup,
        };
        run_dialog(&mut edit, &io, can_store)
    })
    .unwrap();
    (worker, traffic)
}

fn until(worker: &mut Worker, ready: impl Fn(&Worker) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        assert!(worker.poll().is_none(), "worker exited early");
        if ready(worker) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "worker did not reach expected state"
        );
        thread::sleep(Duration::from_millis(1));
    }
}

fn finish(worker: &mut Worker) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(result) = worker.poll() {
            return result;
        }
        assert!(Instant::now() < deadline, "worker did not finish");
        thread::sleep(Duration::from_millis(1));
    }
}

fn preview(worker: &mut Worker, fraction: f32) -> Levels {
    let draft = worker.draft.as_mut().unwrap();
    draft.values[0] = fraction;
    let levels = original().with_workbench_levels(draft.values).unwrap();
    until(worker, |worker| {
        !worker.draft.as_ref().unwrap().awaiting_ram
    });
    levels
}

#[test]
fn backup_precedes_sliders_and_cancel_restores_exact_ram_through_the_fake_link() {
    let root = tempfile::tempdir().unwrap();
    let (mut worker, traffic) = fake_worker(root.path(), false);
    assert!(worker.draft.is_none());
    until(&mut worker, |worker| worker.draft.is_some());
    let draft = worker.draft.as_ref().unwrap();
    assert_eq!(draft.values, original().workbench_levels());
    assert_eq!(traffic.try_recv().unwrap(), Traffic::ReadRam(original()));
    assert!(traffic.try_recv().is_err());
    assert_eq!(
        std::fs::read(draft.backup.join(string_levels::BACKUP_FILE)).unwrap(),
        original().backup_bytes(),
    );
    assert_eq!(line6_devices::backup::verify_manifest(&draft.backup), Ok(2));

    let ctx = egui::Context::default();
    let mut app = crate::editor::Workbench::with_context(&ctx, None, PathBuf::new(), None);
    app.variax_mut().string_volume.worker = Some(worker);
    for _ in 0..2 {
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            app.variax_mut().string_volume_dialog(ctx);
        });
    }
    let mut worker = app.variax_mut().string_volume.worker.take().unwrap();
    assert_eq!(
        worker.draft.as_ref().unwrap().values,
        original().workbench_levels()
    );
    assert!(traffic.try_recv().is_err());

    let edited = preview(&mut worker, 0.432);
    assert_ne!(edited, original());
    worker.confirm_store = true;
    worker.store();
    assert!(!worker.finishing);
    worker.cancel();
    finish(&mut worker).unwrap();
    assert_eq!(
        traffic.try_iter().collect::<Vec<_>>(),
        [
            Traffic::SetRam(edited),
            Traffic::ReadRam(edited),
            Traffic::SetRam(original()),
            Traffic::ReadRam(original()),
        ],
    );
}

#[test]
fn confirmed_ok_sends_one_request_and_saves_only_ram_evidence() {
    let root = tempfile::tempdir().unwrap();
    let (mut worker, traffic) = fake_worker(root.path(), true);
    until(&mut worker, |worker| worker.draft.is_some());
    let edited = preview(&mut worker, 0.579);
    worker.store();
    assert!(!worker.finishing);
    worker.confirm_store = true;
    worker.store();
    worker.store();
    worker.cancel();
    finish(&mut worker).unwrap();
    assert_eq!(worker.store_sent, Some(edited));
    let draft = worker.draft.as_ref().unwrap();
    assert_eq!(
        std::fs::read(store_evidence_path(&draft.backup)).unwrap(),
        edited.backup_bytes(),
    );
    assert_eq!(
        std::fs::read(draft.backup.join(string_levels::BACKUP_FILE)).unwrap(),
        original().backup_bytes(),
    );
    assert_eq!(
        traffic.try_iter().collect::<Vec<_>>(),
        [
            Traffic::ReadRam(original()),
            Traffic::SetRam(edited),
            Traffic::ReadRam(edited),
            Traffic::ReadRam(edited),
            Traffic::Store,
        ],
    );
}

#[test]
fn a_failed_backup_never_enables_the_sliders_or_sends_a_change() {
    let root = tempfile::tempdir().unwrap();
    let backup = root.path().join("levels");
    std::fs::write(&backup, b"existing backup").unwrap();
    let (mut worker, traffic) = fake_worker(root.path(), true);
    assert!(finish(&mut worker).unwrap_err().contains("already exists"));
    assert!(worker.draft.is_none());
    assert_eq!(
        traffic.try_iter().collect::<Vec<_>>(),
        [Traffic::ReadRam(original())]
    );
    assert_eq!(std::fs::read(backup).unwrap(), b"existing backup");
}

#[test]
fn dropping_the_worker_waits_for_exact_restoration() {
    let root = tempfile::tempdir().unwrap();
    let (mut worker, traffic) = fake_worker(root.path(), false);
    until(&mut worker, |worker| worker.draft.is_some());
    let edited = preview(&mut worker, 0.211);
    drop(worker);
    assert_eq!(
        traffic.try_iter().collect::<Vec<_>>(),
        [
            Traffic::ReadRam(original()),
            Traffic::SetRam(edited),
            Traffic::ReadRam(edited),
            Traffic::SetRam(original()),
            Traffic::ReadRam(original()),
        ],
    );
}

#[test]
fn production_approvals_stay_closed_even_on_a_policy_permitted_fake_link() {
    struct Permit;
    impl GatePolicy for Permit {
        fn allows(&self, _: Operation) -> bool {
            true
        }
    }
    struct NoTraffic;
    impl Link for NoTraffic {
        fn path(&self) -> LinkPath {
            LinkPath::VariaxViaPodxt
        }
        fn write(&mut self, _: &[u8]) -> Result<(), String> {
            panic!("unexpected write")
        }
        fn read(&mut self, _: Duration) -> Result<Option<Vec<u8>>, String> {
            panic!("unexpected read")
        }
        fn elapsed(&self) -> Duration {
            Duration::ZERO
        }
        fn pause(&mut self, _: Duration) {
            panic!("unexpected pause")
        }
    }
    assert!(!editing_approved());
    assert!(!store_approved());
    let temp = tempfile::tempdir().unwrap();
    let mut log = Quiet;
    let mut session = Session::new(NoTraffic, &Permit, &mut log);
    assert!(
        string_levels::read_ram(&mut session)
            .unwrap_err()
            .contains("approval")
    );
    let result = string_levels::with_temporary_edit(&mut session, temp.path(), "levels", |_| {
        panic!("unapproved edit callback")
    });
    assert!(matches!(result, Err::<(), _>(error) if error.contains("approval")));
    assert!(!temp.path().join("levels").exists());
}
