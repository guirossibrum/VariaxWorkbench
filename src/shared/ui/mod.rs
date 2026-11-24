//! Theme and controls shared by the shell and the family editors.
//!
//! The knob follows the approved specification: drag is vertical, and
//! double-click or Enter edits the value as text. Shift drags in finer steps.
//! There is no double-click reset.
//!
//! #219 draws an editor with [`chrome::section_panel`], whose heading sits on
//! the inner margin, and places wide Mod and Reverb skins with
//! [`chrome::chain_slot`] so a rack image is not fitted into a stomp-sized box.

#![forbid(unsafe_code)]

pub mod assets;
pub mod chrome;
pub mod controls;
pub mod gallery;
pub mod knob;
pub mod theme;

#[cfg(test)]
mod tests;
