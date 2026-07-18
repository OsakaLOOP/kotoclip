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
  requests: [] as FloatDebugEvent[],
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
  // timer 会在指针抖动时大量产生，不能把用于性能诊断的 request 全部挤出。
  const reservedRequests = Math.min(32, Math.floor(state.settings.historyLimit / 2));
  while (state.events.length > state.settings.historyLimit) {
    const requestCount = state.events.filter((event) => event.category === "request").length;
    const removable = state.events.findIndex((event) => (
      event.category !== "request" || requestCount > reservedRequests
    ));
    state.events.splice(removable >= 0 ? removable : 0, 1);
  }
}

function record(
  category: FloatDebugCategory,
  source: string,
  action: string,
  outcome?: string,
  detail?: Record<string, FloatDebugValue>,
) {
  if (!enabled || state.settings.paused || !state.settings.categories[category]) return;

  const currentSequence = ++sequence;
  const currentTimestamp = performance.now();

  // 1. 维护独立的 state.requests 队列 (专用于“词典请求诊断”面板，排除 cache-hit/skip 噪音，不受 timer 等高频事件挤占)
  if (category === "request" && (source === "component" || source === "whole") && action !== "skip" && outcome !== "cache-hit") {
    const gen = detail?.generation;
    if (typeof gen === "number" && action !== "resolve" && action !== "navigate" && action !== "select") {
      const existingReq = [...state.requests].reverse().find((e) =>
        e.source === source && e.detail?.generation === gen
      );
      if (existingReq) {
        existingReq.action = action;
        if (outcome !== undefined) existingReq.outcome = outcome;
        if (detail !== undefined) {
          existingReq.detail = { ...existingReq.detail, ...detail };
        }
      }
    } else if (action === "resolve" || action === "navigate" || action === "select") {
      state.requests.push({
        sequence: currentSequence,
        timestamp: currentTimestamp,
        category,
        source,
        action,
        outcome,
        detail: detail ? { ...detail } : undefined,
      });
      if (state.requests.length > 60) {
        state.requests.shift();
      }
    }
  }

  // 2. 维护通用的 state.events 队列 (用于“按序历史”展示)
  if (category === "request" && (source === "component" || source === "whole")) {
    const gen = detail?.generation;
    if (typeof gen === "number" && action !== "resolve" && action !== "navigate" && action !== "select") {
      const existing = [...state.events].reverse().find((e) =>
        e.category === "request" &&
        e.source === source &&
        e.detail?.generation === gen
      );
      if (existing) {
        existing.action = action;
        if (outcome !== undefined) existing.outcome = outcome;
        if (detail !== undefined) {
          existing.detail = { ...existing.detail, ...detail };
        }
        return; // 更新完已有事件，无需再 push 到 events 队列
      }
    }
  }

  state.events.push({
    sequence: currentSequence,
    timestamp: currentTimestamp,
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
  state.requests.splice(0);
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
    ...state.events
      .filter((event) => event.category === "request")
      .slice(-12)
      .map((event) => `#${event.sequence} +${event.timestamp.toFixed(1)}ms ${event.source}.${event.action}${event.outcome ? ` => ${event.outcome}` : ""}${event.detail ? ` detail=${JSON.stringify(event.detail)}` : ""}`),
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
