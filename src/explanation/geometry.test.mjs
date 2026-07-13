import assert from "node:assert/strict";
import test from "node:test";
import { explanationPanelWidth, measureIntrinsicPanel, placeExplanationPanels } from "./geometry.ts";

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

test("布局读取内容固有高度而不是受约束的外框高度", () => {
  const constrained = measureIntrinsicPanel({
    getBoundingClientRect: () => rect(0, 0, 420, 300),
    scrollHeight: 980,
  });
  const differentlyConstrained = measureIntrinsicPanel({
    getBoundingClientRect: () => rect(0, 0, 420, 420),
    scrollHeight: 980,
  });
  assert.deepEqual(constrained, { width: 420, height: 980 });
  assert.deepEqual(differentlyConstrained, constrained);

  const anchor = rect(430, 386, 40, 28);
  const viewport = { width: 900, height: 800 };
  assert.deepEqual(
    placeExplanationPanels(anchor, anchor, constrained, viewport),
    placeExplanationPanels(anchor, anchor, differentlyConstrained, viewport),
  );
});

test("单面板保持在视口内且不遮挡语素", () => {
  const anchor = rect(380, 360, 42, 28);
  const placement = placeExplanationPanels(anchor, anchor, { width: 420, height: 360 }, { width: 900, height: 720 });
  assert.equal(overlaps(placement.component, anchor), false);
  assert.ok(placement.component.left >= 12);
  assert.ok(placement.component.top >= 12);
  assert.ok(placement.component.left + placement.component.width <= 888);
  assert.ok(placement.component.top + placement.component.height <= 708);
});

test("16:9 视口中双面板强制左右捆绑并限制高度", () => {
  const anchor = rect(920, 520, 80, 30);
  const placement = placeExplanationPanels(
    anchor,
    anchor,
    { width: 420, height: 1200 },
    { width: 1920, height: 1080 },
    { width: 420, height: 900 },
  );
  assert.ok(placement.whole);
  assert.equal(placement.whole.top, placement.component.top);
  assert.equal(placement.whole.left + placement.whole.width + 10, placement.component.left);
  assert.equal(placement.whole.maxHeight, placement.component.maxHeight);
  assert.ok(placement.component.maxHeight <= 480);
  assert.equal(overlaps(placement.whole, placement.component), false);
  assert.equal(overlaps(placement.whole, anchor), false);
  assert.equal(overlaps(placement.component, anchor), false);
});

test("靠近水平边界时只平移整个双面板组", () => {
  const viewport = { width: 1920, height: 1080 };
  for (const anchor of [rect(8, 700, 60, 28), rect(1850, 700, 60, 28)]) {
    const placement = placeExplanationPanels(
      anchor,
      anchor,
      { width: 420, height: 300 },
      viewport,
      { width: 420, height: 280 },
    );
    assert.ok(placement.whole);
    assert.equal(placement.whole.top, placement.component.top);
    assert.equal(placement.whole.left + placement.whole.width + 10, placement.component.left);
    assert.ok(placement.whole.left >= 12);
    assert.ok(placement.component.left + placement.component.width <= 1908);
  }
});

test("窄屏安全收窄时仍保持左右捆绑", () => {
  assert.equal(explanationPanelWidth(360, true), 163);
  const anchor = rect(150, 320, 60, 28);
  const placement = placeExplanationPanels(
    anchor,
    anchor,
    { width: 336, height: 240 },
    { width: 360, height: 720 },
    { width: 336, height: 220 },
  );
  assert.ok(placement.whole);
  assert.equal(placement.whole.top, placement.component.top);
  assert.equal(placement.whole.left + placement.whole.width + 10, placement.component.left);
  assert.ok(placement.component.left + placement.component.width <= 348);
  assert.equal(overlaps(placement.whole, placement.component), false);
  assert.equal(overlaps(placement.whole, anchor), false);
  assert.equal(overlaps(placement.component, anchor), false);
});
