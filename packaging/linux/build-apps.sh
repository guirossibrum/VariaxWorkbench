#!/usr/bin/env bash
# Native worker. The release command or workflow supplies the exact checkout.
set -euo pipefail

[[ $# -eq 3 ]] || exit 2
revision=$1
profile=$2
output=$3
[[ "$(uname -s)" == Linux && "$(git rev-parse HEAD)" == "$revision" ]]
[[ "$revision" =~ ^[0-9a-f]{40}$ ]]
[[ -z "$(git status --porcelain)" ]]
features=gui,portable-math
case "$profile" in
    offline) export VARIAX_LOG_PROFILE=v1 ;;
    owner-usb) features+=,usb; export VARIAX_LOG_PROFILE=owner ;;
    *) exit 2 ;;
esac
export RUSTUP_TOOLCHAIN=1.99.1 CARGO_NET_OFFLINE=true
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$PWD/target}"
mkdir -p "$output" "$CARGO_TARGET_DIR"
exec 9>"$CARGO_TARGET_DIR/.pair.lock"
flock 9
export RIG_CHECK_GIT_COMMIT="$revision" RIG_CHECK_RUSTC_VERSION="$(rustc --version)"
export RIG_CHECK_TARGET="$(rustc -vV | sed -n 's/^host: //p')"
cargo build --offline --locked --release --no-default-features --features "$features" \
    --bin rig-check --bin variax-workbench-gui
install -m 755 "$CARGO_TARGET_DIR/release/rig-check" "$output/"
install -m 755 "$CARGO_TARGET_DIR/release/variax-workbench-gui" "$output/"
mkdir -p "$output/assets/ui"
cp -R assets/ui/{shell,pod-farm,SOURCES.md} "$output/assets/ui/"
cp docs/rig-check/{catalogue,coverage,release-notes}.md "$output/"
grep -aFq "$revision" "$output/rig-check"
printf 'Commit %s\nProfile %s (%s)\n%s\nTarget %s\n' \
    "$revision" "$profile" "$features" "$RIG_CHECK_RUSTC_VERSION" "$RIG_CHECK_TARGET" > "$output/verification.txt"
if [[ -n "${VARIAX_BUILD_NUMBER:-}" ]]; then
    printf 'Build number %s\n' "$VARIAX_BUILD_NUMBER" >> "$output/verification.txt"
fi
