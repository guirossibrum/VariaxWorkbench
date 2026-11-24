#![forbid(unsafe_code)]
//! Common regular-file and bounded input helpers.
use std::io::{self, Read};
use std::path::Path;
#[cfg(test)]
use {std::cell::Cell, std::rc::Rc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToneError {
    Format { detail: String },
    WrongTarget { device_id: u32 },
}

/// Facts from [`std::fs::metadata`]. That call does not open the path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PathFacts {
    pub regular_file: bool,
    pub len: u64,
}

pub fn path_facts(path: &Path) -> std::io::Result<PathFacts> {
    let meta = std::fs::metadata(path)?;
    Ok(PathFacts {
        regular_file: meta.is_file(),
        len: meta.len(),
    })
}

/// Opens an input only when the opened handle itself names a regular file.
/// Callers also check the path facts first so non-file paths are not opened.
pub fn open_regular_file(path: &Path) -> std::io::Result<std::fs::File> {
    let file = std::fs::File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "input must be a regular file",
        ));
    }
    Ok(file)
}

pub(crate) fn read_bounded_file<R: Read>(
    mut reader: R,
    ceiling: u64,
    too_large: &str,
) -> Result<Vec<u8>, ToneError> {
    let capacity = usize::try_from(
        ceiling
            .checked_add(1)
            .ok_or_else(|| ToneError::format("offline file-size ceiling overflow"))?,
    )
    .map_err(|_| ToneError::format("offline file-size ceiling is not addressable"))?;
    let mut bytes = Vec::with_capacity(capacity);
    let mut buffer = [0u8; 8192];
    loop {
        let remaining = capacity - bytes.len();
        if remaining == 0 {
            break;
        }
        let read_len = buffer.len().min(remaining);
        let count = reader
            .read(&mut buffer[..read_len])
            .map_err(|error| ToneError::format(error.to_string()))?;
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
    if bytes.len() as u64 > ceiling {
        return Err(ToneError::format(too_large));
    }
    Ok(bytes)
}

#[cfg(test)]
pub(crate) struct ContinuousReader(pub(crate) Rc<Cell<usize>>);

#[cfg(test)]
impl Read for ContinuousReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        buffer.fill(0x5a);
        self.0.set(self.0.get() + buffer.len());
        Ok(buffer.len())
    }
}
