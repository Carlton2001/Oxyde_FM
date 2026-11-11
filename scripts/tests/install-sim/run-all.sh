#!/usr/bin/env bash
# Runs every scenario in turn, pausing between them.
for s in "$(dirname "$0")"/scenarios/*.sh; do
    clear; "$s"; read -rp $'\n[Enter] next scenario, Ctrl+C to stop ' _
done
