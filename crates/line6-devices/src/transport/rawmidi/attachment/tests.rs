use super::*;
use crate::device_lock::LockError;
use inventory::{Inode, probe_with};
use native::close_once;
use std::cell::Cell;
use std::fs;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::fs::MetadataExt;
use std::os::unix::{fs::symlink, net::UnixStream};
use std::path::Path as FsPath;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "line6-rawmidi-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn lock(&self) -> DeviceLock {
        DeviceLock::acquire_test_lock(&self.0.join("rig")).unwrap()
    }

    fn assert_locked(&self) {
        assert!(matches!(
            DeviceLock::acquire_test_lock(&self.0.join("rig")),
            Err(LockError::Busy)
        ));
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn identity() -> AttachmentIdentity {
    AttachmentIdentity {
        device: PODXT_LIVE_MIDI.device_node(4),
        node: node(),
        usb: Inode(1, 1),
        interface: Inode(1, 2),
        card: Inode(1, 3),
        midi: Inode(1, 4),
    }
}

fn node() -> NodeIdentity {
    NodeIdentity {
        inode: Inode(2, 5),
        rdev: libc::makedev(116, 8),
    }
}

fn selection() -> RawMidiAttachment {
    RawMidiAttachment {
        usb_path: "8-2.1".into(),
        address: 11,
        identity: identity(),
    }
}

fn pair() -> (File, File) {
    let (port, peer) = UnixStream::pair().unwrap();
    peer.set_nonblocking(true).unwrap();
    (
        File::from(OwnedFd::from(port)),
        File::from(OwnedFd::from(peer)),
    )
}

fn assert_unclosed_without_traffic(mut peer: &File) {
    assert_eq!(
        peer.read(&mut [0]).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
}

fn assert_closed_without_traffic(mut peer: &File) {
    assert_eq!(peer.read(&mut [0]).unwrap(), 0);
}

#[test]
fn fresh_link_uses_one_descriptor_under_the_same_rig_lock() {
    let fixture = Fixture::new();
    let rig = fixture.lock();
    let (port, mut peer) = pair();
    let fd = port.as_raw_fd();
    let probes = Cell::new(0);
    let mut link = ScopedRawMidiLink::open_with(
        &selection(),
        &rig,
        || false,
        || {
            fixture.assert_locked();
            probes.set(probes.get() + 1);
            Ok(identity())
        },
        |device| {
            fixture.assert_locked();
            assert_eq!(probes.get(), 1);
            assert_eq!(device, PODXT_LIVE_MIDI.device_node(4));
            Ok(port)
        },
        |port| {
            assert_eq!(port.as_raw_fd(), fd);
            Ok(node())
        },
        |_| panic!("verified open must transfer ownership"),
    )
    .unwrap();
    assert_eq!(probes.get(), 2);
    assert_unclosed_without_traffic(&peer);
    assert_eq!(link.path(), Path::PodxtMidi);
    link.write(&[0xf0, 0xf7]).unwrap();
    let mut received = [0; 2];
    peer.read_exact(&mut received).unwrap();
    assert_eq!(received, [0xf0, 0xf7]);
    peer.write_all(&[0xf0, 1, 0xf7]).unwrap();
    assert_eq!(
        link.read(Duration::ZERO).unwrap(),
        Some(vec![0xf0, 1, 0xf7])
    );
    link.finish_with(|port| {
        fixture.assert_locked();
        assert_eq!(port.as_raw_fd(), fd);
        close_once(port)
    })
    .unwrap();
    assert_closed_without_traffic(&peer);
    fixture.assert_locked();
    drop(rig);
    drop(fixture.lock());
}

#[test]
fn missing_ambiguous_and_replaced_selections_refuse_before_open() {
    let fixture = Fixture::new();
    let rig = fixture.lock();
    let mut replacement = identity();
    replacement.usb = Inode(1, 10);
    for (reply, evidence) in [
        (Err("no PODxt Live USB attachment".into()), "no PODxt"),
        (
            Err("multiple PODxt Live ALSA MIDI cards".into()),
            "multiple",
        ),
        (Ok(replacement), "changed"),
    ] {
        let error = ScopedRawMidiLink::open_with(
            &selection(),
            &rig,
            || false,
            {
                let mut reply = Some(reply);
                move || reply.take().unwrap()
            },
            |_| panic!("never open a refused attachment"),
            |_| unreachable!(),
            |_| unreachable!(),
        )
        .err()
        .unwrap();
        assert!(
            matches!(error.failure, RawMidiOpenFailure::Attachment(ref reason) if reason.contains(evidence))
        );
        assert!(error.cleanup.is_none());
    }
}

#[test]
fn replacement_after_open_closes_once_before_yielding_a_link() {
    let fixture = Fixture::new();
    let rig = fixture.lock();
    let (port, peer) = pair();
    let fd = port.as_raw_fd();
    let probe = Cell::new(0);
    let error = ScopedRawMidiLink::open_with(
        &selection(),
        &rig,
        || false,
        || {
            let mut result = identity();
            if probe.replace(1) > 0 {
                result.midi = Inode(1, 10);
            }
            Ok(result)
        },
        |_| Ok(port),
        |_| Ok(node()),
        |port| {
            fixture.assert_locked();
            assert_eq!(fd, port.as_raw_fd());
            close_once(port)
        },
    )
    .err()
    .unwrap();
    assert!(
        matches!(error.failure, RawMidiOpenFailure::Attachment(ref reason) if reason.contains("changed"))
    );
    assert!(error.cleanup.is_none());
    assert_closed_without_traffic(&peer);
}

#[test]
fn wrong_descriptor_is_refused_even_with_the_same_device_number() {
    let fixture = Fixture::new();
    let rig = fixture.lock();
    let (port, peer) = pair();
    let error = ScopedRawMidiLink::open_with(
        &selection(),
        &rig,
        || false,
        || Ok(identity()),
        |_| Ok(port),
        |port| {
            let mut wrong = node();
            wrong.inode = Inode(2, port.metadata()?.ino());
            assert_ne!(wrong, node());
            Ok(wrong)
        },
        close_once,
    )
    .err()
    .unwrap();
    assert!(matches!(error.failure, RawMidiOpenFailure::WrongDescriptor));
    assert!(error.cleanup.is_none());
    assert_closed_without_traffic(&peer);
}

#[test]
fn non_character_descriptor_and_open_failure_report_different_stages() {
    let fixture = Fixture::new();
    let rig = fixture.lock();
    let (port, peer) = pair();
    let error = ScopedRawMidiLink::open_with(
        &selection(),
        &rig,
        || false,
        || Ok(identity()),
        |_| Ok(port),
        |port| NodeIdentity::from_metadata(&port.metadata()?),
        close_once,
    )
    .err()
    .unwrap();
    assert!(
        matches!(error.failure, RawMidiOpenFailure::Descriptor(ref error) if error.kind() == io::ErrorKind::InvalidData)
    );
    assert_closed_without_traffic(&peer);

    let error = ScopedRawMidiLink::open_with(
        &selection(),
        &rig,
        || false,
        || Ok(identity()),
        |_| Err(io::Error::from_raw_os_error(libc::EACCES)),
        |_| unreachable!(),
        |_| panic!("there is no descriptor to close"),
    )
    .err()
    .unwrap();
    assert!(
        matches!(error.failure, RawMidiOpenFailure::Open(ref error) if error.raw_os_error() == Some(libc::EACCES))
    );
    assert!(error.cleanup.is_none());
}

#[test]
fn cancellation_before_or_during_open_does_not_return_a_link() {
    let fixture = Fixture::new();
    let rig = fixture.lock();
    for cancellation_poll in 1..=4 {
        let (port, peer) = pair();
        let polls = Cell::new(0);
        let opens = Cell::new(0);
        let closes = Cell::new(0);
        let opened = usize::from(cancellation_poll >= 3);
        let error = ScopedRawMidiLink::open_with(
            &selection(),
            &rig,
            || {
                polls.set(polls.get() + 1);
                polls.get() == cancellation_poll
            },
            || Ok(identity()),
            |_| {
                opens.set(opens.get() + 1);
                Ok(port)
            },
            |_| Ok(node()),
            |port| {
                fixture.assert_locked();
                closes.set(closes.get() + 1);
                close_once(port)
            },
        )
        .err()
        .unwrap();
        assert!(matches!(error.failure, RawMidiOpenFailure::Cancelled));
        assert!(error.cleanup.is_none());
        assert_eq!(opens.get(), opened);
        assert_eq!(closes.get(), opened);
        assert_closed_without_traffic(&peer);
    }
}

#[test]
fn cancellation_keeps_independent_checked_close_error() {
    let fixture = Fixture::new();
    let rig = fixture.lock();
    let (port, peer) = pair();
    let opened = Cell::new(false);
    let error = ScopedRawMidiLink::open_with(
        &selection(),
        &rig,
        || opened.get(),
        || Ok(identity()),
        |_| {
            opened.set(true);
            Ok(port)
        },
        |_| panic!("cancellation already requested"),
        |port| {
            fixture.assert_locked();
            close_once(port)?;
            Err(io::Error::from_raw_os_error(libc::EIO))
        },
    )
    .err()
    .unwrap();
    assert!(matches!(error.failure, RawMidiOpenFailure::Cancelled));
    assert!(error.to_string().contains("checked close failed"));
    let cleanup = error.cleanup.unwrap();
    assert_eq!(cleanup.device, error.device);
    assert_eq!(cleanup.source.raw_os_error(), Some(libc::EIO));
    assert_closed_without_traffic(&peer);
}

#[test]
fn successful_and_failed_finish_transfer_the_fd_once_without_a_drop_close() {
    let fixture = Fixture::new();
    let rig = fixture.lock();
    for errno in [None, Some(libc::EINTR), Some(libc::EIO)] {
        let (port, peer) = pair();
        let fd = port.as_raw_fd();
        let link = ScopedRawMidiLink::open_with(
            &selection(),
            &rig,
            || false,
            || Ok(identity()),
            |_| Ok(port),
            |_| Ok(node()),
            |_| unreachable!(),
        )
        .unwrap();
        let mut retained = None;
        let result = link.finish_with(|port| {
            fixture.assert_locked();
            assert_eq!(port.as_raw_fd(), fd);
            // Keep the transferred number live to detect a second close by
            // the transport after the injected result.
            retained = Some(port);
            errno.map_or(Ok(()), |errno| Err(io::Error::from_raw_os_error(errno)))
        });
        assert_eq!(
            result
                .err()
                .map(|error| error.source.raw_os_error().unwrap()),
            errno
        );
        assert_unclosed_without_traffic(&peer);
        drop(retained.unwrap());
        assert_closed_without_traffic(&peer);
    }
}

#[test]
fn public_selection_is_inactive_in_test_builds() {
    let fixture = Fixture::new();
    assert!(
        RawMidiAttachment::select_podxt_live(&fixture.lock(), "8-2.1", 11)
            .unwrap_err()
            .contains("Test builds")
    );
}

struct InventoryFixture {
    root: Fixture,
    usb: PathBuf,
    sound: PathBuf,
    card: PathBuf,
    midi: PathBuf,
}

impl InventoryFixture {
    fn new() -> Self {
        let root = Fixture::new();
        let usb = root.0.join("bus/usb/devices");
        let sound = root.0.join("class/sound");
        let physical = root.0.join("devices/pci/usb8/8-2.1");
        let interface = physical.join("8-2.1:1.0");
        let card = interface.join("sound/card4");
        let midi = card.join("midiC4D0");
        for dir in [&usb, &sound, &midi] {
            fs::create_dir_all(dir).unwrap();
        }
        fs::write(physical.join("idVendor"), "0e41\n").unwrap();
        fs::write(physical.join("idProduct"), "4650\n").unwrap();
        fs::write(physical.join("devnum"), "11\n").unwrap();
        symlink("../../drivers/snd_usb_pod", interface.join("driver")).unwrap();
        symlink(&physical, usb.join("8-2.1")).unwrap();
        symlink(&interface, usb.join("8-2.1:1.0")).unwrap();
        symlink(&card, sound.join("card4")).unwrap();
        symlink(&midi, sound.join("midiC4D0")).unwrap();
        fs::write(card.join("id"), "PODxtLive\n").unwrap();
        fs::write(midi.join("dev"), "116:8\n").unwrap();
        Self {
            root,
            usb,
            sound,
            card,
            midi,
        }
    }

    fn probe(&self, address: u8) -> Result<AttachmentIdentity, String> {
        probe_with(&self.root.0, "8-2.1", address, |device| {
            assert_eq!(device, FsPath::new("/dev/snd/midiC4D0"));
            Ok(node())
        })
    }
}

#[test]
fn synthetic_inventory_binds_usb_generation_to_card_and_device_number() {
    let tree = InventoryFixture::new();
    let selected = tree.probe(11).unwrap();
    assert_eq!(selected.node, node());
    assert!(tree.probe(12).unwrap_err().contains("changed"));

    // Replace the MIDI sysfs entry at the same path and device number.
    fs::rename(&tree.midi, tree.card.join("retained-old-midi")).unwrap();
    fs::create_dir(&tree.midi).unwrap();
    fs::write(tree.midi.join("dev"), "116:8\n").unwrap();
    assert_ne!(tree.probe(11).unwrap(), selected);

    fs::write(tree.midi.join("dev"), "116:9\n").unwrap();
    assert!(tree.probe(11).unwrap_err().contains("device number"));
}

#[test]
fn synthetic_inventory_rejects_detached_and_ambiguous_topology() {
    let tree = InventoryFixture::new();
    symlink(&tree.card, tree.sound.join("card5")).unwrap();
    assert!(
        tree.probe(11)
            .unwrap_err()
            .contains("multiple PODxt Live ALSA MIDI cards")
    );
    fs::remove_file(tree.sound.join("card5")).unwrap();

    fs::remove_file(tree.sound.join("midiC4D0")).unwrap();
    let other = tree.root.0.join("devices/another-card/midiC4D0");
    fs::create_dir_all(&other).unwrap();
    symlink(other, tree.sound.join("midiC4D0")).unwrap();
    assert!(tree.probe(11).unwrap_err().contains("not beneath"));

    let second = tree.usb.join("8-2.2");
    fs::create_dir(&second).unwrap();
    fs::write(second.join("idVendor"), "0e41\n").unwrap();
    fs::write(second.join("idProduct"), "4650\n").unwrap();
    assert!(
        tree.probe(11)
            .unwrap_err()
            .contains("multiple PODxt Live USB attachments")
    );
}
