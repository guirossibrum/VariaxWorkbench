# Reso-5 chunked buffer read test card

**Approved 2026-11-24 for one owner run of the chunked read.** The reviewer
checks the owner build before the run.

In two owner runs, the PODxt Live delivered no reply to the 64-word request
whose expected reply was 392 bytes. The established 32-word metadata requests return
200-byte replies. A relay size limit is likely, and its exact cutoff remains
unverified. The chunked read uses the same request format as those metadata
reads. [Owner run results](https://github.com/guirossibrum/VariaxWorkbench/issues/234).

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
   (`2A 01`) and waits 100 ms before the first read. It then sends 35
   `0x3C` requests, one at a time. Each reply must contain `8 + 6 × count`
   bytes, including `F7`. [Monkey relay trace](podxt-variax-relay.md).

   | Start words | Words per request | Requests | Reply bytes |
   | --- | ---: | ---: | ---: |
   | 0, 32, …, 1,056 | 32 | 34 | 200 |
   | 1,088 | 4 | 1 | 32 |

5. The host allows up to 10 seconds per attempt. After a damaged reply,
   it waits 100 ms and resends the same chunk request, up to twice. Each
   retry is logged. Each valid `0x3C` reply is saved as
   `start-S-count-N.syx` before the next request.
   The reply carries no start address, so the tool assembles replies in
   request order.
   The raw transport log also records incomplete and malformed traffic.
   After the last request or first failure, the tool attempts guitar session
   off (`2A 00`) and bridge release (`03 23 00`), then releases USB.
6. After all 35 replies arrive, the tool assembles all 1,092 words and
   compares them with the factory Reso-5 program. It prints each differing
   word with its index, active value and factory value.
   Differences at words 871–872 are **unknown, not a failure**. Every other
   differing word is a finding to report.

The firmware trace shows that a valid-checksum load copies all 1,092 words.
Reload restores the flag in word 1 before the read can start, and the traced
tone and volume paths leave the program words unchanged. Words 871–872 contain
tone-position metadata whose live behavior remains unresolved. The trace also
leaves two conditional write paths unbounded, so report findings rather than
attributing them to flash. [Full-buffer firmware findings](firmware-variax-dispatch.md#full-buffer-0x3c-read).

## Stop and recovery

Stop if no reply arrives, three attempts return damaged replies, a file
cannot be saved, or a transport error occurs. Keep the log and the completed
reply files. Ctrl+C takes effect between sends so the host can drain an
in-flight reply through `F7` or its deadline.

If a chunk stalls, the guitar's knobs may freeze while sound continues.
The firmware trace shows that this path touches no flash. Cleanup attempts
may leave messages queued while the parser is stalled. [Stall trace](firmware-variax-dispatch.md#a-reply-that-stops-partway).

After the tool exits and releases USB, unplug the **guitar's VDI cable** for
**10 seconds**, then reconnect it. The owner must check that the guitar
responds to its knobs and selector and that sound is normal. Report the failed
chunk and the outcome. Starting another run needs a new owner approval.
This recovery is based on resetting the guitar's RAM state and has yet to be
observed for this stall.
