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
  { slug: "desktop", title: "Linux Desktop", description: "Download, install, pair, update and remove the Linux desktop app." },
  { slug: "connect-api", title: "Paired Client API", description: "Authorize desktop devices and revoke server access." },
  {
    slug: "docker-options",
    title: "Docker Options",
    description:
      "Reverse proxy, single-container and source-build setups, volumes, and mount propagation.",
  },
  {
    slug: "release",
    title: "Release Workflow",
    description:
      "Use test branches, test GHCR images, and stable tags without moving latest too early.",
  },
  {
    slug: "release-notes",
    title: "Release Notes",
    description: "What changed in each release and what to do when you upgrade.",
  },
  {
    slug: "configuration",
    title: "Configuration",
    description:
      "Configure users, origins, upload limits, ports, and environment overrides.",
  },
  {
    slug: "mounts",
    title: "Mount Points",
    description:
      "Choose the folders BoxBox shows: drives, places, read-only mounts, and Docker paths.",
  },
  {
    slug: "nas",
    title: "NAS Users",
    description: "Read and write as each person's NAS user so files get the right owner.",
  },
  {
    slug: "api",
    title: "API Reference",
    description:
      "Base URLs, authentication, errors, and an index of the endpoint pages.",
  },
  { slug: "api-files", title: "Files API", description: "List mounts and folders, read file info, and create, move or delete entries." },
  { slug: "api-transfers", title: "Downloads and Uploads", description: "Download files, archive folders, stream previews, and upload in chunks." },
  { slug: "api-shares", title: "Share Links API", description: "Create, list, update and revoke share links, and the recipient endpoints." },
  { slug: "api-system", title: "Jobs, Settings and WebSocket", description: "Background jobs, server settings, and the live event stream." },
  {
    slug: "development",
    title: "Development",
    description:
      "Run the Go backend, SvelteKit app, and Astro website locally.",
  },
  {
    slug: "testing",
    title: "Testing and CI",
    description: "Test and validate changes locally, and how CI picks the checks a change needs.",
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
      "Fix problems starting BoxBox and signing in.",
  },
  {
    slug: "troubleshooting-files",
    title: "Files and Sharing Problems",
    description: "Fix mount, permission, upload, preview, share link, and WebSocket issues.",
  },
] as const satisfies readonly DocNavItem[];

type DocSlug = (typeof docNav)[number]["slug"];

export type DocNavSection = {
  title: string;
  items: DocNavItem[];
};

/** The sidebar's sections, in order. Every page in docNav belongs to exactly one. */
const docSections: { title: string; slugs: DocSlug[] }[] = [
  { title: "Get started", slugs: ["index", "quickstart", "docker", "docker-options", "desktop"] },
  {
    title: "Run it",
    slugs: ["configuration", "mounts", "nas", "security", "troubleshooting", "troubleshooting-files", "release-notes"],
  },
  {
    title: "Reference",
    slugs: ["api", "api-files", "api-transfers", "api-shares", "api-system", "connect-api", "architecture"],
  },
  { title: "Contribute", slugs: ["development", "testing", "release"] },
];

const docMeta = new Map<string, Omit<DocNavItem, "slug">>(docNav.map(({ slug, ...meta }) => [slug, meta]));

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

/** The available pages, grouped for the sidebar; empty sections are left out. */
export function getDocSections(): DocNavSection[] {
  const pages = new Map(getDocNav().map((doc) => [doc.slug, doc]));
  return docSections
    .map(({ title, slugs }) => ({
      title,
      items: slugs.flatMap((slug) => pages.get(slug) ?? []),
    }))
    .filter((section) => section.items.length > 0);
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
  const rendered = await renderMarkdown(markdown, { docLinks: true });
  const meta = docMeta.get(normalizedSlug) ?? {
    title: titleFromMarkdown(markdown) ?? titleFromSlug(normalizedSlug),
    description: "",
  };

  return {
    slug: normalizedSlug,
    ...meta,
    ...rendered,
  };
}

export async function renderMarkdown(
  markdown: string,
  options: { docLinks?: boolean; imageRoot?: `/${string}/` } = {},
): Promise<Pick<DocPage, "html" | "headings">> {
  const { imageRoot } = options;
  const rendered = sanitizeHtml(await marked.parse(markdown), {
    ...baseSanitizeOptions,
    exclusiveFilter: ({ tag, attribs }) => {
      if (tag !== "img" || !imageRoot) return false;
      const src = attribs.src ?? "";
      return !src.startsWith(imageRoot) ||
        /%2f|%5c/i.test(src) ||
        !new URL(src, "https://boxbox.invalid").pathname.startsWith(imageRoot);
    },
  });
  const linked = options.docLinks ? normalizeMarkdownDocLinks(rendered) : rendered;
  const highlighted = sanitizeHtml(await highlightCodeBlocks(linked), highlightedSanitizeOptions);
  const { html, headings } = addHeadingIds(highlighted);
  return { html: sanitizeHtml(html, highlightedSanitizeOptions), headings };
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
