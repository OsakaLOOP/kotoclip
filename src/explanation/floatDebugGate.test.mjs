import assert from "node:assert/strict";
import test from "node:test";
import { shouldEnableFloatDebug } from "./floatDebugGate.ts";

test("调试浮层需要 DEV、Tauri 与显式标识同时成立", () => {
  assert.equal(shouldEnableFloatDebug({ dev: true, tauriRuntime: true, search: "?ui-float-debug=true" }), true);
  assert.equal(shouldEnableFloatDebug({ dev: false, tauriRuntime: true, search: "?ui-float-debug=true" }), false);
  assert.equal(shouldEnableFloatDebug({ dev: true, tauriRuntime: false, search: "?ui-float-debug=true" }), false);
  assert.equal(shouldEnableFloatDebug({ dev: true, tauriRuntime: true, search: "?ui-float-debug=false" }), false);
  assert.equal(shouldEnableFloatDebug({ dev: true, tauriRuntime: true, search: "" }), false);
});
