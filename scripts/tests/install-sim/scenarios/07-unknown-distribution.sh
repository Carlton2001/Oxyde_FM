#!/usr/bin/env bash
# Unknown distribution (Void): Fedora names and a warning
SIM_TITLE='Unknown distribution (Void): Fedora names and a warning'
SIM_OS=void
SIM_MISSING='cmd bwrap
lib libpoppler-glib.so.8
gvfs
cmd wl-copy'
source "$(dirname "$0")/../lib.sh"
sim_main "$@"
