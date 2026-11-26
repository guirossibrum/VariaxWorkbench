# Windows x64 package

The unsigned offline alpha ZIP contains **Variax Workbench.exe**,
**Rig Check.exe**, **How to open.txt**, build information and a SHA-256
manifest. Extract the entire ZIP into a folder you can write to, then open
either executable. An installer, Line 6 driver and separate C runtime
installation are unnecessary. Native packages use the MSVC target with a
static C runtime. [Rust linkage reference](https://doc.rust-lang.org/reference/linkage.html)

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

## Build locally or download an artifact

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

After the workflow is on the default branch, build and download it from a
checkout authenticated with the GitHub CLI:

```sh
gh workflow run windows-package.yml --ref master
gh run list --workflow windows-package.yml
gh run watch RUN_ID --exit-status
gh run download RUN_ID -n windows-x64 -D windows-package
```

The Windows runner executes the offline tests before building. A regular
dispatch produces an artifact. The owner may opt in to a GitHub pre-release
labelled **untested alpha** by passing `-f alpha_number=N`. Use an unused
alpha number for the version, since macOS and Windows releases use the same
tag format. [GitHub-hosted runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)

## Native verification

The workflow exercises Workbench version output, Rig Check's catalogue,
a simulated Practice step, and the offline engine report. The packaged
smoke test records the Windows version, matching build number, and
recognized processor architecture from Rig Check step 0.1 in
`verification.txt`. It checks that native USB and audio inventory steps
were skipped, device steps reported the Windows connection notice, and
This computer recorded zero device messages. The inventory parsers and
command selection have offline unit coverage.
On the owner's Windows system, verify SmartScreen, both graphical launches,
opening and saving a synthetic preset, and the USB and audio inventory
screens without connecting the rig. Live transport and audio streams remain
outside this package.
