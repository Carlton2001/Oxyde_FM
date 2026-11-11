#!/usr/bin/env bash
# SHA256SUMS signed by another key
SIM_TITLE='SHA256SUMS signed by another key'
SIM_GPG=bad
source "$(dirname "$0")/../lib.sh"
sim_main "$@"
