#!/usr/bin/env bash
# gpgv missing: signature not checked
SIM_TITLE='gpgv missing: signature not checked'
SIM_MISSING='cmd gpgv'
source "$(dirname "$0")/../lib.sh"
sim_main "$@"
