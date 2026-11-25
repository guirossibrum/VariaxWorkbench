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
        self.send(BRIDGE_ON);
        self.send(&guitar(&[0x2a, 1]));
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
