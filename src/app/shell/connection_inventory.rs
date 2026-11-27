//! Project current detector outcomes into the connection window.
use super::super::detection::{DetectedDevice, DeviceKey, DeviceSupport};
use super::{Workbench, card_icon, registry};
use std::collections::BTreeMap;
use variax_workbench::ui::connections::{Attachment, Connections, Device, Verification};

type RowKey = (String, &'static str);

struct Row {
    attachment: Attachment,
    current: Option<DeviceKey>,
}

#[derive(Default)]
pub(in crate::editor) struct ConnectionInventory {
    rows: BTreeMap<RowKey, Row>,
}

impl Workbench {
    pub(super) fn reconcile_connection_inventory(&mut self, detected: &[DetectedDevice]) {
        let Some(service) = &self.detection else {
            return;
        };
        let mut devices: BTreeMap<_, _> = service
            .offline_inventory(registry::DEVICE_PROFILES)
            .map(|(key, name)| (key, (name, Verification::Unsupported)))
            .collect();
        devices.extend(detected.iter().map(|device| {
            let name = match device.key.family_id {
                "variax" => "Variax 300/600",
                _ => device.model,
            };
            (device.key.clone(), (name, verification(device)))
        }));
        self.connection_inventory
            .reconcile(&mut self.connections, devices);
    }
}

impl ConnectionInventory {
    fn reconcile(
        &mut self,
        connections: &mut Connections,
        devices: BTreeMap<DeviceKey, (&'static str, Verification)>,
    ) {
        for row in self.rows.values_mut() {
            if row
                .current
                .as_ref()
                .is_some_and(|key| !devices.contains_key(key))
            {
                connections.disconnected(row.attachment);
                row.current = None;
            }
        }

        // A linked guitar's row depends on the exact current relay token.
        let (roots, linked): (Vec<_>, Vec<_>) = devices
            .into_iter()
            .partition(|(key, _)| key.podxt_upstream().is_none());
        for (key, (name, verification)) in roots.into_iter().chain(linked) {
            let upstream = if let Some(parent) = key.podxt_upstream() {
                let Some(row) = self
                    .rows
                    .get(&row_key(&parent))
                    .filter(|row| row.current.as_ref() == Some(&parent))
                else {
                    continue;
                };
                Some(row.attachment)
            } else {
                None
            };

            let next_device = self.rows.len() as u64;
            let row = self.rows.entry(row_key(&key)).or_insert(Row {
                attachment: Attachment {
                    device: next_device,
                    generation: 0,
                },
                current: None,
            });
            if row.current.as_ref() != Some(&key) {
                row.attachment.generation += 1;
                connections.arrived(
                    row.attachment,
                    Device {
                        name: name.into(),
                        artwork: card_icon(key.family_id),
                        offline_editor: Some(key.family_id),
                        upstream,
                    },
                );
                row.current = Some(key);
            }
            connections.verified(row.attachment, verification);
        }
    }
}

fn row_key(key: &DeviceKey) -> RowKey {
    (key.attachment.path.clone(), key.family_id)
}

fn verification(device: &DetectedDevice) -> Verification {
    match device.support {
        DeviceSupport::Verified(_) | DeviceSupport::Refreshing(_) => Verification::Verified,
        DeviceSupport::IdentityPending => Verification::Checking,
        DeviceSupport::FileEditingOnly => Verification::Unsupported,
        DeviceSupport::UnverifiedGuitarFirmware(notice) => {
            Verification::UnverifiedFirmware(notice.message())
        }
        DeviceSupport::UsbOnly => match device.pod_warning {
            Some(notice) => Verification::UnverifiedFirmware(notice.message()),
            None => Verification::Unverified(device.guitar_status.clone().unwrap_or_else(|| {
                "No verified identity is available for this connection.".into()
            })),
        },
    }
}
