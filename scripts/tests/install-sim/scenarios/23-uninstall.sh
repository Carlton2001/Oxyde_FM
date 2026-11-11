#!/usr/bin/env bash
# Full uninstall (asks about settings)
SIM_TITLE='Full uninstall (asks about settings)'
SIM_PREINSTALL=0.9.0
SIM_SETTINGS=1
SIM_DEFAULT=1
SIM_PORTAL=1
source "$(dirname "$0")/../lib.sh"
sim_main --uninstall "$@"
