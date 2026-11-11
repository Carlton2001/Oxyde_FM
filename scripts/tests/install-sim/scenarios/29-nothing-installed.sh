#!/usr/bin/env bash
# --uninstall with nothing installed
SIM_TITLE='--uninstall with nothing installed'
source "$(dirname "$0")/../lib.sh"
sim_main --uninstall "$@"
