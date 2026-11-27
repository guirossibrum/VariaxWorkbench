//! Presentation state for the device connection window. Discovery, transport
//! ownership, read authorization and library publication stay with the caller.

use std::num::NonZeroUsize;
use std::time::{Duration, Instant};

mod view;
pub use view::{Action, show};

#[cfg(test)]
mod tests;

pub const COMPLETE_PAUSE: Duration = Duration::from_millis(1400);

/// Stable logical row ID and a strictly increasing generation for that row.
/// A linked device names the exact upstream attachment it depends on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Attachment {
    pub device: u64,
    pub generation: u64,
}

/// Identifies a single read, including retries on the same attachment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Read {
    attachment: Attachment,
    sequence: u64,
}

impl Read {
    pub fn attachment(self) -> Attachment {
        self.attachment
    }
}

pub struct Device {
    pub name: String,
    pub artwork: Option<&'static str>,
    pub offline_editor: Option<&'static str>,
    pub upstream: Option<Attachment>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub enum Detection {
    #[default]
    Idle,
    Checking,
    Complete,
    Unavailable(String),
    Failed(String),
}

pub enum Verification {
    Checking,
    Verified,
    Unverified(String),
    UnverifiedFirmware(String),
    Unsupported,
}

#[derive(Debug, PartialEq, Eq)]
enum Status {
    Checking,
    Connected,
    Unverified(String),
    UnverifiedFirmware(String),
    Unsupported,
    Waiting,
    Reading {
        read: Read,
        done: usize,
        total: NonZeroUsize,
    },
    Complete,
    Failed(String),
    Disconnected,
}

struct Row {
    attachment: Attachment,
    device: Device,
    status: Status,
    reconnected: bool,
    shown: bool,
}

#[derive(Default)]
pub struct Connections {
    detection: Detection,
    rows: Vec<Row>,
    visible: bool,
    manual_close: bool,
    completed_read: bool,
    close_at: Option<Instant>,
    read_sequence: u64,
}

impl Connections {
    /// Open the startup presentation once, before the first detection result.
    pub fn begin_detection(&mut self) -> bool {
        if self.detection != Detection::Idle {
            return false;
        }
        self.detection(Detection::Checking);
        true
    }

    /// Report the startup check or a meaningful change in detection health.
    pub fn detection(&mut self, status: Detection) {
        if self.detection == status {
            return;
        }
        if self.detection == Detection::Idle
            || matches!(status, Detection::Unavailable(_) | Detection::Failed(_))
        {
            self.open();
        }
        self.detection = status;
        self.close_at = None;
    }

    /// Register an arrival before reporting identity or starting a read.
    pub fn arrived(&mut self, attachment: Attachment, device: Device) {
        let existing = self
            .rows
            .iter()
            .position(|row| row.attachment.device == attachment.device);
        if existing
            .is_some_and(|index| self.rows[index].attachment.generation >= attachment.generation)
            || device.upstream.is_some_and(|upstream| {
                !self
                    .rows
                    .iter()
                    .any(|row| row.attachment == upstream && row.status != Status::Disconnected)
            })
        {
            return;
        }
        if let Some(index) = existing
            && self.rows[index].status != Status::Disconnected
        {
            // Retire the old generation without reopening a dismissed
            // window for a disconnect the caller did not report.
            self.mark_disconnected(index);
        }
        self.open();
        let row = Row {
            attachment,
            device,
            status: Status::Checking,
            reconnected: existing.is_some(),
            shown: true,
        };
        if let Some(index) = existing {
            self.rows[index] = row;
        } else {
            self.rows.push(row);
        }
        self.close_at = None;
    }

    pub fn verified(&mut self, attachment: Attachment, verification: Verification) {
        let Some(row) = self.row_mut(attachment) else {
            return;
        };
        if row.status == Status::Disconnected {
            return;
        }
        // A positive snapshot can repeat during an active read or after it
        // finishes. Only loss of verification should invalidate that state.
        if matches!(verification, Verification::Verified)
            && matches!(
                row.status,
                Status::Waiting | Status::Reading { .. } | Status::Complete | Status::Failed(_)
            )
        {
            return;
        }
        row.status = match verification {
            Verification::Checking => Status::Checking,
            Verification::Verified => Status::Connected,
            Verification::Unverified(reason) => Status::Unverified(reason),
            Verification::UnverifiedFirmware(reason) => Status::UnverifiedFirmware(reason),
            Verification::Unsupported => Status::Unsupported,
        };
    }

    pub fn waiting(&mut self, attachment: Attachment) {
        if let Some(row) = self.row_mut(attachment)
            && row.status == Status::Connected
        {
            row.status = Status::Waiting;
        }
    }

    /// Call only when an existing authorized reader starts. The total is the
    /// selected reader scope, including setup entries if those are being read.
    pub fn started(&mut self, attachment: Attachment, total: NonZeroUsize) -> Option<Read> {
        if !matches!(
            self.row_mut(attachment)?.status,
            Status::Waiting | Status::Connected
        ) {
            return None;
        }
        self.read_sequence += 1;
        let read = Read {
            attachment,
            sequence: self.read_sequence,
        };
        self.row_mut(attachment)?.status = Status::Reading {
            read,
            done: 0,
            total,
        };
        Some(read)
    }

    pub fn progress(&mut self, read: Read, completed: usize) {
        if let Some(row) = self.reading_mut(read)
            && let Status::Reading { done, total, .. } = &mut row.status
            && (*done..=total.get()).contains(&completed)
        {
            *done = completed;
        }
    }

    /// Success means the caller has accepted the full library result.
    /// A stall or cancellation can be reported as a failed read.
    pub fn finished(&mut self, read: Read, result: Result<(), String>) {
        let completed_read = self.visible && result.is_ok();
        let Some(row) = self.reading_mut(read) else {
            return;
        };
        row.status = match result {
            Ok(()) => Status::Complete,
            Err(reason) => Status::Failed(reason),
        };
        self.completed_read |= completed_read;
    }

    /// A UI request has no read authority. The caller accepts it after
    /// validating the current attachment and existing consent, then queues it.
    pub fn accept_retry(&mut self, attachment: Attachment) -> bool {
        let Some(row) = self.row_mut(attachment) else {
            return false;
        };
        if matches!(row.status, Status::Failed(_)) {
            row.status = Status::Waiting;
            true
        } else {
            false
        }
    }

    /// Report the loss of a current attachment and reopen the connection
    /// window for acknowledgement if it was already dismissed.
    pub fn disconnected(&mut self, attachment: Attachment) {
        let Some(index) = self
            .rows
            .iter()
            .position(|row| row.attachment == attachment && row.status != Status::Disconnected)
        else {
            return;
        };
        // Open while the newly affected rows are still eligible to be shown.
        self.open();
        self.mark_disconnected(index);
    }

    fn mark_disconnected(&mut self, index: usize) {
        let row = &mut self.rows[index];
        let attachment = row.attachment;
        row.status = Status::Disconnected;
        for row in &mut self.rows {
            if row.device.upstream == Some(attachment) {
                row.status = Status::Disconnected;
            }
        }
        self.manual_close |= self.visible;
        self.close_at = None;
    }

    /// Run after applying all queued events for the current frame. Reevaluate
    /// every time, including the frame in which the deadline expires.
    fn advance(&mut self, now: Instant) -> Option<Duration> {
        if !self.visible
            || self.manual_close
            || self.detection != Detection::Complete
            || !self.completed_read
            || !self
                .shown_rows()
                .all(|row| matches!(row.status, Status::Complete | Status::Unsupported))
        {
            self.close_at = None;
            return None;
        }
        let deadline = *self.close_at.get_or_insert(now + COMPLETE_PAUSE);
        if now >= deadline {
            self.dismiss();
            None
        } else {
            Some(deadline - now)
        }
    }

    fn open(&mut self) {
        if !self.visible {
            self.visible = true;
            self.manual_close = false;
            self.completed_read = false;
            for row in &mut self.rows {
                row.shown = row.status != Status::Disconnected;
            }
        }
    }

    fn dismiss(&mut self) {
        self.visible = false;
        self.close_at = None;
    }

    fn shown_rows(&self) -> impl Iterator<Item = &Row> {
        self.rows.iter().filter(|row| row.shown)
    }

    fn row_mut(&mut self, attachment: Attachment) -> Option<&mut Row> {
        self.rows
            .iter_mut()
            .find(|row| row.attachment == attachment)
    }

    fn reading_mut(&mut self, read: Read) -> Option<&mut Row> {
        self.row_mut(read.attachment).filter(
            |row| matches!(row.status, Status::Reading { read: active, .. } if active == read),
        )
    }
}
