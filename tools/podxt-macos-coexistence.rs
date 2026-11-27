//! Offline model of the #298 diagnostic. Compile with `rustc --test`.
//! All transport implementations in this file are fakes.
#![cfg(test)]
#![forbid(unsafe_code)]

use std::time::Duration;

const BOTH_INTERFACES: u8 = 0b11;
const OBSERVATIONS: usize = 6;
const OBSERVATION_DELAY: Duration = Duration::from_millis(200);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Attachment {
    registry_id: u64,
    location_id: u32,
    address: u8,
    vendor: u16,
    product: u16,
    configurations: u8,
}

#[derive(Clone, Copy)]
struct Snapshot {
    attachment: Attachment,
    registered_client: bool,
    conflicting_binding: bool,
    interfaces: u8,
}

#[derive(Clone, Copy)]
enum Approval {
    ReadOnly,
    ActivateOnce,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Stop {
    InvalidSelection,
    AttachmentChanged,
    PublishedConflict,
    InvalidInterfaceView,
    AdvertisedDescriptor,
    OpenBusy(u32),
    OpenRefused(u32),
    ConfigurationRead,
    InconsistentConfiguration,
    ActivationRefused(u32),
    PublicationIncomplete,
    ActiveDescriptorUnavailable,
}

#[derive(Debug, Eq, PartialEq)]
struct Failure {
    stopped: Option<Stop>,
    close_error: Option<u32>,
}

impl From<Stop> for Failure {
    fn from(stop: Stop) -> Self {
        Self {
            stopped: Some(stop),
            close_error: None,
        }
    }
}

#[derive(Default)]
struct Report {
    registered_client_seen: bool,
    fresh_configuration: Option<u8>,
    activation_accepted: bool,
    interfaces_after_close: Option<u8>,
    active_descriptor_verified: bool,
}

trait Transport {
    type Handle: DeviceHandle;

    fn snapshot(&mut self) -> Result<Snapshot, Stop>;
    fn advertised_descriptor(&mut self) -> &[u8];
    fn open_nonseizing(&mut self) -> Result<Self::Handle, Stop>;
    fn refreshed_active_descriptor(&mut self) -> Result<&[u8], Stop>;
}

struct ConfigurationReply {
    value: u8,
    bytes_read: usize,
}

trait DeviceHandle {
    fn get_configuration(&mut self) -> Result<ConfigurationReply, Stop>;
    fn activate_one_with_interface_matching(&mut self) -> Result<(), Stop>;
    fn close(self) -> Result<(), u32>;
}

fn run<T: Transport, L>(
    transport: &mut T,
    expected: Attachment,
    approval: Approval,
    lock: impl FnOnce() -> Result<L, Stop>,
    mut wait: impl FnMut(Duration),
) -> (Report, Result<(), Failure>) {
    let mut report = Report::default();
    let result = (|| {
        if expected.registry_id == 0
            || expected.location_id == 0
            || !(1..=127).contains(&expected.address)
            || (expected.vendor, expected.product, expected.configurations) != (0x0e41, 0x4650, 1)
        {
            return Err(Stop::InvalidSelection.into());
        }
        let _rig = lock()?;
        let before = observe(transport, expected, &mut report)?;
        if !matches!(before, 0 | BOTH_INTERFACES) {
            return Err(Stop::InvalidInterfaceView.into());
        }
        if !podxt_profile(transport.advertised_descriptor()) {
            return Err(Stop::AdvertisedDescriptor.into());
        }
        if observe(transport, expected, &mut report)? != before {
            return Err(Stop::InvalidInterfaceView.into());
        }
        let mut handle = transport.open_nonseizing()?;
        let operation = (|| {
            if observe(transport, expected, &mut report)? != before {
                return Err(Stop::InvalidInterfaceView);
            }
            let reply = handle.get_configuration()?;
            if reply.bytes_read != 1 {
                return Err(Stop::ConfigurationRead);
            }
            let fresh = reply.value;
            report.fresh_configuration = Some(fresh);
            let interfaces = observe(transport, expected, &mut report)?;
            match (fresh, before, interfaces) {
                (1, BOTH_INTERFACES, BOTH_INTERFACES) => {}
                (0, 0, 0) => {
                    if matches!(approval, Approval::ActivateOnce) {
                        handle.activate_one_with_interface_matching()?;
                        report.activation_accepted = true;
                    }
                }
                _ => return Err(Stop::InconsistentConfiguration),
            }
            Ok(())
        })();
        let close = handle.close();
        if operation.is_err() || close.is_err() {
            return Err(Failure {
                stopped: operation.err(),
                close_error: close.err(),
            });
        }
        if report.fresh_configuration == Some(0) && !report.activation_accepted {
            return Ok(());
        }

        let limit = if report.activation_accepted {
            OBSERVATIONS
        } else {
            1
        };
        for attempt in 0..limit {
            if attempt != 0 {
                wait(OBSERVATION_DELAY);
            }
            let interfaces = observe(transport, expected, &mut report)?;
            report.interfaces_after_close = Some(interfaces);
            if interfaces == BOTH_INTERFACES {
                if !podxt_profile(transport.refreshed_active_descriptor()?) {
                    return Err(Stop::ActiveDescriptorUnavailable.into());
                }
                if observe(transport, expected, &mut report)? != BOTH_INTERFACES {
                    return Err(Stop::InvalidInterfaceView.into());
                }
                report.active_descriptor_verified = true;
                return Ok(());
            }
        }
        Err(Stop::PublicationIncomplete.into())
    })();
    (report, result)
}

fn observe(
    transport: &mut impl Transport,
    expected: Attachment,
    report: &mut Report,
) -> Result<u8, Stop> {
    let snapshot = transport.snapshot()?;
    if snapshot.attachment != expected {
        return Err(Stop::AttachmentChanged);
    }
    report.registered_client_seen |= snapshot.registered_client;
    if snapshot.conflicting_binding {
        return Err(Stop::PublishedConflict);
    }
    if snapshot.interfaces & !BOTH_INTERFACES != 0 {
        return Err(Stop::InvalidInterfaceView);
    }
    Ok(snapshot.interfaces)
}

// Diagnostic profile from the reviewed 7e5a762 activation probe. The
// descriptor must advertise both vendor-class alt-1 MIDI interfaces, with
// the exact interrupt endpoints, packet size and interval used by PODxt Live.
fn podxt_profile(descriptor: &[u8]) -> bool {
    if descriptor.len() < 9
        || descriptor[..2] != [9, 2]
        || usize::from(u16::from_le_bytes([descriptor[2], descriptor[3]])) != descriptor.len()
        || descriptor[4..6] != [2, 1]
    {
        return false;
    }

    let mut interface = None;
    let mut endpoints = 0;
    let mut rest = &descriptor[9..];
    while !rest.is_empty() {
        let length = usize::from(rest[0]);
        if length < 2 || length > rest.len() {
            return false;
        }
        let (item, tail) = rest.split_at(length);
        rest = tail;
        match item[1] {
            4 if length >= 9 => {
                interface = (item[3] == 1 && item[5] == 0xff).then_some(item[2]);
            }
            4 => return false,
            5 if length >= 7 && item[3] & 3 == 3 && item[4..7] == [16, 0, 5] => {
                endpoints |= match (interface, item[2]) {
                    (Some(0), 0x03) => 1,
                    (Some(0), 0x84) => 2,
                    (Some(1), 0x05) => 4,
                    (Some(1), 0x86) => 8,
                    _ => 0,
                };
            }
            _ => {}
        }
    }
    endpoints == 0b1111
}

#[path = "podxt-macos-coexistence/tests.rs"]
mod tests;
