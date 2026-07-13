<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { AnnotatedToken, DictEntry, DictionaryChoiceOption, DictionaryLink, DictionaryLookup } from "../types";
import DictionaryContent from "./dictionary/DictionaryContent.vue";
import DictionaryChoiceBar from "./dictionary/DictionaryChoiceBar.vue";
import LoadingSkeleton from "./common/LoadingSkeleton.vue";

const props = defineProps<{
  show: boolean;
  x: number;
  y: number;
  placement: "above" | "below";
  token: AnnotatedToken | null;
  lookup: DictionaryLookup | null;
  loading: boolean;
  canGoBack: boolean;
  width?: number;
  kindLabel?: string;
}>();

const emit = defineEmits<{
  enter: [];
  leave: [];
  navigate: [target: string];
  select: [target: string];
  back: [];
}>();

const formattedPos = computed(() => {
  if (!props.token) return "";
  const head = props.token.bunsetsu.head_word;
  return [head.pos.major, head.pos.sub1].filter((part) => part && part !== "*").join(" · ");
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

const dictionarySourceLabel = computed(() => dictionarySourceNames.value.join(" · "));
const loadingContentHeight = computed(() => `${cachedContentHeight.value || 220}px`);

const activeEntry = computed(() => visibleEntries.value.find((entry) => entry.is_preferred) ?? visibleEntries.value[0]);

const activeHeadword = computed(() =>
  props.lookup?.selected_target
  ?? activeEntry.value?.headword
  ?? props.lookup?.query
  ?? props.token?.bunsetsu.head_word.base_form
  ?? "",
);

const isSourceQuery = computed(() =>
  !props.lookup || props.lookup.query === props.token?.bunsetsu.head_word.base_form,
);

const activeReading = computed(() => {
  if (activeEntry.value?.reading) return activeEntry.value.reading;
  if (props.lookup?.reading) return props.lookup.reading;
  return isSourceQuery.value ? props.token?.bunsetsu.head_word.reading : null;
});

const panelMaxHeight = computed(() => {
  const viewportHeight = typeof window === "undefined" ? 800 : window.innerHeight;
  const available = props.placement === "above" ? props.y - 20 : viewportHeight - props.y - 20;
  return `${Math.max(160, Math.min(Math.round(viewportHeight * 0.7), 620, available))}px`;
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
      :class="`tooltip-${placement}`"
      :style="{ left: x + 'px', top: y + 'px', width: width ? width + 'px' : undefined, maxHeight: panelMaxHeight }"
      role="dialog"
      aria-label="词典释义"
      @mouseenter="emit('enter')"
      @mouseleave="emit('leave')"
      @wheel.stop
    >
      <header class="tooltip-header">
        <button v-if="canGoBack" type="button" class="back-button" aria-label="返回上一词条" @click="emit('back')">‹</button>
        <div class="headword-block">
          <span class="base-form">{{ activeHeadword }}</span>
          <span v-if="activeReading" class="reading">【{{ activeReading }}】</span>
        </div>
        <span v-if="isSourceQuery" class="tooltip-pos">{{ formattedPos }}</span>
        <span v-if="kindLabel" class="tooltip-kind">{{ kindLabel }}</span>
      </header>

      <div v-if="isSourceQuery && token.bunsetsu.grammar_tags.length" class="tooltip-section grammar-list">
        <div v-for="tag in token.bunsetsu.grammar_tags" :key="tag.pattern_id" class="grammar-desc">
          <strong>「{{ tag.name_ja }}」</strong><span>{{ tag.description }}</span>
        </div>
      </div>

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
          <section v-for="group in dictionaryGroups" :key="group.name" class="dictionary-group">
          <h3 v-if="dictionaryGroups.length > 1">{{ group.name }}</h3>
          <article v-for="(entry, entryIndex) in group.entries" :key="entry.entry_key" class="dictionary-entry">
            <div class="entry-meta">
              <strong><span v-if="entry.is_preferred && readingOptions.length <= 1" class="preferred-mark" title="读音匹配">★</span>{{ entry.headword }}</strong>
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
    </section>
  </Transition>
</template>

<style scoped>
.tooltip-panel { position: fixed; z-index: 1000; width: min(460px, calc(100vw - 24px)); overflow: auto; overscroll-behavior: contain; padding: 14px; background: var(--glass-bg); backdrop-filter: var(--glass-filter); border: 1px solid var(--glass-border); border-radius: var(--radius-md); box-shadow: var(--shadow-md); color: var(--text-primary); font: .88rem/1.55 var(--font-ja); overflow-wrap: anywhere; pointer-events: auto; scrollbar-gutter: stable; }
.tooltip-above { transform: translate(-50%, -100%) translateY(-8px); }
.tooltip-below { transform: translate(-50%, 8px); }
.tooltip-header { position: sticky; top: -14px; z-index: 3; display: flex; gap: 8px; align-items: baseline; margin: -14px -14px 6px; padding: 14px 14px 10px; background: linear-gradient(180deg, color-mix(in srgb, var(--bg-primary) 94%, transparent) 0%, color-mix(in srgb, var(--bg-primary) 82%, transparent) 76%, transparent 100%); border-bottom: 1px solid color-mix(in srgb, var(--border-color) 65%, transparent); backdrop-filter: blur(18px); }
.headword-block { flex: 1; min-width: 0; }
.back-button { flex: 0 0 28px; padding: 0; font-size: 1.35rem; line-height: 26px; }
.base-form { color: var(--accent-color); font-size: 1.25rem; font-weight: 700; }
.reading, .tooltip-pos { color: var(--text-muted); font-size: .78rem; }
.tooltip-kind { flex: 0 0 auto; color: var(--text-muted); font: 700 .68rem var(--font-ui); }
.tooltip-section { border-top: 1px solid var(--border-color); padding-top: 10px; margin-top: 6px; }
.section-title { margin-bottom: 7px; color: var(--text-muted); font: 700 .72rem var(--font-ui); letter-spacing: .04em; }
.definition-heading { display: flex; align-items: baseline; justify-content: space-between; gap: 10px; margin-bottom: 8px; }
.definition-heading .section-title { margin: 0; }
.dictionary-sources { overflow: hidden; color: var(--text-muted); font: 700 .72rem var(--font-ja); text-align: right; text-overflow: ellipsis; white-space: nowrap; }
.definition-viewport.is-loading { overflow: hidden; }
.definition-skeleton { height: 100%; }
.grammar-desc { display: grid; grid-template-columns: auto 1fr; gap: 8px; }
.grammar-desc strong { color: var(--novelty-high-text); }
.grammar-desc span { color: var(--text-secondary); }
.relation-list { display: flex; flex-wrap: wrap; gap: 6px; }
button { border: 1px solid var(--border-color); border-radius: 999px; padding: 3px 9px; background: var(--bg-card); color: var(--accent-color); cursor: pointer; }
button:hover, button.active { border-color: var(--accent-color); background: var(--accent-light); }
.dictionary-entry + .dictionary-entry { border-top: 1px dashed var(--border-color); margin-top: 12px; padding-top: 12px; }
.dictionary-group + .dictionary-group { margin-top: 14px; }
.dictionary-group > h3 { position: sticky; top: 49px; z-index: 2; display: flex; width: max-content; max-width: calc(100% - 12px); margin: 0 0 10px 2px; padding: 4px 10px; border: 1px solid color-mix(in srgb, var(--border-color) 75%, transparent); border-radius: 999px; background: color-mix(in srgb, var(--bg-primary) 84%, transparent); box-shadow: 0 3px 10px color-mix(in srgb, var(--text-primary) 7%, transparent); backdrop-filter: blur(14px); color: var(--text-muted); font: 700 .72rem var(--font-ui); }
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
</style>
