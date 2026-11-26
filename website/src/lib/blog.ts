import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { renderMarkdown, type DocHeading } from "./docs";

export type BlogPost = {
  slug: string;
  version: `v${string}`;
  title: string;
  date: string;
  description: string;
};

export type BlogImageFrame = {
  src: string;
  label: string;
  badge: string;
  width: number;
  height: number;
};

export type BlogComparisonSpec = {
  id: string;
  title: string;
  alt: string;
  caption: string;
  before: BlogImageFrame;
  after: BlogImageFrame;
  supportsSlider: boolean;
};

export type BlogSegment =
  | { kind: "html"; html: string }
  | { kind: "comparison"; comparison: BlogComparisonSpec };

export type RenderedBlogArticle = {
  segments: BlogSegment[];
  headings: DocHeading[];
  readingMinutes: number;
};

export const blogPosts: readonly BlogPost[] = ([
  {
    slug: "v0-4-0",
    version: "v0.4.0",
    title: "BoxBox v0.4.0: native desktop app, Windows support, and way less weight",
    date: "2026-11-26",
    description:
      "BoxBox v0.4.0 is out. The big change is the desktop app. I dropped Electrobun and rewrote it in Rust with GPUI. It is still boxbox, it keeps the same look, it runs on Windows alongside Linux now, and it drops the bundled Chromium browser, Bun runtime, and private Go server for a single native binary.",
  },
] satisfies BlogPost[]).sort((a, b) => b.date.localeCompare(a.date));

const blogComparisons = new Map<string, BlogComparisonSpec>([
  [
    "/blog/v0.4.0/compare-grid.png",
    {
      id: "compare-grid",
      title: "Grid view",
      alt: "Web grid on the left and the native BoxBox grid on the right",
      caption: "Web UI alongside native GPUI",
      before: {
        src: "/blog/v0.4.0/grid-web.png",
        label: "Web UI",
        badge: "v0.3.2",
        width: 1280,
        height: 577,
      },
      after: {
        src: "/blog/v0.4.0/grid-native.png",
        label: "Native GPUI",
        badge: "v0.4.0",
        width: 1280,
        height: 800,
      },
      supportsSlider: false,
    },
  ],
  [
    "/blog/v0.4.0/compare-list.png",
    {
      id: "compare-list",
      title: "List view",
      alt: "The old Electrobun app on the left and the native list view on the right",
      caption: "Electrobun compared with native GPUI",
      before: {
        src: "/blog/v0.4.0/list-electrobun.png",
        label: "Electrobun",
        badge: "v0.3.2",
        width: 1024,
        height: 576,
      },
      after: {
        src: "/blog/v0.4.0/list-native.png",
        label: "Native GPUI",
        badge: "v0.4.0",
        width: 1024,
        height: 576,
      },
      supportsSlider: true,
    },
  ],
]);

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

export function formatBlogDate(isoDate: string): string {
  const parsed = new Date(`${isoDate}T00:00:00Z`);
  if (Number.isNaN(parsed.getTime())) return isoDate;
  return new Intl.DateTimeFormat("en-US", {
    month: "long",
    day: "numeric",
    year: "numeric",
    timeZone: "UTC",
  }).format(parsed);
}

export function estimateReadingMinutes(markdown: string): number {
  const words = markdown
    .replace(/!\[[^\]]*\]\([^)]+\)/g, " ")
    .replace(/[`#*_>-]/g, " ")
    .trim()
    .split(/\s+/)
    .filter(Boolean).length;
  return Math.max(1, Math.round(words / 220));
}

const comparisonParagraphPattern = /<p>\s*<img\s+[^>]*src="([^"]+)"[^>]*>\s*<\/p>/g;

export async function renderBlogArticle(markdown: string): Promise<RenderedBlogArticle> {
  const { html, headings } = await renderBlogMarkdown(markdown);
  const segments: BlogSegment[] = [];
  let cursor = 0;

  for (const match of html.matchAll(comparisonParagraphPattern)) {
    const [fullMatch, src] = match;
    const comparison = blogComparisons.get(src);
    if (!comparison) continue;

    const index = match.index ?? 0;
    const preceding = html.slice(cursor, index).trim();
    if (preceding) segments.push({ kind: "html", html: preceding });

    segments.push({ kind: "comparison", comparison });
    cursor = index + fullMatch.length;
  }

  const trailing = html.slice(cursor).trim();
  if (trailing) segments.push({ kind: "html", html: trailing });

  return {
    segments,
    headings,
    readingMinutes: estimateReadingMinutes(markdown),
  };
}
