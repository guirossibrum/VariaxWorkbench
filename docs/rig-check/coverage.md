# Rig Check device-message coverage

The app's editor and Device Check use the same protocol families as the
steps below. The checks exercise the wire path and read-back. The control
checks sample ordinary and compound controls, and the model upload checks
exercise full program payloads.

| App operation | Shared device exchange | Rig Check |
| --- | --- | --- |
| Variax editor Get Edit Buffer | Identity and 35 bounded current-buffer reads | 3.1, 3.3 |
| Variax full live send or manual Send | Validated full `43` upload and full read-back | 4.2, 4.3 |
| Variax live difference | `3B`, acknowledgement, full read-back with retained checksum | 4.4 |
| Variax alternate-tuning Enable transition | Muted `3B`, acknowledgement, unmuted `43`, full read-back | 4.5 |
| Variax Write to device (including Paste staged for Sync) | `43`, 100 ms, one `61`, one acknowledgement wait and verified reload | 5.1, 5.3 |
| Variax Read unmodified / Sync revert | Program change, full stored-program read, verified return | 3.5 |
| Variax Sync batch store | Two verified slot writes with the same shared store-and-reload operation, then verified return | 5.4 |
| Variax Refresh library and Device Check Variax backup | Shared selection and return sweep, split metadata, twelve `6D` bank names and optional 35-chunk full programs | 3.4 |
| Variax identity, settings read and unverified-firmware evidence | Universal identity with bridge retry, two metadata blocks, or the separately gated evidence read | 3.1, 3.2, 3.7 |
| Variax Follow volume, tone, selector and VDI status | Shared Follow session and decoded CC 7, CC 79, program changes and connection notices | 6.1, 7.1, 7.2 |
| PODxt editor Get, current tone, packs and global values | Universal identity, edit-buffer read, playing-channel query, installed packs, global query | 2.1, 2.2, 2.6 |
| PODxt editor control edits and Undo, Device Check Try | Shared controller messages and read-backs, including ordinary CC and paired tempo values | 4.1 |
| PODxt editor and Tone Locker whole-tone Send | Complete `03 74` payload into the playing buffer and exact read-back | 4.6 |
| PODxt editor and Device Check channel recall | Program change with starting and saved-tone checks | 4.7 |
| PODxt editor and Tone Locker Save | Addressed tone frame and commit, one receipt wait, stored read-back | 5.2 |
| PODxt editor and Device Check library reads and backup | Current and addressed reads of tone channels, amp setups and FX setups | 2.2–2.6 |
| PODxt editor Follow | Shared Follow decodes channel and controller updates, then re-reads a named channel | 7.3–7.5 |
| PODxt tuner | Tuner-mode CC on/off and global note and deviation polls | 4.8 |
| USB/driver verification, claim, release and re-attach | Read-only USB discovery, Rig Check claim/release and driver-binding check, verified identity after VDI replug | 0.2, 1.1, 1.2, 6.1, 6.2 |

Tone Locker file changes have no device traffic. Its store API accepts tone
channels. The app reads amp and FX setups and has no setup-store action.
Incoming clip, DSP and queue notices need no separate outgoing request.
The device-owned `0x44` probe, bank rename, and firmware update are outside
the app. CC 84 alternate-tuning override has no confirmed physical action
for a guided step yet.

The normal run performs reads and guided checks before live edits and
saves. A failed step disables later live edits and stores. A skipped or
otherwise unpassed earlier step also disables saves. A compatibility check skipped
automatically on tested firmware permits later steps to proceed. If a
store outcome is unknown, Rig Check ends with USB release only.


## Comparison severity

| Comparison | Severity |
| --- | --- |
| Computer engine output against a reference, including Windows or Mac differences | REPORT with every differing word |
| PODxt reported starting channel against its stored tone and Variax captured working program against the identified stored slot | REPORT for unsaved changes. Step 6.2 verifies the captured buffers. |
| USB rig identity, required firmware, malformed messages and unresolved checksum or missing replies | STOP. An absent device or a recognized alternate firmware follows its compatibility route. |
| Repeated stored reads, backup preflight, post-send and post-store read-back, switch-back, and restored buffer | STOP if the expected data or state differs |
| Physical Follow target, return and controller read-back | STOP on an unverified or wrong result |
| Device display mismatches entered through Different | STOP |
| Live pedal or tone-position changes in comparisons that admit them | INFO |
| Retried reply damage that recovered | INFO in the technical details |
| Store acknowledgement timeout or unknown stored state | STOP and USB release only |

**Show technical details** displays expected and actual hex, the XOR, the number
of differing bits, retries, missing bytes in retried replies and message counts.
Real runs save raw evidence by default in the report's companion folder.
