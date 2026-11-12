#!/usr/bin/env bash
# Uninstall while Oxyde quits after an update and starts its new version
SIM_TITLE='Uninstall while Oxyde relaunches after an update'
SIM_PREINSTALL=0.9.0
SIM_SETTINGS=1
SIM_RUNNING=2
SIM_RELAUNCH=1
source "$(dirname "$0")/../lib.sh"
sim_main --uninstall "$@"
