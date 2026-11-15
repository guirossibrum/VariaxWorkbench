# Offline bank and collection editing

The Rust GUI opens individual `.vwm` presets, fixed `.vwb` banks and `.l6c`
collections. The file's big-endian `FORM` content determines its type, not its
extension. L6BA and L6CO roots can contain BANK or FLDR children, as supported by
the original Java reader. Unknown chunks, header bytes, parameter types and
padding remain in the retained byte buffer.

The bank browser shows every bank and its presets, including empty-bank labels.
Selecting a preset does not dirty the document or clear redo. Edits immediately
replace that preset in the retained container. Undo/redo and grouped gestures
snapshot the entire document and its selection; unrelated presets remain intact.
Switching presets ends an active gesture. Invalid selection, failed parameter
validation and failed replacement leave bytes unchanged.

Save copy writes the **whole container**, with the corresponding `.vwb` or `.l6c`
extension suggested. It uses the existing sibling temporary-file/sync/atomic
non-overwriting persistence path. Existing files are refused, including the
source. Opening another file or quitting uses the whole-container dirty state.
Browsing and edits are disabled during an active operation; physical access
remains locked, and tests use only a fake completion channel.

The editor starts on the first nonempty bank. An entirely empty bank/collection
is readable by the CLI/library but cannot be opened as a GUI edit document; the
error retains the previous document and the source file. Creating/deleting banks
or presets, changing collection structure and converting historical formats are
not implemented. Existing presets can be edited without those operations.

Unsupported roots, little-endian RIFF/FORM variants, ambiguous/truncated data and
invalid lengths fail without modifying the input. The big-endian Workbench 1.75
files are the verified historical variants; no Workbench HD or POD-bank format
compatibility is inferred. Presets with unknown device IDs remain readable and
copyable; the existing legacy-electric edit guard still applies.

## CLI replacement

```sh
cargo run -- inspect-bundle INPUT.vwb
cargo run -- extract-preset INPUT.vwb PRESET.vwm 1 0
cargo run -- replace-preset INPUT.vwb REPLACEMENT.vwm NEW.vwb 1 0
```

Indices are zero-based. Replacement can change preset length. Only the target
preset and its ancestor length fields change; following presets and unknown
chunks retain their bytes. These commands operate on files only.

## Verification and an original-reader limitation

Synthetic Rust tests cover both roots with three banks, five presets and an
empty first bank. They check browsing, edits across banks, whole-document
undo/redo, grouped edits, failed edits/selections, resized replacement, odd
unknown chunks and their nonzero padding, following-preset preservation,
unsupported inputs and atomic save refusal. Expected complete containers are
rebuilt independently from synthetic source data. A headless GUI test clicks
actual browser rows for both formats, checks edits survive browsing, and verifies
an active simulated operation prevents selection changes.

`python scripts/verify_bundle_matrix.py` uses the original Java bundle writer
to create L6BA and L6CO containers with three banks and five synthetic presets.
For each format, it verifies:

- Original Java and Rust read identical bank/preset summaries, including an empty
  bank and UTF-16 bank names.
- Injected even-length unknown chunks at root, bank and preset levels remain
  readable. Rust replacement preserves every unrelated extension.
- A larger replacement preset is read correctly by original Java; all five
  extracted presets have identical Rust/original-Java parameter summaries.
- Java reserialization of the edited container remains readable by Rust with
  the same summary. Java may drop unknown chunks in its own new output; Rust
  does not drop them from the edited source/copy.

**Observed limitation:** injecting word-padded odd-length unknown chunks makes
the original Java reader throw `EOFException` for both roots. Its container loop
increments by declared chunk size plus eight while chunk reading consumes word
padding. That static observation explains the tested failure; it does not prove
every odd-chunk arrangement fails. Rust accepts/preserves the tested odd chunks,
but original-Java compatibility is not claimed for those files. The harness
records the rejection separately from the passing even-extension cases. The
first unsuccessful reference log remains local; no acceptance tolerance was
added.

Reports are non-overwriting, content-addressed
`analysis/reports/bundle-matrix-*.json`, with original JAR, synthetic source,
helper, driver and Rust executable SHA-256 values. The original JAR remains
ignored/local; only file readers/writers run, without editor/native/device APIs.
The GUI bank fixtures contain only the committed synthetic host preset,
synthetic names and artificial unknown chunks. This proves tested file/editor
behavior, not a guitar backup, sound equivalence, hardware recovery or permission
to send a bank to equipment.
