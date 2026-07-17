<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { ChevronLeft, Star } from "@lucide/vue";
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

const displayableEntries = computed(() => {
  const candidateTargets = new Set(props.lookup?.candidates.map((candidate) => candidate.target) ?? []);
  return (props.lookup?.entries ?? []).filter((entry) => {
    const hasManagedRelation = entry.links.some((link) => !candidateTargets.has(link.target));
    return entry.content_blocks.length || hasManagedRelation;
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

function dictionaryHasReading(dictionaryName: string, readingKey: string | null) {
  if (!readingKey) return displayableEntries.value.some((entry) => entry.dict_name === dictionaryName);
  return displayableEntries.value.some((entry) => (
    entry.dict_name === dictionaryName && normalizeReading(entry.reading) === readingKey
  ));
}

const selectedCandidate = computed(() => (
  props.lookup?.candidates.find((candidate) => candidate.target === props.lookup?.selected_target) ?? null
));

function dictionarySupportsCurrentChoice(dictionaryName: string) {
  if (selectedCandidate.value && !selectedCandidate.value.dictionary_names.includes(dictionaryName)) {
    return false;
  }
  return dictionaryHasReading(dictionaryName, selectedReadingKey.value);
}

function selectDictionaryAfterChoice() {
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
  if (!dictionaryHasReading(dictionaryName, selectedReadingKey.value)) {
    const replacementReading = readingChoices.value.find((choice) =>
      dictionaryHasReading(dictionaryName, choice.key)
    );
    if (replacementReading) selectedReadingKey.value = replacementReading.key;
  }
  if (!dictionarySupportsCurrentChoice(dictionaryName)) selectDictionaryAfterChoice();
}

watch(
  () => props.lookup,
  async () => {
    await nextTick();
    selectDictionaryAfterChoice();
  },
  { immediate: true },
);

const dictionaryOptions = computed<DictionaryChoiceOption[]>(() =>
  dictionaryGroups.value.map((group) => ({
    key: group.name,
    label: group.name,
    active: activeDictionaryName.value === group.name,
    unavailable: !dictionarySupportsCurrentChoice(group.name),
    title: dictionarySupportsCurrentChoice(group.name) ? undefined : "当前表记或读音无此词典释义",
  })),
);

const visibleDictionaryGroups = computed(() => {
  if (!activeDictionaryName.value) return [];
  return dictionaryGroups.value
    .filter((group) => group.name === activeDictionaryName.value)
    .map((group) => {
      const filtered = selectedReadingKey.value
        ? group.entries.filter((entry) => normalizeReading(entry.reading) === selectedReadingKey.value)
        : group.entries;
      return { ...group, entries: filtered.length ? filtered : group.entries };
    });
});
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
    unavailable: Boolean(activeDictionaryName.value && !candidate.dictionary_names.includes(activeDictionaryName.value)),
    title: activeDictionaryName.value && !candidate.dictionary_names.includes(activeDictionaryName.value)
      ? `当前词典未收录此表记`
      : undefined,
  })),
);

const readingOptions = computed<DictionaryChoiceOption[]>(() =>
  readingChoices.value.map((choice) => ({
    key: choice.key,
    label: choice.label,
    active: selectedReadingKey.value === choice.key,
    preferred: choice.preferred,
    unavailable: Boolean(activeDictionaryName.value && !dictionaryHasReading(activeDictionaryName.value, choice.key)),
    title: activeDictionaryName.value && !dictionaryHasReading(activeDictionaryName.value, choice.key)
      ? "当前词典未收录此读音"
      : choice.preferred ? "与正文读音匹配" : "其他收录读音",
  })),
);

function handleCandidateSelect(target: string) {
  emit("select", target);
}

function handleReadingSelect(readingKey: string) {
  selectedReadingKey.value = readingKey;
  selectDictionaryAfterChoice();
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
  } else if (matchesDictionaryShortcut(event, dictionaryShortcutSettings.choiceKey, true) && readingOptions.value.length > 1) {
    handled = selectNextOption(candidateOptions.value, handleCandidateSelect);
  } else if (matchesDictionaryShortcut(event, dictionaryShortcutSettings.choiceKey)) {
    handled = readingOptions.value.length > 1
      ? selectNextOption(readingOptions.value, handleReadingSelect)
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

const readingShortcutKeys = computed(() => (
  dictionaryShortcutSettings.choiceKey
    ? [shortcutKeyLabel(dictionaryShortcutSettings.choiceKey)]
    : []
));

const candidateShortcutKeys = computed(() => {
  if (!dictionaryShortcutSettings.choiceKey) return [];
  const key = shortcutKeyLabel(dictionaryShortcutSettings.choiceKey);
  return readingOptions.value.length > 1 ? ["Shift", key] : [key];
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
        :shortcut-keys="candidateShortcutKeys"
        @select="handleCandidateSelect"
        />

        <DictionaryChoiceBar
        v-if="readingOptions.length > 1"
        label="读音"
        :options="readingOptions"
        :shortcut-keys="readingShortcutKeys"
        @select="handleReadingSelect"
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
          </template>
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
