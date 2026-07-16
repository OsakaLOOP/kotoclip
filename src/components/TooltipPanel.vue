<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { ChevronLeft, Star } from "@lucide/vue";
import { AnnotatedToken, DictEntry, DictionaryChoiceOption, DictionaryLink, DictionaryLookup } from "../types";
import DictionaryContent from "./dictionary/DictionaryContent.vue";
import DictionaryChoiceBar from "./dictionary/DictionaryChoiceBar.vue";
import LoadingSkeleton from "./common/LoadingSkeleton.vue";
import {
  morphologyLemma,
  morphologyPosLabel,
  morphologySteps as buildMorphologySteps,
  primaryMorphologyChain,
  readingForMorphologyLemma,
} from "../explanation/morphologyView";

const props = defineProps<{
  show: boolean;
  x: number;
  y: number;
  token: AnnotatedToken | null;
  lookup: DictionaryLookup | null;
  loading: boolean;
  canGoBack: boolean;
  width?: number;
  maxHeight?: number;
  kindLabel?: string;
  panelId: string;
}>();

const emit = defineEmits<{
  enter: [event: PointerEvent];
  leave: [event: PointerEvent];
  navigate: [target: string];
  select: [target: string];
  back: [];
}>();

const morphologyChain = computed(() => (
  props.token ? primaryMorphologyChain(props.token) : null
));

const sourceLemma = computed(() => (
  morphologyChain.value
    ? morphologyLemma(morphologyChain.value)
    : props.token?.bunsetsu.head_word.base_form ?? ""
));

const sourceQuery = computed(() => (
  morphologyChain.value?.lookup_form
  || props.token?.bunsetsu.head_word.base_form
  || ""
));

const formattedPos = computed(() => {
  if (!props.token) return "";
  return morphologyPosLabel(morphologyChain.value, props.token.bunsetsu.head_word.pos);
});

const morphologySteps = computed(() => buildMorphologySteps(morphologyChain.value));

const showMorphologySummary = computed(() => {
  const chain = morphologyChain.value;
  return Boolean(chain && (
    chain.surface_form !== sourceLemma.value
    || morphologySteps.value.length > 0
  ));
});

function normalizeReading(value: string | null | undefined) {
  return Array.from(value ?? "").map((character) =>
    character >= "ぁ" && character <= "ゖ"
      ? String.fromCharCode(character.charCodeAt(0) + 0x60)
      : character,
  ).join("");
}

const readingChoices = computed(() => {
  const choices = new Map<string, { label: string; preferred: boolean }>();
  for (const entry of props.lookup?.entries ?? []) {
    if (!entry.reading) continue;
    const key = normalizeReading(entry.reading);
    const existing = choices.get(key);
    choices.set(key, { label: existing?.label ?? entry.reading, preferred: Boolean(existing?.preferred || entry.is_preferred) });
  }
  return [...choices.entries()].map(([key, value]) => ({ key, ...value }));
});

const selectedReadingKey = ref<string | null>(null);
watch(
  () => props.lookup,
  () => {
    const choices = readingChoices.value;
    const preferred = choices.find((choice) => choice.preferred)?.key;
    const requested = normalizeReading(props.lookup?.reading);
    selectedReadingKey.value = preferred ?? choices.find((choice) => choice.key === requested)?.key ?? choices[0]?.key ?? null;
  },
  { immediate: true },
);

const visibleEntries = computed(() => {
  if (readingChoices.value.length <= 1 || !selectedReadingKey.value) return props.lookup?.entries ?? [];
  return (props.lookup?.entries ?? []).filter((entry) => normalizeReading(entry.reading) === selectedReadingKey.value);
});

const dictionaryGroups = computed(() => {
  const groups = new Map<string, NonNullable<typeof props.lookup>["entries"]>();
  const candidateTargets = new Set(props.lookup?.candidates.map((candidate) => candidate.target) ?? []);
  for (const entry of visibleEntries.value) {
    const hasManagedRelation = entry.links.some((link) => !candidateTargets.has(link.target));
    if (!entry.content_blocks.length && !hasManagedRelation) continue;
    const group = groups.get(entry.dict_name) ?? [];
    group.push(entry);
    groups.set(entry.dict_name, group);
  }
  return [...groups.entries()].map(([name, entries]) => ({ name, entries }));
});

const activeDictionaryName = ref<string | null>(null);
watch(
  dictionaryGroups,
  (groups) => {
    if (!groups.some((group) => group.name === activeDictionaryName.value)) {
      activeDictionaryName.value = groups[0]?.name ?? null;
    }
  },
  { immediate: true },
);

const dictionaryOptions = computed<DictionaryChoiceOption[]>(() =>
  dictionaryGroups.value.map((group) => ({
    key: group.name,
    label: group.name,
    active: activeDictionaryName.value === group.name,
  })),
);

const visibleDictionaryGroups = computed(() => {
  if (!activeDictionaryName.value) return [];
  return dictionaryGroups.value.filter((group) => group.name === activeDictionaryName.value);
});

const dictionarySourceNames = ref(["三省堂Super大辞林3.1"]);
const definitionViewportRef = ref<HTMLElement | null>(null);
const cachedContentHeight = ref(0);

watch(
  dictionaryGroups,
  (groups) => {
    if (groups.length) dictionarySourceNames.value = groups.map((group) => group.name);
  },
  { immediate: true },
);

watch(
  [() => props.loading, dictionaryGroups],
  async ([loading, groups]) => {
    if (loading || !groups.length) return;
    await nextTick();
    const height = definitionViewportRef.value?.scrollHeight ?? 0;
    if (height) cachedContentHeight.value = height;
  },
  { flush: "post" },
);

const dictionarySourceLabel = computed(() => activeDictionaryName.value ?? dictionarySourceNames.value.join(" · "));
const loadingContentHeight = computed(() => `${cachedContentHeight.value || 220}px`);

const activeEntry = computed(() => {
  const entries = visibleDictionaryGroups.value.flatMap((group) => group.entries);
  return entries.find((entry) => entry.is_preferred) ?? entries[0];
});

const isSourceQuery = computed(() =>
  !props.lookup || props.lookup.query === sourceQuery.value,
);

const showsSourceIdentity = computed(() => (
  isSourceQuery.value
  && (!props.lookup?.selected_target || props.lookup.selected_target === sourceQuery.value)
));

const activeHeadword = computed(() => {
  if (showsSourceIdentity.value) return sourceLemma.value;
  return props.lookup?.selected_target
    ?? activeEntry.value?.headword
    ?? props.lookup?.query
    ?? sourceLemma.value;
});

const activeReading = computed(() => {
  const reading = activeEntry.value?.reading
    || (showsSourceIdentity.value
      ? props.token?.bunsetsu.head_word.reading
      : props.lookup?.reading);
  return showsSourceIdentity.value
    ? readingForMorphologyLemma(morphologyChain.value, reading)
    : reading;
});

function relationLabel(relation: string) {
  return ({ candidate: "表记", antonym: "反义", synonym: "近义", parent: "亲项目", child: "子项目", phrase: "惯用句", reference: "参照", related: "关联", redirect: "转至" } as Record<string, string>)[relation] ?? "关联";
}

function candidateLabel(candidate: DictionaryLink) {
  const match = candidate.target.match(/[【〖（](.*?)[】〗）]$/u);
  return match?.[1] || candidate.label || candidate.target;
}

const candidateOptions = computed<DictionaryChoiceOption[]>(() =>
  (props.lookup?.candidates ?? []).map((candidate) => ({
    key: candidate.target,
    label: candidateLabel(candidate),
    active: props.lookup?.selected_target === candidate.target,
  })),
);

const readingOptions = computed<DictionaryChoiceOption[]>(() =>
  readingChoices.value.map((choice) => ({
    key: choice.key,
    label: choice.label,
    active: selectedReadingKey.value === choice.key,
    preferred: choice.preferred,
    title: choice.preferred ? "与正文读音匹配" : "其他收录读音",
  })),
);

function managedLinkGroups(entry: DictEntry) {
  const candidateTargets = new Set(props.lookup?.candidates.map((candidate) => candidate.target) ?? []);
  const groups = new Map<string, DictionaryLink[]>();
  for (const link of entry.links) {
    if (candidateTargets.has(link.target)) continue;
    const group = groups.get(link.relation) ?? [];
    group.push(link);
    groups.set(link.relation, group);
  }
  return [...groups.entries()].map(([relation, links]) => ({ relation, links }));
}

function handleDefinitionClick(event: MouseEvent) {
  const anchor = (event.target as HTMLElement).closest("a") as HTMLAnchorElement | null;
  if (!anchor) return;
  const marker = "https://kotoclip.invalid/entry/";
  if (anchor.href.startsWith(marker)) {
    event.preventDefault();
    emit("navigate", decodeURIComponent(anchor.href.slice(marker.length)));
  }
}
</script>

<template>
  <Transition name="fade">
    <section
      v-if="show && token"
      class="tooltip-panel"
      :id="panelId"
      :data-explanation-panel="panelId"
      :style="{ left: x + 'px', top: y + 'px', width: width ? width + 'px' : undefined, maxHeight: maxHeight === undefined ? undefined : maxHeight + 'px' }"
      role="dialog"
      aria-label="词典释义"
      @pointerenter="emit('enter', $event)"
      @pointerleave="emit('leave', $event)"
      @wheel.stop
    >
      <div class="tooltip-content" :data-explanation-content="panelId">
        <header class="tooltip-header">
          <button v-if="canGoBack" type="button" class="back-button" aria-label="返回上一词条" @click="emit('back')"><ChevronLeft :size="20" aria-hidden="true" /></button>
          <div class="header-grid">
            <div class="headword-block">
              <div class="headword-line">
                <span class="base-form">{{ activeHeadword }}</span>
                <span v-if="activeReading" class="reading">【{{ activeReading }}】</span>
              </div>
              <div class="headword-meta">
                <span v-if="isSourceQuery" class="tooltip-pos">{{ formattedPos }}</span>
                <span v-if="kindLabel" class="tooltip-kind">{{ kindLabel }}</span>
              </div>
            </div>
            <div v-if="showMorphologySummary && morphologyChain" class="header-morphology" aria-label="本句词形与活用">
              <strong v-if="morphologyChain.surface_form !== sourceLemma" class="current-form">{{ morphologyChain.surface_form }}</strong>
              <div v-for="step in morphologySteps" :key="step.operator_id" class="morphology-step">
                <b>{{ step.label || step.output_state }}</b>
                <span v-if="step.description">{{ step.description }}</span>
              </div>
            </div>
          </div>
        </header>

        <DictionaryChoiceBar
        v-if="candidateOptions.length"
        label="表记"
        :options="candidateOptions"
        @select="emit('select', $event)"
        />

        <DictionaryChoiceBar
        v-if="readingOptions.length > 1"
        label="读音"
        :options="readingOptions"
        @select="selectedReadingKey = $event"
        />

        <div v-if="loading || dictionaryGroups.length || !lookup?.candidates.length" class="tooltip-section definitions" @click="handleDefinitionClick">
        <div class="definition-heading">
          <span class="section-title">词典释义</span>
          <span class="dictionary-sources">{{ dictionarySourceLabel }}</span>
        </div>
        <div
          ref="definitionViewportRef"
          class="definition-viewport"
          :class="{ 'is-loading': loading }"
          :style="loading ? { height: loadingContentHeight } : undefined"
        >
          <LoadingSkeleton v-if="loading" class="definition-skeleton" variant="dictionary" />
          <div v-else-if="!dictionaryGroups.length" class="empty-state">暂无本地词典释义</div>
          <DictionaryChoiceBar
            v-if="dictionaryOptions.length > 1"
            class="dictionary-switcher"
            label="词典"
            :options="dictionaryOptions"
            @select="activeDictionaryName = $event"
          />
          <section v-for="group in visibleDictionaryGroups" :key="group.name" class="dictionary-group">
          <article v-for="(entry, entryIndex) in group.entries" :key="entry.entry_key" class="dictionary-entry">
            <div class="entry-meta">
              <strong><Star v-if="entry.is_preferred && readingOptions.length <= 1" class="preferred-mark" :size="13" fill="currentColor" aria-label="读音匹配" />{{ entry.headword }}</strong>
              <span v-if="group.entries.length > 1">释义 {{ entryIndex + 1 }}</span>
            </div>
            <div class="entry-body">
              <DictionaryContent :entry="entry" />
            <div v-if="managedLinkGroups(entry).length" class="managed-relations">
              <details v-for="relationGroup in managedLinkGroups(entry)" :key="relationGroup.relation" class="relation-group" :open="relationGroup.links.length <= 6">
                <summary><span>{{ relationLabel(relationGroup.relation) }}</span><span>{{ relationGroup.links.length }} 项</span></summary>
                <div class="relation-list">
                  <button v-for="link in relationGroup.links" :key="link.target" type="button" :data-relation="link.relation" @click="emit('navigate', link.target)">{{ link.label || link.target }}</button>
                </div>
              </details>
            </div>
            </div>
          </article>
          </section>
        </div>
        </div>
      </div>
    </section>
  </Transition>
</template>

<style scoped>
.tooltip-panel { position: fixed; z-index: 1000; box-sizing: border-box; width: min(460px, calc(100vw - 24px)); overflow: auto; overscroll-behavior: contain; padding: 14px; background: var(--glass-bg); backdrop-filter: var(--glass-filter); border: 1px solid var(--glass-border); border-radius: var(--radius-md); box-shadow: var(--shadow-md); color: var(--text-primary); font: .88rem/1.55 var(--font-ja); overflow-wrap: anywhere; pointer-events: auto; scrollbar-gutter: stable; }
.tooltip-header { position: sticky; top: -14px; z-index: 3; display: flex; gap: 8px; align-items: flex-start; margin: -14px -14px 6px; padding: 14px 14px 10px; background: linear-gradient(180deg, color-mix(in srgb, var(--bg-primary) 94%, transparent) 0%, color-mix(in srgb, var(--bg-primary) 82%, transparent) 76%, transparent 100%); border-bottom: 1px solid color-mix(in srgb, var(--border-color) 65%, transparent); backdrop-filter: blur(18px); }
.header-grid { flex: 1; min-width: 0; display: grid; grid-template-columns: minmax(0, .9fr) minmax(160px, 1.1fr); gap: 12px; align-items: start; }
.headword-block { min-width: 0; }
.headword-line { display: flex; flex-wrap: wrap; gap: 2px 4px; align-items: baseline; }
.headword-meta { display: flex; flex-wrap: wrap; gap: 4px 8px; margin-top: 2px; }
.back-button { display: grid; place-items: center; flex: 0 0 28px; padding: 0; line-height: 26px; }
.base-form { color: var(--accent-color); font-size: 1.25rem; font-weight: 700; }
.reading, .tooltip-pos { color: var(--text-muted); font-size: .78rem; }
.tooltip-kind { color: var(--text-muted); font: 700 .68rem var(--font-ui); }
.header-morphology { min-width: 0; display: grid; gap: 4px; padding-left: 11px; border-left: 1px solid color-mix(in srgb, var(--border-color) 72%, transparent); }
.current-form { color: var(--text-primary); font-size: .88rem; line-height: 1.35; }
.morphology-step { display: grid; grid-template-columns: max-content minmax(0, 1fr); gap: 6px; align-items: baseline; font-size: .68rem; line-height: 1.35; }
.morphology-step b { color: #6c5ab0; font: 700 .66rem var(--font-ui); }
.morphology-step span { color: var(--text-secondary); }
.tooltip-section { border-top: 1px solid var(--border-color); padding-top: 10px; margin-top: 6px; }
.section-title { margin-bottom: 7px; color: var(--text-muted); font: 700 .72rem var(--font-ui); letter-spacing: .04em; }
.definition-heading { display: flex; align-items: baseline; justify-content: space-between; gap: 10px; margin-bottom: 8px; }
.definition-heading .section-title { margin: 0; }
.dictionary-sources { overflow: hidden; color: var(--text-muted); font: 700 .72rem var(--font-ja); text-align: right; text-overflow: ellipsis; white-space: nowrap; }
.definition-viewport.is-loading { overflow: hidden; }
.definition-skeleton { height: 100%; }
.relation-list { display: flex; flex-wrap: wrap; gap: 6px; }
button { border: 1px solid var(--border-color); border-radius: 999px; padding: 3px 9px; background: var(--bg-card); color: var(--accent-color); cursor: pointer; }
button:hover, button.active { border-color: var(--accent-color); background: var(--accent-light); }
.dictionary-entry + .dictionary-entry { border-top: 1px dashed var(--border-color); margin-top: 12px; padding-top: 12px; }
.dictionary-group + .dictionary-group { margin-top: 14px; }
.dictionary-switcher { margin-bottom: 10px; }
.entry-meta { display: flex; justify-content: space-between; gap: 12px; margin-bottom: 6px; }
.entry-body { padding-top: 3px; }
.entry-meta span, .empty-state { color: var(--text-muted); font: .75rem var(--font-ui); }
.entry-meta .preferred-mark { margin-right: 4px; color: var(--accent-color); }
.relation-list { margin-top: 8px; }
.managed-relations { display: grid; gap: 8px; margin-top: 10px; padding-top: 9px; border-top: 1px dotted var(--border-color); }
.relation-group summary { display: flex; justify-content: space-between; margin-bottom: 4px; color: var(--text-muted); font: 700 .7rem var(--font-ui); cursor: pointer; }
.relation-list button[data-relation="antonym"] { color: var(--novelty-high-text); }
.relation-list button[data-relation="parent"], .relation-list button[data-relation="child"] { color: var(--text-secondary); }
.fade-enter-active, .fade-leave-active { transition: opacity .12s ease; }
.fade-enter-from, .fade-leave-to { opacity: 0; }
@media (max-width: 420px) { .header-grid { grid-template-columns: minmax(0, 1fr); } .header-morphology { padding: 7px 0 0; border-top: 1px solid color-mix(in srgb, var(--border-color) 72%, transparent); border-left: 0; } }
</style>
