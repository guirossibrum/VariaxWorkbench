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

fn with_device_id(bytes: &[u8], device_id: u32) -> Vec<u8> {
    let mut owned = bytes.to_vec();
    owned[24..28].copy_from_slice(&device_id.to_be_bytes());
    owned
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
