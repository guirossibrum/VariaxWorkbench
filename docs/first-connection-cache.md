# First-connection library cache (#281)

Offline feasibility and design checkpoint for Variax 600 firmware 3.10
through PODxt Live firmware 3.01 on Linux, and for PODxt Live 3.01 alone.
A verified attachment can offer the existing Device check backup. After the
owner consents and the backup succeeds, the app can cache a complete library
for offline editing. Verification alone identifies the rig without reading
its library. This checkpoint changes no code or device approvals.

## Available data

| Source | What the app can retain | Scope |
| --- | --- | --- |
| Existing USB discovery and universal MIDI identity | Live attachment identity and verified product/firmware. Backups already include `identity.syx`. | The 17-byte reply has manufacturer, family, member and version. Variax member 4 identifies 300/600, with no per-guitar serial. USB path, address and attachment generation identify the current connection. |
| Variax current settings | Two bounded `0x3C` replies, 192 decoded settings bytes. The sweep includes these as `original-{1,2}.syx`. | A snapshot of the playing buffer, which can include unsaved changes. |
| Variax metadata library | All 60 addressed positions via the existing selection sweep, two 200-byte replies per position in `presets/`, and 12 `0x6D` name replies in `bank-names/`. | Enough for a labelled offline library and editable settings. Program changes temporarily change the sound. The existing sweep attempts a verified return when it can identify one starting preset. |
| Variax full library | The same metadata and names plus `programs/NN-Bank-N.bin` for all 60 slots. Each packed image has 1,092 words, 3,276 bytes, and a verified firmware checksum. | Already available as an explicit full backup. Reads the loaded buffer in 35 bounded spans per preset with the existing `0x3C` retry path. It leaves firmware installation and extraction outside the cache. |
| PODxt Live library | 128 tones, 107 amp setups, 64 FX setups, and a playing-tone read before and after. The v1 layout uses `tones/`, `amp-setups/`, `fx-setups/`, `current-before.syx`, and, when received, `current-after.syx`. The dumps are 168 bytes each. | The address sweep sends no channel recall. The existing backup can also observe an installed pack mask. That mask is optional device metadata, and the backup currently records it in the log. |

The existing Variax **Get Edit Buffer** can also read one whole active
program. PODxt **Get** reads the playing tone, active channel, packs and
available globals. These describe the current session. They can stay in
that view without scheduling another query for the library cache. The
Variax `0x44` probe got no reply through this relay. The cache has no
dependency on it. [Variax procedures](../crates/line6-devices/src/family/variax/procedures.rs) ·
[PODxt read procedures](../crates/line6-devices/src/family/podxt/live_read.rs)

Both backups use the `line6-devices backup v1` layout with a SHA-256
manifest and an index for display. A complete metadata backup does not
contain program bytes. A captured program or tone describes a particular
sound. The app's 28 body, 43 pickup and two special Variax definitions
(#278), and its PODxt parameter and model tables, already ship separately.
The PODxt pack mask identifies installed packs, and the available reads do
not supply their definitions. Firmware images, calibration/global string
levels, unverified-firmware evidence, and complete factory libraries
remain outside this cache. Factory presets from an owner-supplied archive
remain an explicit local-file workflow. [Device check](device-check.md) ·
[Model bundle sources](../assets/variax-models/SOURCES.md)

## Cache contract

- **Trigger and scope.** Offer **Cache library for offline use** after the
  first verified attachment for a selected local rig profile and firmware
  tuple. Reuse the existing consent and backup actions. Default to the
  Variax metadata library for editing. Leave the existing full backup as
  an explicit choice. Warn that both guitar sweeps change the playing
  preset. Use the same gates, lock, bounded retries, progress, cancellation
  and cleanup. Recheck the attachment before opening and discard a
  completion for a replaced attachment. The existing PODxt Device check
  backup opens by MIDI device spec. If the spec cannot be tied to the
  verified attachment, leave automatic publication unavailable. On
  unsupported transports or firmware, offer offline files and backup import.
- **Key and receipt.** Store each family separately under an opaque local
  device-profile ID and the verified family, member and firmware. Include
  the relay's product and firmware for the Variax. Record a schema version,
  capture time in UTC, source, coverage (`variax-settings`,
  `variax-programs`, or `podxt-library`), manifest digest and relative
  snapshot location. Treat the Variax profile as the owner's label for the
  guitar/relay pairing. If the connection changes and identity cannot
  distinguish instruments, ask the owner which profile or a new profile.
  Never bind live sends to a cached identity. Keep identity and preset data
  in private app data.
- **Validation.** Reuse the v1 decoders and require a complete, typed backup
  receipt. Check receipt identity against the manifested raw identity and
  the generation key. Require each expected location once, and consume only
  manifest-listed files. Bound receipt, manifest and file reads. Refuse
  traversal, absolute paths, symlinks, duplicate entries, unsupported
  schemas, and missing or bad digests. Parse both Variax metadata replies
  and every bank name; for full coverage also check the exact program
  filenames, lengths and firmware checksums. Parse all PODxt dumps. The
  automatic cache additionally requires verified Variax switch-back and
  successful cleanup, or a closing PODxt read that matches the starting
  tone under the existing live-control comparison. Some existing backup
  receipts accept weaker return or closing-read evidence. Treat those
  as explicit offline imports under the existing import rules. The
  manifest detects corruption. It does not authenticate a physical unit.
- **Publication.** Reuse `LibraryPreferences`' independent snapshot copy
  pattern. Stage only verified files and the manifest in a newly reserved
  directory on the app-data filesystem. Sync the copies, validate the
  staged generation, then publish a synced small receipt atomically with
  no replacement. Sync directory entries where supported. Only published,
  revalidated generations are startup candidates. Preserve the user's
  source backup and earlier generations. A crash before publication leaves
  no selectable generation.
- **Failure and refresh.** Retain any incomplete evidence separately and
  show the failing stage, including switch-back uncertainty or driver
  release failure. On read, validation, disk, or publication failure, keep
  using the previous complete generation. On startup, revalidate candidates
  newest first within the chosen profile and fall back to the preceding
  good generation. Refresh is owner-initiated, rechecks the live attachment
  and publishes a new generation. A metadata refresh keeps its own
  coverage. Show an earlier full capture separately and never combine its
  program files with the new settings. Missing optional pack information is
  shown as unknown. Cached libraries carry the source and capture time.
- **Fresh offline install.** Open and edit supported Variax preset/bundle
  and PODxt tone files without a cache or a connection. Offer the built-in
  default or an empty library when there is no saved library. Offline
  opening, editing and cache selection send no device messages. Save edits
  to the user's file or a new copy under the existing editor save rules.

## Lane 3 handoff and acceptance tests

Consume a validated immutable snapshot as an editor input. Use
`BackupBundle::load` for Variax and the existing PODxt backup loader for
tone and setup entries. Keep source labels, coverage, and cache age visible.
The app currently has `LibraryPreferences` receipt/copy helpers and a PODxt
editor path-only backup recall. Route first-connection data through the
shared snapshot validation rather than adding a third store. [Snapshot helpers](../src/app/library_preferences.rs) ·
[Variax startup](../src/families/variax/editor/startup_library.rs) ·
[PODxt startup](../src/families/podxt/file/startup.rs)

Coordinate the
verified attachment and consent hook with Lane 1 and the PODxt consumer
with Lane 4. The owner decision before implementation is whether this
offer-based policy is sufficient. An unattended sweep needs a separate
decision, particularly because Variax selection can discard unsaved edits
and a unique return may be unavailable.

| Fake or file fixture | Acceptance result |
| --- | --- |
| Verified rig accepts the offer | The transcript equals the chosen existing backup path. Assert the expected counts, Variax switch-back/cleanup and PODxt before/after checks. No additional device commands. A clean offline restart loads the published generation. |
| Owner declines, attachment changes before the worker opens, firmware is unverified, or app starts offline | No backup traffic or new generation. Supported files and defaults still open without a device. |
| Timeout, malformed reply, cancellation, mid-read replacement, ambiguous guitar return, changed PODxt tone, or failed cleanup | Retain available evidence as incomplete for cache purposes. No receipt is published. Previously cached data remains available. |
| Reopen with wrong profile/firmware, tampered manifest, missing/duplicate slot, unlisted or symlinked file, bad program checksum, or bad filename | Reject that generation and fall back to an earlier valid generation in the same profile, or to the offline defaults. |
| Inject save interruption or collision, then refresh successfully | No partial generation activates and no source or prior generation is overwritten. Refresh publishes a distinct receipt and preserves the earlier snapshot. |
| Open two similarly named rigs, edit from cache, then reconnect | The app requires explicit profile choice where identity is ambiguous. Editor changes do not modify the snapshot. Reconnection requires fresh live identity and never replays a cached program. |
