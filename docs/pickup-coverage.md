# Virtual-pickup coverage and limits

The Variax 600 uses piezo sensing; Workbench pickup controls describe virtual
models. The Rust pickup component does not model physical magnetic-pickup height,
and its coefficient tests do not prove piezo/audio behavior.

The local original Workbench 1.75 Java parameter definitions, parameter map,
`L6PickupControl` and `L6VariaxCoefEngine.VWPatch` were inspected. They expose
virtual pickup model/enable, position, angle and level; phase/wiring controls;
and the associated resistance, capacitance, taper and trim parameters. No separate
height field was found in those sources. `Height` in the pickup control's drawing
code is image/widget geometry, not evidence of a guitar setting. Level must not
be relabeled as height without original evidence.

| Variable | Current evidence | Still pending |
| --- | --- | --- |
| Virtual pickup model | All 43 bundled definitions in relevant component comparisons | Full host combination-selection/override matrix |
| Position | Native normalization, body offsets and selected placements/endpoints | All UI-range transitions and model combinations |
| Inclination/angle | Selected native distance-domain angle/phase placements | Broader coefficient/audio sweep; UI degree bounds and Java conversion are recovered |
| Output level/trim | 216 synthetic gain/trim cases, positive/negative paths and swaps | Broader coefficient interaction matrix; UI endpoints/choices now validated |
| Pole offsets/coil spacing | Typed definitions, pole interpolation, 120 projections and synthetic placements | Full geometry cross-product |
| Enable/phase/series-parallel | Circuit pairings and selected one/two-coil, phase and alternate-tuning collapse paths | Full interaction matrix |
| Electrical controls | Varied R/C loads, both tapers, raw/fitted/smoothed responses | Complete admissible ranges and pairwise/multi-variable coverage |
| Height | No separate Workbench field found | No invented height test or physical-pickup interpretation |
| Piezo/audio response | Not validated by component tests | Independent signal/audio reference and future authorized capture |
| Sustain/bending/tuning | Selected parameter/word conversions | Synthetic audio envelope/pitch trajectories and combined cases (#33) |

The recovered Java UI definitions specify pickup position 0.1..9.5, angle
-45..45 degrees, pickup level -18..6 dB and overall trim -20..6 dB. These are
source definitions, not a claim that every value is safe/valid for every body.
Native host angle arguments are distance-domain values after Java conversion;
the existing -1/0/+1 placement tests must not be described as degree-limit tests.
The current component corpus covers selected values, boundaries and meaningful
combinations, **not an exhaustive Cartesian product** of floating-point inputs.

Shared explicit edit domains and original UI body restrictions are implemented;
all 34 scalar fields have endpoint/invalid-input tests and 84 original-Java reader
comparisons. These do not complete the coefficient/audio cross-product. See
[parameter domains](parameter-domains.md). Broader parameter interactions remain [#7](https://github.com/guirossibrum/VariaxWorkbench/issues/7),
Java/native conversion and orchestration [#4](https://github.com/guirossibrum/VariaxWorkbench/issues/4),
whole-engine parity [#6](https://github.com/guirossibrum/VariaxWorkbench/issues/6),
and late synthetic audio testing [#33](https://github.com/guirossibrum/VariaxWorkbench/issues/33).
Completing the component issue #2 does not complete those gates or authorize
hardware contact.
