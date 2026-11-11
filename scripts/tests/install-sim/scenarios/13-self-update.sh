#!/usr/bin/env bash
# Self-update run by Oxyde (--archive --update)
SIM_TITLE='Self-update run by Oxyde (--archive --update)'
SIM_PREINSTALL=0.8.0
SIM_LOCAL_ARCHIVE=1
SIM_MISSING='cmd bwrap
lib libpoppler-glib.so.8
gvfs
cmd wl-copy'
source "$(dirname "$0")/../lib.sh"
sim_main --archive "$HOME/Downloads/oxyde-0.9.0-x86_64-linux.tar.gz" --update "$@"
