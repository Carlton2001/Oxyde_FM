#!/usr/bin/env bash
# Arch, missing dependencies (pacman)
SIM_TITLE='Arch, missing dependencies (pacman)'
SIM_OS=arch
SIM_MISSING='cmd bwrap
lib libpoppler-glib.so.8
gvfs
cmd wl-copy'
source "$(dirname "$0")/../lib.sh"
sim_main "$@"
