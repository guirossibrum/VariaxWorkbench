use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Severity {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Critical,
}

impl Severity {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Trace => "TRACE",
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
            Self::Critical => "CRITICAL",
        }
    }
}

pub const DEFAULT_LEVEL: Severity = default_level(
    option_env!("VARIAX_LOG_PROFILE"),
    option_env!("VARIAX_BUILD_NUMBER").is_some(),
);

pub(super) const fn default_level(profile: Option<&str>, numbered: bool) -> Severity {
    match profile {
        Some(profile) => match profile.as_bytes() {
            b"owner" | b"development" => Severity::Trace,
            _ => Severity::Warn,
        },
        None if !numbered => Severity::Trace,
        None => Severity::Warn,
    }
}
