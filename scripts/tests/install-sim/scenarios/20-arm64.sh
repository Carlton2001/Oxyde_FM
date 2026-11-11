#!/usr/bin/env bash
# aarch64 machine
SIM_TITLE='aarch64 machine'
SIM_ARCH=aarch64
source "$(dirname "$0")/../lib.sh"
sim_main "$@"
