#!/usr/bin/env bash
# Compile every Windows MSVC target from Linux; nothing is linked or run.
set -euo pipefail

usage() {
    printf 'Usage: bash scripts/check-windows.sh [--offline]\n'
    printf 'Checks all Windows targets with Cargo.lock through cargo-xwin, in a separate cache.\n'
    printf 'Tests still require ./scripts/test-windows.ps1 on Windows.\n'
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
    printf 'Run this check inside Linux; on Windows use scripts/test-windows.ps1.\n' >&2
    exit 1
fi

for tool in cargo cargo-xwin llvm-rc clang-cl llvm-lib lld-link; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        printf '%s is unavailable. See docs/guides/linux-development.md.\n' "$tool" >&2
        exit 1
    fi
done

project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd -- "$project_root"

# The Microsoft CRT and SDK download requires an explicit license acceptance once.
xwin_cache="${XWIN_CACHE_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/cargo-xwin}"
if [[ ! -d $xwin_cache/xwin && -z ${XWIN_ACCEPT_LICENSE:-} ]]; then
    printf 'No MSVC SDK in %s. Run once with XWIN_ACCEPT_LICENSE=1 to accept\n' "$xwin_cache" >&2
    printf 'the Microsoft CRT and Windows SDK license and download them.\n' >&2
    exit 1
fi

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/oxyde-gpui/target-win}"
export RC="$project_root/scripts/llvm-rc-crate-root.sh"
printf 'Windows build cache: %s\n' "$CARGO_TARGET_DIR"
cargo xwin check "${cargo_arguments[@]}" --target x86_64-pc-windows-msvc --all-targets
