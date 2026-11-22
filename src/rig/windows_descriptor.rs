#![forbid(unsafe_code)]
//! Safe offline inspection of a complete POD HD500X Windows-driver descriptor result.
//!
//! This is not a Windows request builder or a transport adapter. Inputs are
//! caller-owned bytes and completion facts from a synthetic/offline source.

use crate::pod_hd500x;

pub const DESCRIPTOR_LENGTH: usize = 18;
pub const USB_DEVICE_DESCRIPTOR_TYPE: u8 = 1;
pub const OBSERVED_DRIVER_PRODUCT_ID: u16 = 0x4159;
pub const CONFIGURATION_DESCRIPTOR_TYPE: u8 = 2;
pub const INTERFACE_DESCRIPTOR_TYPE: u8 = 4;
pub const ENDPOINT_DESCRIPTOR_TYPE: u8 = 5;
/// The supplied Windows driver requests at most 0x400 bytes for a
/// configuration descriptor. This bounds offline parsing too.
pub const MAX_CONFIGURATION_SNAPSHOT_BYTES: usize = 0x400;

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
    InvalidConfigurationCount,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConfigurationSelection {
    /// Caller-supplied descriptor index, bounded by the validated device
    /// descriptor's bNumConfigurations. This is not bConfigurationValue.
    pub configuration_index: u8,
    /// The source-admitted Windows path selects configuration VALUE 1.
    pub configuration_value: u8,
    pub interface_number: u8,
    pub alternate_setting: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotStage {
    DeviceDescriptor,
    ConfigurationDescriptor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotRejectReason {
    DeviceDescriptor(RejectReason),
    WrongHd500xIdentity {
        vendor_id: u16,
        product_id: u16,
    },
    MissingActualByteCount,
    ActualByteCountDoesNotMatchSlice,
    NonNegativeFailureStatus,
    ConfigurationSnapshotEmpty,
    ConfigurationSnapshotTooLarge,
    ConfigurationHeaderTruncated,
    InvalidConfigurationHeaderLength,
    WrongConfigurationDescriptorType,
    ConfigurationIndexOutOfRange {
        index: u8,
        count: u8,
    },
    UnsupportedConfigurationValue {
        requested: u8,
        actual: u8,
    },
    ConfigurationValueMismatch {
        requested: u8,
        actual: u8,
    },
    InvalidTotalLength,
    TrailingBytesAfterTotalLength,
    DescriptorHeaderTruncated {
        offset: usize,
    },
    InvalidNestedDescriptorLength {
        offset: usize,
        length: u8,
    },
    KnownDescriptorTooShort {
        offset: usize,
        descriptor_type: u8,
        length: u8,
    },
    NestedConfigurationDescriptor {
        offset: usize,
    },
    EndpointWithoutInterface {
        offset: usize,
    },
    DuplicateInterfaceAlternate {
        interface_number: u8,
        alternate_setting: u8,
    },
    DeclaredInterfaceCountMismatch {
        declared: u8,
        actual: usize,
    },
    EndpointCountMismatch {
        interface_number: u8,
        alternate_setting: u8,
        declared: u8,
        actual: usize,
    },
    InvalidEndpointAddress {
        offset: usize,
        address: u8,
    },
    InvalidEndpointAttributes {
        offset: usize,
        attributes: u8,
    },
    DuplicateSelectedEndpointAddress {
        address: u8,
    },
    SelectedInterfaceAlternateMissing {
        interface_number: u8,
        alternate_setting: u8,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotFailure {
    DeviceDescriptor { status: i32 },
    ConfigurationDescriptor { status: i32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferType {
    Control,
    Isochronous,
    Bulk,
    Interrupt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Out,
    In,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EndpointMetadata {
    address: u8,
    direction: Direction,
    transfer_type: TransferType,
    maximum_packet_size: u16,
    interval: u8,
    raw_descriptor: Vec<u8>,
}

impl EndpointMetadata {
    pub const fn address(&self) -> u8 {
        self.address
    }
    pub const fn direction(&self) -> Direction {
        self.direction
    }
    pub const fn transfer_type(&self) -> TransferType {
        self.transfer_type
    }
    pub const fn maximum_packet_size(&self) -> u16 {
        self.maximum_packet_size
    }
    pub const fn interval(&self) -> u8 {
        self.interval
    }
    pub fn raw_descriptor(&self) -> &[u8] {
        &self.raw_descriptor
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterfaceAlternateMetadata {
    interface_number: u8,
    alternate_setting: u8,
    declared_endpoint_count: u8,
    endpoints: Vec<EndpointMetadata>,
    raw_descriptor: Vec<u8>,
}

impl InterfaceAlternateMetadata {
    pub const fn interface_number(&self) -> u8 {
        self.interface_number
    }
    pub const fn alternate_setting(&self) -> u8 {
        self.alternate_setting
    }
    pub const fn declared_endpoint_count(&self) -> u8 {
        self.declared_endpoint_count
    }
    pub fn endpoints(&self) -> &[EndpointMetadata] {
        &self.endpoints
    }
    pub fn raw_descriptor(&self) -> &[u8] {
        &self.raw_descriptor
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpaqueDescriptor {
    offset: usize,
    descriptor_type: u8,
    raw_descriptor: Vec<u8>,
}

impl OpaqueDescriptor {
    pub const fn offset(&self) -> usize {
        self.offset
    }
    pub const fn descriptor_type(&self) -> u8 {
        self.descriptor_type
    }
    pub fn raw_descriptor(&self) -> &[u8] {
        &self.raw_descriptor
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteStatus {
    UnverifiedAndLocked,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigurationReport {
    /// Explicit caller-supplied index, not the value in the descriptor.
    configuration_index: u8,
    configuration_value: u8,
    declared_interface_count: u8,
    selected_interface_number: u8,
    selected_alternate_setting: u8,
    interface_alternates: Vec<InterfaceAlternateMetadata>,
    selected_alternate_index: usize,
    /// Exact source bytes through wTotalLength. Trailing bytes are rejected.
    raw_configuration: Vec<u8>,
    /// Unknown and class-specific records, byte-for-byte, in source order.
    opaque_descriptors: Vec<OpaqueDescriptor>,
    route_status: RouteStatus,
}

impl ConfigurationReport {
    pub const fn configuration_index(&self) -> u8 {
        self.configuration_index
    }
    pub const fn configuration_value(&self) -> u8 {
        self.configuration_value
    }
    pub const fn declared_interface_count(&self) -> u8 {
        self.declared_interface_count
    }
    pub const fn selected_interface_number(&self) -> u8 {
        self.selected_interface_number
    }
    pub const fn selected_alternate_setting(&self) -> u8 {
        self.selected_alternate_setting
    }
    pub fn interface_alternates(&self) -> &[InterfaceAlternateMetadata] {
        &self.interface_alternates
    }
    pub fn raw_configuration(&self) -> &[u8] {
        &self.raw_configuration
    }
    pub fn opaque_descriptors(&self) -> &[OpaqueDescriptor] {
        &self.opaque_descriptors
    }
    pub const fn route_status(&self) -> RouteStatus {
        self.route_status
    }

    pub fn selected_alternate(&self) -> &InterfaceAlternateMetadata {
        &self.interface_alternates[self.selected_alternate_index]
    }

    /// A structurally valid zero-endpoint alternate is metadata only. No
    /// alternate, including a nonempty one, becomes a usable route here.
    pub const fn hardware_status(&self) -> HardwareStatus {
        HardwareStatus::UnverifiedAndLocked
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigurationInspection {
    Pending { stage: SnapshotStage },
    Failed(SnapshotFailure),
    Rejected { reason: SnapshotRejectReason },
    Complete(ConfigurationReport),
}

/// Inspects already-returned offline device and configuration descriptor
/// bytes. No request is built or sent. The device identity and descriptor
/// count are revalidated from the supplied device bytes before selection.
pub fn inspect_configuration_snapshot(
    device: ReturnedBytes<'_>,
    configuration: ReturnedBytes<'_>,
    selection: ConfigurationSelection,
) -> ConfigurationInspection {
    let device_report = match inspect(device) {
        Inspection::Pending => {
            return ConfigurationInspection::Pending {
                stage: SnapshotStage::DeviceDescriptor,
            };
        }
        Inspection::Failed { status } => {
            return ConfigurationInspection::Failed(SnapshotFailure::DeviceDescriptor { status });
        }
        Inspection::Rejected { reason } => {
            return ConfigurationInspection::Rejected {
                reason: SnapshotRejectReason::DeviceDescriptor(reason),
            };
        }
        Inspection::Complete(report) => report,
    };
    if device_report.hd500x_preflight != Hd500xPreflight::IdentityMatchesInf {
        return ConfigurationInspection::Rejected {
            reason: SnapshotRejectReason::WrongHd500xIdentity {
                vendor_id: device_report.descriptor.vendor_id,
                product_id: device_report.descriptor.product_id,
            },
        };
    }
    let configuration_count = device_report.descriptor.raw[17];
    if selection.configuration_index >= configuration_count {
        return ConfigurationInspection::Rejected {
            reason: SnapshotRejectReason::ConfigurationIndexOutOfRange {
                index: selection.configuration_index,
                count: configuration_count,
            },
        };
    }
    if selection.configuration_value != 1 {
        return ConfigurationInspection::Rejected {
            reason: SnapshotRejectReason::UnsupportedConfigurationValue {
                requested: selection.configuration_value,
                actual: 1,
            },
        };
    }

    match configuration.status {
        CompletionStatus::Pending => {
            return ConfigurationInspection::Pending {
                stage: SnapshotStage::ConfigurationDescriptor,
            };
        }
        CompletionStatus::Failed(status) if status < 0 => {
            return ConfigurationInspection::Failed(SnapshotFailure::ConfigurationDescriptor {
                status,
            });
        }
        CompletionStatus::Failed(_) => {
            return ConfigurationInspection::Rejected {
                reason: SnapshotRejectReason::NonNegativeFailureStatus,
            };
        }
        CompletionStatus::Success => {}
    };
    let Some(actual_byte_count) = configuration.actual_byte_count else {
        return ConfigurationInspection::Rejected {
            reason: SnapshotRejectReason::MissingActualByteCount,
        };
    };
    if actual_byte_count != configuration.bytes.len() {
        return ConfigurationInspection::Rejected {
            reason: SnapshotRejectReason::ActualByteCountDoesNotMatchSlice,
        };
    }
    if actual_byte_count == 0 {
        return ConfigurationInspection::Rejected {
            reason: SnapshotRejectReason::ConfigurationSnapshotEmpty,
        };
    }
    if actual_byte_count > MAX_CONFIGURATION_SNAPSHOT_BYTES {
        return ConfigurationInspection::Rejected {
            reason: SnapshotRejectReason::ConfigurationSnapshotTooLarge,
        };
    }
    parse_configuration(configuration.bytes, selection)
}

fn parse_configuration(bytes: &[u8], selection: ConfigurationSelection) -> ConfigurationInspection {
    if bytes.len() < 2 {
        return ConfigurationInspection::Rejected {
            reason: SnapshotRejectReason::ConfigurationHeaderTruncated,
        };
    }
    let header_length = bytes[0];
    if usize::from(header_length) < 9 || usize::from(header_length) > bytes.len() {
        return ConfigurationInspection::Rejected {
            reason: SnapshotRejectReason::InvalidConfigurationHeaderLength,
        };
    }
    if bytes[1] != CONFIGURATION_DESCRIPTOR_TYPE {
        return ConfigurationInspection::Rejected {
            reason: SnapshotRejectReason::WrongConfigurationDescriptorType,
        };
    }
    let total_length = usize::from(u16::from_le_bytes([bytes[2], bytes[3]]));
    if total_length < usize::from(header_length) || total_length > bytes.len() {
        return ConfigurationInspection::Rejected {
            reason: SnapshotRejectReason::InvalidTotalLength,
        };
    }
    if total_length != bytes.len() {
        return ConfigurationInspection::Rejected {
            reason: SnapshotRejectReason::TrailingBytesAfterTotalLength,
        };
    }
    let configuration_value = bytes[5];
    if configuration_value != selection.configuration_value {
        return ConfigurationInspection::Rejected {
            reason: SnapshotRejectReason::ConfigurationValueMismatch {
                requested: selection.configuration_value,
                actual: configuration_value,
            },
        };
    }

    let declared_interface_count = bytes[4];
    let mut interface_alternates = Vec::<InterfaceAlternateMetadata>::new();
    let mut opaque_descriptors = Vec::<OpaqueDescriptor>::new();
    let mut offset = usize::from(header_length);
    while offset < total_length {
        if total_length - offset < 2 {
            return ConfigurationInspection::Rejected {
                reason: SnapshotRejectReason::DescriptorHeaderTruncated { offset },
            };
        }
        let length = bytes[offset];
        let descriptor_type = bytes[offset + 1];
        let length_usize = usize::from(length);
        if length_usize < 2 || length_usize > total_length - offset {
            return ConfigurationInspection::Rejected {
                reason: SnapshotRejectReason::InvalidNestedDescriptorLength { offset, length },
            };
        }
        let record = &bytes[offset..offset + length_usize];
        match descriptor_type {
            CONFIGURATION_DESCRIPTOR_TYPE => {
                return ConfigurationInspection::Rejected {
                    reason: SnapshotRejectReason::NestedConfigurationDescriptor { offset },
                };
            }
            INTERFACE_DESCRIPTOR_TYPE => {
                if length_usize < 9 {
                    return ConfigurationInspection::Rejected {
                        reason: SnapshotRejectReason::KnownDescriptorTooShort {
                            offset,
                            descriptor_type,
                            length,
                        },
                    };
                }
                let interface_number = record[2];
                let alternate_setting = record[3];
                if interface_alternates.iter().any(|interface| {
                    interface.interface_number == interface_number
                        && interface.alternate_setting == alternate_setting
                }) {
                    return ConfigurationInspection::Rejected {
                        reason: SnapshotRejectReason::DuplicateInterfaceAlternate {
                            interface_number,
                            alternate_setting,
                        },
                    };
                }
                interface_alternates.push(InterfaceAlternateMetadata {
                    interface_number,
                    alternate_setting,
                    declared_endpoint_count: record[4],
                    endpoints: Vec::new(),
                    raw_descriptor: record.to_vec(),
                });
            }
            ENDPOINT_DESCRIPTOR_TYPE => {
                if length_usize < 7 {
                    return ConfigurationInspection::Rejected {
                        reason: SnapshotRejectReason::KnownDescriptorTooShort {
                            offset,
                            descriptor_type,
                            length,
                        },
                    };
                }
                let Some(interface) = interface_alternates.last_mut() else {
                    return ConfigurationInspection::Rejected {
                        reason: SnapshotRejectReason::EndpointWithoutInterface { offset },
                    };
                };
                let address = record[2];
                if address & 0x70 != 0 || address & 0x0f == 0 {
                    return ConfigurationInspection::Rejected {
                        reason: SnapshotRejectReason::InvalidEndpointAddress { offset, address },
                    };
                }
                let attributes = record[3];
                if attributes & 0xc0 != 0 {
                    return ConfigurationInspection::Rejected {
                        reason: SnapshotRejectReason::InvalidEndpointAttributes {
                            offset,
                            attributes,
                        },
                    };
                }
                let direction = if address & 0x80 == 0 {
                    Direction::Out
                } else {
                    Direction::In
                };
                let transfer_type = match attributes & 0x03 {
                    0 => TransferType::Control,
                    1 => TransferType::Isochronous,
                    2 => TransferType::Bulk,
                    _ => TransferType::Interrupt,
                };
                interface.endpoints.push(EndpointMetadata {
                    address,
                    direction,
                    transfer_type,
                    maximum_packet_size: u16::from_le_bytes([record[4], record[5]]),
                    interval: record[6],
                    raw_descriptor: record.to_vec(),
                });
            }
            _ => opaque_descriptors.push(OpaqueDescriptor {
                offset,
                descriptor_type,
                raw_descriptor: record.to_vec(),
            }),
        }
        offset += length_usize;
    }

    let distinct_interface_count = interface_alternates
        .iter()
        .map(|interface| interface.interface_number)
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    if distinct_interface_count != usize::from(declared_interface_count) {
        return ConfigurationInspection::Rejected {
            reason: SnapshotRejectReason::DeclaredInterfaceCountMismatch {
                declared: declared_interface_count,
                actual: distinct_interface_count,
            },
        };
    }
    for interface in &interface_alternates {
        if interface.endpoints.len() != usize::from(interface.declared_endpoint_count) {
            return ConfigurationInspection::Rejected {
                reason: SnapshotRejectReason::EndpointCountMismatch {
                    interface_number: interface.interface_number,
                    alternate_setting: interface.alternate_setting,
                    declared: interface.declared_endpoint_count,
                    actual: interface.endpoints.len(),
                },
            };
        }
    }
    let selected_matches = interface_alternates
        .iter()
        .enumerate()
        .filter(|(_, interface)| {
            interface.interface_number == selection.interface_number
                && interface.alternate_setting == selection.alternate_setting
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let [selected_alternate_index] = selected_matches.as_slice() else {
        return ConfigurationInspection::Rejected {
            reason: SnapshotRejectReason::SelectedInterfaceAlternateMissing {
                interface_number: selection.interface_number,
                alternate_setting: selection.alternate_setting,
            },
        };
    };
    let selected = &interface_alternates[*selected_alternate_index];
    for (index, endpoint) in selected.endpoints.iter().enumerate() {
        if selected.endpoints[..index]
            .iter()
            .any(|prior| prior.address == endpoint.address)
        {
            return ConfigurationInspection::Rejected {
                reason: SnapshotRejectReason::DuplicateSelectedEndpointAddress {
                    address: endpoint.address,
                },
            };
        }
    }
    ConfigurationInspection::Complete(ConfigurationReport {
        configuration_index: selection.configuration_index,
        configuration_value,
        declared_interface_count,
        selected_interface_number: selection.interface_number,
        selected_alternate_setting: selection.alternate_setting,
        interface_alternates,
        selected_alternate_index: *selected_alternate_index,
        raw_configuration: bytes.to_vec(),
        opaque_descriptors,
        route_status: RouteStatus::UnverifiedAndLocked,
    })
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
    if raw[17] == 0 {
        return Inspection::Rejected {
            reason: RejectReason::InvalidConfigurationCount,
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
        bytes[17] = 1;
        bytes
    }

    fn success(bytes: &[u8], actual_byte_count: usize) -> ReturnedBytes<'_> {
        ReturnedBytes {
            bytes,
            status: CompletionStatus::Success,
            actual_byte_count: Some(actual_byte_count),
        }
    }

    fn config_descriptor(records: &[&[u8]], interfaces: u8, value: u8) -> Vec<u8> {
        let total_length = 9 + records.iter().map(|record| record.len()).sum::<usize>();
        let mut bytes = vec![9, CONFIGURATION_DESCRIPTOR_TYPE];
        bytes.extend_from_slice(&(total_length as u16).to_le_bytes());
        bytes.extend_from_slice(&[interfaces, value, 0, 0x80, 50]);
        for record in records {
            bytes.extend_from_slice(record);
        }
        bytes
    }

    fn interface(number: u8, alternate: u8, endpoint_count: u8) -> [u8; 9] {
        [
            9,
            INTERFACE_DESCRIPTOR_TYPE,
            number,
            alternate,
            endpoint_count,
            0xff,
            0,
            0,
            0,
        ]
    }

    fn endpoint(address: u8, attributes: u8) -> [u8; 7] {
        [7, ENDPOINT_DESCRIPTOR_TYPE, address, attributes, 64, 0, 1]
    }

    fn selection(index: u8, value: u8, interface: u8, alternate: u8) -> ConfigurationSelection {
        ConfigurationSelection {
            configuration_index: index,
            configuration_value: value,
            interface_number: interface,
            alternate_setting: alternate,
        }
    }

    fn inspect_config(bytes: &[u8], selection: ConfigurationSelection) -> ConfigurationInspection {
        let device = descriptor(pod_hd500x::USB_VENDOR_ID, pod_hd500x::USB_PRODUCT_ID);
        inspect_configuration_snapshot(
            success(&device, device.len()),
            success(bytes, bytes.len()),
            selection,
        )
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

    #[test]
    fn parses_explicit_index_value_interface_and_alternate_without_promoting_a_route() {
        let interface_zero_alt_zero = interface(0, 0, 1);
        let endpoint_in = endpoint(0x81, 3);
        let opaque = [3, 0x24, 0x99];
        let interface_zero_alt_one = interface(0, 1, 1);
        let endpoint_in_again = endpoint(0x81, 3);
        let interface_one_empty = interface(1, 0, 0);
        let bytes = config_descriptor(
            &[
                &interface_zero_alt_zero,
                &endpoint_in,
                &opaque,
                &interface_zero_alt_one,
                &endpoint_in_again,
                &interface_one_empty,
            ],
            2,
            1,
        );
        let mut device = descriptor(pod_hd500x::USB_VENDOR_ID, pod_hd500x::USB_PRODUCT_ID);
        device[17] = 2;

        let ConfigurationInspection::Complete(report) = inspect_configuration_snapshot(
            success(&device, device.len()),
            success(&bytes, bytes.len()),
            selection(0, 1, 0, 1),
        ) else {
            panic!("bounded synthetic configuration must parse")
        };
        assert_eq!(report.configuration_index(), 0);
        assert_eq!(report.configuration_value(), 1);
        assert_eq!(report.raw_configuration(), bytes);
        assert_eq!(report.selected_interface_number(), 0);
        assert_eq!(report.selected_alternate_setting(), 1);
        assert_eq!(report.interface_alternates().len(), 3);
        assert_eq!(report.selected_alternate().endpoints().len(), 1);
        assert_eq!(report.selected_alternate().endpoints()[0].address(), 0x81);
        assert_eq!(
            report.selected_alternate().endpoints()[0].direction(),
            Direction::In
        );
        assert_eq!(
            report.selected_alternate().endpoints()[0].transfer_type(),
            TransferType::Interrupt
        );
        assert_eq!(
            report.selected_alternate().endpoints()[0].raw_descriptor(),
            endpoint_in_again
        );
        assert_eq!(report.opaque_descriptors().len(), 1);
        assert_eq!(report.opaque_descriptors()[0].raw_descriptor(), opaque);
        assert_eq!(report.interface_alternates()[2].endpoints().len(), 0);
        assert_eq!(
            report.hardware_status(),
            HardwareStatus::UnverifiedAndLocked
        );
        assert_eq!(report.route_status(), RouteStatus::UnverifiedAndLocked);
    }

    #[test]
    fn configuration_index_value_count_and_identity_are_independent_checks() {
        let empty_interface = interface(0, 0, 0);
        let bytes = config_descriptor(&[&empty_interface], 1, 1);
        let mut device = descriptor(pod_hd500x::USB_VENDOR_ID, pod_hd500x::USB_PRODUCT_ID);
        device[17] = 2;

        assert!(matches!(
            inspect_configuration_snapshot(
                success(&device, device.len()),
                success(&bytes, bytes.len()),
                selection(1, 1, 0, 0),
            ),
            ConfigurationInspection::Complete(_)
        ));
        assert_eq!(
            inspect_configuration_snapshot(
                success(&device, device.len()),
                success(&bytes, bytes.len()),
                selection(2, 1, 0, 0),
            ),
            ConfigurationInspection::Rejected {
                reason: SnapshotRejectReason::ConfigurationIndexOutOfRange { index: 2, count: 2 }
            }
        );
        assert_eq!(
            inspect_configuration_snapshot(
                success(&device, device.len()),
                success(&bytes, bytes.len()),
                selection(0, 2, 0, 0),
            ),
            ConfigurationInspection::Rejected {
                reason: SnapshotRejectReason::UnsupportedConfigurationValue {
                    requested: 2,
                    actual: 1
                }
            }
        );

        let wrong_value = config_descriptor(&[&empty_interface], 1, 2);
        assert_eq!(
            inspect_configuration_snapshot(
                success(&device, device.len()),
                success(&wrong_value, wrong_value.len()),
                selection(0, 1, 0, 0),
            ),
            ConfigurationInspection::Rejected {
                reason: SnapshotRejectReason::ConfigurationValueMismatch {
                    requested: 1,
                    actual: 2
                }
            }
        );

        let wrong_identity = descriptor(0x1234, pod_hd500x::USB_PRODUCT_ID);
        assert!(matches!(
            inspect_configuration_snapshot(
                success(&wrong_identity, wrong_identity.len()),
                success(&bytes, bytes.len()),
                selection(0, 1, 0, 0),
            ),
            ConfigurationInspection::Rejected {
                reason: SnapshotRejectReason::WrongHd500xIdentity { .. }
            }
        ));
    }

    #[test]
    fn rejects_inconsistent_completion_lengths_and_configuration_bounds() {
        let empty_interface = interface(0, 0, 0);
        let bytes = config_descriptor(&[&empty_interface], 1, 1);
        let device = descriptor(pod_hd500x::USB_VENDOR_ID, pod_hd500x::USB_PRODUCT_ID);
        let selected = selection(0, 1, 0, 0);
        assert_eq!(
            inspect_configuration_snapshot(
                success(&device, device.len()),
                ReturnedBytes {
                    bytes: &bytes,
                    status: CompletionStatus::Success,
                    actual_byte_count: None
                },
                selected,
            ),
            ConfigurationInspection::Rejected {
                reason: SnapshotRejectReason::MissingActualByteCount
            }
        );
        assert_eq!(
            inspect_configuration_snapshot(
                success(&device, device.len()),
                success(&bytes, bytes.len() - 1),
                selected,
            ),
            ConfigurationInspection::Rejected {
                reason: SnapshotRejectReason::ActualByteCountDoesNotMatchSlice
            }
        );
        assert_eq!(
            inspect_configuration_snapshot(
                success(&device, device.len()),
                ReturnedBytes {
                    bytes: &bytes,
                    status: CompletionStatus::Pending,
                    actual_byte_count: None
                },
                selected,
            ),
            ConfigurationInspection::Pending {
                stage: SnapshotStage::ConfigurationDescriptor
            }
        );
        assert_eq!(
            inspect_configuration_snapshot(
                success(&device, device.len()),
                ReturnedBytes {
                    bytes: &bytes,
                    status: CompletionStatus::Failed(-5),
                    actual_byte_count: None
                },
                selected,
            ),
            ConfigurationInspection::Failed(SnapshotFailure::ConfigurationDescriptor {
                status: -5
            })
        );

        let mut too_large = vec![0; MAX_CONFIGURATION_SNAPSHOT_BYTES + 1];
        too_large[0] = 9;
        too_large[1] = CONFIGURATION_DESCRIPTOR_TYPE;
        let too_large_length = too_large.len() as u16;
        too_large[2..4].copy_from_slice(&too_large_length.to_le_bytes());
        assert_eq!(
            inspect_configuration_snapshot(
                success(&device, device.len()),
                success(&too_large, too_large.len()),
                selected,
            ),
            ConfigurationInspection::Rejected {
                reason: SnapshotRejectReason::ConfigurationSnapshotTooLarge
            }
        );
    }

    #[test]
    fn malformed_descriptor_sequences_are_rejected_without_panics_or_input_mutation() {
        let empty_interface = interface(0, 0, 0);
        let valid = config_descriptor(&[&empty_interface], 1, 1);
        let original = valid.clone();
        for cut in 0..valid.len() {
            let truncated = &valid[..cut];
            assert!(
                matches!(
                    inspect_config(truncated, selection(0, 1, 0, 0)),
                    ConfigurationInspection::Rejected { .. }
                ),
                "cut at {cut}"
            );
        }
        assert_eq!(valid, original);

        let mut malformed = valid.clone();
        malformed[0] = 8;
        assert!(matches!(
            inspect_config(&malformed, selection(0, 1, 0, 0)),
            ConfigurationInspection::Rejected {
                reason: SnapshotRejectReason::InvalidConfigurationHeaderLength
            }
        ));
        let mut wrong_type = valid.clone();
        wrong_type[1] = 3;
        assert!(matches!(
            inspect_config(&wrong_type, selection(0, 1, 0, 0)),
            ConfigurationInspection::Rejected {
                reason: SnapshotRejectReason::WrongConfigurationDescriptorType
            }
        ));
        let mut overrun = valid.clone();
        overrun[2..4].copy_from_slice(&u16::MAX.to_le_bytes());
        assert!(matches!(
            inspect_config(&overrun, selection(0, 1, 0, 0)),
            ConfigurationInspection::Rejected {
                reason: SnapshotRejectReason::InvalidTotalLength
            }
        ));
        let mut trailing = valid.clone();
        trailing.push(0);
        assert!(matches!(
            inspect_config(&trailing, selection(0, 1, 0, 0)),
            ConfigurationInspection::Rejected {
                reason: SnapshotRejectReason::TrailingBytesAfterTotalLength
            }
        ));
        let mut zero_nested_length = valid.clone();
        zero_nested_length[9] = 0;
        assert!(matches!(
            inspect_config(&zero_nested_length, selection(0, 1, 0, 0)),
            ConfigurationInspection::Rejected {
                reason: SnapshotRejectReason::InvalidNestedDescriptorLength { .. }
            }
        ));
        let mut short_interface = valid.clone();
        short_interface[9] = 8;
        assert!(matches!(
            inspect_config(&short_interface, selection(0, 1, 0, 0)),
            ConfigurationInspection::Rejected {
                reason: SnapshotRejectReason::KnownDescriptorTooShort {
                    descriptor_type: INTERFACE_DESCRIPTOR_TYPE,
                    ..
                }
            }
        ));

        let endpoint_without_interface = config_descriptor(&[&endpoint(0x81, 2)], 0, 1);
        assert!(matches!(
            inspect_config(&endpoint_without_interface, selection(0, 1, 0, 0)),
            ConfigurationInspection::Rejected {
                reason: SnapshotRejectReason::EndpointWithoutInterface { .. }
            }
        ));
    }

    #[test]
    fn validates_interface_alternates_and_selected_endpoint_addresses() {
        let duplicate_interface = interface(0, 0, 0);
        let duplicate_interface_again = interface(0, 0, 0);
        let duplicates =
            config_descriptor(&[&duplicate_interface, &duplicate_interface_again], 1, 1);
        assert!(matches!(
            inspect_config(&duplicates, selection(0, 1, 0, 0)),
            ConfigurationInspection::Rejected {
                reason: SnapshotRejectReason::DuplicateInterfaceAlternate { .. }
            }
        ));

        let endpoint_a = endpoint(0x81, 2);
        let endpoint_b = endpoint(0x81, 2);
        let interface_two_eps = interface(0, 0, 2);
        let duplicate_endpoints =
            config_descriptor(&[&interface_two_eps, &endpoint_a, &endpoint_b], 1, 1);
        assert_eq!(
            inspect_config(&duplicate_endpoints, selection(0, 1, 0, 0)),
            ConfigurationInspection::Rejected {
                reason: SnapshotRejectReason::DuplicateSelectedEndpointAddress { address: 0x81 }
            }
        );

        let endpoint_out = endpoint(0x02, 2);
        let interface_wrong_count = interface(0, 0, 0);
        let wrong_count = config_descriptor(&[&interface_wrong_count, &endpoint_out], 1, 1);
        assert!(matches!(
            inspect_config(&wrong_count, selection(0, 1, 0, 0)),
            ConfigurationInspection::Rejected {
                reason: SnapshotRejectReason::EndpointCountMismatch { .. }
            }
        ));

        let interface_empty = interface(0, 0, 0);
        let wrong_interface_count = config_descriptor(&[&interface_empty], 2, 1);
        assert!(matches!(
            inspect_config(&wrong_interface_count, selection(0, 1, 0, 0)),
            ConfigurationInspection::Rejected {
                reason: SnapshotRejectReason::DeclaredInterfaceCountMismatch { .. }
            }
        ));

        let interface_with_endpoint = interface(0, 0, 1);
        let endpoint_zero = endpoint(0x80, 2);
        let invalid_address = config_descriptor(&[&interface_with_endpoint, &endpoint_zero], 1, 1);
        assert!(matches!(
            inspect_config(&invalid_address, selection(0, 1, 0, 0)),
            ConfigurationInspection::Rejected {
                reason: SnapshotRejectReason::InvalidEndpointAddress { .. }
            }
        ));

        let endpoint_reserved_attributes = endpoint(0x81, 0x82);
        let invalid_attributes = config_descriptor(
            &[&interface_with_endpoint, &endpoint_reserved_attributes],
            1,
            1,
        );
        assert!(matches!(
            inspect_config(&invalid_attributes, selection(0, 1, 0, 0)),
            ConfigurationInspection::Rejected {
                reason: SnapshotRejectReason::InvalidEndpointAttributes { .. }
            }
        ));

        let absent_interface = interface(0, 0, 0);
        let absent = config_descriptor(&[&absent_interface], 1, 1);
        assert!(matches!(
            inspect_config(&absent, selection(0, 1, 1, 0)),
            ConfigurationInspection::Rejected {
                reason: SnapshotRejectReason::SelectedInterfaceAlternateMissing { .. }
            }
        ));
    }

    #[test]
    fn session_entry_is_profile_gated_and_never_changes_hardware_locks() {
        let device = descriptor(pod_hd500x::USB_VENDOR_ID, pod_hd500x::USB_PRODUCT_ID);
        let interface = interface(0, 0, 0);
        let configuration = config_descriptor(&[&interface], 1, 1);
        let device_result = success(&device, device.len());
        let configuration_result = success(&configuration, configuration.len());
        let selection = selection(0, 1, 0, 0);

        let hd = Session::extension(ProductProfile::PodHd500x).unwrap();
        let ConfigurationInspection::Complete(report) = hd
            .inspect_hd500x_windows_configuration_snapshot(
                device_result,
                configuration_result,
                selection,
            )
            .unwrap()
        else {
            panic!("synthetic configuration result should be inspectable")
        };
        assert_eq!(report.route_status(), RouteStatus::UnverifiedAndLocked);
        assert_eq!(
            hd.reject_extension_physical(Operation::OpenAudioStream, None, || panic!("locked")),
            Err(Rejection::Locked {
                operation: Operation::OpenAudioStream
            })
        );
        assert_eq!(
            Session::initial_rig().inspect_hd500x_windows_configuration_snapshot(
                device_result,
                configuration_result,
                selection,
            ),
            Err(Rejection::Unsupported {
                profile: ProductProfile::Variax600,
                operation: Operation::InspectOfflineDeviceDescriptor
            })
        );
        assert_eq!(
            Session::extension(ProductProfile::Jtv69)
                .unwrap()
                .inspect_hd500x_windows_configuration_snapshot(
                    device_result,
                    configuration_result,
                    selection,
                ),
            Err(Rejection::Unverified {
                profile: ProductProfile::Jtv69,
                operation: Operation::InspectOfflineDeviceDescriptor
            })
        );
        assert_eq!(
            crate::rig::capability::capability(
                ProductProfile::PodHd500x,
                Operation::OpenAudioStream
            )
            .state,
            CapabilityState::Locked
        );
    }
}
