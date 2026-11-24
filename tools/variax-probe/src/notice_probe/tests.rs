use super::*;
use std::cell::Cell;
use std::collections::VecDeque;

const PODXT: [u8; 17] = [
    0xf0, 0x7e, 0x7f, 6, 2, 0, 1, 0x0c, 3, 0, 0x0a, 0, 0, 3, 0, 1, 0xf7,
];
const GUITAR: [u8; 17] = [
    0xf0, 0x7e, 0x7f, 6, 2, 0, 1, 0x0c, 7, 0, 4, 0, b'0', b'3', b'1', b'0', 0xf7,
];
const ID: [u8; 6] = [0xf0, 0x7e, 0x7f, 6, 1, 0xf7];
const BRIDGE_ON: [u8; 8] = [0xf0, 0, 1, 0x0c, 3, 0x23, 1, 0xf7];
const BRIDGE_OFF: [u8; 8] = [0xf0, 0, 1, 0x0c, 3, 0x23, 0, 0xf7];
const MIDI_ON: [u8; 9] = [0xf0, 0, 1, 0x0c, 7, 0, 0x2a, 1, 0xf7];
const MIDI_OFF: [u8; 9] = [0xf0, 0, 1, 0x0c, 7, 0, 0x2a, 0, 0xf7];
const CONNECTED: [u8; 11] = [0xf0, 0, 1, 0x0c, 3, 0x11, 0, 0, 0, 7, 0xf7];
const DISCONNECTED: [u8; 12] = [0xf0, 0, 1, 0x0c, 0x7f, 3, 0, 0, 0, 0, 0, 0xf7];

#[derive(Clone, Copy)]
enum Scenario {
    Reply,
    OrphanTail,
    Unplugged,
    Cancel,
    ReadError,
    EnableError,
    WrongPod,
    WrongGuitar,
}

type Sends = Vec<(Path, Duration, Vec<u8>)>;

struct Rig {
    scenario: Scenario,
    locked: Cell<bool>,
    elapsed: Cell<Duration>,
    sent: RefCell<Sends>,
    opened_bridge: Cell<bool>,
    released: Cell<bool>,
    dropped_links: Cell<usize>,
    cancelled: Cell<bool>,
}

impl Rig {
    fn new(scenario: Scenario) -> Self {
        Self {
            scenario,
            locked: Cell::new(false),
            elapsed: Cell::new(Duration::ZERO),
            sent: RefCell::new(Vec::new()),
            opened_bridge: Cell::new(false),
            released: Cell::new(false),
            dropped_links: Cell::new(0),
            cancelled: Cell::new(false),
        }
    }

    fn acquire(&self) -> Result<Lease<'_>, String> {
        assert!(!self.locked.replace(true));
        Ok(Lease(self))
    }

    fn link(&self, path: Path) -> FakeLink<'_> {
        assert!(self.locked.get());
        if path == Path::VariaxViaPodxt {
            self.opened_bridge.set(true);
        }
        FakeLink {
            path,
            rig: self,
            input: VecDeque::new(),
            requests: 0,
            injected: false,
        }
    }

    fn observe(&self, trace: &RefCell<Trace<'_>>) -> Result<(), String> {
        with_rig(
            || self.acquire(),
            || {
                let result = observe(
                    self.link(Path::PodxtMidi),
                    || Ok(self.link(Path::VariaxViaPodxt)),
                    Duration::from_secs(1),
                    &|| self.cancelled.get(),
                    trace,
                );
                assert!(self.locked.get(), "lock also covers recording the result");
                assert_eq!(self.opened_bridge.get(), self.released.get());
                assert_eq!(
                    self.dropped_links.get(),
                    1 + usize::from(self.opened_bridge.get())
                );
                result
            },
        )
    }

    fn bytes(&self) -> Vec<Vec<u8>> {
        self.sent
            .borrow()
            .iter()
            .map(|(_, _, bytes)| bytes.clone())
            .collect()
    }
}

struct Lease<'a>(&'a Rig);

impl Drop for Lease<'_> {
    fn drop(&mut self) {
        assert!(self.0.locked.replace(false));
    }
}

struct FakeLink<'a> {
    path: Path,
    rig: &'a Rig,
    input: VecDeque<Vec<u8>>,
    requests: u8,
    injected: bool,
}

impl Link for FakeLink<'_> {
    fn path(&self) -> Path {
        self.path
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        assert!(self.rig.locked.get());
        self.rig
            .sent
            .borrow_mut()
            .push((self.path, self.elapsed(), bytes.to_vec()));
        match bytes {
            bytes if bytes == ID => {
                self.requests += 1;
                if self.path == Path::PodxtMidi {
                    let mut reply = PODXT;
                    if matches!(self.rig.scenario, Scenario::WrongPod) {
                        reply[15] = 2;
                    }
                    self.input.push_back(reply.to_vec());
                } else {
                    match self.rig.scenario {
                        Scenario::Unplugged => {}
                        Scenario::OrphanTail if self.requests == 1 => {
                            self.input.push_back(vec![0xf7])
                        }
                        Scenario::WrongGuitar => {
                            let mut reply = GUITAR;
                            reply[15] = b'1';
                            self.input.push_back(reply.to_vec());
                        }
                        _ => self.input.push_back(GUITAR.to_vec()),
                    }
                }
            }
            bytes if bytes == MIDI_ON && matches!(self.rig.scenario, Scenario::EnableError) => {
                return Err("injected enable write failure".into());
            }
            _ => {}
        }
        Ok(())
    }

    fn read(&mut self, wait: Duration) -> Result<Option<Vec<u8>>, String> {
        assert!(self.rig.locked.get());
        let midi_on = self
            .rig
            .sent
            .borrow()
            .iter()
            .any(|(_, _, bytes)| bytes == &MIDI_ON);
        if midi_on && !self.injected {
            self.injected = true;
            match self.path {
                Path::PodxtMidi => {
                    let mut bytes = CONNECTED[..6].to_vec();
                    bytes.push(0xfe); // realtime, remains in raw receipt
                    self.input
                        .extend([bytes, CONNECTED[6..].to_vec(), DISCONNECTED.to_vec()]);
                }
                _ => match self.rig.scenario {
                    Scenario::ReadError => return Err("injected USB read failure".into()),
                    Scenario::Cancel => self.rig.cancelled.set(true),
                    _ => {
                        for op in [0x28, 0x4d, 0x74] {
                            self.input.push_back(vec![0xf0, 0, 1, 0x0c, 7, 4, op, 0xf7]);
                        }
                    }
                },
            }
        }
        let packet = self.input.pop_front();
        if packet.is_none() {
            self.pause(wait);
        }
        Ok(packet)
    }

    fn elapsed(&self) -> Duration {
        self.rig.elapsed.get()
    }

    fn pause(&mut self, duration: Duration) {
        self.rig.elapsed.set(self.elapsed() + duration);
    }
}

impl BridgeLink for FakeLink<'_> {
    fn finish(&mut self) -> Result<(), String> {
        assert!(self.rig.locked.get());
        assert!(!self.rig.released.replace(true));
        Ok(())
    }
}

impl Drop for FakeLink<'_> {
    fn drop(&mut self) {
        assert!(self.rig.locked.get());
        if self.path == Path::VariaxViaPodxt {
            assert!(self.rig.released.get());
        }
        self.rig.dropped_links.set(self.rig.dropped_links.get() + 1);
    }
}

#[test]
fn exact_sequence_and_single_orphan_tail_resend() {
    for scenario in [Scenario::Reply, Scenario::OrphanTail] {
        let rig = Rig::new(scenario);
        let elapsed = || rig.elapsed.get();
        let trace = RefCell::new(Trace::new(&elapsed, &|_| {}));
        rig.observe(&trace).unwrap();
        let sent = rig.sent.borrow();
        let mut expected = vec![ID.to_vec(), ID.to_vec()];
        if matches!(scenario, Scenario::OrphanTail) {
            expected.push(ID.to_vec());
        }
        expected.extend([
            BRIDGE_ON.to_vec(),
            MIDI_ON.to_vec(),
            MIDI_OFF.to_vec(),
            BRIDGE_OFF.to_vec(),
        ]);
        assert_eq!(rig.bytes(), expected);
        assert_eq!(sent[0].0, Path::PodxtMidi);
        assert!(
            sent[1..]
                .iter()
                .all(|(path, _, _)| *path == Path::VariaxViaPodxt)
        );
        let tail = &sent[sent.len() - 4..];
        assert_eq!(tail[1].1 - tail[0].1, BRIDGE_SETTLE);
        assert_eq!(tail[2].1 - tail[1].1, Duration::from_secs(1));
        assert!(!rig.locked.get());
    }
}

#[test]
fn marks_both_inputs_and_keeps_raw_realtime_bytes() {
    let rig = Rig::new(Scenario::Reply);
    let elapsed = || rig.elapsed.get();
    let trace = RefCell::new(Trace::new(&elapsed, &|_| {}));
    rig.observe(&trace).unwrap();
    let trace = trace.into_inner();
    assert_eq!(trace.frames, [3, 4]); // two identities and five notices
    assert_eq!(trace.notices.len(), 5);
    for (source, mark) in [
        ("PODxt MIDI", "03 11: connected (state 0x07)"),
        ("PODxt MIDI", "7F 03: disconnected"),
        ("Variax link", "Variax 28 notice"),
        ("Variax link", "Variax 4D notice"),
        ("Variax link", "Variax 74 notice"),
    ] {
        assert!(trace.notices.iter().any(|line| {
            line.split_whitespace().next().unwrap().ends_with('s')
                && line.contains(source)
                && line.contains(mark)
        }));
    }
    assert!(
        trace
            .lines
            .iter()
            .any(|line| line.contains("PODxt MIDI IN  f0 00 01 0c 03 11 fe"))
    );
    let mut frame = CONNECTED;
    frame[9] = 0;
    assert_eq!(
        notice(&frame).as_deref(),
        Some("03 11: disconnected (state 0x00)")
    );
    assert_eq!(
        notice(&[0xf0, 0, 1, 0x0c, 3, 0x11, 0xf7]).as_deref(),
        Some("03 11: malformed length")
    );
}

#[test]
fn unplugged_start_listens_without_resending_identity() {
    let rig = Rig::new(Scenario::Unplugged);
    let elapsed = || rig.elapsed.get();
    let trace = RefCell::new(Trace::new(&elapsed, &|_| {}));
    rig.observe(&trace).unwrap();
    assert_eq!(
        rig.bytes(),
        [
            ID.to_vec(),
            ID.to_vec(),
            BRIDGE_ON.to_vec(),
            MIDI_ON.to_vec(),
            MIDI_OFF.to_vec(),
            BRIDGE_OFF.to_vec()
        ]
    );
    let trace = trace.into_inner();
    assert!(
        trace
            .lines
            .iter()
            .any(|line| line.contains("No guitar identity answered at startup"))
    );
    assert!(
        trace
            .notices
            .iter()
            .any(|line| line.contains("connected (state 0x07)"))
    );
}

#[test]
fn cancellation_and_errors_send_both_off_messages_before_release() {
    for (scenario, expected) in [
        (Scenario::Cancel, CANCELLED),
        (Scenario::ReadError, "injected USB read failure"),
        (Scenario::EnableError, "injected enable write failure"),
    ] {
        let rig = Rig::new(scenario);
        let elapsed = || rig.elapsed.get();
        let trace = RefCell::new(Trace::new(&elapsed, &|_| {}));
        assert_eq!(rig.observe(&trace), Err(expected.into()));
        assert_eq!(rig.bytes()[4..], [MIDI_OFF.to_vec(), BRIDGE_OFF.to_vec()]);
        assert!(rig.released.get());
        assert!(!rig.locked.get());
    }
}

#[test]
fn refuses_wrong_identity_before_enabling_and_busy_lock_before_opening() {
    assert_eq!(
        with_rig::<()>(|| Err("busy".into()), || panic!("busy rig opened")),
        Err("busy".into())
    );
    for (scenario, expected, requests) in [
        (Scenario::WrongPod, "Expected PODxt Live 3.01", 1),
        (Scenario::WrongGuitar, "Expected Variax 300/600 3.10", 2),
    ] {
        let rig = Rig::new(scenario);
        let elapsed = || rig.elapsed.get();
        let trace = RefCell::new(Trace::new(&elapsed, &|_| {}));
        assert!(rig.observe(&trace).unwrap_err().contains(expected));
        assert_eq!(rig.bytes(), vec![ID.to_vec(); requests]);
        assert_eq!(rig.opened_bridge.get(), requests == 2);
        assert!(!rig.locked.get());
    }
}
