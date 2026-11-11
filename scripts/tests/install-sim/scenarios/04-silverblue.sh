#!/usr/bin/env bash
# Fedora Silverblue (image-based), rpm-ostree offered
SIM_TITLE='Fedora Silverblue (image-based), rpm-ostree offered'
SIM_OS=silverblue
SIM_MISSING='cmd bwrap
lib libpoppler-glib.so.8
gvfs
cmd wl-copy'
source "$(dirname "$0")/../lib.sh"
sim_main "$@"
