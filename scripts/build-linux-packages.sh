#!/usr/bin/env bash
# Build Oxyde's RPM on Fedora or its DEB on Debian/Ubuntu from one release build. A
# package only runs on systems at least as recent as the one that built it.
set -euo pipefail

app_id=io.github.Carlton2001.Oxyde

usage() {
    printf 'Usage: bash scripts/build-linux-packages.sh (--rpm | --deb) [--offline] [--skip-build]\n'
    printf 'Writes to target/linux-packages/output. --offline reuses cached crates and keeps\n'
    printf 'the notices of the previous run, which needs every crate of every target.\n'
}

format=
offline=()
skip_build=false
for argument in "$@"; do
    case "$argument" in
        --rpm) format=rpm ;;
        --deb) format=deb ;;
        --offline) offline=(--offline) ;;
        --skip-build) skip_build=true ;;
        --help|-h) usage; exit 0 ;;
        *) printf 'Unknown argument: %s\n' "$argument" >&2; usage >&2; exit 2 ;;
    esac
done
[[ -n $format ]] || { usage >&2; exit 2; }

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd -- "$root"
target=${CARGO_TARGET_DIR:-$root/target}
work=$target/linux-packages
stage=$work/stage-$format
output=$work/output
notices=$work/THIRD-PARTY-NOTICES.md
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
date=$(git -c safe.directory='*' log -1 --format=%cs 2>/dev/null || date -u +%F)

if ! $skip_build; then
    # The application and the two workers that run on Linux; the image worker is empty here.
    cargo build --locked --release "${offline[@]}" \
        --bin oxyde --bin oxyde-archive-io-worker --bin oxyde-archive-list-worker
fi
if [[ ${#offline[@]} -eq 0 || ! -s $notices ]]; then
    mkdir -p "$work"
    python3 -I scripts/linux-third-party-notices.py "$notices"
fi

rm -rf "$stage"
install -d "$stage/usr/bin"
for binary in oxyde oxyde-archive-io-worker oxyde-archive-list-worker; do
    install -m 755 -s "$target/release/$binary" "$stage/usr/bin/$binary"
done
install -Dm 644 "packaging/linux/$app_id.desktop" "$stage/usr/share/applications/$app_id.desktop"
install -Dm 644 assets/icons/logo_oxyde.svg "$stage/usr/share/icons/hicolor/scalable/apps/$app_id.svg"
for icon in packaging/linux/icons/*/"$app_id.png"; do
    size=$(basename "$(dirname "$icon")")
    install -Dm 644 "$icon" "$stage/usr/share/icons/hicolor/$size/apps/$app_id.png"
done
install -d "$stage/usr/share/metainfo"
sed -e "s/@VERSION@/$version/" -e "s/@DATE@/$date/" "packaging/linux/$app_id.metainfo.xml" \
    > "$stage/usr/share/metainfo/$app_id.metainfo.xml"
chmod 644 "$stage/usr/share/metainfo/$app_id.metainfo.xml"

desktop-file-validate "$stage/usr/share/applications/$app_id.desktop"
appstreamcli validate --no-net "$stage/usr/share/metainfo/$app_id.metainfo.xml"

licenses() {
    install -Dm 644 "$notices" "$1/THIRD-PARTY-NOTICES.md"
    install -m 644 licenses/dependencies/LUCIDE-LICENSE.txt licenses/dependencies/LUCIDE-SOURCE.txt "$1"
    for notice in README.md PowerShell-LICENSE.txt PowerShell-NOTICES.txt SCSS-LICENSE.txt bat-LICENSE-MIT.txt; do
        install -Dm 644 "vendor/syntaxes/$notice" "$1/syntaxes/$notice"
    done
}

mkdir -p "$output"
case $format in
    rpm)
        licenses "$stage/usr/share/licenses/oxyde"
        install -m 644 LICENSE "$stage/usr/share/licenses/oxyde/LICENSE"
        rpmbuild -bb packaging/linux/oxyde.spec \
            --define "_topdir $work/rpmbuild" \
            --define "_rpmdir $output" \
            --define "_build_name_fmt %%{NAME}-%%{VERSION}-%%{RELEASE}.%%{ARCH}.rpm" \
            --define "oxyde_stage $stage" \
            --define "oxyde_version $version"
        ;;
    deb)
        licenses "$stage/usr/share/doc/oxyde"
        {
            printf 'Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n'
            printf 'Source: https://github.com/Carlton2001/Oxyde_FM\n\n'
            printf 'Files: *\nCopyright: Carlton2001\nLicense: GPL-3.0-only\n'
            printf 'Comment: Third-party notices are in /usr/share/doc/oxyde.\n\n'
            printf 'License: GPL-3.0-only\n'
            sed -e 's/^$/./' -e 's/^/ /' LICENSE
        } > "$stage/usr/share/doc/oxyde/copyright"
        install -Dm 644 "packaging/linux/apparmor/$app_id" "$stage/etc/apparmor.d/$app_id"
        # dpkg-shlibdeps insists on reading a source package's control file.
        shlibs=$work/shlibs
        rm -rf "$shlibs"
        install -d "$shlibs/debian" "$stage/DEBIAN"
        : > "$shlibs/debian/control"
        depends=$(cd "$shlibs" && dpkg-shlibdeps -O "$stage/usr/bin/oxyde" \
            "$stage/usr/bin/oxyde-archive-io-worker" "$stage/usr/bin/oxyde-archive-list-worker")
        depends=${depends#shlibs:Depends=}
        size=$(du -sk --exclude=DEBIAN "$stage" | cut -f 1)
        sed -e "s/@VERSION@/$version/" -e "s/@SIZE@/$size/" -e "s/@DEPENDS@/$depends/" \
            packaging/linux/debian/control > "$stage/DEBIAN/control"
        install -m 644 packaging/linux/debian/conffiles "$stage/DEBIAN/conffiles"
        install -m 755 packaging/linux/debian/postinst packaging/linux/debian/prerm "$stage/DEBIAN"
        dpkg-deb --root-owner-group -Zxz --build "$stage" "$output/oxyde_${version}_amd64.deb"
        ;;
esac
printf 'Packages in %s:\n' "$output"
ls -l "$output"
