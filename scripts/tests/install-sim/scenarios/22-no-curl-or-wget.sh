#!/usr/bin/env bash
# Neither curl nor wget
SIM_TITLE='Neither curl nor wget'
SIM_MISSING=$'cmd curl\ncmd wget'
source "$(dirname "$0")/../lib.sh"
sim_main "$@"
