//! Project current detector outcomes into the connection window.
use super::super::detection::{DetectedDevice, DeviceKey, DeviceSupport};
use super::tabs::{Connection, PODXT_SYNC_RESOURCE, SyncAdmission, SyncUpdate};
use super::{Workbench, card_icon, egui, registry};
use std::collections::{BTreeMap, BTreeSet};
use variax_workbench::ui::connections::{
    Action, Attachment, Connections, Device, Read, Verification,
};

type RowKey = (String, &'static str);

struct Row {
    attachment: Attachment,
    current: Option<DeviceKey>,
    startup: bool,
    sync: Sync,
}

#[derive(Default)]
enum Sync {
    #[default]
    Pending,
    Consent,
    Approved,
    Running(Read),
    Settled,
    Skipped,
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
        self.connection_inventory.reconcile(
            &mut self.connections,
            devices,
            self.startup_connection,
        );
    }

    pub(super) fn poll_connection_sync(&mut self, ctx: &egui::Context) {
        let allow_start = !self.has_prompt() && !self.permit_close;
        // Keep the relay's startup read ahead of its guitar sweep. The
        // workers still acquire the existing rig lock before opening a link.
        let relays_syncing: BTreeSet<_> = self
            .connection_inventory
            .rows
            .values()
            .filter(|row| matches!(row.sync, Sync::Pending | Sync::Running(_)))
            .filter_map(|row| row.current.as_ref())
            .filter(|key| key.family_id == "podxt" && self.verified_connection_tab(key).is_some())
            .cloned()
            .collect();
        for row in self.connection_inventory.rows.values_mut() {
            let Some(key) = &row.current else {
                continue;
            };
            let Some(entry) = self.tabs.entries.iter_mut().find(|entry| {
                entry
                    .device
                    .as_ref()
                    .is_some_and(|device| &device.device.key == key)
            }) else {
                if let Sync::Running(read) = row.sync {
                    self.connections
                        .finished(read, Err("The device tab was closed.".into()));
                    row.sync = Sync::Settled;
                }
                continue;
            };
            let device = entry.device.as_ref().expect("matched device tab");
            let verified = device.connection == Connection::Present
                && matches!(device.device.support, DeviceSupport::Verified(_));
            if let Sync::Running(read) = row.sync {
                let update = if verified {
                    entry.editor.connection_sync_update()
                } else {
                    entry.editor.cancel_connection_sync();
                    Some(SyncUpdate::Finished(Err(
                        "Device verification was lost.".into()
                    )))
                };
                let update = update.unwrap_or_else(|| {
                    SyncUpdate::Finished(Err("The download is no longer active.".into()))
                });
                row.apply_update(&mut self.connections, read, update);
                continue;
            }
            if !verified || !allow_start || !matches!(row.sync, Sync::Pending | Sync::Approved) {
                continue;
            }
            if row.startup
                && key
                    .podxt_upstream()
                    .is_some_and(|parent| relays_syncing.contains(&parent))
            {
                self.connections
                    .waiting_for_resource(row.attachment, PODXT_SYNC_RESOURCE);
                ctx.request_repaint_after(std::time::Duration::from_millis(60));
                continue;
            }
            match entry
                .editor
                .connection_sync_admission(row.startup || matches!(row.sync, Sync::Approved))
            {
                SyncAdmission::Busy => {
                    self.connections.waiting_for_editor(row.attachment);
                    ctx.request_repaint_after(std::time::Duration::from_millis(80));
                }
                SyncAdmission::Consent(prompt) => {
                    self.connections.consent(row.attachment, prompt);
                    row.sync = Sync::Consent;
                }
                SyncAdmission::Blocked(reason) => {
                    self.connections
                        .fail_to_start(row.attachment, reason.into());
                    row.sync = Sync::Settled;
                }
                SyncAdmission::Unavailable(reason) => {
                    self.connections.skip(row.attachment, reason);
                    row.sync = Sync::Settled;
                }
                SyncAdmission::Ready(total) => {
                    if let Some(read) = self.connections.started(row.attachment, total) {
                        match entry.editor.start_connection_sync(ctx) {
                            Ok(()) => {
                                self.connections.waiting_for_rig(read, PODXT_SYNC_RESOURCE);
                                row.sync = Sync::Running(read);
                                ctx.request_repaint();
                            }
                            Err(error) => row.apply_update(
                                &mut self.connections,
                                read,
                                SyncUpdate::Finished(Err(error)),
                            ),
                        }
                    }
                }
            }
        }
    }

    fn verified_connection_tab(&self, key: &DeviceKey) -> Option<super::TabId> {
        self.tabs.entries.iter().find_map(|entry| {
            let device = entry.device.as_ref()?;
            (device.device.key == *key
                && device.connection == Connection::Present
                && matches!(device.device.support, DeviceSupport::Verified(_)))
            .then_some(entry.id)
        })
    }

    pub(super) fn completed_connection_tab(&self, attachment: Attachment) -> Option<super::TabId> {
        if !self.connections.is_complete(attachment) {
            return None;
        }
        let row = self
            .connection_inventory
            .rows
            .values()
            .find(|row| row.attachment == attachment)?;
        self.verified_connection_tab(row.current.as_ref()?)
    }

    pub(super) fn skip_startup_sync(&mut self) {
        for row in self.connection_inventory.rows.values_mut() {
            match row.sync {
                Sync::Pending | Sync::Consent | Sync::Approved | Sync::Running(_) => {
                    if let Some(entry) = self.tabs.entries.iter_mut().find(|entry| {
                        entry.device.as_ref().map(|device| &device.device.key)
                            == row.current.as_ref()
                            && row.current.is_some()
                    }) {
                        entry.editor.cancel_connection_sync();
                    }
                    self.connections
                        .skip(row.attachment, "Startup download skipped");
                    row.sync = Sync::Skipped;
                }
                Sync::Settled | Sync::Skipped => {}
            }
        }
    }

    pub(super) fn connection_sync_action(&mut self, action: Action, ctx: &egui::Context) {
        let attachment = match action {
            Action::AllowDownload(attachment)
            | Action::SkipDownload(attachment)
            | Action::CancelDownload(attachment)
            | Action::Retry(attachment) => attachment,
            _ => return,
        };
        let Some(row) = self
            .connection_inventory
            .rows
            .values_mut()
            .find(|row| row.attachment == attachment && row.current.is_some())
        else {
            return;
        };
        match action {
            Action::AllowDownload(_) if matches!(row.sync, Sync::Consent) => {
                row.sync = Sync::Approved;
            }
            Action::SkipDownload(_)
                if matches!(row.sync, Sync::Consent)
                    || row.startup && matches!(row.sync, Sync::Pending) =>
            {
                self.connections.skip(attachment, "Download skipped");
                row.sync = Sync::Skipped;
            }
            Action::Retry(_) if self.connections.accept_retry(attachment) => {
                row.sync = Sync::Pending
            }
            Action::CancelDownload(_) => match row.sync {
                Sync::Running(_) => {
                    if let Some(entry) = self.tabs.entries.iter_mut().find(|entry| {
                        entry.device.as_ref().map(|device| &device.device.key)
                            == row.current.as_ref()
                    }) {
                        entry.editor.cancel_connection_sync();
                    }
                }
                Sync::Pending | Sync::Approved => {
                    self.connections
                        .fail_to_start(attachment, "Download cancelled.".into());
                    row.sync = Sync::Settled;
                }
                _ => {}
            },
            _ => {}
        }
        ctx.request_repaint();
    }
}

impl Row {
    fn apply_update(&mut self, connections: &mut Connections, read: Read, update: SyncUpdate) {
        match update {
            SyncUpdate::WaitingForRig => connections.waiting_for_rig(read, PODXT_SYNC_RESOURCE),
            SyncUpdate::Reading(count) => connections.progress(read, count),
            SyncUpdate::Finished(result) => {
                connections.finished(read, result);
                self.sync = Sync::Settled;
            }
        }
    }
}

impl ConnectionInventory {
    #[cfg(test)]
    pub(super) fn attachment_for_test(&self, path: &str, family: &'static str) -> Attachment {
        self.rows[&(path.to_owned(), family)].attachment
    }

    fn reconcile(
        &mut self,
        connections: &mut Connections,
        devices: BTreeMap<DeviceKey, (&'static str, Verification)>,
        startup: bool,
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
            let logical_row = row_key(&key);
            let (upstream, automatic) = if let Some(parent) = key.podxt_upstream() {
                let Some(relay) = self
                    .rows
                    .get(&row_key(&parent))
                    .filter(|row| row.current.as_ref() == Some(&parent))
                else {
                    continue;
                };
                // The first guitar may verify after the user opens the
                // completed relay's editor. A replacement guitar already
                // has a row, and a new relay has its own startup decision.
                let first_startup_guitar = connections.startup_in_background()
                    && relay.startup
                    && !self.rows.contains_key(&logical_row);
                (Some(relay.attachment), startup || first_startup_guitar)
            } else {
                (None, startup)
            };

            let next_device = self.rows.len() as u64;
            let row = self.rows.entry(logical_row).or_insert(Row {
                attachment: Attachment {
                    device: next_device,
                    generation: 0,
                },
                current: None,
                startup: automatic,
                sync: Sync::Pending,
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
                row.startup = automatic;
                row.sync = Sync::Pending;
            }
            if !matches!(verification, Verification::Verified) && !matches!(row.sync, Sync::Skipped)
            {
                row.sync = Sync::Pending;
            }
            let verified = matches!(verification, Verification::Verified);
            connections.verified(row.attachment, verification);
            if verified && matches!(row.sync, Sync::Skipped) {
                connections.skip(row.attachment, "Download skipped");
            }
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
