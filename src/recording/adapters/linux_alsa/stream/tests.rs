use std::cell::RefCell;
use std::collections::VecDeque;

use super::*;
use crate::recording::adapters::linux_alsa::{
    ALSA_ACCESS_RW_INTERLEAVED, ALSA_ERRNO_EPIPE, ALSA_ERRNO_ESTRPIPE, ALSA_FORMAT_S24_3LE,
    tests::source_with_capacity,
};
use crate::recording::{Phase, SampleClock, TakeError};

const GENERATION: u64 = 6;
const PERIOD_FRAMES: u32 = 11;
const V600: Option<ProductProfile> = Some(ProductProfile::Variax600);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Call {
    Verify,
    Negotiate,
    Start,
    Read,
    Drop,
    Close,
}

type Trace = Rc<RefCell<Vec<Call>>>;

struct FakePcm {
    trace: Trace,
    route: Option<VerifiedAlsaRoute>,
    params: Result<AlsaCaptureParams, i32>,
    start_result: Result<(), i32>,
    drop_result: Result<(), i32>,
    close_result: Result<(), i32>,
    reads: VecDeque<(isize, u8)>,
}

impl FakePcm {
    fn new() -> Self {
        Self {
            trace: Trace::default(),
            route: Some(VerifiedAlsaRoute {
                driver: "snd-usb-pod",
                usb_vendor: 0x0e41,
                usb_product: 0x4650,
                usb_interface: 0,
                capture_endpoint: 0x82,
                direct_hardware_pcm: true,
                instrument: ProductProfile::Variax600,
                record_send: RecordRoute::SendCleanGuitar,
                channel_map: ChannelMap { left: 0, right: 1 },
            }),
            params: Ok(AlsaCaptureParams {
                pcm: AlsaNegotiatedPcm {
                    access: ALSA_ACCESS_RW_INTERLEAVED,
                    format: ALSA_FORMAT_S24_3LE,
                    channels: 2,
                    rate_num: 78_125,
                    rate_den: 2,
                },
                period_frames: PERIOD_FRAMES,
                periods: 2,
                buffer_frames: PERIOD_FRAMES * 2,
            }),
            start_result: Ok(()),
            drop_result: Ok(()),
            close_result: Ok(()),
            reads: VecDeque::new(),
        }
    }

    fn call(&self, call: Call) {
        self.trace.borrow_mut().push(call);
    }
}

impl AlsaCapturePcm for FakePcm {
    fn verified_route(&mut self) -> Option<VerifiedAlsaRoute> {
        self.call(Call::Verify);
        self.route
    }

    fn negotiated_params(&mut self) -> Result<AlsaCaptureParams, i32> {
        self.call(Call::Negotiate);
        self.params
    }

    fn start(&mut self) -> Result<(), i32> {
        self.call(Call::Start);
        self.start_result
    }

    fn read_interleaved(&mut self, frames: usize, buffer: &mut [u8]) -> isize {
        self.call(Call::Read);
        assert_eq!(frames, PERIOD_FRAMES as usize);
        assert_eq!(buffer.len(), frames * 6);
        let (result, value) = self.reads.pop_front().expect("unexpected PCM read");
        buffer.fill(value);
        result
    }

    fn drop_stream(&mut self) -> Result<(), i32> {
        self.call(Call::Drop);
        self.drop_result
    }

    fn close(self) -> Result<(), i32> {
        self.call(Call::Close);
        self.close_result
    }
}

fn plan() -> ValidatedPlan {
    source_with_capacity(V600, 1024)
}

fn assert_rejected(pcm: FakePcm, plan: ValidatedPlan, cause: AlsaStartFailure, calls: &[Call]) {
    let trace = pcm.trace.clone();
    let error = AlsaCaptureStream::start(pcm, plan, GENERATION)
        .err()
        .unwrap();
    assert_eq!(error.cause, cause);
    assert_eq!(&*trace.borrow(), calls);
}

#[test]
fn static_plan_and_matching_negotiation_cannot_substitute_for_route_evidence() {
    let cases: [fn(&mut VerifiedAlsaRoute); 5] = [
        |route| route.driver = "snd-usb-audio",
        |route| route.usb_interface = 1,
        |route| route.direct_hardware_pcm = false,
        |route| route.record_send = RecordRoute::SendReampPlayback,
        |route| route.channel_map = ChannelMap { left: 1, right: 0 },
    ];
    for change in cases {
        let mut pcm = FakePcm::new();
        change(pcm.route.as_mut().unwrap());
        assert_rejected(
            pcm,
            plan(),
            AlsaStartFailure::UnverifiedRoute,
            &[Call::Verify, Call::Close],
        );
    }
    let mut pcm = FakePcm::new();
    pcm.route = None;
    assert_rejected(
        pcm,
        plan(),
        AlsaStartFailure::UnverifiedRoute,
        &[Call::Verify, Call::Close],
    );
    assert_rejected(
        FakePcm::new(),
        source_with_capacity(None, 1024),
        AlsaStartFailure::UnverifiedRoute,
        &[Call::Close],
    );
}

#[test]
fn observed_format_and_clock_mismatches_close_before_start() {
    let cases = [
        (
            AlsaNegotiatedPcm {
                access: 0,
                ..FakePcm::new().params.unwrap().pcm
            },
            AlsaAdapterError::UnsupportedAccess(0),
        ),
        (
            AlsaNegotiatedPcm {
                format: 6,
                ..FakePcm::new().params.unwrap().pcm
            },
            AlsaAdapterError::UnsupportedFormat(6),
        ),
        (
            AlsaNegotiatedPcm {
                channels: 1,
                ..FakePcm::new().params.unwrap().pcm
            },
            AlsaAdapterError::InvalidChannels(1),
        ),
    ];
    let calls = &[Call::Verify, Call::Negotiate, Call::Close];
    for (actual, error) in cases {
        let mut pcm = FakePcm::new();
        pcm.params.as_mut().unwrap().pcm = actual;
        assert_rejected(pcm, plan(), AlsaStartFailure::Contract(error), calls);
    }
    for (numerator, denominator) in [
        (39_062, 1),
        (39_063, 1),
        (44_100, 1),
        (48_000, 1),
        (156_250, 4),
        (78_125, 0),
    ] {
        let mut pcm = FakePcm::new();
        let format = &mut pcm.params.as_mut().unwrap().pcm;
        format.rate_num = numerator;
        format.rate_den = denominator;
        assert_rejected(
            pcm,
            plan(),
            AlsaStartFailure::Contract(AlsaAdapterError::InvalidClock {
                numerator,
                denominator,
            }),
            calls,
        );
    }
}

#[test]
fn geometry_is_checked_before_allocating_or_starting() {
    use pod_audio::PlanError;
    for (period_frames, periods, buffer_frames, cause) in [
        (
            10,
            2,
            20,
            AlsaStartFailure::Period(PlanError::PeriodBelowDeclaredMinimum),
        ),
        (
            1366,
            2,
            2732,
            AlsaStartFailure::Period(PlanError::PeriodAboveDeclaredMaximum),
        ),
        (
            11,
            0,
            0,
            AlsaStartFailure::Period(PlanError::PeriodCountOutOfRange),
        ),
        (
            11,
            910,
            10_010,
            AlsaStartFailure::Period(PlanError::BufferAboveDeclaredMaximum),
        ),
        (11, 2, 21, AlsaStartFailure::BufferGeometry),
        (u32::MAX, 2, 22, AlsaStartFailure::BufferGeometry),
    ] {
        let mut pcm = FakePcm::new();
        let params = pcm.params.as_mut().unwrap();
        params.period_frames = period_frames;
        params.periods = periods;
        params.buffer_frames = buffer_frames;
        assert_rejected(
            pcm,
            plan(),
            cause,
            &[Call::Verify, Call::Negotiate, Call::Close],
        );
    }
}

#[test]
fn negotiation_and_start_failure_have_distinct_cleanup_paths() {
    let mut pcm = FakePcm::new();
    pcm.params = Err(-5);
    assert_rejected(
        pcm,
        plan(),
        AlsaStartFailure::Negotiation(-5),
        &[Call::Verify, Call::Negotiate, Call::Close],
    );

    let mut pcm = FakePcm::new();
    pcm.start_result = Err(-5);
    pcm.drop_result = Err(-32);
    pcm.close_result = Err(-19);
    let trace = pcm.trace.clone();
    let error = AlsaCaptureStream::start(pcm, plan(), GENERATION)
        .err()
        .unwrap();
    assert_eq!(
        error,
        AlsaStartError {
            cause: AlsaStartFailure::Start(-5),
            cleanup: AlsaCleanup {
                drop_errno: Some(-32),
                close_errno: Some(-19)
            },
        }
    );
    assert_eq!(
        &*trace.borrow(),
        &[
            Call::Verify,
            Call::Negotiate,
            Call::Start,
            Call::Drop,
            Call::Close
        ]
    );
}

#[test]
fn short_and_zero_reads_keep_pcm_exact_and_stop_is_idempotent() {
    let mut pcm = FakePcm::new();
    pcm.reads = [(2, 0x23), (0, 0xff), (1, 0x67)].into();
    let trace = pcm.trace.clone();
    let mut stream = AlsaCaptureStream::start(pcm, plan(), GENERATION).unwrap();
    assert_eq!(
        stream.read_period(GENERATION + 1).unwrap_err().capture,
        Some(CaptureError::StaleGeneration)
    );
    for frames in [2, 0, 1] {
        assert_eq!(stream.read_period(GENERATION), Ok(frames));
    }
    stream.stop().unwrap();
    stream.stop().unwrap();
    assert_eq!(
        stream.read_period(GENERATION).unwrap_err().capture,
        Some(CaptureError::Stopped)
    );
    let take = stream.session().take();
    assert_eq!(take.phase(), Phase::Stopped);
    assert_eq!(take.clock(), SampleClock::native_podxt());
    assert_eq!(take.pcm(), [&[0x23; 12][..], &[0x67; 6][..]].concat());
    assert_eq!(
        &*trace.borrow(),
        &[
            Call::Verify,
            Call::Negotiate,
            Call::Start,
            Call::Read,
            Call::Read,
            Call::Read,
            Call::Drop,
            Call::Close
        ]
    );
}

#[test]
fn terminal_reads_close_once_and_preserve_first_error_and_prefix() {
    for (frames, expected) in [
        (
            -(ALSA_ERRNO_EPIPE as isize),
            CaptureError::AdapterDiscontinuity(-ALSA_ERRNO_EPIPE),
        ),
        (
            -(ALSA_ERRNO_ESTRPIPE as isize),
            CaptureError::AdapterSuspended(-ALSA_ERRNO_ESTRPIPE),
        ),
        (-19, CaptureError::AdapterFailure(-19)),
        (12, CaptureError::AdapterFailure(i32::MIN)),
    ] {
        let mut pcm = FakePcm::new();
        pcm.reads = [(1, 3), (frames, 0xff)].into();
        pcm.drop_result = Err(-32);
        pcm.close_result = Err(-19);
        let trace = pcm.trace.clone();
        let mut stream = AlsaCaptureStream::start(pcm, plan(), GENERATION).unwrap();
        assert_eq!(stream.read_period(GENERATION), Ok(1));
        let error = stream.read_period(GENERATION).unwrap_err();
        assert_eq!(
            error,
            AlsaRunError {
                capture: Some(expected),
                cleanup: AlsaCleanup {
                    drop_errno: Some(-32),
                    close_errno: Some(-19)
                },
            }
        );
        assert_eq!(stream.read_period(GENERATION).unwrap_err(), error);
        assert_eq!(stream.stop().unwrap_err(), error);
        assert_eq!(stream.session().take().phase(), Phase::Stopped);
        assert_eq!(stream.session().take().pcm(), &[3; 6]);
        assert!(stream.session().take().dropouts().is_empty());
        assert_eq!(
            &*trace.borrow(),
            &[
                Call::Verify,
                Call::Negotiate,
                Call::Start,
                Call::Read,
                Call::Read,
                Call::Drop,
                Call::Close
            ]
        );
    }
}

#[test]
fn cap_failure_keeps_the_accepted_prefix_and_closes_the_pcm() {
    let mut pcm = FakePcm::new();
    pcm.reads = [(1, 3), (1, 5)].into();
    let trace = pcm.trace.clone();
    let mut stream =
        AlsaCaptureStream::start(pcm, source_with_capacity(V600, 6), GENERATION).unwrap();
    stream.read_period(GENERATION).unwrap();
    let error = stream.read_period(GENERATION).unwrap_err();
    assert_eq!(
        error.capture,
        Some(CaptureError::Take(TakeError::CapExceeded))
    );
    assert_eq!(stream.session().take().pcm(), &[3; 6]);
    assert_eq!(stream.stop().unwrap_err(), error);
    assert!(stream.take_pending_events().unwrap().is_empty());
    assert_eq!(&trace.borrow()[5..], &[Call::Drop, Call::Close]);
}

#[test]
fn cancel_disconnect_and_scope_exit_each_release_once() {
    for action in 0..3 {
        let mut pcm = FakePcm::new();
        pcm.reads = [(1, 3)].into();
        let trace = pcm.trace.clone();
        let mut stream = AlsaCaptureStream::start(pcm, plan(), GENERATION).unwrap();
        stream.read_period(GENERATION).unwrap();
        match action {
            0 => {
                stream.cancel().unwrap();
                stream.cancel().unwrap();
                assert_eq!(stream.session().take().phase(), Phase::Cancelled);
                assert!(stream.session().take().pcm().is_empty());
            }
            1 => {
                let error = stream.disconnect().unwrap_err();
                assert_eq!(error.capture, Some(CaptureError::Disconnected));
                assert_eq!(stream.disconnect().unwrap_err(), error);
                assert_eq!(stream.session().take().pcm(), &[3; 6]);
            }
            _ => {}
        }
        drop(stream);
        assert_eq!(
            &*trace.borrow(),
            &[
                Call::Verify,
                Call::Negotiate,
                Call::Start,
                Call::Read,
                Call::Drop,
                Call::Close
            ]
        );
    }
}

#[test]
fn stopped_fake_take_uses_exact_clock_non_overwriting_save() {
    let mut pcm = FakePcm::new();
    pcm.reads = [(1, 0x35)].into();
    let mut stream = AlsaCaptureStream::start(pcm, plan(), GENERATION).unwrap();
    stream.read_period(GENERATION).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fake.aiff");
    assert!(matches!(
        stream.session().take().save_aiff(&path),
        Err(TakeError::NotStopped)
    ));
    assert!(!path.exists());
    stream.stop().unwrap();
    let take = stream.session().take();
    assert!(matches!(
        take.save(&dir.path().join("fake.wav")),
        Err(TakeError::RationalClockNotWav {
            numerator: 78_125,
            denominator: 2
        })
    ));
    let saved = take.save_aiff(&path).unwrap();
    let audio = std::fs::read(&path).unwrap();
    let metadata = std::fs::read(&saved.sidecar_path).unwrap();
    assert_eq!(saved.frames, 1);
    assert!(matches!(
        take.save_aiff(&path),
        Err(TakeError::AlreadyExists)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), audio);
    assert_eq!(std::fs::read(&saved.sidecar_path).unwrap(), metadata);
    assert_eq!(take.pcm(), &[0x35; 6]);
}
