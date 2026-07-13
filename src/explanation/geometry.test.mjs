import assert from "node:assert/strict";
import test from "node:test";
import { placeExplanationPanels } from "./geometry.ts";

const rect = (left, top, width, height) => ({
  left,
  top,
  right: left + width,
  bottom: top + height,
  width,
  height,
});

const overlaps = (left, right) => (
  Math.max(left.left, right.left) < Math.min(left.left + left.width, right.right)
  && Math.max(left.top, right.top) < Math.min(left.top + left.height, right.bottom)
);

test("单面板保持在视口内且不遮挡语素", () => {
  const anchor = rect(380, 360, 42, 28);
  const placement = placeExplanationPanels(anchor, anchor, { width: 420, height: 360 }, { width: 900, height: 720 });
  assert.equal(overlaps(placement.component, anchor), false);
  assert.ok(placement.component.left >= 12);
  assert.ok(placement.component.top >= 12);
  assert.ok(placement.component.left + placement.component.width <= 888);
  assert.ok(placement.component.top + placement.component.height <= 708);
});

test("宽屏双面板作为同组排列且互不覆盖", () => {
  const anchor = rect(430, 500, 80, 30);
  const placement = placeExplanationPanels(
    anchor,
    anchor,
    { width: 400, height: 300 },
    { width: 1100, height: 800 },
    { width: 400, height: 280 },
  );
  assert.ok(placement.whole);
  assert.equal(overlaps(placement.whole, placement.component), false);
  assert.equal(overlaps(placement.whole, anchor), false);
  assert.equal(overlaps(placement.component, anchor), false);
});

test("窄屏双面板分置锚点上下", () => {
  const anchor = rect(150, 320, 60, 28);
  const placement = placeExplanationPanels(
    anchor,
    anchor,
    { width: 336, height: 240 },
    { width: 360, height: 720 },
    { width: 336, height: 220 },
  );
  assert.ok(placement.whole);
  assert.equal(overlaps(placement.whole, placement.component), false);
  assert.equal(overlaps(placement.whole, anchor), false);
  assert.equal(overlaps(placement.component, anchor), false);
});
