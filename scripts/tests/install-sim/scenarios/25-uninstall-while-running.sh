#!/usr/bin/env bash
# Uninstall while Oxyde runs
SIM_TITLE='Uninstall while Oxyde runs'
SIM_PREINSTALL=0.9.0
SIM_SETTINGS=1
SIM_RUNNING=1
source "$(dirname "$0")/../lib.sh"
sim_main --uninstall "$@"
