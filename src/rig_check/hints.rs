//! Known-cause hints for failures understood on the owner's rig (#249).

use super::record::{Outcome, StepRecord};

pub struct Hint {
    /// Lower-case fragments; any one in the failure text or the log selects it.
    pub signs: &'static [&'static str],
    pub text: &'static str,
}

pub const HINTS: &[Hint] = &[
    Hint {
        signs: &[
            "no reply to the identity",
            "no identity reply",
            "no complete identity reply",
        ],
        text: "Known cause: no identity reply. Check that the device is on, that the VDI cable is fully seated at both ends (Variax) and that no other program (Line 6 Edit, Workbench, the editor) is using the PODxt Live. Then run the step again.",
    },
    Hint {
        signs: &[
            "driver not returned",
            "no driver",
            "reattach kernel driver",
            "unbound",
        ],
        text: "Known cause: the guitar interface driver was not given back (#239). Unplug and replug the PODxt Live USB cable, or use Refresh guitar in Variax Workbench.",
    },
    Hint {
        signs: &[
            "timed out waiting for a complete",
            "has 0 bytes",
            "failed after 3 attempts",
        ],
        text: "Known cause: the PODxt Live relay drops guitar replies larger than about 256 bytes. Rig Check reads in 32-word chunks; if a chunk still fails, another program may be talking to the guitar at the same time.",
    },
    Hint {
        signs: &["retrying variax 0x3c"],
        text: "Known cause: a byte was dropped between the guitar and the PODxt Live and the read was retried (100 ms, at most twice). Occasional retries are normal on this rig (#177); many in one run point to the VDI cable or its connectors.",
    },
    Hint {
        signs: &[
            "no pod",
            "device absent",
            "usb disconnect",
            "no such device",
            "pipe error",
        ],
        text: "Known cause: the PODxt Live left the USB bus. A ground fault (power supply, guitar cable or USB cable) can reset it. Use the PODxt Live's own power supply, avoid USB hubs, check the cables, and reconnect before running again.",
    },
    Hint {
        signs: &["permission denied", "access denied"],
        text: "Known cause: this user may not open the PODxt Live. Install the Variax Workbench udev rule from the Linux package (or run the packaged launcher) and replug the PODxt Live.",
    },
    Hint {
        signs: &["busy"],
        text: "Known cause: the interface is busy. Close Variax Workbench, Line 6 Edit and any other tool that uses the PODxt Live, then try again.",
    },
];

/// Add every hint whose signs appear in a failed step's reason or log.
pub fn attach(record: &mut StepRecord, outcome: &Outcome) {
    if record.retries > 0 {
        let retry = HINTS
            .iter()
            .find(|hint| hint.signs.contains(&"retrying variax 0x3c"))
            .expect("retry hint");
        if !record.hints.iter().any(|h| h == retry.text) {
            record.hints.push(retry.text.into());
        }
    }
    let Some(reason) = outcome.reason() else {
        return;
    };
    if !matches!(outcome, Outcome::Failed(_) | Outcome::Stopped(_)) {
        return;
    }
    let reason = reason.to_ascii_lowercase();
    let log = record.messages.join("\n").to_ascii_lowercase();
    for hint in HINTS {
        if hint
            .signs
            .iter()
            .any(|sign| reason.contains(sign) || log.contains(sign))
            && !record.hints.iter().any(|h| h == hint.text)
        {
            record.hints.push(hint.text.into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rig_check::catalogue::StepId;

    #[test]
    fn each_understood_failure_has_a_hint() {
        for (reason, expect) in [
            (
                "No reply to the identity request: nothing arrived from the guitar.",
                "identity",
            ),
            (
                "Driver not returned: interface 1 has no driver after one retry.",
                "#239",
            ),
            (
                "Start 0, count 32: Timed out waiting for a complete 200-byte 0x3C reply ending in F7.",
                "256 bytes",
            ),
            ("No PODxt Live on USB. device absent", "ground fault"),
        ] {
            let mut record = StepRecord::new(StepId(3, 1));
            attach(&mut record, &Outcome::Failed(reason.into()));
            assert!(record.hints.iter().any(|h| h.contains(expect)), "{reason}");
        }
        let mut record = StepRecord::new(StepId(3, 3));
        record
            .messages
            .push("  1.000s Retrying Variax 0x3C start 0, count 32 (attempt 2/3)".into());
        attach(&mut record, &Outcome::Failed("other".into()));
        assert!(record.hints.iter().any(|h| h.contains("dropped")));
        let mut passed = StepRecord::new(StepId(3, 3));
        attach(&mut passed, &Outcome::Passed);
        assert!(passed.hints.is_empty());
    }
}
