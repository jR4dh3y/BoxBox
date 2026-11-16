#!/bin/sh
# Installs BoxBox Files for the current user, under ~/.local. No root needed.
#   ./install.sh                 install or update
#   ./install.sh --set-default   also make it the default file manager
#   ./install.sh --uninstall     remove it
set -eu
base=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
bin="${XDG_BIN_HOME:-$HOME/.local/bin}"
data="${XDG_DATA_HOME:-$HOME/.local/share}"
desktop="$data/applications/io.boxbox.files.desktop"
icon="$data/icons/hicolor/scalable/apps/io.boxbox.files.svg"

# A release archive holds every file beside this script; a checkout builds them.
first() { for file; do [ -f "$file" ] && { echo "$file"; return; }; done; echo "missing: $1" >&2; exit 1; }

case "${1:-}" in
--uninstall)
	rm -f "$bin/boxbox-files" "$desktop" "$icon"
	echo "Removed BoxBox Files. Settings stay in ${XDG_CONFIG_HOME:-$HOME/.config}/boxbox-gpui."
	exit 0
	;;
"" | --set-default) ;;
*)
	echo "usage: $0 [--set-default | --uninstall]" >&2
	exit 2
	;;
esac

install -Dm755 "$(first "$base/boxbox-files" "$base/../target/release/boxbox-gpui")" "$bin/boxbox-files"
mkdir -p "$(dirname "$desktop")"
sed "s|@EXEC@|$bin/boxbox-files|" "$base/io.boxbox.files.desktop" >"$desktop"
install -Dm644 "$(first "$base/io.boxbox.files.svg" "$base/../../frontend/static/logo-boxbox-3d.svg")" "$icon"
command -v update-desktop-database >/dev/null && update-desktop-database "$data/applications" || true
if [ "${1:-}" = --set-default ]; then
	xdg-mime default io.boxbox.files.desktop inode/directory
fi
echo "Installed BoxBox Files to $bin/boxbox-files."
