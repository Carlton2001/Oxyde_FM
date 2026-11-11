#!/usr/bin/env bash
# Archive that does not match its SHA-256
SIM_TITLE='Archive that does not match its SHA-256'
SIM_CORRUPT=1
source "$(dirname "$0")/../lib.sh"
sim_main "$@"
