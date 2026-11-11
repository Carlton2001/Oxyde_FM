#!/usr/bin/env bash
# Uninstall with --purge
SIM_TITLE='Uninstall with --purge'
SIM_PREINSTALL=0.9.0
SIM_SETTINGS=1
source "$(dirname "$0")/../lib.sh"
sim_main --uninstall --purge "$@"
