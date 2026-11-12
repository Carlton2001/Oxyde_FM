#!/usr/bin/env bash
# Uninstall while Oxyde runs, cancelled with q: nothing changes
SIM_TITLE='Uninstall while Oxyde runs, cancelled'
SIM_PREINSTALL=0.9.0
SIM_SETTINGS=1
SIM_RUNNING=0
source "$(dirname "$0")/../lib.sh"
sim_main --uninstall "$@"
