import { test } from "node:test";
import assert from "node:assert/strict";
import { readdirSync, statSync } from "node:fs";
import { join, resolve } from "node:path";

function totalBytesIn(relativeDir: string): number {
  const dir = resolve(process.cwd(), relativeDir);
  return readdirSync(dir).reduce((sum, name) => sum + statSync(join(dir, name)).size, 0);
}

test("homepage wallpapers stay within the compressed WebP byte ceiling", () => {
  const totalBytes = totalBytesIn("public/wallpapers");
  // Down from 27.4 MB of raw PNG/JPEG files to ~813 KB WebP; lower this ceiling when shrinking further.
  assert.ok(totalBytes > 0 && totalBytes <= 850 * 1024, `wallpapers totaled ${totalBytes} bytes`);
});

test("footer parallax scene layers stay within their byte ceiling", () => {
  const totalBytes = totalBytesIn("public/parallax/scene");
  assert.ok(totalBytes > 0 && totalBytes <= 100 * 1024, `parallax/scene totaled ${totalBytes} bytes`);
});
