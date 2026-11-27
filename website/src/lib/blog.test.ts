import { test } from "node:test";
import assert from "node:assert/strict";
import { renderBlogMarkdown } from "./blog";

test("blog markdown highlights code and only keeps images from /blog/", async () => {
  const { html } = await renderBlogMarkdown([
    "![Screenshot](/blog/v0.4.0/files.webp)",
    "![Remote](https://example.com/blog/files.webp)",
    "![Outside](/blog/%2e%2e/logo.svg)",
    '<img src="/blog/files.webp" onerror="alert(1)">',
    "```sh",
    "echo BoxBox",
    "```",
  ].join("\n\n"));

  assert.match(html, /<img src="\/blog\/v0\.4\.0\/files\.webp"/);
  assert.match(html, /<img src="\/blog\/files\.webp"/);
  assert.doesNotMatch(html, /example\.com|logo\.svg|onerror/);
  assert.match(html, /<pre class="shiki/);
});
