#!/usr/bin/env bash
# Start one revision-pinned release build. Nothing is published by default.
set -euo pipefail

die() { printf '%s\n' "$*" >&2; exit 1; }
usage() {
    cat <<'USAGE'
Usage: packaging/build-release.sh [--revision COMMIT] [--alpha-number N]
       packaging/build-release.sh --owner-delivery [--revision COMMIT] [--mac-host HOST] [--output DIR]
       packaging/build-release.sh --owner-delivery-hosted [--revision COMMIT] [--mac-host HOST] [--output DIR]
       packaging/build-release.sh --retry-owner-dmg DIR [--mac-host HOST]
       packaging/build-release.sh --owner-usb [--revision COMMIT] [--mac-host HOST] [--output DIR]

The default builds one offline Linux, macOS and Windows release set on native
GitHub runners. The verified set is attached to the run. --alpha-number
explicitly publishes that verified set as an untested prerelease.

--owner-delivery builds one local set with Linux/macOS USB packages and a
Windows GNU offline ZIP, all from the same source and build number. It copies
the verified DMG to Mac Downloads after saving the complete local set.
--retry-owner-dmg rechecks a saved local set and retries only its DMG transfer.
--owner-delivery-hosted uses the native Actions Windows ZIP for an explicitly
authorized delivery instead.
--owner-usb builds just the private Linux/macOS pair and copies the verified
DMG to Mac Downloads. The owner modes run from Linux.
Build modes default to the freshly fetched origin/master commit.
USAGE
}

transfer_local_owner_dmg() {
    local directory=$1 digest=$2 filename mac_path
    filename="Variax Workbench (Build $number ${revision:0:8} Owner USB ${digest:0:12}).dmg"
    printf 'Transferring verified DMG to %s:~/Downloads/%s\n' "$mac_host" "$filename"
    if mac_path=$(deliver_owner_dmg "$mac_host" "$filename" "$digest" \
        < "$directory/macos/Variax Workbench - Owner USB.dmg") && [[ -n "$mac_path" ]]; then
        printf 'Mac DMG verified at %s:%s (SHA-256 %s)\n' "$mac_host" "$mac_path" "$digest"
    else
        printf 'Mac DMG transfer not confirmed. Verified local set retained at %s\n' "$directory" >&2
        printf 'Retry transfer only: bash %q --retry-owner-dmg %q --mac-host %q\n' \
            "$scripts/build-release.sh" "$directory" "$mac_host" >&2
        return 1
    fi
}

build_number() {
    local revision=$1 number
    if git cat-file -e "$revision:app/BUILD_NUMBER" 2>/dev/null; then
        number=$(git show "$revision:app/BUILD_NUMBER")
    else
        [[ "$(git rev-parse --is-shallow-repository)" == false ]] ||
            die 'A full Git history is required to number this release.'
        number=$(git rev-list --first-parent --count "$revision")
    fi
    [[ "$number" =~ ^[1-9][0-9]*$ ]] || die 'Invalid release build number.'
    printf '%s\n' "$number"
}

if [[ "${1:-}" == --print-build-number ]]; then
    [[ $# -eq 2 && "$2" =~ ^[0-9a-f]{40}$ ]] || die 'Expected a full Git commit SHA.'
    build_number "$2"
    exit
fi

revision_ref=
mode=hosted
alpha_number=
mac_host=
output=
retry_directory=
while (( $# )); do
    case "$1" in
        --help|-h) usage; exit 0 ;;
        --owner-usb|--owner-delivery|--owner-delivery-hosted)
            [[ "$mode" == hosted || "$mode" == "${1#--}" ]] || die 'Choose one owner mode.'
            mode=${1#--}
            shift ;;
        --retry-owner-dmg)
            [[ "$mode" == hosted ]] || die 'Choose one owner mode.'
            (( $# >= 2 )) && [[ -n "$2" && "$2" != --* ]] || die "Missing value for $1"
            mode=retry-owner-dmg
            retry_directory=$2
            shift 2 ;;
        --revision|--alpha-number|--mac-host|--output)
            (( $# >= 2 )) && [[ -n "$2" && "$2" != --* ]] || die "Missing value for $1"
            case "$1" in
                --revision) revision_ref=$2 ;;
                --alpha-number) alpha_number=$2 ;;
                --mac-host) mac_host=$2 ;;
                --output) output=$2 ;;
            esac
            shift 2 ;;
        *) die "Unknown argument: $1" ;;
    esac
done
scripts=$(cd "$(dirname -- "$0")" && pwd -P)
if [[ "$mode" == retry-owner-dmg ]]; then
    [[ -z "$revision_ref" && -z "$alpha_number" && -z "$output" ]] ||
        die '--retry-owner-dmg accepts only the saved local set and an optional --mac-host.'
    retry_directory=$(cd -- "$retry_directory" && pwd -P)
    revision=$(<"$retry_directory/REVISION")
    number=$(<"$retry_directory/BUILD_NUMBER")
    digest=$(VARIAX_BUILD_NUMBER=$number python3 "$scripts/release_manifest.py" \
        "$revision" "$retry_directory" --check-local-owner-delivery --print-owner-dmg-digest)
    mac_host=${mac_host:-guilhermes-macbook-pro-1}
    source "$scripts/deliver-owner-dmg.sh"
    transfer_local_owner_dmg "$retry_directory" "$digest"
    exit
fi

if [[ "$mode" != hosted ]]; then
    [[ -z "$alpha_number" ]] || die 'Private USB packages cannot be published by this command.'
else
    [[ -z "$mac_host" && -z "$output" ]] || die '--mac-host and --output apply to the owner modes.'
    [[ -z "$alpha_number" || "$alpha_number" =~ ^[1-9][0-9]*$ ]] || die 'Supply a positive alpha number.'
fi

repo=$(git rev-parse --show-toplevel)
if [[ -z "$revision_ref" || "$mode" == hosted || "$mode" == owner-delivery-hosted ]]; then
    git -C "$repo" fetch origin master
fi
revision=$(git -C "$repo" rev-parse --verify --end-of-options "${revision_ref:-refs/remotes/origin/master}^{commit}")
[[ "$revision" =~ ^[a-f0-9]{40}$ ]] || die 'Expected a full Git commit SHA.'
printf 'Release source: %s\n' "$revision"

if [[ "$mode" != hosted ]]; then
    number=$(build_number "$revision")
    mac_host=${mac_host:-guilhermes-macbook-pro-1}
    output=${output:-$repo/backups/packages}
    [[ "$output" == /* ]] || output="$PWD/$output"
    if [[ "$mode" == owner-usb ]]; then
        exec /bin/bash "$scripts/build-private-owner-usb.sh" "$revision" "$mac_host" "$output" "$number"
    fi
    [[ "$(uname -s)" == Linux ]] || die 'Start the combined owner delivery from Linux.'
fi

if [[ "$mode" != owner-delivery ]]; then
    git -C "$repo" merge-base --is-ancestor "$revision" refs/remotes/origin/master ||
        die 'The hosted release source must be on origin/master.'
    cd "$repo"
    request_id="$revision-$(date +%s)-$$-$RANDOM"
    dispatch=$(gh workflow run release-package.yml --ref master \
        --raw-field "revision=$revision" --raw-field "alpha_number=$alpha_number" \
        --raw-field "request_id=$request_id")
    printf '%s\n' "$dispatch"
    if [[ "$dispatch" =~ /actions/runs/([0-9]+) ]]; then
        run_id=${BASH_REMATCH[1]}
    else
        run_id=
        for attempt in {1..12}; do
            run_id=$(gh run list --workflow release-package.yml --event workflow_dispatch --limit 30 \
                --json databaseId,displayTitle \
                --jq ".[] | select(.displayTitle == \"Release $request_id\") | .databaseId")
            [[ -n "$run_id" ]] && break
            sleep 5
        done
    fi
    [[ "$run_id" =~ ^[0-9]+$ ]] || die "Could not identify the dispatched run for $request_id."
    printf 'Waiting for verified release run %s\n' "$run_id"
    if [[ "$mode" == hosted ]]; then
        exec gh run watch "$run_id" --exit-status --compact
    fi
    gh run watch "$run_id" --exit-status --compact
fi

umask 077
mkdir -p "$output"
stage=$(mktemp -d "$output/.owner-delivery.XXXXXXXX")
trap 'rm -rf -- "$stage"' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
export VARIAX_BUILD_NUMBER=$number
if [[ "$mode" == owner-delivery ]]; then
    cd "$repo"
    python3 "$scripts/build-windows-gnu.py" "$revision" "$stage/windows"
    manifest_mode=(--local-owner-delivery --print-owner-dmg-digest)
else
    gh run download "$run_id" --name "release-set-$revision" --dir "$stage/hosted"
    python3 "$scripts/release_manifest.py" "$revision" "$stage/hosted" --check
    mv -T -- "$stage/hosted/windows" "$stage/windows"
    manifest_mode=(--owner-delivery "$run_id")
fi

/bin/bash "$scripts/build-private-owner-usb.sh" "$revision" "$mac_host" "$stage" "$number" --stage-only
mv -T -- "$stage/windows" "$stage/pair/windows"
digest=$(python3 "$scripts/release_manifest.py" "$revision" "$stage/pair" "${manifest_mode[@]}")

delivery="$output/variax-workbench-owner-build-$number-${revision:0:8}-${stage##*.}"
mv -T --no-clobber -- "$stage/pair" "$delivery"
[[ ! -e "$stage/pair" ]] || die "Could not publish the owner set: $delivery"
printf 'Owner delivery (Build %s, %s): %s\n' "$number" "$revision" "$delivery"

if [[ "$mode" == owner-delivery ]]; then
    source "$scripts/deliver-owner-dmg.sh"
    transfer_local_owner_dmg "$delivery" "$digest"
fi
