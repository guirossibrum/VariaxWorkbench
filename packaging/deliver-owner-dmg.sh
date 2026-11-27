# Shared Downloads delivery for the private USB pair and the local owner set.
deliver_owner_dmg() {
    local host=$1 filename=$2 digest=$3 source=${4:-} script
    [[ "$host" =~ ^[a-zA-Z0-9][a-zA-Z0-9._@-]*$ ]] || {
        printf '%s\n' 'Use a valid SSH hostname or alias.' >&2
        return 1
    }
    script=$(cat <<'REMOTE'
set -euo pipefail
fail() { printf '%s\n' "$*" >&2; exit 1; }
downloads="$HOME/Downloads"
dest="$downloads/$1"
digest=$2
source=$3

matches_digest() {
    local checksum
    checksum=$(shasum -a 256 "$1") || return 1
    [[ "${checksum%% *}" == "$digest" ]]
}

[[ "$(uname -s)" == Darwin ]] || fail 'The delivery host must run macOS.'
[[ -d "$downloads" ]] || fail "Mac Downloads directory is missing: $downloads"
if [[ -e "$dest" || -L "$dest" ]]; then
    # A prior streamed transfer may have finished before SSH reported failure.
    if [[ -z "$source" && ! -L "$dest" && -f "$dest" ]] && matches_digest "$dest"; then
        printf '%s\n' "$dest"
        exit
    fi
    fail "Mac Downloads destination already exists: $dest"
fi

umask 077
temp=$(mktemp "$downloads/.variax-owner-dmg.XXXXXXXX")
trap 'rm -f -- "$temp"' EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM
if [[ -n "$source" ]]; then
    cp "$source" "$temp" || fail 'Could not stage the built DMG in Mac Downloads.'
else
    cat > "$temp" || fail 'Could not stage the streamed DMG in Mac Downloads.'
fi
matches_digest "$temp" || fail 'Mac Downloads staged DMG checksum mismatch.'
# link(2) atomically refuses any existing destination, including a directory.
# shasum already requires the system Perl.
perl -e 'link($ARGV[0], $ARGV[1]) or die "Could not create $ARGV[1]: $!\n"' "$temp" "$dest"
matches_digest "$dest" || fail "Mac Downloads final DMG checksum mismatch: $dest"
printf '%s\n' "$dest"
REMOTE
    )
    # OpenSSH sends a shell command string. Use portable single-quote escaping
    # so the remote login shell preserves the script and every argument.
    local command='/bin/bash -c' arg
    for arg in "$script" -- "$filename" "$digest" "$source"; do
        command+=" '${arg//\'/\'\\\'\'}'"
    done
    ssh -T -o BatchMode=yes -o ConnectTimeout=10 "$host" "$command"
}
