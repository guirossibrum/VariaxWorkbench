use super::*;
use line6_devices::{
    gate::{GatePolicy, Operation},
    session::{Path, Quiet},
};
use std::{collections::VecDeque, time::Duration};

const HEADER: &[u8] = &[0xf0, 0, 1, 0x0c, 7, 0];
const BRIDGE_ON: &[u8] = &[0xf0, 0, 1, 0x0c, 3, 0x23, 1, 0xf7];
const BRIDGE_OFF: &[u8] = &[0xf0, 0, 1, 0x0c, 3, 0x23, 0, 0xf7];
const ACK: &[u8] = &[0xf0, 0, 1, 0x0c, 7, 4, 0x71, 0xf7];
const NAME: &str = "Original";

fn guitar(payload: &[u8]) -> Vec<u8> {
    [HEADER, payload, &[0xf7]].concat()
}

fn nibbles(bytes: &[u8]) -> Vec<u8> {
    bytes
        .as_chunks::<3>()
        .0
        .iter()
        .flat_map(|&[a, b, c]| [c & 15, c >> 4, b & 15, b >> 4, a & 15, a >> 4])
        .collect()
}

#[derive(Default)]
struct Wire {
    expected: VecDeque<(Vec<u8>, Vec<u8>)>,
    inbox: VecDeque<Vec<u8>>,
    time: Duration,
    pauses: Vec<Duration>,
    upload_ended: Option<Duration>,
    store_delays: Vec<Duration>,
}

impl Wire {
    fn send(&mut self, bytes: &[u8]) {
        self.reply(bytes, &[]);
    }

    fn reply(&mut self, bytes: &[u8], response: &[u8]) {
        self.expected.push_back((bytes.into(), response.into()));
    }

    fn wrapped(&mut self, bytes: &[u8]) {
        self.send(HEADER);
        self.send(bytes);
        self.send(&[0xf7]);
    }

    // Expected owner-tool wire requests, independently of the live adapter.
    fn read_buffer(&mut self, image: &Image) {
        self.read_selected(image, None);
    }

    fn read_selected(&mut self, image: &Image, program: Option<u8>) {
        self.send(BRIDGE_ON);
        self.send(&guitar(&[0x2a, 1]));
        if let Some(program) = program {
            self.send(&[0xc0, program]);
        }
        for start in (0..1092usize).step_by(32) {
            let count = (1092 - start).min(32);
            let request = guitar(&[
                0x3c,
                (count & 15) as u8,
                (count >> 4) as u8,
                0,
                0,
                0,
                0,
                (start & 15) as u8,
                ((start >> 4) & 15) as u8,
                (start >> 8) as u8,
                0,
                0,
                0,
            ]);
            let data = nibbles(&image[start * 3..(start + count) * 3]);
            self.reply(&request, &guitar(&[&[0x3c], data.as_slice()].concat()));
        }
        self.send(&guitar(&[0x2a, 0]));
        self.send(BRIDGE_OFF);
    }

    fn upload(&mut self, image: &Image) {
        let data = nibbles(image);
        let message = guitar(&[&[0x43, 4, 4, 4, 0, 0, 0], data.as_slice()].concat());
        for chunk in message.chunks(256) {
            self.send(chunk);
        }
    }

    fn difference(&mut self) {
        // One changed word at address zero from the owner's difference fixture.
        self.send(HEADER);
        self.send(&[0x3b, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 6, 6]);
        self.reply(&[0xf7], ACK);
    }

    fn close_edit(&mut self) {
        self.wrapped(&[0x4c, 0]);
        self.wrapped(&[0x2a, 0]);
        self.send(BRIDGE_OFF);
    }

    fn run(
        &mut self,
        image: &Image,
        transition: Option<&Image>,
        previous: Option<&Previous>,
    ) -> Result<Image, String> {
        let mut log = Quiet;
        let mut session = Session::new(&mut *self, &EditOnly, &mut log);
        let result = send_on(
            &mut session,
            NAME,
            image,
            transition,
            previous,
            &AtomicBool::new(false),
        );
        assert!(self.expected.is_empty(), "missing requests");
        assert!(self.inbox.is_empty(), "unconsumed reply bytes");
        result
    }
}

impl Link for Wire {
    fn path(&self) -> Path {
        Path::VariaxViaPodxt
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        let (expected, response) = self.expected.pop_front().expect("unexpected request");
        assert_eq!(bytes, expected);
        if bytes.len() > 8 && bytes.last() == Some(&0xf7) {
            self.upload_ended = Some(self.time);
        }
        if matches!(bytes, [0x61, _]) {
            self.store_delays
                .push(self.time - self.upload_ended.take().expect("full upload"));
        }
        self.inbox.extend(response.chunks(16).map(<[u8]>::to_vec));
        Ok(())
    }

    fn read(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, String> {
        let packet = self.inbox.pop_front();
        self.time += if packet.is_some() {
            Duration::from_millis(4)
        } else {
            timeout
        };
        Ok(packet)
    }

    fn elapsed(&self) -> Duration {
        self.time
    }

    fn pause(&mut self, duration: Duration) {
        self.time += duration;
        self.pauses.push(duration);
    }
}

struct EditOnly;

impl GatePolicy for EditOnly {
    fn allows(&self, operation: Operation) -> bool {
        matches!(
            operation,
            Operation::VariaxRead | Operation::VariaxEditBufferWrite
        )
    }
}

fn image(first: u8) -> Image {
    let mut image = [0; procedures::PROGRAM_BYTES];
    image[0] = first;
    let start = procedures::METADATA.start;
    image[start..start + 18].fill(b' ');
    image[start..start + NAME.len()].copy_from_slice(NAME.as_bytes());
    procedures::set_firmware_checksum(&mut image);
    image
}

#[test]
fn full_send_matches_owner_wire_requests_and_verifies_all_words() {
    let mut wire = Wire::default();
    let image = image(1);
    wire.send(BRIDGE_ON);
    wire.send(&guitar(&[0x2a, 1]));
    wire.upload(&image);
    wire.send(&guitar(&[0x2a, 0]));
    wire.send(BRIDGE_OFF);
    wire.read_buffer(&image);
    assert_eq!(wire.run(&image, None, None).unwrap(), image);
}

#[test]
fn difference_matches_owner_wire_requests_and_retains_prior_checksum() {
    let mut wire = Wire::default();
    let previous = Previous {
        image: Box::new(image(1)),
        tuning_transition: false,
    };
    let next = image(102);
    let mut verified = next;
    verified[3273..].copy_from_slice(&previous.image[3273..]);
    wire.read_buffer(&previous.image);
    wire.send(BRIDGE_ON);
    wire.wrapped(&[0x2a, 1]);
    wire.difference();
    wire.close_edit();
    wire.read_buffer(&verified);
    assert_eq!(wire.run(&next, None, Some(&previous)).unwrap(), verified);
}

#[test]
fn tuning_transition_matches_owner_difference_then_full_upload() {
    let mut wire = Wire::default();
    let previous = Previous {
        image: Box::new(image(1)),
        tuning_transition: true,
    };
    let muted = image(102);
    let next = image(103);
    wire.read_buffer(&previous.image);
    wire.send(BRIDGE_ON);
    wire.wrapped(&[0x2a, 1]);
    wire.difference();
    wire.upload(&next);
    wire.close_edit();
    wire.read_buffer(&next);
    assert_eq!(
        wire.run(&next, Some(&muted), Some(&previous)).unwrap(),
        next
    );
}

#[test]
fn changed_buffer_blocks_writes_after_a_new_attachment_lock() {
    let mut wire = Wire::default();
    let previous = Previous {
        image: Box::new(image(1)),
        tuning_transition: false,
    };
    wire.read_buffer(&image(2));
    let error = wire.run(&image(102), None, Some(&previous)).unwrap_err();
    assert!(error.contains("Prior edit-buffer verification"), "{error}");
}

struct LibraryOnly;

impl GatePolicy for LibraryOnly {
    fn allows(&self, operation: Operation) -> bool {
        matches!(
            operation,
            Operation::VariaxRead | Operation::VariaxPresetSwitch | Operation::VariaxStoreRestore
        )
    }
}

impl Wire {
    fn store(&mut self, slot: u8, image: &Image, ack: bool) {
        self.send(BRIDGE_ON);
        self.wrapped(&[0x2a, 1]);
        self.wrapped(&[0x4c, 1]);
        self.upload(image);
        self.send(HEADER);
        self.send(&[0x61, slot]);
        if ack {
            let mut reply = Vec::new();
            for notice in [0x28, 0x4d, 0x74] {
                reply.extend([0xf0, 0, 1, 0x0c, 7, 4, notice, 0xf7]);
            }
            reply.extend(ACK);
            self.reply(&[0xf7], &reply);
        } else {
            self.send(&[0xf7]);
        }
        self.close_edit();
    }

    fn run_sync(
        &mut self,
        targets: &[Target],
        stores: Option<&[StoreImages]>,
    ) -> (Report, Vec<String>) {
        let mut notes = Vec::new();
        let mut log = Quiet;
        let mut session = Session::new(&mut *self, &LibraryOnly, &mut log);
        let report = sync_on(
            &mut session,
            targets,
            stores,
            &AtomicBool::new(false),
            |message| notes.push(message),
        );
        assert!(self.expected.is_empty(), "missing requests");
        assert!(self.inbox.is_empty(), "unconsumed reply bytes");
        (report, notes)
    }
}

fn target(slot: u8) -> Target {
    let mut model = PresetFile::default_legacy_model().unwrap();
    model.rename(NAME).unwrap();
    Target {
        position: line6_devices::family::variax::Preset::new(2, slot).unwrap(),
        model,
    }
}

#[test]
fn store_batch_checks_restore_bytes_and_reloads_each_success_once() {
    let mut wire = Wire::default();
    let targets = [target(0), target(1)];
    let stores = [1, 2].map(|n| StoreImages {
        candidate: image(n),
        backup: image(n + 2),
    });
    for (i, images) in stores.iter().enumerate() {
        let i = i as u8;
        wire.read_selected(&images.backup, Some(69 - i));
        wire.store(14 - i, &images.candidate, true);
        wire.read_selected(
            &procedures::expected_stored_image(&images.candidate),
            Some(69 - i),
        );
    }
    let (report, _) = wire.run_sync(&targets, Some(&stores));
    assert!(report.error.is_none(), "{:?}", report.error);
    assert_eq!(
        report
            .verified
            .iter()
            .map(|entry| entry.1)
            .collect::<Vec<_>>(),
        [0, 1]
    );
    assert_eq!(wire.store_delays, [Duration::from_millis(100); 2]);
}

#[test]
fn silent_store_waits_two_minutes_and_stops_the_batch() {
    let mut wire = Wire::default();
    let targets = [target(0), target(1), target(2)];
    let stores = [1, 2, 3].map(|n| StoreImages {
        candidate: image(n),
        backup: image(n + 3),
    });
    wire.read_selected(&stores[0].backup, Some(69));
    wire.store(14, &stores[0].candidate, true);
    wire.read_selected(
        &procedures::expected_stored_image(&stores[0].candidate),
        Some(69),
    );
    wire.read_selected(&stores[1].backup, Some(68));
    wire.store(13, &stores[1].candidate, false);

    let (report, notes) = wire.run_sync(&targets, Some(&stores));
    assert_eq!(report.verified.len(), 1);
    assert!(report.store_unknown);
    assert!(report.error.unwrap().contains("Slot state is unknown"));
    assert!(notes.last().unwrap().contains("untouched for 2 minutes"));
    assert_eq!(wire.pauses.last(), Some(&Duration::from_secs(120)));
}

#[test]
fn stale_restore_backup_blocks_store() {
    let mut wire = Wire::default();
    wire.read_selected(&image(2), Some(69));
    let (report, _) = wire.run_sync(
        &[target(0)],
        Some(&[StoreImages {
            candidate: image(3),
            backup: image(1),
        }]),
    );
    assert!(report.verified.is_empty());
    assert!(!report.store_unknown);
    assert!(
        report
            .error
            .unwrap()
            .contains("differs from the verified restore backup")
    );
}

#[test]
fn read_unmodified_only_selects_and_reads_saved_slot() {
    let mut wire = Wire::default();
    let target = target(1);
    let mut saved = image(0);
    let model_start = procedures::METADATA.start;
    saved[model_start..model_start + 18].fill(b' ');
    saved[model_start..model_start + 5].copy_from_slice(b"Saved");
    procedures::set_firmware_checksum(&mut saved);
    wire.read_selected(&saved, Some(68));
    let (report, _) = wire.run_sync(&[target], None);
    assert!(report.error.is_none(), "{:?}", report.error);
    assert_eq!(report.verified.len(), 1);
    let (bank, slot, model) = &report.verified[0];
    assert_eq!((*bank, *slot, model.name()), (2, 1, "Saved"));
}

#[test]
fn unmatched_post_store_read_stops_the_batch() {
    let mut wire = Wire::default();
    let targets = [target(0), target(1)];
    let stores = [1, 2].map(|n| StoreImages {
        candidate: image(n),
        backup: image(n + 3),
    });
    wire.read_selected(&stores[0].backup, Some(69));
    wire.store(14, &stores[0].candidate, true);
    wire.read_selected(&image(1), Some(69));

    let (report, _) = wire.run_sync(&targets, Some(&stores));
    assert!(report.verified.is_empty());
    assert!(report.store_unknown);
    assert!(
        report
            .error
            .unwrap()
            .contains("acknowledged; read-back failed")
    );
}
