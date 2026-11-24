# Reso-5 stepped buffer read test card

**Approved 2026-11-24 for one owner run.** The reviewer checks the owner build
before the run.

## Offline check

Use the private fingerprinted Variax 3.10 archive. The check verifies the
canonical Reso-5 factory program at archive slot 50 and creates a fresh log
folder.

```sh
cargo run --offline --locked --no-default-features --features owner-reso5-test \
  --bin variax-reso5-test -- --check full-read ARCHIVE.vxf LOG_ROOT
```

Pass `--full-read-card` to print this card.

## Owner procedure

1. Select **Reso, position 5** by hand on the Variax before running the test.
   The tool sends no program change. Close any other editor or device check.
   Keep the selector, knobs and PODxt channel steady during capture.
2. After the reviewer checks the build, run the offline command
   above with `full-read` in place of `--check full-read`.
3. The tool checks live PODxt Live 3.01 identity on its ALSA MIDI port, then
   Variax 300/600 3.10 identity on the bridge. A lone orphaned identity reply
   tail can trigger the existing one-time identity retry.
4. The tool sends bridge on (`03 23 01`) and waits **1,000 ms**, matching
   Line 6 Monkey's large-buffer enable. It then sends guitar session on
   (`2A 01`) and waits 100 ms before the first read. Every `0x3C` request
   starts at zero. [Monkey relay trace](podxt-variax-relay.md).

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
6. The tool compares the active buffer with the factory Reso-5 program. It
   prints each differing word with its index, active value and factory value.
   Differences at words 871–872 are **unknown, not a failure**. Every other
   differing word is a finding to report.

The firmware trace shows that a valid-checksum load copies all 1,092 words.
Reload restores the flag in word 1 before the read can start, and the traced
tone and volume paths leave the program words unchanged. Words 871–872 contain
tone-position metadata whose live behavior remains unresolved. The trace also
leaves two conditional write paths unbounded, so report findings rather than
attributing them to flash. [Full-buffer firmware findings](firmware-variax-dispatch.md#full-buffer-0x3c-read).

## Stop and recovery

Stop at the first incomplete, malformed or timed-out reply, file-save error,
or transport error. Keep the log and the completed reply files. Ctrl+C takes
effect between requests so the host can drain an in-flight reply through
`F7` or its deadline.

If a step stalls, the guitar's knobs may freeze while sound continues.
The firmware trace shows that this path touches no flash. Cleanup attempts
may leave messages queued while the parser is stalled. [Stall trace](firmware-variax-dispatch.md#a-reply-that-stops-partway).

After the tool exits and releases USB, unplug the **guitar's VDI cable** for
**10 seconds**, then reconnect it. The owner must check that the guitar
responds to its knobs and selector and that sound is normal. Report the failed
step and the outcome. Any retry needs a new owner approval. This recovery is
based on resetting the guitar's RAM state and has yet to be observed for this
stall.
