#!/usr/bin/env bash
# openSUSE, missing dependencies (zypper)
SIM_TITLE='openSUSE, missing dependencies (zypper)'
SIM_OS=opensuse
SIM_MISSING='cmd bwrap
lib libpoppler-glib.so.8
gvfs
cmd wl-copy'
source "$(dirname "$0")/../lib.sh"
sim_main "$@"
