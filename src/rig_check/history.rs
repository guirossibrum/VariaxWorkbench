//! Navigation over recorded results. A history view leaves its prompt pending.

use super::{catalogue::StepId, record::StepRecord};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Review {
    Start,
    Result(StepId),
}

/// With no anchor, review the last recorded result before the current card.
pub(super) fn back(
    records: &[StepRecord],
    anchor: Option<StepId>,
    accepted_start: bool,
) -> Option<Review> {
    let before = match anchor {
        Some(id) => records.iter().position(|record| record.id == id)?,
        None => records.len(),
    };
    records[..before]
        .last()
        .map(|record| Review::Result(record.id))
        .or_else(|| accepted_start.then_some(Review::Start))
}
