#!/usr/bin/env bash
# A second update without restarting Oxyde keeps the version it runs from
SIM_TITLE='Two updates while Oxyde runs from the oldest version'
SIM_PREINSTALL='0.8.0 0.8.5'
SIM_RUNNING=0
source "$(dirname "$0")/../lib.sh"
sim_main --yes "$@"
