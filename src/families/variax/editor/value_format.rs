//! Display and entry formats from Workbench 1.75's parameter definitions.

#[derive(Clone, Copy)]
pub(super) enum ValueFormat {
    Decimal {
        suffix: &'static str,
        decimals: usize,
        signed: bool,
    },
    Percent(f32),
}

impl ValueFormat {
    pub(super) const POSITION: Self = Self::decimal("\"", 2, false);
    pub(super) const ANGLE: Self = Self::decimal("°", 1, false);
    pub(super) const LEVEL: Self = Self::decimal("dB", 1, true);
    pub(super) const TRIM: Self = Self::decimal("dB", 2, true);
    pub(super) const CENTS: Self = Self::decimal(" cents", 2, true);
    pub(super) const SEMITONES: Self = Self::decimal(" semitones", 0, true);

    const fn decimal(suffix: &'static str, decimals: usize, signed: bool) -> Self {
        Self::Decimal {
            suffix,
            decimals,
            signed,
        }
    }

    pub(super) fn display(self, stored: f64) -> String {
        match self {
            Self::Decimal {
                suffix,
                decimals,
                signed,
            } => {
                let scale = 10_f64.powi(decimals as i32);
                let rounded = (stored * scale).round() / scale + 0.0;
                let sign = if signed && rounded > 0.0 { "+" } else { "" };
                format!("{sign}{rounded:.decimals$}{suffix}")
            }
            Self::Percent(max) => {
                // L6FormatPercent normalizes in f32 and truncates to a whole percent.
                let percent = (stored as f32 / max).clamp(0.0, 1.0) * 100.0;
                format!("{:.0}%", percent.trunc())
            }
        }
    }

    pub(super) fn parse_edit(self, text: &str, stored: f64) -> Option<f64> {
        let text = text.trim();
        // egui can submit the initial text on blur. Keep any precision hidden by the display.
        if text == self.display(stored) {
            return None;
        }
        let suffix = match self {
            Self::Decimal { suffix, .. } => suffix.trim_start(),
            Self::Percent(_) => "%",
        };
        let number = text.strip_suffix(suffix).unwrap_or(text).trim_end();
        let number: f64 = number.replace('−', "-").parse().ok()?;
        if !number.is_finite() {
            return None;
        }
        Some(match self {
            Self::Decimal { decimals: 0, .. } => number.round(),
            Self::Decimal { .. } => number,
            Self::Percent(max) => f64::from((number / 100.0) as f32 * max),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::ValueFormat as Format;

    #[test]
    fn workbench_formats_display_units_and_parse_edits_without_quantizing_on_blur() {
        for (format, stored, shown, typed, expected) in [
            (Format::POSITION, 2.756, "2.76\"", "3.125\"", 3.125),
            (Format::ANGLE, -12.54, "-12.5°", " −8.25 ° ", -8.25),
            (Format::LEVEL, 2.25, "+2.3dB", "+1.25 dB", 1.25),
            (Format::TRIM, 3.125, "+3.13dB", "-5.125dB", -5.125),
            (Format::CENTS, -0.001, "0.00 cents", "+12.25 cents", 12.25),
            (Format::SEMITONES, 2.0, "+2 semitones", "-3 semitones", -3.0),
            (Format::Percent(1.0), 0.625, "62%", "75%", 0.75),
            (Format::Percent(25.0), 12.5, "50%", "25%", 6.25),
        ] {
            assert_eq!(format.display(stored), shown);
            assert_eq!(format.parse_edit(shown, stored), None);
            assert_eq!(format.parse_edit(typed, stored), Some(expected));
        }
        assert_eq!(Format::Percent(1.0).parse_edit("25", 0.0), Some(0.25));
        assert_eq!(Format::TRIM.parse_edit("NaN dB", 0.0), None);
        assert_eq!(Format::TRIM.parse_edit("1 cents", 0.0), None);
    }
}
