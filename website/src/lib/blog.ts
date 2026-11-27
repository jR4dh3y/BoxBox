import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { renderMarkdown } from "./docs";

type BlogPost = {
  slug: string;
  version: `v${string}`;
  title: string;
  date: string;
  description: string;
};

export const blogPosts: readonly BlogPost[] = ([
  {
    slug: "v0-4-0",
    version: "v0.4.0",
    title: "BoxBox v0.4.0: native desktop app, Windows support, and way less weight",
    date: "2026-11-26",
    description: "BoxBox v0.4.0 is out. The big change is the desktop app. I dropped Electrobun and rewrote it in Rust with GPUI. It is still boxbox, it keeps the same look, it runs on Windows alongside Linux now, and it drops the bundled Chromium browser, Bun runtime, and private Go server for a single native binary.",
  },
] satisfies BlogPost[]).sort((a, b) => b.date.localeCompare(a.date));

export function blogPostHref(slug: string): string {
  return `/blog/${slug}/`;
}

const [newestPost] = blogPosts;
export const latestReleaseLink = newestPost && {
  href: blogPostHref(newestPost.slug),
  text: `What’s new in ${newestPost.version}`,
};

export function readBlogMarkdown(slug: string): string {
  const fromWebsite = resolve("src/blog");
  const directory = existsSync(fromWebsite) ? fromWebsite : resolve("website/src/blog");
  return readFileSync(resolve(directory, `${slug}.md`), "utf8");
}

export function renderBlogMarkdown(markdown: string) {
  return renderMarkdown(markdown, { imageRoot: "/blog/" });
}
