#!/usr/bin/env bash
# Build a private, matching Linux and macOS pair from one Git commit.
set -euo pipefail
umask 077

die() { printf '%s\n' "$*" >&2; exit 1; }
[[ $# -eq 4 || ( $# -eq 5 && "$5" == --stage-only ) ]] ||
    die 'Use packaging/build-release.sh --owner-usb or --owner-delivery.'
revision=$1
mac_host=$2
output=$3
export VARIAX_BUILD_NUMBER=$4
stage_only=${5:-}
[[ "$(uname -s)" == Linux ]] || die 'Start the private USB build from Linux.'
[[ "$mac_host" =~ ^[a-zA-Z0-9][a-zA-Z0-9._@-]*$ ]] || die 'Use a valid SSH hostname or alias.'
repo=$(git rev-parse --show-toplevel)
scripts=$(cd "$(dirname -- "$0")" && pwd -P)
worker="$scripts/linux/build-apps.sh"
[[ "$revision" =~ ^[a-f0-9]{40}$ ]] || die 'Expected the pinned source commit.'
ssh_options=(-o BatchMode=yes -o ConnectTimeout=10)
short=${revision:0:8}
toolchain=1.99.1
features=gui,portable-math,usb

mkdir -p "$output" "$repo/.worktrees" "$repo/target/shared-builds/owner-pair"
output=$(cd "$output" && pwd -P)
stage=$(mktemp -d "$output/.owner-pair-$short.XXXXXXXX")
build_id=${stage##*.}
worktree="$repo/.worktrees/owner-pair-$short-$build_id"
remote=
cleanup() {
    local status=$?
    if [[ -n "$remote" ]]; then
        ssh "${ssh_options[@]}" "$mac_host" /bin/bash -s -- "$remote" <<'REMOTE' || true
set -eu
[[ "$1" == "${TMPDIR:-/tmp}"* && "$1" =~ /\.variax-owner-pair\.[A-Za-z0-9]+$ ]]
rm -rf -- "$1"
REMOTE
    fi
    if [[ -d "$worktree" ]]; then
        git -C "$repo" worktree remove "$worktree" || true
    fi
    rm -rf -- "$stage"
    exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

git -C "$repo" worktree add --detach "$worktree" "$revision"
[[ "$(git -C "$worktree" rev-parse HEAD)" == "$revision" && -z "$(git -C "$worktree" status --porcelain)" ]]
version=$(sed -n 's/^version = "\([0-9][0-9.]*\)"$/\1/p' "$worktree/Cargo.toml")
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die 'Cannot read the application version.'
pair_name="variax-workbench-$version-linux-macos-$short-$build_id"
release="$output/$pair_name"
linux="$stage/pair/linux"
macos="$stage/pair/macos"
mkdir -p "$linux" "$macos" "$stage/inputs"

remote=$(ssh "${ssh_options[@]}" "$mac_host" /bin/bash -s -- "${stage_only:-deliver}" <<'REMOTE'
set -euo pipefail
[[ "$(uname -s)" == Darwin ]]
if [[ "$1" == deliver ]]; then
    test -d "$HOME/Downloads"
fi
xcrun --sdk macosx --show-sdk-version >/dev/null
umask 077
mktemp -d "${TMPDIR:-/tmp}/.variax-owner-pair.XXXXXXXX"
REMOTE
)
[[ "$remote" =~ ^/[A-Za-z0-9._/-]+/\.variax-owner-pair\.[A-Za-z0-9]+$ ]] ||
    die 'Mac returned an unexpected temporary path.'

printf 'Building Linux Workbench and Rig Check from %s…\n' "$revision"
(
    cd "$worktree"
    export CARGO_TARGET_DIR="$repo/target/shared-builds/owner-pair"
    /bin/bash "$worker" "$revision" owner-usb "$linux"
) > "$stage/pair/linux-build.log" 2>&1 || {
    tail -60 "$stage/pair/linux-build.log" >&2
    die 'Linux build failed.'
}

printf 'Preparing exact source and locked Cargo dependencies for the Mac…\n'
(
    cd "$worktree"
    export RUSTUP_TOOLCHAIN="$toolchain"
    cargo vendor --offline --locked "$stage/inputs/vendor" > /dev/null 2>"$stage/vendor.log"
) || { tail -40 "$stage/vendor.log" >&2; die 'Cargo cache is missing locked dependencies.'; }
(
    cd "$stage/inputs"
    git -C "$worktree" rev-list --objects --no-object-names --no-walk "$revision" |
        git -C "$worktree" pack-objects --stdout > source.pack
    git -C "$worktree" rev-parse 'HEAD^{tree}' > TREE
    tar -czf vendor.tar.gz vendor
    printf '%s\n' "$revision" > REVISION
    sha256sum source.pack vendor.tar.gz TREE REVISION > INPUT-SHA256SUMS
)
scp "${ssh_options[@]}" "$stage/inputs/"{source.pack,vendor.tar.gz,TREE,REVISION,INPUT-SHA256SUMS} "$mac_host:$remote/"

printf 'Building and verifying the universal macOS DMG on %s…\n' "$mac_host"
ssh "${ssh_options[@]}" "$mac_host" /bin/bash -s -- "$remote" "$toolchain" "$revision" "$version" "$VARIAX_BUILD_NUMBER" \
    > "$stage/pair/macos-build.log" 2>&1 <<'REMOTE' || {
set -euo pipefail
umask 077
cd "$1"
scratch="$PWD"
export RUSTUP_TOOLCHAIN="$2"
revision="$3"
version="$4"
export VARIAX_BUILD_NUMBER="$5"
shasum -a 256 -c INPUT-SHA256SUMS
test "$(cat REVISION)" = "$revision"
mkdir source output
git -C source init --quiet --template=
git -C source index-pack --stdin < source.pack
printf '%s\n' "$revision" > source/.git/shallow
git -C source -c advice.detachedHead=false checkout --quiet --detach "$revision"
test "$(git -C source rev-parse HEAD)" = "$revision"
test "$(git -C source rev-parse 'HEAD^{tree}')" = "$(cat TREE)"
git -C source fsck --full --no-reflogs
test -z "$(git -C source status --porcelain)"

tar -xzf vendor.tar.gz
export CARGO_HOME="$scratch/cargo-home" RUSTUP_HOME="$scratch/rustup"
export CARGO_TARGET_DIR="$scratch/target" CARGO_NET_OFFLINE=true
mkdir -p "$CARGO_HOME"
export PATH="$CARGO_HOME/bin:/usr/bin:/bin:/usr/sbin:/sbin"
cat > "$CARGO_HOME/config.toml" <<CONFIG
[source.crates-io]
replace-with = "vendored"
[source.vendored]
directory = "$scratch/vendor"
CONFIG
case "$(uname -m)" in
    arm64) bootstrap_arch=aarch64 ;;
    x86_64) bootstrap_arch=x86_64 ;;
    *) exit 1 ;;
esac
url="https://static.rust-lang.org/rustup/dist/$bootstrap_arch-apple-darwin/rustup-init"
curl -fsSL --retry 2 "$url" -o rustup-init
curl -fsSL --retry 2 "$url.sha256" -o rustup-init.sha256
printf '%s  rustup-init\n' "$(awk 'NR == 1 { print $1 }' rustup-init.sha256)" | shasum -a 256 -c -
chmod 700 rustup-init
./rustup-init -y --no-modify-path --profile minimal --default-toolchain "$RUSTUP_TOOLCHAIN"
/bin/bash source/packaging/macos/build.sh --owner-usb "$scratch/output"
test -z "$(git -C source status --porcelain)"

packages=(output/Owner-USB-"$version"-"${revision:0:8}"-*)
[[ "${#packages[@]}" == 1 && -d "${packages[0]}" ]]
grep -Fxq "Commit $revision" "${packages[0]}/verification.txt"
cp "${packages[0]}/verification.txt" verification.txt
cp "${packages[0]}/Variax Workbench - Owner USB.dmg" owner.dmg
hdiutil verify owner.dmg
shasum -a 256 owner.dmg > DMG.sha256
REMOTE
    tail -60 "$stage/pair/macos-build.log" >&2
    die 'macOS build or package verification failed.'
}
scp "${ssh_options[@]}" "$mac_host:$remote/owner.dmg" "$mac_host:$remote/verification.txt" \
    "$mac_host:$remote/DMG.sha256" "$macos/"
grep -Fxq "Commit $revision" "$macos/verification.txt"
grep -Fxq "Commit $revision" "$linux/verification.txt"
grep -Fxq "Profile owner-usb ($features)" "$macos/verification.txt"
grep -Fxq "Build number $VARIAX_BUILD_NUMBER" "$linux/verification.txt"
if ! git -C "$repo" cat-file -e "$revision:app/BUILD_NUMBER" 2>/dev/null; then
    grep -Fxq "Build number $VARIAX_BUILD_NUMBER" "$macos/verification.txt"
fi
(cd "$macos" && sha256sum -c DMG.sha256)
digest=$(awk '{ print $1 }' "$macos/DMG.sha256")
[[ "$digest" =~ ^[0-9a-f]{64}$ ]]
mv "$macos/owner.dmg" "$macos/Variax Workbench - Owner USB.dmg"
rm "$macos/DMG.sha256"
printf '%s\n' "$revision" > "$stage/pair/REVISION"
printf '%s\n' "$VARIAX_BUILD_NUMBER" > "$stage/pair/BUILD_NUMBER"
(
    cd "$stage/pair"
    sha256sum REVISION BUILD_NUMBER linux/{rig-check,variax-workbench-gui,catalogue.md,coverage.md,release-notes.md,verification.txt} \
        linux/assets/ui/{shell/*.png,pod-farm/*.png,SOURCES.md} \
        'macos/Variax Workbench - Owner USB.dmg' macos/verification.txt > SHA256SUMS
    sha256sum -c SHA256SUMS
)

if [[ "$stage_only" == --stage-only ]]; then
    mv -T --no-clobber -- "$stage/pair" "$output/pair"
    [[ ! -e "$stage/pair" ]] || die 'Could not stage the private pair.'
    exit
fi

printf 'Delivering the verified DMG to Mac Downloads…\n'
source "$scripts/deliver-owner-dmg.sh"
deliver_owner_dmg "$mac_host" "Variax Workbench $version ($revision Owner USB $build_id).dmg" \
    "$digest" "$remote/owner.dmg" </dev/null > "$stage/pair/mac-download.txt"
[[ ! -e "$release" && ! -L "$release" ]] || die "Output already exists: $release"
mv -T --no-clobber "$stage/pair" "$release"
[[ ! -e "$stage/pair" ]] || die "Could not publish the local pair: $release"
printf 'Commit %s\nLinux apps and local DMG: %s\nMac: %s\n' \
    "$revision" "$release" "$(cat "$release/mac-download.txt")"
