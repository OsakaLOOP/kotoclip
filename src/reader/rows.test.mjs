import assert from "node:assert/strict";
import test from "node:test";
import { buildReaderRows, rowIndexForOffset } from "./rows.ts";

const paragraphs = [
  { id: 0, tokens: [], isDialogue: false, charRange: [0, 8] },
  { id: 1, tokens: [], isDialogue: false, charRange: [10, 24] },
];

test("将图片和标题锚点合并到虚拟行但不改变段落对象", () => {
  const document = {
    metadata: {},
    markdown: "",
    analysisText: "",
    blocks: [],
    cleanup: {},
    chapters: [{ id: "chapter-1", title: "第一章", level: 2, charOffset: 0 }],
    images: [{ id: "image-1", kind: "image", src: "cover.jpeg", alt: "", charOffset: 0 }],
  };
  const rows = buildReaderRows(paragraphs, document, () => ({
    src: "asset://cover",
    width: 1200,
    height: 1600,
  }), true);
  assert.deepEqual(rows.map((row) => row.kind), ["image", "text", "text"]);
  assert.equal(rows[1].paragraph, paragraphs[0]);
  assert.equal(rows[1].heading.title, "第一章");
  assert.equal(rows[0].items[0].resolvedSrc, "asset://cover");
  assert.equal(rows[0].items[0].intrinsicHeight, 1600);
});

test("可按正文字符位置定位虚拟行", () => {
  const rows = buildReaderRows(paragraphs, null, () => undefined, true);
  assert.equal(rowIndexForOffset(rows, 0), 0);
  assert.equal(rowIndexForOffset(rows, 12), 1);
  assert.equal(rowIndexForOffset(rows, 99), 1);
});

test("段落 key 不随增量插入导致的临时 ID 变化", () => {
  const before = buildReaderRows(paragraphs, null, () => undefined, false);
  const after = buildReaderRows(
    paragraphs.map((paragraph) => ({ ...paragraph, id: paragraph.id + 20 })),
    null,
    () => undefined,
    false,
  );
  assert.deepEqual(after.map((row) => row.key), before.map((row) => row.key));
});

test("连续的近 400px 竖版图片按顺序两两配对", () => {
  const images = Array.from({ length: 4 }, (_, index) => ({
    id: `image-${index + 1}`,
    kind: "image",
    src: `page-${index + 1}.jpeg`,
    alt: "",
    charOffset: 0,
  }));
  const document = {
    metadata: {},
    markdown: "",
    analysisText: "",
    blocks: images,
    cleanup: {},
    chapters: [],
    images,
  };
  const rows = buildReaderRows([], document, (src) => ({
    src: `asset://${src}`,
    width: src === "page-1.jpeg" || src === "page-2.jpeg" ? 390 : 371,
    height: 545,
  }), true);

  assert.deepEqual(rows.map((row) => row.layout), ["pair", "pair"]);
  assert.deepEqual(rows.map((row) => row.items.length), [2, 2]);
});

test("不同正文锚点或尺寸差异过大的竖版图片保持单页", () => {
  const images = [
    { id: "image-1", kind: "image", src: "page-1.jpeg", alt: "", charOffset: 0 },
    { id: "image-2", kind: "image", src: "page-2.jpeg", alt: "", charOffset: 1 },
    { id: "image-3", kind: "image", src: "page-3.jpeg", alt: "", charOffset: 1 },
  ];
  const document = {
    metadata: {},
    markdown: "",
    analysisText: "",
    blocks: images,
    cleanup: {},
    chapters: [],
    images,
  };
  const rows = buildReaderRows([], document, (src) => ({
    src: `asset://${src}`,
    width: src === "page-3.jpeg" ? 430 : 390,
    height: src === "page-3.jpeg" ? 650 : 545,
  }), true);

  assert.deepEqual(rows.map((row) => row.layout), ["single", "single", "single"]);
});
