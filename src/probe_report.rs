#![forbid(unsafe_code)]
//! Sanitized Device check report for stages 1 and 2.
//!
//! Every token is chosen from a fixed list. The renderer has no place for a
//! path, serial number, preset name, hostname or username.

use crate::identity_stage::{
    BRIDGE_PRODUCT, BRIDGE_VERSION, CanonicalIdentity, GUITAR_PRODUCT, GUITAR_VERSION,
    IdentityFailure, IdentityRole,
};
use std::io::Write;
use std::path::Path;

pub const SCHEMA: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportMode {
    Simulation,
    ThisComputer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportDecision {
    None,
    Yes,
    No,
    Cancel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportStatus {
    NotRun,
    Passed,
    Stopped,
    Locked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportStop {
    None,
    Declined,
    Cancelled,
    Failed,
    Identity(IdentityFailure),
}

/// The gate a report line may name. A free-form label cannot be passed in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportGate {
    OsDiscovery,
    IdentityRequest,
}

/// An identity token copied from a decoded product and version.
/// Passing stage 2 does not select a token by itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportIdentity {
    None,
    PodxtLive301,
    Variax600310,
}

impl ReportIdentity {
    pub fn from_canonical(identity: Option<CanonicalIdentity>) -> Self {
        let Some(identity) = identity else {
            return Self::None;
        };
        Self::from_observed(identity.role, identity.product_id, identity.version)
    }

    pub fn from_observed(role: IdentityRole, product_id: u32, version: &str) -> Self {
        match (role, product_id, version) {
            (IdentityRole::Bridge, BRIDGE_PRODUCT, BRIDGE_VERSION) => Self::PodxtLive301,
            (IdentityRole::Guitar, GUITAR_PRODUCT, GUITAR_VERSION) => Self::Variax600310,
            _ => Self::None,
        }
    }

    const fn token(self) -> &'static str {
        match self {
            Self::None => "",
            Self::PodxtLive301 => "bridge=podxt-live-3.01",
            Self::Variax600310 => "guitar=variax-600-3.10",
        }
    }
}

/// Whether the identity exchange put the connection back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportCleanup {
    NotRun,
    Ok,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StageFact {
    pub stage: u8,
    pub gate: ReportGate,
    pub decision: ReportDecision,
    pub status: ReportStatus,
    pub stop: ReportStop,
    pub cleanup: ReportCleanup,
    pub bridge: ReportIdentity,
    pub guitar: ReportIdentity,
}

/// Which live gates this binary can run. The report copies the value it is
/// given; it does not invent an open gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenGates {
    None,
    G1,
    G2,
    G1AndG2,
    G1ThroughG3,
}

impl OpenGates {
    pub const fn token(self) -> &'static str {
        match self {
            Self::None => "gates-open=none",
            Self::G1 => "gates-open=G1",
            Self::G2 => "gates-open=G2",
            Self::G1AndG2 => "gates-open=G1,G2",
            Self::G1ThroughG3 => "gates-open=G1,G2,G3",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SanitizedFacts {
    pub mode: ReportMode,
    pub gates: OpenGates,
    pub stages: [StageFact; 2],
}

const TOKENS: &[&str] = &[
    "device-check",
    "schema=2",
    "mode=simulation",
    "mode=this-computer",
    "gates-open=none",
    "gates-open=G1",
    "gates-open=G2",
    "gates-open=G1,G2",
    "gates-open=G1,G2,G3",
    "writes=none",
    "stage=1",
    "stage=2",
    "gate=G1",
    "gate=G2",
    "cleanup=not-run",
    "cleanup=ok",
    "cleanup=failed",
    "decision=none",
    "decision=yes",
    "decision=no",
    "decision=cancel",
    "status=not-run",
    "status=passed",
    "status=stopped",
    "status=locked",
    "stop=none",
    "stop=declined",
    "stop=cancelled",
    "stop=failed",
    "stop=unknown",
    "stop=ambiguous",
    "stop=inconsistent",
    "stop=timed-out",
    "stop=cleanup-failed",
    "stop=detach-failed",
    "stop=claim-failed",
    "stop=alternate-failed",
    "stop=open-failed",
    "stop=access-denied",
    "stop=send-failed",
    "stop=read-failed",
    "bridge=none",
    "bridge=podxt-live-3.01",
    "guitar=none",
    "guitar=variax-600-3.10",
];

fn decision_token(decision: ReportDecision) -> &'static str {
    match decision {
        ReportDecision::None => "decision=none",
        ReportDecision::Yes => "decision=yes",
        ReportDecision::No => "decision=no",
        ReportDecision::Cancel => "decision=cancel",
    }
}

fn status_token(status: ReportStatus) -> &'static str {
    match status {
        ReportStatus::NotRun => "status=not-run",
        ReportStatus::Passed => "status=passed",
        ReportStatus::Stopped => "status=stopped",
        ReportStatus::Locked => "status=locked",
    }
}

fn gate_token(gate: ReportGate) -> &'static str {
    match gate {
        ReportGate::OsDiscovery => "gate=G1",
        ReportGate::IdentityRequest => "gate=G2",
    }
}

fn cleanup_token(cleanup: ReportCleanup) -> &'static str {
    match cleanup {
        ReportCleanup::NotRun => "cleanup=not-run",
        ReportCleanup::Ok => "cleanup=ok",
        ReportCleanup::Failed => "cleanup=failed",
    }
}

fn identity_token(kind: ReportIdentity, bridge: bool) -> &'static str {
    match kind {
        ReportIdentity::PodxtLive301 if bridge => kind.token(),
        ReportIdentity::Variax600310 if !bridge => kind.token(),
        _ => {
            if bridge {
                "bridge=none"
            } else {
                "guitar=none"
            }
        }
    }
}

fn stop_token(stop: ReportStop) -> &'static str {
    match stop {
        ReportStop::None => "stop=none",
        ReportStop::Declined => "stop=declined",
        ReportStop::Cancelled => "stop=cancelled",
        ReportStop::Failed => "stop=failed",
        ReportStop::Identity(failure) => failure.stop_token(),
    }
}

pub fn render(facts: &SanitizedFacts) -> String {
    let mode = match facts.mode {
        ReportMode::Simulation => "mode=simulation",
        ReportMode::ThisComputer => "mode=this-computer",
    };
    let mut lines = vec![format!(
        "device-check schema={SCHEMA} {mode} {} writes=none",
        facts.gates.token()
    )];
    for stage in facts.stages {
        lines.push(format!(
            "stage={} {} {} {} {} {} {} {}",
            stage.stage,
            gate_token(stage.gate),
            decision_token(stage.decision),
            status_token(stage.status),
            stop_token(stage.stop),
            cleanup_token(stage.cleanup),
            identity_token(stage.bridge, true),
            identity_token(stage.guitar, false),
        ));
    }
    lines.join("\n")
}

pub fn allowlisted(text: &str) -> bool {
    text.split_whitespace().all(|token| TOKENS.contains(&token))
}

/// Suggested file name for a saved Device check report:
/// `device-check-report-YYMMDDHHMM.txt` in UTC. Saves never overwrite, so each
/// new report gets a new name instead of colliding with the last one.
pub fn report_file_name(unix_seconds: u64) -> String {
    let days = (unix_seconds / 86_400) as i64;
    let rest = unix_seconds % 86_400;
    let (hour, minute) = (rest / 3600, rest % 3600 / 60);
    // Civil-from-days (H. Hinnant), proleptic Gregorian.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "device-check-report-{:02}{month:02}{day:02}{hour:02}{minute:02}.txt",
        year.rem_euclid(100)
    )
}

/// [`report_file_name`] for the current time.
pub fn report_file_name_now() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    report_file_name(seconds)
}

/// Atomic, non-overwriting save. Failure text is a fixed phrase, so a personal
/// path cannot leak through it. Saving does not run a device stage.
pub fn save_sanitized_report(path: &Path, text: &str) -> Result<(), &'static str> {
    if !allowlisted(text) {
        return Err("report could not be saved");
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|_| "report could not be saved")?;
    temporary
        .write_all(text.as_bytes())
        .map_err(|_| "report could not be saved")?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|_| "report could not be saved")?;
    match temporary.persist_noclobber(path) {
        Ok(_) => Ok(()),
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => Err(
            "a report with that name already exists, so nothing was overwritten; save under another name",
        ),
        Err(_) => Err("report could not be saved"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity_stage::{
        BRIDGE_PRODUCT, BRIDGE_VERSION, CanonicalIdentity, GUITAR_PRODUCT, GUITAR_VERSION,
        IdentityFailure, IdentityRole,
    };

    #[test]
    fn report_file_name_carries_the_utc_minute() {
        // 2026-11-23 19:42:59 UTC.
        assert_eq!(
            report_file_name(1_795_462_979),
            "device-check-report-2611231942.txt"
        );
        assert_eq!(report_file_name(0), "device-check-report-7001010000.txt");
        // 2000-02-29 23:59 UTC (leap day).
        assert_eq!(
            report_file_name(951_868_799),
            "device-check-report-0002292359.txt"
        );
        assert_ne!(report_file_name(60), report_file_name(0));
    }

    fn fact(
        stage: u8,
        stop: ReportStop,
        bridge: ReportIdentity,
        guitar: ReportIdentity,
        cleanup: ReportCleanup,
    ) -> StageFact {
        StageFact {
            stage,
            gate: if stage == 1 {
                ReportGate::OsDiscovery
            } else {
                ReportGate::IdentityRequest
            },
            decision: ReportDecision::Yes,
            status: if stop == ReportStop::None {
                ReportStatus::Passed
            } else {
                ReportStatus::Stopped
            },
            stop,
            cleanup,
            bridge,
            guitar,
        }
    }

    #[test]
    fn every_stop_reason_renders_only_allowlisted_tokens() {
        let sentinel = "SECRET-USER serial /home/host.example";
        let stops = [
            ReportStop::None,
            ReportStop::Declined,
            ReportStop::Cancelled,
            ReportStop::Failed,
            ReportStop::Identity(IdentityFailure::Unknown),
            ReportStop::Identity(IdentityFailure::Ambiguous),
            ReportStop::Identity(IdentityFailure::Inconsistent),
            ReportStop::Identity(IdentityFailure::TimedOut),
            ReportStop::Identity(IdentityFailure::CleanupFailed),
            ReportStop::Identity(IdentityFailure::Cancelled),
        ];
        for stop in stops {
            let known = stop == ReportStop::None;
            let facts = SanitizedFacts {
                mode: ReportMode::Simulation,
                gates: OpenGates::G1,
                stages: [
                    fact(
                        1,
                        ReportStop::None,
                        ReportIdentity::None,
                        ReportIdentity::None,
                        ReportCleanup::NotRun,
                    ),
                    fact(
                        2,
                        stop,
                        if known {
                            ReportIdentity::PodxtLive301
                        } else {
                            ReportIdentity::None
                        },
                        if known {
                            ReportIdentity::Variax600310
                        } else {
                            ReportIdentity::None
                        },
                        if stop == ReportStop::Identity(IdentityFailure::CleanupFailed) {
                            ReportCleanup::Failed
                        } else if stop == ReportStop::None {
                            ReportCleanup::Ok
                        } else {
                            ReportCleanup::NotRun
                        },
                    ),
                ],
            };
            let text = render(&facts);
            assert!(allowlisted(&text), "{text}");
            assert!(!text.contains(sentinel), "{text}");
            assert!(!text.contains("SECRET"));
            assert!(!text.contains('/'));
            assert_eq!(SCHEMA, 2);
            assert!(
                text.starts_with("device-check schema=2 mode=simulation gates-open=G1 writes=none")
            );
        }
    }

    #[test]
    fn save_is_atomic_non_overwriting_and_hides_the_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("SECRET-USER-report.txt");
        let facts = SanitizedFacts {
            mode: ReportMode::ThisComputer,
            gates: OpenGates::G1,
            stages: [
                fact(
                    1,
                    ReportStop::None,
                    ReportIdentity::None,
                    ReportIdentity::None,
                    ReportCleanup::NotRun,
                ),
                fact(
                    2,
                    ReportStop::Identity(IdentityFailure::TimedOut),
                    ReportIdentity::None,
                    ReportIdentity::None,
                    ReportCleanup::Failed,
                ),
            ],
        };
        let text = render(&facts);
        save_sanitized_report(&path, text.as_str()).unwrap();
        let body = std::fs::read_to_string(&path).unwrap();
        assert_eq!(body, text);
        assert!(!body.contains("SECRET"));
        let error = save_sanitized_report(&path, &text).unwrap_err();
        assert!(error.contains("report with that name already exists"));
        assert!(!error.contains("SECRET"));
        let missing = dir.path().join("no-such-dir").join("report.txt");
        let error = save_sanitized_report(&missing, &text).unwrap_err();
        assert_eq!(error, "report could not be saved");
        assert!(!error.contains("no-such"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
        assert!(text.contains("stop=timed-out"));
        assert!(text.contains("cleanup=failed"));
    }

    #[test]
    fn passed_stage_prints_only_the_decoded_identity() {
        let passed_without_identity = fact(
            2,
            ReportStop::None,
            ReportIdentity::None,
            ReportIdentity::None,
            ReportCleanup::Ok,
        );
        let blank = render(&SanitizedFacts {
            mode: ReportMode::Simulation,
            gates: OpenGates::G1,
            stages: [
                fact(
                    1,
                    ReportStop::None,
                    ReportIdentity::None,
                    ReportIdentity::None,
                    ReportCleanup::NotRun,
                ),
                passed_without_identity,
            ],
        });
        assert!(blank.contains("status=passed"));
        assert!(blank.contains("bridge=none"));
        assert!(blank.contains("guitar=none"));
        assert!(!blank.contains("podxt-live-3.01"));
        assert!(!blank.contains("variax-600-3.10"));

        let bridge = CanonicalIdentity {
            role: IdentityRole::Bridge,
            product_id: BRIDGE_PRODUCT,
            version: BRIDGE_VERSION,
        };
        let other_version = CanonicalIdentity {
            role: IdentityRole::Bridge,
            product_id: BRIDGE_PRODUCT,
            version: "3.02",
        };
        assert_eq!(
            ReportIdentity::from_canonical(Some(bridge)),
            ReportIdentity::PodxtLive301
        );
        assert_eq!(
            ReportIdentity::from_canonical(Some(other_version)),
            ReportIdentity::None
        );
        assert_eq!(
            ReportIdentity::from_observed(IdentityRole::Guitar, GUITAR_PRODUCT, GUITAR_VERSION),
            ReportIdentity::Variax600310
        );
        assert_eq!(
            ReportIdentity::from_observed(IdentityRole::Guitar, GUITAR_PRODUCT, "3.02"),
            ReportIdentity::None
        );
    }

    #[test]
    fn report_names_the_gates_it_was_given() {
        let stage = fact(
            1,
            ReportStop::None,
            ReportIdentity::None,
            ReportIdentity::None,
            ReportCleanup::NotRun,
        );
        for gates in [
            OpenGates::None,
            OpenGates::G1,
            OpenGates::G2,
            OpenGates::G1AndG2,
            OpenGates::G1ThroughG3,
        ] {
            let text = render(&SanitizedFacts {
                mode: ReportMode::ThisComputer,
                gates,
                stages: [stage, stage],
            });
            assert!(allowlisted(&text), "{text}");
            assert!(text.starts_with(&format!(
                "device-check schema=2 mode=this-computer {} writes=none",
                gates.token()
            )));
            assert_eq!(text.matches("gates-open=").count(), 1, "{text}");
        }
    }
}
