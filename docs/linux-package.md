# Linux offline package

The package contains the Rust editor, three companion command-line tools, an
original SVG icon, desktop/MIME definitions, documentation, dependency notices
and a digest manifest. It contains no Line 6 installers, images, presets, model
definitions, captures, firmware, drivers or updater services. Physical access
remains locked in the compiled source. Installing does not authorize contact
with equipment.

## Build

Use a clean committed Linux checkout, Rust 1.99.1, Python 3.11+ and normal GUI
build prerequisites. Prepare the public Cargo cache with `cargo fetch --locked`.
The build itself uses `--offline --locked`, without a dependency-network fallback.

```sh
python scripts/package_linux.py --output .tools/packages/workbench-host.tar.gz
```

The default preserves host math. Select the experimental software profile with
`--math-profile software`; see [math profiles](math-profiles.md). The compiled
profile, Git revision, compiler, target triple and file hashes are recorded.
Output must be a new archive path. Hardware locks are checked before compilation;
the builder runs no hardware command.

Archive ordering, ownership, modes and timestamps are normalized. Identical
payloads and `SOURCE_DATE_EPOCH` produce identical archive bytes. Different
compilers/system libraries/build hosts need not emit identical binaries. The
default epoch is the source commit time. Packages target the build host's Linux
ABI/architecture; older libc installations may need a build on their own
baseline. Desktop/graphics/portal interfaces remain OS dependencies. Ordinary
Rust operation needs neither Python nor the research environment.

## Install and remove

Extract the generated archive into a new directory. From its package directory:

```sh
python install.py install --prefix "$HOME/.local"
```

Only the explicit manifest is installed. Existing files are never replaced;
no service, driver or shell configuration is installed. The desktop entry uses
the installed executable's absolute path, independently of the session's `PATH`.
Spaces in prefixes work. Control characters, `=` and `%` are refused to avoid
invalid/ambiguous launcher commands. Quoting follows the
[Desktop Entry Exec specification](https://specifications.freedesktop.org/desktop-entry/latest/exec-variables.html).
The receipt records the rewritten desktop file's actual digest.

`$HOME/.local/share/applications` is the usual per-user menu location. A different
prefix must be in the desktop's data search path for menu discovery; its binaries
can always be run directly. If available, refresh MIME/desktop caches after both
installation and removal:

```sh
update-mime-database "$HOME/.local/share/mime"
update-desktop-database "$HOME/.local/share/applications"
```

Those programs are not run automatically. Retain the extracted package to remove
the installation with:

```sh
python install.py uninstall --prefix "$HOME/.local"
```

Removal verifies recorded contents and modes before deleting anything. A changed
or missing file stops removal for review. Only recorded files and empty
directories are removed. User presets/artwork, unrelated applications and
generated caches survive. A late installation failure rolls back only unchanged
files created by that attempt. Updating means removing the recorded package,
then installing the new one.

## Fresh startup and optional artwork

The GUI opens a welcome screen without arguments. It does not look for a
developer-checkout default preset. Open a `.vwm`, `.vwb` or `.l6c` using Open,
a file association or one file argument. Parameter editing works without images;
missing artwork is informational and does not change the document.

```sh
variax-workbench-gui "a preset.vwm"
variax-workbench-gui --preset "a bank.vwb" --artwork /ABS/EXTRACTED/EDITOR/IMAGES
```

The optional image lookup is `DATA_DIR/artwork`. Linux defaults to
`$XDG_DATA_HOME/variax-workbench` or `$HOME/.local/share/variax-workbench`.
Absolute `VARIAX_WORKBENCH_DATA_DIR` overrides that default. `--data-dir` selects
another location; `--artwork` overrides image lookup regardless of option order.
Launch does not create the directory. Windows/macOS fallbacks are defined;
their desktop packages and native GUI behavior are not validated here.

Extract images privately with the [reference recipe](reproducing-references.md).
Do not add them to a redistributed package. Coefficient rendering with original
model resources still requires those separately supplied files; the package
does not fabricate factory defaults or an audio engine.

## Validation scope

Eight synthetic package guards check collisions, tampering, late-failure
rollback, modified-file retention, user-data preservation, symlink redirection,
path namespaces, absolute launcher paths and repeatable archives. Dummy binaries
are never executed. Headless GUI tests cover welcome startup without assets,
opening a file, positional arguments, artwork overrides and protected PNG output.
Actual build/staging and desktop validation are recorded at issue completion.
These tests do not establish device connectivity, physical safety, sound
equivalence, recording or firmware recovery. README device restrictions apply.
