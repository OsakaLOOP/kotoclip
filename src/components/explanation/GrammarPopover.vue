<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import type { GrammarDictionaryTarget, GrammarTag } from "../../types";
import { floatDebug } from "../../explanation/floatDebug";
import type { RectSnapshot } from "../../explanation/geometry";

const props = defineProps<{
  show: boolean;
  tag: GrammarTag | null;
  anchor: RectSnapshot | null;
}>();

const emit = defineEmits<{
  enter: [event: PointerEvent];
  leave: [event: PointerEvent];
  openDictionary: [target: GrammarDictionaryTarget];
}>();
const panelRef = ref<HTMLElement | null>(null);
const expanded = ref(false);
const selectedSenseId = ref<string | null>(null);
const explanation = computed(() => props.tag?.explanation ?? null);
const selectedSense = computed(() => {
  const tag = props.tag;
  if (!tag) return null;
  const id = selectedSenseId.value ?? tag.selected_sense_id;
  return tag.sense_candidates.find((candidate) => candidate.sense_id === id) ?? null;
});

const style = computed(() => {
  const anchor = props.anchor;
  if (!anchor) return { left: "-10000px", top: "-10000px" };
  const width = Math.min(360, window.innerWidth - 24);
  const left = Math.min(window.innerWidth - 12 - width, Math.max(12, anchor.left + anchor.width / 2 - width / 2));
  const above = anchor.top > window.innerHeight / 2;
  return {
    left: `${left}px`,
    top: above ? `${anchor.top - 8}px` : `${anchor.bottom + 8}px`,
    width: `${width}px`,
    transform: above ? "translateY(-100%)" : undefined,
  };
});

watch(
  () => [props.show, props.tag, props.anchor],
  async () => {
    if (!props.show) return;
    await nextTick();
    const panel = panelRef.value;
    if (!panel) return;
    const rect = panel.getBoundingClientRect();
    floatDebug.snapshot("panelBoxes", {
      whole: null,
      component: null,
      grammar: {
        left: Math.round(rect.left),
        top: Math.round(rect.top),
        right: Math.round(rect.right),
        bottom: Math.round(rect.bottom),
        width: Math.round(rect.width),
        height: Math.round(rect.height),
      },
    });
  },
  { flush: "post" },
);

watch(
  () => props.tag?.occurrence_id,
  () => {
    expanded.value = false;
    selectedSenseId.value = props.tag?.selected_sense_id ?? null;
  },
  { immediate: true },
);
</script>

<template>
  <aside
    v-if="show && tag"
    ref="panelRef"
    class="grammar-popover"
    data-explanation-panel="grammar"
    :style="style"
    role="dialog"
    aria-label="语法说明"
    @pointerenter="emit('enter', $event)"
    @pointerleave="emit('leave', $event)"
  >
    <header>
      <div>
        <strong>{{ explanation?.title || tag.name_ja }}</strong>
        <small v-if="explanation?.actual_form">本句：{{ explanation.actual_form }}</small>
      </div>
      <span v-if="tag.jlpt_level">JLPT N{{ tag.jlpt_level }}</span>
    </header>
    <p>{{ selectedSense?.label || explanation?.function_summary || tag.description }}</p>
    <p v-if="explanation?.status === 'partial'" class="uncertainty">当前结构已识别，具体语义仍保留候选。</p>

    <div v-if="tag.sense_candidates.length > 1" class="sense-options" aria-label="语义候选">
      <button
        v-for="candidate in tag.sense_candidates"
        :key="candidate.sense_id"
        type="button"
        :class="{ active: (selectedSenseId ?? tag.selected_sense_id) === candidate.sense_id }"
        @click="selectedSenseId = candidate.sense_id"
      >
        {{ candidate.label }}
        <small>{{ candidate.confidence }}%</small>
      </button>
    </div>
    <p v-if="selectedSense?.evidence.length" class="sense-evidence">
      依据：{{ selectedSense.evidence.join("；") }}
    </p>

    <section v-if="expanded && explanation" class="details">
      <dl>
        <template v-if="explanation.connection">
          <dt>接续</dt>
          <dd>{{ explanation.connection }}</dd>
        </template>
        <template v-if="explanation.morphology_chain.length">
          <dt>活用链</dt>
          <dd>{{ explanation.morphology_chain.join(" → ") }}</dd>
        </template>
        <template v-if="explanation.bound_captures.length">
          <dt>本句捕获</dt>
          <dd>{{ explanation.bound_captures.map((capture) => capture.surface).join(" · ") }}</dd>
        </template>
      </dl>
      <div
        v-for="(block, index) in explanation.content_blocks"
        :key="`${block.kind}-${index}`"
        :class="['content-block', `content-${block.kind}`]"
      >
        <b v-if="block.label">{{ block.label }}</b>
        <p>{{ block.text }}</p>
      </div>
      <div v-if="explanation.dictionary_targets.length" class="dictionary-actions">
        <button
          v-for="target in explanation.dictionary_targets"
          :key="`${target.base_form}-${target.char_range[0]}`"
          type="button"
          @click="emit('openDictionary', target)"
        >
          {{ target.label }}
        </button>
      </div>
      <details v-if="explanation.evidence.length" class="evidence">
        <summary>识别依据</summary>
        <ul>
          <li v-for="item in explanation.evidence" :key="item">{{ item }}</li>
        </ul>
      </details>
      <footer>
        <span>目录状态：{{ explanation.audit_status }} · 内容 v{{ explanation.content_version }}</span>
        <span v-if="explanation.source_refs.length">来源：{{ explanation.source_refs.join("；") }}</span>
      </footer>
    </section>

    <button v-if="explanation" type="button" class="expand-toggle" @click="expanded = !expanded">
      {{ expanded ? "收起" : "展开讲解" }}
    </button>
  </aside>
</template>

<style scoped>
.grammar-popover {
  position: fixed;
  z-index: 1010;
  box-sizing: border-box;
  padding: 14px 16px;
  border: 1px solid color-mix(in srgb, #1769aa 35%, var(--border-color));
  border-radius: var(--radius-md);
  background: var(--glass-bg);
  box-shadow: var(--shadow-md);
  backdrop-filter: var(--glass-filter);
  color: var(--text-primary);
  font: .88rem/1.55 var(--font-ja);
}
header { display: flex; align-items: baseline; justify-content: space-between; gap: 12px; }
header > div { display: grid; min-width: 0; }
strong { color: #1769aa; font-size: 1rem; }
header span { color: var(--text-muted); font: 700 .7rem var(--font-ui); white-space: nowrap; }
header small { color: var(--text-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
p { margin: 8px 0 0; color: var(--text-secondary); }
.uncertainty { padding: 6px 8px; border-radius: var(--radius-sm); background: color-mix(in srgb, #1769aa 8%, transparent); font-size: .78rem; }
.sense-options { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 10px; }
button { border: 1px solid var(--border-color); border-radius: 999px; background: transparent; color: var(--text-secondary); cursor: pointer; font: inherit; }
.sense-options button { display: inline-flex; gap: 5px; align-items: center; padding: 4px 8px; font-size: .75rem; }
.sense-options button.active { border-color: #1769aa; color: #1769aa; background: color-mix(in srgb, #1769aa 8%, transparent); }
.sense-options small { color: var(--text-muted); }
.sense-evidence { color: var(--text-muted); font-size: .75rem; }
.details { max-height: min(48vh, 420px); margin-top: 12px; padding-top: 10px; border-top: 1px solid var(--border-color); overflow: auto; }
dl { display: grid; grid-template-columns: max-content 1fr; gap: 4px 10px; margin: 0; }
dt { color: var(--text-muted); font-size: .75rem; }
dd { margin: 0; color: var(--text-secondary); }
.content-block { margin-top: 10px; }
.content-block b { color: var(--text-primary); font-size: .78rem; }
.content-block p { margin-top: 2px; }
.content-warning { padding: 7px 9px; border-left: 2px solid color-mix(in srgb, #1769aa 55%, var(--border-color)); background: color-mix(in srgb, #1769aa 5%, transparent); }
.dictionary-actions { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 12px; }
.dictionary-actions button, .expand-toggle { padding: 5px 9px; }
.expand-toggle { margin-top: 10px; color: #1769aa; border-color: color-mix(in srgb, #1769aa 45%, var(--border-color)); }
.evidence { margin-top: 12px; color: var(--text-muted); font-size: .75rem; }
.evidence summary { cursor: pointer; }
.evidence ul { margin: 5px 0 0; padding-left: 1.25rem; }
footer { display: grid; gap: 2px; margin-top: 12px; color: var(--text-muted); font-size: .7rem; }
</style>
