import assert from "node:assert/strict";
import test from "node:test";
import { prepareMarkdownDocument } from "./markdownDocument.ts";

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
