use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use super::*;
use crate::pod_audio::RecordRoute;
use crate::recording::Phase;
use crate::recording::adapters::linux_alsa::stream::{AlsaCleanup, AlsaStartFailure};
use crate::recording::adapters::linux_alsa::{
    ALSA_ACCESS_RW_INTERLEAVED, ALSA_FORMAT_S24_3LE, AlsaAdapterError, AlsaNegotiatedPcm,
    tests::source_with_capacity,
};
use crate::recording::capture::{CaptureError, ChannelMap};
use crate::rig::ProductProfile;

const ADDRESS: PcmAddress = PcmAddress {
    card: 3,
    device: 0,
    subdevice: 0,
};
const EVIDENCE: PreparedPcmEvidence = PreparedPcmEvidence {
    observed_handle: NonNull::dangling(),
    address: ADDRESS,
    route: VerifiedAlsaRoute {
        driver: "snd-usb-pod",
        usb_vendor: 0x0e41,
        usb_product: 0x4650,
        usb_interface: 0,
        capture_endpoint: 0x82,
        direct_hardware_pcm: true,
        instrument: ProductProfile::Variax600,
        record_send: RecordRoute::SendCleanGuitar,
        channel_map: ChannelMap { left: 0, right: 1 },
    },
};
const PARAMS: AlsaCaptureParams = AlsaCaptureParams {
    pcm: AlsaNegotiatedPcm {
        access: ALSA_ACCESS_RW_INTERLEAVED,
        format: ALSA_FORMAT_S24_3LE,
        channels: 2,
        rate_num: 78_125,
        rate_den: 2,
    },
    period_frames: 11,
    periods: 2,
    buffer_frames: 22,
};
const GENERATION: u64 = 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Call {
    Identity,
    Params,
    Nonblock,
    Start,
    Read,
    Drop,
    Close,
}

#[derive(Clone)]
struct Fake {
    visits: Rc<RefCell<Vec<Call>>>,
    addresses: RefCell<VecDeque<Option<PcmAddress>>>,
    params: Result<AlsaCaptureParams, i32>,
    nonblock: Result<(), i32>,
    start: Result<(), i32>,
    close: Result<(), i32>,
    reads: RefCell<VecDeque<isize>>,
}

impl Fake {
    fn new() -> Self {
        Self {
            visits: Rc::default(),
            addresses: RefCell::new([Some(ADDRESS), Some(ADDRESS)].into()),
            params: Ok(PARAMS),
            nonblock: Ok(()),
            start: Ok(()),
            close: Ok(()),
            reads: RefCell::default(),
        }
    }

    fn called(&self, handle: Handle, call: Call) {
        assert_eq!(handle, NonNull::dangling());
        self.visits.borrow_mut().push(call);
    }

    fn expect(&self, calls: &[Call]) {
        assert_eq!(self.visits.borrow().as_slice(), calls);
    }
}

impl NativeCalls for Fake {
    fn prepared_capture_address(&self, handle: Handle) -> Option<PcmAddress> {
        self.called(handle, Call::Identity);
        self.addresses
            .borrow_mut()
            .pop_front()
            .expect("unexpected query")
    }

    fn current_params(&self, handle: Handle) -> Result<AlsaCaptureParams, i32> {
        self.called(handle, Call::Params);
        self.params
    }

    fn nonblock(&self, handle: Handle) -> Result<(), i32> {
        self.called(handle, Call::Nonblock);
        self.nonblock
    }

    fn start(&self, handle: Handle) -> Result<(), i32> {
        self.called(handle, Call::Start);
        self.start
    }

    fn readi(&self, handle: Handle, frames: usize, buffer: &mut [u8]) -> isize {
        self.called(handle, Call::Read);
        assert_eq!((frames, buffer.len()), (11, 66));
        buffer.fill(0x42);
        self.reads
            .borrow_mut()
            .pop_front()
            .expect("unexpected read")
    }

    fn drop_pcm(&self, handle: Handle) -> Result<(), i32> {
        self.called(handle, Call::Drop);
        Ok(())
    }

    fn close(&self, handle: Handle) -> Result<(), i32> {
        self.called(handle, Call::Close);
        self.close
    }
}

fn start(
    fake: &Fake,
    evidence: PreparedPcmEvidence,
) -> Result<AlsaCaptureStream<impl AlsaCapturePcm + use<>>, AlsaStartError> {
    start_prepared(
        fake.clone(),
        NonNull::dangling(),
        evidence,
        source_with_capacity(Some(ProductProfile::Variax600), 1024),
        GENERATION,
    )
}

#[test]
fn rejects_wrong_address_or_driver_and_releases_the_taken_handle() {
    for evidence in [
        PreparedPcmEvidence {
            address: PcmAddress { card: 4, ..ADDRESS },
            ..EVIDENCE
        },
        PreparedPcmEvidence {
            route: VerifiedAlsaRoute {
                driver: "snd-usb-audio",
                ..EVIDENCE.route
            },
            ..EVIDENCE
        },
    ] {
        let fake = Fake::new();
        let error = start(&fake, evidence).err().unwrap();
        assert_eq!(error.cause, AlsaStartFailure::UnverifiedRoute);
        fake.expect(&[Call::Identity, Call::Close]);
    }

    let fake = Fake::new();
    let address_from_a_different_handle = NonNull::<u64>::dangling().cast();
    let error = start(
        &fake,
        PreparedPcmEvidence {
            observed_handle: address_from_a_different_handle,
            ..EVIDENCE
        },
    )
    .err()
    .unwrap();
    assert_eq!(error.cause, AlsaStartFailure::UnverifiedRoute);
    fake.expect(&[Call::Close]);

    let fake = Fake::new();
    let wrong_subdevice = PcmAddress {
        subdevice: 1,
        ..ADDRESS
    };
    let error = start(
        &fake,
        PreparedPcmEvidence {
            address: wrong_subdevice,
            ..EVIDENCE
        },
    )
    .err()
    .unwrap();
    assert_eq!(error.cause, AlsaStartFailure::UnverifiedRoute);
    fake.expect(&[Call::Close]);

    let fake = Fake::new();
    fake.addresses.replace([None].into());
    let error = start(&fake, EVIDENCE).err().unwrap();
    assert_eq!(error.cause, AlsaStartFailure::UnverifiedRoute);
    fake.expect(&[Call::Identity, Call::Close]);
}

#[test]
fn rechecks_prepared_address_immediately_before_start() {
    let fake = Fake::new();
    fake.addresses.replace([Some(ADDRESS), None].into());
    let error = start(&fake, EVIDENCE).err().unwrap();
    assert_eq!(error.cause, AlsaStartFailure::Start(-ENODEV));
    fake.expect(&[Call::Identity, Call::Params, Call::Identity, Call::Close]);
}

#[test]
fn rejects_native_format_and_rational_clock_before_start() {
    for (pcm, expected) in [
        (
            AlsaNegotiatedPcm {
                format: 6,
                ..PARAMS.pcm
            },
            AlsaAdapterError::UnsupportedFormat(6),
        ),
        (
            AlsaNegotiatedPcm {
                rate_num: 39_063,
                rate_den: 1,
                ..PARAMS.pcm
            },
            AlsaAdapterError::InvalidClock {
                numerator: 39_063,
                denominator: 1,
            },
        ),
    ] {
        let mut fake = Fake::new();
        fake.params = Ok(AlsaCaptureParams { pcm, ..PARAMS });
        let error = start(&fake, EVIDENCE).err().unwrap();
        assert_eq!(error.cause, AlsaStartFailure::Contract(expected));
        fake.expect(&[Call::Identity, Call::Params, Call::Close]);
    }
}

// Model the integer request and the two adjacent open intervals produced by
// libasound's directional setter. The POD ratden selects from its own list.
fn rate_request_contains(value: u32, direction: i32, (num, den): (u32, u32)) -> bool {
    match direction {
        -1 => (value - 1) * den < num && num < value * den,
        0 => num == value * den,
        1 => value * den < num && num < (value + 1) * den,
        _ => unreachable!(),
    }
}

#[test]
fn directional_rate_request_retains_the_pod_ratden_and_excludes_rounded_rates() {
    // The alternate fractional rate demonstrates why the readback is required
    // even when the directional setter succeeds.
    let candidates = [(39_062, 1), (78_125, 2), (156_249, 4), (39_063, 1)];
    for (value, direction, expected) in [
        (39_062, 1, [false, true, true, false]),
        (39_063, -1, [false, true, true, false]),
        (39_062, 0, [true, false, false, false]),
        (39_063, 0, [false, false, false, true]),
        (78_125, 0, [false, false, false, false]),
        (39_062, -1, [false, false, false, false]),
        (39_063, 1, [false, false, false, false]),
    ] {
        assert_eq!(
            candidates.map(|rate| rate_request_contains(value, direction, rate)),
            expected,
            "rate request ({value}, {direction})"
        );
    }
}

#[test]
fn prepared_capture_requires_exact_rational_readback_with_candidate_geometry() {
    let candidate = AlsaCaptureParams {
        period_frames: 625,
        periods: 4,
        buffer_frames: 2_500,
        ..PARAMS
    };
    let mut fake = Fake::new();
    fake.params = Ok(candidate);
    let mut stream = start(&fake, EVIDENCE).unwrap();
    stream.stop().unwrap();
    fake.expect(&[
        Call::Identity,
        Call::Params,
        Call::Identity,
        Call::Nonblock,
        Call::Start,
        Call::Drop,
        Call::Close,
    ]);

    for (numerator, denominator) in [
        (39_062, 1),
        (39_063, 1),
        (78_125, 1),
        (156_249, 4),
        (78_125, 0),
    ] {
        let mut fake = Fake::new();
        fake.params = Ok(AlsaCaptureParams {
            pcm: AlsaNegotiatedPcm {
                rate_num: numerator,
                rate_den: denominator,
                ..candidate.pcm
            },
            ..candidate
        });
        let error = start(&fake, EVIDENCE).err().unwrap();
        assert_eq!(
            error.cause,
            AlsaStartFailure::Contract(AlsaAdapterError::InvalidClock {
                numerator,
                denominator,
            })
        );
        fake.expect(&[Call::Identity, Call::Params, Call::Close]);
    }

    let mut fake = Fake::new();
    fake.params = Err(-22); // EINVAL when the rational getter has no pair.
    let error = start(&fake, EVIDENCE).err().unwrap();
    assert_eq!(error.cause, AlsaStartFailure::Negotiation(-22));
    fake.expect(&[Call::Identity, Call::Params, Call::Close]);
}

#[test]
fn makes_reads_nonblocking_and_closes_after_terminal_read() {
    let fake = Fake::new();
    fake.reads.replace([-(EAGAIN as isize), 1, -32].into());
    // Share the scripted fake state used by the adapter via the single move.
    let visits = fake.visits.clone();
    let mut stream = start_prepared(
        fake,
        NonNull::dangling(),
        PreparedPcmEvidence {
            route: VerifiedAlsaRoute {
                driver: "snd_usb_pod",
                ..EVIDENCE.route
            },
            ..EVIDENCE
        },
        source_with_capacity(Some(ProductProfile::Variax600), 1024),
        GENERATION,
    )
    .unwrap();
    assert_eq!(stream.read_period(GENERATION).unwrap(), 0);
    assert_eq!(stream.read_period(GENERATION).unwrap(), 1);
    let failure = stream.read_period(GENERATION).unwrap_err();
    assert_eq!(
        failure.capture,
        Some(CaptureError::AdapterDiscontinuity(-32))
    );
    assert_eq!(stream.session().take().phase(), Phase::Stopped);
    assert_eq!(stream.session().take().pcm(), &[0x42; 6]);
    drop(stream);
    assert_eq!(
        visits.borrow().as_slice(),
        &[
            Call::Identity,
            Call::Params,
            Call::Identity,
            Call::Nonblock,
            Call::Start,
            Call::Read,
            Call::Read,
            Call::Read,
            Call::Drop,
            Call::Close,
        ]
    );
}

#[test]
fn nonblock_and_start_failures_close_once_and_report_cleanup() {
    let mut fake = Fake::new();
    fake.nonblock = Err(-5);
    fake.close = Err(-19);
    let error = start(&fake, EVIDENCE).err().unwrap();
    assert_eq!(error.cause, AlsaStartFailure::Start(-5));
    assert_eq!(
        error.cleanup,
        AlsaCleanup {
            drop_errno: None,
            close_errno: Some(-19)
        }
    );
    fake.expect(&[
        Call::Identity,
        Call::Params,
        Call::Identity,
        Call::Nonblock,
        Call::Close,
    ]);

    let mut fake = Fake::new();
    fake.start = Err(-5);
    let error = start(&fake, EVIDENCE).err().unwrap();
    assert_eq!(error.cause, AlsaStartFailure::Start(-5));
    fake.expect(&[
        Call::Identity,
        Call::Params,
        Call::Identity,
        Call::Nonblock,
        Call::Start,
        Call::Drop,
        Call::Close,
    ]);
}
