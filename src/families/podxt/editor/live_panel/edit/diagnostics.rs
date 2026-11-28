use super::*;
use variax_workbench::session_log::{
    Control as LogControl, PodxtChannel, PodxtDraftOrigin, UiAction,
};

#[cfg(any(test, target_os = "linux", target_os = "macos"))]
use variax_workbench::session_log::{CcClass, ControlDifference};

#[derive(Clone)]
pub(in crate::editor::podxt_panel::live_panel) struct RecallLog {
    operation: SessionOperation,
    target: PodxtChannel,
}

fn channel(slot: Location) -> Option<PodxtChannel> {
    if slot.kind() != BankKind::Tones {
        return None;
    }
    // Program Change and Current Channel use the contiguous channel index.
    PodxtChannel::new(u8::try_from(slot.index()).ok()?)
}

impl RecallLog {
    pub(super) fn requested(operation: SessionOperation, target: Location) -> Option<Self> {
        let log = Self {
            operation,
            target: channel(target)?,
        };
        log.button(Disposition::Accepted);
        log.sync(None, Reason::Requested, Evidence::None);
        Some(log)
    }

    pub(super) fn operation(&self) -> &SessionOperation {
        &self.operation
    }

    pub(super) fn button(&self, disposition: Disposition) {
        self.operation
            .ui(LogControl::PodxtChannel, UiAction::Button, disposition);
    }

    pub(super) fn sync(&self, source: Option<Location>, reason: Reason, evidence: Evidence) {
        self.operation.podxt_sync(
            source.and_then(channel),
            self.target,
            reason,
            evidence,
            None,
        );
    }

    pub(super) fn retained(&self, source: Location) {
        if let Some(source) = channel(source) {
            self.operation.podxt_retained_draft(
                source,
                self.target,
                PodxtDraftOrigin::DeviceBuffer,
            );
        }
    }

    #[cfg(any(test, target_os = "linux", target_os = "macos"))]
    pub(super) fn observe(&self, observation: podxt::RecallObservation<'_>) {
        match observation {
            podxt::RecallObservation::Comparison {
                source,
                matches_saved,
                comparison,
                opaque_changes,
            } => {
                if matches_saved {
                    self.sync(Some(source), Reason::Verified, Evidence::AddressedSlot);
                }
                if comparison.name_changed {
                    self.sync(
                        Some(source),
                        Reason::NameDifference,
                        Evidence::AddressedSlot,
                    );
                }
                for control in &comparison.controls {
                    let (reason, class) = if control.volatile() {
                        (Reason::VolatileDifference, CcClass::Volatile)
                    } else if control.name().is_some() {
                        (Reason::ControlDifference, CcClass::Named)
                    } else {
                        (Reason::ControlDifference, CcClass::Unknown)
                    };
                    if let Some(difference) =
                        ControlDifference::cc(control.cc, control.playing, control.saved, class)
                    {
                        self.difference(source, reason, difference);
                    }
                }
                for &(offset, bits) in opaque_changes {
                    if let Some(difference) = ControlDifference::opaque_bits(offset, bits) {
                        self.difference(source, Reason::OpaqueDifference, difference);
                    }
                }
            }
            podxt::RecallObservation::Readback { source, verified } => self.sync(
                Some(source),
                if verified {
                    Reason::Verified
                } else {
                    Reason::Failed
                },
                Evidence::VerifiedPlaying,
            ),
        }
    }

    #[cfg(any(test, target_os = "linux", target_os = "macos"))]
    fn difference(&self, source: Location, reason: Reason, difference: ControlDifference) {
        self.operation.podxt_sync(
            channel(source),
            self.target,
            reason,
            Evidence::AddressedSlot,
            Some(difference),
        );
    }
}

#[derive(Clone, Copy)]
pub(super) struct ControlChange {
    pub(super) control: LiveControl,
    pub(super) old: u16,
    pub(super) new: u16,
}

impl ControlChange {
    fn action(self) -> UiAction {
        match self.control {
            LiveControl::Basic(control) if control.is_switch() => UiAction::Switch {
                old: control.is_on(self.old as u8),
                new: control.is_on(self.new as u8),
            },
            _ => UiAction::Dial {
                old: i32::from(self.old),
                new: i32::from(self.new),
            },
        }
    }
}

pub(super) struct ControlBatch {
    pub(super) operation: SessionOperation,
    changes: Vec<ControlChange>,
}

impl ControlBatch {
    pub(super) fn new(operation: SessionOperation) -> Self {
        Self {
            operation,
            changes: Vec::new(),
        }
    }

    pub(super) fn queue(&mut self, change: ControlChange) {
        self.event(change, Disposition::Queued);
        if let Some(index) = self
            .changes
            .iter()
            .position(|prior| prior.control == change.control)
        {
            self.event(self.changes[index], Disposition::Coalesced);
            self.changes[index] = change;
        } else {
            self.changes.push(change);
        }
    }

    pub(super) fn record(&self, disposition: Disposition, observed: Option<&EditorBuffer>) {
        for &change in &self.changes {
            self.event(
                ControlChange {
                    new: observed.map_or(change.new, |buffer| change.control.read(&buffer.tone)),
                    ..change
                },
                disposition,
            );
        }
    }

    fn event(&self, change: ControlChange, disposition: Disposition) {
        self.operation
            .ui(LogControl::PodxtControl, change.action(), disposition);
    }
}
