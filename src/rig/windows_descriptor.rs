#![forbid(unsafe_code)]
//! Safe offline inspection of a complete POD HD500X Windows-driver descriptor result.
//!
//! This is not a Windows request builder or a transport adapter. Inputs are
//! caller-owned bytes and completion facts from a synthetic/offline source.

use crate::pod_hd500x;

pub const DESCRIPTOR_LENGTH: usize = 18;
pub const USB_DEVICE_DESCRIPTOR_TYPE: u8 = 1;
pub const OBSERVED_DRIVER_PRODUCT_ID: u16 = 0x4159;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompletionStatus {
    Pending,
    Success,
    Failed(i32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReturnedBytes<'a> {
    pub bytes: &'a [u8],
    pub status: CompletionStatus,
    /// Actual transfer length reported by the offline source, when available.
    pub actual_byte_count: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RejectReason {
    NonNegativeFailureStatus,
    MissingActualByteCount,
    ActualByteCountDoesNotMatchSlice,
    IncompleteDescriptor,
    InvalidDescriptorLength,
    WrongDescriptorType,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceDescriptor {
    /// Preserves all descriptor bytes, including fields this inspector does
    /// not interpret.
    pub raw: [u8; DESCRIPTOR_LENGTH],
    pub vendor_id: u16,
    pub product_id: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordListCounts {
    pub first: u8,
    pub second: u8,
}

/// Original binary behavior is admitted only for the observed PID branch.
/// Other product IDs are intentionally unmapped, not assigned a default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DriverRecordLists {
    Observed(RecordListCounts),
    OtherProductUnmapped { product_id: u16 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hd500xPreflight {
    IdentityMatchesInf,
    NotQualified { vendor_id: u16, product_id: u16 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HardwareStatus {
    UnverifiedAndLocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndpointKnowledge {
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InspectionReport {
    pub source_version: &'static str,
    pub source_inf_sha256: &'static str,
    pub descriptor: DeviceDescriptor,
    /// The original driver branch is PID-only; this is not a route or endpoint map.
    pub driver_record_lists: DriverRecordLists,
    /// Conservative Rust qualification additionally requires the INF VID/PID pair.
    pub hd500x_preflight: Hd500xPreflight,
    pub hardware_status: HardwareStatus,
    pub endpoint_knowledge: EndpointKnowledge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Inspection {
    Pending,
    Failed { status: i32 },
    Rejected { reason: RejectReason },
    Complete(InspectionReport),
}

impl Inspection {
    pub const fn hardware_status(self) -> HardwareStatus {
        HardwareStatus::UnverifiedAndLocked
    }

    pub const fn endpoint_knowledge(self) -> EndpointKnowledge {
        EndpointKnowledge::Unknown
    }
}

/// Inspects an already returned offline result. No device request is performed.
pub fn inspect(returned: ReturnedBytes<'_>) -> Inspection {
    match returned.status {
        CompletionStatus::Pending => return Inspection::Pending,
        CompletionStatus::Failed(status) if status < 0 => {
            return Inspection::Failed { status };
        }
        CompletionStatus::Failed(_) => {
            return Inspection::Rejected {
                reason: RejectReason::NonNegativeFailureStatus,
            };
        }
        CompletionStatus::Success => {}
    }

    let Some(actual_byte_count) = returned.actual_byte_count else {
        return Inspection::Rejected {
            reason: RejectReason::MissingActualByteCount,
        };
    };
    if actual_byte_count != returned.bytes.len() {
        return Inspection::Rejected {
            reason: RejectReason::ActualByteCountDoesNotMatchSlice,
        };
    }
    if actual_byte_count != DESCRIPTOR_LENGTH {
        return Inspection::Rejected {
            reason: RejectReason::IncompleteDescriptor,
        };
    }
    let raw: [u8; DESCRIPTOR_LENGTH] = match returned.bytes.try_into() {
        Ok(raw) => raw,
        Err(_) => {
            return Inspection::Rejected {
                reason: RejectReason::IncompleteDescriptor,
            };
        }
    };
    if usize::from(raw[0]) != DESCRIPTOR_LENGTH {
        return Inspection::Rejected {
            reason: RejectReason::InvalidDescriptorLength,
        };
    }
    if raw[1] != USB_DEVICE_DESCRIPTOR_TYPE {
        return Inspection::Rejected {
            reason: RejectReason::WrongDescriptorType,
        };
    }

    let descriptor = DeviceDescriptor {
        raw,
        vendor_id: u16::from_le_bytes([raw[8], raw[9]]),
        product_id: u16::from_le_bytes([raw[10], raw[11]]),
    };
    let driver_record_lists = if descriptor.product_id == OBSERVED_DRIVER_PRODUCT_ID {
        DriverRecordLists::Observed(RecordListCounts {
            first: 2,
            second: 2,
        })
    } else {
        DriverRecordLists::OtherProductUnmapped {
            product_id: descriptor.product_id,
        }
    };
    let hd500x_preflight = if descriptor.vendor_id == pod_hd500x::USB_VENDOR_ID
        && descriptor.product_id == pod_hd500x::USB_PRODUCT_ID
    {
        Hd500xPreflight::IdentityMatchesInf
    } else {
        Hd500xPreflight::NotQualified {
            vendor_id: descriptor.vendor_id,
            product_id: descriptor.product_id,
        }
    };

    Inspection::Complete(InspectionReport {
        source_version: pod_hd500x::DRIVER_VER,
        source_inf_sha256: pod_hd500x::WINDOWS_INF.sha256,
        descriptor,
        driver_record_lists,
        hd500x_preflight,
        hardware_status: HardwareStatus::UnverifiedAndLocked,
        endpoint_knowledge: EndpointKnowledge::Unknown,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rig::{CapabilityState, Operation, ProductProfile, Rejection, Session};

    fn descriptor(vendor_id: u16, product_id: u16) -> [u8; DESCRIPTOR_LENGTH] {
        let mut bytes = [0; DESCRIPTOR_LENGTH];
        bytes[0] = DESCRIPTOR_LENGTH as u8;
        bytes[1] = USB_DEVICE_DESCRIPTOR_TYPE;
        bytes[8..10].copy_from_slice(&vendor_id.to_le_bytes());
        bytes[10..12].copy_from_slice(&product_id.to_le_bytes());
        bytes
    }

    fn success(bytes: &[u8], actual_byte_count: usize) -> ReturnedBytes<'_> {
        ReturnedBytes {
            bytes,
            status: CompletionStatus::Success,
            actual_byte_count: Some(actual_byte_count),
        }
    }

    #[test]
    fn complete_synthetic_descriptor_reports_separate_pid_branch_and_locked_preflight() {
        let bytes = descriptor(pod_hd500x::USB_VENDOR_ID, pod_hd500x::USB_PRODUCT_ID);
        let session = Session::extension(ProductProfile::PodHd500x).unwrap();
        let Inspection::Complete(report) = session
            .inspect_hd500x_windows_descriptor(success(&bytes, bytes.len()))
            .unwrap()
        else {
            panic!("complete descriptor must be inspected")
        };
        assert_eq!(report.descriptor.raw, bytes);
        assert_eq!(report.descriptor.vendor_id, 0x0e41);
        assert_eq!(report.descriptor.product_id, 0x4159);
        assert_eq!(
            report.driver_record_lists,
            DriverRecordLists::Observed(RecordListCounts {
                first: 2,
                second: 2
            })
        );
        assert_eq!(report.hd500x_preflight, Hd500xPreflight::IdentityMatchesInf);
        assert_eq!(report.source_version, "03/15/2019,4.2.7.7");
        assert_eq!(report.source_inf_sha256, pod_hd500x::WINDOWS_INF.sha256);
        assert_eq!(report.hardware_status, HardwareStatus::UnverifiedAndLocked);
        assert_eq!(report.endpoint_knowledge, EndpointKnowledge::Unknown);
        assert_eq!(
            report.hardware_status,
            session
                .inspect_hd500x_windows_descriptor(success(&bytes, bytes.len()))
                .unwrap()
                .hardware_status()
        );
        assert_eq!(
            crate::rig::capability::capability(
                ProductProfile::PodHd500x,
                Operation::InspectOfflineDeviceDescriptor
            )
            .state,
            CapabilityState::Offline
        );
    }

    #[test]
    fn pid_only_observation_does_not_qualify_a_different_vendor() {
        let bytes = descriptor(0x1234, OBSERVED_DRIVER_PRODUCT_ID);
        let Inspection::Complete(report) = inspect(success(&bytes, bytes.len())) else {
            panic!("complete descriptor must be inspected")
        };
        assert_eq!(
            report.driver_record_lists,
            DriverRecordLists::Observed(RecordListCounts {
                first: 2,
                second: 2
            })
        );
        assert_eq!(
            report.hd500x_preflight,
            Hd500xPreflight::NotQualified {
                vendor_id: 0x1234,
                product_id: OBSERVED_DRIVER_PRODUCT_ID
            }
        );
        assert_eq!(report.hardware_status, HardwareStatus::UnverifiedAndLocked);
    }

    #[test]
    fn other_and_reversed_byte_order_identities_are_not_mapped_or_qualified() {
        let other_pid = descriptor(pod_hd500x::USB_VENDOR_ID, 0x414d);
        let Inspection::Complete(report) = inspect(success(&other_pid, other_pid.len())) else {
            panic!("complete descriptor must be inspected")
        };
        assert_eq!(
            report.driver_record_lists,
            DriverRecordLists::OtherProductUnmapped { product_id: 0x414d }
        );
        assert_eq!(
            report.hd500x_preflight,
            Hd500xPreflight::NotQualified {
                vendor_id: pod_hd500x::USB_VENDOR_ID,
                product_id: 0x414d
            }
        );

        let mut reversed = descriptor(pod_hd500x::USB_VENDOR_ID, pod_hd500x::USB_PRODUCT_ID);
        reversed[8..10].reverse();
        reversed[10..12].reverse();
        let Inspection::Complete(report) = inspect(success(&reversed, reversed.len())) else {
            panic!("complete descriptor must be inspected")
        };
        assert_eq!(report.descriptor.vendor_id, 0x410e);
        assert_eq!(report.descriptor.product_id, 0x5941);
        assert_eq!(
            report.driver_record_lists,
            DriverRecordLists::OtherProductUnmapped { product_id: 0x5941 }
        );
        assert!(matches!(
            report.hd500x_preflight,
            Hd500xPreflight::NotQualified { .. }
        ));
    }

    #[test]
    fn every_short_descriptor_and_length_or_type_error_is_rejected_without_mutation() {
        let bytes = descriptor(pod_hd500x::USB_VENDOR_ID, pod_hd500x::USB_PRODUCT_ID);
        for length in 0..DESCRIPTOR_LENGTH {
            let before = bytes;
            assert_eq!(
                inspect(success(&bytes[..length], length)),
                Inspection::Rejected {
                    reason: RejectReason::IncompleteDescriptor
                },
                "length {length}"
            );
            assert_eq!(bytes, before);
        }

        let mut invalid_length = bytes;
        invalid_length[0] = 17;
        let invalid_length_before = invalid_length;
        assert_eq!(
            inspect(success(&invalid_length, invalid_length.len())),
            Inspection::Rejected {
                reason: RejectReason::InvalidDescriptorLength
            }
        );
        assert_eq!(invalid_length, invalid_length_before);

        let mut wrong_type = bytes;
        wrong_type[1] = 2;
        assert_eq!(
            inspect(success(&wrong_type, wrong_type.len())),
            Inspection::Rejected {
                reason: RejectReason::WrongDescriptorType
            }
        );
        assert_eq!(wrong_type[8..], bytes[8..]);
    }

    #[test]
    fn status_and_actual_length_must_be_consistent_before_field_access() {
        let bytes = descriptor(pod_hd500x::USB_VENDOR_ID, pod_hd500x::USB_PRODUCT_ID);
        for bytes_in_response in [&bytes[..], &[][..]] {
            let pending = inspect(ReturnedBytes {
                bytes: bytes_in_response,
                status: CompletionStatus::Pending,
                actual_byte_count: Some(DESCRIPTOR_LENGTH),
            });
            assert_eq!(pending, Inspection::Pending,);
            assert_eq!(
                pending.hardware_status(),
                HardwareStatus::UnverifiedAndLocked
            );
            assert_eq!(pending.endpoint_knowledge(), EndpointKnowledge::Unknown);
            let failed = inspect(ReturnedBytes {
                bytes: bytes_in_response,
                status: CompletionStatus::Failed(-1),
                actual_byte_count: Some(DESCRIPTOR_LENGTH),
            });
            assert_eq!(failed, Inspection::Failed { status: -1 },);
            assert_eq!(
                failed.hardware_status(),
                HardwareStatus::UnverifiedAndLocked
            );
            assert_eq!(failed.endpoint_knowledge(), EndpointKnowledge::Unknown);
        }
        assert_eq!(
            inspect(ReturnedBytes {
                bytes: &bytes,
                status: CompletionStatus::Failed(0),
                actual_byte_count: Some(bytes.len()),
            }),
            Inspection::Rejected {
                reason: RejectReason::NonNegativeFailureStatus
            }
        );
        assert_eq!(
            inspect(ReturnedBytes {
                bytes: &bytes,
                status: CompletionStatus::Success,
                actual_byte_count: None,
            }),
            Inspection::Rejected {
                reason: RejectReason::MissingActualByteCount
            }
        );
        assert_eq!(
            inspect(success(&bytes[..17], DESCRIPTOR_LENGTH)),
            Inspection::Rejected {
                reason: RejectReason::ActualByteCountDoesNotMatchSlice
            }
        );
        assert_eq!(
            inspect(success(&bytes, 17)),
            Inspection::Rejected {
                reason: RejectReason::ActualByteCountDoesNotMatchSlice
            }
        );
        assert_eq!(
            inspect(success(&bytes[..17], 17)),
            Inspection::Rejected {
                reason: RejectReason::IncompleteDescriptor
            }
        );
    }

    #[test]
    fn session_keeps_other_profiles_and_physical_routes_unpromoted() {
        let bytes = descriptor(pod_hd500x::USB_VENDOR_ID, pod_hd500x::USB_PRODUCT_ID);
        let returned = success(&bytes, bytes.len());
        assert_eq!(
            Session::extension(ProductProfile::Jtv69)
                .unwrap()
                .inspect_hd500x_windows_descriptor(returned),
            Err(Rejection::Unverified {
                profile: ProductProfile::Jtv69,
                operation: Operation::InspectOfflineDeviceDescriptor,
            })
        );
        assert_eq!(
            Session::initial_rig().inspect_hd500x_windows_descriptor(returned),
            Err(Rejection::Unsupported {
                profile: ProductProfile::Variax600,
                operation: Operation::InspectOfflineDeviceDescriptor,
            })
        );
        let hd = Session::extension(ProductProfile::PodHd500x).unwrap();
        assert!(matches!(
            hd.select_record_route(crate::pod_audio::RecordRoute::SendProcessedGuitar, 66, 1),
            Err(Rejection::Unverified {
                profile: ProductProfile::PodHd500x,
                operation: Operation::SelectRecordRoute
            })
        ));
        assert_eq!(
            hd.reject_extension_physical(Operation::OpenAudioStream, None, || panic!(
                "must stay locked"
            )),
            Err(Rejection::Locked {
                operation: Operation::OpenAudioStream
            })
        );
    }
}
