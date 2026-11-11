#!/usr/bin/env bash
# --version, --yes, --set-default
SIM_TITLE='--version, --yes, --set-default'
SIM_MISSING='cmd bwrap
lib libpoppler-glib.so.8
gvfs
cmd wl-copy'
source "$(dirname "$0")/../lib.sh"
sim_main --version 0.9.0 --yes --set-default "$@"
