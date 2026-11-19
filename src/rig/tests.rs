#![forbid(unsafe_code)]

use super::capability;
use super::*;
use crate::firmware::synthetic::synthetic_container;
use crate::firmware::{
    PODXT_LIVE_PRODUCT_ID, PODXT_LIVE_VERSION, VARIAX_300_600_PRODUCT_ID, VARIAX_300_600_VERSION,
    VARIAX_500_700_PRODUCT_ID,
};
use crate::linux_transport::PODXT_LIVE_VARIAX_PROPERTIES;

const PRESET: &[u8] = include_bytes!("../../tests/fixtures/editor-preset.vwm");
const BANK: &[u8] = include_bytes!("../../tests/fixtures/synthetic-bank.vwb");

fn synthetic_h5ep() -> Vec<u8> {
    let mut bytes = vec![0; crate::pod_preset::H5EP_MIN_BYTES];
    bytes[..4].copy_from_slice(b"H5EP");
    bytes[4..8].copy_from_slice(&1u32.to_be_bytes());
    let body = &mut bytes[crate::pod_preset::H5EP_HEADER_BYTES..];
    for offset in [0x228, 0x328] {
        body[offset..offset + 4].copy_from_slice(&0x0100_0000u32.to_be_bytes());
    }
    for index in 0..8 {
        let offset = 0x428 + index * 0x100;
        body[offset..offset + 4].copy_from_slice(&0x0200_0000u32.to_be_bytes());
    }
    bytes
}

fn synthetic_h5es() -> Vec<u8> {
    let mut bytes = vec![0; crate::pod_preset::H5ES_MIN_BYTES];
    bytes[..4].copy_from_slice(b"H5ES");
    bytes[4..8].copy_from_slice(&1u32.to_be_bytes());
    for index in 0..crate::pod_preset::H5ES_PRESET_COUNT {
        let start =
            crate::pod_preset::H5ES_HEADER_BYTES + index * crate::pod_preset::H5EP_MIN_BYTES;
        bytes[start..start + crate::pod_preset::H5EP_MIN_BYTES].copy_from_slice(&synthetic_h5ep());
    }
    bytes
}

fn with_device_id(bytes: &[u8], device_id: u32) -> Vec<u8> {
    let mut owned = bytes.to_vec();
    owned[24..28].copy_from_slice(&device_id.to_be_bytes());
    owned
}

#[test]
fn shared_hd_editor_session_keeps_file_context_distinct_from_physical_identity() {
    let session = Session::initial_rig();
    let mut library = session.new_hd_editor_library();
    let mut bytes = synthetic_h5ep();
    // These arbitrary words are accepted and retained as source-qualified
    // Mac writer context, not treated as target or firmware declarations.
    bytes[0x10..0x14].copy_from_slice(&0x0014_0005u32.to_be_bytes());
    bytes[0x14..0x18].copy_from_slice(&0x0000_0000u32.to_be_bytes());
    let body = crate::pod_preset::H5EP_HEADER_BYTES;
    let block = 0x428;
    let row = body + block + 0x10;
    bytes[body + block..body + block + 4].copy_from_slice(&0x0205_000du32.to_be_bytes());
    bytes[row..row + 4].copy_from_slice(&0x3f10_0001u32.to_be_bytes());
    bytes[row + 4..row + 8].copy_from_slice(&0.25f32.to_bits().to_be_bytes());
    let original = bytes.clone();

    let summary = session
        .import_hd_editor_file(&mut library, bytes)
        .expect("offline H5EP import");
    assert_eq!(summary.kind, "H5EP preset");
    assert_eq!(summary.file_family, OfflineFileFamily::PodHdEdit);
    assert_eq!(
        summary.physical_target,
        PhysicalTargetApplicability::Unknown
    );
    assert_eq!(summary.header_words_10_14, Some((0x0014_0005, 0)));
    assert_eq!(library.file(summary.index).unwrap().as_bytes(), original);
    assert_eq!(
        session.set_hd_screamer_control_percent(
            &mut library,
            summary.index,
            0,
            crate::pod_preset::ScreamerControl::Drive,
            40,
        ),
        Ok(())
    );
    assert!(library.is_dirty(summary.index));

    let directory = tempfile::tempdir().expect("temporary output directory");
    let output = directory.path().join("edited.h5p");
    session
        .save_hd_editor_copy(&library, &output, summary.index)
        .expect("new offline copy");
    let saved = std::fs::read(&output).expect("saved bytes");
    assert_eq!(&saved[..0x28], &original[..0x28]);
    let value_at = row + 4;
    assert_eq!(&saved[..value_at], &original[..value_at]);
    assert_eq!(&saved[value_at + 4..], &original[value_at + 4..]);
    assert!(matches!(
        session.save_hd_editor_copy(&library, &output, summary.index),
        Err(Rejection::Format { .. })
    ));

    let mut mapped_context = synthetic_h5ep();
    mapped_context[0x10..0x14].copy_from_slice(&0x0021_0001u32.to_be_bytes());
    mapped_context[0x14..0x18].copy_from_slice(&0x0003_0262u32.to_be_bytes());
    let mapped_summary = session
        .import_hd_editor_file(&mut library, mapped_context)
        .expect("mapped-like context remains structural data");
    assert_eq!(
        mapped_summary.physical_target,
        PhysicalTargetApplicability::Unknown
    );
    assert_eq!(
        mapped_summary.header_words_10_14,
        Some((0x0021_0001, 0x0003_0262))
    );

    let parent_bytes = synthetic_h5es();
    let parent = session
        .import_hd_editor_file(&mut library, parent_bytes.clone())
        .expect("offline H5ES import");
    let child = session
        .extract_hd_editor_preset(&mut library, parent.index, 0, 17)
        .expect("common child extraction");
    assert_eq!(child.kind, "H5EP preset");
    assert_eq!(child.header_words_10_14, Some((0, 0)));
    assert_eq!(library.file(parent.index).unwrap().as_bytes(), parent_bytes);

    let podxt = crate::pod_preset::fixture(
        crate::pod_preset::ContainerKind::L6pa,
        PODXT_LIVE_PRODUCT_ID,
        1,
        &[(crate::pod_preset::AMP_SLOT, 1)],
    );
    let rejection = session.import_hd_editor_file(&mut library, podxt);
    assert!(matches!(rejection, Err(Rejection::Format { .. })));
    assert_eq!(library.len(), 4, "rejected PODxt file is not imported");

    let mut malformed = synthetic_h5ep();
    malformed[7] = 2;
    assert!(matches!(
        session.import_hd_editor_file(&mut library, malformed),
        Err(Rejection::Format { .. })
    ));
    assert_eq!(library.len(), 4, "malformed file is not imported");
    assert_eq!(
        capability::capability(ProductProfile::PodHd500x, Operation::OpenPodPreset).state,
        CapabilityState::Unverified
    );

    // Unrecognized but structurally valid H5EP context remains opaque.
    let mut unknown = synthetic_h5ep();
    unknown[8..0x28].fill(0xa5);
    let unknown_summary = session
        .import_hd_editor_file(&mut library, unknown)
        .expect("unknown context is preserved");
    assert_eq!(
        unknown_summary.physical_target,
        PhysicalTargetApplicability::Unknown
    );
    assert_eq!(
        unknown_summary.header_words_10_14,
        Some((0xa5a5_a5a5, 0xa5a5_a5a5))
    );
}

#[test]
fn initial_rig_is_one_usb_pod_with_two_logical_roles() {
    let session = Session::initial_rig();
    assert!(session.live_backend_is_absent());
    assert!(LIVE_BACKEND_INITIALIZER.is_none());
    let host = session.host().expect("initial rig has a static host");
    assert_eq!(host.vendor_id, 0x0e41);
    assert_eq!(host.product_id, 0x4650);
    assert_eq!(host.roles.len(), 2);
    assert_eq!(host.roles[0].kind, RoleKind::Guitar);
    assert_eq!(host.roles[0].profile, ProductProfile::Variax600);
    assert_eq!(host.roles[0].interface_number, 1);
    assert_eq!(host.roles[0].driver_info, 0);
    assert_eq!(host.roles[1].kind, RoleKind::Pod);
    assert_eq!(host.roles[1].profile, ProductProfile::PodxtLive);
    assert_eq!(host.roles[1].interface_number, 0);
    assert_eq!(host.roles[1].driver_info, 5);
    assert_ne!(
        u32::from(host.roles[0].interface_number),
        PODXT_LIVE_VARIAX_PROPERTIES.ctrl_if
    );
    assert_eq!(WORKBENCH_USB_IS_A_SEPARATE_MATCH.product_id, 0x534d);
    assert!(
        host.roles
            .iter()
            .all(|role| role.profile.archived_firmware_product() != Some(0x534d))
    );
    assert!(Session::extension(ProductProfile::Variax600).is_err());
    assert!(Session::extension(ProductProfile::PodxtLive).is_err());
}

#[test]
fn guitar_files_stay_on_the_variax_backend() {
    let session = Session::initial_rig();
    let preset = session
        .open_preset(RoleKind::Guitar, PRESET)
        .expect("synthetic preset");
    assert_eq!(preset.name, "Synthetic");
    assert_eq!(preset.device_id, VARIAX_300_600_PRODUCT_ID);
    let bank = session.open_guitar_bank(BANK).expect("synthetic bank");
    assert!(bank.fixed);
    assert_eq!(bank.presets, 5);

    let pod_open = session.open_preset(RoleKind::Pod, b"not a preset");
    assert_eq!(
        pod_open,
        Err(Rejection::Unsupported {
            profile: ProductProfile::PodxtLive,
            operation: Operation::OpenGuitarPreset,
        })
    );
    assert!(matches!(
        session.open_preset(RoleKind::Guitar, b"not a preset"),
        Err(Rejection::Format { .. })
    ));
    assert_eq!(
        session.open_preset(
            RoleKind::Guitar,
            &with_device_id(PRESET, VARIAX_500_700_PRODUCT_ID)
        ),
        Err(Rejection::DifferentModel {
            profile: ProductProfile::Variax600,
            device_id: VARIAX_500_700_PRODUCT_ID,
        })
    );
    assert_eq!(
        session.open_preset(
            RoleKind::Guitar,
            &with_device_id(PRESET, PODXT_LIVE_PRODUCT_ID)
        ),
        Err(Rejection::CrossFamily {
            from: DeviceFamily::PodXt,
            to: DeviceFamily::VariaxWorkbench,
            operation: Operation::OpenGuitarPreset,
        })
    );

    let directory = tempfile::tempdir().expect("temp dir");
    let copy = directory.path().join("copy.vwm");
    session.save_guitar_copy(&copy, PRESET).expect("new copy");
    assert!(session.save_guitar_copy(&copy, PRESET).is_err());
    assert_eq!(std::fs::read(&copy).expect("copy"), PRESET);

    session
        .accept_family_math(RoleKind::Guitar, VARIAX_300_600_PRODUCT_ID)
        .expect("same-family id");
    assert!(matches!(
        session.accept_family_math(RoleKind::Guitar, PODXT_LIVE_PRODUCT_ID),
        Err(Rejection::CrossFamily { .. })
    ));
    assert_eq!(
        session.accept_family_math(RoleKind::Guitar, VARIAX_500_700_PRODUCT_ID),
        Err(Rejection::DifferentModel {
            profile: ProductProfile::Variax600,
            device_id: VARIAX_500_700_PRODUCT_ID,
        })
    );
    assert_eq!(
        session.accept_family_math(RoleKind::Pod, VARIAX_300_600_PRODUCT_ID),
        Err(Rejection::Unsupported {
            profile: ProductProfile::PodxtLive,
            operation: Operation::ApplyFamilyMath,
        })
    );
}

#[test]
fn firmware_inspection_rejects_the_other_family() {
    let session = Session::initial_rig();
    let variax = synthetic_container(
        VARIAX_300_600_PRODUCT_ID,
        VARIAX_300_600_VERSION,
        b"variax-image",
    );
    let pod = synthetic_container(PODXT_LIVE_PRODUCT_ID, PODXT_LIVE_VERSION, b"pod-image");
    let wrong_version = synthetic_container(
        VARIAX_300_600_PRODUCT_ID,
        VARIAX_300_600_VERSION.wrapping_add(1),
        b"variax-image",
    );
    let guitar = session
        .inspect_firmware(RoleKind::Guitar, &variax)
        .expect("variax container");
    assert_eq!(guitar.product_id, VARIAX_300_600_PRODUCT_ID);
    assert_eq!(guitar.version, VARIAX_300_600_VERSION);
    assert!(guitar.integrity_is_not_authentication);
    assert_eq!(
        session.inspect_firmware(RoleKind::Pod, &variax),
        Err(Rejection::WrongProduct {
            declared: VARIAX_300_600_PRODUCT_ID,
            expected: PODXT_LIVE_PRODUCT_ID,
        })
    );
    let pod_summary = session
        .inspect_firmware(RoleKind::Pod, &pod)
        .expect("pod container");
    assert_eq!(pod_summary.product_id, PODXT_LIVE_PRODUCT_ID);
    assert_eq!(
        session.inspect_firmware(RoleKind::Guitar, &pod),
        Err(Rejection::WrongProduct {
            declared: PODXT_LIVE_PRODUCT_ID,
            expected: VARIAX_300_600_PRODUCT_ID,
        })
    );
    assert_eq!(
        session.inspect_firmware(RoleKind::Guitar, &wrong_version),
        Err(Rejection::WrongVersion {
            declared: VARIAX_300_600_VERSION.wrapping_add(1),
            expected: VARIAX_300_600_VERSION,
        })
    );

    let extension = Session::extension(ProductProfile::PodHd500x).expect("extension profile");
    assert_eq!(
        extension.inspect_extension_firmware(&variax),
        Err(Rejection::Unverified {
            profile: ProductProfile::PodHd500x,
            operation: Operation::InspectFirmware,
        })
    );
}

#[test]
fn versions_that_are_unknown_stay_unknown() {
    let session = Session::initial_rig();
    match session
        .version(RoleKind::Guitar, Component::ArchivedContainer)
        .expect("catalog")
    {
        VersionKnowledge::ArchivedDeclaration { word, label, .. } => {
            assert_eq!(word, VARIAX_300_600_VERSION);
            assert_eq!(label, "3.10");
        }
        other => panic!("archived guitar container was {other:?}"),
    }
    assert!(matches!(
        session
            .version(RoleKind::Guitar, Component::InstalledUnitRevision)
            .expect("unit"),
        VersionKnowledge::Unknown { .. }
    ));
    assert!(matches!(
        session
            .version(RoleKind::Guitar, Component::InstalledUsbFirmware)
            .expect("guitar usb component"),
        VersionKnowledge::NotApplicable { .. }
    ));
    match session
        .version(RoleKind::Pod, Component::InstalledUsbFirmware)
        .expect("pod usb component")
    {
        VersionKnowledge::Unknown { evidence } => {
            assert!(evidence.contains("#20"));
            assert!(evidence.contains("bcdDevice"));
            assert!(!VersionKnowledge::Unknown { evidence }.is_known_value());
        }
        other => panic!("usb component was {other:?}"),
    }
    for profile in [
        ProductProfile::WorkbenchHd,
        ProductProfile::Jtv69,
        ProductProfile::PodHd500x,
    ] {
        let extension = Session::extension(profile).expect("extension");
        assert!(profile.archived_firmware_product().is_none());
        assert!(matches!(
            extension
                .extension_version(Component::ArchivedContainer)
                .expect("version"),
            VersionKnowledge::NotEstablished { .. }
        ));
        assert_eq!(
            extension.extension_math(VARIAX_300_600_PRODUCT_ID),
            Err(Rejection::Unverified {
                profile,
                operation: Operation::ApplyFamilyMath,
            })
        );
    }
    assert_ne!(
        ProductProfile::WorkbenchHd.family(),
        ProductProfile::Variax600.family()
    );
    assert_ne!(
        ProductProfile::Jtv69.family(),
        ProductProfile::WorkbenchHd.family()
    );
    assert_ne!(
        ProductProfile::PodHd500x.family(),
        ProductProfile::PodxtLive.family()
    );
    assert_ne!(ProductProfile::Jtv69, ProductProfile::WorkbenchHd);
}

#[test]
fn physical_claims_and_planned_work_do_not_start_a_backend() {
    let session = Session::initial_rig();
    for profile in ProductProfile::ALL {
        for operation in Operation::ALL {
            let claimed = capability::resolve(profile, operation, Some(CapabilityState::Offline));
            let bare = capability::resolve(profile, operation, None);
            assert_eq!(claimed, bare);
            if operation.is_physical() {
                assert_eq!(claimed, CapabilityState::Locked);
            } else if profile.is_extension() {
                assert_eq!(claimed, CapabilityState::Unverified);
            }
        }
    }

    let mut attempts = 0;
    let locked = session.reject_physical(
        RoleKind::Pod,
        Operation::OpenAudioStream,
        Some(CapabilityState::Offline),
        || {
            attempts += 1;
            Ok(())
        },
    );
    assert_eq!(attempts, 0);
    assert_eq!(
        locked,
        Err(Rejection::Locked {
            operation: Operation::OpenAudioStream,
        })
    );
    let extension = Session::extension(ProductProfile::Jtv69).expect("jtv");
    let extension_locked = extension.reject_extension_physical(
        Operation::EnumerateUsb,
        Some(CapabilityState::Offline),
        || {
            attempts += 1;
            Ok(())
        },
    );
    assert_eq!(attempts, 0);
    assert_eq!(
        extension_locked,
        Err(Rejection::Locked {
            operation: Operation::EnumerateUsb,
        })
    );

    assert_eq!(
        session.capability(RoleKind::Pod, Operation::EditPodPreset),
        Ok(Capability {
            profile: ProductProfile::PodxtLive,
            operation: Operation::EditPodPreset,
            state: CapabilityState::Offline,
            note: capability::capability(ProductProfile::PodxtLive, Operation::EditPodPreset).note,
        })
    );
    assert_eq!(
        session
            .capability(RoleKind::Pod, Operation::OpenPodPreset)
            .expect("pod open")
            .state,
        CapabilityState::Offline
    );
    assert_eq!(
        session
            .capability(RoleKind::Guitar, Operation::OpenPodPreset)
            .expect("guitar pod open")
            .state,
        CapabilityState::Unsupported
    );
    assert_eq!(
        session
            .capability(RoleKind::Pod, Operation::SelectRecordRoute)
            .expect("route")
            .state,
        CapabilityState::Offline
    );
    assert_eq!(
        session
            .capability(RoleKind::Guitar, Operation::SelectRecordRoute)
            .expect("guitar route")
            .state,
        CapabilityState::Unsupported
    );
    assert_eq!(
        session
            .capability(RoleKind::Pod, Operation::ReadInstalledUsbComponent)
            .expect("usb read")
            .state,
        CapabilityState::Locked
    );
}

#[test]
fn pod_record_route_stays_offline_while_capture_stays_locked() {
    let session = Session::initial_rig();
    assert!(LIVE_BACKEND_INITIALIZER.is_none());
    assert!(session.live_backend_is_absent());
    let planned = session
        .select_record_route(crate::pod_audio::RecordRoute::SendCleanGuitar, 66, 2)
        .expect("clean route");
    assert!(!planned.stream_opened);
    assert!(planned.outbound.is_empty());
    assert_eq!(planned.behavior.mode_number, 2);
    assert!(!planned.coexistence.guitar.source_initializes_pcm);
    assert!(planned.coexistence.pod.source_initializes_pcm);

    assert_eq!(
        session
            .capability(RoleKind::Guitar, Operation::SelectRecordRoute)
            .expect("guitar")
            .state,
        CapabilityState::Unsupported
    );
    let extension = Session::extension(ProductProfile::PodHd500x).expect("extension");
    assert_eq!(
        extension.select_record_route(crate::pod_audio::RecordRoute::SendProcessedGuitar, 66, 1),
        Err(Rejection::Unverified {
            profile: ProductProfile::PodHd500x,
            operation: Operation::SelectRecordRoute,
        })
    );

    let mut attempts = 0;
    let locked = session.reject_physical(
        RoleKind::Pod,
        Operation::StartAudioCapture,
        Some(CapabilityState::Offline),
        || {
            attempts += 1;
            Ok(())
        },
    );
    assert_eq!(attempts, 0);
    assert_eq!(
        locked,
        Err(Rejection::Locked {
            operation: Operation::StartAudioCapture,
        })
    );
    assert_eq!(
        session.select_record_route(crate::pod_audio::RecordRoute::SendProcessedGuitar, 64, 1),
        Err(Rejection::Format {
            detail: "period byte count is not a multiple of the 6-byte frame".into(),
        })
    );
}

#[test]
fn pod_tone_open_rename_and_save_stay_offline_and_reject_variax() {
    let session = Session::initial_rig();
    assert!(LIVE_BACKEND_INITIALIZER.is_none());
    assert!(session.live_backend_is_absent());
    let original = crate::pod_preset::fixture(
        crate::pod_preset::ContainerKind::L6as,
        PODXT_LIVE_PRODUCT_ID,
        0x0102_0304,
        &[
            (crate::pod_preset::AMP_SLOT, 0x1111_0002),
            (crate::pod_preset::CAB_SLOT, 0x2222_0003),
            (0x0003_00aa, 0x3333_0004),
        ],
    );
    let summary = session
        .open_pod_preset(RoleKind::Pod, &original)
        .expect("pod tone");
    assert_eq!(summary.container, "L6AS");
    assert_eq!(summary.device_id, PODXT_LIVE_PRODUCT_ID);
    assert_eq!(summary.device_version, 0x0102_0304);
    assert_ne!(summary.device_version, PODXT_LIVE_VERSION);
    assert_eq!(summary.amp_model_id, Some(0x1111_0002));
    assert_eq!(summary.cab_model_id, Some(0x2222_0003));
    assert_eq!(summary.amp_name, None);
    assert_eq!(summary.cab_name, None);
    assert_eq!(summary.undecoded_slots, 1);
    assert_eq!(summary.recorded_parameter_chunks, 3);
    assert_eq!(summary.undecoded_parameter_chunks, 3);
    assert_eq!(
        session.open_pod_preset(RoleKind::Guitar, &original),
        Err(Rejection::Unsupported {
            profile: ProductProfile::Variax600,
            operation: Operation::OpenPodPreset,
        })
    );
    assert_eq!(
        session.open_pod_preset(RoleKind::Pod, PRESET),
        Err(Rejection::CrossFamily {
            from: DeviceFamily::VariaxWorkbench,
            to: DeviceFamily::PodXt,
            operation: Operation::OpenPodPreset,
        })
    );
    let bass = crate::pod_preset::fixture(
        crate::pod_preset::ContainerKind::L6pa,
        0x0003_000b,
        1,
        &[(crate::pod_preset::AMP_SLOT, 1)],
    );
    assert!(matches!(
        session.open_pod_preset(RoleKind::Pod, &bass),
        Err(Rejection::Format { detail }) if detail.contains("0003000b")
    ));
    let extension = Session::extension(ProductProfile::PodHd500x).expect("extension");
    assert_eq!(
        extension.open_pod_preset(RoleKind::Pod, &original),
        Err(Rejection::Unverified {
            profile: ProductProfile::PodHd500x,
            operation: Operation::OpenPodPreset,
        })
    );
    let renamed = session
        .rename_pod_preset(RoleKind::Pod, &original, "Stage")
        .expect("rename");
    assert_ne!(renamed, original);
    let again = session
        .open_pod_preset(RoleKind::Pod, &renamed)
        .expect("renamed");
    assert_eq!(again.name, "Stage");
    assert_eq!(again.amp_model_id, Some(0x1111_0002));
    assert!(
        session
            .rename_pod_preset(RoleKind::Pod, PRESET, "Nope")
            .is_err()
    );
    let directory = tempfile::tempdir().expect("temp dir");
    let copy = directory.path().join("tone.bin");
    session
        .save_pod_copy(RoleKind::Pod, &copy, &renamed)
        .expect("new copy");
    assert!(
        session
            .save_pod_copy(RoleKind::Pod, &copy, &renamed)
            .is_err()
    );
    assert_eq!(std::fs::read(&copy).expect("copy"), renamed);
    assert!(
        session
            .save_pod_copy(RoleKind::Guitar, &copy, &renamed)
            .is_err()
    );
    assert_eq!(std::fs::read(&copy).expect("copy remains"), renamed);
    let mut attempts = 0;
    let locked = session.reject_physical(
        RoleKind::Pod,
        Operation::SendControl,
        Some(CapabilityState::Offline),
        || {
            attempts += 1;
            Ok(())
        },
    );
    assert_eq!(attempts, 0);
    assert_eq!(
        locked,
        Err(Rejection::Locked {
            operation: Operation::SendControl,
        })
    );
    assert_eq!(
        session
            .capability(RoleKind::Pod, Operation::SwitchBank)
            .expect("bank")
            .state,
        CapabilityState::Locked
    );
}

#[test]
fn offline_recording_gate_does_not_call_capture_or_open_a_stream() {
    let session = Session::initial_rig();
    assert!(LIVE_BACKEND_INITIALIZER.is_none());
    assert!(session.live_backend_is_absent());
    let mut attempts = 0;
    let opened = crate::pod_audio::plan(
        crate::pod_audio::RecordRoute::SendCleanGuitar,
        crate::recording::OFFLINE_ROUTE_PERIOD_BYTES,
        crate::recording::OFFLINE_ROUTE_PERIODS,
        || attempts += 1,
    )
    .expect("plan");
    assert_eq!(attempts, 0);
    assert!(!opened.stream_opened);
    assert!(opened.outbound.is_empty());
    let gate = session
        .gate_offline_recording(
            crate::pod_audio::RecordRoute::SendCleanGuitar,
            crate::recording::OFFLINE_ROUTE_PERIOD_BYTES,
            crate::recording::OFFLINE_ROUTE_PERIODS,
            || {
                attempts += 1;
                Ok(())
            },
        )
        .expect("gate");
    assert_eq!(attempts, 0);
    assert!(!gate.stream_opened);
    assert_eq!(gate.outbound_len, 0);
    assert!(gate.capture_stays_locked);
    assert!(crate::pod_audio::iso_buffers_if_known().is_err());
    assert!(crate::pod_audio::max_packet_size().is_err());

    let extension = Session::extension(ProductProfile::PodHd500x).expect("extension");
    let extension_gate = extension.gate_offline_recording(
        crate::pod_audio::RecordRoute::SendProcessedGuitar,
        crate::recording::OFFLINE_ROUTE_PERIOD_BYTES,
        crate::recording::OFFLINE_ROUTE_PERIODS,
        || {
            attempts += 1;
            Ok(())
        },
    );
    assert_eq!(attempts, 0);
    assert_eq!(
        extension_gate,
        Err(Rejection::Unverified {
            profile: ProductProfile::PodHd500x,
            operation: Operation::SelectRecordRoute,
        })
    );
    assert_eq!(
        session
            .capability(RoleKind::Guitar, Operation::SelectRecordRoute)
            .expect("guitar")
            .state,
        CapabilityState::Unsupported
    );
    let locked = session.reject_physical(
        RoleKind::Pod,
        Operation::StartAudioCapture,
        Some(CapabilityState::Offline),
        || {
            attempts += 1;
            Ok(())
        },
    );
    assert_eq!(attempts, 0);
    assert_eq!(
        locked,
        Err(Rejection::Locked {
            operation: Operation::StartAudioCapture,
        })
    );
    assert_eq!(
        session.gate_offline_recording(
            crate::pod_audio::RecordRoute::SendCleanGuitar,
            64,
            1,
            || {
                attempts += 1;
                Ok(())
            },
        ),
        Err(Rejection::Format {
            detail: "period byte count is not a multiple of the 6-byte frame".into(),
        })
    );
    assert_eq!(attempts, 0);
}

#[test]
fn pod_hd500x_identity_stays_on_its_profile_and_does_not_open_other_families() {
    assert!(LIVE_BACKEND_INITIALIZER.is_none());
    let session = Session::initial_rig();
    assert!(session.live_backend_is_absent());
    assert_eq!(
        session.pod_hd500x(),
        Err(crate::pod_hd500x::ProfileMiss::InitialRig)
    );
    let host = session.host().expect("initial host");
    assert_eq!(host.product_id, 0x4650);
    let hd = Session::extension(ProductProfile::PodHd500x).expect("hd500x");
    assert!(hd.live_backend_is_absent());
    assert!(hd.host().is_none());
    let record = hd.pod_hd500x().expect("hd500x record");
    assert_ne!(host.product_id, record.usb_product_id);
    assert_ne!(
        ProductProfile::PodHd500x.family(),
        ProductProfile::PodxtLive.family()
    );
    assert_ne!(
        ProductProfile::PodHd500x.family(),
        ProductProfile::Jtv69.family()
    );
    assert_ne!(
        ProductProfile::PodHd500x.family(),
        ProductProfile::WorkbenchHd.family()
    );
    assert!(
        ProductProfile::PodHd500x
            .archived_firmware_product()
            .is_none()
    );
    for profile in [ProductProfile::WorkbenchHd, ProductProfile::Jtv69] {
        let other = Session::extension(profile).expect("other extension");
        assert_eq!(
            other.pod_hd500x(),
            Err(crate::pod_hd500x::ProfileMiss::DifferentExtension)
        );
        let note = capability::capability(profile, Operation::OpenPodPreset).note;
        assert!(!note.contains("4159"));
        assert!(!note.contains("48000"));
    }
    let open_note =
        capability::capability(ProductProfile::PodHd500x, Operation::OpenPodPreset).note;
    let route_note =
        capability::capability(ProductProfile::PodHd500x, Operation::SelectRecordRoute).note;
    let firmware_note =
        capability::capability(ProductProfile::PodHd500x, Operation::InspectFirmware).note;
    assert!(open_note.contains("H5EP/H5ES/H5EB framing"));
    assert!(open_note.contains("does not prove physical target compatibility"));
    assert!(route_note.contains("48000"));
    assert!(route_note.contains("EngineFormatLock"));
    assert!(firmware_note.contains("6260"));
    assert!(firmware_note.contains("1.04"));
    match hd
        .extension_version(Component::ArchivedContainer)
        .expect("flash label")
    {
        VersionKnowledge::NotEstablished { evidence } => {
            assert!(evidence.contains("6260"));
            assert!(!VersionKnowledge::NotEstablished { evidence }.is_known_value());
        }
        other => panic!("flash label was {other:?}"),
    }
    match hd
        .extension_version(Component::InstalledUsbFirmware)
        .expect("usb label")
    {
        VersionKnowledge::NotEstablished { evidence } => assert!(evidence.contains("1.04")),
        other => panic!("usb label was {other:?}"),
    }
    let tone = crate::pod_preset::fixture(
        crate::pod_preset::ContainerKind::L6pa,
        PODXT_LIVE_PRODUCT_ID,
        1,
        &[(crate::pod_preset::AMP_SLOT, 1)],
    );
    let h5ep = synthetic_h5ep();
    assert!(matches!(
        crate::pod_preset::PodPresetFile::parse(h5ep.clone()),
        Ok(crate::pod_preset::PodPresetFile::H5ep(_))
    ));
    assert_eq!(
        Session::initial_rig().open_pod_preset(RoleKind::Pod, &h5ep),
        Err(Rejection::Format {
            detail: "expected an individual FORM tone; banks and collections are not POD presets"
                .into(),
        })
    );
    assert_eq!(
        Session::initial_rig().open_pod_preset(RoleKind::Guitar, &h5ep),
        Err(Rejection::Unsupported {
            profile: ProductProfile::Variax600,
            operation: Operation::OpenPodPreset,
        })
    );
    for profile in [
        ProductProfile::PodHd500x,
        ProductProfile::WorkbenchHd,
        ProductProfile::Jtv69,
    ] {
        assert_eq!(
            Session::extension(profile)
                .unwrap()
                .open_pod_preset(RoleKind::Pod, &h5ep),
            Err(Rejection::Unverified {
                profile,
                operation: Operation::OpenPodPreset,
            })
        );
    }
    assert_eq!(
        hd.open_pod_preset(RoleKind::Pod, &tone),
        Err(Rejection::Unverified {
            profile: ProductProfile::PodHd500x,
            operation: Operation::OpenPodPreset,
        })
    );
    assert_eq!(
        hd.open_pod_preset(RoleKind::Pod, PRESET),
        Err(Rejection::Unverified {
            profile: ProductProfile::PodHd500x,
            operation: Operation::OpenPodPreset,
        })
    );
    assert_eq!(
        hd.select_record_route(crate::pod_audio::RecordRoute::SendCleanGuitar, 66, 2),
        Err(Rejection::Unverified {
            profile: ProductProfile::PodHd500x,
            operation: Operation::SelectRecordRoute,
        })
    );
    let pod_image = synthetic_container(PODXT_LIVE_PRODUCT_ID, PODXT_LIVE_VERSION, b"pod-image");
    let variax_image = synthetic_container(
        VARIAX_300_600_PRODUCT_ID,
        VARIAX_300_600_VERSION,
        b"variax-image",
    );
    for image in [&pod_image, &variax_image] {
        assert_eq!(
            hd.inspect_extension_firmware(image),
            Err(Rejection::Unverified {
                profile: ProductProfile::PodHd500x,
                operation: Operation::InspectFirmware,
            })
        );
    }
    let mut attempts = 0;
    for operation in [
        Operation::OpenAudioStream,
        Operation::FlashFirmware,
        Operation::BindDriver,
        Operation::EnumerateUsb,
    ] {
        assert_eq!(
            hd.reject_extension_physical(operation, Some(CapabilityState::Offline), || {
                attempts += 1;
                Ok(())
            }),
            Err(Rejection::Locked { operation })
        );
    }
    assert_eq!(attempts, 0);
    assert_eq!(
        capability::resolve(
            ProductProfile::PodHd500x,
            Operation::OpenPodPreset,
            Some(CapabilityState::Offline)
        ),
        CapabilityState::Unverified
    );
}
