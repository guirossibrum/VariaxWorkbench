//! Offline comparison of recordings across computers (#266).
//!
//! Loads two clips of the same test model (for example from a Linux and a
//! Windows report's evidence folders) and compares level, spectrum, the pitch
//! near each open string and the onset time. With two takes from the same
//! computer as a baseline, it says whether the differences exceed the
//! take-to-take variation; without one it uses fixed tolerances and says so.
//! Engine-check lines (step 0.3) from both reports are shown next to the
//! result so a pitch difference can be linked to differing pitch/tuning
//! words. No audio device is used.

use crate::recording::parse_wav;

/// Open strings in standard tuning, low to high.
pub const STRINGS: [(&str, f64); 6] = [
    ("E2", 82.41),
    ("A2", 110.00),
    ("D3", 146.83),
    ("G3", 196.00),
    ("B3", 246.94),
    ("E4", 329.63),
];
/// One-third-octave band centres for the spectrum summary.
pub const BANDS: [f64; 10] = [
    63.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 6300.0, 8000.0, 10000.0,
];

#[derive(Clone, Debug)]
pub struct Analysis {
    pub rate: u32,
    pub seconds: f64,
    pub peak_dbfs: f64,
    pub rms_dbfs: f64,
    pub bands_db: Vec<f64>,
    /// Strongest frequency within ±60 cents of each open string, and its
    /// offset in cents, if it stands out from the noise.
    pub strings: Vec<Option<(f64, f64)>>,
    pub onset_s: Option<f64>,
}

fn db(value: f64) -> f64 {
    20.0 * value.max(1e-12).log10()
}

/// Power of one frequency over the samples (Goertzel), as amplitude.
fn goertzel(samples: &[f64], rate: f64, frequency: f64) -> f64 {
    let w = 2.0 * std::f64::consts::PI * frequency / rate;
    let coeff = 2.0 * w.cos();
    let (mut s1, mut s2) = (0.0, 0.0);
    for &x in samples {
        let s0 = x + coeff * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    let power = s1 * s1 + s2 * s2 - coeff * s1 * s2;
    2.0 * power.max(0.0).sqrt() / samples.len().max(1) as f64
}

pub fn analyze(samples: &[f64], rate: u32) -> Analysis {
    let fs = f64::from(rate);
    let peak = samples.iter().fold(0.0f64, |m, x| m.max(x.abs()));
    let rms = (samples.iter().map(|x| x * x).sum::<f64>() / samples.len().max(1) as f64).sqrt();
    // Onset: first 5 ms window whose RMS reaches 1/10 of the loudest window.
    let window = ((fs * 0.005) as usize).max(1);
    let envelope: Vec<f64> = samples
        .chunks(window)
        .map(|c| (c.iter().map(|x| x * x).sum::<f64>() / c.len() as f64).sqrt())
        .collect();
    let loudest = envelope.iter().cloned().fold(0.0, f64::max);
    let onset_index = envelope
        .iter()
        .position(|&e| loudest > 0.0 && e >= loudest * 0.1);
    let onset_s = onset_index.map(|i| (i * window) as f64 / fs);
    // Spectrum and pitch over up to one second after the onset.
    let start = onset_index.map_or(0, |i| i * window);
    let segment = &samples[start..samples.len().min(start + rate as usize)];
    let bands_db = BANDS
        .iter()
        .map(|&centre| {
            if centre * 2.0 >= fs {
                return f64::NEG_INFINITY;
            }
            let probes = [centre * 0.891, centre, centre * 1.122];
            let mean = probes
                .iter()
                .map(|&f| goertzel(segment, fs, f))
                .sum::<f64>()
                / 3.0;
            db(mean)
        })
        .collect();
    let floor = db(rms) - 40.0;
    let strings = STRINGS
        .iter()
        .map(|&(_, expected)| {
            let mut best = (expected, 0.0f64);
            let mut cents = -60.0;
            while cents <= 60.0 {
                let f = expected * 2f64.powf(cents / 1200.0);
                let a = goertzel(segment, fs, f);
                if a > best.1 {
                    best = (f, a);
                }
                cents += 1.0;
            }
            (db(best.1) > floor).then(|| (best.0, 1200.0 * (best.0 / expected).log2()))
        })
        .collect();
    Analysis {
        rate,
        seconds: samples.len() as f64 / fs,
        peak_dbfs: db(peak),
        rms_dbfs: db(rms),
        bands_db,
        strings,
        onset_s,
    }
}

/// Analyse a 16-bit stereo WAV (the recorder's format); channels are mixed.
pub fn analyze_wav(bytes: &[u8]) -> Result<Analysis, String> {
    let wav = parse_wav(bytes).map_err(|e| format!("{e:?}"))?;
    let samples: Vec<f64> = wav
        .data
        .as_chunks::<4>()
        .0
        .iter()
        .map(|frame| {
            let l = i16::from_le_bytes([frame[0], frame[1]]) as f64;
            let r = i16::from_le_bytes([frame[2], frame[3]]) as f64;
            (l + r) / 2.0 / 32768.0
        })
        .collect();
    if samples.is_empty() {
        return Err("The clip has no audio frames.".into());
    }
    Ok(analyze(&samples, wav.sample_rate_hz))
}

/// The differences between two analyses, one per measure.
#[derive(Clone, Debug, PartialEq)]
pub struct Deltas {
    pub peak_db: f64,
    pub rms_db: f64,
    pub band_db: f64,
    pub cents: f64,
    pub onset_ms: f64,
}

pub fn deltas(a: &Analysis, b: &Analysis) -> Deltas {
    let band_db = a
        .bands_db
        .iter()
        .zip(&b.bands_db)
        .filter(|(x, y)| x.is_finite() && y.is_finite())
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f64::max);
    let cents = a
        .strings
        .iter()
        .zip(&b.strings)
        .filter_map(|(x, y)| Some((x.as_ref()?.1 - y.as_ref()?.1).abs()))
        .fold(0.0, f64::max);
    let onset_ms = match (a.onset_s, b.onset_s) {
        (Some(x), Some(y)) => (x - y).abs() * 1000.0,
        _ => 0.0,
    };
    Deltas {
        peak_db: (a.peak_dbfs - b.peak_dbfs).abs(),
        rms_db: (a.rms_dbfs - b.rms_dbfs).abs(),
        band_db,
        cents,
        onset_ms,
    }
}

/// Fixed tolerances used when no same-computer baseline is given.
pub const FIXED: Deltas = Deltas {
    peak_db: 1.5,
    rms_db: 1.0,
    band_db: 3.0,
    cents: 5.0,
    onset_ms: 20.0,
};

pub struct Comparison {
    pub lines: Vec<String>,
    /// Measures whose difference exceeds the tolerance.
    pub exceeded: Vec<&'static str>,
}

/// Compare clip `a` and clip `b`. `baseline` is two takes from one computer.
pub fn compare(
    a: &Analysis,
    b: &Analysis,
    baseline: Option<(&Analysis, &Analysis)>,
    engine_a: &[String],
    engine_b: &[String],
) -> Comparison {
    let d = deltas(a, b);
    let (tolerance, source) = match baseline {
        Some((x, y)) => {
            let base = deltas(x, y);
            (
                Deltas {
                    peak_db: (base.peak_db * 2.0).max(0.5),
                    rms_db: (base.rms_db * 2.0).max(0.5),
                    band_db: (base.band_db * 2.0).max(1.0),
                    cents: (base.cents * 2.0).max(2.0),
                    onset_ms: (base.onset_ms * 2.0).max(5.0),
                },
                "twice the take-to-take variation on one computer",
            )
        }
        None => (FIXED, "fixed tolerances (no same-computer baseline given)"),
    };
    let mut lines = vec![format!("Tolerance: {source}.")];
    let mut exceeded = Vec::new();
    for (name, diff, tol, unit) in [
        ("peak level", d.peak_db, tolerance.peak_db, "dB"),
        ("RMS level", d.rms_db, tolerance.rms_db, "dB"),
        (
            "spectrum (largest band difference)",
            d.band_db,
            tolerance.band_db,
            "dB",
        ),
        (
            "string pitch (largest difference)",
            d.cents,
            tolerance.cents,
            "cents",
        ),
        ("onset time", d.onset_ms, tolerance.onset_ms, "ms"),
    ] {
        let over = diff > tol;
        if over {
            exceeded.push(name);
        }
        lines.push(format!(
            "{name}: difference {diff:.2} {unit} (tolerance {tol:.2}) {}",
            if over { "EXCEEDS" } else { "within" }
        ));
    }
    for (i, (string, expected)) in STRINGS.iter().enumerate() {
        let show = |s: &Option<(f64, f64)>| {
            s.map_or("not found".to_string(), |(f, c)| {
                format!("{f:.2} Hz ({c:+.1} cents)")
            })
        };
        lines.push(format!(
            "  {string} ({expected} Hz): A {} · B {}",
            show(&a.strings[i]),
            show(&b.strings[i])
        ));
    }
    let tuning_words = |lines: &[String]| lines.iter().any(|l| l.contains("pitch/tuning"));
    if !engine_a.is_empty() || !engine_b.is_empty() {
        lines.push("Engine check (step 0.3) A:".into());
        lines.extend(engine_a.iter().map(|l| format!("  {l}")));
        lines.push("Engine check (step 0.3) B:".into());
        lines.extend(engine_b.iter().map(|l| format!("  {l}")));
        if exceeded.contains(&"string pitch (largest difference)")
            && (tuning_words(engine_a) || tuning_words(engine_b))
        {
            lines.push("The pitch difference matches an engine-check difference in the pitch/tuning words (680-691).".into());
        }
    }
    Comparison { lines, exceeded }
}

/// The engine-check result lines of a saved report (step 0.3).
pub fn engine_lines(report: &str) -> Vec<String> {
    let Some(start) = report.find("\n[0.3] ") else {
        return Vec::new();
    };
    let section = &report[start + 1..];
    let end = section[1..].find("\n[").map_or(section.len(), |i| i + 1);
    section[..end]
        .lines()
        .skip_while(|l| l.trim() != "Results:")
        .skip(1)
        .take_while(|l| l.starts_with("    "))
        .map(|l| l.trim().to_owned())
        .filter(|l| !l.starts_with("word "))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recording::write_wav;

    const RATE: u32 = 48_000;

    /// Six decaying open strings, optionally detuned, delayed and scaled.
    fn strum(cents: f64, delay_s: f64, gain: f64, seconds: f64) -> Vec<f64> {
        let n = (RATE as f64 * seconds) as usize;
        let start = (delay_s * RATE as f64) as usize;
        (0..n)
            .map(|i| {
                if i < start {
                    return 0.0;
                }
                let t = (i - start) as f64 / RATE as f64;
                STRINGS
                    .iter()
                    .map(|&(_, f)| {
                        let f = f * 2f64.powf(cents / 1200.0);
                        (2.0 * std::f64::consts::PI * f * t).sin() * (-t * 1.5).exp()
                    })
                    .sum::<f64>()
                    * gain
                    / 6.0
            })
            .collect()
    }

    fn wav(samples: &[f64]) -> Vec<u8> {
        let mut pcm = Vec::new();
        for s in samples {
            let v = (s * 32767.0).round() as i16;
            pcm.extend_from_slice(&v.to_le_bytes());
            pcm.extend_from_slice(&v.to_le_bytes());
        }
        write_wav(RATE, &pcm).unwrap()
    }

    #[test]
    fn levels_pitch_and_onset_from_a_synthetic_wav() {
        let a = analyze_wav(&wav(&strum(0.0, 0.25, 0.8, 1.5))).unwrap();
        assert!((a.onset_s.unwrap() - 0.25).abs() < 0.01, "{:?}", a.onset_s);
        assert!(a.peak_dbfs < 0.0 && a.peak_dbfs > -12.0);
        for (i, string) in a.strings.iter().enumerate() {
            let (_, cents) = string.unwrap_or_else(|| panic!("string {i}"));
            assert!(cents.abs() <= 2.0, "string {i}: {cents}");
        }
        assert!(a.bands_db[1] > a.bands_db[8], "low bands carry the strings");
    }

    #[test]
    fn a_detuned_clip_exceeds_and_links_to_the_engine_check() {
        let a = analyze(&strum(0.0, 0.1, 0.8, 1.2), RATE);
        let b = analyze(&strum(20.0, 0.1, 0.8, 1.2), RATE);
        let base1 = analyze(&strum(0.0, 0.1, 0.8, 1.2), RATE);
        let base2 = analyze(&strum(1.0, 0.1, 0.79, 1.2), RATE);
        let engine_b = vec![
            "Synthetic preset (full pipeline): 1 of 1,092 words differ (pitch/tuning 1)"
                .to_string(),
        ];
        let result = compare(&a, &b, Some((&base1, &base2)), &[], &engine_b);
        assert!(
            result
                .exceeded
                .contains(&"string pitch (largest difference)"),
            "{:?}",
            result.lines
        );
        assert!(
            result
                .lines
                .iter()
                .any(|l| l.contains("matches an engine-check difference"))
        );
        let same = compare(&a, &base1, None, &[], &[]);
        assert!(same.exceeded.is_empty(), "{:?}", same.lines);
        let quiet = analyze(&strum(0.0, 0.1, 0.2, 1.2), RATE);
        let louder = compare(&a, &quiet, None, &[], &[]);
        assert!(louder.exceeded.contains(&"RMS level"));
        let late = analyze(&strum(0.0, 0.3, 0.8, 1.4), RATE);
        assert!(
            compare(&a, &late, None, &[], &[])
                .exceeded
                .contains(&"onset time")
        );
    }

    #[test]
    fn engine_lines_come_from_a_saved_report() {
        let report = "STEPS\n-----\n\n[0.3] Engine check (no device) — PASSED\n  Gate: none · Risk: None\n  Results:\n    Synthetic preset (full pipeline): all 1,092 words identical\n    Synthetic engine cases 01-48: 48 of 48 identical\n  Checksums (SHA-256):\n    abc  x\n\n[1.1] See — PASSED\n";
        assert_eq!(
            engine_lines(report),
            vec![
                "Synthetic preset (full pipeline): all 1,092 words identical",
                "Synthetic engine cases 01-48: 48 of 48 identical"
            ]
        );
    }
}
