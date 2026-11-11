#!/usr/bin/env bash
# --uninstall with only the system package
SIM_TITLE='--uninstall with only the system package'
SIM_SYSTEM_PKG=1
source "$(dirname "$0")/../lib.sh"
sim_main --uninstall "$@"
