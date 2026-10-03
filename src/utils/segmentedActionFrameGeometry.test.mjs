import assert from "node:assert/strict";
import test from "node:test";
import {
  frameMotionRatio,
  minimumFrameCenterGap,
  segmentedFrameLayout,
} from "../components/common/segmentedActionFrameGeometry.ts";

test("分段动作框定义横排、竖排与四象限布局", () => {
  const horizontal = segmentedFrameLayout("1x2");
  assert.deepEqual([horizontal.rows, horizontal.columns, horizontal.capacity], [1, 2, 2]);
  assert.equal(horizontal.dividers.length, 1);
  assert.equal(horizontal.dividers[0].orientation, "vertical");
  assert.deepEqual(horizontal.corners.map((corner) => corner.actionIndex), [0, 0, 1, 1]);

  const vertical = segmentedFrameLayout("2x1");
  assert.deepEqual([vertical.rows, vertical.columns, vertical.capacity], [2, 1, 2]);
  assert.equal(vertical.dividers[0].orientation, "horizontal");
  assert.deepEqual(vertical.corners.map((corner) => corner.actionIndex), [0, 0, 1, 1]);

  const matrix = segmentedFrameLayout("2x2");
  assert.deepEqual([matrix.rows, matrix.columns, matrix.capacity], [2, 2, 4]);
  assert.deepEqual(matrix.corners.map((corner) => corner.actionIndex), [0, 1, 2, 3]);
  assert.deepEqual(matrix.dividers.map((divider) => divider.orientation), [
    "horizontal",
    "horizontal",
    "vertical",
    "vertical",
  ]);
});

test("四象限分隔线在中心侧停止轴向扩张", () => {
  const matrix = segmentedFrameLayout("2x2");
  for (const divider of matrix.dividers) {
    assert.notEqual(divider.startOuter, divider.endOuter);
    const centerInset = divider.startOuter ? divider.end : divider.start;
    assert.match(centerInset, /frame-center-gap/);
  }
});

test("中心百分之二十保持最大位移并平滑进入衰减区", () => {
  assert.equal(frameMotionRatio(0, 0), 1);
  assert.equal(frameMotionRatio(0.2, 0), 1);

  const immediatelyOutside = frameMotionRatio(0.2001, 0);
  assert.ok(immediatelyOutside < 1);
  assert.ok(immediatelyOutside > 0.999999);

  const samples = [0.2, 0.3, 0.5, 0.7, 0.9, 1].map((distance) => frameMotionRatio(distance, 0));
  for (let index = 1; index < samples.length; index += 1) {
    assert.ok(samples[index] <= samples[index - 1]);
  }
  assert.equal(samples.at(-1), 0);
});

test("中心留空下限覆盖按压超程并保留一个线宽", () => {
  assert.equal(minimumFrameCenterGap(6, 3, 7), 25);
  assert.equal(minimumFrameCenterGap(-1, 3, 7), 13);
});
