#!/usr/bin/env bash
# Uninstall while Oxyde runs, without a terminal: stops before any change
SIM_TITLE='Uninstall while Oxyde runs, without a terminal'
SIM_PREINSTALL=0.9.0
SIM_SETTINGS=1
SIM_RUNNING=0
source "$(dirname "$0")/../lib.sh"
sim_main --uninstall "$@" < /dev/null 2>&1 | cat
