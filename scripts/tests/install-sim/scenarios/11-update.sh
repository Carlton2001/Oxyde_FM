#!/usr/bin/env bash
# Update from 0.8.0 to 0.9.0
SIM_TITLE='Update from 0.8.0 to 0.9.0'
SIM_PREINSTALL=0.8.0
SIM_SETTINGS=1
source "$(dirname "$0")/../lib.sh"
sim_main "$@"
