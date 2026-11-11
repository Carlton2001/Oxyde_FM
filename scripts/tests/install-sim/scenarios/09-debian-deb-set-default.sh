#!/usr/bin/env bash
# Debian: DEB and --set-default
SIM_TITLE='Debian: DEB and --set-default'
SIM_OS=debian
source "$(dirname "$0")/../lib.sh"
sim_main --set-default "$@"
