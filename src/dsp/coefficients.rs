//! Block coefficient writers. The block's DSP math is a separate stage.
use super::{NativePatch, signed_q23};
impl NativePatch {
    pub(super) fn apply_block_coefficients(
        &mut self,
        name: &str,
        args: &[u32],
    ) -> Result<(), String> {
        let mut writes = Vec::new();
        macro_rules! q {
            ($offset:expr,$value:expr) => {
                writes.push(($offset / 4, signed_q23($value)?))
            };
        }
        let f = |v: u32| f32::from_bits(v);
        let index = |v: u32, limit: u32| {
            if v < limit {
                Ok(v as usize)
            } else {
                Err("coefficient index exceeds block bounds".to_string())
            }
        };
        match (name, args) {
            ("body-resonator", [i, a, b, c]) => {
                let offset = 0x294 + index(*i, 12)? * 12;
                q!(offset, f(*a) * 0.5);
                q!(offset + 4, f(*c) * 0.5);
                q!(offset + 8, f(*b) * 0.5);
            }
            ("body-treble", [a, b, c, d]) => {
                q!(0x27c, f(*a) * 0.5);
                q!(0x280, f(*b) * 0.5);
                q!(0x288, f(*c) * 0.5);
                q!(0x28c, f(*d) * 0.5);
            }
            ("body-mix", [a, b]) => {
                q!(0x270, f(*a));
                q!(0x274, f(*b));
            }
            ("low-cut", [a, b]) => {
                q!(0x328, f(*b) * 0.5);
                q!(0x32c, f(*a) * 0.5);
            }
            ("string-eq", [i, a, b, c, d, e]) | ("twelve-eq", [i, a, b, c, d, e]) => {
                let (base, limit) = if name == "string-eq" {
                    (0x5a0, 36)
                } else {
                    (0xad0, 12)
                };
                let offset = base + index(*i, limit)? * 20;
                for (j, v) in [c, d, e, a, b].into_iter().enumerate() {
                    q!(offset + j * 4, f(*v) * 0.25);
                }
            }
            ("string-eq-gain", [i, v]) => {
                q!(0x570 + index(*i, 6)? * 4, f(*v));
            }
            ("twelve-wet-dry", [i, a, b]) => {
                let i = index(*i, 6)?;
                q!(0xbc0 + i * 4, f(*b));
                q!(0xbd8 + i * 4, f(*a));
            }
            ("string-tone", [i, count, a, b]) => {
                let i = index(*i, 6)?;
                if *count > 6 {
                    return Err("string-tone resonator count exceeds six".into());
                }
                writes.push((0x330 / 4 + i, *count));
                q!(0x4f8 + i * 4, f(*a));
                q!(0x510 + i * 4, f(*b));
            }
            ("string-tone-resonator", [string, resonator, a, b, c]) => {
                let offset = 0x348 + (index(*string, 6)? * 6 + index(*resonator, 6)?) * 12;
                q!(offset, f(*c) * 0.5);
                q!(offset + 4, f(*a) * 0.5);
                q!(offset + 8, f(*b) * 0.5);
            }
            ("global-decay", [i, a, b, c]) => {
                let i = index(*i, 6)?;
                q!(0x528 + i * 4, f(*a));
                q!(0x540 + i * 4, f(*b));
                q!(0x558 + i * 4, f(*c));
            }
            ("drone-string", [i, period, v]) => {
                let i = index(*i, 12)?;
                writes.push((0x930 / 4 + i, *period));
                q!(0x960 + i * 4, f(*v));
            }
            ("sitar-string", [i, a, b, c, d, e]) => {
                let i = index(*i, 6)?;
                for (j, v) in [a, b, c, d, e].into_iter().enumerate() {
                    q!(0x8b0 + j * 24 + i * 4, f(*v));
                }
            }
            ("pitch-string", [i, value, delay]) => {
                let i = index(*i, 6)?;
                q!(0xaa0 + i * 4, f(*value));
                writes.push((0xab8 / 4 + i, *delay));
            }
            ("pick-filter", [a, b, c]) => {
                q!(0x94, f(*b) * 0.25);
                q!(0x98, f(*a) * 0.25);
                q!(0xc0, f(*c));
                writes.push((0xc4 / 4, 0));
            }
            ("compressor", [a, b]) => {
                q!(0xec, f(*a));
                q!(0xf0, f(*b));
            }
            _ => return Err("unknown coefficient opcode or wrong arguments".into()),
        }
        for (index, value) in writes {
            self.words[index] = value;
        }
        Ok(())
    }
}
