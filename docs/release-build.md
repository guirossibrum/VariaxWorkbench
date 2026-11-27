# Build Workbench and Rig Check

For routine owner testing, use the [local three-platform build](#owner-comparison-across-operating-systems).
GitHub Actions dispatch requires explicit owner authorization.

## Authorized hosted release set

Run this from the repository with the GitHub CLI authenticated.

```sh
bash packaging/build-release.sh
```

The command fetches `origin/master` and resolves it to one full commit SHA.
GitHub Actions builds Workbench and Rig Check on native Linux, macOS and
Windows runners. Every worker checks out the pinned SHA. The release command
uses that commit's first-parent count as its build number, so rebuilding the
same revision on any platform gives the same number. A direct Cargo
development build displays `Build dev`. Older pinned revisions with the
original build-number file retain the number recorded at that commit.
The final job
recomputes the package checksums and verifies the source, build number, target and profile
in each worker's report. A missing, corrupt or mismatched package fails
the set. Use `--revision COMMIT` to select a master commit that includes
the unified build.

The command waits for the exact run and returns its result. Open the run URL
it prints and download the
`release-set-<full SHA>` artifact after success. It contains the Linux
x64 tarball, universal macOS DMG and Windows x64 ZIP, verification reports
and a set manifest. Each package includes Workbench and Rig Check. The
Mac disk image carries both universal app bundles. All three packages use
the offline portable-math profile.
An artifact-only run creates no GitHub release. For an authorized untested
alpha, append `--alpha-number N` with an unused positive number. All
three packages and the manifest are published under a single prerelease
tag after verification succeeds.

## Owner comparison across operating systems

Deliver Linux, macOS and Windows from one source revision and build number.
Run the owner build from Linux.

```sh
bash packaging/build-release.sh --owner-delivery
```

The command pins freshly fetched `origin/master` by default. Use
`--revision COMMIT` for an existing local revision without a fetch.
It builds Linux and universal macOS with the private USB worker and
cross-builds both Windows applications for `x86_64-pc-windows-gnu` on Linux.
The complete set appears in a new directory under `backups/packages` after
all three packages pass the metadata and checksum checks. Use `--output DIR`
to choose a different destination and `--mac-host HOST` to select the Mac
build host. The DMG stays in the complete set for manual transfer.

In addition to the private USB build dependencies below, the Linux host
needs Rust stable with `x86_64-pc-windows-gnu` already installed, MinGW-w64
x64 GCC and binutils, and cached locked Cargo dependencies. The Windows
worker checks that stable and the project's pinned Rust 1.99.1 compiler
report the same compiler commit. It records the compiler used in the
manifest, the ZIP's `BUILD.txt`, and the verification report.

`release-manifest.json` records the SHA, build number, per-platform profiles
and targets, checksums, and supported tests. Windows supports UI and file
operations and Rig Check Practice only. Its verification covers unsigned x64
PE files, Windows system imports, ZIP contents, and internal and detached
SHA-256 checksums. The manifest records the Linux cross-build origin and
marks native Windows smoke verification pending. Linux and macOS use the
USB profile and support approved live checks. Track Windows device
support under [#262](https://github.com/guirossibrum/VariaxWorkbench/issues/262)
and [#279](https://github.com/guirossibrum/VariaxWorkbench/issues/279).

With explicit Actions authorization, `--owner-delivery-hosted` uses the
artifact-only workflow's native Windows MSVC ZIP alongside the private
Linux and macOS packages. It records the hosted artifact run ID.

## Private owner USB build

Run this from Linux when only the private USB pair is needed.

```sh
bash packaging/build-release.sh --owner-usb
```

Linux and macOS build both apps with USB and portable math. The command
checks each package's revision and hashes, puts the Linux executables and
local DMG under `backups/packages`, and copies the verified DMG to the Mac's
Downloads folder. Both packages record one source SHA and build number and
use the local Linux and Mac build hosts. Windows USB is pending. `--revision COMMIT`,
`--mac-host HOST`, and `--output DIR` select the revision, SSH host and
local evidence directory. Older owner revisions remain buildable.

The Linux host needs Rust 1.99.1, cached locked Cargo dependencies, OpenSSH
and `flock`. The Mac needs working SSH access and Xcode command-line tools.
The private build uses the established temporary Mac toolchain and DMG
build dependencies. The command prepares packages for a separate owner run.
