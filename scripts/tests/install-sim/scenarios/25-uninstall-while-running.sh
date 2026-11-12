#!/usr/bin/env bash
# Uninstall while Oxyde runs: waits until the user closes it, then removes everything
SIM_TITLE='Uninstall while Oxyde runs (closed after 3 s)'
SIM_PREINSTALL=0.9.0
SIM_SETTINGS=1
SIM_RUNNING=3
source "$(dirname "$0")/../lib.sh"
sim_main --uninstall "$@"
