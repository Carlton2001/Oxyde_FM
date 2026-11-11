#!/usr/bin/env bash
# --version of a release that does not exist
SIM_TITLE='--version of a release that does not exist'
source "$(dirname "$0")/../lib.sh"
sim_main --version 0.7.3 "$@"
