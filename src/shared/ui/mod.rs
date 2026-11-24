//! Theme and controls shared by the shell and the family editors.
//!
//! The knob follows the approved specification: drag is vertical, and
//! double-click or Enter edits the value as text. Shift drags in finer steps.
//! There is no double-click reset.
//!
//! #219 draws an editor with [`chrome::section_panel`], whose heading sits on
//! the inner margin, and places wide Mod and Reverb skins with
//! [`chrome::chain_slot`] so a rack image is not fitted into a stomp-sized box.
//! #224 and #188–#191 compose [`body_canvas`] and [`tuning_panel`]. The caller
//! owns the pickup and tuning values; those widgets only report gestures.

#![forbid(unsafe_code)]

pub mod assets;
pub mod body_canvas;
pub mod chrome;
pub mod controls;
pub mod gallery;
pub mod knob;
pub mod theme;
pub mod tuning_panel;

#[cfg(test)]
mod tests;
