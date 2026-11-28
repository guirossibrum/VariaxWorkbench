use super::*;
use detection::{FreshSavedSlot, SavedSlotRefresh};
use std::collections::BTreeMap;

/// An admitted observer has caught up on the verified attachment and
/// forwards saves through the addressed read, foreground traffic and
/// checked release. Its route must also have verified global 28 semantics.
/// Neither native route currently provides this contract.
pub(super) struct Observer {
    pub(super) listener: Arc<detection::SaveListener>,
    pub(super) ingress: Arc<detection::PodxtSaveIngress>,
}

impl Observer {
    pub(super) fn begin_bank(
        &self,
        ingress: &Arc<detection::PodxtSaveIngress>,
    ) -> Option<BankRead> {
        if !Arc::ptr_eq(ingress, &self.ingress) {
            return None;
        }
        let slots = Location::all(&[BankKind::Tones])
            .map(|slot| {
                self.listener
                    .begin_saved_slot_refresh(slot)
                    .map(|proof| (slot, proof))
            })
            .collect::<Option<Vec<_>>>()?;
        Some(BankRead {
            ingress: Arc::clone(ingress),
            slots,
        })
    }
}

pub(super) struct BankRead {
    ingress: Arc<detection::PodxtSaveIngress>,
    slots: Vec<(Location, SavedSlotRefresh)>,
}

impl BankRead {
    /// Called only after a complete addressed download and checked release.
    pub(super) fn finish(self, snapshot: &Snapshot) -> Option<Arc<Bank>> {
        snapshot.complete_tone_bank()?;
        let slots = self
            .slots
            .into_iter()
            .map(|(location, refresh)| {
                let tone = snapshot.library.get(&location.address())?.clone();
                let stored = podxt::StoredTone { location, tone };
                self.ingress
                    .reconcile_saved_slot(refresh, stored)
                    .map(|proof| (location.address(), proof))
            })
            .collect::<Option<_>>()?;
        Some(Arc::new(Bank {
            ingress: self.ingress,
            slots,
            unusable: AtomicBool::new(false),
        }))
    }
}

pub(super) struct Bank {
    ingress: Arc<detection::PodxtSaveIngress>,
    slots: BTreeMap<u16, FreshSavedSlot>,
    unusable: AtomicBool,
}

impl Bank {
    pub(super) fn slot(&self, slot: Location) -> Option<&podxt::StoredTone> {
        if self.unusable.load(Ordering::Acquire) {
            return None;
        }
        self.ingress
            .fresh_saved_slot(slot, self.slots.get(&slot.address())?)
    }

    pub(super) fn candidate(
        self: &Arc<Self>,
        ingress: &Arc<detection::PodxtSaveIngress>,
        snapshot: &Snapshot,
        target: Location,
    ) -> Option<Arc<Self>> {
        snapshot.complete_tone_bank()?;
        let source = snapshot.channel?;
        (source != target
            && Arc::ptr_eq(ingress, &self.ingress)
            && [source, target].into_iter().all(|slot| {
                self.slot(slot).is_some_and(|stored| {
                    snapshot.library.get(&slot.address()) == Some(&stored.tone)
                })
            }))
        .then(|| Arc::clone(self))
    }

    pub(super) fn observes_pair(&self, source: Location, target: Location) -> bool {
        let revision = self.ingress.revision();
        self.slot(source).is_some()
            && self.slot(target).is_some()
            && self.ingress.revision() == revision
    }

    pub(super) fn invalidate(&self) {
        self.unusable.store(true, Ordering::Release);
    }
}
