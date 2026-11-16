//! File-only independent-decoder fixture generator; never opens audio/devices.
use std::{error::Error, path::PathBuf};
use variax_workbench::{document::save_new, recording::aiff};

fn main() -> Result<(), Box<dyn Error>> {
    let dir = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("new output directory required")?,
    );
    std::fs::create_dir(&dir)?;
    for frames in [0usize, 1, 11, 32, 64, 256, 1025, 8192] {
        let mut source = Vec::new();
        for sample in 0..frames * 2 {
            let value = match sample % 8 {
                0 => 0,
                1 => 1,
                2 => 0xffffff,
                3 => 0x7fffff,
                4 => 0x800000,
                _ => {
                    (sample as u32)
                        .wrapping_mul(1_103_515_245)
                        .wrapping_add(12_345)
                        & 0xffffff
                }
            };
            source.extend_from_slice(&value.to_le_bytes()[..3]);
        }
        let file = aiff::encode(&source)?;
        save_new(&dir.join(format!("{frames}.aiff")), &file)?;
        save_new(&dir.join(format!("{frames}.source.pcm")), &source)?;
    }
    Ok(())
}
