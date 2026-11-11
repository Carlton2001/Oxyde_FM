#!/usr/bin/env bash
# glibc too old (2.35)
SIM_TITLE='glibc too old (2.35)'
SIM_GLIBC=2.35
source "$(dirname "$0")/../lib.sh"
sim_main "$@"
