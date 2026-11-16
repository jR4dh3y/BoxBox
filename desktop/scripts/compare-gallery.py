#!/usr/bin/env python3
"""Compare each gallery cell between the web app and the native app.

    compare-gallery.py <web.png> <native.png> <out-dir> [--threshold PERCENT]

Both screenshots are taken at the manifest's size and device scale 1 (see
UI_MAP.md). A pixel differs when any channel differs by more than 24/255, which
absorbs font antialiasing. Writes web | native | diff strips per cell and
exits 1 when a cell's share of differing pixels exceeds the threshold.
"""

import json
import sys
from pathlib import Path

from PIL import Image, ImageChops

MANIFEST = Path(__file__).resolve().parents[3] / "frontend/src/routes/dev/gallery/gallery.json"
TOLERANCE = 24
# Cells GPUI cannot match exactly, with the reason. They are reported, not failed.
EXCEPTIONS = {
    "toggle-disabled": "GPUI 0.2 has no group opacity, so the dimmed knob blends with the track",
}


def main() -> int:
    args = sys.argv[1:]
    threshold = 3.0
    if "--threshold" in args:
        index = args.index("--threshold")
        threshold = float(args[index + 1])
        del args[index : index + 2]
    if len(args) != 3:
        print(__doc__)
        return 2
    web, native = (Image.open(path).convert("RGB") for path in args[:2])
    out = Path(args[2])
    out.mkdir(parents=True, exist_ok=True)
    manifest = json.loads(MANIFEST.read_text())
    failed = 0
    print(f"{'cell':22} differing")
    for cell in manifest["cells"]:
        box = (cell["x"], cell["y"], cell["x"] + cell["w"], cell["y"] + cell["h"])
        a, b = web.crop(box), native.crop(box)
        diff = ImageChops.difference(a, b)
        # A pixel differs if any channel does.
        channels = diff.split()
        mask = channels[0].point(lambda v: 255 if v > TOLERANCE else 0)
        for channel in channels[1:]:
            mask = ImageChops.lighter(mask, channel.point(lambda v: 255 if v > TOLERANCE else 0))
        differing = mask.histogram()[255]
        share = 100 * differing / (cell["w"] * cell["h"])
        excused = cell["id"] in EXCEPTIONS
        over = share > threshold and not excused
        mark = f"  (exception: {EXCEPTIONS[cell['id']]})" if excused else ("  ✗" if over else "")
        failed += over
        print(f"{cell['id']:22} {share:6.2f}%{mark}")
        strip = Image.new("RGB", (cell["w"] * 3 + 8, cell["h"]), (255, 0, 255))
        strip.paste(a, (0, 0))
        strip.paste(b, (cell["w"] + 4, 0))
        strip.paste(Image.merge("RGB", (mask, Image.new("L", mask.size), Image.new("L", mask.size))), (cell["w"] * 2 + 8, 0))
        strip.save(out / f"{cell['id']}.png")
    print(f"{failed} of {len(manifest['cells'])} cells over {threshold}%")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
