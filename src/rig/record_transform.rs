#![forbid(unsafe_code)]
//! Source-scoped CPU transformation for initialized POD HD500X driver-list
//! records. This models owned initialized bytes only; it is not a queue,
//! callback adapter, physical endpoint, or Windows synchronization model.

use super::model::ProductProfile;
use super::session::Rejection;

/// Record state carried by the accepted owned-memory transform.
///
/// The cursor names are neutral: the extracted driver uses these fields in
/// direction-specific helpers, but they are not physical stream labels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RecordSnapshot {
    /// Source record qword +0x28.
    pub cursor_a: u32,
    /// Source record qword +0x30.
    pub cursor_b: u32,
    /// Source record byte +0x38.
    pub wrapped: bool,
    /// Source record qword +0x40, preserved on source short/error branches.
    pub derived_a: u64,
    /// Source record qword +0x48, preserved on source short/error branches.
    pub derived_b: u64,
    /// Source record qword +0x68 (record-to-caller byte counter).
    pub record_to_caller_bytes: u64,
    /// Source record qword +0x70 (caller-to-record byte counter).
    pub caller_to_record_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Segment {
    pub offset: u32,
    pub len: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceStatus {
    Success,
    ShortRead,
    ShortWrite,
}

impl SourceStatus {
    pub const fn code(self) -> u32 {
        match self {
            Self::Success => 0,
            Self::ShortRead => 1,
            Self::ShortWrite => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndpointState {
    UnverifiedAndLocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferDirection {
    RecordToCaller,
    CallerToRecord,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransferReport {
    pub profile: ProductProfile,
    pub endpoint: EndpointState,
    pub direction: TransferDirection,
    pub requested_bytes: u32,
    pub copied_bytes: u32,
    pub segments: [Option<Segment>; 2],
    pub status: SourceStatus,
    pub source_success: bool,
    pub next: RecordSnapshot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdmissionError {
    UnsupportedPolicy {
        policy: u8,
    },
    EmptyRing {
        source_status: u32,
    },
    CapacityTooLarge,
    RequestTooLarge {
        source_status: u32,
    },
    CursorOutsideRing {
        cursor: u32,
        capacity: u32,
        source_status: u32,
    },
    InvalidReadOrdering {
        source_status: u32,
    },
    CallerBufferTooSmall {
        required: u32,
        available: usize,
    },
    SegmentOutsideRing,
}

impl std::fmt::Display for AdmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedPolicy { policy } => {
                write!(formatter, "record policy {policy} is not admitted")
            }
            Self::EmptyRing { .. } => formatter.write_str("record ring capacity must be nonzero"),
            Self::CapacityTooLarge => {
                formatter.write_str("record ring capacity exceeds the source DWORD bound")
            }
            Self::RequestTooLarge { .. } => {
                formatter.write_str("requested record transfer exceeds ring capacity")
            }
            Self::CursorOutsideRing {
                cursor, capacity, ..
            } => write!(
                formatter,
                "record cursor {cursor} is outside supported range 0..={capacity}"
            ),
            Self::InvalidReadOrdering { .. } => {
                formatter.write_str("record read cursor ordering is not admitted")
            }
            Self::CallerBufferTooSmall {
                required,
                available,
            } => write!(
                formatter,
                "caller buffer has {available} bytes; {required} are required"
            ),
            Self::SegmentOutsideRing => {
                formatter.write_str("source descriptor segment is outside owned ring memory")
            }
        }
    }
}

impl std::error::Error for AdmissionError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransformError {
    Profile(Rejection),
    Admission(AdmissionError),
}

impl std::fmt::Display for TransformError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Profile(rejection) => {
                write!(formatter, "record transform profile: {rejection:?}")
            }
            Self::Admission(error) => write!(formatter, "record transform admission: {error}"),
        }
    }
}

impl std::error::Error for TransformError {}

impl From<AdmissionError> for TransformError {
    fn from(error: AdmissionError) -> Self {
        Self::Admission(error)
    }
}

#[derive(Clone, Copy)]
struct Plan {
    next: RecordSnapshot,
    segments: [Option<Segment>; 2],
    status: SourceStatus,
    copied_bytes: u32,
}

fn validate_common(
    state: RecordSnapshot,
    capacity: usize,
    requested: usize,
    policy: u8,
) -> Result<(u32, u32), AdmissionError> {
    if policy != 1 {
        return Err(AdmissionError::UnsupportedPolicy { policy });
    }
    if capacity == 0 {
        return Err(AdmissionError::EmptyRing { source_status: 4 });
    }
    let capacity = u32::try_from(capacity).map_err(|_| AdmissionError::CapacityTooLarge)?;
    let requested = u32::try_from(requested)
        .map_err(|_| AdmissionError::RequestTooLarge { source_status: 3 })?;
    if requested > capacity {
        return Err(AdmissionError::RequestTooLarge { source_status: 3 });
    }
    for cursor in [state.cursor_a, state.cursor_b] {
        if cursor > capacity {
            return Err(AdmissionError::CursorOutsideRing {
                cursor,
                capacity,
                source_status: 4,
            });
        }
    }
    Ok((capacity, requested))
}

fn segment(offset: u32, len: u32, capacity: u32) -> Result<Option<Segment>, AdmissionError> {
    let end = offset
        .checked_add(len)
        .ok_or(AdmissionError::SegmentOutsideRing)?;
    if end > capacity {
        return Err(AdmissionError::SegmentOutsideRing);
    }
    Ok((len != 0).then_some(Segment { offset, len }))
}

fn validate_caller_buffer(available: usize, requested: u32) -> Result<(), AdmissionError> {
    if available < requested as usize {
        return Err(AdmissionError::CallerBufferTooSmall {
            required: requested,
            available,
        });
    }
    Ok(())
}

fn byte_sum(segments: [Option<Segment>; 2]) -> u32 {
    segments
        .into_iter()
        .flatten()
        .fold(0u32, |sum, part| sum.wrapping_add(part.len))
}

fn update_read_derived_unwrapped(next: &mut RecordSnapshot, capacity: u32, old_a: u32) {
    next.derived_a = u64::from(next.cursor_b).wrapping_sub(u64::from(old_a));
    next.derived_b = u64::from(old_a)
        .wrapping_sub(u64::from(next.cursor_b))
        .wrapping_add(u64::from(capacity));
}

fn update_read_derived_wrapped(next: &mut RecordSnapshot, capacity: u32, old_a: u32) {
    next.derived_a = u64::from(next.cursor_b)
        .wrapping_sub(u64::from(old_a))
        .wrapping_add(u64::from(capacity));
    next.derived_b = u64::from(old_a).wrapping_sub(u64::from(next.cursor_b));
}

fn update_write_derived_unwrapped(next: &mut RecordSnapshot, capacity: u32, old_b: u32) {
    next.derived_a = u64::from(old_b).wrapping_sub(u64::from(next.cursor_a));
    next.derived_b = u64::from(next.cursor_a)
        .wrapping_sub(u64::from(old_b))
        .wrapping_add(u64::from(capacity));
}

fn update_write_derived_wrapped(next: &mut RecordSnapshot, capacity: u32, old_b: u32) {
    next.derived_a = u64::from(old_b)
        .wrapping_sub(u64::from(next.cursor_a))
        .wrapping_add(u64::from(capacity));
    next.derived_b = u64::from(next.cursor_a).wrapping_sub(u64::from(old_b));
}

fn plan_record_to_caller(
    original: RecordSnapshot,
    capacity: u32,
    requested: u32,
) -> Result<Plan, AdmissionError> {
    let mut next = original;
    let mut segments = [None, None];
    let mut status = SourceStatus::Success;
    let base = 0;
    let end = capacity;

    if !original.wrapped {
        if original.cursor_b < original.cursor_a {
            return Err(AdmissionError::InvalidReadOrdering { source_status: 4 });
        }
        let first_available = end - original.cursor_b;
        let first_len = first_available.min(requested);
        segments[0] = segment(original.cursor_b, first_len, capacity)?;
        if first_available >= requested {
            let mut new_b = original.cursor_b + requested;
            let reached_end = new_b == end;
            if reached_end {
                new_b = base;
                next.wrapped = true;
            }
            next.cursor_b = new_b;
            if reached_end {
                update_read_derived_wrapped(&mut next, capacity, original.cursor_a);
            } else {
                update_read_derived_unwrapped(&mut next, capacity, original.cursor_a);
            }
        } else {
            let remaining = requested - first_available;
            let second_available = original.cursor_a - base;
            segments[1] = segment(base, second_available.min(remaining), capacity)?;
            if second_available >= remaining {
                segments[1] = segment(base, remaining, capacity)?;
                next.cursor_b = base + remaining;
                next.wrapped = true;
                update_read_derived_wrapped(&mut next, capacity, original.cursor_a);
            } else {
                status = SourceStatus::ShortRead;
                // In the observed policy-1 nonwrapped short branch, +0x18 is
                // overwritten with the requested remainder, even when that
                // crosses the logical availability bound at cursor A.
                segments[1] = segment(base, remaining, capacity)?;
                next.cursor_a = base + remaining;
                next.cursor_b = base + remaining;
            }
        }
    } else {
        if original.cursor_a < original.cursor_b {
            return Err(AdmissionError::InvalidReadOrdering { source_status: 4 });
        }
        let first_available = original.cursor_a - original.cursor_b;
        if first_available >= requested {
            segments[0] = segment(original.cursor_b, requested, capacity)?;
            next.cursor_b = original.cursor_b + requested;
            update_read_derived_wrapped(&mut next, capacity, original.cursor_a);
        } else {
            status = SourceStatus::ShortRead;
            let tail_available = end - original.cursor_b;
            if tail_available >= requested {
                // VA 0x92ede writes the first length (+8), not the second
                // length (+0x18), before the status-1 cursor commit.
                segments[0] = segment(original.cursor_b, requested, capacity)?;
                let new_cursor = original.cursor_b + requested;
                next.cursor_a = new_cursor;
                next.cursor_b = new_cursor;
            } else {
                let remaining = requested - tail_available;
                segments[0] = segment(original.cursor_b, tail_available, capacity)?;
                segments[1] = segment(base, remaining, capacity)?;
                let new_cursor = base + remaining;
                next.cursor_a = new_cursor;
                next.cursor_b = new_cursor;
            }
        }
    }

    let copied_bytes = byte_sum(segments);
    next.record_to_caller_bytes = next
        .record_to_caller_bytes
        .wrapping_add(u64::from(copied_bytes));
    Ok(Plan {
        next,
        segments,
        status,
        copied_bytes,
    })
}

fn plan_caller_to_record(
    original: RecordSnapshot,
    capacity: u32,
    requested: u32,
) -> Result<Plan, AdmissionError> {
    let mut next = original;
    let mut segments = [None, None];
    let mut status = SourceStatus::Success;
    let base = 0;
    let end = capacity;

    if !original.wrapped {
        if original.cursor_b < original.cursor_a {
            // Source VA 0x92fd9 jumps to the empty status-0 path.
        } else {
            let available = original.cursor_b - original.cursor_a;
            segments[0] = segment(original.cursor_a, available.min(requested), capacity)?;
            if available >= requested {
                segments[0] = segment(original.cursor_a, requested, capacity)?;
                next.cursor_a = original.cursor_a + requested;
                update_write_derived_unwrapped(&mut next, capacity, original.cursor_b);
            } else {
                segments[0] = None;
                status = SourceStatus::ShortWrite;
            }
        }
    } else if original.cursor_a < original.cursor_b {
        // Source VA 0x93041 jumps to the empty status-0 path.
    } else {
        let first_available = end - original.cursor_a;
        segments[0] = segment(original.cursor_a, first_available.min(requested), capacity)?;
        if first_available >= requested {
            segments[0] = segment(original.cursor_a, requested, capacity)?;
            let mut new_a = original.cursor_a + requested;
            let reached_end = new_a == end;
            if reached_end {
                new_a = base;
                next.wrapped = false;
            }
            next.cursor_a = new_a;
            if reached_end {
                update_write_derived_unwrapped(&mut next, capacity, original.cursor_b);
            } else {
                update_write_derived_wrapped(&mut next, capacity, original.cursor_b);
            }
        } else {
            let remaining = requested - first_available;
            let second_available = original.cursor_b - base;
            segments[1] = segment(base, second_available.min(remaining), capacity)?;
            if second_available >= remaining {
                segments[1] = segment(base, remaining, capacity)?;
                next.cursor_a = base + remaining;
                next.wrapped = false;
                update_write_derived_unwrapped(&mut next, capacity, original.cursor_b);
            } else {
                // The helper clears first len, retains the second span and
                // reports status 2 without committing cursors/derived state.
                segments[0] = None;
                status = SourceStatus::ShortWrite;
            }
        }
    }

    let copied_bytes = byte_sum(segments);
    next.caller_to_record_bytes = next
        .caller_to_record_bytes
        .wrapping_add(u64::from(copied_bytes));
    Ok(Plan {
        next,
        segments,
        status,
        copied_bytes,
    })
}

fn report(plan: Plan, requested_bytes: u32, direction: TransferDirection) -> TransferReport {
    TransferReport {
        profile: SOURCE_PROFILE,
        endpoint: EndpointState::UnverifiedAndLocked,
        direction,
        requested_bytes,
        copied_bytes: plan.copied_bytes,
        segments: plan.segments,
        status: plan.status,
        source_success: plan.status == SourceStatus::Success,
        next: plan.next,
    }
}

/// Applies the source-selected policy-1 record-to-caller transform to owned
/// initialized ring bytes. All validation and descriptor planning precede
/// mutation. The destination must be able to hold the requested count even
/// when source policy could return fewer bytes.
fn record_to_caller(
    state: &mut RecordSnapshot,
    ring: &[u8],
    destination: &mut [u8],
    requested: usize,
    policy: u8,
) -> Result<TransferReport, AdmissionError> {
    let (capacity, requested) = validate_common(*state, ring.len(), requested, policy)?;
    validate_caller_buffer(destination.len(), requested)?;
    let plan = plan_record_to_caller(*state, capacity, requested)?;
    let mut output_offset = 0usize;
    for part in plan.segments.into_iter().flatten() {
        let start = part.offset as usize;
        let end = start + part.len as usize;
        let next_output = output_offset + part.len as usize;
        destination[output_offset..next_output].copy_from_slice(&ring[start..end]);
        output_offset = next_output;
    }
    *state = plan.next;
    Ok(report(plan, requested, TransferDirection::RecordToCaller))
}

/// Applies the source record's caller-to-record transform to owned initialized
/// ring bytes. All validation and descriptor planning precede mutation.
fn caller_to_record(
    state: &mut RecordSnapshot,
    ring: &mut [u8],
    source: &[u8],
    requested: usize,
    policy: u8,
) -> Result<TransferReport, AdmissionError> {
    let (capacity, requested) = validate_common(*state, ring.len(), requested, policy)?;
    validate_caller_buffer(source.len(), requested)?;
    let plan = plan_caller_to_record(*state, capacity, requested)?;
    let mut input_offset = 0usize;
    for part in plan.segments.into_iter().flatten() {
        let start = part.offset as usize;
        let end = start + part.len as usize;
        let next_input = input_offset + part.len as usize;
        ring[start..end].copy_from_slice(&source[input_offset..next_input]);
        input_offset = next_input;
    }
    *state = plan.next;
    Ok(report(plan, requested, TransferDirection::CallerToRecord))
}

/// Validates an admitted HD500X session before delegating to the pure
/// transformation. It never opens an endpoint or changes device capability.
pub(super) fn session_record_to_caller(
    session: super::session::Session,
    state: &mut RecordSnapshot,
    ring: &[u8],
    destination: &mut [u8],
    requested: usize,
    policy: u8,
) -> Result<TransferReport, TransformError> {
    session
        .require_record_transform_profile()
        .map_err(TransformError::Profile)?;
    record_to_caller(state, ring, destination, requested, policy).map_err(Into::into)
}

pub(super) fn session_caller_to_record(
    session: super::session::Session,
    state: &mut RecordSnapshot,
    ring: &mut [u8],
    source: &[u8],
    requested: usize,
    policy: u8,
) -> Result<TransferReport, TransformError> {
    session
        .require_record_transform_profile()
        .map_err(TransformError::Profile)?;
    caller_to_record(state, ring, source, requested, policy).map_err(Into::into)
}

pub(super) const SOURCE_PROFILE: ProductProfile = ProductProfile::PodHd500x;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rig::{CapabilityState, Operation, Session};

    const RING: [u8; 8] = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];

    fn hd_session() -> Session {
        Session::extension(ProductProfile::PodHd500x).expect("HD500X extension")
    }

    fn snap(a: u32, b: u32, wrapped: bool) -> RecordSnapshot {
        RecordSnapshot {
            cursor_a: a,
            cursor_b: b,
            wrapped,
            ..RecordSnapshot::default()
        }
    }

    #[test]
    fn record_to_caller_contiguous_and_crossing_vectors_match_literal_expectations() {
        let session = hd_session();
        let mut state = snap(2, 4, false);
        let mut dst = [0xa0; 8];
        let report = session
            .transform_hd500x_record_to_caller(&mut state, &RING, &mut dst, 3, 1)
            .unwrap();
        assert_eq!(report.segments, [Some(Segment { offset: 4, len: 3 }), None]);
        assert_eq!(report.status, SourceStatus::Success);
        assert_eq!(report.direction, TransferDirection::RecordToCaller);
        assert!(report.source_success);
        assert_eq!(report.requested_bytes, 3);
        assert_eq!(report.copied_bytes, 3);
        assert_eq!(&dst[..3], &[0x55, 0x66, 0x77]);
        assert_eq!(&dst[3..], &[0xa0; 5]);
        assert_eq!(
            state,
            RecordSnapshot {
                cursor_a: 2,
                cursor_b: 7,
                wrapped: false,
                derived_a: 5,
                derived_b: 3,
                record_to_caller_bytes: 3,
                caller_to_record_bytes: 0,
            }
        );

        let mut crossing = snap(2, 6, false);
        let mut crossing_dst = [0xa0; 8];
        let crossing_report = session
            .transform_hd500x_record_to_caller(&mut crossing, &RING, &mut crossing_dst, 4, 1)
            .unwrap();
        assert_eq!(
            crossing_report.segments,
            [
                Some(Segment { offset: 6, len: 2 }),
                Some(Segment { offset: 0, len: 2 })
            ]
        );
        assert_eq!(&crossing_dst[..4], &[0x77, 0x88, 0x11, 0x22]);
        assert_eq!(crossing_report.status, SourceStatus::Success);
        assert_eq!(
            crossing,
            RecordSnapshot {
                cursor_a: 2,
                cursor_b: 2,
                wrapped: true,
                derived_a: 8,
                derived_b: 0,
                record_to_caller_bytes: 4,
                caller_to_record_bytes: 0,
            }
        );

        let mut exact_end = snap(2, 5, false);
        let mut exact_end_dst = [0xa0; 8];
        let exact_end_report = session
            .transform_hd500x_record_to_caller(&mut exact_end, &RING, &mut exact_end_dst, 3, 1)
            .unwrap();
        assert_eq!(
            exact_end_report.segments,
            [Some(Segment { offset: 5, len: 3 }), None]
        );
        assert_eq!(&exact_end_dst[..3], &[0x66, 0x77, 0x88]);
        assert_eq!(
            exact_end,
            RecordSnapshot {
                cursor_a: 2,
                cursor_b: 0,
                wrapped: true,
                derived_a: 6,
                derived_b: 2,
                record_to_caller_bytes: 3,
                caller_to_record_bytes: 0,
            }
        );

        let mut zero_nonwrapped = snap(2, 4, false);
        let mut zero_dst = [0xa0; 1];
        let zero = session
            .transform_hd500x_record_to_caller(&mut zero_nonwrapped, &RING, &mut zero_dst, 0, 1)
            .unwrap();
        assert_eq!(zero.segments, [None, None]);
        assert_eq!(zero.status, SourceStatus::Success);
        assert_eq!(
            zero_nonwrapped,
            RecordSnapshot {
                cursor_a: 2,
                cursor_b: 4,
                wrapped: false,
                derived_a: 2,
                derived_b: 6,
                record_to_caller_bytes: 0,
                caller_to_record_bytes: 0,
            }
        );
    }

    #[test]
    fn policy_one_nonwrapped_short_read_copies_requested_initialized_bytes_and_commits_both_cursors()
     {
        let session = hd_session();
        let mut state = snap(1, 6, false);
        state.derived_a = 17;
        state.derived_b = 29;
        let mut dst = [0xa0; 8];
        let report = session
            .transform_hd500x_record_to_caller(&mut state, &RING, &mut dst, 4, 1)
            .unwrap();
        assert_eq!(
            report.segments,
            [
                Some(Segment { offset: 6, len: 2 }),
                Some(Segment { offset: 0, len: 2 })
            ]
        );
        assert_eq!(&dst[..4], &[0x77, 0x88, 0x11, 0x22]);
        assert_eq!(report.copied_bytes, 4);
        assert_eq!(report.status, SourceStatus::ShortRead);
        assert_eq!(report.direction, TransferDirection::RecordToCaller);
        assert!(!report.source_success);
        assert_eq!(state.cursor_a, 2);
        assert_eq!(state.cursor_b, 2);
        assert!(!state.wrapped);
        assert_eq!(state.derived_a, 17);
        assert_eq!(state.derived_b, 29);
        assert_eq!(state.record_to_caller_bytes, 4);
    }

    #[test]
    fn policy_one_wrapped_short_read_has_both_exact_source_branches() {
        let session = hd_session();
        let mut crossing = snap(7, 6, true);
        crossing.derived_a = 41;
        crossing.derived_b = 42;
        let mut dst = [0xa0; 8];
        let report = session
            .transform_hd500x_record_to_caller(&mut crossing, &RING, &mut dst, 5, 1)
            .unwrap();
        assert_eq!(
            report.segments,
            [
                Some(Segment { offset: 6, len: 2 }),
                Some(Segment { offset: 0, len: 3 })
            ]
        );
        assert_eq!(&dst[..5], &[0x77, 0x88, 0x11, 0x22, 0x33]);
        assert_eq!(report.status, SourceStatus::ShortRead);
        assert_eq!(report.copied_bytes, 5);
        assert_eq!(crossing.cursor_a, 3);
        assert_eq!(crossing.cursor_b, 3);
        assert!(crossing.wrapped);
        assert_eq!(crossing.record_to_caller_bytes, 5);
        assert_eq!(crossing.derived_a, 41);
        assert_eq!(crossing.derived_b, 42);

        let mut contiguous = snap(5, 4, true);
        contiguous.derived_a = 51;
        contiguous.derived_b = 52;
        let mut contiguous_dst = [0xa0; 8];
        let contiguous_report = session
            .transform_hd500x_record_to_caller(&mut contiguous, &RING, &mut contiguous_dst, 3, 1)
            .unwrap();
        assert_eq!(
            contiguous_report.segments,
            [Some(Segment { offset: 4, len: 3 }), None]
        );
        assert_eq!(&contiguous_dst[..3], &[0x55, 0x66, 0x77]);
        assert_eq!(contiguous_report.status, SourceStatus::ShortRead);
        assert_eq!(contiguous.cursor_a, 7);
        assert_eq!(contiguous.cursor_b, 7);
        assert!(contiguous.wrapped);
        assert_eq!(contiguous.record_to_caller_bytes, 3);
        assert_eq!(contiguous.derived_a, 51);
        assert_eq!(contiguous.derived_b, 52);
    }

    #[test]
    fn wrapped_full_read_empty_normalization_and_sequential_transition_are_explicit() {
        let session = hd_session();
        let mut state = snap(6, 2, true);
        let mut dst = [0xa0; 8];
        let report = session
            .transform_hd500x_record_to_caller(&mut state, &RING, &mut dst, 3, 1)
            .unwrap();
        assert_eq!(report.segments, [Some(Segment { offset: 2, len: 3 }), None]);
        assert_eq!(&dst[..3], &[0x33, 0x44, 0x55]);
        assert_eq!(
            state,
            RecordSnapshot {
                cursor_a: 6,
                cursor_b: 5,
                wrapped: true,
                derived_a: 7,
                derived_b: 1,
                record_to_caller_bytes: 3,
                caller_to_record_bytes: 0,
            }
        );

        let mut at_end = snap(2, 8, false);
        let mut empty_dst = [0xa0; 8];
        let empty = session
            .transform_hd500x_record_to_caller(&mut at_end, &RING, &mut empty_dst, 0, 1)
            .unwrap();
        assert_eq!(empty.segments, [None, None]);
        assert_eq!(empty.status, SourceStatus::Success);
        assert_eq!(empty.copied_bytes, 0);
        assert_eq!(
            at_end,
            RecordSnapshot {
                cursor_a: 2,
                cursor_b: 0,
                wrapped: true,
                derived_a: 6,
                derived_b: 2,
                record_to_caller_bytes: 0,
                caller_to_record_bytes: 0,
            }
        );

        // A full-capacity request with the read cursor at the base is admitted
        // as one literal contiguous span.
        let mut full = snap(8, 0, true);
        let mut full_dst = [0xa0; 8];
        let full_report = session
            .transform_hd500x_record_to_caller(&mut full, &RING, &mut full_dst, 8, 1)
            .unwrap();
        assert_eq!(
            full_report.segments,
            [Some(Segment { offset: 0, len: 8 }), None]
        );
        assert_eq!(full_dst, RING);
        assert_eq!(full_report.requested_bytes, 8);
        assert_eq!(full_report.copied_bytes, 8);
        assert_eq!(full_report.status, SourceStatus::Success);
        assert_eq!(
            full,
            RecordSnapshot {
                cursor_a: 8,
                cursor_b: 8,
                wrapped: true,
                derived_a: 8,
                derived_b: 0,
                record_to_caller_bytes: 8,
                caller_to_record_bytes: 0,
            }
        );

        let mut wrapped_zero_read = snap(6, 2, true);
        let mut wrapped_zero_dst = [0xa0; 1];
        let wrapped_zero_report = session
            .transform_hd500x_record_to_caller(
                &mut wrapped_zero_read,
                &RING,
                &mut wrapped_zero_dst,
                0,
                1,
            )
            .unwrap();
        assert_eq!(wrapped_zero_report.segments, [None, None]);
        assert_eq!(
            wrapped_zero_read,
            RecordSnapshot {
                cursor_a: 6,
                cursor_b: 2,
                wrapped: true,
                derived_a: 4,
                derived_b: 4,
                record_to_caller_bytes: 0,
                caller_to_record_bytes: 0,
            }
        );

        // Feed the first operation's next snapshot into the opposite source
        // direction to prove that sequential state is carried explicitly.
        let mut sequential = snap(2, 4, false);
        let mut sequence_dst = [0xa0; 4];
        let first = session
            .transform_hd500x_record_to_caller(&mut sequential, &RING, &mut sequence_dst, 2, 1)
            .unwrap();
        assert_eq!(sequence_dst, [0x55, 0x66, 0xa0, 0xa0]);
        assert_eq!(first.next, sequential);
        assert_eq!(
            sequential,
            RecordSnapshot {
                cursor_a: 2,
                cursor_b: 6,
                wrapped: false,
                derived_a: 4,
                derived_b: 4,
                record_to_caller_bytes: 2,
                caller_to_record_bytes: 0,
            }
        );
        let mut sequence_ring = RING;
        let second = session
            .transform_hd500x_caller_to_record(&mut sequential, &mut sequence_ring, &[0xb1], 1, 1)
            .unwrap();
        assert_eq!(second.segments, [Some(Segment { offset: 2, len: 1 }), None]);
        assert_eq!(
            sequence_ring,
            [0x11, 0x22, 0xb1, 0x44, 0x55, 0x66, 0x77, 0x88]
        );
        assert_eq!(
            sequential,
            RecordSnapshot {
                cursor_a: 3,
                cursor_b: 6,
                wrapped: false,
                derived_a: 3,
                derived_b: 5,
                record_to_caller_bytes: 2,
                caller_to_record_bytes: 1,
            }
        );
    }

    #[test]
    fn caller_to_record_contiguous_and_wrapped_full_vectors_are_literal() {
        let session = hd_session();
        let mut ring = RING;
        let mut state = snap(2, 5, false);
        let report = session
            .transform_hd500x_caller_to_record(&mut state, &mut ring, &[0xd1, 0xd2, 0xd3], 3, 1)
            .unwrap();
        assert_eq!(report.segments, [Some(Segment { offset: 2, len: 3 }), None]);
        assert_eq!(ring, [0x11, 0x22, 0xd1, 0xd2, 0xd3, 0x66, 0x77, 0x88]);
        assert_eq!(report.status, SourceStatus::Success);
        assert_eq!(report.direction, TransferDirection::CallerToRecord);
        assert_eq!(
            state,
            RecordSnapshot {
                cursor_a: 5,
                cursor_b: 5,
                wrapped: false,
                derived_a: 0,
                derived_b: 8,
                record_to_caller_bytes: 0,
                caller_to_record_bytes: 3,
            }
        );

        let mut unwrapped_zero_ring = RING;
        let mut unwrapped_zero = snap(2, 5, false);
        let unwrapped_zero_report = session
            .transform_hd500x_caller_to_record(
                &mut unwrapped_zero,
                &mut unwrapped_zero_ring,
                &[],
                0,
                1,
            )
            .unwrap();
        assert_eq!(unwrapped_zero_report.segments, [None, None]);
        assert_eq!(unwrapped_zero_ring, RING);
        assert_eq!(
            unwrapped_zero,
            RecordSnapshot {
                cursor_a: 2,
                cursor_b: 5,
                wrapped: false,
                derived_a: 3,
                derived_b: 5,
                record_to_caller_bytes: 0,
                caller_to_record_bytes: 0,
            }
        );

        let mut wrapped_ring = RING;
        let mut wrapped = snap(6, 2, true);
        let wrapped_report = session
            .transform_hd500x_caller_to_record(
                &mut wrapped,
                &mut wrapped_ring,
                &[0xe1, 0xe2, 0xe3, 0xe4],
                4,
                1,
            )
            .unwrap();
        assert_eq!(
            wrapped_report.segments,
            [
                Some(Segment { offset: 6, len: 2 }),
                Some(Segment { offset: 0, len: 2 })
            ]
        );
        assert_eq!(
            wrapped_ring,
            [0xe3, 0xe4, 0x33, 0x44, 0x55, 0x66, 0xe1, 0xe2]
        );
        assert_eq!(
            wrapped,
            RecordSnapshot {
                cursor_a: 2,
                cursor_b: 2,
                wrapped: false,
                derived_a: 0,
                derived_b: 8,
                record_to_caller_bytes: 0,
                caller_to_record_bytes: 4,
            }
        );

        let mut full_ring = RING;
        let mut full = snap(0, 8, false);
        let full_source = [0xc1, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7, 0xc8];
        let full_report = session
            .transform_hd500x_caller_to_record(&mut full, &mut full_ring, &full_source, 8, 1)
            .unwrap();
        assert_eq!(
            full_report.segments,
            [Some(Segment { offset: 0, len: 8 }), None]
        );
        assert_eq!(full_ring, full_source);
        assert_eq!(full_report.requested_bytes, 8);
        assert_eq!(full_report.copied_bytes, 8);
        assert_eq!(full_report.status, SourceStatus::Success);
        assert_eq!(
            full,
            RecordSnapshot {
                cursor_a: 8,
                cursor_b: 8,
                wrapped: false,
                derived_a: 0,
                derived_b: 8,
                record_to_caller_bytes: 0,
                caller_to_record_bytes: 8,
            }
        );

        // Source-confirmed wrapped contiguous non-end branch: capacity is
        // included in derived_a, not derived_b.
        let mut wrapped_contiguous_ring = RING;
        let mut wrapped_contiguous = snap(4, 2, true);
        let wrapped_contiguous_report = session
            .transform_hd500x_caller_to_record(
                &mut wrapped_contiguous,
                &mut wrapped_contiguous_ring,
                &[0xd7],
                1,
                1,
            )
            .unwrap();
        assert_eq!(
            wrapped_contiguous_report.segments,
            [Some(Segment { offset: 4, len: 1 }), None]
        );
        assert_eq!(
            wrapped_contiguous_ring,
            [0x11, 0x22, 0x33, 0x44, 0xd7, 0x66, 0x77, 0x88]
        );
        assert_eq!(
            wrapped_contiguous,
            RecordSnapshot {
                cursor_a: 5,
                cursor_b: 2,
                wrapped: true,
                derived_a: 5,
                derived_b: 3,
                record_to_caller_bytes: 0,
                caller_to_record_bytes: 1,
            }
        );

        // A zero request in this same branch still refreshes the source's
        // derived fields, even though it copies no bytes.
        let mut wrapped_zero = snap(4, 2, true);
        let wrapped_zero_report = session
            .transform_hd500x_caller_to_record(&mut wrapped_zero, &mut full_ring, &[], 0, 1)
            .unwrap();
        assert_eq!(wrapped_zero_report.segments, [None, None]);
        assert_eq!(
            wrapped_zero,
            RecordSnapshot {
                cursor_a: 4,
                cursor_b: 2,
                wrapped: true,
                derived_a: 6,
                derived_b: 2,
                record_to_caller_bytes: 0,
                caller_to_record_bytes: 0,
            }
        );

        let mut wrapped_zero_at_base = snap(0, 0, true);
        let wrapped_zero_at_base_report = session
            .transform_hd500x_caller_to_record(&mut wrapped_zero_at_base, &mut full_ring, &[], 0, 1)
            .unwrap();
        assert_eq!(wrapped_zero_at_base_report.segments, [None, None]);
        assert_eq!(
            wrapped_zero_at_base,
            RecordSnapshot {
                cursor_a: 0,
                cursor_b: 0,
                wrapped: true,
                derived_a: 8,
                derived_b: 0,
                record_to_caller_bytes: 0,
                caller_to_record_bytes: 0,
            }
        );

        // Reaching capacity from a wrapped ring normalizes to base and takes
        // the source's end/crossing (unwrapped-derived) formula.
        let mut wrapped_end = snap(6, 2, true);
        let mut wrapped_end_ring = RING;
        let wrapped_end_report = session
            .transform_hd500x_caller_to_record(
                &mut wrapped_end,
                &mut wrapped_end_ring,
                &[0xe1, 0xe2],
                2,
                1,
            )
            .unwrap();
        assert_eq!(
            wrapped_end_report.segments,
            [Some(Segment { offset: 6, len: 2 }), None]
        );
        assert_eq!(
            wrapped_end_ring,
            [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0xe1, 0xe2]
        );
        assert_eq!(
            wrapped_end,
            RecordSnapshot {
                cursor_a: 0,
                cursor_b: 2,
                wrapped: false,
                derived_a: 2,
                derived_b: 6,
                record_to_caller_bytes: 0,
                caller_to_record_bytes: 2,
            }
        );

        // An unwrapped writer that lands exactly at E keeps the cursor at E
        // and uses the ordinary unwrapped formula.
        let mut unwrapped_end = snap(5, 8, false);
        let mut unwrapped_end_ring = RING;
        let unwrapped_end_report = session
            .transform_hd500x_caller_to_record(
                &mut unwrapped_end,
                &mut unwrapped_end_ring,
                &[0xf1, 0xf2, 0xf3],
                3,
                1,
            )
            .unwrap();
        assert_eq!(
            unwrapped_end_report.segments,
            [Some(Segment { offset: 5, len: 3 }), None]
        );
        assert_eq!(
            unwrapped_end_ring,
            [0x11, 0x22, 0x33, 0x44, 0x55, 0xf1, 0xf2, 0xf3]
        );
        assert_eq!(
            unwrapped_end,
            RecordSnapshot {
                cursor_a: 8,
                cursor_b: 8,
                wrapped: false,
                derived_a: 0,
                derived_b: 8,
                record_to_caller_bytes: 0,
                caller_to_record_bytes: 3,
            }
        );
    }

    #[test]
    fn short_write_status_two_retains_wrapped_second_span_without_cursor_commit() {
        let session = hd_session();
        let mut ring = RING;
        let mut state = snap(7, 2, true);
        state.derived_a = 71;
        state.derived_b = 72;
        let report = session
            .transform_hd500x_caller_to_record(
                &mut state,
                &mut ring,
                &[0xf1, 0xf2, 0xf3, 0xf4],
                4,
                1,
            )
            .unwrap();
        assert_eq!(report.segments, [None, Some(Segment { offset: 0, len: 2 })]);
        assert_eq!(ring, [0xf1, 0xf2, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88]);
        assert_eq!(report.requested_bytes, 4);
        assert_eq!(report.copied_bytes, 2);
        assert_eq!(report.status, SourceStatus::ShortWrite);
        assert!(!report.source_success);
        assert_eq!(state.cursor_a, 7);
        assert_eq!(state.cursor_b, 2);
        assert!(state.wrapped);
        assert_eq!(state.derived_a, 71);
        assert_eq!(state.derived_b, 72);
        assert_eq!(state.caller_to_record_bytes, 2);
    }

    #[test]
    fn zero_write_empty_order_and_counters_wrap_as_source_u64_addition() {
        let session = hd_session();
        let mut ring = RING;
        let mut empty_order = snap(4, 3, false);
        let report = session
            .transform_hd500x_caller_to_record(&mut empty_order, &mut ring, &[], 0, 1)
            .unwrap();
        assert_eq!(report.status, SourceStatus::Success);
        assert_eq!(report.copied_bytes, 0);
        assert_eq!(empty_order, snap(4, 3, false));

        let mut at_end = snap(8, 2, true);
        let normalized = session
            .transform_hd500x_caller_to_record(&mut at_end, &mut ring, &[], 0, 1)
            .unwrap();
        assert_eq!(normalized.status, SourceStatus::Success);
        assert_eq!(at_end.cursor_a, 0);
        assert_eq!(at_end.cursor_b, 2);
        assert!(!at_end.wrapped);
        assert_eq!(at_end.derived_a, 2);
        assert_eq!(at_end.derived_b, 6);

        let mut overflow = snap(2, 5, false);
        overflow.caller_to_record_bytes = u64::MAX;
        session
            .transform_hd500x_caller_to_record(&mut overflow, &mut ring, &[0x91], 1, 1)
            .unwrap();
        assert_eq!(overflow.caller_to_record_bytes, 0);

        let mut read_overflow = snap(2, 4, false);
        read_overflow.record_to_caller_bytes = u64::MAX;
        let mut output = [0; 1];
        session
            .transform_hd500x_record_to_caller(&mut read_overflow, &ring, &mut output, 1, 1)
            .unwrap();
        assert_eq!(read_overflow.record_to_caller_bytes, 0);
    }

    #[test]
    fn every_rust_admission_error_preserves_ring_caller_bytes_and_snapshot() {
        let session = hd_session();
        let mut state = snap(2, 5, false);
        let initial_state = state;
        let mut ring = RING;
        let initial_ring = ring;
        let mut caller = [0xa5; 8];
        let initial_caller = caller;
        assert_eq!(
            session.transform_hd500x_record_to_caller(&mut state, &ring, &mut caller, 1, 2),
            Err(TransformError::Admission(
                AdmissionError::UnsupportedPolicy { policy: 2 }
            ))
        );
        assert_eq!(
            session.transform_hd500x_record_to_caller(&mut state, &[], &mut caller, 0, 1),
            Err(TransformError::Admission(AdmissionError::EmptyRing {
                source_status: 4
            }))
        );
        assert_eq!(
            session.transform_hd500x_record_to_caller(&mut state, &ring, &mut caller, 9, 1),
            Err(TransformError::Admission(AdmissionError::RequestTooLarge {
                source_status: 3
            }))
        );
        let mut undersized = [0; 1];
        assert!(matches!(
            session.transform_hd500x_record_to_caller(&mut state, &ring, &mut undersized, 2, 1),
            Err(TransformError::Admission(
                AdmissionError::CallerBufferTooSmall { .. }
            ))
        ));
        let mut undersized_write_ring = RING;
        let undersized_write_ring_before = undersized_write_ring;
        assert!(matches!(
            session.transform_hd500x_caller_to_record(
                &mut state,
                &mut undersized_write_ring,
                &[0xd1],
                2,
                1
            ),
            Err(TransformError::Admission(
                AdmissionError::CallerBufferTooSmall { .. }
            ))
        ));
        let mut bad_order = snap(4, 3, false);
        let bad_order_before = bad_order;
        assert_eq!(
            session.transform_hd500x_record_to_caller(&mut bad_order, &ring, &mut caller, 1, 1),
            Err(TransformError::Admission(
                AdmissionError::InvalidReadOrdering { source_status: 4 }
            ))
        );
        let mut bad_cursor = snap(9, 3, false);
        let bad_cursor_before = bad_cursor;
        assert!(matches!(
            session.transform_hd500x_caller_to_record(&mut bad_cursor, &mut ring, &[0x77], 1, 1),
            Err(TransformError::Admission(
                AdmissionError::CursorOutsideRing {
                    source_status: 4,
                    ..
                }
            ))
        ));
        assert_eq!(state, initial_state);
        assert_eq!(bad_order, bad_order_before);
        assert_eq!(bad_cursor, bad_cursor_before);
        assert_eq!(ring, initial_ring);
        assert_eq!(undersized_write_ring, undersized_write_ring_before);
        assert_eq!(caller, initial_caller);
    }

    #[test]
    fn transform_is_profile_gated_and_never_changes_physical_capability() {
        let mut initial_state = snap(2, 5, false);
        let original_state = initial_state;
        let mut dst = [0xa0; 8];
        assert!(matches!(
            Session::initial_rig().transform_hd500x_record_to_caller(
                &mut initial_state,
                &RING,
                &mut dst,
                1,
                1
            ),
            Err(TransformError::Profile(Rejection::Unsupported { .. }))
        ));
        assert_eq!(initial_state, original_state);

        for profile in [ProductProfile::WorkbenchHd, ProductProfile::Jtv69] {
            let mut state = snap(2, 5, false);
            assert!(matches!(
                Session::extension(profile)
                    .unwrap()
                    .transform_hd500x_record_to_caller(&mut state, &RING, &mut dst, 1, 1),
                Err(TransformError::Profile(Rejection::Unverified { .. }))
            ));
            assert_eq!(state, snap(2, 5, false));
        }
        let session = hd_session();
        assert_eq!(
            session.reject_extension_physical(
                Operation::OpenAudioStream,
                Some(CapabilityState::Offline),
                || Ok(())
            ),
            Err(Rejection::Locked {
                operation: Operation::OpenAudioStream
            })
        );
        assert_eq!(
            crate::rig::capability::capability(
                ProductProfile::PodHd500x,
                Operation::TransformOfflineRecord
            )
            .state,
            CapabilityState::Offline
        );
    }
}
