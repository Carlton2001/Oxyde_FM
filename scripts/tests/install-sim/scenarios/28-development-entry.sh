#!/usr/bin/env bash
# --uninstall facing the development script's entry
SIM_TITLE='--uninstall facing the development script'\''s entry'
SIM_DEV_ENTRY=1
source "$(dirname "$0")/../lib.sh"
sim_main --uninstall "$@"
