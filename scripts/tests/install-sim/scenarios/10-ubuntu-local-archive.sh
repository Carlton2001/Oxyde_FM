#!/usr/bin/env bash
# Ubuntu, local archive: DEB or archive choice
SIM_TITLE='Ubuntu, local archive: DEB or archive choice'
SIM_OS=ubuntu
SIM_LOCAL_ARCHIVE=1
source "$(dirname "$0")/../lib.sh"
sim_main --archive "$HOME/Downloads/oxyde-0.9.0-x86_64-linux.tar.gz" "$@"
