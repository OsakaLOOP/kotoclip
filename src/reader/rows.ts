import type { Paragraph } from "../composables/useTokenization.ts";
import type { ReaderChapter, ReaderDocument, ReaderImageBlock } from "./document.ts";

export interface ReaderTextRow {
  key: string;
  kind: "text";
  paragraph: Paragraph;
  heading?: ReaderChapter;
}

export interface ReaderImageRow {
  key: string;
  kind: "image";
  image: ReaderImageBlock;
  resolvedSrc?: string;
}

export type ReaderRow = ReaderTextRow | ReaderImageRow;

export function buildReaderRows(
  paragraphs: Paragraph[],
  document: ReaderDocument | null,
  resolveImage: (src: string) => string | undefined,
  complete: boolean,
): ReaderRow[] {
  if (!document) {
    return paragraphs.map((paragraph) => ({
      key: `paragraph-${paragraph.id}-${paragraph.charRange[0]}`,
      kind: "text",
      paragraph,
    }));
  }

  const rows: ReaderRow[] = [];
  const images = [...document.images].sort((left, right) => left.charOffset - right.charOffset);
  let imageIndex = 0;
  for (const paragraph of paragraphs) {
    while (imageIndex < images.length && images[imageIndex].charOffset <= paragraph.charRange[0]) {
      const image = images[imageIndex++];
      rows.push({
        key: image.id,
        kind: "image",
        image,
        resolvedSrc: resolveImage(image.src),
      });
    }
    const heading = document.chapters.find((chapter) =>
      chapter.charOffset >= paragraph.charRange[0] && chapter.charOffset < paragraph.charRange[1]
    );
    rows.push({
      key: `paragraph-${paragraph.id}-${paragraph.charRange[0]}`,
      kind: "text",
      paragraph,
      heading,
    });
  }
  if (complete) {
    while (imageIndex < images.length) {
      const image = images[imageIndex++];
      rows.push({
        key: image.id,
        kind: "image",
        image,
        resolvedSrc: resolveImage(image.src),
      });
    }
  }
  return rows;
}

export function rowCharacterOffset(row: ReaderRow | undefined): number {
  if (!row) return 0;
  return row.kind === "image" ? row.image.charOffset : row.paragraph.charRange[0];
}

export function rowIndexForOffset(rows: ReaderRow[], offset: number): number {
  const exact = rows.findIndex((row) => {
    if (row.kind === "image") return row.image.charOffset >= offset;
    return row.paragraph.charRange[1] > offset;
  });
  return exact < 0 ? Math.max(0, rows.length - 1) : exact;
}
