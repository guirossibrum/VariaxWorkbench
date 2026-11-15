# Factory reference investigation

Investigation date: 2026-11-15. All work described here was offline analysis of files or public web downloads. No updater was executed, no USB/MIDI message was sent, and no presets or firmware were written. The user's current constraint prohibits live probes and writes until practical offline and simulation work is exhausted.

## What can currently be established

The official [Variax 300/600 flash listing](https://line6.com/software/index.html?hardware=Variax%20300%2F600&name=Flash%20Memory&os=All&submit_form=set&submit_query=Show) lists **3.10**, released **2006-01-06**, release ID **1062**. This matches the guitar's previously observed firmware version. The Central release catalog further identifies this exact model's image as **124,208 bytes**, vendor MD5 **8b64eddd73f1d4a1eaa72c8dbd98ece5**. A matching binary has **not** been obtained; this checksum is catalog metadata, not a locally verified download.

The official [PODxt Live flash listing](https://line6.com/software/index.html?hardware=PODxt%20Live&name=Flash%20Memory&os=All&submit_form=set&submit_query=Show) lists **3.01**, released **2006-04-20**, release ID **1224**, approximately **411 KB**. Its only other listed version is 2.14. The previously observed POD version 3.01 therefore matches the latest flash version in that listing. USB firmware is a separate component and is not established by this comparison.

Matching firmware versions does **not** establish matching presets. User presets can change independently of firmware. An exact comparison requires a full current bank backup plus a trustworthy reference for the same model and version. Moreover, 3.10 factory defaults need not be identical to the state of a guitar shipped before that release.

## Workbench versus factory banks

The extracted Workbench 1.75 installers examined by the main investigation contain electric/acoustic default patches and model definitions. No complete authenticated Variax 600 factory bank has been identified in them. Those model definitions and the electric Default Patch are not evidence that the current guitar matches all original factory slots.

[Line 6's reflash instructions](https://kb.line6.com/variax-reflash-and-software-update-instructions) describe restoring original electric Variax presets as part of firmware reinstallation with preset preservation disabled. This is evidence about where defaults come from, **not a recommendation or authorization to perform that operation**. The plan is to obtain and examine firmware offline first.

An official-community [Factory Bundle thread](https://line6.com/support/topic/3713-factory-bundle/) contains a user attachment posted by hurghanico on 2020-10-22 named `Variax 500_700 backup 22-ott-2020 18.07.vwb` (attachment ID 11442). Its public download returned HTTP 403. It is expressly a **500/700** backup, not a verified 300/600 reference; even successful acquisition would require provenance and model-compatibility checks before using it as a comparison baseline. Standard, Shuriken, JTV/HD, acoustic and bass bundles are likewise not authoritative Variax 600 references.

## Central is an additional offline lead

[Line 6 Central release notes](https://line6.com/software/index.html?name=Line+6+Central&os=All&submit_form=set) explicitly list original `Variax (300/600/500/700/VUI)` in the release-note prose for 1.00 through 1.03. This is stronger than the generic hardware filter. Current listing 1.06 is dated 2026-09-17, but that installer was not obtained in this investigation. [Line 6's Central announcement](https://kb.line6.com/line-6-central-app-now-available) distinguishes Central from editor/librarian applications and says it does not replace preset backups.

A publicly accessible **1.00** Windows installer was downloaded, solely for static extraction:

- Local: `vendor/installers/Line6CentralInstaller1.00.exe`
- Manufacturer CDN: `https://l6c-acdn2.line6.net/media/software/12401/Line6CentralInstaller1.00.exe`
- SHA-256: `6d7a4ba9efb2a6f9f45dd996cd2a7fe34eb7355ec77c8d4a6795b6c64bc9aee6`
- Independent distribution manifest: [Microsoft winget package manifest](https://github.com/microsoft/winget-pkgs/blob/master/manifests/l/Line6/Line6Central/1.00/Line6.Line6Central.installer.yaml), whose recorded hash matches.

Static extraction finds `res/Variax.json`, mapping decimal MIDI identity **458756** (`0x00070004`) to `Variax 300 600`, distinct from **458752** (`0x00070000`, 500/700). It includes an explicit keep-existing-patches choice. No firmware images were found in the extracted installer file inventory. The engine contains `release/list`, `getrelease`, and firmware acquisition method names. This supplies useful additional protocol/reference material without running the updater.

The public catalog endpoint `https://line6.com/api/rest/v1/release/list`, queried with the installer-embedded public application identifier and `midi_id=458756`, returned the 3.10 metadata above. No user account, browser cookies or credentials were used. An image request to the REST `getrelease` route returned **No Token!**, error **15877**; the browser `https://line6.com/getrelease?rid=1062` and POD release 1224 returned HTTP 403. The catalog is reachable but no account-free firmware download route has been established. We do not substitute the 500/700 image: its same-named 3.10 release is **1061**, with a different size and MD5.

Evidence files:

- `analysis/reports/factory-central-release-list-458756.json`: exact 300/600 catalog response.
- `analysis/reports/factory-central-release-list-458752.json`: contrasting 500/700 catalog response.
- `analysis/reports/factory-central-offline-summary.json`: installer hash, Variax compatibility map, update-step names, and acquisition limitations; application identifier omitted.

Public web searches for `.vxf`, factory `.vwb`, release 1062 and `3.10.vxf`, plus Internet Archive's software collection search for Variax, did not yield a verified 300/600 firmware binary or complete factory bundle. These are search limits, not proof that no surviving copy exists.

## Remaining offline work

Obtain the exact public/account-authorized release 1062 and PODxt Live release 1224 files without touching hardware; verify provenance, declared size and available checksums, then record local SHA-256 hashes. Identify their container format, unpack firmware and any default preset tables, and compare those against the recovered Workbench model definitions. Add the factory reference and captured current bank to simulated transport tests before considering any live access. No reset or reflash is part of this investigation.

## User-supplied firmware acquired after the initial report

On 2026-11-15 the user supplied `Downloads/Variax300_3.10.vxf`. It was copied
without execution to `vendor/installers/Variax300_3.10.vxf`. Its **124,208-byte**
length and MD5 **8b64eddd73f1d4a1eaa72c8dbd98ece5** match the exact public
300/600 catalog above. Local SHA-256:
`e9f471e987b04cf31321e8a0da068f44abcdff2edbb32368385c08f92b999c1a`.
This establishes catalog agreement, not an independent digital-signature check.
The source URL was not recorded; the user provided the downloaded file. Firmware
unpacking, factory-bank recovery and executable interpretation remain pending.
PODxt Live firmware has not been located in the Downloads folder.

License Manager 1.16 Windows and Monkey 1.78 macOS downloads were also archived
without execution. The reported POD driver was not located under an identifiable
Line 6/POD/driver filename in Downloads. Inventory and hashes:
`analysis/reports/user-downloads-20261115.json`. License Manager manages optional
software/model-pack authorizations, rather than coefficient generation or preset
file parsing; see [Line 6 FAQ](https://kb.line6.com/line-6-license-manager-faq).
The [retirement notice](https://line6.com/support/announcement/120-pod-studio-owners-line-6-monkey-and-line-6-license-manager-retiring-on-april-10th-2026/)
announced the transaction-server shutdown for April 10, 2026. Preserving the
installer does not preserve server functionality. Hardware access stays locked.

## Both firmware containers unpacked

The subsequently provided PODxt Live 3.01 file and the Variax 3.10 file both
validate and decompress offline, including embedded image MD5 and region coverage.
The POD driver packages were also located and extracted. These later results
supersede the earlier acquisition/driver-location limitations above; see
[offline-generator-results.md](offline-generator-results.md). No factory-bank
recovery, firmware execution or device access was performed.
