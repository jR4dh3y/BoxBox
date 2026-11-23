#!/bin/sh
# Installs BoxBox for the current user, under ~/.local. No root needed. Its
# entry, io.boxbox.desktop, takes the place of the Electrobun app's.
#   ./install.sh                 install or update
#   ./install.sh --set-default   also make it the default file manager
#   ./install.sh --set-picker    also use it for other apps' Open and Save dialogs
#   ./install.sh --uninstall     remove it, and give the dialogs back
set -eu
base=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
bin="${XDG_BIN_HOME:-$HOME/.local/bin}"
data="${XDG_DATA_HOME:-$HOME/.local/share}"
desktop="$data/applications/io.boxbox.desktop"
icons="$data/icons/hicolor"
sizes="16 24 32 48 64 128 256 512"
# What earlier installs left: the BoxBox Files entry, and SVG icons that Qt shells draw black.
stale="$data/applications/io.boxbox.files.desktop $icons/scalable/apps/io.boxbox.files.svg $icons/scalable/apps/io.boxbox.desktop.svg"

# A release archive holds every file beside this script; a checkout builds them.
first() { for file; do [ -f "$file" ] && { echo "$file"; return; }; done; echo "missing: $1" >&2; exit 1; }

case "${1:-}" in
--uninstall)
	[ -x "$bin/boxbox-files" ] && "$bin/boxbox-files" --disable-picker || true
	rm -f "$bin/boxbox-files" "$desktop" $stale
	for size in $sizes; do rm -f "$icons/${size}x$size/apps/io.boxbox.desktop.png"; done
	echo "Removed BoxBox. Settings stay in ${XDG_CONFIG_HOME:-$HOME/.config}/boxbox-gpui."
	exit 0
	;;
"" | --set-default | --set-picker) ;;
*)
	echo "usage: $0 [--set-default | --set-picker | --uninstall]" >&2
	exit 2
	;;
esac

install -Dm755 "$(first "$base/boxbox-files" "$base/../target/release/boxbox-gpui")" "$bin/boxbox-files"
mkdir -p "$(dirname "$desktop")"
# Desktop Entry quoting: inside double quotes, \ " ` $ take a backslash; then the
# value's own string escaping doubles every backslash; % is written %%.
exec_path=$(printf '%s' "$bin/boxbox-files" | sed -e 's/[\\"`$]/\\&/g' -e 's/\\/\\\\/g' -e 's/%/%%/g')
while IFS= read -r line; do
	case $line in
	Exec=@EXEC@*) printf 'Exec="%s"%s\n' "$exec_path" "${line#Exec=@EXEC@}" ;;
	*) printf '%s\n' "$line" ;;
	esac
done <"$base/io.boxbox.desktop" >"$desktop"
rm -f $stale
for size in $sizes; do
	install -Dm644 "$base/icons/$size.png" "$icons/${size}x$size/apps/io.boxbox.desktop.png"
done
# A theme cache hides icons it does not list; refresh one only where it exists.
[ -f "$icons/icon-theme.cache" ] && command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -qtf "$icons" || true
command -v update-desktop-database >/dev/null && update-desktop-database "$data/applications" || true
if [ "${1:-}" = --set-default ]; then
	xdg-mime default io.boxbox.desktop inode/directory
fi
if [ "${1:-}" = --set-picker ]; then
	"$bin/boxbox-files" --enable-picker
fi
echo "Installed BoxBox to $bin/boxbox-files."
