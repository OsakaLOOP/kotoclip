<script setup lang="ts">
import { convertFileSrc } from "@tauri-apps/api/core";
import { useVirtualizer, type Virtualizer } from "@tanstack/vue-virtual";
import { BriefcaseBusiness, ListTree, LoaderCircle, Moon, RotateCcw, Settings2, Type } from "@lucide/vue";
import { computed, nextTick, onBeforeUnmount, onMounted, ref, shallowRef, watch } from "vue";
import TooltipPanel from "../TooltipPanel.vue";
import DictionarySettingsPanel from "../dictionary/DictionarySettingsPanel.vue";
import AppHeader from "../common/AppHeader.vue";
import ExportPanel from "../ExportPanel.vue";
import ReaderAppearancePanel from "./ReaderAppearancePanel.vue";
import ReaderExplanationBubble from "./ReaderExplanationBubble.vue";
import ReaderImageBlock from "./ReaderImageBlock.vue";
import ReaderNavigationPanel from "./ReaderNavigationPanel.vue";
import ReaderProgressBar from "./ReaderProgressBar.vue";
import { isTauri } from "@tauri-apps/api/core";
import { nlpRequest } from "../../services/nlp";
import { readerRequest } from "../../services/reader";
import { isSessionGenerationError, matchesSessionGeneration, type AnalysisUnit, type DocumentSession, type UnitUpdate } from "../../reader/session";
import { buildReaderRows, rowCharacterOffset, rowIndexForOffset, type ReaderRow, type ReaderTextRow } from "../../reader/rows";
import { readingEstimate, type ReaderAppearance } from "../../reader/reading";
import { estimateReaderRow, resolveReaderRowMeasurement } from "../../reader/virtualization";
import { dictionaryLookupFromSearch, readerCapsuleRanges, readerChains, readerMorphologyHits, type ReaderMorphologyDetail, type ReaderMorphologyHit } from "../../reader/lookupPresentation";
import { explanationPanelWidth, placeExplanationPanels, snapshotRect } from "../../explanation/geometry";
import { EXPLANATION_CLOSE_GRACE_MS } from "../../explanation/closeGrace";
import { resourceKey, type LibraryBook, type LibraryResource } from "../../reader/library";
import type { ReaderDocument, ReaderTextBlock } from "../../reader/document";
import type { DictionaryLookup, DictionarySettings } from "../../types";
import type { QueryOutput } from "../../types/nlp";
import type { SavedSelection } from "../../types/reader";
import "../../styles/eink.css";

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
  sessionId: string;
  generation: number;
  unit: UnitUpdate;
  contextOffset: number;
  group: LookupGroup;
  range: [number, number];
  capsules: { key: string; range: [number, number] }[];
  morphology: ReaderMorphologyHit[];
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
  annotation?: { char_range: [number, number]; reading: string };
  range: [number, number];
  hit: TargetHit | null;
  detail: ReaderMorphologyDetail | null;
  capsuleKey: string | null;
}

interface TextCapsule {
  key: string;
  capsuleKey: string | null;
  parts: TextPart[];
}

interface TextRun {
  key: string;
  reading?: string;
  capsules: TextCapsule[];
}

interface MergeDrag {
  pointerId: number;
  source: HTMLElement;
  block: ReaderTextBlock;
  start: TextPart;
  end: TextPart;
  x: number;
  y: number;
  moved: boolean;
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
  range: [range: [number, number]];
  retry: [unitId?: string];
  sync: [];
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
const showDictionarySettings = ref(false);
const einkMode = ref(false);
const groups = shallowRef(new Map<string, UnitLookup>());
const pendingTargets = new Set<string>();
let requestedRange = "";
let rangeTimer: ReturnType<typeof setTimeout> | undefined;
const targetErrors = shallowRef(new Map<string, string>());
const targetError = computed(() => [...targetErrors.value.values()][0] ?? "");
const activeHit = shallowRef<TargetHit | null>(null);
const componentHit = shallowRef<TargetHit | null>(null);
const componentQuery = shallowRef<DictionaryLookup | null>(null);
const componentBusy = ref(false);
const componentError = ref("");
const componentWord = ref("");
const componentSearch = shallowRef<QueryOutput | null>(null);
const componentHistory = ref<{ query: DictionaryLookup; word: string; search: QueryOutput | null }[]>([]);
const activeDetail = ref<ReaderMorphologyDetail | null>(null);
const activeQuery = shallowRef<DictionaryLookup | null>(null);
const queryBusy = ref(false);
const queryError = ref("");
const lookupPosition = ref({ x: 12, y: 72, width: 420, maxHeight: 480 });
const detailPosition = ref({ x: 12, y: 12, width: 310, maxHeight: 240 });
const componentPosition = ref({ x: 12, y: 12, width: 420, maxHeight: 480 });
const relatedWord = ref("");
const relatedSearch = shallowRef<QueryOutput | null>(null);
const dictionaryNames = ref<string[]>([]);
const dictionaryOrder = ref<string[]>([]);
const dictionarySettings = computed<DictionarySettings>(() => ({
  available_dictionaries: dictionaryNames.value,
  dictionary_order: [...dictionaryOrder.value.filter((name) => dictionaryNames.value.includes(name)), ...dictionaryNames.value.filter((name) => !dictionaryOrder.value.includes(name))],
  default_dictionary: dictionaryOrder.value[0] ?? dictionaryNames.value[0] ?? null,
}));
const lookupHistory = ref<{ query: DictionaryLookup; word: string; search: QueryOutput | null }[]>([]);
let queryGeneration = 0;
let componentGeneration = 0;
let disposed = false;
const wordState = ref<{ known: boolean; exposures: number } | null>(null);
const pendingSelection = ref<SelectionDraft | null>(null);
const mergeDrag = shallowRef<MergeDrag | null>(null);
let suppressPartClick = false;
let mergedPointer: { x: number; y: number } | null = null;
const exposedTargets = new Set<string>();
let hoverTimer: ReturnType<typeof setTimeout> | undefined;
let closeTimer: ReturnType<typeof setTimeout> | undefined;
let activeCapsule: HTMLElement | null = null;
let pendingCapsule: HTMLElement | null = null;
let progressTimer: ReturnType<typeof setTimeout> | undefined;
let lastProgressAt = Date.now();

const rows = computed<ReaderRow[]>(() => buildReaderRows(
  props.readerDocument.blocks.filter((block): block is ReaderTextBlock => block.kind !== "image"),
  props.readerDocument,
  resolveImage,
  true,
));
const prepared = computed(() => props.session.plan.prepared);
const preparedCharacters = computed(() => Array.from(prepared.value.text));
const rowElements = new Map<string, HTMLElement>();

function estimateRow(index: number): number {
  const row = rows.value[index];
  const images = row?.kind === "image" ? row.items : [];
  const imageWidth = row?.kind === "image" && row.layout === "pair"
    ? images.reduce((total, item) => total + (item.intrinsicWidth ?? 0), 0)
    : images[0]?.intrinsicWidth;
  return estimateReaderRow({
    kind: row?.kind ?? "text",
    heading: row?.kind === "text" && Boolean(row.heading),
    viewportHeight: scrollElement.value?.clientHeight ?? window.innerHeight,
    fontSize: props.appearance.fontSize,
    lineHeight: props.appearance.lineHeight,
    contentWidth: Math.min(props.appearance.contentWidth, Math.max(0, (scrollElement.value?.clientWidth ?? window.innerWidth) - (window.innerWidth <= 700 ? 32 : 48))),
    text: row?.kind === "text" ? row.paragraph.text : undefined,
    paragraphGap: props.appearance.paragraphGap,
    imageWidth,
    imageHeight: images.reduce((height, item) => Math.max(height, item.intrinsicHeight ?? 0), 0) || undefined,
    imageLayout: row?.kind === "image" ? row.layout : undefined,
    hasCaption: row?.kind === "image" && row.layout !== "symbols" && images.some((item) => Boolean(item.image.title || item.image.alt)),
  });
}

function measureReaderRow(element: HTMLElement, entry: ResizeObserverEntry | undefined, instance: Virtualizer<HTMLElement, HTMLElement>): number {
  const index = Number(element.dataset.index);
  const row = rows.value[index];
  const observedSize = entry?.borderBoxSize?.[0]?.blockSize;
  return resolveReaderRowMeasurement({
    kind: row?.kind ?? "text",
    imageState: element.querySelector<HTMLElement>("[data-image-state]")?.dataset.imageState,
    cachedSize: row ? instance.itemSizeCache.get(row.key) : undefined,
    estimatedSize: row?.kind === "image" ? estimateRow(index) : 0,
    observedSize,
    elementSize: observedSize ?? element.getBoundingClientRect().height,
  });
}

const virtualizer = useVirtualizer<HTMLElement, HTMLElement>(computed(() => ({
  count: rows.value.length,
  getScrollElement: () => scrollElement.value,
  getItemKey: (index: number) => rows.value[index]?.key ?? index,
  estimateSize: estimateRow,
  measureElement: measureReaderRow,
  scrollMargin: 42,
  overscan: 6,
})));
virtualizer.value.shouldAdjustScrollPositionOnItemSizeChange = (item, _delta, instance) =>
  item.end <= (instance.scrollOffset ?? 0);
const visibleRows = computed(() => virtualizer.value.getVirtualItems().map((item) => ({ item, row: rows.value[item.index] })).filter((value) => value.row));
const visibleRange = computed<[number, number]>(() => {
  const ranges = visibleRows.value
    .map(({ row }) => row.kind === "text" ? row.paragraph.charRange : null)
    .filter((range): range is [number, number] => Boolean(range));
  if (ranges.length === 0) return [0, 1536];
  return [Math.max(0, Math.min(...ranges.map((range) => range[0])) - 768), Math.max(...ranges.map((range) => range[1])) + 768];
});
const currentOffset = ref(props.initialOffset);
const estimate = computed(() => readingEstimate(currentOffset.value, preparedCharacters.value.length));
const currentChapter = computed(() => {
  const chapters = props.readerDocument.chapters.filter((chapter) => chapter.charOffset <= currentOffset.value);
  return chapters[chapters.length - 1]?.title || "正文";
});
const readableCount = computed(() => props.session.progress.complete);
const statusLabel = computed(() => {
  if (props.session.progress.failed) return "部分分析失败";
  if (props.session.paused) return "分析已暂停";
  if (props.session.progress.complete === props.session.progress.total && props.session.progress.pending > 0) return "恢复阅读内容";
  if (props.session.progress.complete >= props.session.progress.total && props.session.progress.total > 0) return "分析完成";
  return `已分析 ${readableCount.value}/${props.session.progress.total}`;
});
const chainSummaries = computed(() => activeHit.value
  ? readerChains(activeHit.value.target, activeHit.value.unit.unit.document)
  : []);
const chainOverview = computed(() => {
  const chains = chainSummaries.value;
  const primary = chains.find((chain) => chain.role === "词汇核心") ?? chains[0];
  if (!primary) return null;
  const names = chains.flatMap((chain) => chain.formName.split(" · ").filter(Boolean));
  const specific = names.filter((name) => name !== "接续");
  return {
    name: [...new Set(specific.length ? specific : names)].join(" · ") || primary.conjugation || primary.role,
    conjugation: primary.conjugation,
    surface: chains.map((chain) => chain.surface).join(""),
    lemma: primary.lemma,
    members: chains.flatMap((chain) => chain.members.map((member) => member.surface)),
  };
});
function createUnitLookup(unitPlan: AnalysisUnit, unit: UnitUpdate, group: LookupGroup): UnitLookup {
  const offset = unitPlan.context_range[0];
  const [start, end] = localLookupRange(unitPlan, unit.document!);
  const globalRange = (range: [number, number]): [number, number] => [range[0] + offset, range[1] + offset];
  return {
    sessionId: props.session.session_id, generation: props.session.generation,
    unit, group, contextOffset: offset, range: unitPlan.anchor.char_range,
    capsules: [
      ...group.outer_targets.filter((target) => target.decision === "accepted" && target.morpheme_ids.length > 1)
        .map((target) => ({ key: `${unit.unit_id}:${target.id}`, range: globalRange(target.char_range) })),
      ...readerCapsuleRanges(unit.document!)
        .filter((capsule) => capsule.range[0] >= start && capsule.range[1] <= end)
        .map((capsule) => ({ key: `${unit.unit_id}:${capsule.id}`, range: globalRange(capsule.range) })),
    ],
    morphology: readerMorphologyHits(
      [...group.inner_targets.filter((target) => target.decision === "queryable"), ...group.outer_targets], unit.document,
    ).filter((hit) => hit.range[0] >= start && hit.range[1] <= end)
      .map((hit) => ({ ...hit, range: globalRange(hit.range) })),
  };
}

function resolveImage(source: string) {
  const resource = props.resources.find((item) => resourceKey(item.href) === resourceKey(source));
  if (!resource) return undefined;
  return { src: isTauri() ? convertFileSrc(resource.path) : resource.path, width: resource.width, height: resource.height };
}

function measureRow(node: unknown, key: string) {
  if (node instanceof HTMLElement) {
    if (rowElements.get(key) === node) return;
    rowElements.set(key, node);
    virtualizer.value.measureElement(node);
  } else {
    const previous = rowElements.get(key);
    if (previous && !previous.isConnected) rowElements.delete(key);
  }
}

async function measureSettledImage(key: string) {
  await nextTick();
  const element = rowElements.get(key);
  if (!element?.isConnected) return;
  const index = Number(element.dataset.index);
  if (rows.value[index]?.key === key) virtualizer.value.resizeItem(index, measureReaderRow(element, undefined, virtualizer.value));
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
  if (!unit.document || unit.stage !== "complete") return;
  const sessionId = props.session.session_id;
  const generation = props.session.generation;
  const key = `${sessionId}:${generation}:${unit.unit_id}:${unit.artifact_revision}`;
  if (groups.value.has(key) || pendingTargets.has(key) || targetErrors.value.has(key)) return;
  if (unit.lookup) {
    groups.value = new Map(groups.value).set(key, createUnitLookup(unitPlan, unit, unit.lookup as LookupGroup));
    return;
  }
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
    if (disposed || !matchesSessionGeneration(props.session, sessionId, generation)
      || props.session.units[unit.unit_id]?.artifact_revision !== unit.artifact_revision
      || !props.session.units[unit.unit_id]?.document) return;
    groups.value = new Map(groups.value).set(key, createUnitLookup(unitPlan, unit, group));
    if (targetErrors.value.has(key)) {
      const errors = new Map(targetErrors.value);
      errors.delete(key);
      targetErrors.value = errors;
    }
  } catch (error) {
    if (!disposed && matchesSessionGeneration(props.session, sessionId, generation)
      && props.session.units[unit.unit_id]?.artifact_revision === unit.artifact_revision) {
      if (isSessionGenerationError(error)) { emit("sync"); return; }
      targetErrors.value = new Map(targetErrors.value).set(key, error instanceof Error ? error.message : String(error));
    }
  } finally {
    pendingTargets.delete(key);
  }
}

async function syncTargets(retry = false) {
  if (retry) targetErrors.value = new Map();
  if (targetErrors.value.size) {
    const validKeys = new Set(Object.values(props.session.units).filter((unit) => unit.stage === "complete" && unit.document)
      .map((unit) => `${props.session.session_id}:${props.session.generation}:${unit.unit_id}:${unit.artifact_revision}`));
    const errors = new Map([...targetErrors.value].filter(([key]) => validKeys.has(key)));
    if (errors.size !== targetErrors.value.size) targetErrors.value = errors;
  }
  const retained = new Map([...groups.value].filter(([, lookup]) => {
    const current = documentUnit(lookup.unit.unit_id);
    return current?.document && current.stage === "complete" && current.artifact_revision === lookup.unit.artifact_revision;
  }));
  if (retained.size !== groups.value.size) groups.value = retained;
  const tasks = props.session.plan.units
    .map((unitPlan) => [unitPlan, documentUnit(unitPlan.id)] as const)
    .filter((item): item is [typeof props.session.plan.units[number], UnitUpdate] => Boolean(item[1])
      && item[0].anchor.char_range[1] > visibleRange.value[0]
      && item[0].anchor.char_range[0] < visibleRange.value[1])
    .map(([unitPlan, unit]) => loadTargets(unitPlan, unit));
  await Promise.all(tasks);
}

function requestVisibleUnits() {
  clearTimeout(rangeTimer);
  if (!visibleRows.value.length || props.session.paused) return;
  const visible = props.session.plan.units.filter((unit) =>
    unit.anchor.char_range[1] > visibleRange.value[0] && unit.anchor.char_range[0] < visibleRange.value[1]);
  const key = `${props.session.session_id}:${visible[0]?.id}:${visible[visible.length - 1]?.id}`;
  if (key === requestedRange || !visible.length) return;
  rangeTimer = setTimeout(() => {
    requestedRange = key;
    emit("range", [visible[0].anchor.char_range[0], visible[visible.length - 1].anchor.char_range[1]]);
  }, 120);
}

function unitLookups(): UnitLookup[] {
  return [...groups.value.values()].filter(lookupIsCurrent);
}

function lookupIsCurrent(lookup: UnitLookup): boolean {
  const current = props.session.units[lookup.unit.unit_id];
  return !disposed && matchesSessionGeneration(props.session, lookup.sessionId, lookup.generation)
    && current?.stage === "complete" && current.artifact_revision === lookup.unit.artifact_revision && Boolean(current.document);
}

function hitsForBlock(block: ReaderTextBlock, lookups = unitLookups()): TargetHit[] {
  const hits: TargetHit[] = [];
  for (const lookup of lookups) {
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
    for (const grammar of lookup.group.grammar_targets) {
      const range: [number, number] = [
        grammar.char_range[0] + lookup.contextOffset,
        grammar.char_range[1] + lookup.contextOffset,
      ];
      if (range[1] <= block.charRange[0] || range[0] >= block.charRange[1]) continue;
      const target: LookupTarget = {
        id: `grammar:${lookup.unit.unit_id}:${grammar.char_range[0]}:${grammar.char_range[1]}`,
        parent_outer_id: null,
        char_range: grammar.char_range,
        surface: grammar.display_form,
        morpheme_ids: [],
        lexical_core_ids: [],
        source_formation_ids: [],
        lookup_forms: [],
        reading_evidence: [],
        decision: "grammar",
        reason: grammar.normalized_form,
      };
      hits.push({ key: `${lookup.unit.unit_id}:${target.id}`, unit: lookup, target, range, children: [] });
    }
  }
  return hits.sort((left, right) => left.range[0] - right.range[0] || right.range[1] - left.range[1]);
}

function partsFor(row: ReaderTextRow, lookups: UnitLookup[]): TextPart[] {
  const block = row.paragraph;
  const outerHits = hitsForBlock(block, lookups);
  const hits = outerHits;
  const details = lookups.flatMap((lookup) => lookup.morphology.map((hit) => ({ ...hit, lookup })))
    .filter((item) => item.range[0] < block.charRange[1] && item.range[1] > block.charRange[0])
    .map((item) => ({ ...item, hit: hits.find((hit) => hit.unit === item.lookup && hit.target.id === item.targetId)
      ?? hits.find((hit) => hit.unit === item.lookup && hit.target.decision === "grammar"
        && hit.range[0] < item.range[1] && hit.range[1] > item.range[0])
      ?? null }))
    .filter((item) => item.hit)
    .sort((left, right) => (left.range[1] - left.range[0]) - (right.range[1] - right.range[0]));
  const boundaries = new Set<number>([block.charRange[0], block.charRange[1]]);
  const capsules = lookups.flatMap((lookup) => lookup.capsules).filter((capsule) =>
    capsule.range[0] < block.charRange[1] && capsule.range[1] > block.charRange[0]);
  const componentRanges = outerHits.flatMap((outer) => outer.children.map((child) => ({ range: child.char_range })));
  for (const hit of [...hits, ...componentRanges, ...details, ...capsules]) {
    if (hit.range[0] >= block.charRange[1] || hit.range[1] <= block.charRange[0]) continue;
    boundaries.add(Math.max(block.charRange[0], hit.range[0]));
    boundaries.add(Math.min(block.charRange[1], hit.range[1]));
  }
  const annotations = prepared.value.annotations.filter((annotation) =>
    annotation.char_range[0] >= block.charRange[0] && annotation.char_range[1] <= block.charRange[1]);
  for (const annotation of annotations) {
    boundaries.add(annotation.char_range[0]);
    boundaries.add(annotation.char_range[1]);
  }
  const points = [...boundaries].sort((left, right) => left - right);
  return points.slice(0, -1).map((start, index) => {
    const end = points[index + 1];
    const detail = details.find((candidate) => candidate.range[0] <= start && end <= candidate.range[1]);
    const candidateHit = detail?.hit ?? hits.find((candidate) => candidate.range[0] <= start && end <= candidate.range[1]) ?? null;
    const hit = candidateHit?.target.decision === "grammar" && !detail ? null : candidateHit;
    const annotation = annotations.find((annotation) => annotation.char_range[0] <= start && end <= annotation.char_range[1]);
    const capsule = capsules.find((candidate) => candidate.range[0] <= start && end <= candidate.range[1]);
    return {
      key: `${block.id}:${start}`, text: preparedCharacters.value.slice(start, end).join(""), annotation,
      range: [start, end] as [number, number], hit, detail: detail?.detail ?? null,
      capsuleKey: capsule?.key ?? null,
    };
  }).filter((part) => part.text.length > 0);
}

function capsulesForParts(parts: TextPart[]): TextCapsule[] {
  const capsules: TextCapsule[] = [];
  for (const part of parts) {
    const previous = capsules[capsules.length - 1];
    if (previous && previous.capsuleKey === part.capsuleKey && previous.parts[previous.parts.length - 1].range[1] === part.range[0]) {
      previous.parts.push(part);
    } else {
      capsules.push({ key: part.key, capsuleKey: part.capsuleKey, parts: [part] });
    }
  }
  return capsules;
}

function runsForParts(parts: TextPart[]): TextRun[] {
  const groups: TextPart[][] = [];
  for (const part of parts) {
    const previous = groups[groups.length - 1];
    if (previous && previous[0].annotation === part.annotation) previous.push(part);
    else groups.push([part]);
  }
  return groups.map((group) => ({
    key: group[0].key, reading: group[0].annotation?.reading, capsules: capsulesForParts(group),
  }));
}

let rowProjectionCache = new Map<ReaderTextRow, {
  lookups: UnitLookup[];
  prepared: typeof prepared.value;
  runs: TextRun[];
}>();
const visibleRuns = computed(() => {
  const lookups = unitLookups();
  const next: typeof rowProjectionCache = new Map();
  const result = new Map<string, TextRun[]>();
  for (const { row } of visibleRows.value) {
    if (row.kind !== "text") continue;
    const relevant = lookups.filter((lookup) => lookup.range[0] < row.paragraph.charRange[1] && lookup.range[1] > row.paragraph.charRange[0]);
    let entry = rowProjectionCache.get(row);
    if (!entry || entry.prepared !== prepared.value
      || entry.lookups.length !== relevant.length || entry.lookups.some((lookup, index) => lookup !== relevant[index])) {
      entry = { lookups: relevant, prepared: prepared.value, runs: runsForParts(partsFor(row, relevant)) };
    }
    next.set(row, entry);
    result.set(row.key, entry.runs);
  }
  rowProjectionCache = next;
  return result;
});

const visibleParts = computed(() => new Map([...visibleRuns.value.values()]
  .flatMap((runs) => runs.flatMap((run) => run.capsules.flatMap((capsule) => capsule.parts)))
  .map((part) => [part.key, part])));
const mergeRange = computed<[number, number] | null>(() => {
  const drag = mergeDrag.value;
  if (!drag?.moved) return null;
  return [
    Math.max(drag.block.charRange[0], Math.min(drag.start.hit!.range[0], drag.start.range[0], drag.end.hit!.range[0], drag.end.range[0])),
    Math.min(drag.block.charRange[1], Math.max(drag.start.hit!.range[1], drag.start.range[1], drag.end.hit!.range[1], drag.end.range[1])),
  ];
});
const highlightedRange = computed(() => mergeRange.value
  ?? (activeHit.value?.target.decision === "selection" ? activeHit.value.range : null));

function startMerge(part: TextPart, block: ReaderTextBlock, event: PointerEvent) {
  suppressPartClick = false;
  if (!part.hit || event.button !== 0 || !event.isPrimary || event.pointerType === "touch" || !lookupIsCurrent(part.hit.unit)) return;
  event.preventDefault();
  closeLookup();
  clearPendingSelection();
  mergedPointer = null;
  const source = event.currentTarget as HTMLElement;
  source.setPointerCapture(event.pointerId);
  mergeDrag.value = { pointerId: event.pointerId, source, block, start: part, end: part, x: event.clientX, y: event.clientY, moved: false };
}

function moveMerge(event: PointerEvent) {
  const drag = mergeDrag.value;
  if (!drag || event.pointerId !== drag.pointerId) return;
  if (!drag.moved && Math.hypot(event.clientX - drag.x, event.clientY - drag.y) < 5) return;
  event.preventDefault();
  const element = document.elementFromPoint(event.clientX, event.clientY)?.closest<HTMLElement>("[data-part-key]");
  const part = element ? visibleParts.value.get(element.dataset.partKey!) : undefined;
  const end = part?.hit && element?.closest<HTMLElement>("[data-block-id]")?.dataset.blockId === drag.block.id
    && lookupIsCurrent(part.hit.unit) ? part : drag.end;
  if (!drag.moved || end.key !== drag.end.key) mergeDrag.value = { ...drag, end, moved: true };
}

function cancelMerge() {
  const drag = mergeDrag.value;
  if (drag?.moved) suppressPartClick = true;
  mergeDrag.value = null;
  if (drag?.source.hasPointerCapture(drag.pointerId)) drag.source.releasePointerCapture(drag.pointerId);
}

function finishMerge(event: PointerEvent) {
  const drag = mergeDrag.value;
  if (!drag || event.pointerId !== drag.pointerId) return;
  const range = mergeRange.value;
  cancelMerge();
  if (!range) return;
  suppressPartClick = true;
  event.preventDefault();
  if (!lookupIsCurrent(drag.start.hit!.unit) || !lookupIsCurrent(drag.end.hit!.unit)) return;
  const source = document.elementFromPoint(event.clientX, event.clientY)?.closest<HTMLElement>("[data-part-key]") ?? drag.source;
  activeCapsule = source.closest<HTMLElement>(".reader-capsule") ?? source;
  positionLookupAt(snapshotRect(source.getBoundingClientRect()));
  mergedPointer = { x: event.clientX, y: event.clientY };
  const hit = drag.start.hit!;
  const target: LookupTarget = {
    ...hit.target, id: `selection:${range[0]}:${range[1]}`, parent_outer_id: null,
    char_range: [range[0] - hit.unit.contextOffset, range[1] - hit.unit.contextOffset],
    surface: preparedCharacters.value.slice(...range).join(""), decision: "selection",
    morpheme_ids: [], lexical_core_ids: [], source_formation_ids: [], lookup_forms: [], reading_evidence: [],
  };
  componentHit.value = null;
  componentQuery.value = null;
  ++componentGeneration;
  void queryTarget({ key: `${hit.unit.unit.unit_id}:${target.id}`, unit: hit.unit, target, range, children: [] });
}

function clickPart(part: TextPart, event: MouseEvent) {
  if (suppressPartClick) { suppressPartClick = false; return; }
  if (part.hit && part.hit.target.decision !== "grammar") openLookup(part.hit, event.currentTarget as HTMLElement, part.range, null);
  else if (part.hit && part.detail) scheduleLookup(part.hit, event, part.detail);
}

function selectionOffset(node: Node, offset: number): number | null {
  const element = node.nodeType === Node.ELEMENT_NODE ? node as Element : node.parentElement;
  const part = element?.closest<HTMLElement>("[data-char-start]");
  if (!part || !scrollElement.value?.contains(part)) return null;
  const start = Number(part.dataset.charStart);
  if (!Number.isFinite(start)) return null;
  const prefix = document.createRange();
  prefix.selectNodeContents(part);
  prefix.setEnd(node, offset);
  const content = prefix.cloneContents();
  content.querySelectorAll("rt, rp").forEach((reading) => reading.remove());
  return start + Array.from(content.textContent ?? "").length;
}

function selectedRange(): [number, number] | null {
  const selection = window.getSelection();
  if (!selection || selection.isCollapsed || !scrollElement.value || !selection.rangeCount) return null;
  const range = selection.getRangeAt(0);
  if (!scrollElement.value.contains(range.commonAncestorContainer)) return null;
  const anchor = selectionOffset(range.startContainer, range.startOffset);
  const focus = selectionOffset(range.endContainer, range.endOffset);
  return anchor === null || focus === null ? null : [Math.min(anchor, focus), Math.max(anchor, focus)];
}

function copySelection(event: ClipboardEvent) {
  const range = selectedRange();
  if (!range || range[0] === range[1] || !event.clipboardData) return;
  event.clipboardData.setData("text/plain", preparedCharacters.value.slice(...range).join(""));
  event.preventDefault();
}

function captureSelection() {
  pendingSelection.value = null;
  const range = selectedRange();
  if (!range) return;
  let [start, end] = range;
  while (start < end && /\s/u.test(preparedCharacters.value[start])) start++;
  while (end > start && /\s/u.test(preparedCharacters.value[end - 1])) end--;
  const surface = preparedCharacters.value.slice(start, end).join("");
  if (!surface) return;
  const block = props.readerDocument.blocks.find((item): item is ReaderTextBlock => item.kind !== "image" && item.charRange[0] <= start && start < item.charRange[1])
    || props.readerDocument.blocks.find((item): item is ReaderTextBlock => item.kind !== "image");
  if (!block) return;
  const hit = hitsForBlock(block);
  const form = hit.find((item) => item.range[0] <= start && start < item.range[1])?.target.lookup_forms[0];
  pendingSelection.value = { start, end, surface, baseForm: form?.form || "", reading: form?.reading || "" };
}

function clearPendingSelection() {
  pendingSelection.value = null;
  window.getSelection()?.removeAllRanges();
}

function savePendingSelection() {
  if (!pendingSelection.value) return;
  emit("saveSelection", pendingSelection.value);
  clearPendingSelection();
}

function updateSelection(selection: SavedSelection, note: string) {
  emit("updateSelection", selection, note);
}

function exportSelections() {
  emit("exportSelections");
}

function positionLookupAt(anchor: ReturnType<typeof snapshotRect>, companion: "component" | "detail" | null = null) {
  const width = explanationPanelWidth(window.innerWidth, Boolean(companion));
  const dictionaryWidth = companion === "component" ? Math.min(365, width) : width;
  const placement = placeExplanationPanels(anchor, anchor,
    { width: companion === "component" ? dictionaryWidth : companion === "detail" ? Math.min(310, width) : width,
      height: companion === "detail" ? 240 : 420 },
    { width: window.innerWidth, height: window.innerHeight },
    companion ? { width: dictionaryWidth, height: 420 } : undefined);
  const dictionary = companion ? placement.whole! : placement.component;
  lookupPosition.value = { x: dictionary.left, y: dictionary.top, width: dictionary.width, maxHeight: dictionary.maxHeight };
  if (companion) {
    const detail = placement.component;
    componentPosition.value = { x: detail.left, y: detail.top, width: detail.width, maxHeight: detail.maxHeight };
    detailPosition.value = componentPosition.value;
  }
}

function orderedLookup(lookup: DictionaryLookup): DictionaryLookup {
  dictionaryNames.value = [...new Set([...dictionaryNames.value, ...lookup.dictionary_names])];
  return {
    ...lookup,
    dictionary_names: [...dictionaryOrder.value.filter((name) => lookup.dictionary_names.includes(name)),
      ...lookup.dictionary_names.filter((name) => !dictionaryOrder.value.includes(name))],
  };
}

function updateDictionaryOrder(order: string[]) {
  dictionaryOrder.value = order;
  localStorage.setItem("kotoclip.reader.dictionaryOrder", JSON.stringify(order));
  if (activeQuery.value) activeQuery.value = orderedLookup(activeQuery.value);
  if (componentQuery.value) componentQuery.value = orderedLookup(componentQuery.value);
}

function componentForHit(hit: TargetHit, range: [number, number]): TargetHit | null {
  const outer = outerForHit(hit);
  if (outer.target.decision === "selection") return null;
  const child = outer.children.filter((target) => target.decision === "queryable"
    && target.char_range[0] <= range[0] && range[1] <= target.char_range[1])
    .sort((left, right) => (left.char_range[1] - left.char_range[0]) - (right.char_range[1] - right.char_range[0]))[0];
  return child ? { key: `${outer.key}:${child.id}`, unit: outer.unit, target: child, range: child.char_range, children: [] } : null;
}

function openLookup(hit: TargetHit, source: HTMLElement, range: [number, number], detail: ReaderMorphologyDetail | null) {
  const outer = outerForHit(hit);
  const component = componentForHit(hit, range);
  activeCapsule = source.closest<HTMLElement>(".reader-capsule") ?? source;
  positionLookupAt(snapshotRect(source.getBoundingClientRect()), component ? "component" : detail ? "detail" : null);
  activeDetail.value = detail;
  if (activeHit.value?.key !== outer.key || relatedWord.value) void queryTarget(outer);
  if (componentHit.value?.key !== component?.key) {
    componentHit.value = component;
    componentQuery.value = null;
    componentError.value = "";
    componentWord.value = "";
    componentSearch.value = null;
    componentHistory.value = [];
    if (component) void queryComponent(component);
    else ++componentGeneration;
  }
}

async function queryComponent(hit: TargetHit, selectedForm?: string) {
  const generation = ++componentGeneration;
  componentBusy.value = true;
  componentError.value = "";
  try {
    const result = await nlpRequest<DictionaryLookup>({
      command: "query_lookup_document",
      session_id: props.session.session_id,
      text_version: props.session.text_version,
      generation: props.session.generation,
      unit_id: hit.unit.unit.unit_id,
      artifact_revision: hit.unit.unit.artifact_revision,
      target_id: hit.target.id,
      selected_form: selectedForm || null,
    });
    if (generation === componentGeneration && lookupIsCurrent(hit.unit)) componentQuery.value = orderedLookup(result);
  } catch (error) {
    if (generation === componentGeneration) componentError.value = error instanceof Error ? error.message : String(error);
  } finally {
    if (generation === componentGeneration) componentBusy.value = false;
  }
}

function scheduleLookup(hit: TargetHit, event: MouseEvent, detail: ReaderMorphologyDetail | null) {
  if (mergeDrag.value || event.buttons !== 0 || !lookupIsCurrent(hit.unit) || (hit.target.decision === "grammar" && !detail) || window.getSelection()?.isCollapsed === false) return;
  if (mergedPointer && Math.hypot(event.clientX - mergedPointer.x, event.clientY - mergedPointer.y) < 5) return;
  mergedPointer = null;
  cancelCloseLookup();
  clearTimeout(hoverTimer);
  pendingCapsule = null;
  const source = event.currentTarget as HTMLElement;
  const capsule = source.closest<HTMLElement>(".reader-capsule") ?? source;
  const anchor = snapshotRect(source.getBoundingClientRect());
  const range: [number, number] = [Number(source.dataset.charStart), Number(source.dataset.charEnd)];
  const outer = outerForHit(hit);
  const component = componentForHit(hit, range);
  if (activeHit.value?.key === outer.key && componentHit.value?.key === component?.key && !relatedWord.value) {
    activeCapsule = capsule;
    activeDetail.value = detail;
    positionLookupAt(anchor, component ? "component" : detail ? "detail" : null);
    return;
  }
  const open = () => {
    if (!capsule.isConnected || !lookupIsCurrent(hit.unit) || window.getSelection()?.isCollapsed === false) return;
    pendingCapsule = null;
    activeCapsule = capsule;
    if (hit.target.decision !== "grammar") openLookup(hit, source, range, detail);
    else {
      ++queryGeneration;
      ++componentGeneration;
      componentHit.value = null;
      wordState.value = null;
      relatedWord.value = "";
      relatedSearch.value = null;
      lookupHistory.value = [];
    }
  };
  pendingCapsule = capsule;
  hoverTimer = setTimeout(open, 60);
}

function scheduleCloseLookup() {
  if (closeTimer !== undefined) return;
  closeTimer = setTimeout(closeLookup, EXPLANATION_CLOSE_GRACE_MS);
}

function cancelCloseLookup() {
  clearTimeout(closeTimer);
  closeTimer = undefined;
}

function insideLookupRegion(target: EventTarget | null): boolean {
  if (!(target instanceof Node)) return false;
  return Boolean(activeCapsule?.contains(target)
    || document.getElementById("reader-dictionary")?.contains(target)
    || document.getElementById("reader-component-dictionary")?.contains(target)
    || document.querySelector(".reader-explanation")?.contains(target));
}

function leaveCapsule(event: PointerEvent) {
  if (pendingCapsule === event.currentTarget) {
    clearTimeout(hoverTimer);
    pendingCapsule = null;
  }
  if (activeHit.value && !insideLookupRegion(event.relatedTarget)) scheduleCloseLookup();
}

function leavePart() {
  clearTimeout(hoverTimer);
  pendingCapsule = null;
}

function outerForHit(hit: TargetHit): TargetHit {
  if (!hit.target.parent_outer_id) return hit;
  const parent = hit.unit.group.outer_targets.find((target) => target.id === hit.target.parent_outer_id);
  if (!parent) return hit;
  return {
    key: `${hit.unit.unit.unit_id}:${parent.id}`, unit: hit.unit, target: parent,
    range: [parent.char_range[0] + hit.unit.contextOffset, parent.char_range[1] + hit.unit.contextOffset],
    children: hit.unit.group.inner_targets.filter((target) => target.parent_outer_id === parent.id)
      .map((target) => ({ ...target, char_range: [target.char_range[0] + hit.unit.contextOffset, target.char_range[1] + hit.unit.contextOffset] as [number, number] })),
  };
}

async function queryTarget(hit: TargetHit, selectedForm?: string) {
  if (!lookupIsCurrent(hit.unit)) return;
  const generation = ++queryGeneration;
  activeHit.value = hit;
  activeQuery.value = null;
  wordState.value = null;
  relatedWord.value = "";
  relatedSearch.value = null;
  lookupHistory.value = [];
  queryBusy.value = true;
  queryError.value = "";
  try {
    if (hit.target.decision === "selection") {
      relatedWord.value = hit.target.surface;
      const result = await nlpRequest<QueryOutput>({ command: "search", word: hit.target.surface });
      if (generation !== queryGeneration || !lookupIsCurrent(hit.unit)) return;
      relatedSearch.value = result;
      activeQuery.value = orderedLookup(dictionaryLookupFromSearch(result, hit.target.surface));
      return;
    }
    const result = await nlpRequest<DictionaryLookup>({
      command: "query_lookup_document",
      session_id: props.session.session_id,
      text_version: props.session.text_version,
      generation: props.session.generation,
      unit_id: hit.unit.unit.unit_id,
      artifact_revision: hit.unit.unit.artifact_revision,
      target_id: hit.target.id,
      selected_form: selectedForm || null,
    });
    if (generation !== queryGeneration || !lookupIsCurrent(hit.unit)) return;
    activeQuery.value = orderedLookup(result);
    const form = hit.target.lookup_forms[0];
    if (form) {
      await refreshWord(form.form, form.reading || "", generation);
      if (generation !== queryGeneration) return;
      if (!exposedTargets.has(hit.key)) {
        exposedTargets.add(hit.key);
        void readerRequest("reader_expose", { base: form.form, reading: form.reading || "" }).catch(() => undefined);
      }
    }
  } catch (error) {
    if (generation === queryGeneration) {
      if (isSessionGenerationError(error)) { closeLookup(); emit("sync"); }
      else queryError.value = error instanceof Error ? error.message : String(error);
    }
  } finally {
    if (generation === queryGeneration) queryBusy.value = false;
  }
}

function closeLookup() {
  ++queryGeneration;
  ++componentGeneration;
  clearTimeout(hoverTimer);
  clearTimeout(closeTimer);
  closeTimer = undefined;
  activeCapsule = null;
  pendingCapsule = null;
  activeHit.value = null;
  activeDetail.value = null;
  componentHit.value = null;
  componentQuery.value = null;
  componentBusy.value = false;
  componentError.value = "";
  componentWord.value = "";
  componentSearch.value = null;
  componentHistory.value = [];
  activeQuery.value = null;
  relatedSearch.value = null;
  relatedWord.value = "";
  lookupHistory.value = [];
  queryBusy.value = false;
  wordState.value = null;
}

function selectLookupForm(formId: string) {
  const form = activeQuery.value?.forms.find((item) => item.form_id === formId);
  if (!form || !activeHit.value) return;
  if (relatedSearch.value) activeQuery.value = orderedLookup(dictionaryLookupFromSearch(relatedSearch.value, relatedWord.value, formId));
  else void queryTarget(activeHit.value, form.display_form);
}

function selectComponentForm(formId: string) {
  const form = componentQuery.value?.forms.find((item) => item.form_id === formId);
  if (!form || !componentHit.value) return;
  if (componentSearch.value) componentQuery.value = orderedLookup(dictionaryLookupFromSearch(componentSearch.value, componentWord.value, formId));
  else void queryComponent(componentHit.value, form.display_form);
}

async function navigateComponent(word: string) {
  if (!componentQuery.value) return;
  const previous = { query: componentQuery.value, word: componentWord.value, search: componentSearch.value };
  const generation = ++componentGeneration;
  componentBusy.value = true;
  componentError.value = "";
  try {
    const result = await nlpRequest<QueryOutput>({ command: "search", word });
    if (generation !== componentGeneration) return;
    componentHistory.value = [...componentHistory.value, previous];
    componentWord.value = word;
    componentSearch.value = result;
    componentQuery.value = orderedLookup(dictionaryLookupFromSearch(result, word));
  } catch (error) {
    if (generation === componentGeneration) componentError.value = error instanceof Error ? error.message : String(error);
  } finally {
    if (generation === componentGeneration) componentBusy.value = false;
  }
}

function backComponent() {
  const previous = componentHistory.value[componentHistory.value.length - 1];
  if (!previous) return;
  ++componentGeneration;
  componentHistory.value = componentHistory.value.slice(0, -1);
  componentQuery.value = orderedLookup(previous.query);
  componentWord.value = previous.word;
  componentSearch.value = previous.search;
  componentError.value = "";
  componentBusy.value = false;
}

async function navigateLookup(word: string) {
  if (!activeQuery.value) return;
  const previous = { query: activeQuery.value, word: relatedWord.value, search: relatedSearch.value };
  const generation = ++queryGeneration;
  queryBusy.value = true;
  queryError.value = "";
  try {
    const result = await nlpRequest<QueryOutput>({ command: "search", word });
    if (generation !== queryGeneration) return;
    lookupHistory.value = [...lookupHistory.value, previous];
    relatedWord.value = word;
    relatedSearch.value = result;
    activeQuery.value = orderedLookup(dictionaryLookupFromSearch(result, word));
  } catch (error) {
    if (generation === queryGeneration) queryError.value = error instanceof Error ? error.message : String(error);
  } finally {
    if (generation === queryGeneration) queryBusy.value = false;
  }
}

function backLookup() {
  const previous = lookupHistory.value[lookupHistory.value.length - 1];
  if (!previous) return;
  ++queryGeneration;
  lookupHistory.value = lookupHistory.value.slice(0, -1);
  activeQuery.value = orderedLookup(previous.query);
  relatedWord.value = previous.word;
  relatedSearch.value = previous.search;
  queryError.value = "";
  queryBusy.value = false;
}

async function refreshWord(baseForm: string, reading: string, generation: number) {
  try {
    const result = await readerRequest<{ known: boolean; exposures: number }>("reader_word", { base: baseForm, reading });
    if (generation === queryGeneration) wordState.value = result;
  } catch {
    if (generation === queryGeneration) wordState.value = null;
  }
}

async function markWord(known: boolean) {
  const generation = queryGeneration;
  const form = activeHit.value?.target.lookup_forms[0];
  if (!form || relatedWord.value) return;
  try {
    const result = await readerRequest<{ known: boolean; exposures: number }>("reader_mark", { base: form.form, reading: form.reading || "", known });
    if (generation === queryGeneration) wordState.value = result;
  } catch (error) {
    if (generation === queryGeneration) queryError.value = error instanceof Error ? error.message : String(error);
  }
}

function handleScroll() {
  const element = scrollElement.value;
  if (!element) return;
  cancelMerge();
  closeLookup();
  const firstVisible = virtualizer.value.getVirtualItems().find((item) => item.end > element.scrollTop);
  if (firstVisible) currentOffset.value = rowCharacterOffset(rows.value[firstVisible.index]);
  clearTimeout(progressTimer);
  progressTimer = setTimeout(() => {
    const now = Date.now();
    const seconds = Math.max(0, Math.floor((now - lastProgressAt) / 1000));
    lastProgressAt = now;
    emit("progress", currentOffset.value, currentChapter.value, seconds);
  }, 700);
}

function navigate(chapter: { charOffset: number }) {
  const headingIndex = rows.value.findIndex((row) => row.kind === "text" && row.heading?.charOffset === chapter.charOffset);
  virtualizer.value.scrollToIndex(headingIndex >= 0 ? headingIndex : rowIndexForOffset(rows.value, chapter.charOffset), { align: "start" });
  currentOffset.value = chapter.charOffset;
  showNavigation.value = false;
}

async function restoreOffset() {
  await nextTick();
  if (!scrollElement.value || !props.initialOffset) return;
  virtualizer.value.scrollToIndex(rowIndexForOffset(rows.value, props.initialOffset), { align: "start" });
  currentOffset.value = props.initialOffset;
}

function dismissLookup(event: PointerEvent) {
  if (mergeDrag.value) return;
  if (!insideLookupRegion(event.target)) closeLookup();
}

function handleEscape(event: KeyboardEvent) {
  if (event.key === "Escape") { cancelMerge(); closeLookup(); }
}

watch(() => props.session, () => {
  const drag = mergeDrag.value;
  if (drag && (!lookupIsCurrent(drag.start.hit!.unit) || !lookupIsCurrent(drag.end.hit!.unit))) cancelMerge();
  if (activeHit.value && !lookupIsCurrent(activeHit.value.unit)) closeLookup();
}, { flush: "sync" });
watch(() => [showNavigation.value, showAppearance.value, showExport.value, showDictionarySettings.value], () => closeLookup());

watch([() => props.session.session_id, () => props.session.revision, () => props.session.generation,
  () => visibleRange.value[0], () => visibleRange.value[1]], ([sessionId, , generation], previous) => {
  if (!previous || sessionId !== previous[0] || generation !== previous[2]) {
    requestedRange = "";
    groups.value = new Map();
    targetErrors.value = new Map();
  }
  void syncTargets();
  requestVisibleUnits();
}, { immediate: true });
watch(() => [props.readerDocument.analysisText, props.appearance.fontSize, props.appearance.lineHeight, props.appearance.contentWidth], () => {
  virtualizer.value.measure();
});
onMounted(() => {
  window.addEventListener("blur", cancelMerge);
  document.addEventListener("pointermove", moveMerge);
  document.addEventListener("pointerup", finishMerge);
  document.addEventListener("pointercancel", cancelMerge);
  document.addEventListener("pointerdown", dismissLookup);
  document.addEventListener("keydown", handleEscape);
  window.addEventListener("resize", closeLookup);
  lastProgressAt = Date.now();
  try {
    const saved = JSON.parse(localStorage.getItem("kotoclip.reader.dictionaryOrder") || "[]");
    if (Array.isArray(saved) && saved.every((name) => typeof name === "string")) dictionaryOrder.value = saved;
  } catch { dictionaryOrder.value = []; }
  void restoreOffset();
});
onBeforeUnmount(() => {
  disposed = true;
  window.removeEventListener("blur", cancelMerge);
  cancelMerge();
  document.removeEventListener("pointermove", moveMerge);
  document.removeEventListener("pointerup", finishMerge);
  document.removeEventListener("pointercancel", cancelMerge);
  closeLookup();
  document.removeEventListener("pointerdown", dismissLookup);
  document.removeEventListener("keydown", handleEscape);
  window.removeEventListener("resize", closeLookup);
  clearTimeout(rangeTimer);
  clearTimeout(hoverTimer);
  clearTimeout(closeTimer);
  clearTimeout(progressTimer);
  document.body.classList.remove("eink-mode");
});

function toggleEinkMode() {
  einkMode.value = !einkMode.value;
  document.body.classList.toggle("eink-mode", einkMode.value);
}
</script>

<template>
  <section class="reader-view">
    <AppHeader show-back collapse-brand back-label="返回书架" :title="book?.title || readerDocument.metadata.title || '文本阅读'" :description="readerDocument.metadata.author || statusLabel" @back="emit('back')">
      <template #actions>
        <div class="reader-view__actions">
          <button type="button" :title="currentChapter" aria-label="章节" @click="showNavigation = true"><ListTree :size="17" aria-hidden="true" /><span class="reader-view__chapter">{{ currentChapter }}</span></button>
          <button type="button" title="阅读排版" aria-label="阅读排版" @click="showAppearance = true"><Type :size="17" aria-hidden="true" /></button>
          <button type="button" title="词典设置" aria-label="词典设置" @click="showDictionarySettings = true"><Settings2 :size="17" aria-hidden="true" /></button>
          <button type="button" :title="`选择与导出（${selections.length}）`" aria-label="选择与导出" @click="showExport = true"><BriefcaseBusiness :size="17" aria-hidden="true" /><span v-if="selections.length" class="reader-view__count">{{ selections.length }}</span></button>
          <button type="button" title="墨水屏模式" aria-label="墨水屏模式" :aria-pressed="einkMode" @click="toggleEinkMode"><Moon :size="17" aria-hidden="true" /></button>
          <button v-if="session.paused" type="button" title="继续分析" aria-label="继续分析" @click="emit('continue')"><LoaderCircle :size="17" aria-hidden="true" /></button>
          <button v-if="session.progress.failed" type="button" title="重试失败单元" aria-label="重试失败单元" @click="emit('retry')"><RotateCcw :size="17" aria-hidden="true" /></button>
        </div>
      </template>
    </AppHeader>

    <div v-if="targetError" class="reader-view__lookup-error" role="alert">
      <span>查词加载失败：{{ targetError }}</span>
      <button type="button" @click="void syncTargets(true)">重试</button>
    </div>

    <div ref="scrollElement" class="reader-view__scroll" @scroll.passive="handleScroll" @mouseup="captureSelection" @keyup="captureSelection" @copy="copySelection">
      <main class="reader-view__content" :style="{ maxWidth: `${appearance.contentWidth}px`, fontSize: `${appearance.fontSize}px`, lineHeight: appearance.lineHeight, '--reader-paragraph-gap': `${appearance.paragraphGap}px` }">
        <div class="reader-row-layer" :style="{ height: `${virtualizer.getTotalSize()}px` }">
          <section v-for="visible in visibleRows" :key="visible.row.key" :ref="(node) => measureRow(node, visible.row.key)" class="reader-row" :data-index="visible.item.index" :style="{ transform: `translateY(${visible.item.start - 42}px)` }">
            <ReaderImageBlock v-if="visible.row.kind === 'image'" :items="visible.row.items" :layout="visible.row.layout" class="reader-view__image" @settled="measureSettledImage(visible.row.key)" />
            <section v-else class="reader-view__paragraph" :class="{ 'reader-view__heading': visible.row.heading }" :data-char-start="visible.row.paragraph.charRange[0]" :data-block-id="visible.row.paragraph.id">
              <component :is="visible.row.heading ? 'h2' : 'p'">
                <component v-for="run in visibleRuns.get(visible.row.key) ?? []" :key="run.key" :is="run.reading ? 'ruby' : 'span'"><span
                  v-for="capsule in run.capsules"
                  :key="capsule.key"
                  class="reader-capsule"
                  :class="{ 'reader-capsule--group': capsule.capsuleKey }"
                  @pointerleave="leaveCapsule"
                ><span
                  v-for="part in capsule.parts"
                  :key="part.key"
                  class="reader-capsule__part"
                  :class="{ 'reader-capsule__part--lookup': part.hit, 'reader-capsule__part--merging': highlightedRange && part.range[0] < highlightedRange[1] && part.range[1] > highlightedRange[0] }"
                  :data-part-key="part.key"
                  :data-char-start="part.range[0]"
                  :data-char-end="part.range[1]"
                  @pointerenter="part.hit ? scheduleLookup(part.hit, $event, part.detail) : scheduleCloseLookup()"
                  @pointerleave="leavePart"
                  @pointerdown="startMerge(part, visible.row.paragraph, $event)"
                  @click="clickPart(part, $event)"
                >{{ part.text }}</span></span><rt v-if="run.reading">{{ run.reading }}</rt></component>
              </component>
            </section>
          </section>
        </div>
      </main>

    </div>

    <TooltipPanel :show="Boolean(activeHit) && activeHit?.target.decision !== 'grammar'" :x="lookupPosition.x" :y="lookupPosition.y" :width="lookupPosition.width" :max-height="lookupPosition.maxHeight" :token="null" :headword="relatedWord || activeHit?.target.surface" :lookup="activeQuery" :loading="queryBusy" :can-go-back="lookupHistory.length > 0" :summary-visible="!relatedWord && Boolean(chainOverview)" :kind-label="componentHit ? '整体' : undefined" panel-id="reader-dictionary" @enter="cancelCloseLookup" @leave="scheduleCloseLookup" @close="closeLookup" @back="backLookup" @select-form="selectLookupForm" @navigate="void navigateLookup($event)">
      <template #summary>
        <div v-if="!relatedWord && chainOverview" class="reader-lookup__summary">
          <strong>{{ chainOverview.name }}</strong>
          <span>{{ chainOverview.surface }}<template v-if="chainOverview.lemma && chainOverview.lemma !== chainOverview.surface"> → {{ chainOverview.lemma }}</template></span>
          <small v-if="chainOverview.conjugation && chainOverview.name !== chainOverview.conjugation">{{ chainOverview.conjugation }}</small>
          <small v-if="chainOverview.members.length > 1">{{ chainOverview.members.join(' + ') }}</small>
        </div>
      </template>
      <template #context>
        <div v-if="activeHit" class="reader-lookup__context">
          <div v-if="wordState && !relatedWord" class="reader-lookup__state">
            <span>{{ wordState.known ? '已知' : '未标记' }} · 曝光 {{ wordState.exposures }}</span>
            <button v-if="!wordState.known" type="button" @click="void markWord(true)">标为已知</button>
            <button v-else type="button" @click="void markWord(false)">标为未掌握</button>
          </div>
          <p v-if="queryError" class="reader-lookup__error">{{ queryError }}</p>
        </div>
      </template>
    </TooltipPanel>
    <TooltipPanel v-if="componentHit" :show="Boolean(activeHit)" :x="componentPosition.x" :y="componentPosition.y" :width="componentPosition.width" :max-height="componentPosition.maxHeight" :token="null" :headword="componentWord || componentHit.target.surface" :lookup="componentQuery" :loading="componentBusy" :can-go-back="componentHistory.length > 0" kind-label="组分" panel-id="reader-component-dictionary" @enter="cancelCloseLookup" @leave="scheduleCloseLookup" @close="closeLookup" @back="backComponent" @select-form="selectComponentForm" @navigate="void navigateComponent($event)">
      <template #context>
        <div v-if="activeDetail && !componentWord" class="reader-lookup__detail">
          <strong>{{ activeDetail.title }}</strong>
          <span v-if="activeDetail.description">{{ activeDetail.description }}</span>
          <span v-if="activeDetail.normalizedForm">{{ activeDetail.normalizedForm }}</span>
        </div>
        <p v-if="componentError" class="reader-lookup__error">{{ componentError }}</p>
      </template>
    </TooltipPanel>
    <ReaderExplanationBubble v-if="activeDetail && !relatedWord && !componentHit" :show="Boolean(activeHit) && Boolean(activeDetail)" :x="detailPosition.x" :y="detailPosition.y" :width="detailPosition.width" :max-height="detailPosition.maxHeight" :title="activeDetail.title" :surface="activeDetail.surface" @enter="cancelCloseLookup" @leave="scheduleCloseLookup">
      <p v-if="activeDetail.description">{{ activeDetail.description }}</p>
      <p v-if="activeDetail.normalizedForm">完整形式：{{ activeDetail.normalizedForm }}</p>
      <p v-if="activeDetail.candidates.length">可能含义：{{ activeDetail.candidates.join('、') }}</p>
    </ReaderExplanationBubble>

    <ReaderProgressBar :percent="estimate.percent" :current-chapter="currentChapter" :remaining-label="`剩余 ${estimate.remainingCharacters} 字符`" :completion-label="estimate.completionLabel" />
    <div v-if="pendingSelection" class="reader-selection-action">
      <span>已选择 {{ pendingSelection.end - pendingSelection.start }} 字符</span>
      <button type="button" class="reader-selection-action__cancel" @click="clearPendingSelection">取消</button>
      <button type="button" @click="savePendingSelection">保存选择</button>
    </div>
    <ReaderNavigationPanel :show="showNavigation" :chapters="readerDocument.chapters" :current-id="readerDocument.chapters.find((item) => item.title === currentChapter)?.id" @close="showNavigation = false" @navigate="navigate" />
    <ReaderAppearancePanel :show="showAppearance" :appearance="appearance" @close="showAppearance = false" @update="emit('updateAppearance', $event)" />
    <DictionarySettingsPanel :show="showDictionarySettings" :settings="dictionarySettings" @close="showDictionarySettings = false" @reorder="updateDictionaryOrder" />
    <ExportPanel :show="showExport" :selections="selections" @close="showExport = false" @remove="emit('removeSelection', $event)" @clear-all="emit('clearSelections')" @update-note="updateSelection" @export="exportSelections" />
  </section>
</template>

<style scoped>
.reader-view { position: relative; display: flex; min-height: 0; flex: 1; flex-direction: column; background: var(--bg-primary); }
.reader-view__actions { display: flex; align-items: center; gap: 4px; }
.reader-view__actions button { position: relative; display: inline-flex; min-width: 32px; height: 32px; align-items: center; justify-content: center; gap: 6px; padding: 0 6px; border: 0; border-radius: var(--radius-sm); background: transparent; color: var(--text-secondary); cursor: pointer; }
.reader-view__actions button:hover, .reader-view__actions button[aria-pressed="true"] { background: var(--accent-light); color: var(--accent-color); }
.reader-view__lookup-error { display: flex; align-items: center; justify-content: center; gap: 10px; padding: 6px 14px; color: var(--novelty-high-text); font-size: .75rem; }
.reader-view__lookup-error span { min-width: 0; overflow-wrap: anywhere; }
.reader-view__lookup-error button { flex: 0 0 auto; border: 0; background: transparent; color: var(--accent-color); cursor: pointer; }
.reader-view__chapter { max-width: 170px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: .75rem; }
.reader-view__count { font-size: .65rem; }
.reader-view__scroll { min-height: 0; flex: 1; overflow: auto; overflow-anchor: none; scrollbar-gutter: stable; padding: 42px 24px 92px; }
.reader-view__content { margin: 0 auto; color: var(--text-primary); font-family: var(--font-ja); }
.reader-row-layer { position: relative; width: 100%; }
.reader-row { position: absolute; top: 0; left: 0; width: 100%; }
.reader-view__paragraph { margin: 0 0 var(--reader-paragraph-gap, 20px); }
.reader-view__paragraph p, .reader-view__paragraph h2 { white-space: pre-wrap; overflow-wrap: anywhere; }
.reader-view__heading { margin-top: 34px; }
.reader-view__heading h2 { margin-bottom: 18px; color: var(--accent-color); font-size: 1.25em; font-weight: 600; }
.reader-view__image { max-width: 100%; margin: 24px auto; }
.reader-capsule { display: inline; }
.reader-capsule--group { border-radius: var(--radius-sm); box-decoration-break: clone; -webkit-box-decoration-break: clone; }
.reader-capsule--group:hover { outline: 1px solid var(--border-color); }
.reader-capsule__part--lookup { cursor: pointer; }
.reader-capsule__part--merging { background: var(--accent-light); text-decoration: underline; text-decoration-color: var(--accent-color); }
.reader-capsule__part { white-space: pre-wrap; }
.reader-selection-action { position: fixed; z-index: 42; bottom: 56px; left: 50%; display: flex; align-items: center; gap: 10px; padding: 7px 10px 7px 13px; border: 1px solid var(--border-color); border-radius: 999px; background: var(--bg-primary); box-shadow: var(--shadow-sm); transform: translateX(-50%); color: var(--text-secondary); font-size: .75rem; }
.reader-selection-action button { padding: 5px 9px; border: 0; border-radius: 999px; background: var(--accent-color); color: #fff; cursor: pointer; font-size: .74rem; }
.reader-selection-action__cancel { background: transparent !important; color: var(--text-secondary) !important; }
.reader-lookup__context { padding-bottom: 6px; }
.reader-lookup__summary { display: grid; min-width: 0; gap: 2px; font-size: .75rem; }
.reader-lookup__summary strong { color: var(--accent-color); }
.reader-lookup__summary span, .reader-lookup__summary small { overflow-wrap: anywhere; color: var(--text-secondary); font-family: var(--font-ja); }
.reader-lookup__detail { display: grid; gap: 2px; color: var(--text-secondary); font-size: .75rem; }
.reader-lookup__detail strong { color: var(--accent-color); }
.reader-lookup__state { display: flex; flex-wrap: wrap; gap: 5px; margin: 8px 0; }
.reader-lookup__state button { padding: 4px 8px; border: 1px solid var(--border-color); border-radius: 4px; background: transparent; color: var(--text-secondary); cursor: pointer; font-size: .75rem; }
.reader-lookup__state button:hover { border-color: var(--accent-color); color: var(--accent-color); }
.reader-lookup__state { align-items: center; justify-content: space-between; color: var(--text-muted); font-size: .72rem; }
.reader-lookup__error { margin: 8px 0; color: var(--novelty-high-text); font-size: .8rem; }
@media (max-width: 700px) { .reader-view__chapter { display: none; } .reader-view__scroll { padding-inline: 16px; } }
</style>
