# Build a release set

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
The hosted packages support offline editing and Rig Check Practice on all
three. For a hardware comparison, run the private USB build below at the same
full SHA and provide the Linux and macOS USB packages alongside the Windows
offline ZIP. The current Windows package cannot run live PODxt or Variax
checks. Track Windows device support under [#262](https://github.com/guirossibrum/VariaxWorkbench/issues/262)
and cross-platform release verification under [#263](https://github.com/guirossibrum/VariaxWorkbench/issues/263).

## Private owner USB build

Run the same command from Linux with the owner profile.

```sh
bash packaging/build-release.sh --owner-usb
```

Linux and macOS build both apps with USB and portable math. The command
checks each package's revision and hashes, puts the Linux executables and
local DMG under `backups/packages`, and copies the verified DMG to the Mac's
Downloads folder. Windows USB is pending. `--revision COMMIT`,
`--mac-host HOST`, and `--output DIR` select the revision, SSH host and
local evidence directory. Older owner revisions remain buildable.

The Linux host needs Rust 1.99.1, cached locked Cargo dependencies, OpenSSH
and `flock`. The Mac needs working SSH access and Xcode command-line tools.
The private build uses the established temporary Mac toolchain and DMG
build dependencies. The command prepares packages for a separate owner run.
