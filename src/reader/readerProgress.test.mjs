import assert from "node:assert/strict";
import test from "node:test";
import { readerAnalysisProgress } from "./readerProgress.ts";

test("进度按完整单元计算，不受已释放产物数量影响", () => {
  const session = { session_id: "s1", paused: false, progress: { total: 5, basic: 1, complete: 3, failed: 0, pending: 2 } };
  const progress = readerAnalysisProgress(session, false);
  assert.equal(progress.phase, "processing");
  assert.equal(progress.completed, 3);
  assert.equal(progress.percent, 60);
});

test("失败单元结束后停止活动进度", () => {
  const session = { session_id: "s1", paused: false, progress: { total: 2, basic: 1, complete: 1, failed: 1, pending: 0 } };
  const progress = readerAnalysisProgress(session, false);
  assert.equal(progress.phase, "completed");
  assert.equal(progress.message, "部分单元失败");
});
