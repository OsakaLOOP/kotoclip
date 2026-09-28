import assert from "node:assert/strict";
import test from "node:test";
import { readerAnalysisProgress } from "./readerProgress.ts";

test("准备分析占三份，后续按完整单元计算", () => {
  const session = { session_id: "s1", paused: false, progress: { total: 5, basic: 1, complete: 3, failed: 0, pending: 2, analysis: { complete: 3, total: 5 }, cache: { complete: 0, total: 0 } } };
  const progress = readerAnalysisProgress(session, false);
  assert.equal(progress.phase, "processing");
  assert.equal(progress.completed, 6);
  assert.equal(progress.total, 8);
  assert.equal(progress.percent, 75);
});

test("准备阶段从零开始，取得单元数后计入三份准备进度", () => {
  const preparing = readerAnalysisProgress(null, true);
  assert.equal(preparing.message, "准备分析");
  assert.equal(preparing.percent, 0);
  const session = { session_id: "s1", paused: false, progress: { total: 7, basic: 0, complete: 0, failed: 0, pending: 7, analysis: { complete: 0, total: 7 }, cache: { complete: 0, total: 0 } } };
  assert.equal(readerAnalysisProgress(session, false).percent, 30);
});

test("失败单元结束后停止活动进度", () => {
  const session = { session_id: "s1", paused: false, progress: { total: 2, basic: 1, complete: 1, failed: 1, pending: 0, analysis: { complete: 1, total: 2 }, cache: { complete: 0, total: 0 } } };
  const progress = readerAnalysisProgress(session, false);
  assert.equal(progress.phase, "completed");
  assert.equal(progress.message, "部分单元失败");
});
