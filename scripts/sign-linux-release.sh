#!/usr/bin/env bash
# Sign a release's Linux files with Oxyde's package key, as Soliton does: the RPM gets
# an embedded signature, the archive, the DEB and the RPM a detached armored .asc (the
# update manifest carries them), then a single SHA256SUMS lists every release file,
# Windows included, with its detached signature SHA256SUMS.asc. The per-file .sha256
# are removed. Every signature is checked against packaging/linux/signing before the
# script succeeds. The Windows Setup keeps its own minisign signature (.sig).
# Needs gpg, gpgv, rpmsign and rpmkeys. Keys come from the environment:
#   OXYDE_GPG_PRIVATE_KEY (armored secret key), OXYDE_GPG_PASSPHRASE
# Usage: bash scripts/sign-linux-release.sh <version> <directory>
set -euo pipefail

if [[ $# -ne 2 ]]; then
    printf 'Usage: %s <version> <directory>\n' "$0" >&2
    exit 2
fi
version=$1
dir=$(cd -- "$2" && pwd)
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
public_key=$root/packaging/linux/signing/oxyde-packages.asc
for name in OXYDE_GPG_PRIVATE_KEY OXYDE_GPG_PASSPHRASE; do
    if [[ -z ${!name:-} ]]; then
        printf 'Missing %s.\n' "$name" >&2
        exit 1
    fi
done

archive=$dir/oxyde-$version-x86_64-linux.tar.gz
deb=$dir/oxyde_$version-1_amd64.deb
rpms=("$dir"/oxyde-"$version"-1.*.x86_64.rpm)
for file in "$archive" "$deb" "${rpms[@]}"; do
    [[ -f $file ]] || { printf 'Missing %s.\n' "$file" >&2; exit 1; }
done

work=$(mktemp -d)
trap 'rm -rf -- "$work"' EXIT
chmod 700 "$work"
umask 077
printf '%s\n' "$OXYDE_GPG_PASSPHRASE" > "$work/passphrase"
export GNUPGHOME=$work/gnupg
mkdir "$GNUPGHOME"
printf '%s\n' "$OXYDE_GPG_PRIVATE_KEY" | gpg --batch --quiet --import
fingerprint=$(gpg --batch --with-colons --show-keys "$public_key" | awk -F: '$1 == "fpr" { print $10; exit }')
if ! gpg --batch --with-colons --list-secret-keys "$fingerprint" >/dev/null 2>&1; then
    printf 'OXYDE_GPG_PRIVATE_KEY is not the key of %s.\n' "$public_key" >&2
    exit 1
fi
sign() { gpg --batch --yes --pinentry-mode loopback --passphrase-file "$work/passphrase" \
    --local-user "$fingerprint" --armor --detach-sign -o "$1.asc" "$1"; }

# rpmsign rewrites the header, so the RPM is signed before anything hashes it.
for rpm in "${rpms[@]}"; do
    rpmsign --addsign --define "__gpg /usr/bin/gpg" --define "_gpg_name $fingerprint" \
        --define "_gpg_sign_cmd_extra_args --batch --pinentry-mode loopback --passphrase-file $work/passphrase" \
        "$rpm" >/dev/null
done
for file in "$archive" "$deb" "${rpms[@]}"; do
    sign "$file"
done

rm -f -- "$dir"/*.sha256 "$dir/SHA256SUMS" "$dir/SHA256SUMS.asc"
(
    cd "$dir"
    for file in *; do
        case $file in
            *.sig|*.asc|latest.json) continue ;;
        esac
        [[ -f $file ]] && sha256sum "$file"
    done
) > "$work/SHA256SUMS"
mv "$work/SHA256SUMS" "$dir/SHA256SUMS"
chmod 644 "$dir/SHA256SUMS"
sign "$dir/SHA256SUMS"
chmod 644 "$dir"/*.asc

# Checks with nothing but the committed public key.
gpg --batch --quiet --dearmor < "$public_key" > "$work/keyring.gpg"
mkdir "$work/check"
for signed in "$archive" "$deb" "${rpms[@]}" "$dir/SHA256SUMS"; do
    gpgv --homedir "$work/check" --keyring "$work/keyring.gpg" "$signed.asc" "$signed" 2>/dev/null \
        || { printf 'Signature check failed: %s\n' "$signed" >&2; exit 1; }
done
rpm --dbpath "$work/rpmdb" --initdb
rpmkeys --dbpath "$work/rpmdb" --import "$public_key"
for rpm in "${rpms[@]}"; do
    rpmkeys --dbpath "$work/rpmdb" -K "$rpm" | grep -q 'signatures OK' \
        || { printf 'RPM signature check failed: %s\n' "$rpm" >&2; exit 1; }
done
(cd "$dir" && sha256sum --quiet -c SHA256SUMS)
printf 'Signed with GPG key %s.\n' "$fingerprint"
