#!/usr/bin/env bash
# embed-resource runs the resource compiler from the .rc directory, while GPUI and
# Oxyde resources name files relative to their crate root: search that root too.
set -euo pipefail

crate=$PWD
while [[ $crate != / && ! -f $crate/Cargo.toml ]]; do
    crate=$(dirname -- "$crate")
done
exec llvm-rc /I "$crate" "$@"
