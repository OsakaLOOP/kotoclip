import type { Paragraph } from "../composables/useTokenization.ts";
import type { ReaderChapter, ReaderDocument, ReaderImageBlock } from "./document.ts";

export interface ReaderTextRow {
  key: string;
  kind: "text";
  paragraph: Paragraph;
  heading?: ReaderChapter;
}

export type ReaderImageLayout = "single" | "pair" | "symbols";

export interface ReaderRowImage {
  image: ReaderImageBlock;
  resolvedSrc?: string;
  intrinsicWidth?: number;
  intrinsicHeight?: number;
}

export interface ReaderImageRow {
  key: string;
  kind: "image";
  layout: ReaderImageLayout;
  items: ReaderRowImage[];
}

export type ReaderRow = ReaderTextRow | ReaderImageRow;

export interface ResolvedReaderImage {
  src: string;
  width?: number;
  height?: number;
}

// 部分 EPUB 将名义 400px 页面编码为 371/390px，保留约 10% 尺寸容差。
const PORTRAIT_PAGE_MIN_WIDTH = 360;
const PORTRAIT_PAGE_MIN_RATIO = 0.58;
const PORTRAIT_PAGE_MAX_RATIO = 0.82;
const PAIRED_PAGE_MAX_SIZE_RATIO = 1.12;

function isPortraitPage(item: ReaderRowImage): boolean {
  const { intrinsicWidth: width, intrinsicHeight: height } = item;
  if (!width || !height || width < PORTRAIT_PAGE_MIN_WIDTH) return false;
  const ratio = width / height;
  return ratio >= PORTRAIT_PAGE_MIN_RATIO && ratio <= PORTRAIT_PAGE_MAX_RATIO;
}

function canPairPortraitPages(items: ReaderRowImage[], index: number): boolean {
  if (index + 1 >= items.length) return false;
  const [left, right] = items.slice(index, index + 2);
  if (left.image.charOffset !== right.image.charOffset
    || !isPortraitPage(left)
    || !isPortraitPage(right)) return false;
  const widthRatio = Math.max(left.intrinsicWidth!, right.intrinsicWidth!)
    / Math.min(left.intrinsicWidth!, right.intrinsicWidth!);
  const heightRatio = Math.max(left.intrinsicHeight!, right.intrinsicHeight!)
    / Math.min(left.intrinsicHeight!, right.intrinsicHeight!);
  return widthRatio <= PAIRED_PAGE_MAX_SIZE_RATIO
    && heightRatio <= PAIRED_PAGE_MAX_SIZE_RATIO;
}

function isSmallSymbol(item: ReaderRowImage): boolean {
  const { intrinsicWidth: width, intrinsicHeight: height } = item;
  return Boolean(width && height && width <= 256 && height <= 256);
}

function imageRow(items: ReaderRowImage[], index: number): { row: ReaderImageRow; nextIndex: number } {
  let layout: ReaderImageLayout = "single";
  let grouped = [items[index]];
  if (canPairPortraitPages(items, index)) {
    layout = "pair";
    grouped = items.slice(index, index + 2);
  } else if (isSmallSymbol(items[index])) {
    layout = "symbols";
    const charOffset = items[index].image.charOffset;
    let end = index + 1;
    while (end < items.length
      && items[end].image.charOffset === charOffset
      && isSmallSymbol(items[end])) end++;
    grouped = items.slice(index, end);
  }
  return {
    row: {
      key: grouped.map((item) => item.image.id).join("+"),
      kind: "image",
      layout,
      items: grouped,
    },
    nextIndex: index + grouped.length,
  };
}

export function buildReaderRows(
  paragraphs: Paragraph[],
  document: ReaderDocument | null,
  resolveImage: (src: string) => ResolvedReaderImage | undefined,
  complete: boolean,
): ReaderRow[] {
  if (!document) {
    return paragraphs.map((paragraph) => ({
      key: paragraphKey(paragraph),
      kind: "text",
      paragraph,
    }));
  }

  const rows: ReaderRow[] = [];
  const images = [...document.images]
    .sort((left, right) => left.charOffset - right.charOffset)
    .map((image): ReaderRowImage => {
      const resolved = resolveImage(image.src);
      return {
        image,
        resolvedSrc: resolved?.src,
        intrinsicWidth: resolved?.width,
        intrinsicHeight: resolved?.height,
      };
    });
  const chapters = [...document.chapters].sort((left, right) => left.charOffset - right.charOffset);
  let imageIndex = 0;
  let chapterIndex = 0;
  for (const paragraph of paragraphs) {
    while (imageIndex < images.length && images[imageIndex].image.charOffset <= paragraph.charRange[0]) {
      const group = imageRow(images, imageIndex);
      rows.push(group.row);
      imageIndex = group.nextIndex;
    }
    while (chapterIndex < chapters.length && chapters[chapterIndex].charOffset < paragraph.charRange[0]) {
      chapterIndex++;
    }
    const chapter = chapters[chapterIndex];
    const heading = chapter?.charOffset < paragraph.charRange[1] ? chapter : undefined;
    if (heading) chapterIndex++;
    rows.push({
      key: paragraphKey(paragraph),
      kind: "text",
      paragraph,
      heading,
    });
  }
  if (complete) {
    while (imageIndex < images.length) {
      const group = imageRow(images, imageIndex);
      rows.push(group.row);
      imageIndex = group.nextIndex;
    }
  }
  return rows;
}

function paragraphKey(paragraph: Paragraph): string {
  // 渐进插入会改变数组下标和临时段落 ID，正文起点才是稳定身份。
  return `paragraph-${paragraph.charRange[0]}`;
}

export function rowCharacterOffset(row: ReaderRow | undefined): number {
  if (!row) return 0;
  return row.kind === "image" ? row.items[0].image.charOffset : row.paragraph.charRange[0];
}

export function rowIndexForOffset(rows: ReaderRow[], offset: number): number {
  const exact = rows.findIndex((row) => {
    if (row.kind === "image") return row.items[0].image.charOffset >= offset;
    return row.paragraph.charRange[1] > offset;
  });
  return exact < 0 ? Math.max(0, rows.length - 1) : exact;
}
