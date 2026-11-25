#!/bin/sh
# Build a portable Linux Rig Check folder (#263). Nothing is published.
#
#   packaging/rig-check/build-linux.sh OUTPUT_DIR
#
# Creates OUTPUT_DIR/rig-check-<version>-linux-<arch>/ with the binary, a
# launcher that keeps every report inside that folder, the catalogue and the
# release notes. OUTPUT_DIR/<name> must not exist yet (nothing is replaced).
# The build records the git commit, compiler and target in the binary so the
# report header can quote them. It does not run the program.
set -eu

out=${1:?usage: build-linux.sh OUTPUT_DIR}
root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$root"

version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
arch=$(uname -m)
name="rig-check-$version-linux-$arch"
dest="$out/$name"
if [ -e "$dest" ]; then
    echo "$dest already exists; choose another OUTPUT_DIR." >&2
    exit 1
fi

commit=$(git rev-parse --short=12 HEAD 2>/dev/null || echo unknown)
if [ -n "$(git status --porcelain 2>/dev/null || true)" ]; then
    commit="$commit-modified"
fi
RIG_CHECK_GIT_COMMIT=$commit \
RIG_CHECK_RUSTC_VERSION=$(rustc --version) \
RIG_CHECK_TARGET=$(rustc -vV | sed -n 's/^host: //p') \
    cargo build --release --locked --offline --bin rig-check

mkdir -p "$out"
mkdir "$dest"
cp target/release/rig-check "$dest/rig-check"
cp docs/rig-check/catalogue.md "$dest/CATALOGUE.md"
cp docs/rig-check/coverage.md "$dest/coverage.md"
cp docs/rig-check/release-notes.md "$dest/RELEASE-NOTES.md"
cat > "$dest/rig-check.sh" <<'LAUNCHER'
#!/bin/sh
# Keeps reports and evidence inside this folder.
here=$(cd "$(dirname "$0")" && pwd)
exec "$here/rig-check" --out "$here/rig-check-output" "$@"
LAUNCHER
chmod 755 "$dest/rig-check.sh" "$dest/rig-check"
(cd "$dest" && sha256sum rig-check CATALOGUE.md coverage.md RELEASE-NOTES.md rig-check.sh > SHA256SUMS)
echo "$dest"
