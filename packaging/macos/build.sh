#!/bin/bash
set -euo pipefail

die() { printf '%s\n' "$*" >&2; exit 1; }

[[ "$(uname -s)" == Darwin ]] || die "Run this script on macOS with Rust and Xcode command-line tools."
profile=offline
features=gui,portable-math
if [[ "${1:-}" == --owner-usb ]]; then
    [[ $# -eq 2 ]] || die "Usage: $0 --owner-usb <private-output-directory>"
    profile=owner-usb
    features+=,usb
    shift
fi
[[ $# -le 1 ]] || die "Usage: $0 [output-directory] or $0 --owner-usb <private-output-directory>"
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
mkdir -p "${1:-$repo/dist/macos}"
output="$(cd "${1:-$repo/dist/macos}" && pwd)"
cd "$repo"

export RUSTUP_TOOLCHAIN="${RUSTUP_TOOLCHAIN:-1.99.1}"
export SDKROOT="$(xcrun --sdk macosx --show-sdk-path)"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$repo/target}"
python3 -c 'import sys; sys.exit(sys.version_info < (3, 8))' ||
    die "Python 3.8 or newer is required (provided by Xcode command-line tools)."
if ! rustc --version >/dev/null 2>&1; then
    rustup toolchain install "$RUSTUP_TOOLCHAIN" --profile minimal
fi
rustup target add x86_64-apple-darwin aarch64-apple-darwin

scratch="$(mktemp -d "$output/.build.XXXXXX")"
mount="$scratch/mount"
mounted=no
cleanup() {
    if [[ "$mounted" == yes ]]; then
        hdiutil detach "$mount" >/dev/null || return
    fi
    rm -rf "$scratch"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
stage="$scratch/stage"
mkdir -p "$stage" "$mount"
report="$scratch/verification.txt"
version="$(sed -n 's/^version = "\([0-9][0-9.]*\)"$/\1/p' Cargo.toml | head -n 1)"
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "Cannot read the package version from Cargo.toml."
revision="$(git rev-parse HEAD)"
{
    printf 'Commit %s\n' "$revision"
    printf 'Profile %s (%s)\n' "$profile" "$features"
    rustc -vV
    xcrun --sdk macosx --show-sdk-version
} | tee "$report"

build_slice() {
    local target="$1" arch="$2" minimum="$3" reported
    local -a cargo_options=(--locked --release --no-default-features --features "$features"
        --target "$target" --bin variax-workbench-gui --bin rig-check)
    export MACOSX_DEPLOYMENT_TARGET="$minimum"
    reported="$(rustc --print deployment-target --target "$target")"
    printf '%s %s\n' "$target" "$reported" | tee -a "$report"
    [[ "$reported" == "MACOSX_DEPLOYMENT_TARGET=$minimum" ]] ||
        die "$RUSTUP_TOOLCHAIN cannot target macOS $minimum on $target."
    export RIG_CHECK_GIT_COMMIT="$revision" RIG_CHECK_RUSTC_VERSION="$(rustc --version)" \
        RIG_CHECK_TARGET="$target"
    if [[ "$profile" == owner-usb ]]; then
        local cargo_log="$scratch/$arch-cargo.jsonl"
        cargo build "${cargo_options[@]}" --offline --message-format=json-render-diagnostics \
            > "$cargo_log"
        python3 "$here/package.py" verify-usb-archive "$arch" "$cargo_log" \
            "$CARGO_TARGET_DIR/$target/release" | tee -a "$report"
    else
        cargo build "${cargo_options[@]}"
    fi
}
build_slice x86_64-apple-darwin x86_64 10.15
build_slice aarch64-apple-darwin arm64 11.0

make_icon() {
    local source="$1" dest="$2" points
    local iconset="$scratch/AppIcon.iconset"
    mkdir "$iconset"
    for points in 16 32 128 256 512; do
        sips -z "$points" "$points" "$source" --out "$iconset/icon_${points}x${points}.png" >/dev/null
        sips -z "$((points * 2))" "$((points * 2))" "$source" \
            --out "$iconset/icon_${points}x${points}@2x.png" >/dev/null
    done
    iconutil --convert icns --output "$dest" "$iconset"
    rm -r "$iconset"
}

make_app() {
    local name="$1" binary="$2" identifier="$3" icon="$4"
    local app="$stage/$name.app"
    mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
    xcrun lipo -create \
        "$CARGO_TARGET_DIR/x86_64-apple-darwin/release/$binary" \
        "$CARGO_TARGET_DIR/aarch64-apple-darwin/release/$binary" \
        -output "$app/Contents/MacOS/$binary"
    chmod +x "$app/Contents/MacOS/$binary"
    make_icon "$here/$icon" "$app/Contents/Resources/AppIcon.icns"
    if [[ "$binary" == variax-workbench-gui ]]; then
        python3 "$here/package.py" artwork "$app"
    fi
    python3 "$here/package.py" plist "$app/Contents/Info.plist" "$name" "$binary" "$identifier" "$version"
    plutil -lint "$app/Contents/Info.plist"
    codesign --sign - --force --deep "$app"
}
make_app "Variax Workbench" variax-workbench-gui io.github.guirossibrum.variax-workbench workbench.png
make_app "Rig Check" rig-check io.github.guirossibrum.rig-check rig-check.png
cp "$here/How to open.txt" "$stage/How to open.txt"

# Build-only wheel dependencies, verified against the pinned hashes.
wheel() {
    local path="$1" digest="$2" file="$scratch/${1##*/}"
    curl -fsSL --retry 3 "https://files.pythonhosted.org/packages/$path" -o "$file"
    printf '%s  %s\n' "$digest" "$file" | shasum -a 256 -c
    python3 -m zipfile -e "$file" "$scratch/python"
}
wheel 07/9d/90db632f591f122caa510155f142811aab86731f9d13541ee8ba592cb3ce/dmgbuild-1.6.2-py3-none-any.whl \
    f78200b992027ccd2aa63872ed6ed11464e9149249303b8eee477e65280993d1
wheel 47/bf/b1c10362a0d670ee8ae086d92c3ab795fca2a927e4ff25e7cd15224d3863/ds_store-1.3.1-py3-none-any.whl \
    fbacbb0bd5193ab3e66e5a47fff63619f15e374ffbec8ae29744251a6c8f05b5
wheel 39/a1/4136777ed6a56df83e7c748ad28892f0672cbbcdc3b3d15a57df6ba72443/mac_alias-2.2.2-py3-none-any.whl \
    504ab8ac546f35bbd75ad014d6ad977c426660aa721f2cd3acf3dc2f664141bd

# dmgbuild uses the importlib.resources backport on Catalina's Python 3.8.
if python3 -c 'import sys; sys.exit(sys.version_info >= (3, 9))'; then
    wheel d3/91/4df247dd4da18b72b5bbabe1fa2b85029c34e1d6f0afdd6329d15d6bf2b5/importlib_resources-5.9.0-py3-none-any.whl \
        f78a8df21a79bcc30cfd400bdc38f314333de7c0fb619763f6b9dabab8268bb7
    wheel 62/8b/5ba542fa83c90e09eac972fc9baca7a88e7e7ca4b221a89251954019308b/zipp-3.20.2-py3-none-any.whl \
        a817ac80d6cf4b23bf7f2828b7cabf326f15a001bea8b1f9b49631780ba28350
fi

image="$scratch/Variax Workbench.dmg"
if [[ "$profile" == owner-usb ]]; then
    image="$scratch/Variax Workbench - Owner USB.dmg"
fi
PYTHONPATH="$scratch/python" python3 "$here/package.py" image "$image" "$stage" "$here/background.png"
hdiutil verify "$image"
hdiutil attach -readonly -nobrowse -mountpoint "$mount" "$image"
mounted=yes
[[ "$(readlink "$mount/Applications")" == /Applications ]]
cmp "$here/How to open.txt" "$mount/How to open.txt"
cmp "$here/background.png" "$mount/.background.png"
[[ -s "$mount/.DS_Store" ]]
codesign --verify --deep --strict "$mount/Variax Workbench.app"
codesign --verify --deep --strict "$mount/Rig Check.app"
python3 "$here/package.py" verify "$mount" "$scratch" "$image" "$profile" | tee -a "$report"
if [[ "$profile" == offline ]]; then
    "$mount/Variax Workbench.app/Contents/MacOS/variax-workbench-gui" --version | tee -a "$report"
    "$mount/Rig Check.app/Contents/MacOS/rig-check" --catalogue > "$scratch/catalogue.txt"
    grep -q '^0\.1 ' "$scratch/catalogue.txt"
    printf 'Command-line startup checks passed on %s\n' "$(uname -m)" | tee -a "$report"
fi
hdiutil detach "$mount"
mounted=no

# Publish only fully verified output, atomically and without overwriting prior builds.
release="$output/Variax-Workbench-$version-${revision:0:8}-${scratch##*.}"
if [[ "$profile" == owner-usb ]]; then
    release="$output/Owner-USB-$version-${revision:0:8}-${scratch##*.}"
fi
[[ ! -e "$release" ]] || die "The output directory already exists: $release"
mkdir "$scratch/package"
mv "$report" "$image" "$scratch/package/"
mv "$scratch/package" "$release"
printf '\nPackage and verification report: %s\n' "$release"
