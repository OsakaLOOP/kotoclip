export interface FloatDebugGateInput {
  dev: boolean;
  tauriRuntime: boolean;
  search: string;
}

/** 调试浮层只允许由 Tauri dev URL 的显式标识开启。 */
export function shouldEnableFloatDebug(input: FloatDebugGateInput) {
  if (!input.dev || !input.tauriRuntime) return false;
  return new URLSearchParams(input.search).get("ui-float-debug") === "true";
}
