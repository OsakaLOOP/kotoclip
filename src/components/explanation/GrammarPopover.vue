<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import type { GrammarDictionaryTarget, GrammarTag } from "../../types";
import { grammarReviewOverrides } from "../../grammar/review";
import { floatDebug } from "../../explanation/floatDebug";
import type { RectSnapshot } from "../../explanation/geometry";
import GrammarTrustBadges from "../grammar/GrammarTrustBadges.vue";

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
const selectedSenseId = ref<string | null>(null);
const explanation = computed(() => props.tag?.explanation ?? null);
const selectedSense = computed(() => {
  const tag = props.tag;
  if (!tag) return null;
  const id = selectedSenseId.value ?? tag.selected_sense_id;
  return tag.sense_candidates.find((candidate) => candidate.sense_id === id) ?? null;
});
const reviewOverride = computed(() => {
  const conceptId = props.tag?.concept_id;
  return conceptId ? grammarReviewOverrides.value[conceptId] : undefined;
});
const reviewStatus = computed(() => reviewOverride.value?.status ?? explanation.value?.review_status ?? "unverified");
const reviewer = computed(() => reviewOverride.value?.reviewer ?? "");
const reviewedAt = computed(() => reviewOverride.value?.reviewedAt ?? "");

const kindLabel = computed(() => ({
  morphology_feature: "活用",
  functional_morpheme: "功能语素",
  grammar_construction: "语法构式",
  bunsetsu_function: "文节功能",
  correlative_grammar: "呼应语法",
  unknown: "语法",
})[props.tag?.occurrence_kind ?? "unknown"]);

const summary = computed(() => selectedSense.value?.label || explanation.value?.function_summary || props.tag?.description || "");

function normalizeForm(value: string) {
  return value.replace(/[〜～○…（）()／/・\s]/g, "").toLocaleLowerCase();
}

const actualFormDiffers = computed(() => {
  const value = explanation.value;
  if (!value?.actual_form) return false;
  return normalizeForm(value.actual_form) !== normalizeForm(value.title);
});

const roleLabels: Record<string, string> = {
  predicate: "核心动作",
  connector: "接续",
  functional_verb: "补助用言",
  support_verb: "补助用言",
  particle: "助词",
  auxiliary: "助动词",
  operator: "活用",
};

const formParts = computed(() => {
  return (explanation.value?.bound_captures ?? []).map((capture) => ({
    ...capture,
    roleLabel: roleLabels[capture.name] ?? "成分",
  }));
});

const variants = computed(() => formParts.value.filter((part) => (
  part.base_form
  && part.base_form !== "*"
  && normalizeForm(part.surface) !== normalizeForm(part.base_form)
)));

const morphologyLabels = computed(() => (
  explanation.value?.morphology_chain?.filter((item) => !["て形", "で形"].includes(item)) ?? []
));

const showFormCard = computed(() => (
  actualFormDiffers.value
  || formParts.value.length > 1
  || variants.value.length > 0
  || morphologyLabels.value.length > 0
));

const usefulConnection = computed(() => {
  return explanation.value?.connection.trim() ?? "";
});

const displayBlocks = computed(() => {
  return explanation.value?.content_blocks ?? [];
});

function dictionaryLabel(target: GrammarDictionaryTarget) {
  return `词典 · ${target.base_form}`;
}

const style = computed(() => {
  const anchor = props.anchor;
  if (!anchor) return { left: "-10000px", top: "-10000px" };
  const width = Math.min(390, window.innerWidth - 24);
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
    <header class="popover-header">
      <div class="heading-copy">
        <span class="eyebrow">
          {{ kindLabel }}<template v-if="tag.jlpt_level"> · JLPT N{{ tag.jlpt_level }}</template>
        </span>
        <h2>{{ explanation?.title || tag.name_ja }}</h2>
      </div>
    </header>

    <p class="core-summary">{{ summary }}</p>

    <div v-if="tag.sense_candidates.length > 1" class="sense-options" aria-label="语义候选">
      <button
        v-for="candidate in tag.sense_candidates"
        :key="candidate.sense_id"
        type="button"
        :class="{ active: (selectedSenseId ?? tag.selected_sense_id) === candidate.sense_id }"
        @click="selectedSenseId = candidate.sense_id"
      >
        {{ candidate.label }}
      </button>
    </div>

    <section v-if="showFormCard && explanation" class="form-card">
      <div v-if="actualFormDiffers" class="actual-form">
        <span>本句形态</span>
        <strong>{{ explanation.actual_form }}</strong>
      </div>
      <div v-if="variants.length" class="variant-list">
        <span v-for="part in variants" :key="`${part.name}-${part.char_range[0]}`">
          {{ part.surface }} <b>←</b> {{ part.base_form }}
        </span>
      </div>
      <div v-if="formParts.length > 1" class="formation-line" aria-label="本句构成">
        <template v-for="(part, index) in formParts" :key="`${part.name}-${part.char_range[0]}`">
          <span><em>{{ part.roleLabel }}</em>{{ part.surface }}</span>
          <b v-if="index < formParts.length - 1">·</b>
        </template>
      </div>
      <div v-if="morphologyLabels.length" class="morphology-line">
        <span v-for="item in morphologyLabels" :key="item">{{ item }}</span>
      </div>
    </section>

    <dl v-if="usefulConnection" class="core-details">
      <dt>接续</dt>
      <dd>{{ usefulConnection }}</dd>
    </dl>

    <section v-if="displayBlocks.length" class="content-section">
      <div
        v-for="(block, index) in displayBlocks"
        :key="`${block.kind}-${index}`"
        :class="['content-block', `content-${block.kind}`]"
      >
        <strong v-if="block.label">{{ block.label }}</strong>
        <p>{{ block.text }}</p>
      </div>
    </section>

    <div v-if="explanation?.dictionary_targets.length" class="dictionary-actions">
      <button
        v-for="target in explanation.dictionary_targets"
        :key="`${target.base_form}-${target.char_range[0]}`"
        type="button"
        @click="emit('openDictionary', target)"
      >
        {{ dictionaryLabel(target) }}
      </button>
    </div>

    <footer v-if="explanation" class="popover-footer">
      <GrammarTrustBadges
        :provenance="explanation.provenance"
        :review-status="reviewStatus"
        :reviewer="reviewer"
        :reviewed-at="reviewedAt"
      />
    </footer>
  </aside>
</template>

<style scoped>
.grammar-popover { position: fixed; z-index: 1010; box-sizing: border-box; max-height: min(72vh, 620px); overflow: auto; overscroll-behavior: contain; padding: 17px 18px 14px; border: 1px solid color-mix(in srgb, #337eb7 28%, var(--border-color)); border-radius: 15px; background: color-mix(in srgb, var(--bg-primary) 94%, transparent); box-shadow: 0 18px 48px rgba(29, 58, 94, .14); backdrop-filter: blur(18px); color: var(--text-primary); font: .88rem/1.55 var(--font-ja); }
.popover-header { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; }
.heading-copy { display: grid; min-width: 0; }
.eyebrow { color: var(--text-muted); font: 750 .66rem/1.3 var(--font-ui); letter-spacing: .025em; }
h2 { margin: 2px 0 0; color: #1769aa; font-size: 1.18rem; line-height: 1.35; }
.core-summary { margin: 9px 0 0; color: var(--text-primary); font-size: .93rem; line-height: 1.65; }
.sense-options { display: flex; flex-wrap: wrap; gap: 0 13px; margin-top: 10px; padding-top: 9px; border-top: 1px solid color-mix(in srgb, var(--border-color) 70%, transparent); }
button { border: 1px solid var(--border-color); border-radius: 999px; background: transparent; color: var(--text-secondary); cursor: pointer; font: inherit; }
.sense-options button { padding: 2px 0 4px; border: 0; border-radius: 0; color: var(--text-muted); font-size: .74rem; }
.sense-options button.active { color: #1769aa; box-shadow: inset 0 -2px #1769aa; }
.form-card { display: grid; gap: 8px; margin-top: 13px; padding: 11px 12px; border-radius: 10px; background: color-mix(in srgb, #337eb7 5%, var(--bg-secondary)); }
.actual-form { display: grid; grid-template-columns: max-content minmax(0, 1fr); gap: 10px; align-items: baseline; }
.actual-form span { color: var(--text-muted); font: 700 .68rem var(--font-ui); }
.actual-form strong { color: var(--text-primary); font-size: .9rem; }
.variant-list { display: flex; flex-wrap: wrap; gap: 5px; }
.variant-list span { padding: 3px 7px; border-radius: 6px; background: color-mix(in srgb, var(--bg-primary) 82%, transparent); color: var(--text-secondary); font-size: .72rem; }
.variant-list b { color: #5487ae; font-weight: 500; }
.formation-line { display: flex; flex-wrap: wrap; gap: 5px; align-items: baseline; color: var(--text-secondary); font-size: .76rem; }
.formation-line > span { display: inline; }
.formation-line em { margin-right: 3px; color: var(--text-muted); font: 650 .62rem var(--font-ui); font-style: normal; }
.formation-line > b { color: var(--text-muted); font-weight: 400; }
.morphology-line { display: flex; flex-wrap: wrap; gap: 0 10px; color: #6c5ab0; font: 700 .65rem var(--font-ui); }
.morphology-line span + span::before { margin-right: 10px; color: var(--text-muted); content: "→"; }
.core-details { display: grid; grid-template-columns: max-content minmax(0, 1fr); gap: 7px 11px; margin: 13px 0 0; padding-top: 11px; border-top: 1px solid color-mix(in srgb, var(--border-color) 76%, transparent); }
.core-details dt { color: var(--text-muted); font: 700 .68rem var(--font-ui); }
.core-details dd { margin: 0; color: var(--text-secondary); font-size: .78rem; }
.content-section { display: grid; gap: 8px; margin-top: 12px; }
.content-block strong { display: block; margin-bottom: 2px; color: var(--text-muted); font: 700 .68rem var(--font-ui); }
.content-block p { margin: 0; color: var(--text-secondary); font-size: .8rem; }
.content-warning { padding: 8px 9px; border-left: 2px solid #d59a4d; background: color-mix(in srgb, #d59a4d 6%, transparent); }
.dictionary-actions { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 13px; }
.dictionary-actions button { padding: 4px 8px; color: #1769aa; border-color: color-mix(in srgb, #1769aa 28%, var(--border-color)); font-size: .7rem; }
.popover-footer { margin-top: 13px; padding-top: 10px; border-top: 1px solid color-mix(in srgb, var(--border-color) 70%, transparent); }
</style>
