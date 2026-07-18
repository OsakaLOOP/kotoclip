<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { ChevronLeft } from "@lucide/vue";
import { AnnotatedToken, DictEntry, DictionaryChoiceOption, DictionaryLink, DictionaryLookup } from "../types";
import {
  dictionaryShortcutSettings,
  matchesDictionaryShortcut,
  shortcutKeyLabel,
} from "../composables/useDictionaryShortcuts";
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
  shortcutsEnabled?: boolean;
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

const displayableEntries = computed(() => {
  const candidateTargets = new Set(props.lookup?.candidates.map((candidate) => candidate.target) ?? []);
  return (props.lookup?.entries ?? []).filter((entry) => {
    const hasManagedRelation = entry.links.some((link) => !candidateTargets.has(link.target));
    return entry.senses.length || entry.sections.length || entry.content_blocks.length || hasManagedRelation;
  });
});

const dictionaryGroups = computed(() => {
  const groupedEntries = new Map<string, NonNullable<typeof props.lookup>["entries"]>();
  for (const entry of displayableEntries.value) {
    const group = groupedEntries.get(entry.dict_name) ?? [];
    group.push(entry);
    groupedEntries.set(entry.dict_name, group);
  }
  const names = props.lookup?.dictionary_names?.length
    ? props.lookup.dictionary_names
    : [...groupedEntries.keys()];
  return names.map((name) => ({ name, entries: groupedEntries.get(name) ?? [] }));
});

const activeDictionaryName = ref<string | null>(null);
const selectedOccurrenceByDictionary = ref<Record<string, string>>({});

function meaningfulEntries(entries: DictEntry[]) {
  const withContent = entries.filter((entry) => (
    entry.entry_kind !== "navigation"
    && entry.entry_kind !== "redirect"
    && (entry.senses.length || entry.sections.length || entry.content_blocks.length)
  ));
  return withContent.length ? withContent : entries;
}

const selectedCandidate = computed(() => (
  props.lookup?.candidates.find((candidate) => candidate.target === props.lookup?.selected_target) ?? null
));

function dictionarySupportsCurrentChoice(dictionaryName: string) {
  if (selectedCandidate.value && !selectedCandidate.value.dictionary_names.includes(dictionaryName)) {
    return false;
  }
  return dictionaryGroups.value.some((group) => group.name === dictionaryName && group.entries.length > 0);
}

function defaultOccurrence(entries: DictEntry[]) {
  const candidates = meaningfulEntries(entries);
  return candidates.find((entry) => entry.occurrence_id === props.lookup?.selected_occurrence_id)
    ?? candidates.find((entry) => entry.is_preferred)
    ?? candidates[0]
    ?? null;
}

function synchronizeSelection() {
  const names = dictionaryGroups.value.map((group) => group.name);
  const previous = activeDictionaryName.value;
  const supported = names.filter(dictionarySupportsCurrentChoice);
  const defaultDictionary = names[0] ?? null;
  activeDictionaryName.value = (
    (defaultDictionary && supported.includes(defaultDictionary) ? defaultDictionary : null)
    ?? (previous && supported.includes(previous) ? previous : null)
    ?? supported[0]
    ?? defaultDictionary
    ?? previous
    ?? null
  );
  const nextSelection: Record<string, string> = {};
  for (const group of dictionaryGroups.value) {
    const previousId = selectedOccurrenceByDictionary.value[group.name];
    const entries = meaningfulEntries(group.entries);
    const selected = entries.find((entry) => entry.occurrence_id === previousId)
      ?? defaultOccurrence(entries);
    if (selected) nextSelection[group.name] = selected.occurrence_id;
  }
  selectedOccurrenceByDictionary.value = nextSelection;
}

function handleDictionarySelect(dictionaryName: string) {
  const selected = selectedCandidate.value;
  if (selected && !selected.dictionary_names.includes(dictionaryName)) {
    const replacement = (props.lookup?.candidates ?? [])
      .find((candidate) => candidate.dictionary_names.includes(dictionaryName));
    if (replacement) {
      activeDictionaryName.value = dictionaryName;
      emit("select", replacement.target);
      return;
    }
  }

  activeDictionaryName.value = dictionaryName;
  if (!dictionarySupportsCurrentChoice(dictionaryName)) synchronizeSelection();
}

watch(
  () => props.lookup,
  async () => {
    await nextTick();
    synchronizeSelection();
  },
  { immediate: true },
);

const dictionaryOptions = computed<DictionaryChoiceOption[]>(() =>
  dictionaryGroups.value.map((group) => ({
    key: group.name,
    label: group.name,
    active: activeDictionaryName.value === group.name,
    unavailable: !dictionarySupportsCurrentChoice(group.name),
    title: dictionarySupportsCurrentChoice(group.name) ? undefined : "当前词条无此词典释义",
  })),
);

const activeDictionaryEntries = computed(() => {
  const entries = dictionaryGroups.value.find((group) => group.name === activeDictionaryName.value)?.entries ?? [];
  return meaningfulEntries(entries);
});

const activeEntry = computed(() => {
  const selectedId = activeDictionaryName.value
    ? selectedOccurrenceByDictionary.value[activeDictionaryName.value]
    : null;
  return activeDictionaryEntries.value.find((entry) => entry.occurrence_id === selectedId)
    ?? defaultOccurrence(activeDictionaryEntries.value)
    ?? undefined;
});

function entryKindLabel(kind: string) {
  return ({ lexical: "词汇", phrase: "短语", surname: "姓氏", kanji: "汉字条", prefix: "接头成分", suffix: "接尾成分", bound_morpheme: "拘束成分", onomatopoeia: "拟声", navigation: "导航", redirect: "跳转" } as Record<string, string>)[kind] ?? "词条";
}

function plainHtml(value: string) {
  return value.replace(/<[^>]*>/gu, "").replace(/&amp;/gu, "&").replace(/&lt;/gu, "<").replace(/&gt;/gu, ">");
}

function firstSenseSummary(senses: DictEntry["senses"]): string | undefined {
  for (const sense of senses) {
    const value = sense.glosses[0]?.html
      || sense.definitions[0]?.html
      || firstSenseSummary(sense.children);
    if (value) return plainHtml(value);
  }
  return undefined;
}

function occurrenceLabel(entry: DictEntry) {
  const form = entry.header.display_form || entry.headword;
  const peers = activeDictionaryEntries.value.filter((candidate) => (
    (candidate.header.display_form || candidate.headword) === form
    && (candidate.header.reading || candidate.reading) === (entry.header.reading || entry.reading)
  ));
  const summary = firstSenseSummary(entry.senses);
  const qualifier = peers.length > 1 && summary
    ? summary
    : entry.entry_kind !== "lexical" ? entryKindLabel(entry.entry_kind) : "";
  return qualifier ? `${form} · ${qualifier}` : form;
}

const occurrenceOptions = computed<DictionaryChoiceOption[]>(() => (
  activeDictionaryEntries.value.map((entry) => ({
    key: entry.occurrence_id,
    label: occurrenceLabel(entry),
    active: entry.occurrence_id === activeEntry.value?.occurrence_id,
    preferred: entry.is_preferred,
    title: [
      entry.header.reading ? `读音：${entry.header.reading}` : "",
      entryKindLabel(entry.entry_kind),
      entry.match_evidence?.kind ? `命中：${entry.match_evidence.kind}` : "",
    ].filter(Boolean).join("；"),
  }))
));

function handleOccurrenceSelect(occurrenceId: string) {
  if (!activeDictionaryName.value) return;
  selectedOccurrenceByDictionary.value = {
    ...selectedOccurrenceByDictionary.value,
    [activeDictionaryName.value]: occurrenceId,
  };
}
const definitionViewportRef = ref<HTMLElement | null>(null);
const cachedContentHeight = ref(0);

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

const loadingContentHeight = computed(() => `${cachedContentHeight.value || 220}px`);

const isSourceQuery = computed(() =>
  !props.lookup || props.lookup.query === sourceQuery.value,
);

const showsSourceIdentity = computed(() => (
  isSourceQuery.value
  && (!props.lookup?.selected_target || props.lookup.selected_target === sourceQuery.value)
));

const activeHeadword = computed(() => {
  return (activeEntry.value?.header.display_form
    || activeEntry.value?.headword)
    ?? props.lookup?.query
    ?? sourceLemma.value;
});

const activeReading = computed(() => {
  const reading = activeEntry.value?.header.reading
    || activeEntry.value?.reading
    || (showsSourceIdentity.value
      ? props.token?.bunsetsu.head_word.reading
      : props.lookup?.reading);
  return showsSourceIdentity.value
    ? readingForMorphologyLemma(morphologyChain.value, reading)
    : reading;
});

const activeHeaderTags = computed(() => [
  ...(activeEntry.value?.header.pos_tags ?? []),
  ...(activeEntry.value?.header.usage_tags ?? []),
]);

const activeHeaderFacts = computed(() => {
  const header = activeEntry.value?.header;
  if (!header) return [];
  const facts = header.pronunciations.map((item) => `${item.label} ${item.value}`);
  if (header.origin) facts.push(`词源 ${header.origin}`);
  if (header.historical_reading) facts.push(`历史读音 ${header.historical_reading}`);
  for (const form of header.scoped_forms) {
    if (form.form !== header.display_form) facts.push(`异表记 ${form.form}`);
  }
  if (header.short_note) facts.push(header.short_note);
  return facts;
});

const matchHint = computed(() => {
  const evidence = activeEntry.value?.match_evidence;
  if (!evidence) return "";
  return ({ explicit_alias: "词典别名", compatibility_alias: "兼容表记", reading_fallback: "读音回退", fuzzy: "模糊命中" } as Record<string, string>)[evidence.kind] ?? "";
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
    unavailable: Boolean(activeDictionaryName.value && !candidate.dictionary_names.includes(activeDictionaryName.value)),
    title: activeDictionaryName.value && !candidate.dictionary_names.includes(activeDictionaryName.value)
      ? `当前词典未收录此表记`
      : undefined,
  })),
);

function handleCandidateSelect(target: string) {
  emit("select", target);
}

function selectNextOption(options: DictionaryChoiceOption[], select: (key: string) => void) {
  if (options.length <= 1) return false;
  const activeIndex = options.findIndex((option) => option.active);
  select(options[(activeIndex + 1 + options.length) % options.length].key);
  return true;
}

function handleShortcut(event: KeyboardEvent) {
  if (!props.shortcutsEnabled || !props.show || props.loading) return;
  if (event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement || event.target instanceof HTMLSelectElement) return;

  let handled = false;
  if (matchesDictionaryShortcut(event, dictionaryShortcutSettings.dictionaryKey)) {
    handled = selectNextOption(dictionaryOptions.value, handleDictionarySelect);
  } else if (matchesDictionaryShortcut(event, dictionaryShortcutSettings.choiceKey, true) && candidateOptions.value.length > 1) {
    handled = selectNextOption(candidateOptions.value, handleCandidateSelect);
  } else if (matchesDictionaryShortcut(event, dictionaryShortcutSettings.choiceKey)) {
    handled = occurrenceOptions.value.length > 1
      ? selectNextOption(occurrenceOptions.value, handleOccurrenceSelect)
      : selectNextOption(candidateOptions.value, handleCandidateSelect);
  }

  if (handled) {
    event.preventDefault();
    event.stopPropagation();
  }
}

const dictionaryShortcutKeys = computed(() => (
  dictionaryShortcutSettings.dictionaryKey
    ? [shortcutKeyLabel(dictionaryShortcutSettings.dictionaryKey)]
    : []
));

const occurrenceShortcutKeys = computed(() => (
  dictionaryShortcutSettings.choiceKey
    ? [shortcutKeyLabel(dictionaryShortcutSettings.choiceKey)]
    : []
));

const candidateShortcutKeys = computed(() => {
  if (!dictionaryShortcutSettings.choiceKey) return [];
  const key = shortcutKeyLabel(dictionaryShortcutSettings.choiceKey);
  return occurrenceOptions.value.length > 1 ? ["Shift", key] : [key];
});

onMounted(() => window.addEventListener("keydown", handleShortcut));
onBeforeUnmount(() => window.removeEventListener("keydown", handleShortcut));

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
                <span v-for="tag in activeHeaderTags" :key="`${tag.kind}:${tag.label}`" class="header-tag" :data-kind="tag.kind">{{ tag.label }}</span>
                <span v-if="!activeHeaderTags.length && isSourceQuery" class="tooltip-pos">{{ formattedPos }}</span>
                <span v-if="activeEntry && activeEntry.entry_kind !== 'lexical'" class="header-tag" data-kind="entry-kind">{{ entryKindLabel(activeEntry.entry_kind) }}</span>
                <span v-if="matchHint" class="match-hint">{{ matchHint }}</span>
                <span v-if="kindLabel" class="tooltip-kind">{{ kindLabel }}</span>
              </div>
            </div>
            <div v-if="activeHeaderFacts.length || showMorphologySummary && morphologyChain && isSourceQuery" class="header-morphology" aria-label="当前词条与本句信息">
              <div v-for="fact in activeHeaderFacts" :key="fact" class="header-fact">{{ fact }}</div>
              <template v-if="showMorphologySummary && morphologyChain && isSourceQuery">
                <strong v-if="morphologyChain.surface_form !== sourceLemma" class="current-form">{{ morphologyChain.surface_form }}</strong>
                <div v-for="step in morphologySteps" :key="step.operator_id" class="morphology-step">
                  <b>{{ step.label || step.output_state }}</b>
                  <span v-if="step.description">{{ step.description }}</span>
                </div>
              </template>
            </div>
          </div>
        </header>

        <DictionaryChoiceBar
          v-if="occurrenceOptions.length > 1"
          label="词条"
          :options="occurrenceOptions"
          :shortcut-keys="occurrenceShortcutKeys"
          @select="handleOccurrenceSelect"
        />

        <DictionaryChoiceBar
          v-if="candidateOptions.length"
          label="跳转"
          :options="candidateOptions"
          :shortcut-keys="candidateShortcutKeys"
          @select="handleCandidateSelect"
        />

        <div v-if="loading || dictionaryGroups.length || !lookup?.candidates.length" class="tooltip-section definitions" @click="handleDefinitionClick">
          <div
            ref="definitionViewportRef"
            class="definition-viewport"
            :class="{ 'is-loading': loading }"
            :style="loading ? { height: loadingContentHeight } : undefined"
          >
            <LoadingSkeleton v-if="loading" class="definition-skeleton" variant="dictionary" />
            <template v-else>
              <DictionaryChoiceBar
                v-if="dictionaryOptions.length"
                class="dictionary-switcher"
                label="词典"
                :options="dictionaryOptions"
                :shortcut-keys="dictionaryShortcutKeys"
                @select="handleDictionarySelect"
              />
              <section v-if="activeEntry" class="dictionary-group">
                <article :key="activeEntry.occurrence_id" class="dictionary-entry">
                  <div class="entry-body">
                    <DictionaryContent :entry="activeEntry" @navigate="emit('navigate', $event)" />
                    <div v-if="managedLinkGroups(activeEntry).length" class="managed-relations">
                      <details v-for="relationGroup in managedLinkGroups(activeEntry)" :key="relationGroup.relation" class="relation-group" :open="relationGroup.links.length <= 6">
                        <summary><span>{{ relationLabel(relationGroup.relation) }}</span><span>{{ relationGroup.links.length }} 项</span></summary>
                        <div class="relation-list">
                          <button v-for="link in relationGroup.links" :key="`${link.relation}:${link.target}`" type="button" :data-relation="link.relation" @click="emit('navigate', link.target)">{{ link.label || link.target }}</button>
                        </div>
                      </details>
                    </div>
                  </div>
                </article>
              </section>
              <div v-else class="empty-state">当前词典没有可显示的 occurrence。</div>
            </template>
          </div>
        </div>
      </div>
    </section>
  </Transition>
</template>

<style scoped>
.tooltip-panel { position: fixed; z-index: 1000; box-sizing: border-box; width: min(480px, calc(100vw - 24px)); overflow: auto; overscroll-behavior: contain; padding: 14px; background: var(--glass-bg); backdrop-filter: var(--glass-filter); border: 1px solid var(--glass-border); border-radius: var(--radius-md); box-shadow: var(--shadow-md); color: var(--text-primary); font: .88rem/1.55 var(--font-ui); overflow-wrap: anywhere; pointer-events: auto; scrollbar-gutter: stable; }
.tooltip-header { position: sticky; top: -14px; z-index: 3; display: flex; gap: 8px; align-items: flex-start; margin: -14px -14px 6px; padding: 14px 14px 10px; background: linear-gradient(180deg, color-mix(in srgb, var(--bg-primary) 94%, transparent) 0%, color-mix(in srgb, var(--bg-primary) 82%, transparent) 76%, transparent 100%); border-bottom: 1px solid color-mix(in srgb, var(--border-color) 65%, transparent); backdrop-filter: blur(18px); }
.header-grid { flex: 1; min-width: 0; display: grid; grid-template-columns: minmax(0, .9fr) minmax(160px, 1.1fr); gap: 12px; align-items: start; }
.headword-block { min-width: 0; }
.headword-line { display: flex; flex-wrap: wrap; gap: 2px 4px; align-items: baseline; }
.headword-meta { display: flex; flex-wrap: wrap; gap: 4px 8px; margin-top: 2px; }
.back-button { display: grid; place-items: center; flex: 0 0 28px; padding: 0; line-height: 26px; }
.base-form { color: var(--accent-color); font: 700 1.25rem/1.35 var(--font-ja); }
.reading { color: var(--text-muted); font: .78rem var(--font-ja); }
.tooltip-pos { color: var(--text-muted); font-size: .75rem; }
.tooltip-kind { color: var(--text-muted); font: 700 .68rem var(--font-ui); }
.header-tag, .match-hint { display: inline-flex; align-items: center; border: 1px solid color-mix(in srgb, var(--border-color) 82%, transparent); border-radius: 4px; padding: 0 5px; color: var(--text-secondary); font: 700 .66rem/1.55 var(--font-ui); }
.header-tag[data-kind="usage"], .header-tag[data-kind="entry-kind"], .match-hint { background: var(--accent-light); color: var(--accent-color); }
.header-morphology { min-width: 0; display: grid; gap: 4px; padding-left: 11px; border-left: 1px solid color-mix(in srgb, var(--border-color) 72%, transparent); }
.header-fact { color: var(--text-secondary); font: .7rem/1.4 var(--font-ui); }
.current-form { color: var(--text-primary); font-size: .88rem; line-height: 1.35; }
.morphology-step { display: grid; grid-template-columns: max-content minmax(0, 1fr); gap: 6px; align-items: baseline; font-size: .68rem; line-height: 1.35; }
.morphology-step b { color: #6c5ab0; font: 700 .66rem var(--font-ui); }
.morphology-step span { color: var(--text-secondary); }
.tooltip-section { border-top: 1px solid var(--border-color); padding-top: 10px; margin-top: 6px; }
.tooltip-header + :deep(.dictionary-choice-bar) { margin-top: 0; padding-top: 9px; border-top: 0; }
.definition-viewport.is-loading { overflow: hidden; }
.definition-skeleton { height: 100%; }
.relation-list { display: flex; flex-wrap: wrap; gap: 6px; }
button { border: 1px solid var(--border-color); border-radius: 999px; padding: 3px 9px; background: var(--bg-card); color: var(--accent-color); cursor: pointer; }
button:hover, button.active { border-color: var(--accent-color); background: var(--accent-light); }
.dictionary-entry + .dictionary-entry { border-top: 1px dashed var(--border-color); margin-top: 12px; padding-top: 12px; }
.dictionary-group + .dictionary-group { margin-top: 14px; }
.dictionary-switcher { margin: 0 0 10px; padding-top: 0; border-top: 0; }
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
