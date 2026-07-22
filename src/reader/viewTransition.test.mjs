import assert from "node:assert/strict";
import test from "node:test";
import { createViewTransitionGuard } from "./viewTransition.ts";

function deferred() {
  let resolve;
  const promise = new Promise((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

function fakeTransition() {
  const completion = deferred();
  let skipped = 0;
  return {
    transition: {
      updateCallbackDone: Promise.resolve(),
      finished: completion.promise,
      skipTransition() {
        skipped += 1;
        completion.resolve();
      },
    },
    completion,
    skipped: () => skipped,
  };
}

test("返回操作会先结束活动视图过渡，再继续卸载", async () => {
  const guard = createViewTransitionGuard();
  const current = fakeTransition();
  guard.track(current.transition);

  await guard.finish();
  await guard.finish();

  assert.equal(current.skipped(), 1);
});

test("旧过渡自然结束不会清除后来登记的活动过渡", async () => {
  const guard = createViewTransitionGuard();
  const previous = fakeTransition();
  const current = fakeTransition();
  guard.track(previous.transition);
  guard.track(current.transition);

  previous.completion.resolve();
  await previous.transition.finished;
  await Promise.resolve();
  await guard.finish();

  assert.equal(previous.skipped(), 0);
  assert.equal(current.skipped(), 1);
});

test("组件销毁会终止过渡且不保留活动引用", async () => {
  const guard = createViewTransitionGuard();
  const current = fakeTransition();
  guard.track(current.transition);

  guard.dispose();
  await guard.finish();

  assert.equal(current.skipped(), 1);
});
