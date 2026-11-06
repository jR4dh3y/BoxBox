#!/bin/bash
set -euo pipefail
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
destination=${1:?Usage: prepare-aur.sh destination version archive-sha256}
version=${2:?Missing version}
archive_sha256=${3:?Missing archive checksum}
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]
[[ "$archive_sha256" =~ ^[a-f0-9]{64}$ ]]
mkdir -p "$destination"
cp "$root/desktop/assets/boxbox.portal" "$destination/boxbox.portal"
cp "$root/desktop/assets/org.freedesktop.impl.portal.desktop.boxbox.service" "$destination/org.freedesktop.impl.portal.desktop.boxbox.service"
cp "$root/desktop/assets/AppRun" "$destination/AppRun"
sed 's|@EXEC@|/usr/bin/boxbox|' "$root/desktop/assets/boxbox.desktop" > "$destination/boxbox.desktop"
cp "$root/frontend/static/logo-boxbox-3d.svg" "$destination/boxbox.svg"
portal_sha256=$(sha256sum "$destination/boxbox.portal" | cut -d' ' -f1)
service_sha256=$(sha256sum "$destination/org.freedesktop.impl.portal.desktop.boxbox.service" | cut -d' ' -f1)
apprun_sha256=$(sha256sum "$destination/AppRun" | cut -d' ' -f1)
desktop_sha256=$(sha256sum "$destination/boxbox.desktop" | cut -d' ' -f1)
icon_sha256=$(sha256sum "$destination/boxbox.svg" | cut -d' ' -f1)
sed -e "s/^pkgver=.*/pkgver=$version/" \
    -e "s/^_archive_sha256=.*/_archive_sha256=$archive_sha256/" \
    -e "s/@APPRUN_SHA256@/$apprun_sha256/" \
    -e "s/@DESKTOP_SHA256@/$desktop_sha256/" \
    -e "s/@ICON_SHA256@/$icon_sha256/" \
    -e "s/@PORTAL_SHA256@/$portal_sha256/" \
    -e "s/@SERVICE_SHA256@/$service_sha256/" \
    "$root/packaging/aur/PKGBUILD" > "$destination/PKGBUILD"
