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

say() { printf '%s\n' "$*"; }
warn() { printf '%s\n' "$*" >&2; }
die() { warn "Error: $*"; exit 1; }

usage() {
    say 'Usage: install.sh [--version X.Y.Z] [--archive FILE] [--set-default] [--yes]'
    say '       install.sh --uninstall'
    say ''
    say '  --version      install this release instead of the one this script belongs to'
    say '  --archive      install a downloaded oxyde-*-x86_64-linux.tar.gz or its folder, offline'
    say '  --set-default  make Oxyde the default application for folders'
    say '  --yes          install missing system packages without asking'
    say '  --uninstall    remove the per-user installation; settings are kept'
    say '  --update       with --archive, as Oxyde runs it: no dependency check or question'
    say 'Running it again installs the newer release and keeps the previous one.'
}

# Answers yes to a question; reads the terminal because stdin may be this script.
confirm() {
    $assume_yes && return 0
    local answer
    { printf '%s [Y/n] ' "$1" > /dev/tty && read -r answer < /dev/tty; } 2>/dev/null || return 1
    [[ -z $answer || $answer == [yYoO]* ]]
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

is_atomic() { [[ -e /run/ostree-booted ]]; }

download() {
    local url=$1 destination=$2
    if command -v curl >/dev/null; then
        curl -fL --proto '=https' --tlsv1.2 --retry 3 --progress-bar -o "$destination" "$url"
    elif command -v wget >/dev/null; then
        wget -q --show-progress -O "$destination" "$url"
    else
        die 'curl or wget is required.'
    fi
}

# Downloads a release file and checks it against the .sha256 published beside it.
fetch_release_file() {
    local version=$1 name=$2 directory=$3
    local base=https://github.com/$repository/releases/download/v$version
    say "Downloading $name"
    download "$base/$name" "$directory/$name" || die "$name is not in release v$version."
    download "$base/$name.sha256" "$directory/$name.sha256" 2>/dev/null \
        || die "$name.sha256 is not in release v$version."
    (cd "$directory" && sha256sum --quiet -c "$name.sha256") \
        || die "$name does not match its SHA-256; nothing was installed."
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

check_dependencies() {
    local family=$1 column
    case $family in
        fedora) column=4 ;;
        arch) column=5 ;;
        suse) column=6 ;;
        debian) column=7 ;;
        *) column=4 ;;
    esac
    local -a packages=() lines=()
    local level check feature fedora arch suse debian names required_missing=false
    while IFS='|' read -r level check feature fedora arch suse debian; do
        probe "$check" && continue
        case $column in 4) names=$fedora ;; 5) names=$arch ;; 6) names=$suse ;; 7) names=$debian ;; esac
        [[ $level == required ]] && required_missing=true
        lines+=("  [$level] $feature: $names")
        local name
        for name in $names; do
            [[ " ${packages[*]} " == *" $name "* ]] || packages+=("$name")
        done
    done < <(dependencies)

    ((${#packages[@]})) || return 0
    say ''
    say 'Missing system packages (each recommended one only removes its feature):'
    printf '%s\n' "${lines[@]}"

    local -a command
    case $family in
        fedora) if is_atomic; then command=(rpm-ostree install "${packages[@]}")
                else command=(sudo dnf install -y "${packages[@]}"); fi ;;
        arch) command=(sudo pacman -S --needed "${packages[@]}") ;;
        suse) command=(sudo zypper install "${packages[@]}") ;;
        debian) command=(sudo apt-get install -y "${packages[@]}") ;;
        *)
            say 'Package names above are Fedora'"'"'s; install the equivalents from your distribution.'
            $required_missing && warn 'Oxyde may not start until the required ones are installed.'
            return 0 ;;
    esac
    say ''
    say "Command: ${command[*]}"
    if is_atomic; then
        say 'On this image-based system the packages are layered and take effect after a reboot.'
    fi
    if confirm 'Run it now?'; then
        "${command[@]}" || warn 'The package installation failed; Oxyde is installed anyway.'
    elif $required_missing; then
        warn 'Oxyde may not start until the required packages are installed.'
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

    say ''
    if [[ -n $previous && $previous != "$version" ]]; then
        say "Oxyde $previous was updated to $version in $lib_root."
    else
        say "Oxyde $version is installed in $lib_root."
    fi
    if [[ " $(os_field ID) $(os_field ID_LIKE) " == *" ubuntu "* ]]; then
        warn 'On Ubuntu, bubblewrap only confines decoders for /usr/bin/oxyde; here they keep Landlock and seccomp alone. Prefer the DEB.'
    fi
    if [[ -e /usr/bin/oxyde ]]; then
        warn 'A system package also provides /usr/bin/oxyde; this installation takes its place in menus.'
    fi
    case ":$PATH:" in
        *":$(dirname "$bin_link"):"*) ;;
        *) say "Note: $(dirname "$bin_link") is not in PATH; the launcher still works." ;;
    esac
}

set_default() {
    if xdg-mime default "$app_id.desktop" inode/directory 2>/dev/null; then
        say 'Oxyde now opens folders by default.'
    else
        warn 'xdg-mime could not make Oxyde the default for folders.'
    fi
}

uninstall() {
    local found=false
    if [[ -f $entry ]]; then
        grep -qx "$marker" "$entry" || die "$entry was not written by this script; nothing is removed."
        found=true
    fi
    [[ -d $lib_root ]] && found=true
    if ! $found; then
        if [[ -e /usr/bin/oxyde ]]; then
            say 'Oxyde is installed by a system package: remove it with your package manager.'
        else
            say 'No per-user installation of Oxyde was found.'
        fi
        return 0
    fi
    if [[ -L $bin_link && $(readlink "$bin_link") == "$lib_root"/* ]]; then
        rm -f "$bin_link"
    fi
    rm -f "$entry" "$data/metainfo/$app_id.metainfo.xml"
    rm -f "$data"/icons/hicolor/*/apps/"$app_id".*
    [[ -d $data/icons/hicolor ]] && touch "$data/icons/hicolor"
    update-desktop-database -q "$data/applications" 2>/dev/null || true
    rm -rf -- "$lib_root"
    say "Oxyde is uninstalled. Settings remain in ${XDG_CONFIG_HOME:-$HOME/.config}/Oxyde."
}

install_deb() {
    local version=$1 work=$2 name
    name=oxyde_${version}-1_amd64.deb
    fetch_release_file "$version" "$name" "$work"
    say ''
    say 'Installing the DEB with apt, which asks for your password.'
    # apt reads the package from a path that its sandbox user can access.
    chmod 755 "$work"
    chmod 644 "$work/$name"
    sudo apt-get install -y "$work/$name"
    say "Oxyde $version is installed. Run this script again to update it."
}

cleanup=()
remove_temporary() { ((${#cleanup[@]} == 0)) || rm -rf -- "${cleanup[@]}"; }

main() {
    trap remove_temporary EXIT
    local archive= set_default_handler=false remove=false update=false
    requested_version=
    assume_yes=false
    while (($#)); do
        case $1 in
            --version) requested_version=${2:?--version needs a value}; shift ;;
            --archive) archive=${2:?--archive needs a file}; shift ;;
            --set-default) set_default_handler=true ;;
            --yes|-y) assume_yes=true ;;
            --uninstall) remove=true ;;
            --update) update=true ;;
            --help|-h) usage; return 0 ;;
            *) warn "Unknown argument: $1"; usage >&2; return 2 ;;
        esac
        shift
    done

    if $remove; then
        uninstall
        return 0
    fi
    if $update && [[ -z $archive ]]; then
        die '--update needs --archive.'
    fi
    check_platform
    local family
    family=$(os_family)

    # Run from an extracted archive: install that folder.
    local here=${BASH_SOURCE[0]:-}
    if [[ -z $archive && -f $here ]]; then
        here=$(cd -- "$(dirname -- "$here")" && pwd)
        [[ $(basename "$here") == oxyde-*-x86_64-linux && -x $here/bin/oxyde ]] && archive=$here
    fi
    if [[ -n $archive ]]; then
        archive=$(realpath -- "$archive")
        [[ -e $archive ]] || die "$archive does not exist."
        install_archive "$archive"
    else
        local version work
        version=$(resolve_version)
        work=$(mktemp -d)
        cleanup+=("$work")
        if [[ $family == debian ]]; then
            install_deb "$version" "$work"
            $set_default_handler && set_default
            return 0
        fi
        local name=oxyde-$version-x86_64-linux.tar.gz
        fetch_release_file "$version" "$name" "$work"
        install_archive "$work/$name"
    fi
    $update && return 0
    check_dependencies "$family"
    $set_default_handler && set_default
    say 'Oxyde appears in your applications. Run this script again to update it,'
    say 'or with --uninstall to remove it.'
}

main "$@"
