//! Synthetic logical POD bridge and Variax metadata session. The identities and
//! metadata here are scenario inputs, not copies of a unit's saved identity.

use crate::read_session::{ControlTransport, IDENTITY_REQUEST, MIDI_DISABLE};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VirtualSessionState {
    Idle,
    Identified,
    Active { next_block: usize },
    Disabled,
    Disconnected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlFault {
    None,
    DropMetadata(usize),
    DuplicateFirstMetadata,
    DisconnectBeforeBlock(usize),
    CancelBeforeBlock(usize),
    FailDisable,
}

/// A virtual POD logical interface routes only supported metadata operations to
/// its virtual guitar. All POD editing/audio/update operations remain unsupported.
/// This type stores vectors and counters only; there is no passthrough backend.
pub struct VirtualControl {
    metadata: [u8; 192],
    state: VirtualSessionState,
    queued: VecDeque<Vec<u8>>,
    sent: Vec<Vec<u8>>,
    fault: ControlFault,
    pending_cancel: bool,
    fragment_bytes: usize,
    realtime: bool,
    product: u32,
    virtual_pause_ms: u64,
    disable_attempts: usize,
}

impl VirtualControl {
    pub fn new(metadata: [u8; 192]) -> Result<Self, String> {
        crate::PresetMetadata::from_bytes(metadata)?;
        Ok(Self {
            metadata,
            state: VirtualSessionState::Idle,
            queued: VecDeque::new(),
            sent: Vec::new(),
            fault: ControlFault::None,
            pending_cancel: false,
            fragment_bytes: 32,
            realtime: false,
            product: 0x0007_0004,
            virtual_pause_ms: 0,
            disable_attempts: 0,
        })
    }

    pub fn with_fault(mut self, fault: ControlFault) -> Result<Self, String> {
        if matches!(fault, ControlFault::DropMetadata(index) | ControlFault::DisconnectBeforeBlock(index) | ControlFault::CancelBeforeBlock(index) if index > 1)
        {
            return Err("virtual metadata block must be 0 or 1".into());
        }
        self.fault = fault;
        Ok(self)
    }

    pub fn with_fragmentation(mut self, bytes: usize, realtime: bool) -> Result<Self, String> {
        if !(1..=256).contains(&bytes) {
            return Err("virtual fragment bound is 1..=256".into());
        }
        self.fragment_bytes = bytes;
        self.realtime = realtime;
        Ok(self)
    }

    /// Explicit synthetic wrong-product scenario, not a device discovery result.
    pub fn with_declared_product(mut self, product: u32) -> Result<Self, String> {
        if product >> 16 > 127 || product & 0xffff > 127 {
            return Err("synthetic identity fields must fit the modeled low bytes".into());
        }
        self.product = product;
        Ok(self)
    }

    pub const fn state(&self) -> VirtualSessionState {
        self.state
    }

    pub fn sent_messages(&self) -> &[Vec<u8>] {
        &self.sent
    }

    pub const fn virtual_pause_ms(&self) -> u64 {
        self.virtual_pause_ms
    }

    pub const fn disable_attempts(&self) -> usize {
        self.disable_attempts
    }

    /// Logical interface number from the static registry, not a USB open.
    pub fn route(&mut self, logical_interface: u8, message: &[u8]) -> Result<(), String> {
        if logical_interface != 1 {
            return Err("only the virtual guitar role on logical interface 1 is modeled".into());
        }
        if message == MIDI_DISABLE {
            self.disable_attempts = self.disable_attempts.saturating_add(1);
        }
        if self.state == VirtualSessionState::Disconnected {
            return Err("virtual bridge is disconnected".into());
        }
        if self.sent.len() >= 32 || !crate::read_session::is_read_session_message(message) {
            return Err("virtual control operation is unsupported or exceeds its bound".into());
        }
        self.sent.push(message.to_vec());
        if message == IDENTITY_REQUEST {
            if !matches!(
                self.state,
                VirtualSessionState::Idle | VirtualSessionState::Disabled
            ) {
                return Err("virtual identity request is out of order".into());
            }
            // Revision ASCII 0001 deliberately denotes synthetic 0.01. It is
            // not the saved 3.10/3.01 identity of either owner's device.
            let mut reply = vec![0xf0, 0x7e, 0x7f, 6, 2, 0, 1, 12];
            reply.extend([
                ((self.product >> 16) & 127) as u8,
                0,
                (self.product & 127) as u8,
                0,
            ]);
            reply.extend_from_slice(b"0001");
            reply.push(0xf7);
            self.queued.push_back(reply);
            self.state = VirtualSessionState::Identified;
            return Ok(());
        }
        if message == MIDI_DISABLE {
            if self.fault == ControlFault::FailDisable {
                return Err("virtual disable failed".into());
            }
            self.state = VirtualSessionState::Disabled;
            return Ok(());
        }
        if message == crate::midi_enable() {
            if self.state != VirtualSessionState::Identified {
                return Err("virtual activation is out of order".into());
            }
            self.state = VirtualSessionState::Active { next_block: 0 };
            return Ok(());
        }
        let requests = crate::metadata_requests();
        let VirtualSessionState::Active { next_block } = self.state else {
            return Err("virtual metadata request requires an active session".into());
        };
        if next_block > 1 || message != requests[next_block] {
            return Err("virtual metadata requests must use the recovered block order".into());
        }
        if self.fault == ControlFault::DisconnectBeforeBlock(next_block) {
            self.state = VirtualSessionState::Disconnected;
            return Err("virtual disconnect".into());
        }
        self.state = VirtualSessionState::Active {
            next_block: next_block + 1,
        };
        if self.fault == ControlFault::CancelBeforeBlock(next_block) {
            self.pending_cancel = true;
        }
        if self.fault != ControlFault::DropMetadata(next_block) {
            let reply = self.metadata_reply(next_block);
            self.queued.push_back(reply.clone());
            if next_block == 0 && self.fault == ControlFault::DuplicateFirstMetadata {
                self.queued.push_back(reply);
            }
        }
        Ok(())
    }

    fn metadata_reply(&self, block: usize) -> Vec<u8> {
        let mut out = vec![0xf0, 0, 1, 12, 7, 4, 0x3c];
        // Independent byte/word traversal rather than using the client encoder.
        for word in self.metadata[block * 96..(block + 1) * 96]
            .as_chunks::<3>()
            .0
        {
            out.extend([
                word[2] & 15,
                word[2] >> 4,
                word[1] & 15,
                word[1] >> 4,
                word[0] & 15,
                word[0] >> 4,
            ]);
        }
        out.push(0xf7);
        out
    }
}

impl ControlTransport for VirtualControl {
    fn send(&mut self, message: &[u8]) -> Result<(), String> {
        self.route(1, message)
    }

    fn next_message(
        &mut self,
        cancel: &AtomicBool,
        _: Duration,
    ) -> Result<Option<Vec<u8>>, String> {
        if self.pending_cancel {
            cancel.store(true, Ordering::Relaxed);
            self.pending_cancel = false;
        }
        if cancel.load(Ordering::Relaxed) {
            return Err("virtual read cancelled".into());
        }
        if self.state == VirtualSessionState::Disconnected {
            return Err("virtual bridge disconnected".into());
        }
        let Some(message) = self.queued.pop_front() else {
            return Ok(None);
        };
        let mut decoder = crate::midi::SysexDecoder::default();
        let mut decoded = Vec::new();
        for part in message.chunks(self.fragment_bytes) {
            if self.realtime {
                decoded.extend(decoder.feed(&[0xf8])?);
            }
            decoded.extend(decoder.feed(part)?);
        }
        if decoded.len() != 1 {
            return Err("virtual fragmentation did not yield exactly one message".into());
        }
        Ok(decoded.pop())
    }

    fn pause(&mut self, duration: Duration) {
        let elapsed = u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
        self.virtual_pause_ms = self.virtual_pause_ms.saturating_add(elapsed);
    }
}
