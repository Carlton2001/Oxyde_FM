#!/usr/bin/env bash
# Fedora, bubblewrap and recommended ones missing (dnf offered)
SIM_TITLE='Fedora, bubblewrap and recommended ones missing (dnf offered)'
SIM_OS=fedora
SIM_MISSING='cmd bwrap
lib libpoppler-glib.so.8
gvfs
cmd wl-copy'
source "$(dirname "$0")/../lib.sh"
sim_main "$@"
