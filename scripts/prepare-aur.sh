#!/bin/bash
set -euo pipefail
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
destination=${1:?Usage: prepare-aur.sh destination version archive-sha256}
version=${2:?Missing version}
archive_sha256=${3:?Missing archive checksum}
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]
[[ "$archive_sha256" =~ ^[a-f0-9]{64}$ ]]
mkdir -p "$destination"
sed -e "s/^pkgver=.*/pkgver=$version/" \
    -e "s/^_archive_sha256=.*/_archive_sha256=$archive_sha256/" \
    "$root/packaging/aur/PKGBUILD" > "$destination/PKGBUILD"
