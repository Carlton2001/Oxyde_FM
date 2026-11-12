# Simulation harness for packaging/linux/install.sh. Sourced by the scenario scripts.
# Everything happens in a throw-away HOME: no real package, sudo, network or file
# outside the sandbox is touched. Variables a scenario may set before sourcing:
#   SIM_TITLE, SIM_OS (fedora|silverblue|ubuntu|debian|arch|opensuse|void),
#   SIM_MISSING (probes from install.sh's dependency table, one per line, e.g. "cmd bwrap"),
#   SIM_ARCH, SIM_GLIBC, SIM_UID, SIM_GPG (ok|bad), SIM_CORRUPT=1,
#   SIM_RUNNING=SECONDS (Oxyde runs, from the first version preinstalled, and quits after
#   that time; 0 never quits), SIM_RELAUNCH=1 (quitting starts it again for 3 s, as after
#   an update), SIM_PREINSTALL="VERSION..." (installed in turn), SIM_SETTINGS=1, SIM_DEFAULT=1, SIM_PORTAL=1,
#   SIM_SYSTEM_PKG=1, SIM_DEV_ENTRY=1, SIM_VERSION (release published, default 0.9.0),
#   SIM_LOCAL_ARCHIVE=1 (puts an archive in ~/Downloads for --archive).
sim_here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo=$(cd "$sim_here/../../.." && pwd)
SIM_VERSION=${SIM_VERSION:-0.9.0}
SIM_OS=${SIM_OS:-fedora}

bold=$'\e[1m' dim=$'\e[2m' cyan=$'\e[36m' reset=$'\e[0m'
mkdir -p "$sim_here/runs"
sandbox=$(mktemp -d "$sim_here/runs/$(basename "$0" .sh)-XXXX")
export HOME=$sandbox/home TMPDIR=$sandbox/tmp
unset XDG_DATA_HOME XDG_CONFIG_HOME XDG_CACHE_HOME
export XDG_DATA_DIRS=$sandbox/usr/share
mkdir -p "$HOME" "$TMPDIR" "$XDG_DATA_DIRS" "$sandbox/release"
export PATH="$HOME/.local/bin:$PATH"

app_id=io.github.Carlton2001.Oxyde
make_archive() { # version destination-dir
    local v=$1 out=$2 tree=$sandbox/build/oxyde-$1-x86_64-linux
    mkdir -p "$tree/bin" "$tree/share/applications" "$tree/share/metainfo" "$tree/share/icons/hicolor/scalable/apps"
    printf '#!/bin/sh\necho "Oxyde %s (dummy)"\n' "$v" > "$tree/bin/oxyde"; chmod +x "$tree/bin/oxyde"
    cp "$repo/packaging/linux/$app_id.desktop" "$tree/share/applications/"
    cp "$repo/packaging/linux/$app_id.metainfo.xml" "$tree/share/metainfo/"
    cp "$repo/assets/icons/logo_oxyde.svg" "$tree/share/icons/hicolor/scalable/apps/$app_id.svg"
    cp "$repo/packaging/linux/install.sh" "$tree/"
    tar -czf "$out/oxyde-$v-x86_64-linux.tar.gz" -C "$sandbox/build" "oxyde-$v-x86_64-linux"
}
make_archive "$SIM_VERSION" "$sandbox/release"
printf 'fake deb\n' > "$sandbox/release/oxyde_${SIM_VERSION}-1_amd64.deb"
(cd "$sandbox/release" && sha256sum -- *.tar.gz *.deb > SHA256SUMS && printf 'fake signature\n' > SHA256SUMS.asc)
[[ -z ${SIM_CORRUPT:-} ]] || printf 'corrupted' >> "$sandbox/release/oxyde-$SIM_VERSION-x86_64-linux.tar.gz"

# shellcheck disable=SC1091
source "$repo/packaging/linux/install.sh"
release_version=$SIM_VERSION

# --- Fake system -------------------------------------------------------------
sim_missing() { [[ $'\n'"${SIM_MISSING:-}"$'\n' == *$'\n'"$1"$'\n'* ]]; }
command() {
    if [[ ${1:-} == -v ]] && sim_missing "cmd $2"; then return 1; fi
    builtin command "$@"
}
probe() { ! sim_missing "$1"; }
os_field() {
    local ID ID_LIKE NAME PRETTY_NAME
    case $SIM_OS in
        fedora) ID=fedora NAME='Fedora Linux' PRETTY_NAME='Fedora Linux 43 (Workstation Edition)' ;;
        silverblue) ID=fedora NAME='Fedora Linux' PRETTY_NAME='Fedora Linux 43 (Silverblue)' ;;
        ubuntu) ID=ubuntu ID_LIKE=debian NAME=Ubuntu PRETTY_NAME='Ubuntu 26.04 LTS' ;;
        debian) ID=debian NAME='Debian GNU/Linux' PRETTY_NAME='Debian GNU/Linux 13 (trixie)' ;;
        arch) ID=arch NAME='Arch Linux' PRETTY_NAME='Arch Linux' ;;
        opensuse) ID=opensuse-tumbleweed ID_LIKE="opensuse suse" NAME='openSUSE Tumbleweed' PRETTY_NAME='openSUSE Tumbleweed' ;;
        *) ID=$SIM_OS NAME='Void Linux' PRETTY_NAME='Void Linux' ;;
    esac
    printf '%s' "${!1:-}"
}
is_atomic() { [[ $SIM_OS == silverblue ]]; }
uname() { printf '%s\n' "${SIM_ARCH:-x86_64}"; }
getconf() { printf 'glibc %s\n' "${SIM_GLIBC:-2.41}"; }
id() { if [[ ${1:-} == -u ]]; then printf '%s\n' "${SIM_UID:-1000}"; else builtin command id "$@"; fi; }
# A running Oxyde: $sandbox/running exists while it runs, and the folder it was started
# from holds .sim-running, which follows the folder as its inode would.
oxyde_pids() { [[ ! -e $sandbox/running ]] || printf '4242\n'; }
versions_in_use() {
    [[ -e $sandbox/running ]] || return 0
    local marker
    for marker in "$lib_root"/*/.sim-running "$lib_root"/.retired-*/.sim-running; do
        [[ ! -e $marker ]] || basename "$(dirname "$marker")"
    done
}
gpgv() { [[ ${SIM_GPG:-ok} == ok ]]; }
download() {
    command -v curl >/dev/null || command -v wget >/dev/null || return 1
    local file=$sandbox/release/$(basename "$1")
    [[ $1 == */v$SIM_VERSION/* && -f $file ]] || return 1
    [[ $(basename "$1") == SHA256SUMS* ]] || printf '%s\n' "######################################################################## 100,0%" >&2
    cp "$file" "$2"
}
curl() { printf '{\n  "tag_name": "v%s",\n' "$SIM_VERSION"; }
sim_run() { printf '%s  [simulated] %s%s\n' "$dim" "$*" "$reset"; }
sudo() { [[ ${SIM_SUDO_SILENT:-} ]] || sim_run "sudo $*"; }
rpm-ostree() { sim_run "rpm-ostree $*"; }
systemctl() { sim_run "systemctl $*"; }
update-desktop-database() { :; }
gtk-update-icon-cache() { :; }
dpkg-query() { [[ -n ${SIM_DEB_INSTALLED:-} ]] && printf 'install ok installed %s-1\n' "$SIM_DEB_INSTALLED"; }
xdg-mime() {
    if [[ $1 == query ]]; then
        sed -n "s|^$3=||p" "$HOME/.config/mimeapps.list" 2>/dev/null | head -n 1; return 0
    fi
    mkdir -p "$HOME/.config"
    printf '[Default Applications]\n%s=%s\n' "$3" "$2" > "$HOME/.config/mimeapps.list"
}

# --- Initial state -------------------------------------------------------------
for sim_version in ${SIM_PREINSTALL:-}; do
    mkdir -p "$sandbox/old"; make_archive "$sim_version" "$sandbox/old"
    ( install_archive "$sandbox/old/oxyde-$sim_version-x86_64-linux.tar.gz" ) >/dev/null 2>&1
done
if [[ -n ${SIM_RUNNING:-} ]]; then
    : > "$sandbox/running"
    sim_version=${SIM_PREINSTALL%% *}
    [[ -z $sim_version ]] || : > "$HOME/.local/lib/oxyde/$sim_version/.sim-running"
    if ((SIM_RUNNING > 0)); then
        {
            sleep "$SIM_RUNNING"; rm -f "$sandbox/running"
            [[ -z ${SIM_RELAUNCH:-} ]] || { sleep 0.3; : > "$sandbox/running"; sleep 3; rm -f "$sandbox/running"; }
        } &
    fi
fi
if [[ -n ${SIM_SETTINGS:-} ]]; then
    mkdir -p "$HOME/.config/Oxyde" "$HOME/.cache/Oxyde/thumbnails"
    printf '{ "theme": "dark" }\n' > "$HOME/.config/Oxyde/settings.json"
fi
[[ -z ${SIM_DEFAULT:-} ]] || xdg-mime default "$app_id.desktop" inode/directory
if [[ -n ${SIM_PORTAL:-} ]]; then
    mkdir -p "$HOME/.config/xdg-desktop-portal"
    printf '[preferred]\ndefault=gnome;gtk;\n# Added by Oxyde for its file chooser.\norg.freedesktop.impl.portal.FileChooser=oxyde;gnome;\n' \
        > "$HOME/.config/xdg-desktop-portal/gnome-portals.conf"
fi
if [[ -n ${SIM_SYSTEM_PKG:-} ]]; then
    mkdir -p "$XDG_DATA_DIRS/applications"
    cp "$repo/packaging/linux/$app_id.desktop" "$XDG_DATA_DIRS/applications/"
fi
if [[ -n ${SIM_DEV_ENTRY:-} ]]; then
    mkdir -p "$HOME/.local/share/applications"
    sed 's|^Exec=.*|Exec="/home/dev/Oxyde_FM/target/debug/oxyde" %U|' \
        "$repo/packaging/linux/$app_id.desktop" > "$HOME/.local/share/applications/$app_id.desktop"
fi
if [[ -n ${SIM_LOCAL_ARCHIVE:-} ]]; then
    mkdir -p "$HOME/Downloads"; make_archive "$SIM_VERSION" "$HOME/Downloads"
fi

# --- Run and report ---------------------------------------------------------------
sim_state() {
    printf '%s-- %s --%s\n' "$cyan" "$1" "$reset"
    (cd "$HOME" && find . \( -path ./.local/lib/oxyde/\*/share -o -path ./.local/lib/oxyde/\*/bin \) -prune -o \
        \( -type f -o -type l \) -print 2>/dev/null | sed 's|^\./|  ~/|' | sort)
    [[ -L $HOME/.local/lib/oxyde/current ]] && printf '  current -> %s\n' "$(readlink "$HOME/.local/lib/oxyde/current")"
    [[ -f $HOME/.config/mimeapps.list ]] && sed 's/^/  mimeapps: /' "$HOME/.config/mimeapps.list"
    local p=$HOME/.config/xdg-desktop-portal/gnome-portals.conf
    [[ -f $p ]] && sed 's/^/  portals: /' "$p"
    true
}

sim_main() {
    printf '\n%s%s%s\n' "$bold" "${SIM_TITLE:-Scenario}" "$reset"
    printf '%sOS: %s  |  missing: %s%s\n' "$dim" "$SIM_OS" "$(printf '%s' "${SIM_MISSING:-none}" | tr '\n' ',' )" "$reset"
    sim_state 'Before'
    printf '\n%s$ curl -fsSL .../install.sh | bash -s -- %s%s\n\n' "$bold" "$*" "$reset"
    set +e
    ( set -e; main "$@" )
    local status=$?
    printf '\n%s(exit status: %s)%s\n\n' "$dim" "$status" "$reset"
    sim_state 'After'
    printf '\n%sSandbox kept in %s%s\n' "$dim" "$sandbox" "$reset"
}
