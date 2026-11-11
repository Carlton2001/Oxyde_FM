#!/usr/bin/env bash
# Ubuntu: the DEB removed from the menu gives folders and dialogs back to the system
SIM_TITLE='Ubuntu: removing the DEB from the menu (default, file chooser)'
SIM_OS=ubuntu SIM_DEB_INSTALLED=0.8.0 SIM_SETTINGS=1 SIM_DEFAULT=1 SIM_PORTAL=1
source "$(dirname "$0")/../lib.sh"
sim_main "$@"
