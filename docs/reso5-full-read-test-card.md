# Draft Reso-5 stepped buffer read test card

**Not approved.** The owner approves a live run after the reviewer explains
the firmware findings and the procedure. The draft build enables the offline
check and keeps the live stage closed.

## Offline check

Use the private fingerprinted Variax 3.10 archive. The check verifies the
canonical Reso-5 factory program at archive slot 50 and creates a fresh log
folder.

```sh
cargo run --offline --locked --no-default-features --features owner-reso5-test \
  --bin variax-reso5-test -- --check full-read ARCHIVE.vxf LOG_ROOT
```

Pass `--full-read-card` to print this card.

## Draft owner procedure

1. On the Variax, select **Reso, position 5** by hand. Close any other editor
   or device check. Keep the selector, knobs and PODxt channel steady during
   capture.
2. After approval, use the reviewed owner build. Run the offline command
   above with `full-read` in place of `--check full-read`.
3. The tool checks live PODxt Live 3.01 identity on its ALSA MIDI port, then
   Variax 300/600 3.10 identity on the bridge. A lone orphaned identity reply
   tail can trigger the existing one-time identity retry.
4. The tool sends bridge on (`03 23 01`), guitar session on (`2A 01`), and
   waits 100 ms before the first read. Every `0x3C` request starts at zero.

   | Words | Expected reply bytes, including `F7` |
   | ---: | ---: |
   | 64 | 392 |
   | 128 | 776 |
   | 256 | 1,544 |
   | 512 | 3,080 |
   | 1,092 | 6,560 |

5. The host allows at most 10 seconds per step. Each complete, valid
   `0x3C` reply is saved as `start-0-count-N.syx` before the next request.
   The raw transport log also records incomplete and malformed traffic.
   After the last request or first failure, the tool attempts guitar session
   off (`2A 00`) and bridge release (`03 23 00`), then releases USB.
6. A complete comparison prints every differing word with its index, active
   value and factory value. This draft includes all 1,092 words. The result
   describes the active buffer and awaits the firmware findings below before
   it can be interpreted as evidence about a stored slot.

## Stop rules

Stop at the first incomplete, malformed or timed-out reply, file-save error,
or transport error. Keep the log and the completed reply files. Ctrl+C takes
effect between requests so the host can continue draining an in-flight reply
through `F7` or its deadline. A host timeout and attempted cleanup leave the
device's receive and audio state unresolved. Report the first failed step and
wait for the reviewed recovery procedure before further device interaction.

## Firmware TODOs before approval

- Confirm that loading a slot with a valid checksum copies all 1,092 words
  into the active buffer. Identify the exact reload, tone, volume and other
  live or transient words to exclude from a stored-image comparison.
- Confirm reply pacing, what a stalled response leaves in the parser and
  audio path, whether cleanup reaches the guitar, and the recovery procedure.
  Establish whether a power cycle recovers without a flash effect.

The current static findings and open questions are tracked in #234.
