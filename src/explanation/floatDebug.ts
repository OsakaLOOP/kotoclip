import { reactive } from "vue";
import { shouldEnableFloatDebug } from "./floatDebugGate";

export type FloatDebugCategory = "hit" | "decision" | "session" | "timer" | "request" | "gate" | "layout";
export type FloatDebugDock = "top-left" | "top-right" | "bottom-left" | "bottom-right";
export type FloatDebugValue = string | number | boolean | null | FloatDebugValue[] | { [key: string]: FloatDebugValue };

export interface FloatDebugEvent {
  sequence: number;
  timestamp: number;
  category: FloatDebugCategory;
  source: string;
  action: string;
  outcome?: string;
  detail?: Record<string, FloatDebugValue>;
}

interface FloatDebugSettings {
  paused: boolean;
  collapsed: boolean;
  historyLimit: number;
  dock: FloatDebugDock;
  opacity: number;
  categories: Record<FloatDebugCategory, boolean>;
}

const categoryDefaults: Record<FloatDebugCategory, boolean> = {
  hit: true,
  decision: true,
  session: true,
  timer: true,
  request: true,
  gate: true,
  layout: false,
};

const enabled = typeof window !== "undefined" && shouldEnableFloatDebug({
  dev: import.meta.env.DEV,
  tauriRuntime: "__TAURI_INTERNALS__" in window,
  search: window.location.search,
});

const storageKey = "kotoclip.ui-float-debug.settings.v1";
const initialSettings = loadSettings();
const state = reactive({
  enabled,
  settings: initialSettings,
  events: [] as FloatDebugEvent[],
  snapshots: {} as Record<string, FloatDebugValue>,
});
let sequence = 0;

function loadSettings(): FloatDebugSettings {
  const fallback: FloatDebugSettings = {
    paused: false,
    collapsed: false,
    historyLimit: 240,
    dock: "bottom-right",
    opacity: 0.96,
    categories: { ...categoryDefaults },
  };
  if (!enabled) return fallback;
  try {
    const stored = JSON.parse(window.localStorage.getItem(storageKey) ?? "null") as Partial<FloatDebugSettings> | null;
    if (!stored) return fallback;
    return {
      ...fallback,
      ...stored,
      paused: false,
      historyLimit: clampHistoryLimit(stored.historyLimit ?? fallback.historyLimit),
      opacity: clampOpacity(stored.opacity ?? fallback.opacity),
      categories: { ...categoryDefaults, ...stored.categories },
    };
  } catch {
    return fallback;
  }
}

function persistSettings() {
  if (!enabled) return;
  try {
    window.localStorage.setItem(storageKey, JSON.stringify(state.settings));
  } catch {
    // 调试配置持久化失败不应影响被观察的交互路径。
  }
}

function clampHistoryLimit(value: number) {
  if (!Number.isFinite(value)) return 240;
  return Math.min(1000, Math.max(50, Math.round(value)));
}

function clampOpacity(value: number) {
  if (!Number.isFinite(value)) return 0.96;
  return Math.min(1, Math.max(0.55, value));
}

function trimHistory() {
  const overflow = state.events.length - state.settings.historyLimit;
  if (overflow > 0) state.events.splice(0, overflow);
}

function record(
  category: FloatDebugCategory,
  source: string,
  action: string,
  outcome?: string,
  detail?: Record<string, FloatDebugValue>,
) {
  if (!enabled || state.settings.paused || !state.settings.categories[category]) return;
  state.events.push({
    sequence: ++sequence,
    timestamp: performance.now(),
    category,
    source,
    action,
    outcome,
    detail,
  });
  trimHistory();
}

function snapshot(name: string, value: FloatDebugValue) {
  if (!enabled || state.settings.paused) return;
  state.snapshots[name] = value;
}

function clear() {
  state.events.splice(0);
}

function report() {
  const lines = [
    "KOTOCLIP UI FLOAT DEBUG REPORT",
    `capturedAt=${new Date().toISOString()}`,
    `enabled=${enabled} paused=${state.settings.paused}`,
    "",
    "[现场]",
    snapshotText("interactionScene"),
    snapshotText("sessionScene"),
    "",
    "[正文矩形]",
    snapshotText("textBoxes"),
    "",
    "[浮层矩形]",
    snapshotText("panelBoxes"),
    "",
    "[定时器序列]",
    ...state.events
      .filter((event) => event.category === "timer")
      .map((event) => `#${event.sequence} +${event.timestamp.toFixed(1)}ms ${event.action}${event.outcome ? ` => ${event.outcome}` : ""}${event.detail ? ` detail=${JSON.stringify(event.detail)}` : ""}`),
    "",
    "[最终门控]",
    snapshotText("gate"),
    "",
    "[请求与会话]",
    snapshotText("session"),
    snapshotText("request.component"),
    snapshotText("request.whole"),
    "",
    "[按序历史]",
    ...state.events.map((event) => {
      const detail = event.detail ? ` detail=${JSON.stringify(event.detail)}` : "";
      return `#${event.sequence} +${event.timestamp.toFixed(1)}ms [${event.category}] ${event.source}.${event.action}${event.outcome ? ` => ${event.outcome}` : ""}${detail}`;
    }),
  ];
  return lines.join("\n");
}

function snapshotText(name: string) {
  const value = state.snapshots[name];
  return value === undefined ? "<none>" : JSON.stringify(value, null, 2);
}

function setPaused(paused: boolean) {
  state.settings.paused = paused;
}

function setCollapsed(collapsed: boolean) {
  state.settings.collapsed = collapsed;
  persistSettings();
}

function setHistoryLimit(limit: number) {
  state.settings.historyLimit = clampHistoryLimit(limit);
  trimHistory();
  persistSettings();
}

function setDock(dock: FloatDebugDock) {
  state.settings.dock = dock;
  persistSettings();
}

function setOpacity(opacity: number) {
  state.settings.opacity = clampOpacity(opacity);
  persistSettings();
}

function setCategory(category: FloatDebugCategory, visible: boolean) {
  state.settings.categories[category] = visible;
  persistSettings();
}

export const floatDebug = {
  enabled,
  state,
  record,
  snapshot,
  clear,
  report,
  setPaused,
  setCollapsed,
  setHistoryLimit,
  setDock,
  setOpacity,
  setCategory,
};
