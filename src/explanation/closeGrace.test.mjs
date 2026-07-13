import assert from "node:assert/strict";
import test from "node:test";
import { EXPLANATION_CLOSE_GRACE_MS, scheduleCloseGrace } from "./closeGrace.ts";

test("关闭宽限从首次离开开始，外部事件不会续期", () => {
  let nextTimerId = 1;
  const scheduled = [];
  const scheduleTimer = (callback, delay) => {
    const id = nextTimerId++;
    scheduled.push({ id, callback, delay });
    return id;
  };
  const close = () => {};

  let timer = scheduleCloseGrace(null, scheduleTimer, close);
  timer = scheduleCloseGrace(timer, scheduleTimer, close);
  timer = scheduleCloseGrace(timer, scheduleTimer, close);

  assert.equal(timer, 1);
  assert.equal(scheduled.length, 1);
  assert.equal(scheduled[0].delay, EXPLANATION_CLOSE_GRACE_MS);
  assert.equal(scheduled[0].callback, close);

  timer = scheduleCloseGrace(null, scheduleTimer, close);
  assert.equal(timer, 2);
  assert.equal(scheduled.length, 2);
});
