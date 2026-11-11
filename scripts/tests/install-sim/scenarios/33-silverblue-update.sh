#!/usr/bin/env bash
# Silverblue, update from 0.8.0 to 0.9.0, components missing
SIM_TITLE='Silverblue, update from 0.8.0 to 0.9.0, components missing'
SIM_OS=silverblue SIM_PREINSTALL=0.8.0 SIM_SETTINGS=1
SIM_MISSING=$'lib libpoppler-glib.so.8\ncmd wl-copy'
source "$(dirname "$0")/../lib.sh"
sim_main "$@"
