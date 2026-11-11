#!/usr/bin/env bash
# --purge without --uninstall (try --foo, or --update alone)
SIM_TITLE='--purge without --uninstall (try --foo, or --update alone)'
source "$(dirname "$0")/../lib.sh"
sim_main --purge "$@"
