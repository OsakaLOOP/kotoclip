import assert from "node:assert/strict";
import test from "node:test";
import { belongsToSameToken, keepsExplanationOpen } from "./hitTest.ts";

const morpheme = (paragraphId, tokenIndex, morphemeIndex) => ({
  kind: "morpheme",
  paragraphId,
  tokenIndex,
  morphemeIndex,
  key: `morpheme:${paragraphId}:${tokenIndex}:${morphemeIndex}`,
});

test("同文节跨语素只改变目标，不触发关闭", () => {
  assert.equal(belongsToSameToken(morpheme(1, 2, 0), morpheme(1, 2, 1)), true);
  assert.equal(keepsExplanationOpen(morpheme(1, 2, 1)), true);
});

test("跨文节不是同一目标，但仍属于连续交互区域", () => {
  assert.equal(belongsToSameToken(morpheme(1, 2, 0), morpheme(1, 3, 0)), false);
  assert.equal(keepsExplanationOpen(morpheme(1, 3, 0)), true);
});

test("只有真正离开正文与面板才进入缝隙关闭", () => {
  assert.equal(keepsExplanationOpen({ kind: "panel", panel: "component", key: "panel:component" }), true);
  assert.equal(keepsExplanationOpen({ kind: "outside", key: "outside" }), false);
});
