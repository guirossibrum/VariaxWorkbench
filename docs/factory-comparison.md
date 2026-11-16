# Factory defaults and saved-preset comparison

The archived Variax 300/600 **3.10** firmware is a reference for defaults embedded
in that release. It does not establish the settings of an earlier factory
shipment. No authenticated earlier shipping bank, manufacture-time configuration
or serial-specific reference is available locally. Matching the latest firmware
version does not establish that every stored preset is original.

The [factory extraction](factory-extraction.md) binds the reference to the exact
archived SHA-256 and retains all 60 raw slots separately from decoded settings.
Catalog MD5 agreement, embedded checksums and the source fingerprint do not
constitute publisher authentication. Known global string-level storage is
separate from the per-preset model/settings data. Its fallback defaults are not
substitutes for the owner's calibration.

## Result for the existing Reso-5 capture

The previously saved current-preset metadata named **Reso-5** matches archived
firmware slot 50: name, all **6 model records**, their identity/ordinal/enabled
fields and all **34 parameter type/value pairs** agree exactly. The Rust settings
files are also byte-identical. The earlier original-Java capture validation and
the independent Java decoding of all 60 firmware slots support that comparison.

The private subject is `backups/current-variax-600.vwm`, SHA-256
`13f8a99828851153eebff71b2fa644bf390cf144f9da6b73b9ee6c66f502aaf7`.
It came from the earlier two-block metadata capture in
`analysis/captures/variax-usb-20261114-235059/`. It is historical saved evidence,
not a newly queried device state. The source-bound comparison output is retained
privately as `.tools/issue-15-reso5-comparison.txt`.

This establishes equality of that saved settings document with the firmware
3.10 default. It does **not** compare the other 59 slots, the guitar's complete
DSP program, global levels, other calibration, firmware/flash, audible tuning,
body sustain, string bending or original shipping state. No device was contacted,
preset selected or firmware/reset operation performed for this result.

## Repeatable file-only comparison

```sh
cargo run --bin variax-firmware-inspect -- \
  --compare-slot 50 /ABS/SAVED_SETTINGS.vwm /ABS/Variax300_3.10.vxf
```

The firmware must match the exact archive fingerprint and valid layout. The
subject must have the same device identity and firmware version. The command
reads regular files only, applies bounded input-size checks and performs no
save or device operation. If combined with `--variax-reference`, reference export
occurs only after comparison inputs validate. With `--json`, the container report
stays on stdout and the comparison text goes to stderr.

`factory_comparison::compare_slot` compares every represented model and parameter,
including unknown IDs, exact raw floating-point bits and duplicate multiplicity.
It ignores record ordering when comparing model/settings content. Name equality,
changed model keys and byte-for-byte file equality are reported separately.
Unknown container chunks are outside semantic comparison; only full file equality
covers those bytes. No numeric tolerance hides signed-zero, tuning or other raw
parameter changes. Different products/versions are explicit errors, not automatic
conversion or a claim that one firmware's defaults apply to another.

Four synthetic core tests cover raw-bit/tuning changes, wrong targets/versions,
rename distinctions, unknown parameters, duplicate multiplicity, order changes
and matching settings with differing unknown container bytes.
Two CLI tests reject invalid slot arguments before reading files and require the
exact reference fingerprint before opening subject settings. Actual capture and
vendor-derived files remain private and are not default test fixtures.

## Later full-bank comparison workflow: blocked pending authorized backup

This is a plan, not an available capture command or permission to connect.

1. Finish the offline readiness and read-session review in #22/#24. Any later
   identity, read, session or audio operation requires separate explicit owner
   authorization. A read transmits requests and may change device session state.
2. Establish a complete, independently validated backup plan on #23 before any
   write is considered. A `.vwm` or metadata-only `.vwb` does not contain all
   coefficients, global settings, calibration, flash or recovery material.
   Stored-bank reading may require state-changing selection; no safe sequence is
   assumed. If required ranges or preservation behavior remain unknown, the
   corresponding backup and writes remain blocked.
3. If such capture is separately authorized later, preserve originals and record
   product/version, all requested/received ranges, ordering and completeness,
   timeouts/retries, any state changes and SHA-256 digests. Retain independent
   copies. Replies without addresses cannot establish their own ordering; the
   two saved metadata blocks are not evidence of complete bank coverage.
4. Compare only a same-model/same-version backup with the fingerprinted baseline.
   Resolve slot/program mapping independently; archive order and names alone do
   not prove physical knob/switch positions. Compare all 60 settings and full
   program bytes separately, preserving unknown/reserved fields. Keep device-wide
   levels/calibration separate and report absent data explicitly.
5. Report matches, differences and unverified areas without resetting, reflashing,
   normalizing or uploading. Equal metadata does not prove audio equivalence or
   restoration safety. Earlier shipping state stays unknown unless independent
   provenance for that state is recovered.

All physical source locks remain enabled. The saved-file comparator cannot start
this workflow, discover devices or change presets. #16/#22–#25 retain update,
recovery, backup and write-safety gates.
