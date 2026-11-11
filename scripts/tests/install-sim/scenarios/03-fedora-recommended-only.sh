#!/usr/bin/env bash
# Fedora, only recommended dependencies missing
SIM_TITLE='Fedora, only recommended dependencies missing'
SIM_OS=fedora
SIM_MISSING='lib libpoppler-glib.so.8
lib libgstapp-1.0.so.0
cmd udisksctl'
source "$(dirname "$0")/../lib.sh"
sim_main "$@"
