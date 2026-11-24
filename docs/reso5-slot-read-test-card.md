# Reso-5 read-only slot test card

Owner run only, under the [#225 approval](https://github.com/guirossibrum/VariaxWorkbench/issues/225). Use the Variax 600 on firmware 3.10 through one PODxt Live on firmware 3.01. Wait for the #181 review to confirm the association of indices 50 and 62 with the store target and spare before interpreting the results for #177.

1. Use Device check to make same-day (UTC) Variax and PODxt Live library backups on this rig. Confirm both firmware versions in Device check, then close the app and any other editor. Keep the complete backup folders.
2. Select **Reso, position 5** on the guitar and leave the PODxt channel and guitar controls alone during the read. The tool checks the captured firmware identities from the backups and checks the first reply against the factory Reso-5 metadata. The Variax 300 and 600 share a MIDI identity, so verify the guitar model and selector in person.
3. From the issue worktree, use the completed backup folders, the private factory archive and a local log root such as `backups`.

   ```sh
   cargo run --offline --locked --no-default-features --features owner-reso5-test \
     --bin variax-reso5-test -- --check slot-read VARIAX_BACKUP PODXT_BACKUP ARCHIVE.vxf LOG_ROOT
   ```

4. Run the same command with `slot-read` in place of `--check slot-read`. To print this card, use `--slot-read-card`. The live stage sends the seven messages below and waits up to 10 seconds for each reply. It attempts session and bridge cleanup on failure or Ctrl+C. Each invocation creates a fresh folder and logs raw transport bytes to `log.txt`. Validated frames are saved as canonical 3,276-byte program files immediately, including in partial captures.

| Order | Host sends |
| --- | --- |
| 1 | `F0 00 01 0C 03 23 01 F7` |
| 2 | `F0 00 01 0C 07 00 2A 01 F7` |
| 3 | `F0 00 01 0C 07 00 44 7F F7` |
| 4 | `F0 00 01 0C 07 00 44 32 F7` |
| 5 | `F0 00 01 0C 07 00 44 3E F7` |
| 6 | `F0 00 01 0C 07 00 2A 00 F7` |
| 7 | `F0 00 01 0C 03 23 00 F7` |

Expect three 6,561-byte replies framed as `F0 00 01 0C 07 04 45 <index> <6552 nibble bytes> F7` (the decoder also accepts family byte `00` in place of `04`). Expect index 0x32 to equal the factory program whose SHA-256 starts `f8694ec3` and ends `83c5`, index 0x3E to return all `FF` packed bytes, and no audible change. The tool reports the first differing word if index 0x32 differs from the reference, compares the active RAM program with index 0x32, and reports all three checksums. A checksum mismatch for the blank program is expected. Blankness applies to the returned program bytes.

Stop and report any refusal, missing or malformed reply, unexpected program result, audible change, or loss of guitar response afterwards. Keep the printed log folder for review. After a successful read, verify normal response at the guitar and report the results before proceeding to #177.
