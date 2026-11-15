These expected outputs use the existing synthetic input vectors with the
`software-wide-v1/libm-0.2.16` math profile. They were calculated by the original
i386 coefficient instructions running in private Unicorn memory, with the
explicit software math adapter. No vendor model data or device capture is here.

The corresponding original host-math fixtures remain in the parent directory.
Tests select exact expectations for their compiled profile. `provenance.json`
records source/math hashes, input/output hashes, CPU controls, case counts and
scope. `scripts/audit_engine_math_components.py` reproduces the expectations in
an ignored analysis directory and compares debug and release Rust results.

This is independent verification of the surrounding algorithms under a chosen
math adapter. It is not independent proof of that math algorithm's rounding,
historical macOS behavior, sound equivalence or physical hardware safety.
