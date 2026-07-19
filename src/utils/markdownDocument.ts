export interface MarkdownMetadata {
  title?: string;
  author?: string;
  date?: string;
  language?: string;
}

export interface PreparedMarkdownDocument {
  body: string;
  metadata: MarkdownMetadata;
}

const FRONTMATTER_PATTERN = /^\uFEFF?---\r?\n([\s\S]*?)\r?\n---(?:\r?\n|$)/;
const SUPPORTED_KEYS = new Set<keyof MarkdownMetadata>([
  "title",
  "author",
  "date",
  "language",
]);

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

export function prepareMarkdownDocument(source: string): PreparedMarkdownDocument {
  const match = FRONTMATTER_PATTERN.exec(source);
  if (!match) return { body: source, metadata: {} };

  const metadata: MarkdownMetadata = {};
  for (const line of match[1].split(/\r?\n/)) {
    const separator = line.indexOf(":");
    if (separator <= 0) continue;
    const key = line.slice(0, separator).trim() as keyof MarkdownMetadata;
    if (!SUPPORTED_KEYS.has(key)) continue;
    const value = parseScalar(line.slice(separator + 1));
    if (value) metadata[key] = value;
  }

  return {
    body: source.slice(match[0].length).replace(/^\r?\n+/, ""),
    metadata,
  };
}
