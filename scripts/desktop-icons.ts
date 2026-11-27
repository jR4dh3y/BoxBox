// Renders the BoxBox logo into the square PNGs the Linux desktop entry uses.
// Qt-based shells draw the logo's SVG (masks, currentColor, <use href>) as a
// black shape, so the packages ship these instead. Run after the logo changes:
//   bun scripts/desktop-icons.ts
import { mkdir } from "node:fs/promises";
import { join } from "node:path";

const root = join(import.meta.dir, "..");
const logo = join(root, "frontend/static/logo-boxbox-3d.svg");
const output = join(root, "desktop/packaging/icons");
// The logo's viewBox, 570 by 410: it is centred on a square page.
const [width, height] = [570, 410];
const SIZES = [16, 24, 32, 48, 64, 128, 256, 512];

await mkdir(output, { recursive: true });
for (const size of SIZES) {
  const drawn = Math.round((size * height) / width);
  const render = Bun.spawn(
    [
      "rsvg-convert",
      "--width",
      String(size),
      "--height",
      String(drawn),
      "--page-width",
      String(size),
      "--page-height",
      String(size),
      "--top",
      String(Math.floor((size - drawn) / 2)),
      "--output",
      join(output, `${size}.png`),
      logo,
    ],
    { stdout: "inherit", stderr: "inherit" },
  );
  if ((await render.exited) !== 0) throw new Error(`Could not render the ${size}px icon`);
}
