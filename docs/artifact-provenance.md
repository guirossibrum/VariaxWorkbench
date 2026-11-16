# Artifact provenance

Offline record of the preserved Workbench, Monkey, Central, firmware, and POD
driver packages. The machine-readable copy is `src/provenance.rs`. Hashes below
were recomputed from the local ignored archives and agree with the saved
inventory files. This pass did not download anything, did not use an account,
and did not open USB, MIDI, or audio devices.

Integrity and listing checks are not authenticity. No Authenticode or codesign
result is claimed. A passing comparison is not permission to install, flash, or
unlock hardware.

## Local archives

Paths are under `vendor/installers/`. Two Monkey 1.78 filenames are one byte
image. "URL not captured" means the saved inventory does not name a source URL.

| Product | Version | Bytes | MD5 | SHA-256 | Source |
| --- | --- | --- | --- | --- | --- |
| Variax 300/600 firmware | 3.10 | 124208 | `8b64eddd73f1d4a1eaa72c8dbd98ece5` | `e9f471e987b04cf31321e8a0da068f44abcdff2edbb32368385c08f92b999c1a` | URL not captured |
| PODxt Live main firmware | 3.01 | 421218 | `faabb5e526bcba858d2defe96547e26c` | `738f72e95f72e0bf4e52c4973ca86f098fae54a5d68e1c7b46112ad261ff07f5` | URL not captured |
| Line 6 Central | 1.00 | 98587952 | `f8b524cc176dd1b6e00504e0be339408` | `6d7a4ba9efb2a6f9f45dd996cd2a7fe34eb7355ec77c8d4a6795b6c64bc9aee6` | Recorded CDN URL |
| Line 6 Monkey macOS | 1.78 | 3792653 | `72b37b904ad7838b8d849ff3cde04dee` | `96dbb7f11635a736a84def227a76dd99aa8028cff4d01f047774e2d1f3b3b3a0` | Historical URL, not in the user inventory |
| Line 6 Monkey Windows | 1.77 | 5328432 | `0e4a0bf7bd0ee0ed5ca07b4f008eea7c` | `9e4e487226f44ae8b58b67e67c55cf460447af5d290de1e1fd02ab3dce49b590` | URL not captured |
| Line 6 License Manager | 1.16 | 2700824 | `bbd1856f956b06916a9cb6af825e95d1` | `5c2eb5ee09cd2f029507969bbf9c9279d80ea9489c31f830784048cc962123d2` | URL not captured |
| Variax Workbench macOS | 1.75 | 18085510 | `f7a2bec33a1efc33b98b9ed53b314b63` | `d70b73a1b0e441e7a640c387f13e42f44284388063b155fa162fcc771a614fa1` | URL not captured |
| Variax Workbench Windows | 1.75 | 19871616 | `3f452b92bd7a3234d3d8afb23c9b059d` | `a6870ad2792fb0ee70683be0402cbc828ac316dcd1aa66abb2813cdc1df40f99` | URL not captured |
| Line 6 Drivers Windows | 4.2.7.7 | 29821648 | `e11c8ad8bdd169692f724da4ac290def` | `dd2f3a787006ecb0341b0ff2f4a8699c00df274b494eb6ed2c6beae4e13c7b92` | URL not captured |
| Line 6 Audio-Midi Driver macOS | 7.6.8 | 2869592 | `a54c2f5da10ea8838c4e85ca992975ed` | `48099370936bea11e09b9d8ee4828e2c9050dd1a1da1586828b7f5a72b559436` | URL not captured |

The Central URL recorded with the file is
`https://l6c-acdn2.line6.net/media/software/12401/Line6CentralInstaller1.00.exe`.
`docs/factory-reference.md` already records that the Microsoft winget manifest
for Central 1.00 has the same SHA-256. This change did not download that
manifest again. The winget agreement is another copy of the hash, not a
signature.

`docs/native-analysis.md` says Mac Monkey 1.78 was downloaded from
`https://line6.com/getrelease?rid=8688`. The same SHA-256 is also the
user-supplied `Line 6 Monkey 1.78.dmg`, whose inventory says the URL was not
recorded. The endpoint was not contacted again, so that URL stays unconfirmed.

Driver hashes above are the installer archives. They are not hashes of
extracted `.sys` or kext files. Those packages were not installed.

## Catalog agreement

The saved Central response for MIDI id **458756** (`0x00070004`, Variax
300/600) is `analysis/reports/factory-central-release-list-458756.json`.

| Release | Product | Version | Date | Bytes | MD5 | Local file |
| --- | --- | --- | --- | --- | --- | --- |
| 1062 | Variax 300/600 | 3.10 | 2006-01-06 17:34:03 | 124208 | `8b64eddd73f1d4a1eaa72c8dbd98ece5` | size and MD5 match `Variax300_3.10.vxf` |
| 1061 | Variax 500/700 | 3.10 | 2006-01-06 17:34:01 | 121870 | `d6c5566c19ee4183a105b0ef302e7c47` | not obtained |
| 740 | Variax 500/700 | 3.05 | 2005-08-03 09:07:20 | 121346 | `bb0256d9b10d6b64ef4c4923be4029bb` | not obtained |

Release 1062 is the only local size-and-MD5 agreement. That agreement does not
prove a signature and does not, by itself, prove every uncompressed image byte
beyond the recorded digest. The 500/700 rows are a different product. They were
not substituted.

The public PODxt Live flash listing names release **1224**, version **3.01**,
date **2006-04-20**, approximately **411 KB**. The saved evidence has no catalog
MD5 and no exact catalog byte length for that listing. The local 421,218-byte
file is the main FORM/L6FF flash container. It is not the separate USB-component
firmware, which remains [#20](https://github.com/guirossibrum/VariaxWorkbench/issues/20).

The three saved catalog rows have empty `pre_update_url`, `post_update_url`,
and `fail_update_url`.

## Central and Monkey, from saved static evidence

`analysis/reports/factory-central-offline-summary.json` records an offline read
of Central 1.00. The public application identifier required by the catalog
query was omitted there and is omitted here. No user credential was used.
`hardware_access` and `executed_updater` are false.

- Release list: `https://line6.com/api/rest/v1/release/list` with MIDI id 458756.
- Image request: `https://line6.com/api/rest/v1/getrelease?rid=1062` returned
  **ERROR 15877**, **No Token!**
- Browser `https://line6.com/getrelease?rid=1062`, and the noted POD release
  1224 request, returned **HTTP 403**. The saved POD note does not give a second
  full URL. No account-free firmware download route was established.
- Extracted `res/Variax.json` SHA-256
  `0212bb5832870d6d8f74d61445d1c0ce1043d7657b50892f561766277fef1b1e`.
  It maps 458752 to Variax 500/700, 458753 Acoustic, 458754 Bass, 458756
  300/600, 458757 Tyler, and 458758 Bass 705. Each declares
  `minInstalledFirmwareVersion` **0** and `minAppVersion` **0.90.0a**. Zero is
  an installer field, not an observed device version. The installer inventory
  contained no firmware images.
- Named update steps, not executed: `sendEula` / `displayEula`,
  `sendKeepPresetsOption` / `displayYesNoView`,
  `sendReportEngineProgressOrder` / `startUpdateEngine`,
  `sendUpdateCompletedHTML`. `updateFailed` is present and is not one of the
  ordered steps.

Monkey 1.77 and 1.78 were not run. Named symbols
`L6CamelSystem::UpdateFirmware` and `MNKYUpdateFirmware` are an inventory from
static analysis, not a reconstructed updater. The 3.10 catalog notes say to
update with Monkey 1.10 or later; that sentence is catalog text, not evidence
that an update ran. Preserving Monkey and License Manager installers does not
preserve the transaction server after the announced 2026-04-10 retirement.
Central listing 1.06, dated 2026-09-17, was not obtained.

## POD HD500X catalog and driver text

The POD HD500X release rows are not in the page shell
`index.html?hardware=POD%20HD500X&os=All`. They are in the GET that adds
`submit_form=set` and `submit_query=Show`. The 2026-11-16 response is HTTP
200, 315367 bytes, SHA-256
`048b200988464d1e73942a36afe657e2af7e9aa933236b6000a452de48fe3657`, saved as
ignored `analysis/reports/pod-hd500x-public-catalog-2026-11-16.html` in the
primary checkout. That file lists Flash Memory v2.62 as release 6260, sidebar
Version 2.62.00, Released 4/9/15, estimated 892 KB, with no catalog MD5. The
notes name USB Firmware 1.04 separately. Edit 9781 and 8559 both print
Version 2.27, with sidebar dates 8/4/20 and 9/10/18. On 2026-11-16,
unauthenticated HEAD requests for both Edit readeula links returned HTTP 302
to the account login page. HEAD requests for the CDN directories
`media/software/9781/` and `media/software/8559/` returned HTTP 404. The login
page was not requested. The same day's flash probe remains HTTP 302 for
`readeula.html?rid=6260` and HTTP 403 for `getrelease?rid=6260`. No editor
installer and no flash image were saved. Two public 2013 manuals are hashed in
[POD HD500X](pod-hd500x.md). Their printed file suffixes disagree, and neither
manual is a preset container. Details and the driver-text USB match are in
that document.

A 7-Zip data extract of `l6PODHD5SE.inf` from the Windows 4.2.7.7 archive is
10484 bytes, SHA-256
`50080c9e9efb7b104e2520ea577db3eb2c13f9c16fb0cf90c1628ef31f5b1790`.
Both audio-kext `Info.plist` files reached through the macOS 7.6.8 DMG are
47343 bytes, SHA-256
`17f83de2fcfc818e7ea50699406fcbe7ab15ae52dd74db735a5f33e695c14b9a`.
Those member hashes are not the installer hashes. No driver binary was loaded
and no installer was run.

## Checks

```sh
cargo test --all-features provenance
cargo test --all-features
cargo clippy --all-features --all-targets -- -D warnings
```

Default tests use the compiled constants and a synthetic empty buffer. They do
not open `vendor/`, the network, or a device.

## Decisions left for review

- The Monkey 1.78 URL in `docs/native-analysis.md` stays unconfirmed because the
  user-download inventory for the same digest says the URL was not recorded.
- The PODxt Live listing stays qualitative ("approximately 411 KB"). It is not
  compared numerically with the local 421,218-byte file.
- The opening of `docs/factory-reference.md` had said the 300/600 binary was not
  obtained. This change corrects that sentence. The later addendum in that file
  already recorded the local match.
- No signature type is represented. Adding one requires a real scheme and
  evidence, which this record does not have.
