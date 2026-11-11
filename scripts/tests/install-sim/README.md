# install.sh simulation

Runs the real `packaging/linux/install.sh` in a throw-away home (`runs/`) on a simulated system: the distribution, dependencies, network, `sudo`, `gpgv`, `apt` and `rpm-ostree` are fakes. Nothing is installed or downloaded. Run it before publishing a change to `install.sh`.

- `./scenarios/NN-*.sh [install.sh options]`: one case, with the home before and after. Run it in a real terminal to answer the menus.
- `./run-all.sh`: every case in turn. `./clean.sh`: empties `runs/`.
- `python3 -I drive_tty.py E,D,E ./scenarios/11-update.sh`: answers the menus in a pseudo-terminal (`E` Enter, `D` down, `U` up, `Q` quit); `SIM_COLUMNS` sets its width (100).
- `SIM_*` variables, listed at the top of `lib.sh`, compose a new case.

`lib.sh` replaces some of `install.sh`'s functions (`probe`, `os_field`, `is_atomic`, `download`, `deb_version`...): rename one there, and rename it here too.
