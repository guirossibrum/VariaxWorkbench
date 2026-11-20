#![forbid(unsafe_code)]
//! Semantic request validation for one macOS Line 6 Driver 7.6.8 declaration.
//!
//! This describes source-declared constraints only. It is not an OS audio
//! adapter, a wire-format codec, or evidence that a device can stream.

/// The installer package scope for this source-derived contract.
pub const DRIVER_VERSION: &str = "7.6.8";
pub const DRIVER_PACKAGE_SHA256: &str =
    "48099370936bea11e09b9d8ee4828e2c9050dd1a1da1586828b7f5a72b559436";

pub const HD500X_VENDOR_ID: u16 = 0x0e41;
pub const HD500X_PRODUCT_ID: u16 = 0x4159;
pub const CONFIGURATION: u8 = 1;
pub const AUDIO_INTERFACE: u8 = 0;
pub const BULK_INTERFACE: u8 = 1;
pub const REQUESTED_ALTERNATE_SETTING: u8 = 1;
pub const INPUT_STREAMS: u8 = 1;
pub const OUTPUT_STREAMS: u8 = 1;
pub const CHANNELS_PER_STREAM: u8 = 2;
pub const SETUP_FORMAT_VALUE: u8 = 24;
pub const ALLOWED_SAMPLE_RATES_HZ: [u32; 4] = [44_100, 48_000, 88_200, 96_000];

/// Internal branch selected by this driver build. This name does not imply
/// physical equivalence with another product or driver family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InternalEngineBehavior {
    PodX3,
}

/// Semantic inputs accepted by the offline macOS 7.6.8 request validator.
/// The separate bulk interface is carried for identity separation only; no
/// bulk communication request or transport is implemented here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Request {
    pub vendor_id: u16,
    pub product_id: u16,
    pub configuration: u8,
    pub audio_interface: u8,
    pub bulk_interface: u8,
    pub requested_alternate_setting: u8,
    pub input_streams: u8,
    pub output_streams: u8,
    pub input_channels_per_stream: u8,
    pub output_channels_per_stream: u8,
    pub sample_rate_hz: u32,
    /// Driver setup argument. Kept separate from wire packing and encoding.
    pub setup_format_value: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestError {
    WrongIdentity,
    WrongConfiguration,
    WrongAudioInterface,
    WrongBulkInterface,
    UnsupportedAlternateSetting,
    UnsupportedStreamCounts,
    UnsupportedChannelCounts,
    UnsupportedSampleRate,
    UnsupportedSetupFormat,
}

impl std::fmt::Display for RequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::WrongIdentity => "request is not for the declared HD500X USB identity",
            Self::WrongConfiguration => "request does not use the declared USB configuration",
            Self::WrongAudioInterface => "request does not use the declared audio interface",
            Self::WrongBulkInterface => "request does not keep the bulk interface separate",
            Self::UnsupportedAlternateSetting => {
                "request does not use the source-declared alternate-setting value"
            }
            Self::UnsupportedStreamCounts => {
                "request does not match the source-declared input/output stream counts"
            }
            Self::UnsupportedChannelCounts => {
                "request does not match the source-declared channels per stream"
            }
            Self::UnsupportedSampleRate => "sample rate is outside the source-declared set",
            Self::UnsupportedSetupFormat => {
                "setup format value is outside the source-declared request predicate"
            }
        };
        f.write_str(message)
    }
}

impl std::error::Error for RequestError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HardwareStatus {
    /// Always returned by this pure offline contract; no state can enable open.
    UnverifiedAndLocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaticPlan {
    pub driver_version: &'static str,
    pub driver_package_sha256: &'static str,
    pub engine_behavior: InternalEngineBehavior,
    pub configuration: u8,
    pub audio_interface: u8,
    pub separate_bulk_interface: u8,
    pub requested_alternate_setting: u8,
    pub input_streams: u8,
    pub output_streams: u8,
    pub channels_per_stream: u8,
    pub requested_sample_rate_hz: u32,
    pub setup_format_value: u8,
}

impl StaticPlan {
    pub const fn hardware_status(self) -> HardwareStatus {
        HardwareStatus::UnverifiedAndLocked
    }

    pub const fn stream_opened(self) -> bool {
        false
    }

    /// Assess synthetic/offline statuses only. No endpoint or pipe identity is
    /// represented, and even all-synthetic success never means hardware-ready.
    pub fn assess_offline_provider(
        self,
        evidence: &OfflineProviderEvidence,
    ) -> Result<ProviderAssessment, EvidenceError> {
        validate_evidence_lengths(evidence)?;
        if let Some(state) =
            assess_one(evidence.alternate_setting, Stage::AlternateSetting, None, 0)
        {
            return Ok(state);
        }
        if let Some(state) = assess_ordered(&evidence.ordered_pipe_lookups, 2, Stage::PipeLookup) {
            return Ok(state);
        }
        if let Some(state) = assess_ordered(
            &evidence.ordered_transfer_preparations,
            2,
            Stage::TransferPreparation,
        ) {
            return Ok(state);
        }
        Ok(ProviderAssessment::SyntheticPrerequisitesReported)
    }
}

/// Each state is an offline fake/status value. It is never populated by an OS
/// call in this crate and must not be interpreted as physical evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OfflineResult {
    Pending,
    SyntheticSuccess,
    SyntheticFailure,
}

/// Ordered slots correspond only to the first and second generic pipe lookup;
/// they are deliberately not named capture/playback or assigned endpoints.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OfflineProviderEvidence {
    pub alternate_setting: OfflineResult,
    pub ordered_pipe_lookups: Vec<OfflineResult>,
    pub ordered_transfer_preparations: Vec<OfflineResult>,
}

impl Default for OfflineProviderEvidence {
    fn default() -> Self {
        Self {
            alternate_setting: OfflineResult::Pending,
            ordered_pipe_lookups: Vec::new(),
            ordered_transfer_preparations: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    AlternateSetting,
    PipeLookup,
    TransferPreparation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderAssessment {
    Pending {
        stage: Stage,
        completed: usize,
    },
    SyntheticFailure {
        stage: Stage,
        ordinal: Option<usize>,
    },
    /// Software-only prerequisites were represented as successes by a test.
    /// This still is not a live-ready/openable route.
    SyntheticPrerequisitesReported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvidenceError {
    TooManyPipeLookupResults,
    TooManyTransferPreparationResults,
}

impl std::fmt::Display for EvidenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooManyPipeLookupResults => {
                f.write_str("expected at most two ordered pipe results")
            }
            Self::TooManyTransferPreparationResults => {
                f.write_str("expected at most two ordered transfer-preparation results")
            }
        }
    }
}

impl std::error::Error for EvidenceError {}

pub fn validate(request: Request) -> Result<StaticPlan, RequestError> {
    if (request.vendor_id, request.product_id) != (HD500X_VENDOR_ID, HD500X_PRODUCT_ID) {
        return Err(RequestError::WrongIdentity);
    }
    if request.configuration != CONFIGURATION {
        return Err(RequestError::WrongConfiguration);
    }
    if request.audio_interface != AUDIO_INTERFACE {
        return Err(RequestError::WrongAudioInterface);
    }
    if request.bulk_interface != BULK_INTERFACE {
        return Err(RequestError::WrongBulkInterface);
    }
    if request.requested_alternate_setting != REQUESTED_ALTERNATE_SETTING {
        return Err(RequestError::UnsupportedAlternateSetting);
    }
    if request.input_streams != INPUT_STREAMS || request.output_streams != OUTPUT_STREAMS {
        return Err(RequestError::UnsupportedStreamCounts);
    }
    if request.input_channels_per_stream != CHANNELS_PER_STREAM
        || request.output_channels_per_stream != CHANNELS_PER_STREAM
    {
        return Err(RequestError::UnsupportedChannelCounts);
    }
    if !ALLOWED_SAMPLE_RATES_HZ.contains(&request.sample_rate_hz) {
        return Err(RequestError::UnsupportedSampleRate);
    }
    if request.setup_format_value != SETUP_FORMAT_VALUE {
        return Err(RequestError::UnsupportedSetupFormat);
    }
    Ok(StaticPlan {
        driver_version: DRIVER_VERSION,
        driver_package_sha256: DRIVER_PACKAGE_SHA256,
        engine_behavior: InternalEngineBehavior::PodX3,
        configuration: CONFIGURATION,
        audio_interface: AUDIO_INTERFACE,
        separate_bulk_interface: BULK_INTERFACE,
        requested_alternate_setting: REQUESTED_ALTERNATE_SETTING,
        input_streams: INPUT_STREAMS,
        output_streams: OUTPUT_STREAMS,
        channels_per_stream: CHANNELS_PER_STREAM,
        requested_sample_rate_hz: request.sample_rate_hz,
        setup_format_value: SETUP_FORMAT_VALUE,
    })
}

fn validate_evidence_lengths(evidence: &OfflineProviderEvidence) -> Result<(), EvidenceError> {
    if evidence.ordered_pipe_lookups.len() > 2 {
        return Err(EvidenceError::TooManyPipeLookupResults);
    }
    if evidence.ordered_transfer_preparations.len() > 2 {
        return Err(EvidenceError::TooManyTransferPreparationResults);
    }
    Ok(())
}

fn assess_one(
    result: OfflineResult,
    stage: Stage,
    ordinal: Option<usize>,
    completed: usize,
) -> Option<ProviderAssessment> {
    match result {
        OfflineResult::Pending => Some(ProviderAssessment::Pending { stage, completed }),
        OfflineResult::SyntheticSuccess => None,
        OfflineResult::SyntheticFailure => {
            Some(ProviderAssessment::SyntheticFailure { stage, ordinal })
        }
    }
}

fn assess_ordered(
    results: &[OfflineResult],
    expected: usize,
    stage: Stage,
) -> Option<ProviderAssessment> {
    for (ordinal, result) in results.iter().copied().enumerate() {
        if let Some(assessment) = assess_one(result, stage, Some(ordinal), ordinal) {
            return Some(assessment);
        }
    }
    if results.len() < expected {
        Some(ProviderAssessment::Pending {
            stage,
            completed: results.len(),
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_request() -> Request {
        Request {
            vendor_id: HD500X_VENDOR_ID,
            product_id: HD500X_PRODUCT_ID,
            configuration: CONFIGURATION,
            audio_interface: AUDIO_INTERFACE,
            bulk_interface: BULK_INTERFACE,
            requested_alternate_setting: REQUESTED_ALTERNATE_SETTING,
            input_streams: INPUT_STREAMS,
            output_streams: OUTPUT_STREAMS,
            input_channels_per_stream: CHANNELS_PER_STREAM,
            output_channels_per_stream: CHANNELS_PER_STREAM,
            sample_rate_hz: 48_000,
            setup_format_value: SETUP_FORMAT_VALUE,
        }
    }

    fn successful_evidence() -> OfflineProviderEvidence {
        OfflineProviderEvidence {
            alternate_setting: OfflineResult::SyntheticSuccess,
            ordered_pipe_lookups: vec![
                OfflineResult::SyntheticSuccess,
                OfflineResult::SyntheticSuccess,
            ],
            ordered_transfer_preparations: vec![
                OfflineResult::SyntheticSuccess,
                OfflineResult::SyntheticSuccess,
            ],
        }
    }

    #[test]
    fn independent_source_declared_sample_rate_set_is_accepted_and_neighbors_rejected() {
        let independently_declared_rates = [44_100, 48_000, 88_200, 96_000];
        assert_eq!(ALLOWED_SAMPLE_RATES_HZ, independently_declared_rates);
        for rate in independently_declared_rates {
            assert_eq!(
                validate(Request {
                    sample_rate_hz: rate,
                    ..synthetic_request()
                })
                .unwrap()
                .requested_sample_rate_hz,
                rate
            );
        }
        for rate in [
            0,
            44_099,
            44_101,
            47_999,
            48_001,
            88_199,
            88_201,
            95_999,
            96_001,
            u32::MAX,
        ] {
            assert_eq!(
                validate(Request {
                    sample_rate_hz: rate,
                    ..synthetic_request()
                }),
                Err(RequestError::UnsupportedSampleRate),
                "rate {rate}"
            );
        }
    }

    #[test]
    fn setup_format_predicate_is_exact_and_separate_from_sample_rate() {
        assert!(
            validate(Request {
                setup_format_value: 24,
                ..synthetic_request()
            })
            .is_ok()
        );
        for value in [0, 1, 23, 25, u8::MAX] {
            assert_eq!(
                validate(Request {
                    setup_format_value: value,
                    ..synthetic_request()
                }),
                Err(RequestError::UnsupportedSetupFormat)
            );
        }
    }

    #[test]
    fn wrong_identity_interface_alternate_stream_or_channel_request_is_rejected() {
        let wrong = [
            (
                Request {
                    vendor_id: 1,
                    ..synthetic_request()
                },
                RequestError::WrongIdentity,
            ),
            (
                Request {
                    product_id: 1,
                    ..synthetic_request()
                },
                RequestError::WrongIdentity,
            ),
            (
                Request {
                    configuration: 2,
                    ..synthetic_request()
                },
                RequestError::WrongConfiguration,
            ),
            (
                Request {
                    audio_interface: 1,
                    ..synthetic_request()
                },
                RequestError::WrongAudioInterface,
            ),
            (
                Request {
                    bulk_interface: 0,
                    ..synthetic_request()
                },
                RequestError::WrongBulkInterface,
            ),
            (
                Request {
                    requested_alternate_setting: 0,
                    ..synthetic_request()
                },
                RequestError::UnsupportedAlternateSetting,
            ),
            (
                Request {
                    input_streams: 0,
                    ..synthetic_request()
                },
                RequestError::UnsupportedStreamCounts,
            ),
            (
                Request {
                    output_streams: 2,
                    ..synthetic_request()
                },
                RequestError::UnsupportedStreamCounts,
            ),
            (
                Request {
                    input_channels_per_stream: 1,
                    ..synthetic_request()
                },
                RequestError::UnsupportedChannelCounts,
            ),
            (
                Request {
                    output_channels_per_stream: 3,
                    ..synthetic_request()
                },
                RequestError::UnsupportedChannelCounts,
            ),
        ];
        for (request, expected) in wrong {
            assert_eq!(validate(request), Err(expected));
        }
    }

    #[test]
    fn static_plan_preserves_unknowns_and_never_becomes_openable() {
        let plan = validate(synthetic_request()).unwrap();
        assert_eq!(plan.hardware_status(), HardwareStatus::UnverifiedAndLocked);
        assert!(!plan.stream_opened());
        assert_eq!(
            plan.assess_offline_provider(&OfflineProviderEvidence::default()),
            Ok(ProviderAssessment::Pending {
                stage: Stage::AlternateSetting,
                completed: 0
            })
        );
        assert_eq!(
            plan.assess_offline_provider(&successful_evidence()),
            Ok(ProviderAssessment::SyntheticPrerequisitesReported)
        );
        assert_eq!(plan.hardware_status(), HardwareStatus::UnverifiedAndLocked);
        assert!(!plan.stream_opened());
    }

    #[test]
    fn missing_and_failed_selection_and_zero_one_two_pipe_results_are_explicit() {
        let plan = validate(synthetic_request()).unwrap();
        let mut evidence = successful_evidence();
        evidence.alternate_setting = OfflineResult::Pending;
        assert_eq!(
            plan.assess_offline_provider(&evidence),
            Ok(ProviderAssessment::Pending {
                stage: Stage::AlternateSetting,
                completed: 0
            })
        );
        evidence.alternate_setting = OfflineResult::SyntheticFailure;
        assert_eq!(
            plan.assess_offline_provider(&evidence),
            Ok(ProviderAssessment::SyntheticFailure {
                stage: Stage::AlternateSetting,
                ordinal: None
            })
        );
        evidence.alternate_setting = OfflineResult::SyntheticSuccess;
        evidence.ordered_pipe_lookups.clear();
        assert_eq!(
            plan.assess_offline_provider(&evidence),
            Ok(ProviderAssessment::Pending {
                stage: Stage::PipeLookup,
                completed: 0
            })
        );
        evidence
            .ordered_pipe_lookups
            .push(OfflineResult::SyntheticSuccess);
        evidence.ordered_transfer_preparations.clear();
        assert_eq!(
            plan.assess_offline_provider(&evidence),
            Ok(ProviderAssessment::Pending {
                stage: Stage::PipeLookup,
                completed: 1
            })
        );
        evidence
            .ordered_pipe_lookups
            .push(OfflineResult::SyntheticSuccess);
        assert_eq!(
            plan.assess_offline_provider(&evidence),
            Ok(ProviderAssessment::Pending {
                stage: Stage::TransferPreparation,
                completed: 0
            })
        );
    }

    #[test]
    fn explicit_pending_preserves_prefix_and_matches_omitted_status_in_ordered_stages() {
        let plan = validate(synthetic_request()).unwrap();
        let pending_pipe = |pipe_results: Vec<OfflineResult>| OfflineProviderEvidence {
            alternate_setting: OfflineResult::SyntheticSuccess,
            ordered_pipe_lookups: pipe_results,
            ordered_transfer_preparations: vec![OfflineResult::SyntheticFailure; 2],
        };
        let omitted_pipe_pending = pending_pipe(vec![OfflineResult::SyntheticSuccess]);
        let explicit_second_pipe_pending = pending_pipe(vec![
            OfflineResult::SyntheticSuccess,
            OfflineResult::Pending,
        ]);
        let expected_pipe_pending = Ok(ProviderAssessment::Pending {
            stage: Stage::PipeLookup,
            completed: 1,
        });
        assert_eq!(
            plan.assess_offline_provider(&omitted_pipe_pending),
            expected_pipe_pending
        );
        assert_eq!(
            plan.assess_offline_provider(&explicit_second_pipe_pending),
            expected_pipe_pending
        );
        assert_eq!(
            plan.assess_offline_provider(&explicit_second_pipe_pending),
            expected_pipe_pending,
            "repeated assessments are stateless"
        );

        let explicit_first_pipe_pending = pending_pipe(vec![
            OfflineResult::Pending,
            OfflineResult::SyntheticSuccess,
        ]);
        assert_eq!(
            plan.assess_offline_provider(&explicit_first_pipe_pending),
            Ok(ProviderAssessment::Pending {
                stage: Stage::PipeLookup,
                completed: 0,
            })
        );

        let pending_transfer = |transfer_results: Vec<OfflineResult>| OfflineProviderEvidence {
            alternate_setting: OfflineResult::SyntheticSuccess,
            ordered_pipe_lookups: vec![OfflineResult::SyntheticSuccess; 2],
            ordered_transfer_preparations: transfer_results,
        };
        let omitted_transfer_pending = pending_transfer(vec![OfflineResult::SyntheticSuccess]);
        let explicit_second_transfer_pending = pending_transfer(vec![
            OfflineResult::SyntheticSuccess,
            OfflineResult::Pending,
        ]);
        let expected_transfer_pending = Ok(ProviderAssessment::Pending {
            stage: Stage::TransferPreparation,
            completed: 1,
        });
        assert_eq!(
            plan.assess_offline_provider(&omitted_transfer_pending),
            expected_transfer_pending
        );
        assert_eq!(
            plan.assess_offline_provider(&explicit_second_transfer_pending),
            expected_transfer_pending
        );
        let explicit_first_transfer_pending = pending_transfer(vec![
            OfflineResult::Pending,
            OfflineResult::SyntheticSuccess,
        ]);
        assert_eq!(
            plan.assess_offline_provider(&explicit_first_transfer_pending),
            Ok(ProviderAssessment::Pending {
                stage: Stage::TransferPreparation,
                completed: 0,
            })
        );

        let mut earlier_stage_pending = explicit_second_transfer_pending.clone();
        earlier_stage_pending.ordered_pipe_lookups[1] = OfflineResult::Pending;
        assert_eq!(
            plan.assess_offline_provider(&earlier_stage_pending),
            expected_pipe_pending
        );
        earlier_stage_pending.alternate_setting = OfflineResult::Pending;
        assert_eq!(
            plan.assess_offline_provider(&earlier_stage_pending),
            Ok(ProviderAssessment::Pending {
                stage: Stage::AlternateSetting,
                completed: 0,
            })
        );

        let mut earlier_failure = explicit_second_transfer_pending;
        earlier_failure.ordered_pipe_lookups[1] = OfflineResult::SyntheticFailure;
        assert_eq!(
            plan.assess_offline_provider(&earlier_failure),
            Ok(ProviderAssessment::SyntheticFailure {
                stage: Stage::PipeLookup,
                ordinal: Some(1),
            })
        );
    }

    #[test]
    fn ordered_pipe_and_transfer_failures_are_deterministic_and_do_not_retain_partial_state() {
        let plan = validate(synthetic_request()).unwrap();
        let mut evidence = successful_evidence();
        evidence.ordered_pipe_lookups[0] = OfflineResult::SyntheticFailure;
        assert_eq!(
            plan.assess_offline_provider(&evidence),
            Ok(ProviderAssessment::SyntheticFailure {
                stage: Stage::PipeLookup,
                ordinal: Some(0)
            })
        );
        evidence.ordered_pipe_lookups[0] = OfflineResult::SyntheticSuccess;
        evidence.ordered_pipe_lookups[1] = OfflineResult::SyntheticFailure;
        assert_eq!(
            plan.assess_offline_provider(&evidence),
            Ok(ProviderAssessment::SyntheticFailure {
                stage: Stage::PipeLookup,
                ordinal: Some(1)
            })
        );
        evidence.ordered_pipe_lookups[1] = OfflineResult::SyntheticSuccess;
        evidence.ordered_transfer_preparations[0] = OfflineResult::SyntheticFailure;
        let first = plan.assess_offline_provider(&evidence);
        assert_eq!(
            first,
            Ok(ProviderAssessment::SyntheticFailure {
                stage: Stage::TransferPreparation,
                ordinal: Some(0)
            })
        );
        assert_eq!(plan.assess_offline_provider(&evidence), first);
        evidence.ordered_transfer_preparations[0] = OfflineResult::SyntheticSuccess;
        evidence.ordered_transfer_preparations[1] = OfflineResult::SyntheticFailure;
        assert_eq!(
            plan.assess_offline_provider(&evidence),
            Ok(ProviderAssessment::SyntheticFailure {
                stage: Stage::TransferPreparation,
                ordinal: Some(1)
            })
        );
        assert_eq!(plan.hardware_status(), HardwareStatus::UnverifiedAndLocked);
        assert!(!plan.stream_opened());
    }

    #[test]
    fn provider_status_rejects_extra_pipe_results() {
        let plan = validate(synthetic_request()).unwrap();
        let mut evidence = successful_evidence();
        evidence
            .ordered_pipe_lookups
            .push(OfflineResult::SyntheticSuccess);
        assert_eq!(
            plan.assess_offline_provider(&evidence),
            Err(EvidenceError::TooManyPipeLookupResults)
        );
        evidence.ordered_pipe_lookups.truncate(2);
        evidence
            .ordered_transfer_preparations
            .push(OfflineResult::SyntheticSuccess);
        assert_eq!(
            plan.assess_offline_provider(&evidence),
            Err(EvidenceError::TooManyTransferPreparationResults)
        );
    }
}
