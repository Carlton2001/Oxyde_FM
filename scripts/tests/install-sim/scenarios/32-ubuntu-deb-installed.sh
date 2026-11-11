#!/usr/bin/env bash
# Ubuntu with the 0.8.0 DEB installed (menu)
SIM_TITLE='Ubuntu with the 0.8.0 DEB installed (menu)'
SIM_OS=ubuntu SIM_DEB_INSTALLED=0.8.0 SIM_SETTINGS=1
source "$(dirname "$0")/../lib.sh"
sim_main "$@"
