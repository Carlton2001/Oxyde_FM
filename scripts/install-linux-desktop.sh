#!/usr/bin/env bash
# Install or remove Oxyde's desktop entry and icon for the current user, so that
# GNOME shows its windows with their name and icon. Packages install the same files
# under /usr/share instead.
set -euo pipefail

app_id=io.github.Carlton2001.Oxyde
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
data=${XDG_DATA_HOME:-$HOME/.local/share}

usage() {
    printf 'Usage: bash scripts/install-linux-desktop.sh --exec BINARY [--profile DIR]\n'
    printf '       bash scripts/install-linux-desktop.sh --remove\n'
    printf 'The launcher runs BINARY; with --profile, it keeps every setting, cache and\n'
    printf 'temporary file of that Oxyde under DIR, as the development test profile does.\n'
}

binary=
profile=
remove=false
while (($#)); do
    case "$1" in
        --exec) binary=${2:?}; shift ;;
        --profile) profile=${2:?}; shift ;;
        --remove) remove=true ;;
        --help|-h) usage; exit 0 ;;
        *) printf 'Unknown argument: %s\n' "$1" >&2; usage >&2; exit 2 ;;
    esac
    shift
done

entry=$data/applications/$app_id.desktop
icon=$data/icons/hicolor/scalable/apps/$app_id.svg

refresh() {
    update-desktop-database -q "$data/applications" 2>/dev/null || true
    gtk-update-icon-cache -q -t "$data/icons/hicolor" 2>/dev/null || true
}

if $remove; then
    rm -f "$entry" "$icon"
    refresh
    printf 'Removed %s and its icon.\n' "$app_id"
    exit 0
fi

if [[ -z $binary ]]; then
    usage >&2
    exit 2
fi
binary=$(realpath "$binary")
[[ -x $binary ]] || { printf 'Not an executable: %s\n' "$binary" >&2; exit 2; }

command="'$binary'"
if [[ -n $profile ]]; then
    profile=$(realpath "$profile")
    command="env XDG_CONFIG_HOME='$profile/config' XDG_CACHE_HOME='$profile/cache' XDG_DATA_HOME='$profile/data' TMPDIR='$profile/tmp' $command"
fi

mkdir -p "$(dirname "$entry")" "$(dirname "$icon")"
cp "$root/assets/icons/logo_oxyde.svg" "$icon"
# The desktop entry quotes arguments with double quotes only.
sed "s|^Exec=.*|Exec=${command//\'/\"} %U|" "$root/packaging/linux/$app_id.desktop" > "$entry"
desktop-file-validate "$entry" | sed '/: hint: /d'
refresh
printf 'Installed %s: %s\n' "$app_id" "$entry"
