//! Short guided commissioning plan. No device or audio interfaces are used.
use std::io::{BufRead, Write};

pub struct Step {
    pub title: &'static str,
    pub request: &'static str,
    pub evidence: &'static str,
}

pub const STEPS: [Step; 5] = [
    Step {
        title: "Model selector and pickup switch (~90 seconds)",
        request: "At the future authorized session: turn through every marked model-selector position, pausing briefly. Then choose one model and cycle every pickup-switch position. Note the chosen labels and any missed/duplicate changes. Physical controls select existing settings; a body name must be read from the preset, never inferred solely from a switch position.",
        evidence: "Timestamped observed selector/preset changes, decoded body/preset identity and switch positions compared with your choices. Live control notifications and their mapping are not implemented or established yet.",
    },
    Step {
        title: "Volume and tone (~45 seconds)",
        request: "For each knob, sweep slowly from minimum to maximum and back, pausing at the middle. Keep the other knob still. Include any other physical control fitted to your guitar in the notes.",
        evidence: "Independent timestamped decoded values, endpoints and direction for each knob; identify unsupported/unreported controls explicitly. Audio loudness/timbre alone does not prove the control value was read.",
    },
    Step {
        title: "Short note sample (~60 seconds)",
        request: "On one agreed model/body and switch position, play each of the six strings open, at fret 5 and at fret 12, one note at a time. This is an 18-note spot check, not every note on the neck. A complete neck sweep is optional later.",
        evidence: "Recorded audio with the model, tuning, string/fret and expected versus measured pitch. Mixed USB audio may not expose individual string telemetry; do not claim it does. Pickup/model timbre needs an independent audio reference.",
    },
    Step {
        title: "Bend and release (~30 seconds)",
        request: "On a comfortable treble-string fret, play the note, bend up roughly a semitone and then a whole tone, hold, release and add a little vibrato. Repeat once with a neighboring note ringing if convenient.",
        evidence: "Pitch trajectory, continuity and audible glitches. Your listening notes are manual observations; automated pitch/artifact measurements are pending.",
    },
    Step {
        title: "Existing alternate tuning (~60 seconds, optional)",
        request: "If an alternate-tuned preset is already available, select it manually and state its actual six-string tuning. Play six open strings and repeat one bend. Otherwise skip. Do not create, upload or overwrite a preset for this check. Return physical controls to the starting positions if desired.",
        evidence: "Compare six expected pitches and the bent-note trajectory against audio. Stored tuning metadata alone cannot prove the audible tuning. Restoring control positions is not a verified restoration of device state.",
    },
];

pub fn plan() -> String {
    let mut text = String::from(
        "Variax quick check — approximately 5 minutes\nCURRENT MODE: OFFLINE REHEARSAL. Keep equipment disconnected.\nNo USB/MIDI/audio access or automated recognition is implemented here.\nThe actions below describe a future separately authorized session.\n\n",
    );
    for (i, step) in STEPS.iter().enumerate() {
        text.push_str(&format!(
            "{}. {}\n{}\nRequired evidence: {}\n\n",
            i + 1,
            step.title,
            step.request,
            step.evidence
        ));
    }
    text
}

/// Record workflow feedback only. Operator responses cannot become sensor passes.
/// EOF/cancellation leaves remaining steps unverified, and input errors propagate.
pub fn rehearse(reader: &mut impl BufRead, writer: &mut impl Write) -> Result<String, String> {
    let mut report = String::from(
        "Variax guided-check rehearsal\nSource: operator feedback in offline rehearsal; no device or audio data.\nHardware recognition: UNVERIFIED\nAudio/pitch behavior: UNVERIFIED\n\n",
    );
    writeln!(writer, "{}", plan()).map_err(|e| e.to_string())?;
    report.push_str(&format!(
        "Application version: {}\nReport time (Unix seconds): {}\n\n",
        env!("CARGO_PKG_VERSION"),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs()
    ));
    let mut stopped = false;
    for (i, step) in STEPS.iter().enumerate() {
        let status = if stopped {
            "NOT RUN"
        } else {
            loop {
                writeln!(
                    writer,
                    "Step {}: {}\nCan you follow this request? [ok/problem/skip/quit]",
                    i + 1,
                    step.title
                )
                .map_err(|e| e.to_string())?;
                writer.flush().map_err(|e| e.to_string())?;
                let mut line = String::new();
                if reader.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
                    stopped = true;
                    break "NOT RUN";
                }
                match line.trim() {
                    "ok" => break "PROMPT UNDERSTOOD (no hardware pass)",
                    "problem" => break "PROMPT NEEDS REVISION",
                    "skip" => break "SKIPPED",
                    "quit" => {
                        stopped = true;
                        break "NOT RUN";
                    }
                    _ => writeln!(writer, "Enter ok, problem, skip or quit.")
                        .map_err(|e| e.to_string())?,
                }
            }
        };
        report.push_str(&format!("{}. {}: {}\n", i + 1, step.title, status));
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn workflow_feedback_never_becomes_hardware_or_audio_evidence() {
        let report = rehearse(
            &mut Cursor::new(b"invalid\nok\nproblem\nskip\nok\nok\n"),
            &mut Vec::new(),
        )
        .unwrap();
        assert!(report.contains("Hardware recognition: UNVERIFIED"));
        assert!(report.contains("Audio/pitch behavior: UNVERIFIED"));
        assert_eq!(report.matches("PROMPT UNDERSTOOD").count(), 3);
        assert!(report.contains("PROMPT NEEDS REVISION"));
        assert!(report.contains("SKIPPED"));
    }

    #[test]
    fn cancellation_and_eof_preserve_unrun_steps() {
        for input in [b"quit\n".as_slice(), b"".as_slice()] {
            let report = rehearse(&mut Cursor::new(input), &mut Vec::new()).unwrap();
            assert_eq!(report.matches("NOT RUN").count(), STEPS.len());
            assert!(!report.contains("PROMPT UNDERSTOOD"));
        }
    }
}
