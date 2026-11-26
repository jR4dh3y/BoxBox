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
    title: "BoxBox 0.4: the desktop app is native now",
    date: "2026-11-26",
    description: "The desktop app is a native Rust app now, it runs on Windows, and the web app picks up its file-manager features.",
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
