export interface MarkdownMetadata {
  title?: string;
  author?: string;
  date?: string;
  language?: string;
}

export interface ReaderTextBlock {
  id: string;
  kind: "heading" | "paragraph";
  text: string;
  level?: number;
  charRange: [number, number];
}

export interface ReaderImageBlock {
  id: string;
  kind: "image";
  src: string;
  alt: string;
  title?: string;
  charOffset: number;
}

export type ReaderBlock = ReaderTextBlock | ReaderImageBlock;

export interface ReaderChapter {
  id: string;
  title: string;
  level: number;
  charOffset: number;
}

export interface ReaderCleanupStats {
  anchors: number;
  attributes: number;
  htmlBlocks: number;
  navigationLines: number;
  boilerplateLines: number;
}

export interface ReaderDocument {
  metadata: MarkdownMetadata;
  markdown: string;
  analysisText: string;
  blocks: ReaderBlock[];
  chapters: ReaderChapter[];
  images: ReaderImageBlock[];
  cleanup: ReaderCleanupStats;
}

const FRONTMATTER_PATTERN = /^\uFEFF?---\r?\n([\s\S]*?)\r?\n---(?:\r?\n|$)/;
const SUPPORTED_KEYS = new Set<keyof MarkdownMetadata>([
  "title",
  "author",
  "date",
  "language",
]);
const RAW_HTML_FENCE = /^(```|~~~)\s*(?:\{=html\}|html|svg)\s*$/i;
const HEADING_PATTERN = /^(#{1,6})[ \t]+(.+?)\s*$/;
const EMPTY_ANCHOR_PATTERN = /^\s*\[\]\{#[^}]+\}\s*$/;
const PANDOC_DIV_PATTERN = /^\s*:::(?:\s+[^\s].*)?\s*$/;
const IMAGE_PATTERN = /!\[([^\]]*)\]\((?:<([^>]+)>|([^\s)]+))(?:\s+["']([^"']*)["'])?\)(?:\{[^}]*\})?/g;
const TOC_TITLE_PATTERN = /^(?:目次|もくじ|contents?|table of contents)$/i;
const EPUB_NOTICE_PATTERNS = [
  /この本は縦書きでレイアウトされています/,
  /ご覧になる機種により、?表示の差が認められることがあります/,
];

function parseScalar(value: string): string {
  const trimmed = value.trim();
  if (trimmed.length >= 2) {
    const quote = trimmed[0];
    if ((quote === '"' || quote === "'") && trimmed[trimmed.length - 1] === quote) {
      const inner = trimmed.slice(1, -1);
      return quote === '"'
        ? inner.replace(/\\([\\"nrt])/g, (_, escaped: string) => ({
            "\\": "\\",
            '"': '"',
            n: "\n",
            r: "\r",
            t: "\t",
          })[escaped] ?? escaped)
        : inner.replace(/''/g, "'");
    }
  }
  return trimmed;
}

function normalizeSpacedTitle(value: string): string {
  const characters = Array.from(value.trim());
  const visible = characters
    .map((character, index) => ({ character, index }))
    .filter(({ character }) => !/\s/u.test(character));
  if (visible.length < 3) return value.trim();
  const spacedBoundaries = visible.slice(1).filter((current, index) => {
    const previous = visible[index];
    return current.index > previous.index + 1
      && characters.slice(previous.index + 1, current.index).every((character) => /\s/u.test(character));
  }).length;
  return spacedBoundaries >= 2 && spacedBoundaries * 2 >= visible.length - 1
    ? visible.map(({ character }) => character).join("")
    : value.trim();
}

function extractFrontmatter(source: string): { body: string; metadata: MarkdownMetadata } {
  const match = FRONTMATTER_PATTERN.exec(source);
  if (!match) {
    return {
      body: source.replace(/^\uFEFF/, ""),
      metadata: {},
    };
  }

  const metadata: MarkdownMetadata = {};
  for (const line of match[1].split(/\r?\n/)) {
    const separator = line.indexOf(":");
    if (separator <= 0) continue;
    const key = line.slice(0, separator).trim() as keyof MarkdownMetadata;
    if (!SUPPORTED_KEYS.has(key)) continue;
    const parsed = parseScalar(line.slice(separator + 1));
    const value = key === "title" ? normalizeSpacedTitle(parsed) : parsed;
    if (value) metadata[key] = value;
  }

  return {
    body: source.slice(match[0].length).replace(/^\r?\n+/, ""),
    metadata,
  };
}

function codePointLength(value: string): number {
  return Array.from(value).length;
}

function decodeHtmlEntities(value: string): string {
  const named: Record<string, string> = {
    amp: "&",
    apos: "'",
    gt: ">",
    lt: "<",
    nbsp: " ",
    quot: '"',
  };
  return value.replace(/&(#x[\da-f]+|#\d+|[a-z]+);/gi, (entity, name: string) => {
    if (name[0] !== "#") return named[name.toLowerCase()] ?? entity;
    const hexadecimal = name[1]?.toLowerCase() === "x";
    const parsed = Number.parseInt(name.slice(hexadecimal ? 2 : 1), hexadecimal ? 16 : 10);
    return Number.isFinite(parsed) ? String.fromCodePoint(parsed) : entity;
  });
}

function stripHtmlTags(value: string): string {
  let result = "";
  let insideTag = false;
  for (const character of value) {
    if (character === "<") {
      insideTag = true;
      continue;
    }
    if (insideTag && character === ">") {
      insideTag = false;
      continue;
    }
    if (!insideTag) result += character;
  }
  return result;
}

function cleanInline(source: string, stats: ReaderCleanupStats): string {
  let value = source;
  value = value.replace(
    /`<ruby>`\{=html\}(.+?)`<rt>`\{=html\}(.+?)`<\/rt>`\{=html\}`<\/ruby>`\{=html\}/g,
    "$1《$2》",
  );
  value = value.replace(/\[\[#[^\]|]+\|([^\]]+)\]\]/g, "$1");
  value = value.replace(/\[\[#([^\]]+)\]\]/g, "$1");

  // Pandoc span 属性可能嵌套，逐层解包直到稳定。
  for (;;) {
    const cleaned = value.replace(/\[([^\[\]]*)\]\{[^}\n]*\}/g, "$1");
    if (cleaned === value) break;
    stats.attributes += 1;
    value = cleaned;
  }

  value = value.replace(/\[([^\]]+)\]\((?:<[^>]+>|[^)]+)\)(?:\{[^}]*\})?/g, "$1");
  value = value.replace(/\{(?:[.#][^}\n]*|[^}\n]*=[^}\n]*)\}/g, () => {
    stats.attributes += 1;
    return "";
  });
  value = value.replace(/\{=html\}/g, () => {
    stats.attributes += 1;
    return "";
  });
  value = stripHtmlTags(value);
  value = decodeHtmlEntities(value);
  value = value.replace(/(\*\*|__|~~)(.+?)\1/g, "$2");
  value = value.replace(/`([^`]+)`/g, "$1");
  value = value.replace(/^\s*>\s?/, "");
  value = value.replace(/^\s*[-+*]\s+/, "• ");
  value = value.replace(/\\([\\`*{}\[\]()#+.!_-])/g, "$1");
  value = value.replace(/[ \t]+$/g, "");
  return value.trim();
}

function looksLikeNavigationLine(line: string): boolean {
  const normalized = normalizeSpacedTitle(line);
  const markdownLinks = line.match(/\[[^\]]+\]\([^)]+\)/g)?.length ?? 0;
  const epubTargets = line.match(/(?:x?html?|toc[-_#]|a_m\d+|b_m\d+)/gi)?.length ?? 0;
  return TOC_TITLE_PATTERN.test(normalized)
    || (/^\s*contents?\b/i.test(normalized) && markdownLinks > 0)
    || (markdownLinks >= 2 && epubTargets >= 2)
    || /^\s*[-*]\s+\[\[#/.test(line);
}

function slugifyHeading(title: string, index: number): string {
  const slug = title
    .normalize("NFKC")
    .toLocaleLowerCase()
    .replace(/[^\p{Letter}\p{Number}]+/gu, "-")
    .replace(/^-|-$/g, "");
  return `chapter-${index + 1}-${slug || "untitled"}`;
}

export function prepareMarkdownDocument(source: string): { body: string; metadata: MarkdownMetadata } {
  return extractFrontmatter(source);
}

/**
 * 将已完成 EPUB 前置清理的 Markdown 编译为阅读器使用的稳定文档模型。
 * 这里保留防御性清理，但不会承担 EPUB spine、TOC 或资源解包职责。
 */
export function compileReaderDocument(source: string): ReaderDocument {
  const { body, metadata } = extractFrontmatter(source);
  const normalized = body.replace(/\r\n?/g, "\n");
  const stats: ReaderCleanupStats = {
    anchors: 0,
    attributes: 0,
    htmlBlocks: 0,
    navigationLines: 0,
    boilerplateLines: 0,
  };
  const draftBlocks: Array<
    | { kind: "heading"; text: string; level: number }
    | { kind: "paragraph"; text: string }
    | { kind: "image"; src: string; alt: string; title?: string }
  > = [];
  let paragraphLines: string[] = [];
  let rawFence: string | null = null;
  let skippingNavigation = false;

  const flushParagraph = () => {
    const text = paragraphLines.join("\n").trim();
    paragraphLines = [];
    if (!text) return;
    draftBlocks.push({ kind: "paragraph", text });
  };

  for (const rawLine of normalized.split("\n")) {
    const trimmed = rawLine.trim();
    if (rawFence) {
      if (trimmed.startsWith(rawFence)) rawFence = null;
      stats.htmlBlocks += 1;
      continue;
    }
    const fence = RAW_HTML_FENCE.exec(trimmed);
    if (fence) {
      flushParagraph();
      rawFence = fence[1];
      stats.htmlBlocks += 1;
      continue;
    }
    if (EMPTY_ANCHOR_PATTERN.test(trimmed)) {
      flushParagraph();
      stats.anchors += 1;
      continue;
    }
    if (PANDOC_DIV_PATTERN.test(trimmed) || /^<\/?(?:div|svg)(?:\s[^>]*)?>$/i.test(trimmed)) {
      flushParagraph();
      stats.htmlBlocks += 1;
      continue;
    }
    if (/^\\\s*$/.test(trimmed) || /^ {0,3}(?:[-*_]\s*){3,}$/.test(trimmed)) {
      flushParagraph();
      continue;
    }
    if (!trimmed) {
      flushParagraph();
      continue;
    }
    if (EPUB_NOTICE_PATTERNS.some((pattern) => pattern.test(trimmed))) {
      flushParagraph();
      stats.boilerplateLines += 1;
      continue;
    }

    const heading = HEADING_PATTERN.exec(trimmed);
    if (heading) {
      flushParagraph();
      const title = normalizeSpacedTitle(cleanInline(heading[2], stats));
      if (!title) continue;
      if (TOC_TITLE_PATTERN.test(title)) {
        skippingNavigation = true;
        stats.navigationLines += 1;
        continue;
      }
      skippingNavigation = false;
      const previous = draftBlocks[draftBlocks.length - 1];
      if (previous?.kind === "heading" && previous.text === title) continue;
      draftBlocks.push({ kind: "heading", text: title, level: heading[1].length });
      continue;
    }

    if (looksLikeNavigationLine(trimmed)) {
      flushParagraph();
      skippingNavigation = true;
      stats.navigationLines += 1;
      continue;
    }

    // 链接包裹的图片先还原为普通图片，避免资源被链接属性吞掉。
    const unwrapped = rawLine.replace(
      /\[(!\[[^\]]*\]\([^)]+\)(?:\{[^}]*\})?)\]\([^)]+\)(?:\{[^}]*\})?/g,
      "$1",
    );
    IMAGE_PATTERN.lastIndex = 0;
    const images = Array.from(unwrapped.matchAll(IMAGE_PATTERN));
    if (images.length > 0) {
      if (skippingNavigation) skippingNavigation = false;
      let cursor = 0;
      for (const image of images) {
        const prefix = cleanInline(unwrapped.slice(cursor, image.index), stats);
        if (prefix) paragraphLines.push(prefix);
        flushParagraph();
        draftBlocks.push({
          kind: "image",
          alt: image[1].trim(),
          src: (image[2] || image[3]).trim(),
          title: image[4]?.trim() || undefined,
        });
        cursor = (image.index ?? 0) + image[0].length;
      }
      const suffix = cleanInline(unwrapped.slice(cursor), stats);
      if (suffix) paragraphLines.push(suffix);
      continue;
    }

    if (skippingNavigation) {
      stats.navigationLines += 1;
      continue;
    }
    const cleaned = cleanInline(rawLine, stats);
    if (cleaned) paragraphLines.push(cleaned);
  }
  flushParagraph();

  const blocks: ReaderBlock[] = [];
  const chapters: ReaderChapter[] = [];
  const images: ReaderImageBlock[] = [];
  const analysisParts: string[] = [];
  let charOffset = 0;
  let textIndex = 0;
  let imageIndex = 0;

  for (const draft of draftBlocks) {
    if (draft.kind === "image") {
      const image: ReaderImageBlock = {
        id: `image-${++imageIndex}`,
        kind: "image",
        src: draft.src,
        alt: draft.alt,
        title: draft.title,
        charOffset,
      };
      blocks.push(image);
      images.push(image);
      continue;
    }
    if (analysisParts.length > 0) {
      analysisParts.push("\n\n");
      charOffset += 2;
    }
    const start = charOffset;
    analysisParts.push(draft.text);
    charOffset += codePointLength(draft.text);
    const block: ReaderTextBlock = {
      id: `text-${++textIndex}`,
      kind: draft.kind,
      text: draft.text,
      level: draft.kind === "heading" ? draft.level : undefined,
      charRange: [start, charOffset],
    };
    blocks.push(block);
    if (draft.kind === "heading") {
      chapters.push({
        id: slugifyHeading(draft.text, chapters.length),
        title: draft.text,
        level: draft.level,
        charOffset: start,
      });
    }
  }

  const markdown = draftBlocks.map((block) => {
    if (block.kind === "heading") return `${"#".repeat(block.level)} ${block.text}`;
    if (block.kind === "image") {
      const title = block.title ? ` \"${block.title.replace(/\"/g, "\\\"")}\"` : "";
      return `![${block.alt}](${block.src}${title})`;
    }
    return block.text;
  }).join("\n\n");

  return {
    metadata,
    markdown,
    analysisText: analysisParts.join(""),
    blocks,
    chapters,
    images,
    cleanup: stats,
  };
}
