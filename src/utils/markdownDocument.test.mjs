import assert from "node:assert/strict";
import test from "node:test";
import { compileReaderDocument, prepareMarkdownDocument } from "./markdownDocument.ts";

test("提取 frontmatter 元数据并从正文剥离", () => {
  const prepared = prepareMarkdownDocument(`---
title: "测试书"
author: 测试作者
date: "2026-07-19"
language: ja
---

第一段正文。`);

  assert.deepEqual(prepared.metadata, {
    title: "测试书",
    author: "测试作者",
    date: "2026-07-19",
    language: "ja",
  });
  assert.equal(prepared.body, "第一段正文。");
});

test("没有 frontmatter 时保持正文不变", () => {
  const source = "普通正文。";
  assert.deepEqual(prepareMarkdownDocument(source), { body: source, metadata: {} });
});

test("将标题、正文和图片编译为带字符锚点的阅读文档", () => {
  const document = compileReaderDocument(`---
title: 本
author: 著者
---

## 第一章

第一段。
第二行。

![插图](./images/01.jpeg "场景")

## 第二章

第二段。`);

  assert.equal(document.analysisText, "第一章\n\n第一段。\n第二行。\n\n第二章\n\n第二段。");
  assert.deepEqual(document.chapters.map(({ title, charOffset }) => ({ title, charOffset })), [
    { title: "第一章", charOffset: 0 },
    { title: "第二章", charOffset: 16 },
  ]);
  assert.deepEqual(document.images.map(({ src, alt, charOffset }) => ({ src, alt, charOffset })), [
    { src: "./images/01.jpeg", alt: "插图", charOffset: 14 },
  ]);
  assert.match(document.markdown, /!\[插图\]\(\.\/images\/01\.jpeg "场景"\)/);
});

test("ruby 标记不计入章节和图片的后端字符坐标", () => {
  const document = compileReaderDocument(`## 序章

七《なの》日。

![插图](./images/01.jpeg)

## 第一章

本文。`);

  assert.equal(document.analysisText, "序章\n\n七《なの》日。\n\n第一章\n\n本文。");
  assert.deepEqual(document.chapters.map(({ title, charOffset }) => ({ title, charOffset })), [
    { title: "序章", charOffset: 0 },
    { title: "第一章", charOffset: 9 },
  ]);
  assert.equal(document.images[0].charOffset, 7);
  assert.deepEqual(document.blocks
    .filter((block) => block.kind !== "image")
    .map((block) => block.charRange), [[0, 2], [4, 7], [9, 12], [14, 17]]);
});

test("防御性清理截图中的 EPUB 锚点、TOC 和 Pandoc 属性", () => {
  const document = compileReaderDocument(`{.fit} この本は縦書きでレイアウトされています。

また、ご覧になる機種により、表示の差が認められることがあります。{.ft}

CONTENTS [プロローグ](#p-001.xhtml#toc-001) [第一章](#p-004.xhtml#toc-002) [第二章](#p-011.xhtml#toc-003)

[]{#p-001.xhtml}

## プロローグ

[ほんと]{.em-sesame_f}、もう、ムリ。

\`\`\`{=html}
<svg><image href="cover.jpeg" /></svg>
\`\`\`
`);

  assert.equal(document.analysisText, "プロローグ\n\nほんと、もう、ムリ。");
  assert.equal(document.chapters[0].title, "プロローグ");
  assert.equal(document.cleanup.anchors, 1);
  assert.equal(document.cleanup.navigationLines, 1);
  assert.equal(document.cleanup.boilerplateLines, 2);
  assert.ok(document.cleanup.htmlBlocks >= 2);
  assert.doesNotMatch(document.markdown, /(?:\.xhtml|\{\.|CONTENTS|<svg)/);
});

test("目次章节列表不会作为正文，后续图片会结束目次区", () => {
  const document = compileReaderDocument(`## 目次

- [[#第一章]]
- [[#第二章]]

![](./cover.jpeg)

## 第一章

正文。`);

  assert.equal(document.analysisText, "第一章\n\n正文。");
  assert.deepEqual(document.chapters.map((chapter) => chapter.title), ["第一章"]);
  assert.equal(document.images[0].src, "./cover.jpeg");
});
