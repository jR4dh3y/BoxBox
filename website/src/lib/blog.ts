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
    title: "What’s new in BoxBox v0.4.0",
    date: "2026-11-26",
    description: "Release notes for BoxBox v0.4.0.",
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
