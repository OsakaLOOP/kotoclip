import assert from "node:assert/strict";
import test from "node:test";
import { belongsToSameToken, isExplanationBridgePoint, keepsExplanationOpen } from "./hitTest.ts";

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

test("正文与上方面板之间的实际缝隙保留关闭宽限", () => {
  const source = { left: 480, top: 400, right: 540, bottom: 430 };
  const panel = { left: 300, top: 90, right: 720, bottom: 390 };
  assert.equal(isExplanationBridgePoint({ x: 510, y: 399 }, source, [panel]), true);
});

test("沿面板上沿向外移动不再被误判为物理缝隙", () => {
  const panel = { left: 300, top: 90, right: 720, bottom: 390 };
  const source = { left: 480, top: 400, right: 540, bottom: 430 };
  assert.equal(isExplanationBridgePoint({ x: 510, y: 88 }, panel, [source]), false);
});

test("双面板之间的窄缝仍可连续穿越", () => {
  const whole = { left: 100, top: 80, right: 520, bottom: 440 };
  const component = { left: 530, top: 80, right: 950, bottom: 440 };
  assert.equal(isExplanationBridgePoint({ x: 521, y: 220 }, whole, [component]), true);
});
