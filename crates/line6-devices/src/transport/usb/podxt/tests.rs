use super::*;
use crate::family::variax::transport::VARIAX_VIA_PODXT_LIVE;
use crate::midi::Assembler;

#[test]
fn the_closed_gate_precedes_the_device_list_for_interface_zero() {
    let error = super::super::UsbInterfaceLink::open(SPEC).err().unwrap();
    assert!(error.contains("closed"), "{error}");
    assert!(check_open(SPEC, true).is_err());
    assert!(check_open(VARIAX_VIA_PODXT_LIVE, true).is_ok());
}

#[test]
fn transport_writes_raw_chunks_to_endpoint_three_and_stops_on_a_short_write() {
    let outgoing: Vec<_> = (0..35).collect();
    let mut packets = Vec::new();
    super::super::write_packets(
        SPEC,
        &outgoing,
        |endpoint, data, timeout| {
            assert_eq!(endpoint, 0x03);
            assert_eq!(timeout, Duration::from_secs(1));
            packets.push(data.to_vec());
            Ok(data.len())
        },
        |gap| assert!(gap.is_zero()),
    )
    .unwrap();
    assert_eq!(packets, outgoing.chunks(16).collect::<Vec<_>>());

    let mut writes = 0;
    let error = super::super::write_packets(
        SPEC,
        &outgoing,
        |_, _, _| {
            writes += 1;
            Ok(15)
        },
        |_| panic!("stop on short write"),
    )
    .unwrap_err();
    assert_eq!(writes, 1);
    assert_eq!(error, "Short USB write.");
}

#[test]
fn raw_identity_reassembles_from_short_reads_with_pod_status_normalization() {
    let fixture = crate::identity::tests::PODXT_LIVE;
    let mut raw = fixture.to_vec();
    raw[0] = 0xf2;
    let mut assembler = Assembler::default();
    for bytes in raw.chunks(16) {
        let packet = super::super::read_packet(SPEC, Duration::ZERO, |endpoint, target, wait| {
            assert_eq!(endpoint, 0x84);
            assert_eq!(target.len(), 16);
            assert_eq!(wait, Duration::from_millis(1));
            target[..bytes.len()].copy_from_slice(bytes);
            Ok(bytes.len())
        })
        .unwrap()
        .unwrap();
        let events = assembler.feed(&packet).unwrap();
        if bytes.len() == 1 {
            assert_eq!(events, [crate::midi::Event::Sysex(fixture.to_vec())]);
        } else {
            assert!(events.is_empty());
        }
    }
    assert_eq!(assembler.orphans, 0);
    let mut notices = [0xb2, 69, 0x7f, 0xc2, 0x10, 0xf7];
    normalize_received(&mut notices);
    assert_eq!(notices, [0xb0, 69, 0x7f, 0xc0, 0x10, 0xf7]);

    let on_interface_one =
        super::super::read_packet(VARIAX_VIA_PODXT_LIVE, Duration::ZERO, |_, target, _| {
            target[0] = 0xf2;
            Ok(1)
        })
        .unwrap()
        .unwrap();
    assert_eq!(on_interface_one, [0xf2]);
}

#[cfg(target_os = "linux")]
#[test]
fn driver_probe_checks_the_exact_attachment_and_interface() {
    use std::{fs, os::unix::fs::symlink};
    let root = std::env::temp_dir().join(format!("podxt-usb-binding-{}", std::process::id()));
    let device = root.join("8-2.1");
    let interface = root.join("8-2.1:1.0");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&device).unwrap();
    fs::create_dir(&interface).unwrap();
    fs::write(device.join("devnum"), "11\n").unwrap();
    let check = |address| binding(&root, 8, &[2, 1], address);
    assert_eq!(check(11), Ok(false));
    symlink("/drivers/snd-usb-variax", interface.join("driver")).unwrap();
    assert!(check(11).unwrap_err().contains("unexpected driver"));
    fs::remove_file(interface.join("driver")).unwrap();
    symlink("/drivers/snd_usb_pod", interface.join("driver")).unwrap();
    assert_eq!(check(11), Ok(true));
    assert!(check(12).unwrap_err().contains("attachment changed"));
    fs::remove_dir_all(&root).unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn driver_return_checks_binding_even_if_attach_succeeds_and_retries_once() {
    use std::cell::Cell;
    for attached_after in [Some(1), Some(2), None] {
        let calls = Cell::new(0);
        let waits = Cell::new(0);
        let result = restore_binding(
            || Ok(Some(calls.get()) == attached_after),
            || {
                calls.set(calls.get() + 1);
                Ok(())
            },
            |duration| {
                assert_eq!(duration, Duration::from_millis(150));
                waits.set(waits.get() + 1);
            },
        );
        assert_eq!(result.is_ok(), attached_after.is_some());
        assert_eq!(calls.get(), attached_after.unwrap_or(2));
        assert_eq!(waits.get(), usize::from(attached_after != Some(1)));
    }

    let error = restore_binding(
        || Err("attachment changed".into()),
        || panic!("changed attachment must not be reattached"),
        |_| panic!("changed attachment requires no retry"),
    );
    assert_eq!(error, Err("attachment changed".into()));
}
