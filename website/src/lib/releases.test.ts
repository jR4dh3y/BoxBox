import { test } from "node:test";
import assert from "node:assert/strict";
import { parseRelease } from "./releases";
const prefix = "https://github.com/jR4dh3y/BoxBox/releases/download/v0.4.0/";
const name = "boxbox-desktop-v0.4.0-linux-amd64.tar.gz";

test("older server-only releases cannot advertise a desktop download", () => {
  assert.equal(
    parseRelease({ tag_name: "v0.3.1", name: "Security update", assets: [] })
      ?.desktop,
    null,
  );
});
test("published desktop and checksums use exact assets from the same version", () => {
  const release = parseRelease({
    tag_name: "v0.4.0",
    assets: [
      { name, browser_download_url: prefix + name, size: 200000000 },
      { name: "SHA256SUMS", browser_download_url: prefix + "SHA256SUMS" },
    ],
  });
  assert.equal(release?.desktop?.url, prefix + name);
  assert.equal(release?.checksumsUrl, prefix + "SHA256SUMS");
});
test("malformed releases and off-origin asset URLs are rejected", () => {
  assert.equal(parseRelease(null), null);
  assert.equal(parseRelease({ tag_name: "../../unsafe" }), null);
  assert.equal(
    parseRelease({
      tag_name: "v0.4.0",
      assets: [
        { name, browser_download_url: "https://example.test/" + name, size: 1 },
      ],
    })?.desktop,
    null,
  );
  assert.equal(
    parseRelease({
      tag_name: "v0.4.0",
      assets: [{ name, browser_download_url: prefix + name, size: -1 }],
    })?.desktop,
    null,
  );
});
