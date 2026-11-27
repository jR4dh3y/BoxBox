import { test } from "node:test";
import assert from "node:assert/strict";
import { blogPosts, latestReleaseLink, readBlogMarkdown, renderBlogMarkdown } from "./blog";

test("blog posts are listed newest first with unique slugs", () => {
  const dates = blogPosts.map((post) => post.date);
  assert.deepEqual(dates, [...dates].sort().reverse());
  assert.equal(new Set(blogPosts.map((post) => post.slug)).size, blogPosts.length);
});

test("every listed post has a markdown file", () => {
  assert.ok(blogPosts.length > 0);
  for (const post of blogPosts) {
    assert.ok(readBlogMarkdown(post.slug).trim(), `${post.slug}.md is empty`);
  }
});

test("the nav link uses the newest blog post", () => {
  const [newest] = blogPosts;
  assert.ok(newest);
  assert.deepEqual(latestReleaseLink, {
    href: `/blog/${newest.slug}/`,
    text: `What’s new in ${newest.version}`,
  });
});

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
