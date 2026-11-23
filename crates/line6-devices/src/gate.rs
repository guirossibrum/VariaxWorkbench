//! Device operations and the caller's policy for them.
//!
//! The crate names operations; the application decides which are allowed.
//! The main app maps them onto its compile-time hardware gates; the probe
//! tool uses the owner's grants recorded in its own source.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    /// Universal identity request to a Variax through the PODxt Live.
    VariaxIdentity,
    /// Variax current-preset metadata read: session on, two blocks, session off.
    VariaxRead,
    /// Universal identity request to a PODxt-family device.
    PodxtIdentity,
    /// PODxt-family current tone (edit buffer) dump request.
    PodxtRead,
}

pub trait GatePolicy {
    fn allows(&self, operation: Operation) -> bool;
}

/// Allows nothing. Useful default and test policy.
pub struct AllClosed;

impl GatePolicy for AllClosed {
    fn allows(&self, _: Operation) -> bool {
        false
    }
}
