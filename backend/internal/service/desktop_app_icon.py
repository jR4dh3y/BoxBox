"""Resolve an installed entry's icon through the host GTK theme and return a small PNG."""
import os
import sys

import gi

gi.require_version("Gtk", "3.0")
gi.require_version("GdkPixbuf", "2.0")
from gi.repository import GdkPixbuf, Gtk

icon = sys.argv[1]
if os.path.isabs(icon):
    image = GdkPixbuf.Pixbuf.new_from_file_at_scale(icon, 32, 32, True)
else:
    available, _ = Gtk.init_check()
    theme = Gtk.IconTheme.get_default() if available else Gtk.IconTheme.new()
    if not available:
        theme.set_custom_theme("hicolor")
    # Older desktop entries sometimes include the extension on a themed icon.
    names = [icon, os.path.splitext(icon)[0]] if icon.endswith((".png", ".svg", ".xpm")) else [icon]
    info = theme.choose_icon(names, 32, Gtk.IconLookupFlags.FORCE_SIZE)
    if info is None:
        sys.exit(1)
    image = info.load_icon()
success, data = image.save_to_bufferv("png", [], [])
if not success:
    sys.exit(1)
sys.stdout.buffer.write(data)
