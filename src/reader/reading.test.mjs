import assert from "node:assert/strict";
import test from "node:test";
import { normalizeAppearance, readingEstimate } from "./reading.ts";

test("阅读排版设置始终限制在可用范围", () => {
  assert.deepEqual(normalizeAppearance({ fontSize: 100, lineHeight: 0, paragraphGap: 18 }), {
    fontSize: 28,
    lineHeight: 1.5,
    paragraphGap: 18,
    contentWidth: 760,
  });
});

test("阅读预计完成按剩余正文字符计算", () => {
  const estimate = readingEstimate(400, 1200, new Date("2026-07-20T10:00:00"));
  assert.equal(estimate.percent, 1 / 3);
  assert.equal(estimate.remainingMinutes, 2);
  assert.match(estimate.completionLabel, /10:02|10：02/);
});
