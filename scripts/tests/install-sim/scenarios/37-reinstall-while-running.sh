#!/usr/bin/env bash
# Reinstalling the version Oxyde runs from moves it aside, kept while it runs
SIM_TITLE='Reinstall the version Oxyde runs from'
SIM_PREINSTALL=0.9.0
SIM_RUNNING=0
source "$(dirname "$0")/../lib.sh"
sim_main --yes "$@"
