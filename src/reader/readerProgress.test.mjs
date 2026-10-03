import assert from "node:assert/strict";
import test from "node:test";
import { readerAnalysisProgress } from "./readerProgress.ts";

test("准备分析占三份，后续按完整单元计算", () => {
  const session = { session_id: "s1", paused: false, progress: { total: 5, basic: 1, complete: 3, failed: 0, pending: 2, analysis: { complete: 3, total: 5 }, cache: { complete: 0, total: 0 } } };
  const progress = readerAnalysisProgress(session, false);
  assert.equal(progress.phase, "processing");
  assert.equal(progress.completed, 3);
  assert.equal(progress.total, 5);
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

test("进入阅读页后保持全文批次的计数", () => {
  const session = { session_id: "s1", paused: false, progress: { total: 70, basic: 4, complete: 4, failed: 0, pending: 66, analysis: { complete: 4, total: 70 }, cache: { complete: 0, total: 0 } } };
  const progress = readerAnalysisProgress(session, false);
  assert.deepEqual(progress.work.analysis, { complete: 4, total: 70 });
  assert.equal(progress.message, "剩余 66 个单元");
  assert.equal(progress.percent, 9.5);
});

test("全文完成后新缓存批次仍显示活动进度", () => {
  const session = { session_id: "s1", paused: false, progress: { total: 70, basic: 24, complete: 70, failed: 0, pending: 4, analysis: { complete: 0, total: 0 }, cache: { complete: 0, total: 4 } } };
  const progress = readerAnalysisProgress(session, false);
  assert.equal(progress.phase, "processing");
  assert.equal(progress.message, "剩余 4 个单元");
  assert.equal(progress.percent, 75);
  session.progress.cache.complete = 4;
  session.progress.pending = 0;
  const complete = readerAnalysisProgress(session, false);
  assert.equal(complete.phase, "completed");
  assert.equal(complete.percent, 100);
});

test("混合批次按固定分析项和四分之一权重的缓存项计算", () => {
  const session = { session_id: "s1", paused: false, progress: { total: 70, complete: 68, failed: 0, pending: 4, analysis: { complete: 2, total: 4 }, cache: { complete: 0, total: 2 } } };
  const progress = readerAnalysisProgress(session, false);
  assert.equal(progress.total, 6);
  assert.equal(progress.completed, 2);
  assert.equal(progress.message, "剩余 4 个单元");
  assert.equal(progress.percent, 66.6);
  session.paused = true;
  assert.equal(readerAnalysisProgress(session, false).message, "分析已暂停");
});
