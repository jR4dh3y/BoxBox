import { existsSync, readFileSync, readdirSync } from "node:fs";
import { basename, join, resolve } from "node:path";
import { decodeHTML } from "entities";
import { marked } from "marked";
import sanitizeHtml from "sanitize-html";
import { codeToHtml } from "shiki";

export type DocNavItem = {
  slug: string;
  title: string;
  description: string;
};

export type DocHeading = {
  depth: number;
  text: string;
  id: string;
};

export type DocPage = DocNavItem & {
  html: string;
  headings: DocHeading[];
};

const docsDirectory = resolveDocsDirectory();

const docNav = [
  {
    slug: "index",
    title: "Overview",
    description:
      "Start here: what BoxBox is and where to find each project reference.",
  },
  {
    slug: "quickstart",
    title: "Quick Start",
    description:
      "Install BoxBox quickly with Docker Compose and the published GHCR image.",
  },
  {
    slug: "docker",
    title: "Docker Deployment",
    description:
      "Deploy from GHCR with Compose, port binding, and optional reverse proxy examples.",
  },
  {
    slug: "release",
    title: "Release Workflow",
    description:
      "Use test branches, test GHCR images, and stable tags without moving latest too early.",
  },
  {
    slug: "configuration",
    title: "Configuration",
    description:
      "Configure users, mount points, origins, upload limits, ports, and environment overrides.",
  },
  {
    slug: "api",
    title: "API Reference",
    description:
      "REST endpoints, chunked uploads, streaming previews, background jobs, and WebSocket messages.",
  },
  {
    slug: "development",
    title: "Development",
    description:
      "Run the Go backend, SvelteKit app, and Astro website locally.",
  },
  {
    slug: "security",
    title: "Security",
    description:
      "Harden credentials, mounted paths, origins, reverse proxy behavior, and upload storage.",
  },
  {
    slug: "architecture",
    title: "Architecture",
    description:
      "How the Go server, embedded SvelteKit app, services, jobs, and storage paths fit together.",
  },
  {
    slug: "troubleshooting",
    title: "Troubleshooting",
    description:
      "Fix common deployment, login, mount, upload, preview, and WebSocket issues.",
  },
] satisfies DocNavItem[];

const docMeta = new Map(docNav.map(({ slug, ...meta }) => [slug, meta]));

marked.setOptions({
  gfm: true,
  breaks: false,
});

const safeDocTags = [
  ...sanitizeHtml.defaults.allowedTags,
  "h1",
  "h2",
  "h3",
  "h4",
  "h5",
  "h6",
  "img",
  "kbd",
  "span",
  "table",
  "thead",
  "tbody",
  "tr",
  "th",
  "td",
  "details",
  "summary",
];

const baseSanitizeOptions = {
  allowedTags: safeDocTags,
  allowedAttributes: {
    a: ["href", "name", "target", "rel"],
    code: ["class"],
    img: ["src", "alt", "title", "width", "height", "loading"],
    th: ["align"],
    td: ["align"],
  },
  allowedSchemes: ["http", "https", "mailto", "tel"],
  allowProtocolRelative: false,
};

const highlightedSanitizeOptions = {
  ...baseSanitizeOptions,
  allowedAttributes: {
    ...baseSanitizeOptions.allowedAttributes,
    h1: ["id"],
    h2: ["id"],
    h3: ["id"],
    h4: ["id"],
    h5: ["id"],
    h6: ["id"],
    pre: ["class", "style", "tabindex"],
    span: ["class", "style"],
  },
  allowedStyles: {
    pre: {
      "background-color": [/^#[0-9a-f]{3,8}$/i],
      color: [/^#[0-9a-f]{3,8}$/i],
    },
    span: {
      color: [/^#[0-9a-f]{3,8}$/i],
    },
  },
};

export function getDocNav(): DocNavItem[] {
  const available = new Set(getDocSlugs());
  return docNav.filter((doc) => available.has(doc.slug));
}

export function getDocSlugs(): string[] {
  return readdirSync(docsDirectory)
    .filter((file) => file.endsWith(".md"))
    .map((file) => basename(file, ".md"));
}

export async function getDocPage(slug: string): Promise<DocPage> {
  const normalizedSlug = slug === "" ? "index" : slug;
  const markdown = readFileSync(
    join(docsDirectory, `${normalizedSlug}.md`),
    "utf8",
  );
  const rendered = sanitizeDocHtml(await marked.parse(markdown));
  const linked = normalizeMarkdownDocLinks(rendered);
  const highlighted = sanitizeHighlightedHtml(
    await highlightCodeBlocks(linked),
  );
  const { html, headings } = addHeadingIds(highlighted);
  const meta = docMeta.get(normalizedSlug) ?? {
    title: titleFromMarkdown(markdown) ?? titleFromSlug(normalizedSlug),
    description: "",
  };

  return {
    slug: normalizedSlug,
    ...meta,
    html: sanitizeHighlightedHtml(html),
    headings,
  };
}

function sanitizeDocHtml(html: string): string {
  return sanitizeHtml(html, baseSanitizeOptions);
}

function sanitizeHighlightedHtml(html: string): string {
  return sanitizeHtml(html, highlightedSanitizeOptions);
}

function normalizeMarkdownDocLinks(html: string): string {
  return html.replace(
    /href="([^"#][^"]*?\.md)(#[^"]*)?"/g,
    (_match, href: string, hash = "") => {
      if (/^[a-z][a-z0-9+.-]*:/i.test(href) || href.startsWith("/")) {
        return `href="${href}${hash}"`;
      }

      const fileName = href.split("/").pop() ?? href;
      const slug = fileName.replace(/\.md$/, "");
      const path = slug === "index" ? "/docs/" : `/docs/${slug}/`;
      return `href="${path}${hash}"`;
    },
  );
}

function resolveDocsDirectory(): string {
  const fromRepoRoot = resolve(process.cwd(), "docs");
  if (existsSync(fromRepoRoot)) {
    return fromRepoRoot;
  }

  return resolve(process.cwd(), "..", "docs");
}

function addHeadingIds(html: string): { html: string; headings: DocHeading[] } {
  const headings: DocHeading[] = [];
  const usedIds = new Map<string, number>();

  const withIds = html.replace(
    /<h([23])>(.*?)<\/h\1>/g,
    (_match, depth: string, content: string) => {
      const text = htmlToText(content);
      const baseId = slugify(text);
      const count = usedIds.get(baseId) ?? 0;
      const id = count === 0 ? baseId : `${baseId}-${count + 1}`;
      usedIds.set(baseId, count + 1);
      headings.push({ depth: Number(depth), text, id });
      return `<h${depth} id="${id}">${content}</h${depth}>`;
    },
  );

  return { html: withIds, headings };
}

async function highlightCodeBlocks(html: string): Promise<string> {
  const codeBlockPattern =
    /<pre><code(?: class="language-([^"]+)")?>([\s\S]*?)<\/code><\/pre>/g;
  const parts: string[] = [];
  let lastIndex = 0;

  for (const match of html.matchAll(codeBlockPattern)) {
    const [block, language, encodedCode] = match;
    const index = match.index ?? 0;
    parts.push(html.slice(lastIndex, index));
    parts.push(await highlightCodeBlock(decodeHTML(encodedCode), language));
    lastIndex = index + block.length;
  }

  parts.push(html.slice(lastIndex));
  return parts.join("");
}

async function highlightCodeBlock(
  code: string,
  language?: string,
): Promise<string> {
  const lang = normalizeCodeLanguage(language);

  try {
    return await codeToHtml(code, {
      lang,
      theme: "github-dark-default",
    });
  } catch {
    return codeToHtml(code, {
      lang: "text",
      theme: "github-dark-default",
    });
  }
}

function normalizeCodeLanguage(language?: string): string {
  if (!language) {
    return "text";
  }

  const aliases: Record<string, string> = {
    plaintext: "text",
    sh: "bash",
    shell: "bash",
    yml: "yaml",
    compose: "yaml",
    dockerfile: "docker",
  };

  return aliases[language] ?? language;
}

function htmlToText(value: string): string {
  return decodeHTML(
    sanitizeHtml(value, { allowedTags: [], allowedAttributes: {} }),
  ).trim();
}

function titleFromMarkdown(markdown: string): string | null {
  const match = markdown.match(/^#\s+(.+)$/m);
  return match?.[1] ?? null;
}

function titleFromSlug(slug: string): string {
  return slug
    .split("-")
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(" ");
}

function slugify(value: string): string {
  return value
    .toLowerCase()
    .replace(/`/g, "")
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}
