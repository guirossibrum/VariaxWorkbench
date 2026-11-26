use super::*;
use crate::editor::podxt_panel::live_panel::tests::FakeLock;
use line6_devices::backup::BackupFolder;
use line6_devices::session::{Path as MidiPath, Request};
use podxt::PodxtRequest;
use std::cell::Cell;
use std::collections::VecDeque;

struct ScriptedLink<'a> {
    held: &'a Cell<bool>,
    control: &'a StoreControl,
    script: VecDeque<(Vec<u8>, Vec<u8>)>,
    reply: Option<Vec<u8>>,
    clock: Duration,
}

impl Link for ScriptedLink<'_> {
    fn path(&self) -> MidiPath {
        MidiPath::PodxtMidi
    }

    fn write(&mut self, frame: &[u8]) -> Result<(), String> {
        assert!(self.held.get());
        let (expected, reply) = self.script.pop_front().expect("unexpected write");
        assert_eq!(frame, expected);
        if frame.get(5).is_some_and(|byte| *byte == 0x71) {
            assert!(self.control.started());
            assert!(!self.control.cancel());
        }
        self.reply = (!reply.is_empty()).then_some(reply);
        Ok(())
    }

    fn read(&mut self, duration: Duration) -> Result<Option<Vec<u8>>, String> {
        assert!(self.held.get());
        self.clock += duration;
        Ok(self.reply.take())
    }

    fn elapsed(&self) -> Duration {
        self.clock
    }

    fn pause(&mut self, duration: Duration) {
        self.clock += duration;
    }
}

impl Drop for ScriptedLink<'_> {
    fn drop(&mut self) {
        assert!(self.held.get(), "rig lock must outlive the store transport");
        assert!(self.script.is_empty(), "unfulfilled store transcript");
    }
}

fn fixture(root: &std::path::Path) -> (ConfirmedStore, Vec<u8>, Location) {
    let mut raw = vec![0; podxt::EDIT_BUFFER_LEN];
    raw[..7].copy_from_slice(&[0xf0, 0, 1, 0x0c, 3, 0x74, 0x0a]);
    raw[7..11].copy_from_slice(b"Test");
    raw[167] = 0xf7;
    let mut folder = BackupFolder::create(root, "backup").unwrap();
    folder.record("PODxt Live library backup at 2024-01-01 00:00:00 UTC.".into());
    folder
        .file(
            "identity.syx",
            &variax_workbench::identity_stage::bridge_reply(),
        )
        .unwrap();
    for slot in Location::all(&BankKind::ALL) {
        folder
            .file(&podxt::backup::podxt_entry_name(slot), &raw)
            .unwrap();
    }
    let path = folder.finish(true).unwrap();
    let slot = Location::new(BankKind::Tones, 64).unwrap();
    let plan = podxt::prepare_tone_store(&path, slot, StoreSource::OriginalSlot)
        .unwrap()
        .confirm("STORE PODXT 17A")
        .unwrap();
    (plan, raw, slot)
}

#[test]
fn store_holds_the_rig_lock_and_cancellation_ends_before_the_first_write() {
    let root = tempfile::tempdir().unwrap();
    let (plan, raw, slot) = fixture(root.path());
    let held = Cell::new(false);
    let control = StoreControl::default();
    let mut store = vec![0xf0, 0, 1, 0x0c, 3, 0x71, 0x0a, 0x01, 0x40];
    store.extend_from_slice(&raw[7..]);
    let result = store_transaction(
        plan,
        &control,
        || {
            assert!(!held.replace(true));
            Ok(FakeLock(&held))
        },
        |_, run| {
            assert!(held.get());
            run(&mut ScriptedLink {
                held: &held,
                control: &control,
                reply: None,
                clock: Duration::ZERO,
                script: VecDeque::from([
                    (
                        PodxtRequest::Identity.bytes(),
                        variax_workbench::identity_stage::bridge_reply(),
                    ),
                    (PodxtRequest::Stored(slot).bytes(), raw.clone()),
                    (store, vec![]),
                    (
                        vec![0xf0, 0, 1, 0x0c, 3, 0x72, 0xf7],
                        vec![0xf0, 0, 1, 0x0c, 3, 0x50, 0xf7],
                    ),
                    (PodxtRequest::Stored(slot).bytes(), raw.clone()),
                ]),
            })
        },
    )
    .unwrap();
    assert!(
        matches!(result, StoreResult::Verified { slot: actual, tone } if actual == slot && tone.raw == raw)
    );
    assert!(!held.get());
}

#[test]
fn offline_build_denies_native_store_and_store_policy_denies_other_writes() {
    const {
        assert!(!STORE_OWNER_APPROVED);
    }
    const {
        assert!(!SETUP_STORE_OWNER_APPROVED && !SYNC_OWNER_APPROVED);
    }
    for stage in [StoreStage::Tone, StoreStage::Setup, StoreStage::Sync] {
        assert!(!admitted(stage));
    }
    for denied in [
        Operation::PodxtEditBufferWrite,
        Operation::PodxtAdditionalEditBufferWrite,
        Operation::PodxtChannelSwitch,
        Operation::PodxtLinkedVariaxModelEdit,
        Operation::VariaxStoreRestore,
    ] {
        assert!(!StorePolicy.allows(denied));
    }
}
