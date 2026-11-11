#!/usr/bin/env bash
# Install, update or remove Oxyde on Linux, for example with:
#   curl -fsSL https://github.com/Carlton2001/Oxyde_FM/releases/latest/download/install.sh | bash
# Debian and Ubuntu get the DEB through apt: its AppArmor profile lets bubblewrap
# confine the decoders. Every other x86-64 system gets the archive under ~/.local,
# without root, which also suits read-only systems such as Fedora Silverblue.
# Everything runs from main, at the end, so a truncated download executes nothing.
# Oxyde updates itself by running the new archive's copy of this script with
# --archive FILE --update: keep both options working from one release to the next.
set -euo pipefail

app_id=io.github.Carlton2001.Oxyde
repository=Carlton2001/Oxyde_FM
# Replaced by the version of the release that publishes this script.
release_version=@VERSION@
minimum_glibc=2.39

lib_root=$HOME/.local/lib/oxyde
bin_link=$HOME/.local/bin/oxyde
data=${XDG_DATA_HOME:-$HOME/.local/share}
entry=$data/applications/$app_id.desktop
marker=X-Oxyde-Install=user

# Presentation -----------------------------------------------------------------

# Colors only on a terminal, and never with NO_COLOR. Symbols carry the meaning,
# colors and weight only repeat it.
if [[ -t 1 && -z ${NO_COLOR:-} && ${TERM:-dumb} != dumb ]]; then
    bold=$'\e[1m' dim=$'\e[2m' reset=$'\e[0m'
    accent=$'\e[1;38;5;209m' good=$'\e[32m' caution=$'\e[1;33m' bad=$'\e[1;31m'
else
    bold='' dim='' reset='' accent='' good='' caution='' bad=''
fi

# Symbols in a UTF-8 locale, their ASCII equivalents otherwise.
case ${LC_ALL:-${LC_CTYPE:-${LANG:-}}} in
    *[Uu][Tt][Ff]-8*|*[Uu][Tt][Ff]8*)
        s_brand='◆' s_ok='✓' s_bad='✗' s_pointer='❯' s_down='↓' s_item='•' s_optional='○'
        s_dot='·' s_arrows='↑ ↓' s_path='›' s_cut='…' ;;
    *)
        s_brand='*' s_ok='+' s_bad='x' s_pointer='>' s_down='v' s_item='-' s_optional='o'
        s_dot='-' s_arrows='Up/Down' s_path='>' s_cut='~' ;;
esac

# Plain output: also returns values from functions run in $(...).
say() { printf '%s\n' "$*"; }
# A section title.
step() { say ''; say "${bold}$*${reset}"; }
# Something done, or present.
ok() { say "  ${good}${s_ok}${reset} $*"; }
# A detail under the line above.
info() { say "    ${dim}$*${reset}"; }
warn() { printf '  %s!%s %s\n' "$caution" "$reset" "$*" >&2; }
die() { printf '\n  %s%s Error:%s %s\n' "$bad" "$s_bad" "$reset" "$*" >&2; exit 1; }
tilde() { printf '%s' "${1/#"$HOME"/\~}"; }

banner() {
    local system
    system=$(os_field PRETTY_NAME)
    say ''
    say "  ${accent}${s_brand} Oxyde${reset}  ${dim}installer ${s_dot} ${system:-Linux}${reset}"
}

# True when a person can answer: the terminal opens and output goes to it.
interactive() {
    [[ -t 1 ]] && { : < /dev/tty; } 2>/dev/null
}

# The terminal's width, for lines that are drawn again and must not wrap.
terminal_columns() {
    local size
    size=$(stty size < /dev/tty 2>/dev/null || true)
    size=${size#* }
    [[ $size =~ ^[1-9][0-9]*$ ]] || size=${COLUMNS:-80}
    say "$size"
}

# Cuts text to width characters.
fit() {
    local text=$1 width=$2
    ((width > 8)) || width=8
    ((${#text} <= width)) || text="${text:0:width-1}$s_cut"
    printf '%s' "$text"
}

cursor_hidden=false
show_cursor() {
    $cursor_hidden || return 0
    printf '\e[?25h' > /dev/tty 2>/dev/null || true
    cursor_hidden=false
}

# Lets the user pick an option with the arrow keys, or its number, and Enter; sets
# choice to its index. The last option must be the harmless one: Esc, q and the end
# of input choose it. Reads the terminal because stdin may be this script.
choose() {
    local question=$1 selected=$2
    shift 2
    local -a options=("$@")
    local count=${#options[@]} key rest i width
    # Each option must stay on one line, or redrawing would climb too little.
    width=$(($(terminal_columns) - 5))
    for ((i = 0; i < count; i++)); do
        options[i]=$(fit "${options[i]}" "$width")
    done
    cursor_hidden=true
    {
        printf '\n  %s%s%s\n\e[?25l' "$bold" "$question" "$reset"
        while :; do
            for ((i = 0; i < count; i++)); do
                if ((i == selected)); then
                    printf '\e[2K  %s%s %s%s\n' "$accent" "$s_pointer" "${options[i]}" "$reset"
                else
                    printf '\e[2K    %s\n' "${options[i]}"
                fi
            done
            printf '\e[2K    %s%s%s' "$dim" "$(fit "$s_arrows to move, Enter to choose" "$width")" "$reset"
            if ! IFS= read -rsn1 key < /dev/tty; then
                selected=$((count - 1))
                break
            fi
            if [[ $key == $'\e' ]]; then
                IFS= read -rsn2 -t 0.05 rest < /dev/tty || rest=''
                key+=$rest
            fi
            case $key in
                $'\e[A'|k) selected=$(((selected + count - 1) % count)) ;;
                $'\e[B'|j) selected=$(((selected + 1) % count)) ;;
                [1-9]) if ((key <= count)); then selected=$((key - 1)); break; fi ;;
                '') break ;;
                $'\e'|q) selected=$((count - 1)); break ;;
            esac
            printf '\r\e[%dA' "$count"
        done
        # Only the answer stays on screen.
        printf '\r\e[%dA\e[J  %s%s%s %s\n' "$count" "$good" "$s_ok" "$reset" "${options[selected]}"
    } > /dev/tty
    show_cursor
    choice=$selected
}

# Yes or no, yes first. --yes answers yes; without a terminal the answer is no.
confirm() {
    $assume_yes && return 0
    interactive || return 1
    choose "$1" 0 "$2" "$3"
    ((choice == 0))
}

usage() {
    say 'Usage: install.sh [--version X.Y.Z] [--archive FILE] [--set-default] [--yes]'
    say '       install.sh --uninstall [--purge]'
    say ''
    say '  --version      install this release instead of the one this script belongs to'
    say '  --archive      install a downloaded oxyde-*-x86_64-linux.tar.gz or its folder, offline'
    say '  --set-default  make Oxyde the default application for folders'
    say '  --yes          install missing system packages without asking'
    say '  --uninstall    remove the per-user installation, its cache and its associations;'
    say '                asks whether to delete the settings too'
    say '  --purge        with --uninstall, delete the settings without asking'
    say '  --update       with --archive, as Oxyde runs it: no dependency check or question'
    say ''
    say 'Without options on a terminal, it shows a menu: install, update or uninstall.'
    say 'Oxyde becomes the default file manager in Preferences, System integration.'
    say 'Running it again installs the newer release and keeps the previous one.'
}

os_field() {
    (. /etc/os-release 2>/dev/null && eval "printf '%s' \"\${$1:-}\"") || true
}

# debian, fedora, suse, arch or other, from ID and ID_LIKE.
os_family() {
    local ids
    ids=" $(os_field ID) $(os_field ID_LIKE) "
    case $ids in
        *" debian "*|*" ubuntu "*) say debian ;;
        *" fedora "*|*" rhel "*) say fedora ;;
        *" suse "*|*" opensuse "*) say suse ;;
        *" arch "*) say arch ;;
        *) say other ;;
    esac
}

is_ubuntu() { [[ " $(os_field ID) $(os_field ID_LIKE) " == *" ubuntu "* ]]; }
# Set once the user chose an archive on Ubuntu knowing its isolation is reduced.
isolation_accepted=false

is_atomic() { [[ -e /run/ostree-booted ]]; }

# retries defaults to 3; the small files checked before the menu take 1, so that an
# offline user who came to uninstall does not wait long.
download() {
    local url=$1 destination=$2 retries=${3:-3}
    if command -v curl >/dev/null; then
        curl -fL --proto '=https' --tlsv1.2 --connect-timeout 10 --retry "$retries" \
            --progress-bar -o "$destination" "$url"
    elif command -v wget >/dev/null; then
        wget -q --show-progress --connect-timeout=10 --tries=$((retries + 1)) -O "$destination" "$url"
    else
        return 1
    fi
}

# Oxyde's package signing key, the same as packaging/linux/signing/oxyde-packages.asc.
package_key() {
    cat <<'KEY'
-----BEGIN PGP PUBLIC KEY BLOCK-----

mDMEauvNxRYJKwYBBAHaRw8BAQdAhzxjWxqmVfqyifvS26Zphk3iZE427h1QD6kN
lskMxUm0L094eWRlIChwYWNrYWdlIHNpZ25pbmcpIDxjYXJsdG9uMjAwMUBnbWFp
bC5jb20+iJMEExYKADsWIQT512ueO+3V6syc/n2sLpYfMgKXBgUCauvNxQIbAwUL
CQgHAgIiAgYVCgkICwIEFgIDAQIeBwIXgAAKCRCsLpYfMgKXBnJ7AQD5cEvWNk2w
YHru+mIpnXZA1rfWanABRgm8GDlQFzzLRgD+NCefhjYnUGPmZ6PyzywBR0TMSrd0
5MEPv4QQRsHhdww=
=gYzm
-----END PGP PUBLIC KEY BLOCK-----
KEY
}

# The binary packets of an armored block, which is all gpgv reads as a keyring.
dearmor() {
    sed -e '1,/^$/d' -e '/^-----END/,$d' -e '/^=/d' | base64 -d
}

# Checks SHA256SUMS against SHA256SUMS.asc with the key above. Without gpgv, only the
# checksums protect the download, which HTTPS already does.
verify_checksums() {
    local directory=$1
    if ! command -v gpgv >/dev/null; then
        warn 'gpgv is missing: the checksums are used without checking their signature.'
        return 0
    fi
    mkdir -p -m 700 "$directory/gnupg"
    package_key | dearmor > "$directory/keyring.gpg" || die 'cannot read the package signing key.'
    gpgv --homedir "$directory/gnupg" --keyring "$directory/keyring.gpg" \
        "$directory/SHA256SUMS.asc" "$directory/SHA256SUMS" 2>/dev/null \
        || die "SHA256SUMS is not signed by Oxyde's package key; nothing was installed."
    ok "Release signed by Oxyde's key"
}

# Checks a downloaded file against its line of SHA256SUMS.
check_file() {
    local directory=$1 name=$2 line
    line=$(awk -v name="$name" '$2 == name' "$directory/SHA256SUMS")
    [[ -n $line ]] || die "$name is not listed in SHA256SUMS."
    (cd "$directory" && printf '%s\n' "$line" | sha256sum --quiet -c -) \
        || die "$name does not match its SHA-256; nothing was installed."
    ok "$name is intact"
}

# Downloads the release's SHA256SUMS, checks its signature and that it lists name:
# everything that can fail before the download itself, so that it fails first.
fetch_checksums() {
    local version=$1 name=$2 directory=$3
    local base=https://github.com/$repository/releases/download/v$version
    step "Checking Oxyde $version"
    # Checked here: download runs with its errors silenced.
    command -v curl >/dev/null || command -v wget >/dev/null || die 'curl or wget is required.'
    download "$base/SHA256SUMS" "$directory/SHA256SUMS" 1 2>/dev/null \
        || die "Release v$version cannot be reached, or has no SHA256SUMS."
    download "$base/SHA256SUMS.asc" "$directory/SHA256SUMS.asc" 1 2>/dev/null \
        || die "SHA256SUMS.asc is not in release v$version."
    verify_checksums "$directory"
    awk -v name="$name" '$2 == name { found = 1 } END { exit !found }' "$directory/SHA256SUMS" \
        || die "$name is not in release v$version."
}

# Downloads a release file and checks it against the release's signed SHA256SUMS.
fetch_release_file() {
    local version=$1 name=$2 directory=$3
    local base=https://github.com/$repository/releases/download/v$version
    [[ -f $directory/SHA256SUMS ]] || fetch_checksums "$version" "$name" "$directory"
    step "Downloading Oxyde $version"
    say "  ${dim}${s_down}${reset} $name"
    download "$base/$name" "$directory/$name" || die "$name is not in release v$version."
    check_file "$directory" "$name"
}

resolve_version() {
    if [[ -n $requested_version ]]; then
        say "${requested_version#v}"
    elif [[ $release_version != @*@ ]]; then
        say "$release_version"
    else
        # Run from a checkout: ask GitHub for the latest published release.
        local answer
        answer=$(curl -fsSL "https://api.github.com/repos/$repository/releases/latest") \
            || die 'cannot reach GitHub to find the latest release; pass --version.'
        answer=$(printf '%s' "$answer" | sed -n 's/^ *"tag_name": *"v\{0,1\}\([^"]*\)".*/\1/p' | head -n 1)
        [[ -n $answer ]] || die 'no published release found; pass --version.'
        say "$answer"
    fi
}

check_platform() {
    [[ $(id -u) -ne 0 ]] || die 'run this script as your own user, not as root.'
    [[ $(uname -m) == x86_64 ]] || die "Oxyde is built for x86-64 only, not $(uname -m)."
    local glibc
    glibc=$(getconf GNU_LIBC_VERSION 2>/dev/null | awk '{print $2}') \
        || die 'Oxyde needs glibc; this system does not seem to use it.'
    [[ -n $glibc ]] || die 'Oxyde needs glibc; this system does not seem to use it.'
    if [[ $(printf '%s\n%s\n' "$minimum_glibc" "$glibc" | sort -V | head -n 1) != "$minimum_glibc" ]]; then
        die "Oxyde needs glibc $minimum_glibc or later; this system has $glibc."
    fi
}

# Dependencies of the per-user installation ------------------------------------

library_cache=
has_library() {
    if [[ -z $library_cache ]]; then
        local ldconfig
        for ldconfig in ldconfig /sbin/ldconfig /usr/sbin/ldconfig; do
            library_cache=$("$ldconfig" -p 2>/dev/null) && break
        done
        library_cache+=$'\n'
    fi
    [[ $library_cache == *" $1 "* ]] && return 0
    local directory
    for directory in /usr/lib64 /usr/lib /usr/lib/x86_64-linux-gnu /lib64; do
        [[ -e $directory/$1 ]] && return 0
    done
    return 1
}

has_vulkan_driver() {
    local directory
    for directory in /usr/share/vulkan/icd.d /etc/vulkan/icd.d; do
        compgen -G "$directory/*.json" >/dev/null && return 0
    done
    return 1
}

has_gvfs() {
    local directory
    for directory in /usr/libexec /usr/lib/gvfs /usr/libexec/gvfs /usr/lib/x86_64-linux-gnu/gvfs; do
        [[ -x $directory/gvfsd ]] && return 0
    done
    return 1
}

# Each line: level|probe|feature|Fedora|Arch|openSUSE|Debian packages.
dependencies() {
    cat <<'EOF'
required|cmd bwrap|archive and preview confinement|bubblewrap|bubblewrap|bubblewrap|bubblewrap
required|lib libseccomp.so.2|archive and preview confinement|libseccomp|libseccomp|libseccomp2|libseccomp2
required|lib libgio-2.0.so.0|trash, volumes and network|glib2|glib2|libgio-2_0-0|libglib2.0-0t64
required|lib libvulkan.so.1|rendering|vulkan-loader|vulkan-icd-loader|libvulkan1|libvulkan1
required|vulkan|rendering|mesa-vulkan-drivers|vulkan-swrast|libvulkan_lvp|mesa-vulkan-drivers
required|lib libwayland-client.so.0|windows|libwayland-client|wayland|libwayland-client0|libwayland-client0
required|lib libxkbcommon.so.0|keyboard|libxkbcommon|libxkbcommon|libxkbcommon0|libxkbcommon0
required|lib libxkbcommon-x11.so.0|keyboard|libxkbcommon-x11|libxkbcommon-x11|libxkbcommon-x11-0|libxkbcommon-x11-0
required|lib libxcb.so.1|windows|libxcb|libxcb|libxcb1|libxcb1
required|lib libfontconfig.so.1|fonts|fontconfig|fontconfig|libfontconfig1|libfontconfig1
required|lib liblzma.so.5|archives|xz-libs|xz|liblzma5|liblzma5
required|cmd xdg-open|opening files in another application|xdg-utils|xdg-utils|xdg-utils|xdg-utils
recommended|lib libpoppler-glib.so.8|PDF previews|poppler-glib|poppler-glib|libpoppler-glib8|libpoppler-glib8t64
recommended|lib libgstapp-1.0.so.0|audio and video previews|gstreamer1-plugins-base gstreamer1-plugins-good|gst-plugins-base gst-plugins-good|gstreamer-plugins-base gstreamer-plugins-good|gstreamer1.0-plugins-base gstreamer1.0-plugins-good
recommended|gvfs|network locations|gvfs gvfs-fuse gvfs-smb|gvfs gvfs-smb|gvfs gvfs-backends gvfs-fuse|gvfs gvfs-fuse gvfs-backends
recommended|cmd udisksctl|mounting volumes|udisks2|udisks2|udisks2|udisks2
recommended|cmd gpgv|checking the signature of updates|gnupg2|gnupg|gpg2|gpgv
recommended|cmd wl-copy|copying files to other applications under Wayland|wl-clipboard|wl-clipboard|wl-clipboard|wl-clipboard
EOF
}

probe() {
    case $1 in
        cmd\ *) command -v "${1#cmd }" >/dev/null ;;
        lib\ *) has_library "${1#lib }" ;;
        vulkan) has_vulkan_driver ;;
        gvfs) has_gvfs ;;
    esac
}

# Prints feature|packages items in two columns, or packages below their feature when
# the line would not fit.
list_components() {
    local width=$1 columns=$2 item feature names
    shift 2
    for item in "$@"; do
        feature=${item%%|*} names=${item#*|}
        if ((8 + width + 2 + ${#names} <= columns)); then
            printf '      %s %-*s  %s\n' "$s_item" "$width" "$feature" "$dim$names$reset"
        else
            printf '      %s %s\n        %s\n' "$s_item" "$feature" "$dim$names$reset"
        fi
    done
}

check_dependencies() {
    local family=$1 column
    case $family in
        fedora) column=4 ;;
        arch) column=5 ;;
        suse) column=6 ;;
        debian) column=7 ;;
        *) column=4 ;;
    esac
    local -a packages=() required=() recommended=()
    local level check feature fedora arch suse debian names width=0
    while IFS='|' read -r level check feature fedora arch suse debian; do
        probe "$check" && continue
        case $column in 4) names=$fedora ;; 5) names=$arch ;; 6) names=$suse ;; 7) names=$debian ;; esac
        ((${#feature} <= width)) || width=${#feature}
        if [[ $level == required ]]; then required+=("$feature|$names"); else recommended+=("$feature|$names"); fi
        local name
        for name in $names; do
            [[ " ${packages[*]} " == *" $name "* ]] || packages+=("$name")
        done
    done < <(dependencies)

    step 'System components'
    if ((${#packages[@]} == 0)); then
        ok 'Everything Oxyde uses is present.'
        return 0
    fi
    local columns
    columns=$(terminal_columns)
    if ((${#required[@]})); then
        say "  ${caution}!${reset} Required: Oxyde may not start without them."
        list_components "$width" "$columns" "${required[@]}"
    fi
    if ((${#recommended[@]})); then
        say "  ${dim}${s_optional}${reset} Recommended: without them, only the feature named is missing."
        list_components "$width" "$columns" "${recommended[@]}"
    fi
    say ''

    local -a command
    local system
    system=$(os_field NAME)
    case $family in
        fedora) if is_atomic; then command=(rpm-ostree install "${packages[@]}")
                else command=(sudo dnf install -y "${packages[@]}"); fi ;;
        arch) command=(sudo pacman -S --needed "${packages[@]}") ;;
        suse) command=(sudo zypper install "${packages[@]}") ;;
        debian) command=(sudo apt-get install -y "${packages[@]}") ;;
        *)
            say "    These are Fedora's package names: install the equivalents from"
            say "    ${system:-your distribution}'s repositories."
            ((${#required[@]} == 0)) || warn 'Oxyde may not start until the required ones are installed.'
            return 0 ;;
    esac
    local yes no
    if is_atomic; then
        say "    Oxyde itself is installed in your home folder and needs nothing more."
        say "    These components belong to the system image of ${system:-this system}, which is"
        say "    read-only: rpm-ostree adds them to the image. They become available after"
        say "    the next restart, and the system's updates keep them up to date."
        yes='Add them to the system image (restart needed)'
    else
        say "    They come from the repositories of ${system:-your distribution}, which keep them up to date."
        yes='Install them now (asks for your password)'
    fi
    no='Not now'
    info "Command: ${command[*]}"
    if confirm 'Add the missing components?' "$yes" "$no"; then
        if "${command[@]}"; then
            if is_atomic; then
                ok 'Components added. Restart the computer to use them.'
            else
                ok 'Components installed.'
            fi
        else
            warn 'Installing the components failed; Oxyde is installed anyway.'
        fi
    elif ((${#required[@]})); then
        warn 'Oxyde may not start until the required components are installed:'
        info 'run the command above.'
    else
        info 'Oxyde works without them; the command above adds them later.'
    fi
}

# Per-user installation --------------------------------------------------------

install_icons() {
    local source=$1/share/icons/hicolor icon size
    for icon in "$source"/*/apps/"$app_id".*; do
        size=$(basename "$(dirname "$(dirname "$icon")")")
        install -Dm644 "$icon" "$data/icons/hicolor/$size/apps/$(basename "$icon")"
    done
    touch "$data/icons/hicolor"
    gtk-update-icon-cache -q -t "$data/icons/hicolor" 2>/dev/null || true
}

install_entry() {
    local source=$1 executable=$lib_root/current/bin/oxyde exec_line
    # Exec takes one quoted argument: escape " ` $ \ for the quoting, then \ again
    # for the string value, and double % (Desktop Entry specification).
    exec_line=$(printf '%s' "$executable" | sed -e 's/[\\"`$]/\\&/g' -e 's/\\/\\\\/g' -e 's/%/%%/g')
    local temporary line
    temporary=$(mktemp)
    cleanup+=("$temporary")
    while IFS= read -r line || [[ -n $line ]]; do
        if [[ $line == Exec=* ]]; then
            printf 'Exec="%s" %%U\n' "$exec_line"
        else
            printf '%s\n' "$line"
        fi
    done < "$source/share/applications/$app_id.desktop" > "$temporary"
    printf '%s\n' "$marker" >> "$temporary"
    install -Dm644 "$temporary" "$entry"
    install -Dm644 "$source/share/metainfo/$app_id.metainfo.xml" "$data/metainfo/$app_id.metainfo.xml"
    update-desktop-database -q "$data/applications" 2>/dev/null || true
}

# Keeps the new version and the one it replaces, which a running Oxyde may still use.
prune_versions() {
    local keep_new=$1 keep_old=$2 directory
    for directory in "$lib_root"/*/; do
        directory=${directory%/}
        [[ -L $directory ]] && continue
        case $(basename "$directory") in
            "$keep_new"|"$keep_old"|.*) ;;
            *) rm -rf -- "$directory" ;;
        esac
    done
}

install_archive() {
    local archive=$1
    mkdir -p "$lib_root"
    # One installation at a time, should Oxyde and a terminal update together.
    if command -v flock >/dev/null; then
        exec 9> "$lib_root/.lock"
        flock 9
    fi
    local staging
    staging=$(mktemp -d "$lib_root/.staging-XXXXXX")
    cleanup+=("$staging")
    if [[ -d $archive ]]; then
        cp -a -- "$archive" "$staging/"
    else
        tar -xzf "$archive" -C "$staging" --no-same-owner
    fi
    local tree
    tree=$(find "$staging" -mindepth 1 -maxdepth 1 -type d -name 'oxyde-*-x86_64-linux' | head -n 1)
    [[ -n $tree && -x $tree/bin/oxyde ]] || die "$archive is not an Oxyde Linux archive."
    local version
    version=$(basename "$tree")
    version=${version#oxyde-}
    version=${version%-x86_64-linux}

    local previous=
    if [[ -L $lib_root/current ]]; then
        previous=$(readlink "$lib_root/current")
    fi
    rm -rf -- "${lib_root:?}/$version.new"
    mv -T "$tree" "$lib_root/$version.new"
    # A running Oxyde of the same version keeps its open files after the swap.
    rm -rf -- "${lib_root:?}/$version"
    mv -T "$lib_root/$version.new" "$lib_root/$version"
    ln -sfn "$version" "$lib_root/.current"
    mv -T "$lib_root/.current" "$lib_root/current"
    mkdir -p "$(dirname "$bin_link")"
    ln -sfn "$lib_root/current/bin/oxyde" "$bin_link"

    install_icons "$lib_root/$version"
    install_entry "$lib_root/$version"
    prune_versions "$version" "$previous"

    if [[ -n $previous && $previous != "$version" ]]; then
        ok "Oxyde $previous updated to $version in $(tilde "$lib_root")"
    else
        ok "Oxyde $version installed in $(tilde "$lib_root"), for you only"
    fi
    if is_ubuntu && ! $isolation_accepted; then
        warn 'On Ubuntu, bubblewrap confines previews and archives only for the DEB package;'
        info 'this installation keeps Landlock and seccomp alone. Prefer the DEB.'
    fi
    if [[ -e /usr/bin/oxyde ]]; then
        warn 'A system package also provides Oxyde; this installation takes its place in menus.'
    fi
    case ":$PATH:" in
        *":$(dirname "$bin_link"):"*) ;;
        *) info "$(tilde "$(dirname "$bin_link")") is not in PATH: the oxyde command needs its full path." ;;
    esac
}

set_default() {
    if xdg-mime default "$app_id.desktop" inode/directory 2>/dev/null; then
        ok 'Oxyde now opens folders by default.'
    else
        warn 'xdg-mime could not make Oxyde the default for folders.'
    fi
}

# True when a desktop entry outside $data still provides Oxyde, as a system package does.
system_entry_exists() {
    local directory
    local -a directories
    IFS=: read -ra directories <<< "${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"
    for directory in "${directories[@]}"; do
        [[ -n $directory && ${directory%/} != "${data%/}" ]] || continue
        [[ -f $directory/applications/$app_id.desktop ]] && return 0
    done
    return 1
}

# The mimeapps.list files where xdg-mime, GIO and desktops record associations.
association_files() {
    local config=${XDG_CONFIG_HOME:-$HOME/.config} file
    for file in "$config/mimeapps.list" "$config"/*-mimeapps.list "$data/applications/mimeapps.list"; do
        [[ ! -f $file ]] || printf '%s\n' "$file"
    done
}

# Drops Oxyde's desktop entry from every association, and a key left without any.
# Other lines are written back unchanged, through the file itself to keep links.
forget_associations() {
    local file temporary
    while IFS= read -r file; do
        grep -qF "$app_id.desktop" "$file" || continue
        temporary=$(mktemp)
        cleanup+=("$temporary")
        awk -v id="$app_id.desktop" '
            /^[[:space:]]*[#\[]/ || index($0, "=") == 0 { print; next }
            {
                split_at = index($0, "=")
                count = split(substr($0, split_at + 1), parts, ";")
                kept = ""; found = 0
                for (i = 1; i <= count; i++) {
                    if (parts[i] == id) { found = 1; continue }
                    if (parts[i] != "") kept = kept parts[i] ";"
                }
                if (!found) { print; next }
                if (kept != "") print substr($0, 1, split_at) kept
            }' "$file" > "$temporary" || continue
        cat "$temporary" > "$file"
    done < <(association_files)
}

# Takes Oxyde out of the portal's FileChooser line, as turning its file chooser off in
# Settings does: the portals after it remain, a line Oxyde added goes with its comment,
# and a copy Oxyde wrote of the system's configuration goes when nothing else changed.
forget_file_chooser() {
    local folder=${XDG_CONFIG_HOME:-$HOME/.config}/xdg-desktop-portal file temporary first source
    for file in "$folder"/*portals.conf; do
        [[ -f $file ]] || continue
        grep -qE '^[[:space:]]*org\.freedesktop\.impl\.portal\.FileChooser[[:space:]]*=(.*;)?[[:space:]]*oxyde[[:space:]]*(;|$)' "$file" || continue
        temporary=$(mktemp)
        cleanup+=("$temporary")
        awk '
            function trim(text) { gsub(/^[[:space:]]+|[[:space:]]+$/, "", text); return text }
            function release() { if (holding) { print held; holding = 0 } }
            BEGIN { key = "org.freedesktop.impl.portal.FileChooser"; added = "# Added by Oxyde for its file chooser." }
            {
                line = $0
                if (line ~ /^[[:space:]]*\[.*\][[:space:]]*$/) {
                    release(); section = line
                    gsub(/^[[:space:]]*\[[[:space:]]*|[[:space:]]*\][[:space:]]*$/, "", section)
                    print line; next
                }
                split_at = index(line, "=")
                if (section == "preferred" && split_at > 0 && trim(substr(line, 1, split_at - 1)) == key) {
                    was_added = holding; holding = 0
                    count = split(substr(line, split_at + 1), parts, ";")
                    kept = ""; found = 0
                    for (i = 1; i <= count; i++) {
                        portal = trim(parts[i])
                        if (portal == "oxyde") { found = 1; continue }
                        if (portal != "") kept = kept portal ";"
                    }
                    if (!found) { print line; next }
                    if (!was_added && kept != "") print key "=" kept
                    next
                }
                release()
                if (line == added) { held = line; holding = 1; next }
                print line
            }
            END { release() }' "$file" > "$temporary" || continue
        first=$(head -n 1 "$temporary")
        source=${first#"# Written by Oxyde from "}
        if [[ $source != "$first" && -f $source && $(tail -n +2 "$temporary") == "$(cat "$source")" ]]; then
            rm -f "$file"
        else
            cat "$temporary" > "$file"
        fi
        portal_changed=true
    done
}

is_running() { command -v pgrep >/dev/null && pgrep -u "$(id -u)" -x oxyde >/dev/null; }

# purge deletes the settings without asking, keep keeps them without asking.
uninstall() {
    local purge=$1 keep=${2:-false} found=false shared=false portal_changed=false file
    if [[ -f $entry ]]; then
        grep -qx "$marker" "$entry" || die "$entry was not written by this script; nothing is removed."
        found=true
    fi
    [[ -d $lib_root ]] && found=true
    if system_entry_exists; then
        $found || { say 'Oxyde is installed by a system package: remove it with your package manager.'; return 0; }
        shared=true
    fi
    step 'Uninstalling Oxyde'
    if ! $shared; then
        # Without any copy left, associations naming Oxyde lead nowhere.
        local was_default=false line=inode/directory=$app_id.desktop
        local -a files
        mapfile -t files < <(association_files)
        if ((${#files[@]})) && grep -qxF -e "$line" -e "$line;" "${files[@]}"; then
            was_default=true
        fi
        forget_associations
        $was_default && ok 'Oxyde is no longer the default application for folders.'
        forget_file_chooser
        $portal_changed && ok "Open and save dialogs are the system's again."
    fi
    local running=false
    is_running && running=true
    if $found; then
        if [[ -L $bin_link && $(readlink "$bin_link") == "$lib_root"/* ]]; then
            rm -f "$bin_link"
        fi
        rm -f "$entry" "$data/metainfo/$app_id.metainfo.xml"
        rm -f "$data"/icons/hicolor/*/apps/"$app_id".*
        # Written by Oxyde for FileManager1 and the file chooser portal; a remaining
        # system copy writes its own again.
        rm -f "$data/dbus-1/services/$app_id.FileManager1.service"
        local portal_files=("$data/xdg-desktop-portal/portals/oxyde.portal"
            "$data/dbus-1/services/org.freedesktop.impl.portal.desktop.oxyde.service")
        for file in "${portal_files[@]}"; do
            [[ -e $file ]] && { rm -f "$file"; portal_changed=true; }
        done
        [[ -d $data/icons/hicolor ]] && touch "$data/icons/hicolor"
        update-desktop-database -q "$data/applications" 2>/dev/null || true
        rm -rf -- "$lib_root"
        # Thumbnails, the GStreamer registry and copies of files opened from archives.
        local cache=${XDG_CACHE_HOME:-$HOME/.cache}/Oxyde
        if [[ -d $cache ]]; then
            if $running; then
                warn "Oxyde is running: its cache in $(tilde "$cache") is kept;"
                info 'delete it once Oxyde is closed.'
            else
                rm -rf -- "$cache"
            fi
        fi
        ok 'Oxyde is uninstalled.'
    else
        say '  No per-user installation of Oxyde was found.'
    fi
    # The portal reads its portals and configuration only when it starts.
    if $portal_changed; then
        systemctl --user try-restart xdg-desktop-portal.service 2>/dev/null || true
    fi
    if $shared; then
        local settings=${XDG_CONFIG_HOME:-$HOME/.config}/Oxyde
        [[ ! -d $settings ]] || info "Settings remain in $(tilde "$settings"), where the system package still uses them."
    else
        handle_settings "$purge" "$keep" "$running"
    fi
}

# Preferences, themes, session and log: kept unless the user asks.
handle_settings() {
    local purge=$1 keep=$2 running=$3
    local settings=${XDG_CONFIG_HOME:-$HOME/.config}/Oxyde
    [[ -d $settings ]] || return 0
    if $running; then
        warn "Oxyde is running and would write its settings again:"
        info "they remain in $(tilde "$settings")."
        return 0
    fi
    if ! $purge && ! $keep && interactive; then
        choose "Your settings, themes and session are in $(tilde "$settings")." 1 \
            'Delete them' 'Keep them, for a later installation'
        ((choice == 0)) && purge=true
    fi
    if $purge; then
        rm -rf -- "$settings"
        ok 'Settings deleted.'
    else
        info "Settings remain in $(tilde "$settings")."
    fi
}

install_deb() {
    local version=$1 work=$2 name
    name=oxyde_${version}-1_amd64.deb
    fetch_release_file "$version" "$name" "$work"
    step 'Installing the DEB package'
    info 'apt asks for your password.'
    # apt reads the package from a path that its sandbox user can access.
    chmod 755 "$work"
    chmod 644 "$work/$name"
    sudo apt-get install -y "$work/$name"
    ok "Oxyde $version installed"
}

remove_deb() {
    step 'Uninstalling Oxyde'
    info 'apt asks for your password.'
    sudo apt-get remove -y oxyde
    ok 'Oxyde is uninstalled.'
    local running=false
    is_running && running=true
    handle_settings false false "$running"
}

# The version of a DEB package installed, without its revision.
deb_version() {
    command -v dpkg-query >/dev/null || return 0
    dpkg-query -W -f='${Status} ${Version}\n' oxyde 2>/dev/null \
        | awk '$3 == "installed" { sub(/-[^-]*$/, "", $4); print $4 }' || true
}

is_default() {
    [[ $(xdg-mime query default inode/directory 2>/dev/null || true) == "$app_id.desktop" ]]
}

# The version an archive file or folder holds, from its name.
archive_version() {
    local name
    name=$(basename "$1")
    name=${name%.tar.gz}
    [[ $name == oxyde-*-x86_64-linux ]] || return 0
    name=${name#oxyde-}
    say "${name%-x86_64-linux}"
}

menu_labels=()
menu_actions=()
offer() { menu_labels+=("$1"); menu_actions+=("$2"); }

# Says what is installed and asks what to do; sets action.
main_menu() {
    local target=$1 with_deb=$2 choosing_format=$3 release_ok=$4 current deb have
    current=$([[ -L $lib_root/current ]] && readlink "$lib_root/current" || true)
    deb=$(deb_version)
    have=${current:-$deb}
    local name="Oxyde${target:+ $target}"
    menu_labels=() menu_actions=()
    say ''
    if [[ -n $have ]]; then
        if [[ -n $current ]]; then
            ok "Oxyde $have is installed in $(tilde "$lib_root"), for you."
        else
            ok "Oxyde $have is installed by its DEB package."
        fi
        if ! $release_ok; then
            warn "$name cannot be installed now (see above); the rest remains possible."
        elif [[ -z $target || $have == "$target" ]]; then
            offer "Reinstall $name" install
        elif [[ $(printf '%s\n%s\n' "$have" "$target" | sort -V | tail -n 1) == "$target" ]]; then
            offer "Update to $name" install
        else
            offer "Go back to $name" install
        fi
        if [[ -n $current ]]; then
            offer 'Uninstall Oxyde' uninstall
        else
            offer 'Uninstall Oxyde (removes the DEB package)' remove_deb
        fi
    else
        if $choosing_format; then
            say "  $name is ready to install."
        elif $with_deb; then
            say "  $name will be installed with its DEB package, through apt."
        else
            say "  $name will be installed in $(tilde "$lib_root"), for you only:"
            say '  no administrator password is needed.'
        fi
        system_entry_exists && warn 'A system package also provides Oxyde; this installation takes its place in menus.'
        offer "Install $name" install
    fi
    offer 'Quit' quit
    choose 'What would you like to do?' 0 "${menu_labels[@]}"
    action=${menu_actions[choice]}
}

# On Ubuntu, only the DEB's AppArmor profile lets bubblewrap confine decoders: offer
# it in place of the archive, before anything is installed.
choose_ubuntu_format() {
    step 'DEB package or archive'
    say '    On Ubuntu, the DEB package confines previews and archives with bubblewrap,'
    say '    allowed by its AppArmor profile. This archive keeps Landlock and seccomp'
    say '    alone: it protects less against a malicious file.'
    choose 'How would you like to install Oxyde?' 0 \
        'Install the DEB package (recommended, asks for your password)' \
        'Install this archive for me only (less isolation, no password)'
    if ((choice == 0)); then
        archive=''
        with_deb=true
        [[ -n $version ]] || version=$(resolve_version)
    else
        isolation_accepted=true
    fi
}

cleanup=()
remove_temporary() {
    show_cursor
    ((${#cleanup[@]} == 0)) || rm -rf -- "${cleanup[@]}"
}

main() {
    trap remove_temporary EXIT
    local archive='' set_default_handler=false remove=false purge=false update=false
    requested_version=
    assume_yes=false
    while (($#)); do
        case $1 in
            --version) requested_version=${2:?--version needs a value}; shift ;;
            --archive) archive=${2:?--archive needs a file}; shift ;;
            --set-default) set_default_handler=true ;;
            --yes|-y) assume_yes=true ;;
            --uninstall) remove=true ;;
            --purge) purge=true ;;
            --update) update=true ;;
            --help|-h) usage; return 0 ;;
            *) warn "Unknown argument: $1"; usage >&2; return 2 ;;
        esac
        shift
    done

    if $purge && ! $remove; then
        die '--purge goes with --uninstall.'
    fi
    if $remove; then
        banner
        uninstall "$purge"
        return 0
    fi
    if $update && [[ -z $archive ]]; then
        die '--update needs --archive.'
    fi
    $update || banner
    check_platform
    local family
    family=$(os_family)

    # Run from an extracted archive: install that folder.
    local here=${BASH_SOURCE[0]:-}
    if [[ -z $archive && -f $here ]]; then
        here=$(cd -- "$(dirname -- "$here")" && pwd)
        [[ $(basename "$here") == oxyde-*-x86_64-linux && -x $here/bin/oxyde ]] && archive=$here
    fi
    local version
    if [[ -n $archive ]]; then
        archive=$(realpath -- "$archive")
        [[ -e $archive ]] || die "$archive does not exist."
        version=$(archive_version "$archive")
    else
        version=$(resolve_version)
    fi
    local with_deb=false
    [[ -z $archive && $family == debian ]] && with_deb=true
    local menu=false
    if ! $update && ! $assume_yes && ! $set_default_handler && interactive; then
        menu=true
    fi

    # What would stop the installation is checked before any question: the tools,
    # the release and its signature. Only the download itself remains to fail.
    local work='' name='' release_ok=true
    if [[ -z $archive ]]; then
        work=$(mktemp -d)
        cleanup+=("$work")
        if $with_deb; then name=oxyde_${version}-1_amd64.deb; else name=oxyde-$version-x86_64-linux.tar.gz; fi
        ( fetch_checksums "$version" "$name" "$work" ) || release_ok=false
        if ! $release_ok; then
            # Someone with Oxyde installed may still want to uninstall it.
            if $menu && [[ -L $lib_root/current || -n $(deb_version) ]]; then
                rm -f "$work/SHA256SUMS"
            else
                exit 1
            fi
        fi
    fi

    # Without options, a person chooses; options act directly, as scripts expect.
    if $menu; then
        local action
        local choosing_format=false
        [[ -n $archive ]] && is_ubuntu && choosing_format=true
        main_menu "$version" "$with_deb" "$choosing_format" "$release_ok"
        case $action in
            quit) say ''; say '  Nothing was changed.'; return 0 ;;
            uninstall) uninstall false; return 0 ;;
            remove_deb) remove_deb; return 0 ;;
        esac
        $choosing_format && choose_ubuntu_format
    fi

    if [[ -n $archive ]]; then
        $update || step 'Installing'
        install_archive "$archive"
    else
        if [[ -z $work ]]; then
            # The DEB chosen in place of an archive.
            work=$(mktemp -d)
            cleanup+=("$work")
        fi
        if $with_deb; then
            install_deb "$version" "$work"
            $set_default_handler && set_default
            finish "$version"
            return 0
        fi
        fetch_release_file "$version" "$name" "$work"
        step 'Installing'
        install_archive "$work/$name"
    fi
    $update && return 0
    $set_default_handler && set_default
    check_dependencies "$family"
    finish "$version"
}

finish() {
    step "${good}${s_ok}${reset}${bold} Oxyde${1:+ $1} is ready."
    say '    Find it among your applications, or type oxyde in a terminal.'
    if ! is_default; then
        say '    To have it open folders for other applications, turn on'
        say "    ${bold}Preferences $s_path System integration $s_path Default file manager${reset}."
    fi
    say '    Run this installer again to update or uninstall it.'
    say ''
}

# Sourced, by the tests, it only defines its functions.
if [[ -z ${BASH_SOURCE[0]:-} || ${BASH_SOURCE[0]} == "$0" ]]; then
    main "$@"
fi
