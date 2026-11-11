#!/usr/bin/env bash
# Uninstall while a system package also exists
SIM_TITLE='Uninstall while a system package also exists'
SIM_PREINSTALL=0.9.0
SIM_SETTINGS=1
SIM_SYSTEM_PKG=1
SIM_DEFAULT=1
source "$(dirname "$0")/../lib.sh"
sim_main --uninstall "$@"
