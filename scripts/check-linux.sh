#!/usr/bin/env bash
# Compile every Linux target without running filesystem or desktop operations.
set -euo pipefail

usage() {
    printf 'Usage: bash scripts/check-linux.sh [--offline]\n'
    printf 'Checks all targets with Cargo.lock, using a separate Linux build cache.\n'
}

cargo_arguments=(--locked)
for argument in "$@"; do
    case "$argument" in
        --offline) cargo_arguments+=(--offline) ;;
        --help|-h) usage; exit 0 ;;
        *) printf 'Unknown argument: %s\n' "$argument" >&2; usage >&2; exit 2 ;;
    esac
done

if [[ $(uname -s) != Linux ]]; then
    printf 'Run this check inside Linux (including Fedora WSL).\n' >&2
    exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
    printf 'Cargo is unavailable. Add the Linux Rust toolchain to PATH first.\n' >&2
    exit 1
fi

project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd -- "$project_root"

# Keep Linux artifacts off the Windows target directory and, in WSL, off DrvFS.
# Honor an explicitly selected cache, for example in CI or another checkout.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/oxyde-gpui/target}"
printf 'Linux build cache: %s\n' "$CARGO_TARGET_DIR"
cargo check "${cargo_arguments[@]}" --all-targets
