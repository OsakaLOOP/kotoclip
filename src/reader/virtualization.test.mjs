import assert from "node:assert/strict";
import test from "node:test";
import { estimateReaderRow, resolveReaderRowMeasurement } from "./virtualization.ts";

test("图片解码前不会用折叠高度覆盖虚拟行估算", () => {
  assert.equal(resolveReaderRowMeasurement({
    kind: "image",
    imageState: "loading",
    estimatedSize: 620,
    elementSize: 0,
  }), 620);
  assert.equal(resolveReaderRowMeasurement({
    kind: "image",
    imageState: "loading",
    cachedSize: 734,
    estimatedSize: 620,
    elementSize: 0,
  }), 734);
});

test("图片完成解码后提交可测盒模型高度", () => {
  assert.equal(resolveReaderRowMeasurement({
    kind: "image",
    imageState: "ready",
    cachedSize: 620,
    estimatedSize: 620,
    observedSize: 811.4,
    elementSize: 810,
  }), 812);
});

test("图片估算受视口约束", () => {
  assert.equal(estimateReaderRow({ kind: "image", heading: false, viewportHeight: 500, fontSize: 19, lineHeight: 2, contentWidth: 760 }), 420);
  assert.equal(estimateReaderRow({ kind: "image", heading: false, viewportHeight: 1400, fontSize: 19, lineHeight: 2, contentWidth: 760 }), 760);
});

test("已知图片宽高时按版心与视口上限精确估算", () => {
  assert.equal(estimateReaderRow({
    kind: "image",
    heading: false,
    viewportHeight: 1000,
    fontSize: 19,
    lineHeight: 2,
    contentWidth: 600,
    imageWidth: 1200,
    imageHeight: 1600,
  }), 780);
});
