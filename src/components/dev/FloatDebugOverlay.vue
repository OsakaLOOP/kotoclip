<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import {
  floatDebug,
  type FloatDebugCategory,
  type FloatDebugDock,
  type FloatDebugEvent,
  type FloatDebugValue,
} from "../../explanation/floatDebug";

type DebugRecord = Record<string, FloatDebugValue>;

const categories: Array<{ key: FloatDebugCategory; label: string }> = [
  { key: "hit", label: "命中" },
  { key: "decision", label: "判断" },
  { key: "session", label: "会话" },
  { key: "timer", label: "定时器" },
  { key: "request", label: "请求" },
  { key: "gate", label: "门控" },
  { key: "layout", label: "布局" },
];
const rawSnapshots = ["interactionScene", "sessionScene", "textBoxes", "panelBoxes", "gate", "session", "request.component", "request.whole"];
const now = ref(performance.now());
const copied = ref(false);
let clock: number | null = null;
let copiedTimer: number | null = null;

const dockClass = computed(() => `dock-${floatDebug.state.settings.dock}`);
const overlayStyle = computed(() => ({ opacity: String(floatDebug.state.settings.opacity) }));
const interactionScene = computed(() => snapshotRecord("interactionScene"));
const sessionScene = computed(() => snapshotRecord("sessionScene"));
const textBoxes = computed(() => snapshotRecord("textBoxes"));
const panelBoxes = computed(() => snapshotRecord("panelBoxes"));
const gate = computed(() => snapshotRecord("gate"));
const timer = computed(() => snapshotRecord("timer"));
const timerSequence = computed(() => floatDebug.state.events.filter((event) => event.category === "timer").slice(-10));
const visibleEvents = computed(() => floatDebug.state.events
  .filter((event) => floatDebug.state.settings.categories[event.category])
  .slice(-80)
  .reverse());
const timerRemaining = computed(() => {
  const deadline = numberValue(timer.value?.deadline);
  return deadline === null ? null : Math.max(0, Math.ceil(deadline - now.value));
});
const token = computed(() => recordValue(interactionScene.value?.token) ?? recordValue(sessionScene.value?.token));
const morpheme = computed(() => recordValue(interactionScene.value?.morpheme) ?? recordValue(sessionScene.value?.morpheme));
const source = computed(() => recordValue(sessionScene.value?.source));
const phase = computed(() => stringValue(interactionScene.value?.phase) ?? stringValue(sessionScene.value?.phase) ?? "等待事件");
const pointer = computed(() => recordValue(interactionScene.value?.pointer));
const hitRoute = computed(() => {
  const previous = recordValue(interactionScene.value?.previous) ?? recordValue(interactionScene.value?.current);
  const next = recordValue(interactionScene.value?.hit) ?? recordValue(interactionScene.value?.next);
  const left = stringValue(previous?.key) ?? "outside";
  const right = stringValue(next?.key) ?? "outside";
  return `${left} → ${right}`;
});

function snapshotRecord(name: string) {
  return recordValue(floatDebug.state.snapshots[name]);
}

function recordValue(value: FloatDebugValue | undefined): DebugRecord | null {
  return value !== null && typeof value === "object" && !Array.isArray(value) ? value as DebugRecord : null;
}

function stringValue(value: FloatDebugValue | undefined) {
  return typeof value === "string" ? value : typeof value === "number" ? String(value) : null;
}

function numberValue(value: FloatDebugValue | undefined) {
  return typeof value === "number" ? value : null;
}

function field(record: DebugRecord | null, key: string, fallback = "—") {
  return stringValue(record?.[key]) ?? fallback;
}

function rectText(value: FloatDebugValue | undefined) {
  const rect = recordValue(value);
  if (!rect) return "—";
  return `L${field(rect, "left")} T${field(rect, "top")}  R${field(rect, "right")} B${field(rect, "bottom")}  ${field(rect, "width")}×${field(rect, "height")}`;
}

function nestedRect(record: DebugRecord | null, key: string) {
  const value = record?.[key];
  const element = recordValue(value);
  return rectText(element?.rect ?? value);
}

function gateEnabled(key: string) {
  return gate.value?.[key] === true;
}

function eventTime(event: FloatDebugEvent) {
  return `${event.timestamp.toFixed(1)}ms`;
}

function eventDetail(event: FloatDebugEvent) {
  return event.detail ? JSON.stringify(event.detail) : "";
}

function snapshotText(name: string) {
  const value = floatDebug.state.snapshots[name];
  return value === undefined ? "尚无数据" : JSON.stringify(value, null, 2);
}

function timerLine(event: FloatDebugEvent) {
  const id = event.detail?.timerId ?? "—";
  const reason = event.detail?.reason ?? event.outcome ?? "—";
  return `#${event.sequence} ${event.action}  id=${String(id)}  ${String(reason)}`;
}

async function copyReport() {
  await navigator.clipboard.writeText(floatDebug.report());
  copied.value = true;
  if (copiedTimer !== null) window.clearTimeout(copiedTimer);
  copiedTimer = window.setTimeout(() => {
    copied.value = false;
    copiedTimer = null;
  }, 1200);
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
  if (copiedTimer !== null) window.clearTimeout(copiedTimer);
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
      <div class="title-block">
        <strong>UI Float Debug</strong>
        <span :class="{ armed: timerRemaining !== null }">
          {{ timerRemaining === null ? '关闭计时：空闲' : `关闭倒计时：${timerRemaining}ms` }}
        </span>
      </div>
      <div class="header-actions">
        <button type="button" @click="copyReport">{{ copied ? '已复制' : '复制报告' }}</button>
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
      <section class="live-scene" aria-label="当前交互现场">
        <div class="scene-heading">
          <strong>当前现场</strong>
          <span>{{ phase }}</span>
        </div>
        <div class="scene-grid">
          <div><label>文节</label><b>P{{ field(source, 'paragraphId', field(token, 'paragraphId')) }} / T{{ field(source, 'tokenIndex', field(token, 'tokenIndex')) }}</b><strong>{{ field(token, 'surface') }}</strong></div>
          <div><label>语素</label><b>M{{ field(source, 'morphemeIndex', field(morpheme, 'morphemeIndex')) }}</b><strong>{{ field(morpheme, 'surface') }}</strong><span>{{ field(morpheme, 'baseForm') }} / {{ field(morpheme, 'reading') }}</span></div>
          <div><label>命中链</label><code>{{ hitRoute }}</code></div>
          <div><label>Pointer</label><code>x={{ field(pointer, 'x') }} y={{ field(pointer, 'y') }}</code></div>
        </div>
      </section>

      <section class="geometry-grid" aria-label="空间范围">
        <article>
          <h3>正文范围</h3>
          <p><label>文节 box</label><code>{{ rectText(textBoxes?.textCapsule) }}</code></p>
          <p><label>语素 box</label><code>{{ rectText(textBoxes?.textMorpheme) }}</code></p>
          <p><label>事件文节</label><code>{{ nestedRect(interactionScene, 'capsule') }}</code></p>
          <p><label>事件语素</label><code>{{ nestedRect(interactionScene, 'morphemeElement') }}</code></p>
        </article>
        <article>
          <h3>浮层范围</h3>
          <p><label>整体 panel</label><code>{{ nestedRect(panelBoxes, 'whole') }}</code></p>
          <p><label>内部 panel</label><code>{{ nestedRect(panelBoxes, 'component') }}</code></p>
          <p><label>语法 panel</label><code>{{ nestedRect(panelBoxes, 'grammar') }}</code></p>
          <p><label>当前 panel</label><code>{{ nestedRect(interactionScene, 'panel') }}</code></p>
        </article>
      </section>

      <section class="timer-section" aria-label="关闭定时器序列">
        <div class="section-heading">
          <strong>关闭定时器序列</strong>
          <span>{{ timerSequence.length ? `最近 ${timerSequence.length} 步` : '尚未启动' }}</span>
        </div>
        <ol>
          <li v-for="entry in timerSequence" :key="entry.sequence" :class="`timer-${entry.action}`">
            <code>{{ timerLine(entry) }}</code>
          </li>
        </ol>
      </section>

      <section class="gate-section" aria-label="最终门控">
        <strong>最终门控</strong>
        <span :class="{ on: gateEnabled('dictionary') }">词典</span>
        <span :class="{ on: gateEnabled('whole') }">整体</span>
        <span :class="{ on: gateEnabled('component') }">内部</span>
        <span :class="{ on: gateEnabled('grammar') }">语法</span>
        <code>{{ field(gate, 'mode', 'closed') }} · {{ JSON.stringify(gate?.blockers ?? []) }}</code>
      </section>

      <details class="debug-config">
        <summary>采样与显示配置</summary>
        <div class="config-content">
          <label>停靠
            <select :value="floatDebug.state.settings.dock" @change="setDock">
              <option value="top-left">左上</option><option value="top-right">右上</option>
              <option value="bottom-left">左下</option><option value="bottom-right">右下</option>
            </select>
          </label>
          <label>历史 <input type="number" min="50" max="1000" step="10" :value="floatDebug.state.settings.historyLimit" @change="setHistoryLimit"></label>
          <label>透明度 <input type="range" min="0.55" max="1" step="0.05" :value="floatDebug.state.settings.opacity" @input="setOpacity"></label>
          <label v-for="category in categories" :key="category.key">
            <input type="checkbox" :checked="floatDebug.state.settings.categories[category.key]" @change="setCategory(category.key, $event)">
            {{ category.label }}
          </label>
        </div>
      </details>

      <section class="history-section" aria-label="探针历史">
        <div class="section-heading">
          <strong>按序历史</strong>
          <span>{{ visibleEvents.length }} 条</span>
        </div>
        <ol class="event-list">
          <li v-for="event in visibleEvents" :key="event.sequence" :data-category="event.category">
            <div class="event-main">
              <span>#{{ event.sequence }}</span><time>{{ eventTime(event) }}</time>
              <b>{{ event.category }}</b><strong>{{ event.source }} · {{ event.action }}</strong>
              <em v-if="event.outcome">{{ event.outcome }}</em>
            </div>
            <details v-if="event.detail"><summary>字段</summary><code>{{ eventDetail(event) }}</code></details>
          </li>
        </ol>
      </section>

      <details class="raw-state">
        <summary>原始快照</summary>
        <article v-for="name in rawSnapshots" :key="name"><h3>{{ name }}</h3><pre>{{ snapshotText(name) }}</pre></article>
      </details>
    </template>
  </aside>
</template>

<style scoped>
.float-debug-overlay { position: fixed; z-index: 5000; display: flex; flex-direction: column; width: min(720px, calc(100vw - 24px)); max-height: min(88vh, 940px); overflow: hidden; border: 1px solid #425e70; border-radius: 10px; background: #101820; box-shadow: 0 12px 40px rgb(0 0 0 / 38%); color: #d9e8ef; font: 12px/1.4 Consolas, "Yu Gothic UI", monospace; }
.dock-top-left { top: 12px; left: 12px; }.dock-top-right { top: 12px; right: 12px; }.dock-bottom-left { bottom: 12px; left: 12px; }.dock-bottom-right { right: 12px; bottom: 12px; }.collapsed { width: min(500px, calc(100vw - 24px)); }
.debug-header,.title-block,.header-actions,.scene-heading,.section-heading,.gate-section,.event-main { display: flex; align-items: center; }
.debug-header { flex: 0 0 auto; justify-content: space-between; gap: 10px; padding: 8px 10px; border-bottom: 1px solid #304653; background: #172630; }.title-block { gap: 10px; }.title-block strong { color: #fff; font-size: 13px; }.title-block span { color: #8da3b0; }.title-block span.armed { color: #ffca6b; }.header-actions { gap: 5px; }
button,select,input { box-sizing: border-box; max-width: 100%; border: 1px solid #496170; border-radius: 5px; background: #20313c; color: #e2edf3; font: inherit; }button { padding: 3px 8px; cursor: pointer; }
.live-scene { padding: 9px 10px; border-bottom: 1px solid #2b414d; background: #14222b; }.scene-heading,.section-heading { justify-content: space-between; margin-bottom: 7px; }.scene-heading span,.section-heading span { color: #8299a8; }.scene-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 6px 14px; }.scene-grid > div { display: flex; align-items: baseline; gap: 7px; min-width: 0; }.scene-grid label,.geometry-grid label { flex: 0 0 auto; color: #75c7f2; }.scene-grid b { color: #ffcf7d; }.scene-grid strong { color: #fff; font-size: 13px; }.scene-grid span { color: #9db0bb; }.scene-grid code { overflow-wrap: anywhere; }
.geometry-grid { display: grid; grid-template-columns: 1fr 1fr; border-bottom: 1px solid #2b414d; }.geometry-grid article { min-width: 0; padding: 8px 10px; }.geometry-grid article + article { border-left: 1px solid #2b414d; }.geometry-grid h3 { margin: 0 0 6px; color: #9adf93; font-size: 11px; }.geometry-grid p { display: flex; gap: 7px; margin: 3px 0; }.geometry-grid code { min-width: 0; color: #c1d0d8; overflow-wrap: anywhere; }
.timer-section { padding: 8px 10px; border-bottom: 1px solid #2b414d; }.timer-section ol { display: grid; grid-template-columns: 1fr 1fr; gap: 3px 12px; margin: 0; padding: 0; list-style: none; }.timer-section li { min-width: 0; color: #b9cad4; }.timer-section code { overflow-wrap: anywhere; }.timer-scheduled code { color: #ffca6b; }.timer-expired code { color: #ff8a80; }.timer-cancel code { color: #8bd4ff; }
.gate-section { flex-wrap: wrap; gap: 6px; padding: 7px 10px; border-bottom: 1px solid #2b414d; }.gate-section > span { padding: 1px 6px; border: 1px solid #405560; border-radius: 999px; color: #718895; }.gate-section > span.on { border-color: #6bbd66; background: #213d2b; color: #a9e8a3; }.gate-section code { margin-left: auto; color: #9dafb9; overflow-wrap: anywhere; }
.debug-config,.raw-state { flex: 0 0 auto; border-bottom: 1px solid #2b414d; }.debug-config > summary,.raw-state > summary { padding: 6px 10px; color: #8fa6b3; cursor: pointer; }.config-content { display: flex; flex-wrap: wrap; align-items: center; gap: 7px 12px; padding: 0 10px 8px; }.config-content label { display: flex; align-items: center; gap: 4px; white-space: nowrap; }.config-content input[type="number"] { width: 66px; }
.history-section { min-height: 120px; overflow: hidden; display: flex; flex: 1 1 auto; flex-direction: column; }.section-heading { flex: 0 0 auto; padding: 6px 10px; margin: 0; background: #14232c; }.event-list { flex: 1 1 auto; margin: 0; padding: 0; overflow: auto; list-style: none; }.event-list li { padding: 5px 8px; border-top: 1px solid #1f323d; }.event-main { gap: 7px; min-width: 0; }.event-main > span,.event-main time { flex: 0 0 auto; color: #6e8796; }.event-main b { flex: 0 0 auto; min-width: 52px; color: #70c4ee; }.event-main strong { min-width: 0; color: #d9e6ec; overflow-wrap: anywhere; }.event-main em { margin-left: auto; color: #ffcf7d; font-style: normal; text-align: right; }.event-list details summary { color: #718895; cursor: pointer; }.event-list details code { display: block; margin-top: 3px; color: #8fa6b3; font-size: 10px; overflow-wrap: anywhere; }
.raw-state { max-height: 260px; overflow: auto; }.raw-state article { padding: 5px 10px; border-top: 1px solid #223641; }.raw-state h3 { margin: 0 0 3px; color: #75c7f2; font-size: 10px; }pre { margin: 0; color: #b9cad4; font: 10px/1.3 Consolas, monospace; white-space: pre-wrap; overflow-wrap: anywhere; }
@media (max-width: 760px) { .scene-grid,.geometry-grid,.timer-section ol { grid-template-columns: 1fr; }.geometry-grid article + article { border-top: 1px solid #2b414d; border-left: 0; }.event-main { flex-wrap: wrap; }.gate-section code { width: 100%; margin-left: 0; } }
</style>
