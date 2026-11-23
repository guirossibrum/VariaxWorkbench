//! Line 6 device families: what each one understands, how to reach it, and
//! how to decode what it sends back.
//!
//! * [`family`] — one module per product family. Each owns its allowed
//!   requests ([`session::Request`]), its procedures and its decoders. A new
//!   family is a new module plus a [`family::Family`] entry.
//! * [`transport`] — thin, replaceable links: a USB interrupt-interface link
//!   (feature `usb`) and the Linux ALSA rawmidi link.
//! * [`session`] — the only path from a request to a device. It refuses a
//!   request for the wrong link and any request whose [`gate::Operation`] the
//!   caller's [`gate::GatePolicy`] does not allow. This crate never decides
//!   that a gate is open.
//! * [`midi`], [`identity`] — shared byte-level pieces.
//! * [`backup`] — recoverable library backups on disk (write, verify, load,
//!   compare), so every front end produces the same layout.
//!
//! No request writes, stores, resets, or touches firmware. The Variax library
//! backup is the only procedure that sends program changes: it switches
//! through the stored presets to read them, then tries to switch back. The
//! caller's policy decides whether that procedure is allowed.

#![forbid(unsafe_code)]

pub mod backup;
pub mod family;
pub mod gate;
pub mod identity;
pub mod midi;
pub mod session;
pub mod transport;
