import assert from "node:assert/strict";
import test from "node:test";
import { runSessionBoundOperation } from "./documentOperation.ts";

function deferred() {
  let resolve;
  const promise = new Promise((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

test("排队期间切换会话时不会发送旧操作", async () => {
  let activeSessionId = "document-new";
  let invoked = false;

  const result = await runSessionBoundOperation(
    "document-old",
    () => activeSessionId,
    async () => {
      invoked = true;
      return "patch";
    },
    (value) => value,
  );

  assert.equal(result, null);
  assert.equal(invoked, false);
});

test("请求返回前退出文档时不会应用旧 Patch", async () => {
  let activeSessionId = "document-old";
  let applied = false;
  const response = deferred();
  const operation = runSessionBoundOperation(
    "document-old",
    () => activeSessionId,
    () => response.promise,
    () => {
      applied = true;
      return "applied";
    },
  );

  activeSessionId = null;
  response.resolve("patch");

  assert.equal(await operation, null);
  assert.equal(applied, false);
});

test("会话保持一致时正常应用响应", async () => {
  const result = await runSessionBoundOperation(
    "document-current",
    () => "document-current",
    async () => "patch",
    (value) => `${value}-applied`,
  );

  assert.equal(result, "patch-applied");
});
