#!/usr/bin/env bash
# Start one revision-pinned release build. Nothing is published by default.
set -euo pipefail

die() { printf '%s\n' "$*" >&2; exit 1; }
usage() {
    cat <<'USAGE'
Usage: packaging/build-release.sh [--revision COMMIT] [--alpha-number N]
       packaging/build-release.sh --owner-usb [--revision COMMIT] [--mac-host HOST] [--output DIR]

The default builds one offline Linux, macOS and Windows release set on native
GitHub runners. The verified set is attached to the run. --alpha-number
explicitly publishes that verified set as an untested prerelease.

--owner-usb builds the private Linux and macOS USB packages together from
Linux and copies the verified DMG to Mac Downloads. Windows USB is pending.
Both modes default to the freshly fetched origin/master commit.
USAGE
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
owner_usb=false
alpha_number=
mac_host=
output=
while (( $# )); do
    case "$1" in
        --help|-h) usage; exit 0 ;;
        --owner-usb) owner_usb=true; shift ;;
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
if "$owner_usb"; then
    [[ -z "$alpha_number" ]] || die 'Private USB packages cannot be published by this command.'
else
    [[ -z "$mac_host" && -z "$output" ]] || die '--mac-host and --output apply to --owner-usb.'
    [[ -z "$alpha_number" || "$alpha_number" =~ ^[1-9][0-9]*$ ]] || die 'Supply a positive alpha number.'
fi

repo=$(git rev-parse --show-toplevel)
if [[ -z "$revision_ref" ]] || ! "$owner_usb"; then
    git -C "$repo" fetch origin master
fi
revision=$(git -C "$repo" rev-parse --verify --end-of-options "${revision_ref:-refs/remotes/origin/master}^{commit}")
[[ "$revision" =~ ^[a-f0-9]{40}$ ]] || die 'Expected a full Git commit SHA.'
printf 'Release source: %s\n' "$revision"

if "$owner_usb"; then
    number=$(build_number "$revision")
    exec /bin/bash "$(dirname -- "$0")/build-private-owner-usb.sh" "$revision" \
        "${mac_host:-guilhermes-macbook-pro-1}" "${output:-$repo/backups/packages}" "$number"
fi

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
exec gh run watch "$run_id" --exit-status --compact
