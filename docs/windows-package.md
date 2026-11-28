# Windows x64 package

The unsigned offline alpha ZIP contains **Variax Workbench.exe**,
**Rig Check.exe**, **How to open.txt**, **BUILD.txt** and a SHA-256
manifest. Extract the entire ZIP into a folder you can write to, then open
either executable. An installer, Line 6 driver and separate C runtime
installation are unnecessary.

Both package variants use the same archive name. Check `BUILD.txt` for the
build number, source commit and target.

| Package | Rust target | Verification |
| --- | --- | --- |
| Local owner delivery (`--owner-delivery`) | `x86_64-pc-windows-gnu` | Linux MinGW-w64 cross-build. Static package checks, with native Windows smoke pending. |
| Authorized hosted release | `x86_64-pc-windows-msvc` | Native Windows runner. Static C runtime and packaged command-line smoke. |

[Release build guide](release-build.md) · [Rust linkage reference](https://doc.rust-lang.org/reference/linkage.html)

If Windows SmartScreen shows **Windows protected your PC**, select
**More info → Run anyway** for a download you trust. The ZIP download
includes a detached `.zip.sha256` file. In PowerShell, run
`Get-FileHash .\Variax-Workbench-<version>-windows-x64.zip -Algorithm SHA256`
with the actual archive name and compare the result to that file.
`SHA256SUMS.txt` inside the ZIP covers both executables and the enclosed
text files. [Microsoft SmartScreen documentation](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation)

## Available offline

Variax Workbench opens, edits and saves offline presets and libraries. Rig
Check offers **Practice** with a simulated rig and **This computer** with the
Windows version and build, Windows USB and driver inventory, the built-in
offline engine comparison, and Windows sound-device inventory. The audio
inventory reports PODxt names when Windows lists them. It sends no messages
and opens no audio stream. Rig Check saves reports to
`%LOCALAPPDATA%\Rig Check\reports`.

Device actions show “The Windows connection isn't available in this version
yet.” The build omits the USB feature.

## Rig Check inventory

Step 0.1 reads the dotted version from `cmd /c ver` and reports its third
component as the Windows build. The architecture comes from the Windows
process environment, preferring `PROCESSOR_ARCHITEW6432` when Windows
supplies it for an emulated process.

Step 0.2 reads the third-party driver store and connected PnP records with
`pnputil /enum-drivers` and `pnputil /enum-devices /connected`. It recognizes
English structural fields in UTF-8-compatible output and reports package
INF names and numeric versions, plus USB vendor/product IDs, OS-reported
device status and the listed driver INF. The USB instance suffix is omitted.
An absence means the collector parsed records and found no matching Line 6
provider or USB vendor ID. Failed, localized or unrecognized results,
including banner-only output, are reported as unavailable. Optional values
missing from a recognized Line 6 record are unavailable individually.

A driver-store package does not establish driver loading or MIDI
availability. The connected records show individual PnP status and driver
names. They supply no verified mapping from a Line 6 USB record to its host
controller or to WinMM endpoints, so both report fields remain unavailable.
The collector uses no MIDI endpoint query. Native validation of the command
output and the Windows desktop report remains pending. See the
[Windows transport findings](driver-windows-podxt.md).

## Build natively or download a hosted artifact

On x64 Windows, use Rust via rustup, Git, Python 3.9 or newer, and Visual
Studio C++ build tools with a Windows SDK. Install the pinned Rust toolchain
if needed, then run:

```powershell
rustup toolchain install 1.99.1 --profile minimal
py -3 packaging/windows/build.py
```

The default output is `dist/windows`. The script inspects both unsigned
x64 executables, checks their DLL imports, verifies both layers of checksums,
and runs a command-line smoke test from the extracted ZIP.
`verification.txt` records the target, compiler, system DLL imports, sizes,
and smoke results.

Use the [unified release command](release-build.md) for the hosted offline
set. Its native Windows runner executes the offline suite and packages the
Windows x64 ZIP from the same commit as the Linux and macOS jobs.

## Hosted MSVC verification

The workflow exercises Workbench version output, Rig Check's catalogue,
a simulated Practice step, and the offline engine report. The packaged
smoke test records the Windows version, matching build number, and
recognized processor architecture from Rig Check step 0.1 in
`verification.txt`. It checks that native USB and audio inventory steps
were skipped, device steps reported the Windows connection notice, and
This computer recorded zero device messages. The inventory parsers and
command selection have offline unit coverage.
Native desktop validation of the USB and audio inventory screens remains
pending.

## Owner smoke test on Windows

For Build 526, use the GNU ZIP in the owner set's `windows/` directory.
Its source commit is
`b60e7367b52e9f814268631803f22105c6434c5d` and its profile is `offline`
with `gui,portable-math`. Static verification covered both unsigned x64 PE
executables, their Windows system DLL imports, the ZIP contents, and both
layers of SHA-256 checksums. See `windows-verification.txt` beside the ZIP
and `release-manifest.json` at the root of the owner set. The steps below
are the pending native Windows check.

1. Keep the PODxt Live and Variax disconnected. In PowerShell, run
   `Get-FileHash .\Variax-Workbench-0.9.0-windows-x64.zip -Algorithm SHA256`
   and compare the result with the adjacent `.zip.sha256` file. Build 526
   should match
   `a301a1f706b85040e862d2e4e4fd59ab04078b1ea56c63036d1de61b8e9c942d`.
   After a match, extract the whole ZIP. Check that `BUILD.txt` says
   Build number 526 and `x86_64-pc-windows-gnu`.
2. Launch **Variax Workbench.exe** from the extracted folder. Follow the
   SmartScreen guidance above if prompted. In the Variax editor, use
   **Open…** with a local copy of `synthetic-host-preset.vwm`. If you only
   have the ZIP, create a scratch preset with **New Variax file → New Model…**,
   then reopen it with **Open…**. Change a control, use
   **Save As…** to write a new `.vwm`, and reopen the saved copy to confirm
   the change persisted. [Synthetic preset fixture](../tests/fixtures/synthetic-host-preset.vwm)
3. Launch **Rig Check.exe** from the same folder. Choose **Practice**,
   follow the prompts to run a simulated device step, and confirm it
   displays a result. Practice uses simulated devices. Live USB, MIDI and
   audio transport are outside the smoke test.
