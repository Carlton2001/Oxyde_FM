#!/usr/bin/env bash
# Check a file's Tauri updater signature (<file>.sig, base64 of a minisign signature)
# against the public key compiled into Oxyde (PUBLIC_KEY in src/systems/updates.rs).
# Needs minisign. Usage: bash scripts/check-updater-signature.sh <file>
set -euo pipefail

if [[ $# -ne 1 ]]; then
    printf 'Usage: %s <file>\n' "$0" >&2
    exit 2
fi
file=$1
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
key=$(sed -n 's/^const PUBLIC_KEY: &str = "\([A-Za-z0-9+\/=]*\)";$/\1/p' "$root/src/systems/updates.rs")
[[ -n $key ]] || { printf 'PUBLIC_KEY not found in src/systems/updates.rs.\n' >&2; exit 1; }
[[ -f $file.sig ]] || { printf 'Missing %s.sig.\n' "$file" >&2; exit 1; }

work=$(mktemp -d)
trap 'rm -rf -- "$work"' EXIT
printf '%s' "$key" | base64 -d > "$work/updater.pub"
base64 -d < "$file.sig" > "$work/signature.minisig"
if ! minisign -Vq -p "$work/updater.pub" -m "$file" -x "$work/signature.minisig"; then
    printf '%s is not signed with the updater key Oxyde trusts: check TAURI_SIGNING_PRIVATE_KEY.\n' "$file" >&2
    exit 1
fi
printf 'Updater signature of %s matches the key compiled into Oxyde.\n' "$(basename -- "$file")"
