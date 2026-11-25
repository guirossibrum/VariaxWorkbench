//! Step 3.7 (#271): evidence for a Variax 300/600 reporting unverified firmware.

use super::{
    procedures::{Lines, VARIAX_READ, absorb, with_variax},
    record::Mode,
    runner::Ctx,
};
use line6_devices::{
    family::variax::{
        IDENTITY_FAMILY,
        firmware_read::{self, CurrentProgram, FirmwareStatus},
        procedures::{CHUNK_COUNT, pack, set_firmware_checksum},
    },
    identity::Identity,
    midi::hex,
};

pub const NOTICE: &str = "Variax firmware evidence mode. This guitar reports firmware other than 3.10. Its response to the diagnostic reads has not been verified. Keep the rig powered and connected, and leave the selector and knobs alone. Rig Check will only read the current sound. Choose Run on the evidence card to collect replies. The regular rig checks end here.";

pub fn applicable(identity: Option<&Identity>) -> bool {
    identity.is_some_and(|identity| {
        identity.is_line6()
            && identity.family == IDENTITY_FAMILY
            && identity.member == 4
            && identity.version != *b"0310"
    })
}

pub fn read(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let mut chunks = Vec::new();
    let received = |reply| {
        chunks.push(reply);
        Ok(())
    };
    let read = match ctx.mode {
        Mode::Real => with_variax(ctx, VARIAX_READ, |session, stopping| {
            firmware_read::read_current_program(session, stopping, received)
        }),
        Mode::Practice => {
            let rig = ctx.port.simulation().ok_or("No practice rig.")?;
            let replies = rig.borrow().variax_evidence_replies();
            let mut log = Lines::default();
            let read = firmware_read::replay_current_program(
                replies,
                &mut log,
                || ctx.stopping(),
                received,
            );
            absorb(ctx, log);
            read
        }
    };
    for chunk in &chunks {
        frame(
            ctx,
            &format!(
                "Program reply at word {} ({} words)",
                chunk.span.start(),
                chunk.span.count()
            ),
            &chunk.raw,
            chunk.span.reply_bytes(),
        );
    }
    let read = read.inspect_err(|_| {
        ctx.rec.note(format!(
            "Partial evidence: received {} of {CHUNK_COUNT} complete program replies. See the message log for received packets.",
            chunks.len()
        ));
    })?;
    record_layout(ctx, &read);
    ctx.rec.show(format!(
        "Collected the identity, two settings replies and {} program replies for comparison with firmware 3.10.",
        chunks.len()
    ));
    Ok(())
}

fn frame(ctx: &mut Ctx<'_>, label: &str, reply: &[u8], reference_len: usize) {
    ctx.rec.note(format!(
        "{label}: {} bytes, 3.10 layout {reference_len} bytes (delta {:+}). Raw: {}",
        reply.len(),
        reply.len() as isize - reference_len as isize,
        hex(reply)
    ));
}

fn differences(ctx: &mut Ctx<'_>, label: &str, start: usize, actual: &[u8], reference: &[u8]) {
    if actual == reference {
        ctx.rec
            .note(format!("{label}: matches the 3.10 reference."));
        return;
    }
    for (index, (&actual, &reference)) in actual.iter().zip(reference).enumerate() {
        if actual != reference {
            ctx.rec.note(format!(
                "{label} byte {}: read {actual:02X}, 3.10 reference {reference:02X}, XOR mask {:08b}.",
                start + index,
                actual ^ reference
            ));
        }
    }
}

fn record_layout(ctx: &mut Ctx<'_>, read: &CurrentProgram) {
    let identity = &read.settings.identified;
    ctx.rec.note(format!(
        "Variax 300/600 firmware {}, {}. Evidence describes the replies on this connection.",
        identity.identity.version_text(),
        match read.firmware {
            FirmwareStatus::Verified310 => "verified firmware identity",
            FirmwareStatus::Unverified => "unverified firmware",
        }
    ));
    frame(ctx, "Identity reply", &identity.reply, 17);
    differences(
        ctx,
        "Identity version",
        12,
        &identity.identity.version,
        b"0310",
    );
    for (index, reply) in read.settings.replies.iter().enumerate() {
        frame(ctx, &format!("Settings reply {}", index + 1), reply, 200);
    }

    let image = pack(&read.words);
    let mut recomputed = image;
    set_firmware_checksum(&mut recomputed);
    let checksum = 1091 * 3;
    differences(
        ctx,
        "Checksum word 1091 (3.10 sum of words 0–1090)",
        checksum,
        &image[checksum..],
        &recomputed[checksum..],
    );
    let version = 767 * 3..771 * 3;
    differences(
        ctx,
        "Program version stamp (3.10 stored-image layout)",
        version.start,
        &image[version],
        &[0, 0, b'0', 0, 0, b'3', 0, 0, b'1', 0, 0, b'0'],
    );
    ctx.rec.note("The program version stamp is a stored-image reference. The active buffer can carry a different stamp.");
    ctx.rec
        .checksum("Captured working buffer (diagnostic)", &image);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rig_check::{
        approvals,
        catalogue::{RIG_GUARD, StepId, VARIAX_EVIDENCE},
        port::PracticePort,
        practice::{Faults, SimRig},
        record::Outcome,
        report::render_plain,
        run_on, run_practice,
        runner::{Decision, NoReport},
        steps::RunData,
        testing::Script,
    };
    use std::{cell::RefCell, rc::Rc};

    fn unverified() -> Faults {
        Faults {
            unverified_variax: true,
            ..Faults::default()
        }
    }

    #[test]
    fn unverified_309_offers_notice_then_read_only_card_and_reports_raw_layout_differences() {
        let mut model = SimRig::new(unverified());
        model.variax.active[1091] ^= 1;
        let original = model.variax.active;
        let rig = Rc::new(RefCell::new(model));
        let mut port = PracticePort::new(rig.clone());
        let mut operator = Script::run_all();
        let (run, data) = run_on(&mut port, &mut operator, &mut NoReport, RunData::default());
        assert_eq!(operator.notices.last().map(String::as_str), Some(NOTICE));
        assert!(matches!(
            &operator.cards[operator.cards.len() - 2..],
            [StepId(3, 1), VARIAX_EVIDENCE]
        ));
        let evidence = run.step(VARIAX_EVIDENCE);
        assert!(evidence.outcome.passed(), "{evidence:?}");
        let report = render_plain(&run, "test");
        assert!(report.contains("firmware 3.09, unverified firmware"));
        assert!(report.contains("Identity reply: 17 bytes, 3.10 layout 17 bytes (delta +0). Raw:"));
        assert!(
            report.contains("Settings reply 1: 200 bytes, 3.10 layout 200 bytes (delta +0). Raw:")
        );
        assert!(report.contains(
            "Program reply at word 1088 (4 words): 32 bytes, 3.10 layout 32 bytes (delta +0). Raw:"
        ));
        assert!(
            report.contains(
                "Identity version byte 14: read 30, 3.10 reference 31, XOR mask 00000001"
            )
        );
        assert!(report.contains("Checksum word 1091 (3.10 sum of words 0–1090) byte 3275:"));
        assert_eq!(
            evidence
                .evidence
                .iter()
                .filter(|line| line.starts_with("Program reply at word"))
                .count(),
            35
        );
        assert_eq!(
            evidence
                .messages
                .iter()
                .filter(|line| line.contains("OUT f0 00 01 0c 07 00 3c"))
                .count(),
            37
        );
        assert!(
            evidence
                .messages
                .iter()
                .filter_map(|line| line.split_once("OUT ").map(|(_, bytes)| bytes))
                .all(|bytes| bytes.starts_with("f0 00 01 0c 07 00 3c")
                    || bytes.starts_with("f0 00 01 0c 07 00 2a")
                    || bytes.starts_with("f0 00 01 0c 03 23")
                    || bytes == "f0 7e 7f 06 01 f7")
        );
        assert!(data.start_program.is_none() && data.variax_library.is_empty());
        assert_eq!(run.step(StepId(3, 4)).outcome, Outcome::NotReached);
        let rig = rig.borrow();
        assert_eq!(rig.variax.active, original);
        assert_eq!((rig.variax.stores, rig.variax.working_uploads), (0, 0));
        assert!(
            !rig.written
                .iter()
                .any(|(_, bytes)| bytes.first() == Some(&0xc0))
        );
        assert!(rig.driver_bound && !rig.variax_claimed);
        assert!(matches!(
            run.step(RIG_GUARD).outcome,
            Outcome::NotAvailable(_)
        ));
    }

    #[test]
    fn declining_notice_or_skipping_card_leaves_no_evidence_traffic() {
        for accept in [false, true] {
            let mut operator = Script::run_all();
            operator.accept_evidence = accept;
            operator.overrides.push((VARIAX_EVIDENCE, Decision::Skip));
            let (run, _, rig) = run_practice(
                unverified(),
                &mut operator,
                &mut NoReport,
                RunData::default(),
            );
            assert_eq!(operator.notices.len(), 2);
            assert_eq!(operator.cards.contains(&VARIAX_EVIDENCE), accept);
            let evidence = run.step(VARIAX_EVIDENCE);
            assert_eq!(evidence.outcome, Outcome::SkippedByPerson);
            assert!(evidence.messages.is_empty());
            assert!(rig.borrow().driver_bound && !rig.borrow().variax_claimed);
        }
    }

    #[test]
    fn stop_during_evidence_marks_it_partial_and_releases_the_guitar() {
        let mut operator = Script::run_all();
        operator.stop_during = Some(VARIAX_EVIDENCE);
        let (run, _, rig) = run_practice(
            unverified(),
            &mut operator,
            &mut NoReport,
            RunData::default(),
        );
        let step = run.step(VARIAX_EVIDENCE);
        assert!(matches!(step.outcome, Outcome::Stopped(_)));
        assert!(
            step.evidence
                .iter()
                .any(|line| line.contains("Partial evidence: received 0 of 35"))
        );
        assert!(step.messages.is_empty());
        assert!(rig.borrow().driver_bound && !rig.borrow().variax_claimed);
    }

    #[test]
    fn normal_310_run_has_no_evidence_offer_and_real_approval_is_closed() {
        let mut operator = Script::run_all();
        let (run, _, _) = run_practice(
            Faults::default(),
            &mut operator,
            &mut NoReport,
            RunData::default(),
        );
        assert_eq!(operator.notices.len(), 1);
        assert!(!operator.cards.contains(&VARIAX_EVIDENCE));
        assert!(run.step(StepId(3, 3)).outcome.passed());
        assert!(approvals::real_device_check(VARIAX_EVIDENCE).is_err());
    }
}
