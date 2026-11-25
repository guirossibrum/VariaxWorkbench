#![forbid(unsafe_code)]
//! PODxt Live tone and bank files.
use crate::shared::document;
use crate::shared::file::{PathFacts, ToneError};
use crate::shared::iff as preset;
use std::io::Read;
use std::{ops::Range, path::Path};

mod bank;
mod controls;
mod edit;
mod iff_edit;
mod library;
mod metadata;
#[cfg(any(feature = "podxt-table", test))]
mod parameters;
#[cfg(feature = "gui")]
mod startup;
#[cfg(test)]
pub(crate) mod tests;
mod tone;

pub use bank::*;
pub use controls::*;
pub use library::*;
pub use metadata::*;
#[cfg(any(feature = "podxt-table", test))]
pub use parameters::*;
#[cfg(feature = "gui")]
pub use startup::*;
pub use tone::*;
