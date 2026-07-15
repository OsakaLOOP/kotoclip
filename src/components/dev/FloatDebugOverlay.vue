<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import {
  floatDebug,
  type FloatDebugCategory,
  type FloatDebugDock,
  type FloatDebugEvent,
  type FloatDebugValue,
} from "../../explanation/floatDebug";

const categories: Array<{ key: FloatDebugCategory; label: string }> = [
  { key: "hit", label: "命中" },
  { key: "decision", label: "判断" },
  { key: "session", label: "会话" },
  { key: "timer", label: "定时器" },
  { key: "request", label: "请求" },
  { key: "gate", label: "门控" },
  { key: "layout", label: "布局" },
];
const snapshotOrder = ["interaction", "timer", "gate", "session", "request.component", "request.whole", "anchor", "layout"];
const now = ref(performance.now());
let clock: number | null = null;

const dockClass = computed(() => `dock-${floatDebug.state.settings.dock}`);
const overlayStyle = computed(() => ({ opacity: String(floatDebug.state.settings.opacity) }));
const visibleEvents = computed(() => floatDebug.state.events
  .filter((event) => floatDebug.state.settings.categories[event.category])
  .slice()
  .reverse());
const timerRemaining = computed(() => {
  const timer = snapshotRecord("timer");
  const deadline = typeof timer?.deadline === "number" ? timer.deadline : null;
  return deadline === null ? null : Math.max(0, Math.ceil(deadline - now.value));
});

function snapshotRecord(name: string) {
  const value = floatDebug.state.snapshots[name];
  return value && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, FloatDebugValue>
    : null;
}

function snapshotText(name: string) {
  const value = floatDebug.state.snapshots[name];
  return value === undefined ? "尚无数据" : JSON.stringify(value, null, 2);
}

function eventTime(event: FloatDebugEvent) {
  return `${event.timestamp.toFixed(1)}ms`;
}

function eventDetail(event: FloatDebugEvent) {
  return event.detail ? JSON.stringify(event.detail) : "";
}

function setHistoryLimit(event: Event) {
  floatDebug.setHistoryLimit(Number((event.target as HTMLInputElement).value));
}

function setOpacity(event: Event) {
  floatDebug.setOpacity(Number((event.target as HTMLInputElement).value));
}

function setDock(event: Event) {
  floatDebug.setDock((event.target as HTMLSelectElement).value as FloatDebugDock);
}

function setCategory(category: FloatDebugCategory, event: Event) {
  floatDebug.setCategory(category, (event.target as HTMLInputElement).checked);
}

onMounted(() => {
  clock = window.setInterval(() => {
    now.value = performance.now();
  }, 40);
});

onBeforeUnmount(() => {
  if (clock !== null) window.clearInterval(clock);
});
</script>

<template>
  <aside
    class="float-debug-overlay"
    :class="[dockClass, { collapsed: floatDebug.state.settings.collapsed }]"
    :style="overlayStyle"
    aria-label="悬浮交互调试器"
  >
    <header class="debug-header">
      <div>
        <strong>UI Float Debug</strong>
        <span :class="{ armed: timerRemaining !== null }">
          {{ timerRemaining === null ? 'timer idle' : `close ${timerRemaining}ms` }}
        </span>
      </div>
      <div class="header-actions">
        <button type="button" @click="floatDebug.setPaused(!floatDebug.state.settings.paused)">
          {{ floatDebug.state.settings.paused ? '继续' : '暂停' }}
        </button>
        <button type="button" @click="floatDebug.clear">清空</button>
        <button type="button" @click="floatDebug.setCollapsed(!floatDebug.state.settings.collapsed)">
          {{ floatDebug.state.settings.collapsed ? '展开' : '收起' }}
        </button>
      </div>
    </header>

    <template v-if="!floatDebug.state.settings.collapsed">
      <section class="debug-config" aria-label="调试配置">
        <label>
          停靠
          <select :value="floatDebug.state.settings.dock" @change="setDock">
            <option value="top-left">左上</option>
            <option value="top-right">右上</option>
            <option value="bottom-left">左下</option>
            <option value="bottom-right">右下</option>
          </select>
        </label>
        <label>
          历史
          <input type="number" min="50" max="1000" step="10" :value="floatDebug.state.settings.historyLimit" @change="setHistoryLimit">
        </label>
        <label class="opacity-control">
          透明度
          <input type="range" min="0.55" max="1" step="0.05" :value="floatDebug.state.settings.opacity" @input="setOpacity">
        </label>
        <div class="category-switches">
          <label v-for="category in categories" :key="category.key">
            <input
              type="checkbox"
              :checked="floatDebug.state.settings.categories[category.key]"
              @change="setCategory(category.key, $event)"
            >
            {{ category.label }}
          </label>
        </div>
      </section>

      <section class="snapshot-grid" aria-label="实时状态">
        <article v-for="name in snapshotOrder" :key="name" class="snapshot-card">
          <h3>{{ name }}</h3>
          <pre>{{ snapshotText(name) }}</pre>
        </article>
      </section>

      <section class="history-section" aria-label="探针历史">
        <div class="history-heading">
          <strong>历史</strong>
          <span>{{ visibleEvents.length }} / {{ floatDebug.state.settings.historyLimit }}</span>
        </div>
        <ol class="event-list">
          <li v-for="event in visibleEvents" :key="event.sequence" :data-category="event.category">
            <div class="event-main">
              <span class="sequence">#{{ event.sequence }}</span>
              <time>{{ eventTime(event) }}</time>
              <span class="category">{{ event.category }}</span>
              <strong>{{ event.source }} · {{ event.action }}</strong>
              <span v-if="event.outcome" class="outcome">{{ event.outcome }}</span>
            </div>
            <code v-if="event.detail">{{ eventDetail(event) }}</code>
          </li>
        </ol>
      </section>
    </template>
  </aside>
</template>

<style scoped>
.float-debug-overlay {
  position: fixed;
  z-index: 5000;
  display: flex;
  flex-direction: column;
  width: min(680px, calc(100vw - 24px));
  max-height: min(82vh, 920px);
  border: 1px solid #3f596d;
  border-radius: 10px;
  background: #101820;
  box-shadow: 0 12px 40px rgb(0 0 0 / 38%);
  color: #d7e5ef;
  font: 12px/1.35 Consolas, "Yu Gothic UI", monospace;
  overflow: hidden;
}
.dock-top-left { top: 12px; left: 12px; }
.dock-top-right { top: 12px; right: 12px; }
.dock-bottom-left { bottom: 12px; left: 12px; }
.dock-bottom-right { right: 12px; bottom: 12px; }
.collapsed { width: min(430px, calc(100vw - 24px)); }
.debug-header,
.history-heading,
.event-main {
  display: flex;
  align-items: center;
}
.debug-header {
  flex: 0 0 auto;
  justify-content: space-between;
  gap: 12px;
  padding: 8px 10px;
  border-bottom: 1px solid #304653;
  background: #172630;
}
.debug-header > div:first-child { display: flex; align-items: baseline; gap: 10px; min-width: 0; }
.debug-header strong { color: #f1f7fa; font-size: 13px; }
.debug-header span { color: #8299a8; }
.debug-header span.armed { color: #ffca6b; }
.header-actions { display: flex; flex: 0 0 auto; gap: 6px; }
button,
select,
input {
  box-sizing: border-box;
  max-width: 100%;
  border: 1px solid #496170;
  border-radius: 5px;
  background: #20313c;
  color: #e2edf3;
  font: inherit;
}
button { padding: 3px 8px; cursor: pointer; }
.debug-config {
  flex: 0 0 auto;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px 12px;
  padding: 8px 10px;
  border-bottom: 1px solid #283d49;
}
.debug-config > label { display: flex; align-items: center; gap: 5px; white-space: nowrap; }
.debug-config input[type="number"] { width: 68px; padding: 2px 4px; }
.opacity-control input { width: 88px; }
.category-switches { display: flex; flex-wrap: wrap; gap: 5px 9px; min-width: 240px; }
.category-switches label { display: flex; align-items: center; gap: 3px; white-space: nowrap; }
.snapshot-grid {
  flex: 0 0 auto;
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  max-height: 300px;
  overflow: auto;
  border-bottom: 1px solid #283d49;
}
.snapshot-card { min-width: 0; padding: 7px 8px; border-right: 1px solid #223641; border-bottom: 1px solid #223641; }
.snapshot-card h3 { margin: 0 0 4px; color: #75c7f2; font-size: 11px; }
pre {
  margin: 0;
  color: #b9cad4;
  font: 10px/1.3 Consolas, monospace;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
.history-section { min-height: 100px; overflow: hidden; display: flex; flex-direction: column; }
.history-heading { flex: 0 0 auto; justify-content: space-between; padding: 6px 10px; background: #14232c; }
.history-heading span { color: #8299a8; }
.event-list { flex: 1 1 auto; margin: 0; padding: 0; overflow: auto; list-style: none; }
.event-list li { padding: 5px 8px; border-top: 1px solid #1f323d; }
.event-main { gap: 7px; min-width: 0; }
.event-main time,
.sequence { flex: 0 0 auto; color: #6e8796; }
.category { flex: 0 0 auto; min-width: 50px; color: #70c4ee; }
.event-main strong { min-width: 0; color: #d9e6ec; overflow-wrap: anywhere; }
.outcome { margin-left: auto; color: #ffcf7d; text-align: right; overflow-wrap: anywhere; }
.event-list code { display: block; margin-top: 3px; color: #8fa6b3; font: 10px/1.25 Consolas, monospace; overflow-wrap: anywhere; }
.event-list li[data-category="timer"] .category { color: #ffca6b; }
.event-list li[data-category="gate"] .category { color: #9adf93; }
.event-list li[data-category="request"] .category { color: #c7a7ff; }
@media (max-width: 760px) {
  .snapshot-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .outcome { margin-left: 0; }
  .event-main { flex-wrap: wrap; }
}
</style>
