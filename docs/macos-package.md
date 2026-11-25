# macOS package

The phase-one disk image places **Variax Workbench**, **Rig Check** and an
Applications shortcut in a Finder window. Drag both apps onto Applications.
The disk image includes [How to open.txt](../packaging/macos/How%20to%20open.txt)
for authorizing the first launch. Both apps use ad-hoc signatures and ship
without notarization. The app icons and disk image background were created
for this package.

| Slice | Deployment target | Intended test machines |
| --- | --- | --- |
| Intel `x86_64` | macOS 10.15 | 2012 Intel iMac on Catalina 10.15.8 |
| Apple silicon `arm64` | macOS 11.0 | M1 Mac and Mac mini on current macOS |

Rust 1.99.1 accepts `MACOSX_DEPLOYMENT_TARGET=10.15` for Intel. On arm64
it raises that setting to 11.0, the earliest macOS release for Apple silicon.
The script sets each slice's effective target explicitly and checks the linked
binaries. [Rust Darwin targets](https://doc.rust-lang.org/rustc/platform-support/apple-darwin.html)

Both Mac apps choose eframe's Glow/OpenGL renderer. Apple deprecated OpenGL
but ships it on Intel Catalina and Apple silicon. Glow uses the existing renderer
and avoids introducing wgpu's Metal backend and its separate adapter and
feature-level requirements on the 2012 iMac. Metal is the long-term API on
macOS. Linux retains its existing renderer selection. Launch and rendering on
the test Macs remain to be verified.
[Apple silicon porting guide](https://developer.apple.com/documentation/apple-silicon/porting-your-macos-apps-to-apple-silicon) ·
[eframe renderer features](https://github.com/emilk/egui/blob/0.33.3/crates/eframe/Cargo.toml)

## What is available

Variax Workbench supports offline editing. The Mac package omits the USB
feature and device actions show “The Mac connection isn't available in this
version yet”. The Mac file dialogs use macOS Standard Additions through
`osascript`, which avoids a dependency on save-panel APIs introduced after
Catalina. File types are checked when the app opens the selected path.

Rig Check still offers **Practice**. Choose **This Mac** for steps 0.1
(system and build information), 0.2 (USB and driver inventory from
`system_profiler`, `kextstat` and `systemextensionsctl`), 0.3 (offline engine
comparison, with the built byte hashes and any differences in the report)
and 8.1 (Core Audio device listing from `system_profiler`). It sends no device
commands and starts no audio stream. Reports default to
`~/Library/Application Support/Rig Check/reports`.

## Build on a Mac

Install Rust via rustup and Xcode Command Line Tools with a macOS SDK that
supports arm64 (SDK 11 or later). The tools provide Python 3.8 or newer.
Run this from the repository root:

```sh
packaging/macos/build.sh
```

Pass an output directory to use a location other than `dist/macos`. The
script downloads the Rust target libraries and Cargo dependencies as needed.
It uses hash-pinned wheels for the disk image's Finder layout. Those build
tools stay outside the apps. Each build puts its disk image and
`verification.txt` in a fresh output directory. The report records each
bundle's architecture, size and the raw `vtool -show-build` and `otool -L`
output for each slice. The build fails on an unexpected deployment target,
a non-system dylib, a broken signature or missing disk image contents.

## Build from Linux using GitHub Actions

From the repository checkout with the GitHub CLI authenticated, run:

```sh
gh workflow run macos-package.yml --ref master
gh run list --workflow macos-package.yml
gh run watch RUN_ID --exit-status
gh run download RUN_ID -n macos-universal -D macos-package
```

Replace `RUN_ID` with the ID from the list. The macOS runner tests the offline
package profile and runs the same build script. The artifact contains the disk
image and the verification report. A successful build verifies the package structure and links,
and checks command-line startup on the runner’s architecture. App launch, the Finder layout and dialogs on each target
macOS version require a separate smoke test.

To additionally publish a GitHub pre-release labelled **untested alpha**,
the owner supplies a positive alpha number:

```sh
gh workflow run macos-package.yml --ref master -f alpha_number=1
```

The tag uses the version in `Cargo.toml`, for example `v0.9.0-alpha.1`.
