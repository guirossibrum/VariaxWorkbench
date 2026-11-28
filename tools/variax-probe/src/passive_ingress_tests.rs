use super::*;
use std::{collections::VecDeque, rc::Rc};
use variax_workbench::identity_stage::guitar_reply;

const IDENTITY: &[u8] = &[0xf0, 0x7e, 0x7f, 0x06, 0x01, 0xf7];
const RELAY_ON: &[u8] = &[0xf0, 0, 1, 12, 3, 0x23, 1, 0xf7];
const RELAY_OFF: &[u8] = &[0xf0, 0, 1, 12, 3, 0x23, 0, 0xf7];

#[derive(Clone, Debug, PartialEq, Eq)]
enum Step {
    Claim,
    Send(Vec<u8>),
    Waiter,
    UsbRelease,
    AlsaClose,
}

#[derive(Default)]
struct Fixture {
    steps: RefCell<Vec<Step>>,
    reads: Cell<usize>,
}

struct FakePodxt {
    fixture: Rc<Fixture>,
    script: VecDeque<Option<Vec<u8>>>,
    fail_close: bool,
}

impl PodxtInput for FakePodxt {
    fn read(&mut self, wait: Duration) -> Result<Option<Vec<u8>>, String> {
        assert_eq!(wait, Duration::ZERO, "ALSA must never delay a USB poll");
        Ok(self.script.pop_front().flatten())
    }

    fn finish(self) -> Result<(), String> {
        self.fixture.steps.borrow_mut().push(Step::AlsaClose);
        if self.fail_close {
            return Err("Injected ALSA close failure".into());
        }
        Ok(())
    }
}

struct FakeGuitar {
    fixture: Rc<Fixture>,
    script: VecDeque<Vec<u8>>,
    elapsed: Duration,
    fail_off: bool,
    fail_usb_release: bool,
}

impl Link for FakeGuitar {
    fn path(&self) -> Path {
        Path::VariaxViaPodxt
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.fixture
            .steps
            .borrow_mut()
            .push(Step::Send(bytes.into()));
        if self.fail_off && bytes == VariaxRequest::SessionOff.bytes() {
            return Err("Injected session off failure".into());
        }
        Ok(())
    }

    fn read(&mut self, wait: Duration) -> Result<Option<Vec<u8>>, String> {
        self.fixture.reads.set(self.fixture.reads.get() + 1);
        self.elapsed += wait;
        Ok(self.script.pop_front())
    }

    fn elapsed(&self) -> Duration {
        self.elapsed
    }

    fn pause(&mut self, duration: Duration) {
        self.elapsed += duration;
    }
}

impl GuitarInput for FakeGuitar {
    fn finish(&mut self) -> Result<(), String> {
        self.fixture.steps.borrow_mut().push(Step::UsbRelease);
        if self.fail_usb_release {
            return Err("Injected driver return failure".into());
        }
        Ok(())
    }
}

impl Fixture {
    fn podxt(self: &Rc<Self>) -> FakePodxt {
        FakePodxt {
            fixture: self.clone(),
            script: [
                Some(vec![0xc0, 4]), // buffered before the marker
                None,
                Some(vec![0xc0]), // first fresh event spans two reads
                Some(vec![127]),
            ]
            .into(),
            fail_close: false,
        }
    }

    fn guitar(self: &Rc<Self>) -> FakeGuitar {
        self.steps.borrow_mut().push(Step::Claim);
        FakeGuitar {
            fixture: self.clone(),
            script: [
                guitar_reply(),
                vec![0xc0, 77],
                vec![0xb0, 7, 63],
                vec![0xb0, 79, 96],
            ]
            .into(),
            elapsed: Duration::ZERO,
            fail_off: false,
            fail_usb_release: false,
        }
    }

    fn stop_after_events(&self) -> Stop {
        if self.reads.get() >= 5 {
            Stop::Cancelled
        } else {
            Stop::Running
        }
    }

    fn waiter(&self) -> Result<(), String> {
        assert!(!self.steps.borrow().contains(&Step::UsbRelease));
        assert!(!self.steps.borrow().contains(&Step::AlsaClose));
        self.steps.borrow_mut().push(Step::Waiter);
        Ok(())
    }

    fn steps(&self) -> Vec<Step> {
        self.steps.borrow().clone()
    }
}

#[test]
fn decoded_routes_overlap_and_cancel_with_the_waiter_pending_through_both_releases() {
    let fixture = Rc::new(Fixture::default());
    let lines = RefCell::new(Vec::new());
    exercise(
        fixture.podxt(),
        || Ok(fixture.guitar()),
        || Ok(()),
        || fixture.stop_after_events(),
        || fixture.waiter(),
        |line| lines.borrow_mut().push(line.to_owned()),
    )
    .unwrap();

    assert_eq!(
        fixture.steps(),
        [
            Step::Claim,
            Step::Send(IDENTITY.into()),
            Step::Send(RELAY_ON.into()),
            Step::Send(VariaxRequest::SessionOn.bytes()),
            Step::Waiter,
            Step::Send(VariaxRequest::SessionOff.bytes()),
            Step::Send(RELAY_OFF.into()),
            Step::UsbRelease,
            Step::AlsaClose,
        ]
    );
    let lines = lines.into_inner().join("\n");
    for event in [
        "PODxt ALSA channel program 127",
        "Variax USB selector",
        "Variax USB Volume(63)",
        "Variax USB TonePosition(96)",
        "Variax session off and relay off succeeded.",
    ] {
        assert!(lines.contains(event), "{lines}");
    }
    assert!(!lines.contains("PODxt ALSA channel program 4"));
}

#[test]
fn a_missing_podxt_change_keeps_the_run_incomplete_after_clean_release() {
    let fixture = Rc::new(Fixture::default());
    let mut podxt = fixture.podxt();
    podxt.script = [Some(vec![0xc0, 4]), None].into();
    let error = exercise(
        podxt,
        || Ok(fixture.guitar()),
        || Ok(()),
        || fixture.stop_after_events(),
        || fixture.waiter(),
        |_| {},
    )
    .unwrap_err();
    assert!(
        error.contains("Missing physical input: PODxt channel"),
        "{error}"
    );
    assert!(
        fixture
            .steps()
            .ends_with(&[Step::UsbRelease, Step::AlsaClose])
    );
}

#[test]
fn busy_or_cancelled_before_usb_claim_still_closes_open_alsa() {
    for reason in [Stop::Cancelled, Stop::Foreground] {
        let fixture = Rc::new(Fixture::default());
        let error = exercise::<_, FakeGuitar>(
            fixture.podxt(),
            || panic!("must stop before touching USB"),
            || panic!("must stop before checking USB"),
            || reason,
            || panic!("no overlap to yield"),
            |_| {},
        )
        .unwrap_err();
        assert!(error.contains("Stopped before claiming"));
        assert_eq!(fixture.steps(), [Step::AlsaClose]);
    }
}

#[test]
fn stale_same_address_is_rejected_before_claim_or_before_identity() {
    for stale_at in [1, 2] {
        let fixture = Rc::new(Fixture::default());
        let checks = Cell::new(0);
        let error = exercise(
            fixture.podxt(),
            || Ok(fixture.guitar()),
            || {
                checks.set(checks.get() + 1);
                if checks.get() >= stale_at {
                    Err("Attachment changed at the same USB address.".into())
                } else {
                    Ok(())
                }
            },
            || Stop::Running,
            || unreachable!(),
            |_| {},
        )
        .unwrap_err();
        assert!(error.contains("Attachment changed"));
        let expected: &[_] = if stale_at == 1 {
            &[Step::AlsaClose]
        } else {
            &[Step::Claim, Step::UsbRelease, Step::AlsaClose]
        };
        assert_eq!(fixture.steps(), expected);
    }
}

#[test]
fn wrong_guitar_identity_closes_both_inputs_without_enabling_relay() {
    let fixture = Rc::new(Fixture::default());
    let error = exercise(
        fixture.podxt(),
        || {
            let mut guitar = fixture.guitar();
            guitar.script[0][13] = b'9';
            Ok(guitar)
        },
        || Ok(()),
        || Stop::Running,
        || unreachable!(),
        |_| {},
    )
    .unwrap_err();
    assert!(error.contains("did not verify"));
    assert_eq!(
        fixture.steps(),
        [
            Step::Claim,
            Step::Send(IDENTITY.into()),
            Step::UsbRelease,
            Step::AlsaClose,
        ]
    );
}

#[test]
fn protocol_driver_and_alsa_failures_remain_visible_and_all_cleanup_is_attempted() {
    let fixture = Rc::new(Fixture::default());
    let mut podxt = fixture.podxt();
    podxt.fail_close = true;
    let error = exercise(
        podxt,
        || {
            let mut guitar = fixture.guitar();
            guitar.fail_off = true;
            guitar.fail_usb_release = true;
            Ok(guitar)
        },
        || Ok(()),
        || fixture.stop_after_events(),
        || fixture.waiter(),
        |_| {},
    )
    .unwrap_err();
    for failure in [
        "session off failure",
        "driver return failure",
        "ALSA close failure",
    ] {
        assert!(error.contains(failure), "{error}");
    }
    assert!(fixture.steps().ends_with(&[
        Step::Waiter,
        Step::Send(VariaxRequest::SessionOff.bytes()),
        Step::Send(RELAY_OFF.into()),
        Step::UsbRelease,
        Step::AlsaClose,
    ]));
}

#[test]
fn separate_lock_only_waiter_obeys_clean_and_failed_handoffs() {
    const CHILD: &str = "VARIAX_PASSIVE_INGRESS_TEST_RUNTIME";
    if std::env::var_os(CHILD).is_none() {
        let runtime =
            std::env::temp_dir().join(format!("variax-ingress-waiter-{}", std::process::id()));
        std::fs::create_dir(&runtime).unwrap();
        let (_, module) = module_path!().split_once("::").unwrap();
        let test = format!("{module}::separate_lock_only_waiter_obeys_clean_and_failed_handoffs");
        let output = std::process::Command::new(std::env::args_os().next().unwrap())
            .args(["--exact", &test])
            .env(CHILD, &runtime)
            .env("XDG_RUNTIME_DIR", &runtime)
            .output()
            .unwrap();
        std::fs::remove_dir_all(runtime).unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && stdout.contains("1 passed"),
            "{stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    assert_eq!(std::env::var_os(CHILD), std::env::var_os("XDG_RUNTIME_DIR"));
    for failed in [false, true] {
        let lease = FollowDeviceLock::acquire().unwrap();
        let fixture = Rc::new(Fixture::default());
        let mut podxt = fixture.podxt();
        podxt.fail_close = failed;
        let waiter = RefCell::new(None);
        let result = exercise(
            podxt,
            || Ok(fixture.guitar()),
            || Ok(()),
            || fixture.stop_after_events(),
            || begin_waiter(&lease, &waiter),
            |_| {},
        );
        assert_eq!(result.is_err(), failed);
        assert!(
            fixture
                .steps()
                .ends_with(&[Step::UsbRelease, Step::AlsaClose])
        );
        let waiter = waiter.into_inner().unwrap();
        assert!(!waiter.is_finished(), "the rig is still held");
        if failed {
            lease.cleanup_failed();
        }
        drop(lease);
        let result = waiter.join().unwrap();
        if failed {
            assert!(result.unwrap_err().contains("could not cleanly release"));
        } else {
            result.unwrap();
        }
    }
}

#[test]
fn timeout_attempts_all_cleanup_without_starting_a_waiter() {
    let fixture = Rc::new(Fixture::default());
    let error = exercise(
        fixture.podxt(),
        || Ok(fixture.guitar()),
        || Ok(()),
        || Stop::Running,
        || panic!("only an explicit cancellation starts the waiter"),
        |_| {},
    )
    .unwrap_err();
    assert!(error.contains("must end with Ctrl+C"));
    assert!((300..305).contains(&fixture.reads.get()));
    assert!(fixture.steps().ends_with(&[
        Step::Send(VariaxRequest::SessionOff.bytes()),
        Step::Send(RELAY_OFF.into()),
        Step::UsbRelease,
        Step::AlsaClose,
    ]));
}

#[test]
fn failed_guitar_open_keeps_the_original_error_and_checks_alsa_close() {
    let fixture = Rc::new(Fixture::default());
    let mut podxt = fixture.podxt();
    podxt.fail_close = true;
    let error = exercise::<_, FakeGuitar>(
        podxt,
        || Err("Expected USB bridge not available.".into()),
        || Ok(()),
        || Stop::Running,
        || unreachable!(),
        |_| {},
    )
    .unwrap_err();
    assert!(error.contains("Expected USB bridge not available"));
    assert!(error.contains("Injected ALSA close failure"));
    assert_eq!(fixture.steps(), [Step::AlsaClose]);
}
