<script setup lang="ts">
import { convertFileSrc } from "@tauri-apps/api/core";
import { useVirtualizer } from "@tanstack/vue-virtual";
import { ArrowLeft, BookOpen, ChevronDown, Download, LoaderCircle, RotateCcw, Settings2 } from "@lucide/vue";
import { computed, nextTick, onBeforeUnmount, onMounted, ref, shallowRef, watch } from "vue";
import DictionaryContent from "../dictionary/DictionaryContent.vue";
import ExportPanel from "../ExportPanel.vue";
import ReaderAppearancePanel from "./ReaderAppearancePanel.vue";
import ReaderImageBlock from "./ReaderImageBlock.vue";
import ReaderNavigationPanel from "./ReaderNavigationPanel.vue";
import ReaderProgressBar from "./ReaderProgressBar.vue";
import { isTauri } from "@tauri-apps/api/core";
import { nlpRequest } from "../../services/nlp";
import { readerRequest } from "../../services/reader";
import type { AnalysisUnit, DocumentSession, UnitUpdate } from "../../reader/session";
import { buildReaderRows, type ReaderRow, type ReaderTextRow } from "../../reader/rows";
import { readingEstimate, type ReaderAppearance } from "../../reader/reading";
import { resourceKey, type LibraryBook, type LibraryResource } from "../../reader/library";
import type { ReaderDocument, ReaderTextBlock } from "../../reader/document";
import type { DictEntry } from "../../types";
import type { SavedSelection } from "../../types/reader";

interface LookupTarget {
  id: string;
  parent_outer_id: string | null;
  char_range: [number, number];
  surface: string;
  morpheme_ids: string[];
  lexical_core_ids: string[];
  source_formation_ids: string[];
  lookup_forms: { kind: string; form: string; reading: string | null }[];
  reading_evidence: string[];
  decision: string;
  reason: string;
}

interface LookupGroup {
  outer_targets: LookupTarget[];
  inner_targets: LookupTarget[];
  grammar_targets: { char_range: [number, number]; display_form: string; normalized_form: string }[];
  coverage_status: string;
}

interface UnitLookup {
  unit: UnitUpdate;
  contextOffset: number;
  group: LookupGroup;
}

interface TargetHit {
  key: string;
  unit: UnitLookup;
  target: LookupTarget;
  range: [number, number];
  children: LookupTarget[];
}

interface TextPart {
  key: string;
  text: string;
  range: [number, number];
  hit: TargetHit | null;
}

interface SelectionDraft {
  start: number;
  end: number;
  surface: string;
  baseForm: string;
  reading: string;
}

const props = defineProps<{
  readerDocument: ReaderDocument;
  session: DocumentSession;
  book: LibraryBook | null;
  resources: LibraryResource[];
  appearance: ReaderAppearance;
  initialOffset: number;
  selections: SavedSelection[];
}>();

const emit = defineEmits<{
  back: [];
  progress: [offset: number, chapter: string | null, seconds: number];
  updateAppearance: [appearance: ReaderAppearance];
  continue: [];
  retry: [unitId?: string];
  saveSelection: [selection: SelectionDraft];
  removeSelection: [selection: SavedSelection];
  clearSelections: [];
  updateSelection: [selection: SavedSelection, note: string];
  exportSelections: [];
}>();

const scrollElement = ref<HTMLElement | null>(null);
const showNavigation = ref(false);
const showAppearance = ref(false);
const showExport = ref(false);
const groups = shallowRef(new Map<string, UnitLookup>());
const pendingTargets = new Set<string>();
const activeHit = ref<TargetHit | null>(null);
const outerHit = ref<TargetHit | null>(null);
const activeQuery = ref<Record<string, unknown> | null>(null);
const queryBusy = ref(false);
const queryError = ref("");
const wordState = ref<{ known: boolean; exposures: number } | null>(null);
const innerOuterId = ref<string | null>(null);
const pendingSelection = ref<SelectionDraft | null>(null);
const hoveredHit = ref<TargetHit | null>(null);
const exposedTargets = new Set<string>();
let hoverTimer: ReturnType<typeof setTimeout> | undefined;
let progressTimer: ReturnType<typeof setTimeout> | undefined;
let lastProgressAt = Date.now();

const rows = computed<ReaderRow[]>(() => buildReaderRows(
  props.readerDocument.blocks.filter((block): block is ReaderTextBlock => block.kind !== "image"),
  props.readerDocument,
  resolveImage,
  true,
));
const virtualizer = useVirtualizer<HTMLElement, HTMLElement>(computed(() => ({
  count: rows.value.length,
  getScrollElement: () => scrollElement.value,
  estimateSize: () => Math.max(72, props.appearance.fontSize * props.appearance.lineHeight * 2 + props.appearance.paragraphGap),
  overscan: 6,
})));
const visibleRows = computed(() => virtualizer.value.getVirtualItems().map((item) => ({ item, row: rows.value[item.index] })).filter((value) => value.row));
const currentOffset = ref(props.initialOffset);
const estimate = computed(() => readingEstimate(currentOffset.value, props.session.plan.prepared.text.length));
const currentChapter = computed(() => {
  const chapters = props.readerDocument.chapters.filter((chapter) => chapter.charOffset <= currentOffset.value);
  return chapters[chapters.length - 1]?.title || "正文";
});
const readableCount = computed(() => props.session.progress.basic);
const statusLabel = computed(() => {
  if (props.session.progress.failed) return "部分分析失败";
  if (props.session.paused) return "分析已暂停";
  if (props.session.progress.complete >= props.session.progress.total && props.session.progress.total > 0) return "分析完成";
  return `已分析 ${readableCount.value}/${props.session.progress.total}`;
});
const activeChildren = computed(() => {
  if (!outerHit.value) return [];
  return outerHit.value.children.filter((target) => target.decision === "queryable");
});
const queryEntries = computed<DictEntry[]>(() => {
  const value = activeQuery.value;
  if (!value) return [];
  if (Array.isArray(value.entries)) return value.entries as DictEntry[];
  if (!Array.isArray(value.groups)) return [];
  return (value.groups as { entries?: DictEntry[] }[]).flatMap((group) => group.entries || []);
});
const queryForms = computed(() => Array.isArray(activeQuery.value?.forms) ? activeQuery.value?.forms as { display_form?: string; readings?: string[] }[] : []);

function resolveImage(source: string) {
  const resource = props.resources.find((item) => resourceKey(item.href) === resourceKey(source));
  if (!resource) return undefined;
  return { src: isTauri() ? convertFileSrc(resource.path) : resource.path, width: resource.width, height: resource.height };
}

function measureRow(node: unknown) {
  if (node instanceof HTMLElement) virtualizer.value.measureElement(node);
}

function documentUnit(unitId: string): UnitUpdate | undefined {
  return props.session.units[unitId];
}

function localLookupRange(unit: AnalysisUnit, document: NonNullable<UnitUpdate["document"]>): [number, number] {
  const offset = unit.context_range[0];
  return [
    Math.max(0, unit.anchor.char_range[0] - offset),
    Math.min(document.characters, unit.anchor.char_range[1] - offset),
  ];
}

async function loadTargets(unitPlan: typeof props.session.plan.units[number], unit: UnitUpdate) {
  if (!unit.document || !["basic", "enriching", "complete"].includes(unit.stage)) return;
  const key = `${props.session.session_id}:${props.session.generation}:${unit.unit_id}:${unit.artifact_revision}`;
  if (groups.value.has(key) || pendingTargets.has(key)) return;
  pendingTargets.add(key);
  try {
    const group = await nlpRequest<LookupGroup>({
      command: "lookup_document",
      session_id: props.session.session_id,
      text_version: props.session.text_version,
      generation: props.session.generation,
      unit_id: unit.unit_id,
      artifact_revision: unit.artifact_revision,
      range: localLookupRange(unitPlan, unit.document),
    });
    if (props.session.session_id !== key.split(":", 1)[0]) return;
    groups.value = new Map(groups.value).set(key, { unit, contextOffset: unitPlan.context_range[0], group });
  } catch {
    // 词典目标可以在分析完成后再次请求，正文保持可读。
  } finally {
    pendingTargets.delete(key);
  }
}

async function syncTargets() {
  const tasks = props.session.plan.units
    .map((unitPlan) => [unitPlan, documentUnit(unitPlan.id)] as const)
    .filter((item): item is [typeof props.session.plan.units[number], UnitUpdate] => Boolean(item[1]))
    .map(([unitPlan, unit]) => loadTargets(unitPlan, unit));
  await Promise.all(tasks);
}

function unitLookups(): UnitLookup[] {
  return [...groups.value.values()].filter((lookup) => {
    const current = props.session.units[lookup.unit.unit_id];
    return current?.artifact_revision === lookup.unit.artifact_revision && current.document?.id === lookup.unit.document?.id;
  });
}

function hitsForBlock(block: ReaderTextBlock): TargetHit[] {
  const hits: TargetHit[] = [];
  for (const lookup of unitLookups()) {
    for (const target of lookup.group.outer_targets) {
      const range: [number, number] = [
        target.char_range[0] + lookup.contextOffset,
        target.char_range[1] + lookup.contextOffset,
      ];
      if (range[1] <= block.charRange[0] || range[0] >= block.charRange[1]) continue;
      const children = lookup.group.inner_targets
        .filter((child) => child.parent_outer_id === target.id)
        .map((child) => ({ ...child, char_range: [child.char_range[0] + lookup.contextOffset, child.char_range[1] + lookup.contextOffset] as [number, number] }));
      hits.push({ key: `${lookup.unit.unit_id}:${target.id}`, unit: lookup, target, range, children });
    }
  }
  return hits.sort((left, right) => left.range[0] - right.range[0] || right.range[1] - left.range[1]);
}

function partsFor(row: ReaderTextRow): TextPart[] {
  const block = row.paragraph;
  let hits = hitsForBlock(block);
  if (innerOuterId.value) {
    const outer = hits.find((hit) => hit.key === innerOuterId.value);
    if (outer) {
      hits = outer.children.map((target) => ({ key: `${outer.key}:${target.id}`, unit: outer.unit, target, range: target.char_range, children: [] }));
    }
  }
  const boundaries = new Set<number>([block.charRange[0], block.charRange[1]]);
  for (const hit of hits) {
    boundaries.add(Math.max(block.charRange[0], hit.range[0]));
    boundaries.add(Math.min(block.charRange[1], hit.range[1]));
  }
  const points = [...boundaries].sort((left, right) => left - right);
  const characters = Array.from(block.text);
  return points.slice(0, -1).map((start, index) => {
    const end = points[index + 1];
    const hit = hits.find((candidate) => candidate.range[0] <= start && end <= candidate.range[1]) || null;
    return { key: `${block.id}:${start}`, text: characters.slice(start - block.charRange[0], end - block.charRange[0]).join(""), range: [start, end] as [number, number], hit };
  }).filter((part) => part.text.length > 0);
}

function isSelected(range: [number, number]) {
  return props.selections.some((selection) => range[0] < selection.end && selection.start < range[1]);
}

function selectionOffset(node: Node, offset: number): number | null {
  const element = node.nodeType === Node.ELEMENT_NODE ? node as Element : node.parentElement;
  const part = element?.closest<HTMLElement>("[data-char-start]");
  if (!part) return null;
  const start = Number(part.dataset.charStart);
  return Number.isFinite(start) ? start + (node.nodeType === Node.TEXT_NODE ? offset : 0) : null;
}

function captureSelection() {
  const selection = window.getSelection();
  if (!selection || selection.isCollapsed || !scrollElement.value || !selection.rangeCount) return;
  const range = selection.getRangeAt(0);
  if (!scrollElement.value.contains(range.commonAncestorContainer)) return;
  const anchor = selectionOffset(range.startContainer, range.startOffset);
  const focus = selectionOffset(range.endContainer, range.endOffset);
  const surface = selection.toString().trim();
  if (anchor === null || focus === null || !surface) return;
  const start = Math.min(anchor, focus);
  const end = Math.max(anchor, focus);
  const block = props.readerDocument.blocks.find((item): item is ReaderTextBlock => item.kind !== "image" && item.charRange[0] <= start && start < item.charRange[1])
    || props.readerDocument.blocks.find((item): item is ReaderTextBlock => item.kind !== "image");
  if (!block) return;
  const hit = hitsForBlock(block);
  const form = hit.find((item) => item.range[0] <= start && start < item.range[1])?.target.lookup_forms[0];
  pendingSelection.value = { start, end, surface, baseForm: form?.form || "", reading: form?.reading || "" };
}

function savePendingSelection() {
  if (!pendingSelection.value) return;
  emit("saveSelection", pendingSelection.value);
  pendingSelection.value = null;
  window.getSelection()?.removeAllRanges();
}

function updateSelection(selection: SavedSelection, note: string) {
  emit("updateSelection", selection, note);
}

function exportSelections() {
  emit("exportSelections");
}

function scheduleLookup(hit: TargetHit) {
  hoveredHit.value = hit;
  clearTimeout(hoverTimer);
  hoverTimer = setTimeout(() => void queryTarget(hit), 220);
}

function openChildren(hit: TargetHit) {
  if (hit.children.length > 0 && hit.target.source_formation_ids.length === 0) {
    innerOuterId.value = hit.key;
    outerHit.value = hit;
    activeHit.value = hit;
    void queryTarget(hit);
    return;
  }
  void queryTarget(hit);
}

async function queryTarget(hit: TargetHit, selectedForm?: string) {
  if (!hit.target.parent_outer_id) outerHit.value = hit;
  activeHit.value = hit;
  queryBusy.value = true;
  queryError.value = "";
  try {
    activeQuery.value = await nlpRequest<Record<string, unknown>>({
      command: "query_lookup_document",
      session_id: props.session.session_id,
      text_version: props.session.text_version,
      generation: props.session.generation,
      unit_id: hit.unit.unit.unit_id,
      artifact_revision: hit.unit.unit.artifact_revision,
      target_id: hit.target.id,
      selected_form: selectedForm || null,
    });
    const form = hit.target.lookup_forms[0];
    if (form) {
      await refreshWord(form.form, form.reading || "");
      if (!exposedTargets.has(hit.key)) {
        exposedTargets.add(hit.key);
        void readerRequest("reader_expose", { base: form.form, reading: form.reading || "" }).catch(() => undefined);
      }
    }
  } catch (error) {
    queryError.value = error instanceof Error ? error.message : String(error);
  } finally {
    queryBusy.value = false;
  }
}

function selectChild(child: LookupTarget) {
  if (!outerHit.value) return;
  void queryTarget({
    ...outerHit.value,
    key: `${outerHit.value.key}:${child.id}`,
    target: child,
    range: child.char_range,
    children: [],
  });
}

async function refreshWord(baseForm: string, reading: string) {
  try {
    wordState.value = await readerRequest<{ known: boolean; exposures: number }>("reader_word", { base: baseForm, reading });
  } catch {
    wordState.value = null;
  }
}

async function markWord(known: boolean) {
  const form = activeHit.value?.target.lookup_forms[0];
  if (!form) return;
  try {
    wordState.value = await readerRequest<{ known: boolean; exposures: number }>("reader_mark", { base: form.form, reading: form.reading || "", known });
  } catch (error) {
    queryError.value = error instanceof Error ? error.message : String(error);
  }
}

function handleScroll() {
  const element = scrollElement.value;
  if (!element) return;
  const max = Math.max(1, element.scrollHeight - element.clientHeight);
  currentOffset.value = Math.round((element.scrollTop / max) * Array.from(props.readerDocument.analysisText).length);
  clearTimeout(progressTimer);
  progressTimer = setTimeout(() => {
    const now = Date.now();
    const seconds = Math.max(0, Math.floor((now - lastProgressAt) / 1000));
    lastProgressAt = now;
    emit("progress", currentOffset.value, currentChapter.value, seconds);
  }, 700);
}

function navigate(chapter: { charOffset: number }) {
  const element = scrollElement.value;
  if (!element) return;
  const target = element.querySelector<HTMLElement>(`[data-char-start="${chapter.charOffset}"]`);
  if (target) target.scrollIntoView({ behavior: "smooth", block: "start" });
  showNavigation.value = false;
}

async function restoreOffset() {
  await nextTick();
  const element = scrollElement.value;
  if (!element || !props.initialOffset) return;
  element.scrollTop = (props.initialOffset / Math.max(1, Array.from(props.readerDocument.analysisText).length)) * Math.max(0, element.scrollHeight - element.clientHeight);
  currentOffset.value = props.initialOffset;
}

watch(() => [props.session.session_id, props.session.revision, props.session.generation], ([sessionId, , generation], previous) => {
  if (!previous || sessionId !== previous[0] || generation !== previous[2]) groups.value = new Map();
  void syncTargets();
}, { immediate: true });
watch(() => [props.readerDocument.analysisText, props.appearance.fontSize, props.appearance.lineHeight, props.appearance.contentWidth], () => {
  virtualizer.value.measure();
  void restoreOffset();
});
onMounted(() => { lastProgressAt = Date.now(); void restoreOffset(); });
onBeforeUnmount(() => { clearTimeout(hoverTimer); clearTimeout(progressTimer); });
</script>

<template>
  <section class="reader-view">
    <header class="reader-view__toolbar">
      <button class="reader-view__back" type="button" title="返回书架" @click="emit('back')"><ArrowLeft :size="18" aria-hidden="true" /><span>书架</span></button>
      <div class="reader-view__title"><BookOpen :size="17" aria-hidden="true" /><strong>{{ book?.title || '文本阅读' }}</strong><span>{{ statusLabel }}</span></div>
      <div class="reader-view__actions">
        <button type="button" title="章节" aria-label="章节" @click="showNavigation = true"><ChevronDown :size="17" aria-hidden="true" /></button>
        <button type="button" title="阅读排版" aria-label="阅读排版" @click="showAppearance = true"><Settings2 :size="17" aria-hidden="true" /></button>
        <button v-if="session.paused" type="button" title="继续分析" aria-label="继续分析" @click="emit('continue')"><LoaderCircle :size="17" aria-hidden="true" /></button>
        <button v-if="session.progress.failed" type="button" title="重试失败单元" aria-label="重试失败单元" @click="emit('retry')"><RotateCcw :size="17" aria-hidden="true" /></button>
      </div>
    </header>

    <div ref="scrollElement" class="reader-view__scroll" @scroll.passive="handleScroll" @mouseup="captureSelection">
      <main class="reader-view__content" :style="{ maxWidth: `${appearance.contentWidth}px`, fontSize: `${appearance.fontSize}px`, lineHeight: appearance.lineHeight, '--reader-paragraph-gap': `${appearance.paragraphGap}px` }">
        <div class="reader-row-layer" :style="{ height: `${virtualizer.getTotalSize()}px` }">
          <section v-for="visible in visibleRows" :key="visible.row.key" :ref="measureRow" class="reader-row" :data-index="visible.item.index" :style="{ transform: `translateY(${visible.item.start}px)` }">
            <ReaderImageBlock v-if="visible.row.kind === 'image'" :items="visible.row.items" :layout="visible.row.layout" class="reader-view__image" />
            <section v-else class="reader-view__paragraph" :class="{ 'reader-view__heading': visible.row.heading }" :data-char-start="visible.row.paragraph.charRange[0]">
              <h2 v-if="visible.row.heading">{{ visible.row.heading.title }}</h2>
              <p>
                <span
                  v-for="part in partsFor(visible.row)"
                  :key="part.key"
                  :class="{ 'reader-hit': part.hit, 'reader-hit--active': part.hit?.key === activeHit?.key, 'reader-hit--selected': isSelected(part.range) }"
                  :data-char-start="part.range[0]"
                  :data-char-end="part.range[1]"
                  @mouseenter="part.hit && scheduleLookup(part.hit)"
                  @click="part.hit && openChildren(part.hit)"
                >{{ part.text }}</span>
              </p>
            </section>
          </section>
        </div>
      </main>

      <aside v-if="activeHit" class="reader-lookup" aria-label="词典查询">
        <header class="reader-lookup__header">
          <div><strong>{{ activeHit.target.surface }}</strong><span v-if="activeHit.target.reading_evidence.length">{{ activeHit.target.reading_evidence.join('、') }}</span></div>
          <button type="button" aria-label="关闭查询" @click="activeHit = null; outerHit = null; activeQuery = null; innerOuterId = null">×</button>
        </header>
        <div class="reader-lookup__chain">
          <span v-for="form in activeHit.target.lookup_forms" :key="`${form.form}:${form.reading}`">{{ form.form }}<small v-if="form.reading">・{{ form.reading }}</small></span>
        </div>
        <div v-if="activeChildren.length" class="reader-lookup__children">
          <button type="button" @click="innerOuterId = null; void queryTarget(activeHit!)">外围整体</button>
          <button v-for="child in activeChildren" :key="child.id" type="button" @click="selectChild(child)">{{ child.surface }}</button>
        </div>
        <div class="reader-lookup__state" v-if="wordState">
          <span>{{ wordState.known ? '已知' : '未标记' }} · 曝光 {{ wordState.exposures }}</span>
          <button v-if="!wordState.known" type="button" @click="void markWord(true)">标为已知</button>
          <button v-else type="button" @click="void markWord(false)">标为未掌握</button>
        </div>
        <p v-if="queryBusy" class="reader-lookup__message">正在查询…</p>
        <p v-else-if="queryError" class="reader-lookup__error">{{ queryError }}</p>
        <template v-else-if="activeQuery">
          <div v-if="queryForms.length" class="reader-lookup__forms">
            <button v-for="form in queryForms" :key="form.display_form" type="button" @click="void queryTarget(activeHit!, form.display_form)">{{ form.display_form }}<small v-if="form.readings?.length">・{{ form.readings.join('、') }}</small></button>
          </div>
          <DictionaryContent v-for="(entry, index) in queryEntries" :key="`${entry.occurrence_id}:${index}`" :entry="entry" />
          <p v-if="!queryEntries.length" class="reader-lookup__message">没有匹配的词典条目</p>
        </template>
      </aside>
    </div>

    <ReaderProgressBar :percent="estimate.percent" :current-chapter="currentChapter" :remaining-label="`剩余 ${estimate.remainingCharacters} 字符`" :completion-label="estimate.completionLabel" />
    <div v-if="pendingSelection" class="reader-selection-action">
      <span>已选择 {{ pendingSelection.surface.length }} 字符</span>
      <button type="button" @click="savePendingSelection">保存选择</button>
    </div>
    <button class="reader-export-button" type="button" title="选择与导出" aria-label="选择与导出" @click="showExport = true"><Download :size="17" aria-hidden="true" /><span v-if="selections.length">{{ selections.length }}</span></button>
    <ReaderNavigationPanel :show="showNavigation" :chapters="readerDocument.chapters" :current-id="readerDocument.chapters.find((item) => item.title === currentChapter)?.id" @close="showNavigation = false" @navigate="navigate" />
    <ReaderAppearancePanel :show="showAppearance" :appearance="appearance" @close="showAppearance = false" @update="emit('updateAppearance', $event)" />
    <ExportPanel :show="showExport" :selections="selections" @close="showExport = false" @remove="emit('removeSelection', $event)" @clear-all="emit('clearSelections')" @update-note="updateSelection" @export="exportSelections" />
  </section>
</template>

<style scoped>
.reader-view { position: relative; display: flex; min-height: 0; flex: 1; flex-direction: column; background: var(--bg-primary); }
.reader-view__toolbar { display: flex; min-height: 54px; align-items: center; justify-content: space-between; gap: 12px; padding: 8px 18px; border-bottom: 1px solid var(--border-color); background: var(--glass-bg); backdrop-filter: var(--glass-filter); }
.reader-view__back, .reader-view__actions button { display: inline-flex; align-items: center; gap: 6px; border: 0; border-radius: var(--radius-sm); background: transparent; color: var(--text-secondary); cursor: pointer; }
.reader-view__back { padding: 7px 9px; }
.reader-view__back:hover, .reader-view__actions button:hover { background: var(--accent-light); color: var(--accent-color); }
.reader-view__title { display: flex; min-width: 0; align-items: center; gap: 8px; color: var(--text-secondary); }
.reader-view__title strong { max-width: min(56vw, 540px); overflow: hidden; color: var(--text-primary); font-size: .9rem; font-weight: 600; text-overflow: ellipsis; white-space: nowrap; }
.reader-view__title span { color: var(--text-muted); font-size: .72rem; }
.reader-view__actions { display: flex; gap: 4px; }
.reader-view__actions button { width: 32px; height: 32px; justify-content: center; }
.reader-view__scroll { min-height: 0; flex: 1; overflow: auto; padding: 42px 24px 92px; }
.reader-view__content { margin: 0 auto; color: var(--text-primary); font-family: var(--font-ja); }
.reader-row-layer { position: relative; width: 100%; }
.reader-row { position: absolute; top: 0; left: 0; width: 100%; }
.reader-view__paragraph { margin: 0 0 var(--reader-paragraph-gap, 20px); }
.reader-view__paragraph p { white-space: pre-wrap; overflow-wrap: anywhere; }
.reader-view__heading { margin-top: 34px; }
.reader-view__heading h2 { margin-bottom: 18px; color: var(--accent-color); font-size: 1.25em; font-weight: 600; }
.reader-view__image { max-width: 100%; margin: 24px auto; }
.reader-hit { border-radius: 3px; cursor: pointer; transition: background-color 120ms ease, box-shadow 120ms ease; }
.reader-hit:hover, .reader-hit--active, .reader-hit--selected { background: var(--accent-light); box-shadow: inset 0 -2px var(--accent-color); }
.reader-selection-action { position: fixed; z-index: 42; bottom: 56px; left: 50%; display: flex; align-items: center; gap: 10px; padding: 7px 10px 7px 13px; border: 1px solid var(--border-color); border-radius: 999px; background: var(--bg-primary); box-shadow: var(--shadow-sm); transform: translateX(-50%); color: var(--text-secondary); font-size: .75rem; }
.reader-selection-action button { padding: 5px 9px; border: 0; border-radius: 999px; background: var(--accent-color); color: #fff; cursor: pointer; font-size: .74rem; }
.reader-export-button { position: fixed; z-index: 41; right: 18px; bottom: 55px; display: flex; width: 34px; height: 34px; align-items: center; justify-content: center; gap: 2px; border: 1px solid var(--border-color); border-radius: 50%; background: var(--bg-primary); color: var(--text-secondary); cursor: pointer; box-shadow: var(--shadow-sm); }
.reader-export-button:hover { color: var(--accent-color); }
.reader-export-button span { font-size: .64rem; }
.reader-lookup { position: fixed; z-index: 40; right: 18px; bottom: 48px; width: min(430px, calc(100vw - 36px)); max-height: min(70vh, 640px); overflow: auto; padding: 14px 16px 18px; border: 1px solid var(--border-color); border-radius: var(--radius-md); background: var(--bg-primary); box-shadow: var(--shadow-md); }
.reader-lookup__header { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; }
.reader-lookup__header div { display: flex; align-items: baseline; gap: 10px; }
.reader-lookup__header strong { font-family: var(--font-ja); font-size: 1.15rem; }
.reader-lookup__header span, .reader-lookup__chain small { color: var(--text-muted); font-size: .76rem; }
.reader-lookup__header button { border: 0; background: transparent; color: var(--text-muted); cursor: pointer; font-size: 1.35rem; line-height: 1; }
.reader-lookup__chain { display: flex; flex-wrap: wrap; gap: 6px 10px; margin: 8px 0 12px; color: var(--text-secondary); font-family: var(--font-ja); font-size: .78rem; }
.reader-lookup__children, .reader-lookup__forms, .reader-lookup__state { display: flex; flex-wrap: wrap; gap: 5px; margin: 8px 0; }
.reader-lookup__children button, .reader-lookup__forms button, .reader-lookup__state button { padding: 4px 8px; border: 1px solid var(--border-color); border-radius: 4px; background: transparent; color: var(--text-secondary); cursor: pointer; font-size: .75rem; }
.reader-lookup__children button:hover, .reader-lookup__forms button:hover, .reader-lookup__state button:hover { border-color: var(--accent-color); color: var(--accent-color); }
.reader-lookup__state { align-items: center; justify-content: space-between; color: var(--text-muted); font-size: .72rem; }
.reader-lookup__message, .reader-lookup__error { padding: 14px 0; color: var(--text-muted); font-size: .8rem; }
.reader-lookup__error { color: var(--novelty-high-text); }
@media (max-width: 700px) { .reader-view__toolbar { padding-inline: 10px; } .reader-view__back span { display: none; } .reader-view__title strong { max-width: 38vw; } .reader-view__scroll { padding-inline: 16px; } .reader-lookup { right: 10px; bottom: 46px; width: calc(100vw - 20px); } }
</style>
