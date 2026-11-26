//! Severity and bit-level evidence for Rig Check comparisons.

use super::{engine_check, practice::Program, procedures, record::StepRecord};
use line6_devices::family::podxt::{Control, LiveControl, Setting};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Stop,
    Report,
    Info,
}

impl Severity {
    pub fn label(self) -> &'static str {
        match self {
            Self::Stop => "Stops the test",
            Self::Report => "Reported only",
            Self::Info => "Info",
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::Stop => "STOP",
            Self::Report => "REPORT",
            Self::Info => "INFO",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Comparison {
    pub severity: Severity,
    pub summary: String,
    pub details: Vec<String>,
}

impl StepRecord {
    pub fn difference(
        &mut self,
        severity: Severity,
        summary: impl Into<String>,
        details: Vec<String>,
    ) {
        self.comparisons.push(Comparison {
            severity,
            summary: summary.into(),
            details,
        });
    }
}

pub fn program(
    record: &mut StepRecord,
    label: &str,
    expected: &Program,
    actual: &Program,
    severity: Severity,
    allow_live_position: bool,
) -> bool {
    let mut tolerance = None;
    let mut checksum_failed = false;
    if allow_live_position {
        let a = procedures::neutral_program(expected);
        let b = procedures::neutral_program(actual);
        for (side, words, normalized) in [("expected", expected, &a), ("actual", actual, &b)] {
            if let Err(computed) = normalized {
                checksum_failed = true;
                let check = checksum_line(*computed, words[procedures::CHECKSUM_WORD]);
                record.difference(
                    Severity::Stop,
                    format!("{label}: {side} firmware checksum is invalid"),
                    vec![check],
                );
            }
        }
        if let (Ok(a), Ok(b)) = (a, b) {
            tolerance = Some(a == b);
        }
    }
    let differences = engine_check::compare(
        &procedures::program_bytes(expected),
        &procedures::program_bytes(actual),
    );
    let (live, content): (Vec<_>, Vec<_>) = differences
        .into_iter()
        .filter(|diff| !checksum_failed || diff.word != procedures::CHECKSUM_WORD)
        .partition(|diff| {
            tolerance.is_some_and(|only_live| {
                diff.xor & !procedures::tone_position_mask(diff.word) == 0
                    || (only_live && diff.word == procedures::CHECKSUM_WORD)
            })
        });
    if !live.is_empty() {
        record.difference(
            Severity::Info,
            format!("{label}: live tone position and dependent checksum"),
            live.iter().map(variax_word_line).collect(),
        );
    }
    if !content.is_empty() {
        let fields = field_summary(content.iter().map(|diff| diff.part));
        record.difference(
            if checksum_failed {
                Severity::Stop
            } else {
                severity
            },
            format!("{label}: {} words differ ({fields})", content.len()),
            content.iter().map(variax_word_line).collect(),
        );
    }
    !checksum_failed && content.is_empty()
}

pub(super) fn checksum_line(expected: u32, actual: u32) -> String {
    let xor = expected ^ actual;
    format!(
        "word {} (firmware checksum) · expected 0x{expected:06X} · actual 0x{actual:06X} · XOR 0x{xor:06X} · {} bit(s)",
        procedures::CHECKSUM_WORD,
        xor.count_ones(),
    )
}

fn variax_word_line(diff: &engine_check::WordDiff) -> String {
    if procedures::tone_position_mask(diff.word) == 0 {
        return word_line(diff);
    }
    use line6_devices::family::variax::{PresetMetadata, procedures::METADATA};
    let start = diff.word * 3 - METADATA.start;
    let fields = field_summary(
        diff.xor.to_be_bytes()[1..]
            .iter()
            .enumerate()
            .filter(|&(_, &byte)| byte != 0)
            .filter_map(|(offset, _)| PresetMetadata::field_at(start + offset)),
    );
    format!("{} · changed settings: {fields}", word_line(diff))
}

pub fn word_line(diff: &engine_check::WordDiff) -> String {
    format!(
        "word {} ({}) · expected 0x{:06X} · actual 0x{:06X} · XOR 0x{:06X} · {} bit(s)",
        diff.word,
        diff.part,
        diff.expected,
        diff.actual,
        diff.xor,
        diff.xor.count_ones(),
    )
}

pub fn tone(
    record: &mut StepRecord,
    label: &str,
    expected: &[u8],
    actual: &[u8],
    severity: Severity,
    allow_live_position: bool,
) -> usize {
    let mut content = Vec::new();
    let mut live = Vec::new();
    for (index, (&expected, &actual)) in expected.iter().zip(actual).enumerate() {
        if expected == actual {
            continue;
        }
        let field = tone_field(index);
        let xor = expected ^ actual;
        let detail = format!(
            "byte {index} ({field}) · expected 0x{expected:02X} · actual 0x{actual:02X} · XOR 0x{xor:02X} · {} bit(s)",
            xor.count_ones()
        );
        if allow_live_position && podxt_live_byte(index) {
            live.push(detail);
        } else {
            let field = if severity == Severity::Report {
                format!("{field} {expected} → {actual}")
            } else {
                field
            };
            content.push((field, detail));
        }
    }
    if !live.is_empty() {
        record.difference(
            Severity::Info,
            format!("{label}: live controller position"),
            live,
        );
    }
    let count = content.len();
    if count != 0 {
        let fields = field_summary(content.iter().map(|(field, _)| field.as_str()));
        let summary = if severity == Severity::Report {
            format!("{label} is playing with {count} unsaved changes ({fields})")
        } else {
            format!("{label}: {count} bytes differ ({fields})")
        };
        record.difference(
            severity,
            summary,
            content.into_iter().map(|(_, detail)| detail).collect(),
        );
    }
    count
}

pub fn same_podxt_tone(expected: &[u8], actual: &[u8]) -> bool {
    expected.len() == actual.len()
        && expected
            .iter()
            .zip(actual)
            .enumerate()
            .all(|(index, (expected, actual))| expected == actual || podxt_live_byte(index))
}

fn podxt_live_byte(index: usize) -> bool {
    // Presence (CC 21) is part of the captured starting edits in #277.
    index
        .checked_sub(39)
        .is_some_and(|cc| [4, 7, 17, 105].contains(&cc))
}

/// For shared PODxt procedures that return an error on mismatch, recover the
/// actual full dump from that exchange's existing diagnostic packet log.
pub fn last_podxt_dump(messages: &[String]) -> Option<Vec<u8>> {
    use line6_devices::{
        family::podxt::EditBuffer,
        midi::{Assembler, Event},
    };
    let mut assembler = Assembler::default();
    let mut last = None;
    for line in messages {
        let Some((_, raw)) = line.split_once(" IN  ") else {
            continue;
        };
        let bytes: Vec<_> = raw
            .split_whitespace()
            .map_while(|byte| {
                (byte.len() == 2)
                    .then(|| u8::from_str_radix(byte, 16).ok())
                    .flatten()
            })
            .collect();
        if let Ok(events) = assembler.feed(&bytes) {
            for event in events {
                if let Event::Sysex(raw) = event
                    && let Ok(tone) = EditBuffer::parse(raw)
                {
                    last = Some(tone.raw);
                }
            }
        }
    }
    last
}

fn field_summary<'a>(fields: impl Iterator<Item = &'a str>) -> String {
    let mut unique = Vec::new();
    for field in fields {
        if !unique.contains(&field) {
            unique.push(field);
        }
    }
    unique.join(", ")
}

fn tone_field(index: usize) -> String {
    match index {
        0..=6 | 167 => "MIDI frame".into(),
        7..=22 => "tone name".into(),
        23..=38 => "tone header".into(),
        _ => {
            let cc = (index - 39) as u8;
            let ordinary = Control::ALL.into_iter().find(|control| control.cc() == cc);
            let extra = Setting::ALL
                .into_iter()
                .map(LiveControl::Additional)
                .find(|control| control.cc() == cc || control.low_cc() == Some(cc));
            ordinary
                .map(|control| format!("{} (CC {cc})", control.name()))
                .or_else(|| extra.map(|control| format!("{} (CC {cc})", control.name())))
                .unwrap_or_else(|| format!("CC {cc}"))
        }
    }
}
