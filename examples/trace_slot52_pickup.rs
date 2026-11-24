//! Offline component replay driven by the private native trace.
use std::io::{self, Read};
use variax_workbench::{
    dsp::{LEGACY_SAMPLE_RATE, vx_float_words},
    models::{PickupCircuit, PickupWiring},
    pickup::{
        Biquad, FilterParameters, SecondaryFit, TableSettings, VolumeTaper, WiringProperties,
    },
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let values = input
        .split_whitespace()
        .map(str::parse::<f64>)
        .collect::<Result<Vec<_>, _>>()?;
    let [
        dc,
        magnetic,
        inductance,
        capacitance,
        load_c,
        load_r,
        tone_r,
        volume_r,
        tone_c,
        frequency,
        q,
        gain,
        volume,
        tone,
        limit,
        samples @ ..,
    ] = values.as_slice()
    else {
        return Err(
            "expected circuit, wiring, primary fit, controls and sample frequencies".into(),
        );
    };
    let circuit = PickupCircuit {
        dc_resistance: *dc,
        magnetic_resistance: *magnetic,
        inductance: *inductance,
        capacitance: *capacitance,
    };
    let wiring = WiringProperties {
        load_capacitance: *load_c,
        load_resistance: *load_r,
        tone_resistance: *tone_r,
        volume_resistance: *volume_r,
        tone_capacitance: *tone_c,
    };
    let primary = FilterParameters {
        frequency: *frequency,
        q: *q,
        gain: *gain,
    };
    let fit = circuit.fit_secondary_filter(
        wiring,
        primary,
        SecondaryFit {
            sample_rate: LEGACY_SAMPLE_RATE,
            volume: *volume,
            tone: *tone,
            frequency_limit: *limit,
        },
    )?;
    println!("{} {} {}", fit.frequency, fit.q, fit.gain);

    let other = PickupCircuit {
        dc_resistance: 0.,
        magnetic_resistance: 0.,
        inductance: 0.,
        capacitance: 0.,
    };
    let settings = TableSettings {
        sample_rate: LEGACY_SAMPLE_RATE,
        wiring: PickupWiring::Parallel,
        taper: VolumeTaper::Linear,
    };
    let table = circuit
        .response_tables(other, wiring, settings)?
        .ok_or("inactive circuit")?;
    let response = circuit
        .calculated_response(other, wiring, settings)?
        .ok_or("inactive circuit")?;
    for (raw, narrowed) in table.0[45..54].iter().zip(response.secondary_q) {
        let [exponent, mantissa] = vx_float_words(narrowed);
        println!("{raw} {} {exponent} {mantissa}", f64::from(narrowed));
    }

    let biquad = Biquad::bandpass(LEGACY_SAMPLE_RATE, primary);
    for &sample in samples {
        let filter = biquad.response(LEGACY_SAMPLE_RATE, sample);
        let analog = circuit.response(wiring, sample, *volume, *tone);
        let residual = analog.magnitude() / filter.magnitude();
        println!("{sample} {} {} {residual}", filter.real, filter.imaginary);
    }
    Ok(())
}
