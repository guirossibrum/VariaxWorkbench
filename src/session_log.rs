//! Local Workbench session diagnostics. Producers submit typed, redacted facts.
//! The file worker owns serialization, writes, retention and explicit exports.

mod profile;
mod sink;
#[cfg(test)]
pub(crate) mod tests;

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;

pub use profile::{DEFAULT_LEVEL, Severity};
pub use sink::{
    Export, LogStatus, SessionGuard, SessionLog, SinkState, current, export_current, start, status,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Endpoint {
    Workbench,
    Variax,
    Podxt,
    Hd500x,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Disposition {
    Accepted,
    Queued,
    Coalesced,
    Blocked,
    Cancelled,
    Applied,
    Failed,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Control {
    HdOpenFile,
    HdEffectEnabled,
    HdEffectSlot,
    PodxtChannel,
    PodxtControl,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UiAction {
    Button,
    Switch { old: bool, new: bool },
    Dial { old: i32, new: i32 },
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PodxtSyncReason {
    Requested,
    PendingEdits,
    NameDifference,
    ControlDifference,
    VolatileDifference,
    OpaqueDifference,
    ConfirmationRequired,
    Confirmed,
    Cancelled,
    Expired,
    RetainedDraft,
    Verified,
    Failed,
    Superseded,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PodxtSyncEvidence {
    None,
    EditorQueue,
    CachedSlot,
    AddressedSlot,
    VerifiedPlaying,
    CheckedRelease,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PodxtDraftOrigin {
    DeviceBuffer,
    EditorEdit,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct PodxtChannel(u8);

impl PodxtChannel {
    /// Zero-based protocol address.
    pub const fn new(address: u8) -> Option<Self> {
        if address < 128 {
            Some(Self(address))
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CcClass {
    Named,
    Unknown,
    Volatile,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Difference {
    Cc {
        number: u8,
        playing: u8,
        saved: u8,
        class: CcClass,
    },
    OpaqueBits {
        offset: u8,
        changed_bits: u8,
    },
}

/// A single bounded control comparison. The caller may emit multiple facts
/// under one operation. Opaque data is represented by offsets and changed
/// bit positions only.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(transparent)]
pub struct ControlDifference(Difference);

impl ControlDifference {
    pub const fn cc(number: u8, playing: u8, saved: u8, class: CcClass) -> Option<Self> {
        if number < 128 && playing < 128 && saved < 128 && playing != saved {
            Some(Self(Difference::Cc {
                number,
                playing,
                saved,
                class,
            }))
        } else {
            None
        }
    }

    pub const fn opaque_bits(offset: u8, changed_bits: u8) -> Option<Self> {
        if offset >= 23 && offset <= 38 && changed_bits != 0 {
            Some(Self(Difference::OpaqueBits {
                offset,
                changed_bits,
            }))
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Serialize)]
#[serde(tag = "category", rename_all = "snake_case")]
enum Event {
    SessionStarted,
    SessionFinished,
    QueueOverflow {
        dropped: u64,
    },
    SizeLimit,
    Ui {
        control: Control,
        action: UiAction,
        disposition: Disposition,
    },
    PodxtSync {
        source: Option<PodxtChannel>,
        target: PodxtChannel,
        reason: PodxtSyncReason,
        evidence: PodxtSyncEvidence,
        difference: Option<ControlDifference>,
        #[serde(skip_serializing_if = "Option::is_none")]
        draft_origin: Option<PodxtDraftOrigin>,
    },
    Transport {
        #[serde(serialize_with = "serialize_observation")]
        observation: line6_devices::session::Observation,
    },
    CheckedRelease {
        success: bool,
        duration_ms: u64,
    },
    OperationFinished {
        success: bool,
    },
    GuiStartupFailed,
}

fn serialize_observation<S: serde::Serializer>(
    observation: &line6_devices::session::Observation,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeStruct;

    let mut record = serializer.serialize_struct("TransportObservation", 4)?;
    // The upstream enum variants are closed and carry no arguments or payloads.
    record.serialize_field("stage", &format!("{:?}", observation.stage))?;
    record.serialize_field(
        "request",
        &observation.request.map(|request| format!("{request:?}")),
    )?;
    record.serialize_field("outcome", &format!("{:?}", observation.outcome))?;
    record.serialize_field("duration_ms", &observation.duration.map(millis))?;
    record.end()
}

#[derive(Clone, Copy, Serialize)]
struct Record {
    unix_ms: u64,
    severity: Severity,
    endpoint: Endpoint,
    operation: Option<u64>,
    #[serde(flatten)]
    event: Event,
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn unix_ms() -> u64 {
    millis(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default(),
    )
}

static NEXT_OPERATION: AtomicU64 = AtomicU64::new(1);

/// Carry this handle through queues, confirmations and worker results to keep
/// records correlated. Creating an operation never opens a log or a device.
#[derive(Clone)]
pub struct Operation {
    sink: Option<SessionLog>,
    id: u64,
    endpoint: Endpoint,
}

impl Operation {
    pub fn begin(endpoint: Endpoint) -> Self {
        Self::new(current().cloned(), endpoint)
    }

    pub fn with_log(log: &SessionLog, endpoint: Endpoint) -> Self {
        Self::new(Some(log.clone()), endpoint)
    }

    fn new(sink: Option<SessionLog>, endpoint: Endpoint) -> Self {
        Self {
            sink,
            id: NEXT_OPERATION.fetch_add(1, Ordering::Relaxed),
            endpoint,
        }
    }

    fn record(&self, severity: Severity, event: Event) {
        if let Some(sink) = &self.sink {
            sink.record(severity, self.endpoint, Some(self.id), event);
        }
    }

    pub fn ui(&self, control: Control, action: UiAction, disposition: Disposition) {
        self.record(
            Severity::Trace,
            Event::Ui {
                control,
                action,
                disposition,
            },
        );
    }

    pub fn podxt_sync(
        &self,
        source: Option<PodxtChannel>,
        target: PodxtChannel,
        reason: PodxtSyncReason,
        evidence: PodxtSyncEvidence,
        difference: Option<ControlDifference>,
    ) {
        let severity = match reason {
            PodxtSyncReason::Requested => Severity::Trace,
            PodxtSyncReason::Verified | PodxtSyncReason::RetainedDraft => Severity::Info,
            PodxtSyncReason::Failed => Severity::Error,
            _ => Severity::Debug,
        };
        self.record(
            severity,
            Event::PodxtSync {
                source,
                target,
                reason,
                evidence,
                difference,
                draft_origin: None,
            },
        );
    }

    /// Record a retained draft after the first checked release and before
    /// continuing recall. Keep the same operation handle through continuation.
    pub fn podxt_retained_draft(
        &self,
        source: PodxtChannel,
        target: PodxtChannel,
        origin: PodxtDraftOrigin,
    ) {
        self.record(
            Severity::Info,
            Event::PodxtSync {
                source: Some(source),
                target,
                reason: PodxtSyncReason::RetainedDraft,
                evidence: PodxtSyncEvidence::CheckedRelease,
                difference: None,
                draft_origin: Some(origin),
            },
        );
    }

    pub fn finished(&self, success: bool) {
        self.record(
            if success {
                Severity::Info
            } else {
                Severity::Error
            },
            Event::OperationFinished { success },
        );
    }

    pub fn transport(&self, observation: line6_devices::session::Observation) {
        use line6_devices::session::Outcome;
        let severity = match observation.outcome {
            Outcome::Failed => Severity::Error,
            Outcome::Timeout | Outcome::OrphanQuiet => Severity::Warn,
            _ => Severity::Trace,
        };
        self.record(severity, Event::Transport { observation });
    }

    /// Call with the result of the existing checked release.
    pub fn checked_release(&self, success: bool, duration: Duration) {
        self.record(
            if success {
                Severity::Trace
            } else {
                Severity::Error
            },
            Event::CheckedRelease {
                success,
                duration_ms: millis(duration),
            },
        );
    }
}

impl line6_devices::session::Log for Operation {
    fn line(&mut self, _: String) {}

    fn observing(&self) -> bool {
        self.sink.as_ref().is_some_and(SessionLog::accepting)
    }

    fn observe(&mut self, observation: line6_devices::session::Observation) {
        self.transport(observation);
    }
}
