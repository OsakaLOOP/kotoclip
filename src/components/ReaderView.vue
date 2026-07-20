<script setup lang="ts">
import {
  ref,
  computed,
  watch,
  onMounted,
  onBeforeUnmount,
  nextTick,
} from "vue";
import {
  AlertTriangle,
  ArrowLeft,
  BookMarked,
  BookOpen,
  BriefcaseBusiness,
  FileUp,
  Gauge,
  Link2,
  Library,
  ListTree,
  Moon,
  Plus,
  Settings2,
  Type,
  X,
} from "@lucide/vue";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import type { ComponentPublicInstance } from "vue";
import { useVirtualizer, type Virtualizer } from "@tanstack/vue-virtual";
import { useTokenization } from "../composables/useTokenization";
import { useSelection } from "../composables/useSelection";
import { useDictionary } from "../composables/useDictionary";
import { useDragMerge } from "../composables/useDragMerge";
import { useScrollFocus } from "../composables/useScrollFocus";
import {
  DictEntry,
  ExpressionBoundaryEffect,
  ExpressionRule,
  ExpressionType,
  SegmentationCandidate,
  AnnotatedToken,
  GrammarDictionaryTarget,
} from "../types";

import BunsetsuCapsule from "./BunsetsuCapsule.vue";
import ExplanationPopover from "./explanation/ExplanationPopover.vue";
import GrammarPopover from "./explanation/GrammarPopover.vue";
import GrammarLibraryPanel from "./GrammarLibraryPanel.vue";
import ContextMenu from "./ContextMenu.vue";
import ExportPanel from "./ExportPanel.vue";
import AnalysisProgressPanel from "./AnalysisProgressPanel.vue";
import RuleWorkbench from "./RuleWorkbench.vue";
import DictionaryContent from "./dictionary/DictionaryContent.vue";
import DictionarySettingsPanel from "./dictionary/DictionarySettingsPanel.vue";
import LibraryHome from "./reader/LibraryHome.vue";
import ReaderAppearancePanel from "./reader/ReaderAppearancePanel.vue";
import ReaderImageBlock from "./reader/ReaderImageBlock.vue";
import ReaderNavigationPanel from "./reader/ReaderNavigationPanel.vue";
import ReaderProgressBar from "./reader/ReaderProgressBar.vue";
import { dictionaryTargetForToken } from "../utils/dictionaryTarget";
import {
  compileReaderDocument,
  prepareMarkdownDocument,
  type MarkdownMetadata,
  type ReaderChapter,
  type ReaderDocument,
} from "../utils/markdownDocument";
import {
  resourceKey,
  resourceMap,
  type LibraryBook,
  type LibraryBookSummary,
  type LibraryResource,
} from "../reader/library";
import {
  buildReaderRows,
  rowCharacterOffset,
  rowIndexForOffset,
} from "../reader/rows";
import {
  DEFAULT_READER_APPEARANCE,
  formatReadingDuration,
  normalizeAppearance,
  readingEstimate,
  type ReaderAppearance,
} from "../reader/reading";
import {
  estimateReaderRow,
  resolveReaderRowMeasurement,
} from "../reader/virtualization";
import { useExplanationSession } from "../composables/useExplanationSession";
import { useExplanationInteraction } from "../composables/useExplanationInteraction";

// 状态定义
const inputText = ref("");
const showLibrary = ref(true);
const showInput = ref(false);
const isReading = computed(() => !showLibrary.value && !showInput.value);
const isImportingEpub = ref(false);
const epubImportError = ref<string | null>(null);
const libraryBooks = ref<LibraryBookSummary[]>([]);
const libraryLoading = ref(true);
const libraryError = ref<string | null>(null);
const openingLibraryBookId = ref<string | null>(null);
const bookAnalysisTransitioning = ref(false);
let bookAnalysisGeneration = 0;
const libraryPath = ref("");
const activeLibraryBook = ref<LibraryBook | null>(null);
const activeResources = ref<Map<string, LibraryResource>>(new Map());
const readerDocument = ref<ReaderDocument | null>(null);
const currentDocumentMetadata = ref<MarkdownMetadata | null>(null);
const inputMetadata = computed(
  () => prepareMarkdownDocument(inputText.value).metadata,
);
const einkMode = ref(false);
const showAppearance = ref(false);
const showNavigation = ref(false);
const currentCharOffset = ref(0);
const appearanceStorageKey = "kotoclip:reader-appearance:v1";
const readerAppearance = ref<ReaderAppearance>(loadReaderAppearance());
const showDevMetrics =
  import.meta.env.DEV || import.meta.env.VITE_SHOW_DEV_METRICS === "true";
const analysisMetrics = ref<{
  characterCount: number;
  durationMs: number;
  listenerSetupMs: number;
  invokeAndTransferMs: number;
  paragraphBuildMs: number;
  renderSetupMs: number;
  backendDurationMs: number;
  ipcAndParseMs: number;
} | null>(null);

const scrollContainerRef = ref<HTMLElement | null>(null);
const { triggerUpdate } = useScrollFocus(scrollContainerRef);

// 初始化 composables
const {
  paragraphs,
  isAnalyzing,
  errorMsg,
  analysisProgress,
  frontendTiming,
  analyzeText,
  continueDocumentAnalysis,
  requestDocumentRange,
  documentComplete,
  documentCharRange,
  availableRanges,
  lastOpenCacheHit,
  lastPatchBytes,
  lastInvalidation,
  backendReady,
  backendError,
  initializeBackendStatus,
  disposeBackendStatusListener,
  cancelAnalysis,
  addExpressionRule,
  previewExpressionRule,
  getExpressionRules,
  deleteExpressionRule,
  refreshDocumentExpressions,
  markDocumentKnown,
  getCandidates,
  chooseSegmentation,
} = useTokenization();
const {
  selectedKeys,
  toggleSelect,
  markAsKnown,
  markAsUnknown,
  exportSelected,
  updateNote,
} = useSelection(paragraphs, markDocumentKnown);
const {
  lookupWord,
  chooseDictionaryTarget,
  dictionarySettings,
  loadDictionarySettings,
  setDictionaryOrder,
} = useDictionary();
const explanation = useExplanationSession(lookupWord, chooseDictionaryTarget);
const explanationInteraction = useExplanationInteraction({
  findToken(paragraphId, tokenIndex) {
    return paragraphs.value.find((item) => item.id === paragraphId)?.tokens[
      tokenIndex
    ];
  },
  session: explanation,
});

function loadReaderAppearance(): ReaderAppearance {
  try {
    return normalizeAppearance(
      JSON.parse(localStorage.getItem(appearanceStorageKey) || "{}"),
    );
  } catch {
    return { ...DEFAULT_READER_APPEARANCE };
  }
}

function updateReaderAppearance(value: ReaderAppearance) {
  readerAppearance.value = normalizeAppearance(value);
  try {
    localStorage.setItem(
      appearanceStorageKey,
      JSON.stringify(readerAppearance.value),
    );
  } catch {
    // 设置仍在当前会话生效。
  }
}

function resolveReaderImage(src: string) {
  if (src.startsWith("data:")) return { src };
  const resource = activeResources.value.get(resourceKey(src));
  return resource
    ? {
        src: convertFileSrc(resource.path),
        width: resource.width,
        height: resource.height,
      }
    : undefined;
}

const readerRows = computed(() =>
  buildReaderRows(
    paragraphs.value,
    readerDocument.value,
    resolveReaderImage,
    documentComplete.value,
  ),
);
const currentChapter = computed(() => {
  const chapters = readerDocument.value?.chapters ?? [];
  let current: ReaderChapter | undefined;
  for (const chapter of chapters) {
    if (chapter.charOffset > currentCharOffset.value) break;
    current = chapter;
  }
  return current;
});
const documentTotalCharacters = computed(() => {
  const blocks = readerDocument.value?.blocks ?? [];
  for (let index = blocks.length - 1; index >= 0; index--) {
    const block = blocks[index];
    if (block.kind !== "image") return block.charRange[1];
  }
  return documentCharRange.value[1];
});
const progressEstimate = computed(() =>
  readingEstimate(currentCharOffset.value, documentTotalCharacters.value),
);
const readerStyle = computed(() => ({
  "--reader-font-size": `${readerAppearance.value.fontSize}px`,
  "--reader-line-height": String(readerAppearance.value.lineHeight),
  "--reader-paragraph-gap": `${readerAppearance.value.paragraphGap}px`,
  "--reader-content-width": `${readerAppearance.value.contentWidth}px`,
}));

function textRowAt(index: number) {
  const row = readerRows.value[index];
  if (row?.kind !== "text") throw new Error(`阅读行 ${index} 不是文本段落`);
  return row;
}

function imageRowAt(index: number) {
  const row = readerRows.value[index];
  if (row?.kind !== "image") throw new Error(`阅读行 ${index} 不是图片`);
  return row;
}

function openGrammarDictionary(target: GrammarDictionaryTarget) {
  for (const paragraph of paragraphs.value) {
    for (
      let tokenIndex = 0;
      tokenIndex < paragraph.tokens.length;
      tokenIndex++
    ) {
      const token = paragraph.tokens[tokenIndex];
      const morphemeIndex = token.bunsetsu.morphemes.findIndex(
        (morpheme) =>
          morpheme.char_range[0] === target.char_range[0] &&
          morpheme.char_range[1] === target.char_range[1],
      );
      if (morphemeIndex < 0) continue;
      const capsule = document.querySelector<HTMLElement>(
        `[data-paragraph-id="${paragraph.id}"][data-token-index="${tokenIndex}"]`,
      );
      const morpheme = capsule?.querySelector<HTMLElement>(
        `[data-morpheme-index="${morphemeIndex}"]`,
      );
      if (!capsule || !morpheme) return;
      explanation.focusMorpheme(
        { paragraphId: paragraph.id, tokenIndex, morphemeIndex },
        token,
        capsule,
        morpheme,
      );
      return;
    }
  }
}

// 详细释义弹窗状态
const showDefinitionModal = ref(false);
const activeWordForModal = ref("");
const modalDefinitions = ref<DictEntry[]>([]);

// 上下文右键菜单状态
const contextMenuShow = ref(false);
const contextMenuX = ref(0);
const contextMenuY = ref(0);
const contextMenuToken = ref<any | null>(null);
const contextMenuParagraphId = ref(0);
const contextMenuTokenIndex = ref(0);
const contextMenuCandidates = ref<SegmentationCandidate[]>([]);
const candidatesLoading = ref(false);

// 侧边栏导出面板显示状态
const showExportPanel = ref(false);
const showGrammarLibrary = ref(false);
const showDictionarySettings = ref(false);
const expressionRules = ref<ExpressionRule[]>([]);
const expressionDraft = ref<AnnotatedToken[]>([]);
const expressionDraftMorphemeRange = ref({
  startMorphemeIdx: 0,
  endMorphemeIdx: 0,
});
const showRuleWorkbench = ref(false);
const ruleWorkbenchView = ref<"library" | "editor">("library");

async function openExpressionRules() {
  ruleWorkbenchView.value = "library";
  showRuleWorkbench.value = true;
  try {
    expressionRules.value = await getExpressionRules();
  } catch (error) {
    console.error("Rule catalog load error:", error);
  }
}

async function removeExpressionRule(id: number) {
  await deleteExpressionRule(id);
  expressionRules.value = await getExpressionRules();
  if (isReading.value && inputText.value.trim()) {
    await refreshDocumentExpressions();
  }
}

async function saveExpressionDraft(
  label: string,
  description: string,
  bunsetsuStates: ("fixed" | "slot" | "any")[],
  morphemeMasks: boolean[][],
  gapAfter: number | null,
  expressionType: ExpressionType,
  priority: number,
  boundaryEffect: ExpressionBoundaryEffect,
) {
  try {
    await addExpressionRule(
      expressionDraft.value,
      label,
      description,
      bunsetsuStates,
      morphemeMasks,
      gapAfter,
      expressionType,
      priority,
      boundaryEffect,
    );
    showRuleWorkbench.value = false;
    expressionDraft.value = [];
    expressionDraftMorphemeRange.value = {
      startMorphemeIdx: 0,
      endMorphemeIdx: 0,
    };
    await refreshDocumentExpressions();
  } catch (err) {
    alert(`保存跨文节表达失败：${String(err)}`);
  }
}

// 拖拽合并 Composable
const {
  isDragging,
  isTokenDragSelected,
  handleMouseDown,
  handleMouseMove,
  handleMouseUp,
} = useDragMerge(
  paragraphs,
  async (tokens, _paragraphId, startMorphemeIdx, endMorphemeIdx) => {
    expressionDraft.value = tokens.filter((t) => t.display_class === "content");
    expressionDraftMorphemeRange.value = { startMorphemeIdx, endMorphemeIdx };
    ruleWorkbenchView.value = "editor";
    showRuleWorkbench.value = true;
    try {
      expressionRules.value = await getExpressionRules();
    } catch (error) {
      console.error("Rule catalog load error:", error);
    }
  },
);

function estimateVirtualRow(index: number): number {
  const row = readerRows.value[index];
  const viewportHeight =
    scrollContainerRef.value?.clientHeight ?? window.innerHeight;
  return estimateReaderRow({
    kind: row?.kind ?? "text",
    heading: row?.kind === "text" && Boolean(row.heading),
    viewportHeight,
    fontSize: readerAppearance.value.fontSize,
    lineHeight: readerAppearance.value.lineHeight,
    contentWidth: Math.min(
      readerAppearance.value.contentWidth,
      Math.max(0, window.innerWidth - 40),
    ),
    imageWidth: row?.kind === "image" ? row.intrinsicWidth : undefined,
    imageHeight: row?.kind === "image" ? row.intrinsicHeight : undefined,
    hasCaption:
      row?.kind === "image" && Boolean(row.image.title || row.image.alt),
  });
}

function measureReaderRow(
  element: HTMLElement,
  entry: ResizeObserverEntry | undefined,
  instance: Virtualizer<HTMLElement, HTMLElement>,
): number {
  const index = Number(element.dataset.index);
  const row = readerRows.value[index];
  const observedSize = entry?.borderBoxSize?.[0]?.blockSize;
  return resolveReaderRowMeasurement({
    kind: row?.kind ?? "text",
    imageState:
      element.querySelector<HTMLElement>("[data-image-state]")?.dataset
        .imageState,
    cachedSize: row ? instance.itemSizeCache.get(row.key) : undefined,
    estimatedSize: estimateVirtualRow(index),
    observedSize,
    elementSize: element.getBoundingClientRect().height,
  });
}

// 使用 @tanstack/vue-virtual 虚拟滚动；尺寸缓存按稳定正文 key 保留。
const virtualizer = useVirtualizer(
  computed(() => ({
    count: readerRows.value.length,
    getScrollElement: () => scrollContainerRef.value,
    estimateSize: estimateVirtualRow,
    measureElement: measureReaderRow,
    overscan: 5,
    gap: readerAppearance.value.paragraphGap,
    getItemKey: (index) => readerRows.value[index]?.key ?? index,
    useAnimationFrameWithResizeObserver: true,
  })),
);
virtualizer.value.shouldAdjustScrollPositionOnItemSizeChange = (item) =>
  item.end <= (scrollContainerRef.value?.scrollTop ?? 0) + 1;

const virtualRowElements = new Map<string, HTMLElement>();

function measureVirtualRow(
  element: Element | ComponentPublicInstance | null,
  key: string,
) {
  if (element instanceof HTMLElement) {
    virtualRowElements.set(key, element);
    virtualizer.value.measureElement(element);
  } else {
    const previous = virtualRowElements.get(key);
    if (previous && !previous.isConnected) virtualRowElements.delete(key);
    virtualizer.value.measureElement(null);
  }
}

async function measureSettledImage(key: string) {
  await nextTick();
  const element = virtualRowElements.get(key);
  if (!element?.isConnected) return;
  const index = Number(element.dataset.index);
  if (!Number.isInteger(index) || readerRows.value[index]?.key !== key) return;
  // 正常滚动期间 virtualizer 会跳过 measureElement 的同步读数；图片解码完成必须
  // 直接提交真实 border-box，否则下方绝对定位行会暂时沿用旧估算高度。
  virtualizer.value.resizeItem(
    index,
    measureReaderRow(element, undefined, virtualizer.value),
  );
}

interface ViewportAnchor {
  key: string;
  inset: number;
}

function captureViewportAnchor(rows = readerRows.value): ViewportAnchor | null {
  const container = scrollContainerRef.value;
  if (!container) return null;
  const item = virtualizer.value
    .getVirtualItems()
    .find((candidate) => candidate.end > container.scrollTop);
  const row = item ? rows[item.index] : undefined;
  return item && row
    ? { key: row.key, inset: container.scrollTop - item.start }
    : null;
}

let anchorRestoreGeneration = 0;
async function restoreViewportAnchor(anchor: ViewportAnchor | null) {
  if (!anchor) return;
  const generation = ++anchorRestoreGeneration;
  await nextTick();
  requestAnimationFrame(() => {
    if (generation !== anchorRestoreGeneration) return;
    const index = readerRows.value.findIndex((row) => row.key === anchor.key);
    const offset =
      index >= 0
        ? virtualizer.value.getOffsetForIndex(index, "start")?.[0]
        : undefined;
    if (offset !== undefined)
      virtualizer.value.scrollToOffset(Math.max(0, offset + anchor.inset));
  });
}

// 阅读态切换只处理交互状态；新文档在 triggerAnalysis 中执行一次完整测量。
watch(
  isReading,
  (reading) => {
    explanation.closeAll();
    if (!reading) return;
    triggerUpdate();
  },
  { flush: "post" },
);

watch(
  readerRows,
  (rows, previousRows) => {
    if (!isReading.value) return;
    const anchor = captureViewportAnchor(previousRows);
    if (anchor) {
      const previousIndex = previousRows.findIndex(
        (row) => row.key === anchor.key,
      );
      const nextIndex = rows.findIndex((row) => row.key === anchor.key);
      if (nextIndex >= 0 && nextIndex !== previousIndex)
        void restoreViewportAnchor(anchor);
    }
    triggerUpdate();
  },
  { flush: "pre" },
);

let remeasureFrame: number | undefined;
let pendingRemeasureAnchor: ViewportAnchor | null = null;
function scheduleReaderRemeasure() {
  if (!isReading.value) return;
  pendingRemeasureAnchor ??= captureViewportAnchor();
  if (remeasureFrame !== undefined) cancelAnimationFrame(remeasureFrame);
  remeasureFrame = requestAnimationFrame(() => {
    remeasureFrame = undefined;
    const anchor = pendingRemeasureAnchor;
    pendingRemeasureAnchor = null;
    virtualizer.value.measure();
    void restoreViewportAnchor(anchor);
  });
}

watch(readerAppearance, scheduleReaderRemeasure, { deep: true, flush: "sync" });

let rangePrefetchPending = false;

async function prefetchNextMissingRange() {
  const container = scrollContainerRef.value;
  if (!container || documentComplete.value || rangePrefetchPending) return;
  const nearEnd =
    container.scrollTop + container.clientHeight >=
    container.scrollHeight - container.clientHeight * 2;
  if (!nearEnd) return;
  const loadedEnd = availableRanges.value.reduce(
    (end, range) => (range[0] <= end ? Math.max(end, range[1]) : end),
    0,
  );
  if (loadedEnd >= documentCharRange.value[1]) return;
  rangePrefetchPending = true;
  try {
    await requestDocumentRange([
      loadedEnd,
      Math.min(documentCharRange.value[1], loadedEnd + 4_000),
    ]);
  } finally {
    rangePrefetchPending = false;
  }
}

async function handleReaderScroll() {
  updateCurrentReadingPosition();
  explanation.refreshAnchor();
  void prefetchNextMissingRange().catch((error) => {
    console.error("Viewport range prefetch failed:", error);
  });
  await nextTick();
  explanation.refreshAnchor();
}

let progressPersistTimer: number | undefined;
let lastProgressPersistedAt = Date.now();

function updateCurrentReadingPosition() {
  const container = scrollContainerRef.value;
  if (!container) return;
  const probe =
    container.scrollTop + Math.min(120, container.clientHeight * 0.2);
  const visible = virtualizer.value.getVirtualItems();
  const item = visible.find((row) => row.end >= probe) ?? visible[0];
  currentCharOffset.value = rowCharacterOffset(
    item ? readerRows.value[item.index] : undefined,
  );
  if (!activeLibraryBook.value) return;
  window.clearTimeout(progressPersistTimer);
  progressPersistTimer = window.setTimeout(
    () => void persistLibraryProgress(),
    1200,
  );
}

async function persistLibraryProgress() {
  const book = activeLibraryBook.value;
  if (!book) return;
  const now = Date.now();
  const readingSeconds = Math.max(
    0,
    Math.round((now - lastProgressPersistedAt) / 1000),
  );
  lastProgressPersistedAt = now;
  try {
    const updated = await invoke<LibraryBookSummary>(
      "update_library_progress",
      {
        id: book.id,
        progressOffset: currentCharOffset.value,
        totalCharacters: documentTotalCharacters.value,
        currentChapter: currentChapter.value?.title,
        readingSeconds,
      },
    );
    Object.assign(book, updated);
    const shelf = libraryBooks.value.find((item) => item.id === book.id);
    if (shelf) Object.assign(shelf, updated);
  } catch (error) {
    console.error("Reading progress persist failed:", error);
  }
}

async function navigateToOffset(offset: number) {
  const covered = availableRanges.value.some(
    (range) => offset >= range[0] && offset < range[1],
  );
  if (!covered && offset < documentCharRange.value[1]) {
    await requestDocumentRange([
      offset,
      Math.min(documentCharRange.value[1], offset + 4_000),
    ]);
  }
  await nextTick();
  virtualizer.value.measure();
  virtualizer.value.scrollToIndex(rowIndexForOffset(readerRows.value, offset), {
    align: "start",
  });
  currentCharOffset.value = offset;
}

async function navigateChapter(chapter: ReaderChapter) {
  showNavigation.value = false;
  try {
    await navigateToOffset(chapter.charOffset);
  } catch (error) {
    console.error("Chapter navigation failed:", error);
  }
}

// 监听拖拽的鼠标松开事件 (挂载在 window 以防在胶囊外松开失效)
onMounted(() => {
  void initializeBackendStatus();
  void loadLibrary();
  window.addEventListener("mouseup", handleMouseUp);
  window.addEventListener("resize", explanation.refreshAnchor);
  window.addEventListener("resize", scheduleReaderRemeasure, { passive: true });
});

watch(
  backendReady,
  (ready) => {
    if (!ready) return;
    void loadDictionarySettings().catch((error) => {
      console.error("Dictionary settings load error:", error);
    });
  },
  { immediate: true },
);

async function updateDictionaryOrder(order: string[]) {
  try {
    await setDictionaryOrder(order);
  } catch (error) {
    console.error("Dictionary order save error:", error);
    alert(`保存词典排序失败：${String(error)}`);
    await loadDictionarySettings();
  }
}

onBeforeUnmount(() => {
  window.clearTimeout(progressPersistTimer);
  if (activeLibraryBook.value) void persistLibraryProgress();
  disposeBackendStatusListener();
  window.removeEventListener("mouseup", handleMouseUp);
  window.removeEventListener("resize", explanation.refreshAnchor);
  window.removeEventListener("resize", scheduleReaderRemeasure);
  if (remeasureFrame !== undefined) cancelAnimationFrame(remeasureFrame);
  explanation.closeAll();
});

// 执行文本分析
async function triggerAnalysis(
  recordExposure = true,
  beforeReader?: Promise<void>,
  shouldPresentReader: () => boolean = () => true,
) {
  if (!inputText.value.trim()) return;
  const compiled = compileReaderDocument(inputText.value);
  const sourceText = compiled.analysisText;
  if (!sourceText.trim()) return;
  const metadata =
    Object.keys(compiled.metadata).length > 0
      ? compiled.metadata
      : activeLibraryBook.value
        ? {
            title: activeLibraryBook.value.title,
            author: activeLibraryBook.value.author,
            language: activeLibraryBook.value.language,
          }
        : {};
  const startedAt = performance.now();
  const succeeded = await analyzeText(sourceText, recordExposure);
  if (succeeded) {
    await beforeReader;
    if (!shouldPresentReader()) return;
    inputText.value = compiled.markdown;
    readerDocument.value = compiled;
    currentDocumentMetadata.value = metadata;
    const renderSetupStartedAt = performance.now();
    showLibrary.value = false;
    showInput.value = false;
    await nextTick();
    virtualizer.value.measure();
    triggerUpdate();
    const resumeOffset = activeLibraryBook.value?.progressOffset ?? 0;
    if (resumeOffset > 0) {
      await navigateToOffset(resumeOffset);
    } else {
      virtualizer.value.scrollToOffset(0);
      currentCharOffset.value = 0;
    }
    await nextTick();

    // 覆盖盲区二：等待浏览器真正的 Layout 与 Paint 绘制完成
    await new Promise<void>((resolve) => {
      requestAnimationFrame(() => {
        setTimeout(resolve, 0);
      });
    });

    analysisMetrics.value = {
      characterCount: Array.from(sourceText).length,
      durationMs: Math.round(performance.now() - startedAt),
      listenerSetupMs: frontendTiming.value?.listenerSetupMs ?? 0,
      invokeAndTransferMs: frontendTiming.value?.invokeAndTransferMs ?? 0,
      paragraphBuildMs: frontendTiming.value?.paragraphBuildMs ?? 0,
      renderSetupMs: Math.round(performance.now() - renderSetupStartedAt),
      backendDurationMs: frontendTiming.value?.backendDurationMs ?? 0,
      ipcAndParseMs: frontendTiming.value?.ipcAndParseMs ?? 0,
    };
    void continueDocumentAnalysis().catch((error) => {
      console.error("Background document analysis failed:", error);
    });
  }
}

async function importEpub() {
  epubImportError.value = null;
  const selected = await open({
    multiple: false,
    directory: false,
    filters: [{ name: "EPUB 电子书", extensions: ["epub"] }],
  });
  if (!selected || Array.isArray(selected)) return;

  isImportingEpub.value = true;
  libraryError.value = null;
  try {
    const imported = await invoke<LibraryBook>("import_epub_document", {
      path: selected,
    });
    await loadLibrary();
    await openBookData(imported);
  } catch (error) {
    epubImportError.value = String(error);
    libraryError.value = `EPUB 导入失败：${String(error)}`;
  } finally {
    isImportingEpub.value = false;
  }
}

async function loadLibrary() {
  libraryLoading.value = true;
  libraryError.value = null;
  try {
    const [books, location] = await Promise.all([
      invoke<LibraryBookSummary[]>("list_library_books"),
      invoke<string>("get_library_location"),
    ]);
    libraryBooks.value = books;
    libraryPath.value = location;
  } catch (error) {
    libraryError.value = `无法读取书库：${String(error)}`;
  } finally {
    libraryLoading.value = false;
  }
}

async function openLibraryBook(id: string) {
  if (openingLibraryBookId.value) return;
  const generation = ++bookAnalysisGeneration;
  openingLibraryBookId.value = id;
  libraryError.value = null;
  try {
    const [book] = await Promise.all([
      invoke<LibraryBook>("open_library_book", { id }),
      waitForBookOpeningAnimation(),
    ]);
    if (generation !== bookAnalysisGeneration) return;
    await openBookData(book, generation);
  } catch (error) {
    if (generation === bookAnalysisGeneration) {
      libraryError.value = `无法打开书籍：${String(error)}`;
      showLibrary.value = true;
      showInput.value = false;
    }
  } finally {
    if (
      generation === bookAnalysisGeneration &&
      openingLibraryBookId.value === id
    ) {
      openingLibraryBookId.value = null;
    }
  }
}

async function openBookData(
  book: LibraryBook,
  generation = ++bookAnalysisGeneration,
) {
  activeLibraryBook.value = book;
  activeResources.value = resourceMap(book.resources);
  inputText.value = book.markdown;
  currentDocumentMetadata.value = null;
  lastProgressPersistedAt = Date.now();
  let analysis: Promise<void> | undefined;
  try {
    await runReaderViewTransition(() => {
      showLibrary.value = false;
      showInput.value = true;
      openingLibraryBookId.value = null;
      bookAnalysisTransitioning.value = true;
      analysis = triggerAnalysis(
        true,
        waitForBookAnalysisEntry(),
        () => generation === bookAnalysisGeneration && !showLibrary.value,
      );
    });
    await analysis;
  } finally {
    if (generation === bookAnalysisGeneration)
      bookAnalysisTransitioning.value = false;
  }
}

function waitForBookOpeningAnimation(): Promise<void> {
  if (window.matchMedia("(prefers-reduced-motion: reduce)").matches)
    return Promise.resolve();
  return new Promise((resolve) => window.setTimeout(resolve, 300));
}

function waitForBookAnalysisEntry(): Promise<void> {
  if (window.matchMedia("(prefers-reduced-motion: reduce)").matches)
    return Promise.resolve();
  return new Promise((resolve) => window.setTimeout(resolve, 320));
}

async function runReaderViewTransition(update: () => void): Promise<void> {
  const transitionDocument = document as Document & {
    startViewTransition?: (callback: () => void | Promise<void>) => {
      updateCallbackDone: Promise<void>;
    };
  };
  if (
    !transitionDocument.startViewTransition ||
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  ) {
    update();
    await nextTick();
    return;
  }
  const transition = transitionDocument.startViewTransition(async () => {
    update();
    await nextTick();
  });
  await transition.updateCallbackDone;
}

function showMarkdownInput() {
  bookAnalysisGeneration++;
  openingLibraryBookId.value = null;
  bookAnalysisTransitioning.value = false;
  cancelAnalysis();
  if (activeLibraryBook.value && isReading.value)
    void persistLibraryProgress();
  showLibrary.value = false;
  showInput.value = true;
  activeLibraryBook.value = null;
  activeResources.value = new Map();
  readerDocument.value = null;
  inputText.value = "";
}

function returnToLibrary() {
  bookAnalysisGeneration++;
  openingLibraryBookId.value = null;
  bookAnalysisTransitioning.value = false;
  cancelAnalysis();
  if (activeLibraryBook.value && isReading.value)
    void persistLibraryProgress();
  showAppearance.value = false;
  showNavigation.value = false;
  showInput.value = false;
  showLibrary.value = true;
  activeLibraryBook.value = null;
  activeResources.value = new Map();
  readerDocument.value = null;
  inputText.value = "";
  void loadLibrary();
}

async function revealLibrary() {
  if (!libraryPath.value) return;
  try {
    await revealItemInDir(libraryPath.value);
  } catch (error) {
    libraryError.value = `无法打开书库目录：${String(error)}`;
  }
}

async function removeLibraryBook(book: LibraryBookSummary) {
  if (
    !window.confirm(
      `从书架移除《${book.title}》？\n\n原始 EPUB、Markdown、图片和阅读进度都会删除。`,
    )
  )
    return;
  try {
    await invoke("remove_library_book", { id: book.id });
    await loadLibrary();
  } catch (error) {
    libraryError.value = `移除失败：${String(error)}`;
  }
}

function libraryCoverUrl(book: LibraryBookSummary): string | undefined {
  return book.coverPath ? convertFileSrc(book.coverPath) : undefined;
}

// 事件委托：段落内的点击 (切换选中/已知)
function handleParagraphClick(e: MouseEvent, paragraphId: number) {
  // 右键菜单显示或拖拽期间，不切换导出选择。
  if (contextMenuShow.value || isDragging.value) return;

  const target = e.target as HTMLElement;
  const capsuleEl = target.closest("[data-token-index]") as HTMLElement;
  if (!capsuleEl) return;

  const tokenIndex = parseInt(
    capsuleEl.getAttribute("data-token-index") || "",
    10,
  );
  if (isNaN(tokenIndex)) return;

  const p = paragraphs.value.find((para) => para.id === paragraphId);
  const token = p?.tokens[tokenIndex];
  const isPunc =
    token &&
    (token.display_class === "punctuation" ||
      token.display_class === "line_break");
  if (!token || isPunc) return;

  // 切换该 token 选中状态 (用于 Anki 导出)
  toggleSelect(paragraphId, tokenIndex);
}

// 右键仅保留词条操作和 N-best 分词候选；词汇与语素解释统一由悬浮进入。
function handleParagraphContextMenu(e: MouseEvent, paragraphId: number) {
  const target = e.target as HTMLElement;
  const capsuleEl = target.closest("[data-token-index]") as HTMLElement;
  if (!capsuleEl) return;
  const tokenIndex = parseInt(
    capsuleEl.getAttribute("data-token-index") || "",
    10,
  );
  if (isNaN(tokenIndex)) return;
  const p = paragraphs.value.find((para) => para.id === paragraphId);
  const token = p?.tokens[tokenIndex];
  const isPunc =
    token &&
    (token.display_class === "punctuation" ||
      token.display_class === "line_break");
  if (!token || isPunc) return;
  contextMenuX.value = e.clientX;
  contextMenuY.value = e.clientY;
  contextMenuToken.value = token;
  contextMenuParagraphId.value = paragraphId;
  contextMenuTokenIndex.value = tokenIndex;
  contextMenuCandidates.value = [];
  contextMenuShow.value = true;
}

async function applyContextCandidate(candidate: SegmentationCandidate) {
  if (!contextMenuToken.value) return;
  try {
    await chooseSegmentation(contextMenuToken.value, candidate);
    contextMenuShow.value = false;
  } catch (err) {
    console.error("Candidate Apply Error:", err);
    alert(`应用 N-best 候选失败：${String(err)}`);
  }
}

async function loadContextCandidates() {
  if (!contextMenuToken.value) return;
  candidatesLoading.value = true;
  try {
    contextMenuCandidates.value = await getCandidates(
      contextMenuToken.value,
      5,
    );
  } catch (err) {
    console.error("Candidate Lookup Error:", err);
    contextMenuCandidates.value = [];
  } finally {
    candidatesLoading.value = false;
  }
}

// 查看完整释义对话框
async function viewFullDefinition(paragraphId: number, tokenIndex: number) {
  const p = paragraphs.value.find((para) => para.id === paragraphId);
  const token = p?.tokens[tokenIndex];
  if (!token) return;

  const target = dictionaryTargetForToken(token);
  activeWordForModal.value = target.word;
  showDefinitionModal.value = true;
  modalDefinitions.value = [];

  const lookup = await lookupWord(
    target.word,
    target.reading,
    false,
    target.pos,
  );
  modalDefinitions.value = lookup?.entries ?? [];
}

// 切换 E-ink 降级模式
function toggleEinkMode() {
  einkMode.value = !einkMode.value;
  if (einkMode.value) {
    document.body.classList.add("eink-mode");
  } else {
    document.body.classList.remove("eink-mode");
  }
  triggerUpdate();
}

// 触发 Anki 数据包生成并保存
async function executeExport() {
  try {
    const jsonStr = await exportSelected(
      readerDocument.value?.analysisText ?? inputText.value,
      async (word, reading) => {
        return (await lookupWord(word, reading))?.entries ?? [];
      },
    );

    // 创建本地 Blob 并触发浏览器下载 (Tauri 环境下可直接调用本地存储，此处通过浏览器 API 下载十分通用)
    const blob = new Blob([jsonStr], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `kotoclip_export_${Date.now()}.json`;
    a.click();
    URL.revokeObjectURL(url);

    alert("生词本数据包已成功导出！");
  } catch (err) {
    alert("导出失败，请检查释义词典配置。");
  }
}

// 清理全部已选 Keys
function clearAllSelections() {
  for (const p of paragraphs.value) {
    for (const token of p.tokens) {
      token.is_selected = false;
    }
  }
  selectedKeys.value = [];
}

// 单个移出选中列表
function removeSelectedKey(paragraphId: number, tokenIndex: number) {
  toggleSelect(paragraphId, tokenIndex);
}
</script>

<template>
  <div class="reader-container">
    <!-- 顶部导航栏 -->
    <header class="app-header">
      <div class="logo-title">
        <button
          v-if="!showLibrary"
          class="header-back"
          type="button"
          title="返回书架"
          @click="returnToLibrary"
        >
          <ArrowLeft :size="19" aria-hidden="true" />
        </button>
        <BookOpen
          class="logo-icon"
          :size="24"
          stroke-width="1.8"
          aria-hidden="true"
        />
        <span class="logo-text">Kotoclip</span>
        <span v-if="showLibrary" class="logo-sub">日语生肉阅读助手</span>
        <span v-else-if="showInput" class="logo-sub">{{
          activeLibraryBook ? "正在分析书籍" : "Markdown 文本"
        }}</span>
        <div v-else-if="currentDocumentMetadata" class="document-identity">
          <strong>{{ currentDocumentMetadata.title || "未命名文本" }}</strong>
          <span v-if="currentDocumentMetadata.author">{{
            currentDocumentMetadata.author
          }}</span>
        </div>
      </div>
      <div v-if="isReading" class="action-bar">
        <div v-if="showDevMetrics && analysisMetrics" class="dev-metrics-entry">
          <button
            class="icon-btn compact-tool"
            type="button"
            aria-label="开发者分析指标"
          >
            <Gauge :size="17" aria-hidden="true" />
          </button>
          <div class="dev-metrics-popover" role="status">
            <strong>分析指标</strong>
            <dl>
              <div>
                <dt>正文</dt>
                <dd>{{ analysisMetrics.characterCount }} 字</dd>
              </div>
              <div>
                <dt>会话</dt>
                <dd>
                  {{
                    lastOpenCacheHit
                      ? "暖缓存"
                      : documentComplete
                        ? "已补全"
                        : "渐进"
                  }}
                </dd>
              </div>
              <div>
                <dt>范围</dt>
                <dd>
                  {{ availableRanges[availableRanges.length - 1]?.[1] ?? 0 }}/{{
                    documentCharRange[1]
                  }}
                </dd>
              </div>
              <div>
                <dt>Patch</dt>
                <dd>{{ Math.round(lastPatchBytes / 1024) }} KB</dd>
              </div>
              <div v-if="lastInvalidation">
                <dt>失效</dt>
                <dd>
                  {{ lastInvalidation.reason }}
                  {{ lastInvalidation.recomputedCharacters }}/{{
                    lastInvalidation.totalCharacters
                  }}
                </dd>
              </div>
              <div>
                <dt>总耗时</dt>
                <dd>{{ analysisMetrics.durationMs }} ms</dd>
              </div>
              <div>
                <dt>后端</dt>
                <dd>{{ analysisMetrics.backendDurationMs }} ms</dd>
              </div>
              <div>
                <dt>IPC/解析</dt>
                <dd>{{ analysisMetrics.ipcAndParseMs }} ms</dd>
              </div>
              <div>
                <dt>组段</dt>
                <dd>{{ analysisMetrics.paragraphBuildMs }} ms</dd>
              </div>
              <div>
                <dt>首帧绘制</dt>
                <dd>{{ analysisMetrics.renderSetupMs }} ms</dd>
              </div>
            </dl>
          </div>
        </div>
        <button
          class="icon-btn chapter-button"
          :class="{ active: showNavigation }"
          @click="showNavigation = !showNavigation"
        >
          <ListTree :size="16" aria-hidden="true" />
          {{ currentChapter?.title || "章节" }}
        </button>
        <button
          class="icon-btn compact-tool"
          :class="{ active: showAppearance }"
          title="阅读排版"
          aria-label="阅读排版"
          @click="showAppearance = !showAppearance"
        >
          <Type :size="17" aria-hidden="true" />
        </button>
        <button
          class="icon-btn compact-tool export-tool"
          :class="{ active: showExportPanel }"
          :title="`导出本（${selectedKeys.length}）`"
          :aria-label="`导出本（${selectedKeys.length}）`"
          @click="showExportPanel = !showExportPanel"
        >
          <BriefcaseBusiness :size="16" aria-hidden="true" /><span
            v-if="selectedKeys.length"
            class="tool-count"
            >{{ selectedKeys.length }}</span
          >
        </button>
        <button
          class="icon-btn compact-tool"
          :class="{ active: showRuleWorkbench }"
          title="表达规则"
          aria-label="表达规则"
          @click="openExpressionRules"
        >
          <Link2 :size="16" aria-hidden="true" />
        </button>
        <button
          class="icon-btn compact-tool"
          :class="{ active: showGrammarLibrary }"
          title="文法库"
          aria-label="文法库"
          @click="showGrammarLibrary = true"
        >
          <Library :size="16" aria-hidden="true" />
        </button>
        <button
          class="icon-btn compact-tool"
          :class="{ active: showDictionarySettings }"
          title="词典设置"
          aria-label="词典设置"
          @click="showDictionarySettings = true"
        >
          <Settings2 :size="16" aria-hidden="true" />
        </button>
        <button
          class="icon-btn compact-tool"
          :class="{ active: einkMode }"
          title="墨水屏模式"
          aria-label="墨水屏模式"
          @click="toggleEinkMode"
        >
          <Moon :size="16" aria-hidden="true" />
        </button>
        <button
          class="icon-btn compact-tool highlight"
          title="新建 Markdown 文本"
          aria-label="新建 Markdown 文本"
          @click="showMarkdownInput"
        >
          <Plus :size="16" aria-hidden="true" />
        </button>
      </div>
    </header>

    <!-- 主布局 -->
    <div class="main-layout">
      <LibraryHome
        v-if="showLibrary"
        :books="libraryBooks"
        :loading="libraryLoading"
        :importing="isImportingEpub"
        :opening-book-id="openingLibraryBookId"
        :error="libraryError"
        :library-path="libraryPath"
        :cover-url="libraryCoverUrl"
        @import="importEpub"
        @open="openLibraryBook"
        @input="showMarkdownInput"
        @reveal="revealLibrary"
        @remove="removeLibraryBook"
      />

      <!-- 1. 分析与文本输入模块 -->
      <section
        v-if="showInput && activeLibraryBook"
        class="book-analysis-view"
        :aria-busy="isAnalyzing || bookAnalysisTransitioning"
      >
        <div class="book-analysis-identity">
          <div class="book-analysis-cover">
            <img
              v-if="libraryCoverUrl(activeLibraryBook)"
              :src="libraryCoverUrl(activeLibraryBook)"
              alt=""
            />
            <BookMarked
              v-else
              :size="34"
              stroke-width="1.8"
              aria-hidden="true"
            />
          </div>
          <div class="book-analysis-copy">
            <strong>{{ activeLibraryBook.title }}</strong>
            <span>{{ activeLibraryBook.author }}</span>
            <small
              >{{ activeLibraryBook.chapterTitles.length }} 章 ·
              {{ activeLibraryBook.sourceName }}</small
            >
          </div>
        </div>
        <div v-if="errorMsg" class="error-message">
          <AlertTriangle :size="16" aria-hidden="true" /> 分析出错:
          {{ errorMsg }}
        </div>
        <div v-if="backendError" class="error-message">
          <AlertTriangle :size="16" aria-hidden="true" /> 本地分析引擎启动失败:
          {{ backendError }}
        </div>
        <div v-else-if="!backendReady" class="backend-status" role="status">
          正在启动本地分析引擎，请稍候…
        </div>
        <AnalysisProgressPanel
          class="book-analysis-progress"
          :progress="analysisProgress"
          :active="isAnalyzing || bookAnalysisTransitioning"
        />
      </section>

      <Transition name="analysis-view">
        <div v-if="showInput && !activeLibraryBook" class="input-section">
          <div class="input-source-bar">
            <div>
              <strong>{{ inputMetadata.title || "Markdown 文本" }}</strong>
              <span>可直接粘贴文本，或从 EPUB 转换后继续编辑。</span>
            </div>
            <button
              class="icon-btn import-btn"
              :disabled="isAnalyzing || isImportingEpub"
              @click="importEpub"
            >
              <FileUp :size="16" aria-hidden="true" />
              {{ isImportingEpub ? "正在转换…" : "导入 EPUB" }}
            </button>
          </div>
          <textarea
            v-model="inputText"
            placeholder="在此粘贴日文文本，或点击“导入 EPUB”生成 Markdown..."
            class="raw-textarea"
            :disabled="isAnalyzing || isImportingEpub"
            :aria-busy="isAnalyzing || isImportingEpub"
          ></textarea>
          <div v-if="epubImportError" class="error-message">
            <AlertTriangle :size="16" aria-hidden="true" /> EPUB 导入失败:
            {{ epubImportError }}
          </div>
          <div v-if="errorMsg" class="error-message">
            <AlertTriangle :size="16" aria-hidden="true" /> 分析出错:
            {{ errorMsg }}
          </div>
          <div v-if="backendError" class="error-message">
            <AlertTriangle :size="16" aria-hidden="true" />
            本地分析引擎启动失败: {{ backendError }}
          </div>
          <div v-else-if="!backendReady" class="backend-status" role="status">
            正在启动本地分析引擎，请稍候…
          </div>
          <AnalysisProgressPanel
            :progress="analysisProgress"
            :active="isAnalyzing"
          />
          <div class="btn-group">
            <button
              class="analyze-btn"
              :disabled="isAnalyzing || isImportingEpub || !backendReady"
              @click="triggerAnalysis()"
            >
              {{
                isAnalyzing
                  ? analysisProgress.message
                  : backendReady
                    ? "解析生词胶囊"
                    : "正在启动分析引擎…"
              }}
            </button>
          </div>
        </div>
      </Transition>

      <AnalysisProgressPanel
        v-if="isReading"
        class="reader-progress-overlay"
        :progress="analysisProgress"
        :active="isAnalyzing"
      />

      <!-- 2. 阅读展示区域 -->
      <div
        v-if="isReading"
        ref="scrollContainerRef"
        class="reader-viewport"
        :style="readerStyle"
        @scroll="handleReaderScroll"
      >
        <div
          :style="{
            height: `${virtualizer.getTotalSize()}px`,
            width: '100%',
            position: 'relative',
          }"
        >
          <!-- 虚拟滚动段落渲染 -->
          <div
            v-for="virtualRow in virtualizer.getVirtualItems()"
            :key="readerRows[virtualRow.index].key"
            :style="{
              position: 'absolute',
              top: 0,
              left: '50%',
              transform: `translateY(${virtualRow.start}px) translateX(-50%)`,
            }"
            :data-index="virtualRow.index"
            :ref="
              (element) =>
                measureVirtualRow(element, readerRows[virtualRow.index].key)
            "
            :class="[
              'reader-row',
              readerRows[virtualRow.index].kind === 'text'
                ? 'paragraph-block'
                : 'reader-image-row',
              {
                'dialogue-block':
                  readerRows[virtualRow.index].kind === 'text' &&
                  textRowAt(virtualRow.index).paragraph.isDialogue,
                'reader-heading-row':
                  readerRows[virtualRow.index].kind === 'text' &&
                  textRowAt(virtualRow.index).heading,
              },
              readerRows[virtualRow.index].kind === 'text' &&
              textRowAt(virtualRow.index).heading
                ? `heading-level-${textRowAt(virtualRow.index).heading?.level || 2}`
                : '',
            ]"
            @pointerover="
              readerRows[virtualRow.index].kind === 'text' &&
              explanationInteraction.handleParagraphPointerOver($event)
            "
            @pointerout="
              readerRows[virtualRow.index].kind === 'text' &&
              explanationInteraction.handleParagraphPointerOut($event)
            "
            @mousedown="
              readerRows[virtualRow.index].kind === 'text' &&
              handleMouseDown($event, textRowAt(virtualRow.index).paragraph.id)
            "
            @mousemove="
              readerRows[virtualRow.index].kind === 'text' &&
              handleMouseMove($event, textRowAt(virtualRow.index).paragraph.id)
            "
            @click="
              readerRows[virtualRow.index].kind === 'text' &&
              handleParagraphClick(
                $event,
                textRowAt(virtualRow.index).paragraph.id,
              )
            "
            @contextmenu.prevent="
              readerRows[virtualRow.index].kind === 'text' &&
              handleParagraphContextMenu(
                $event,
                textRowAt(virtualRow.index).paragraph.id,
              )
            "
          >
            <ReaderImageBlock
              v-if="readerRows[virtualRow.index].kind === 'image'"
              :src="imageRowAt(virtualRow.index).resolvedSrc"
              :alt="imageRowAt(virtualRow.index).image.alt"
              :title="imageRowAt(virtualRow.index).image.title"
              :width="imageRowAt(virtualRow.index).intrinsicWidth"
              :height="imageRowAt(virtualRow.index).intrinsicHeight"
              @settled="measureSettledImage(imageRowAt(virtualRow.index).key)"
            />
            <template
              v-else-if="
                textRowAt(virtualRow.index).paragraph.tokens.length > 0
              "
            >
              <template
                v-for="(token, tokenIndex) in textRowAt(virtualRow.index)
                  .paragraph.tokens"
                :key="tokenIndex"
              >
                <BunsetsuCapsule
                  :token="token"
                  :paragraphId="textRowAt(virtualRow.index).paragraph.id"
                  :tokenIndex="tokenIndex"
                  :isDragSelected="
                    isTokenDragSelected(
                      textRowAt(virtualRow.index).paragraph.id,
                      tokenIndex,
                    )
                  "
                  :tokens="textRowAt(virtualRow.index).paragraph.tokens"
                />
              </template>
            </template>
            <template v-else>
              <span class="empty-line-placeholder">&nbsp;</span>
            </template>
          </div>
        </div>
      </div>
    </div>

    <ReaderNavigationPanel
      :show="isReading && showNavigation"
      :chapters="readerDocument?.chapters || []"
      :current-id="currentChapter?.id"
      @close="showNavigation = false"
      @navigate="navigateChapter"
    />
    <ReaderAppearancePanel
      :show="isReading && showAppearance"
      :appearance="readerAppearance"
      @close="showAppearance = false"
      @update="updateReaderAppearance"
    />
    <ReaderProgressBar
      v-if="isReading"
      :percent="progressEstimate.percent"
      :current-chapter="currentChapter?.title || ''"
      :remaining-label="
        formatReadingDuration(progressEstimate.remainingMinutes)
      "
      :completion-label="progressEstimate.completionLabel"
    />

    <!-- 3. 词典浮层组与独立语法说明 -->
    <ExplanationPopover
      :show="explanation.renderGate.value.dictionary"
      :anchor="explanation.anchorRect.value"
      :component-anchor="
        explanation.hasWholePanel.value
          ? explanation.anchorRect.value
          : explanation.componentAnchorRect.value
      "
      :whole-token="explanation.wholeToken.value"
      :whole-lookup="explanation.wholeLookup.value"
      :whole-loading="explanation.wholeLoading.value"
      :whole-can-go-back="explanation.wholeHistory.value.length > 0"
      :component-token="explanation.componentToken.value"
      :component-lookup="explanation.componentLookup.value"
      :component-loading="explanation.componentLoading.value"
      :component-can-go-back="explanation.componentHistory.value.length > 0"
      :component-label="explanation.componentLabel.value"
      @enter="explanationInteraction.handlePopoverEnter"
      @leave="explanationInteraction.handlePopoverLeave"
      @navigate-whole="explanation.navigateWhole"
      @navigate-component="explanation.navigateComponent"
      @select-whole="explanation.selectWhole"
      @select-component="explanation.selectComponent"
      @back-whole="explanation.backWhole"
      @back-component="explanation.backComponent"
    />
    <GrammarPopover
      :show="explanation.renderGate.value.grammar"
      :tag="explanation.grammarTag.value"
      :anchor="explanation.grammarAnchorRect.value"
      @enter="explanationInteraction.handlePopoverEnter"
      @leave="explanationInteraction.handlePopoverLeave"
      @open-dictionary="openGrammarDictionary"
    />

    <!-- 4. 右键上下文操作菜单 -->
    <ContextMenu
      :show="contextMenuShow"
      :x="contextMenuX"
      :y="contextMenuY"
      :token="contextMenuToken"
      :paragraphId="contextMenuParagraphId"
      :tokenIndex="contextMenuTokenIndex"
      :candidates="contextMenuCandidates"
      :candidatesLoading="candidatesLoading"
      @close="contextMenuShow = false"
      @mark-known="markAsKnown"
      @mark-unknown="markAsUnknown"
      @view-definition="viewFullDefinition"
      @load-candidates="loadContextCandidates"
      @apply-candidate="applyContextCandidate"
    />

    <!-- 5. 生词导出侧边栏 -->
    <ExportPanel
      :show="showExportPanel"
      :selectedKeys="selectedKeys"
      :paragraphs="paragraphs"
      @close="showExportPanel = false"
      @remove-key="removeSelectedKey"
      @clear-all="clearAllSelections"
      @update-note="updateNote"
      @export="executeExport"
    />

    <RuleWorkbench
      :show="showRuleWorkbench"
      :initialView="ruleWorkbenchView"
      :rules="expressionRules"
      :tokens="expressionDraft"
      :startMorphemeIdx="expressionDraftMorphemeRange.startMorphemeIdx"
      :endMorphemeIdx="expressionDraftMorphemeRange.endMorphemeIdx"
      :previewRule="previewExpressionRule"
      @close="showRuleWorkbench = false"
      @delete="removeExpressionRule"
      @save="saveExpressionDraft"
    />

    <GrammarLibraryPanel
      :show="showGrammarLibrary"
      @close="showGrammarLibrary = false"
    />

    <DictionarySettingsPanel
      :show="showDictionarySettings"
      :settings="dictionarySettings"
      @close="showDictionarySettings = false"
      @reorder="updateDictionaryOrder"
    />

    <!-- 6. 详细词典释义弹窗 (Modal) -->
    <Transition name="fade">
      <div
        v-if="showDefinitionModal"
        class="modal-overlay"
        @click="showDefinitionModal = false"
      >
        <div class="modal-card" @click.stop>
          <div class="modal-header">
            <h3>{{ activeWordForModal }} 完整词典释义</h3>
            <button
              class="modal-close"
              aria-label="关闭完整释义"
              @click="showDefinitionModal = false"
            >
              <X :size="19" aria-hidden="true" />
            </button>
          </div>
          <div class="modal-body no-scrollbar">
            <div v-if="modalDefinitions.length === 0" class="no-defs">
              未检索到本地 SQLite 词库中关于该词的完整释义。
            </div>
            <div v-else class="modal-defs-container">
              <div
                v-for="(def, idx) in modalDefinitions"
                :key="idx"
                class="dict-section"
              >
                <div class="dict-name">
                  <BookMarked :size="15" aria-hidden="true" />
                  {{ def.dict_name }}
                </div>
                <DictionaryContent :entry="def" />
              </div>
            </div>
          </div>
        </div>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.reader-container {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  width: 100vw;
  background-color: var(--bg-primary);
}

.app-header {
  flex: 0 0 auto;
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 24px;
  background: var(--glass-bg);
  backdrop-filter: var(--glass-filter);
  border-bottom: 1px solid var(--border-color);
  z-index: 10;
  min-height: 58px;
}

.logo-title {
  display: flex;
  flex: 1 1 auto;
  align-items: center;
  gap: 8px;
  min-width: 0;
  overflow: hidden;
}

.header-back {
  display: grid;
  width: 32px;
  height: 32px;
  flex: 0 0 auto;
  place-items: center;
  border: 0;
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
}

.logo-icon {
  flex: 0 0 auto;
  color: var(--accent-color);
}

.logo-text {
  font-size: 1.25rem;
  font-weight: bold;
  color: var(--accent-color);
}

.logo-sub {
  font-size: 0.75rem;
  color: var(--text-muted);
  border-left: 1px solid var(--border-color);
  padding-left: 8px;
}

.action-bar {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  gap: 7px;
  flex-wrap: nowrap;
  justify-content: flex-end;
}

.chapter-button {
  max-width: 170px;
  overflow: hidden;
  text-overflow: ellipsis;
}

.icon-btn.compact-tool {
  position: relative;
  width: 38px;
  height: 36px;
  flex: 0 0 auto;
  justify-content: center;
  padding: 0;
}

.tool-count {
  position: absolute;
  top: -5px;
  right: -5px;
  min-width: 17px;
  height: 17px;
  padding: 0 4px;
  border-radius: 9px;
  background: var(--accent-color);
  color: white;
  font-size: 0.62rem;
  font-variant-numeric: tabular-nums;
  line-height: 17px;
}

.dev-metrics-entry {
  position: relative;
}

.dev-metrics-popover {
  position: absolute;
  z-index: 100;
  top: calc(100% + 8px);
  right: 0;
  display: none;
  width: 270px;
  padding: 13px 14px;
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  background: var(--bg-primary);
  box-shadow: var(--shadow-md);
  color: var(--text-secondary);
}

.dev-metrics-entry:hover .dev-metrics-popover,
.dev-metrics-entry:focus-within .dev-metrics-popover {
  display: block;
}

.dev-metrics-popover > strong {
  display: block;
  margin-bottom: 8px;
  color: var(--text-primary);
  font-size: 0.8rem;
}

.dev-metrics-popover dl,
.dev-metrics-popover dl div {
  display: grid;
}

.dev-metrics-popover dl {
  gap: 5px;
}

.dev-metrics-popover dl div {
  grid-template-columns: 76px minmax(0, 1fr);
  gap: 8px;
  font-size: 0.7rem;
}

.dev-metrics-popover dt {
  color: var(--text-muted);
}

.dev-metrics-popover dd {
  overflow-wrap: anywhere;
  font-variant-numeric: tabular-nums;
  text-align: right;
}

.icon-btn {
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  color: var(--text-secondary);
  padding: 6px 14px;
  border-radius: var(--radius-sm);
  cursor: pointer;
  font-weight: 500;
  display: flex;
  align-items: center;
  gap: 6px;
  transition: all 0.2s ease;
  box-shadow: none;
  white-space: nowrap;
  font-size: 0.85rem;
}

.icon-btn:hover {
  border-color: var(--accent-color);
  color: var(--accent-color);
}

.icon-btn.active {
  background: var(--accent-light);
  border-color: var(--accent-color);
  color: var(--accent-color);
}

.icon-btn.highlight {
  background: var(--accent-color);
  color: white;
  border: none;
}

.icon-btn.highlight:hover {
  background: var(--accent-hover);
}

.main-layout {
  flex: 1;
  overflow: hidden;
  position: relative;
  display: flex;
  justify-content: center;
}

.reader-progress-overlay {
  position: absolute;
  z-index: 20;
  top: 12px;
  left: 50%;
  width: min(680px, calc(100% - 32px));
  transform: translateX(-50%);
}

.book-analysis-view {
  display: flex;
  width: min(720px, calc(100% - 48px));
  height: 100%;
  flex-direction: column;
  justify-content: center;
  gap: 26px;
  padding: 42px 0 72px;
}

.book-analysis-identity {
  display: flex;
  align-items: center;
  gap: 22px;
}

.book-analysis-cover {
  position: relative;
  isolation: isolate;
  display: grid;
  width: 86px;
  aspect-ratio: 3 / 4;
  flex: 0 0 auto;
  place-items: center;
  border: 1px solid var(--border-color);
  border-radius: 4px;
  background: var(--accent-light);
  color: var(--accent-color);
  view-transition-name: book-cover;
}

.book-analysis-cover::before {
  position: absolute;
  z-index: 0;
  inset: -20px;
  border-radius: 50%;
  background: color-mix(in srgb, var(--accent-color) 12%, transparent);
  content: "";
  animation: analysis-cover-halo 360ms cubic-bezier(0, 0, 0.2, 1) both;
}

.book-analysis-cover img {
  position: relative;
  z-index: 1;
  width: 100%;
  height: 100%;
  border-radius: inherit;
  object-fit: cover;
}

.book-analysis-cover > svg {
  position: relative;
  z-index: 1;
}

.book-analysis-copy {
  display: flex;
  min-width: 0;
  flex-direction: column;
  animation: analysis-content-enter 220ms 60ms cubic-bezier(0, 0, 0.2, 1) both;
}

.book-analysis-copy strong,
.book-analysis-copy span,
.book-analysis-copy small {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.book-analysis-copy strong {
  color: var(--text-primary);
  font-size: 1.08rem;
}

.book-analysis-copy span {
  color: var(--text-secondary);
  font-size: 0.82rem;
}

.book-analysis-copy small {
  margin-top: 5px;
  color: var(--text-muted);
  font-size: 0.72rem;
}

.book-analysis-progress,
.book-analysis-view > .error-message,
.book-analysis-view > .backend-status {
  animation: analysis-content-enter 240ms 90ms cubic-bezier(0, 0, 0.2, 1) both;
}

@keyframes analysis-cover-halo {
  0% {
    opacity: 0.7;
    transform: scale(0.55);
  }
  100% {
    opacity: 0;
    transform: scale(1.35);
  }
}

@keyframes analysis-content-enter {
  from {
    opacity: 0;
    transform: translateY(12px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

@media (max-width: 640px) {
  .book-analysis-view {
    width: min(100% - 32px, 720px);
    gap: 22px;
    padding-block: 32px 56px;
  }

  .book-analysis-identity {
    gap: 16px;
  }

  .book-analysis-cover {
    width: 72px;
  }
}

.analysis-view-enter-active {
  transition:
    opacity 220ms ease,
    transform 260ms cubic-bezier(0, 0, 0.2, 1);
}

.analysis-view-enter-from {
  opacity: 0;
  transform: translateY(12px);
}

/* 输入模块样式 */
.input-section {
  width: 100%;
  max-width: clamp(600px, 75vw, 960px);
  padding: 40px clamp(24px, 5vw, 64px);
  display: flex;
  flex-direction: column;
  gap: 20px;
  height: 100%;
}

.document-identity {
  display: flex;
  flex-direction: column;
  min-width: 0;
  max-width: min(46vw, 760px);
  border-left: 1px solid var(--border-color);
  padding-left: 10px;
  line-height: 1.25;
}

.document-identity strong,
.document-identity span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.document-identity strong {
  color: var(--text-primary);
  font-size: 0.86rem;
}

.document-identity span {
  color: var(--text-muted);
  font-size: 0.72rem;
}

.input-source-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  min-width: 0;
}

.input-source-bar > div {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.input-source-bar strong,
.input-source-bar span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.input-source-bar strong {
  color: var(--text-primary);
  font-size: 0.95rem;
}

.input-source-bar span {
  color: var(--text-muted);
  font-size: 0.78rem;
}

.import-btn {
  flex: 0 0 auto;
}

.import-btn:disabled {
  cursor: progress;
  opacity: 0.6;
}

.import-warning {
  margin-top: -10px;
  color: var(--text-secondary);
  font-size: 0.85rem;
  text-align: center;
}

@media (max-width: 760px) {
  .input-source-bar {
    align-items: stretch;
    flex-direction: column;
  }

  .import-btn {
    justify-content: center;
  }
}

.raw-textarea {
  flex: 1;
  border: 1px solid var(--border-color);
  border-radius: var(--radius-md);
  padding: 20px;
  background: var(--bg-secondary);
  color: var(--text-primary);
  resize: none;
  outline: none;
  font-size: 1.1rem;
  line-height: 1.6;
  font-family: var(--font-ja);
  box-shadow: inset var(--shadow-sm);
  transition: border-color 0.2s;
}

.raw-textarea:focus {
  border-color: var(--accent-color);
}

.raw-textarea:disabled {
  cursor: progress;
  opacity: 0.72;
}

.btn-group {
  display: flex;
  justify-content: center;
}

.analyze-btn {
  background-color: var(--accent-color);
  color: white;
  border: none;
  padding: 12px 40px;
  font-size: 1.1rem;
  border-radius: var(--radius-md);
  cursor: pointer;
  font-weight: bold;
  box-shadow: var(--shadow-md);
  transition: background-color 0.2s;
}

.analyze-btn:hover {
  background-color: var(--accent-hover);
}

.analyze-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.error-message {
  display: flex;
  align-items: center;
  gap: 7px;
  background-color: var(--novelty-high-bg);
  border: 1px solid var(--novelty-high-border);
  color: var(--novelty-high-text);
  padding: 12px;
  border-radius: var(--radius-sm);
  font-size: 0.9rem;
  margin-top: -10px;
  margin-bottom: 10px;
}

.backend-status {
  color: var(--text-secondary);
  font-size: 0.9rem;
  text-align: center;
}

/* 阅读器视口样式 */
.reader-viewport {
  flex: 1;
  width: 100%;
  overflow-y: auto;
  padding: 46px 0 78px;
  box-sizing: border-box;
}

.reader-row {
  width: min(var(--reader-content-width, 760px), calc(100% - 40px));
  box-sizing: border-box;
}

.reader-image-row {
  padding: 10px 0;
}

.reader-heading-row {
  padding-top: 42px;
  padding-bottom: calc(var(--reader-paragraph-gap, 24px) * 0.35);
  color: var(--text-primary);
  font-weight: 650;
  line-height: 1.55;
  text-align: left;
}

.reader-heading-row.heading-level-1 {
  font-size: calc(var(--reader-font-size, 19px) * 1.55);
}

.reader-heading-row.heading-level-2 {
  font-size: calc(var(--reader-font-size, 19px) * 1.35);
}

.reader-heading-row.heading-level-3,
.reader-heading-row.heading-level-4,
.reader-heading-row.heading-level-5,
.reader-heading-row.heading-level-6 {
  font-size: calc(var(--reader-font-size, 19px) * 1.16);
}

.reader-heading-row :deep(.bunsetsu-capsule) {
  cursor: text;
}

.reader-viewport::-webkit-scrollbar {
  width: 8px;
}

.reader-viewport::-webkit-scrollbar-track {
  background: transparent;
}

.reader-viewport::-webkit-scrollbar-thumb {
  background: var(--border-color);
  border-radius: 4px;
}

.reader-viewport::-webkit-scrollbar-thumb:hover {
  background: var(--text-muted);
}

@media (max-width: 1180px) {
  .dev-metrics,
  .compact-tool span {
    display: none;
  }

  .action-bar {
    gap: 7px;
  }

  .icon-btn {
    padding-right: 9px;
    padding-left: 9px;
  }
}

@media (max-width: 820px) {
  .app-header {
    padding-right: 12px;
    padding-left: 12px;
  }

  .action-bar
    .icon-btn:not(.chapter-button):not(.compact-tool):not(.highlight) {
    display: none;
  }

  .logo-text {
    display: none;
  }

  .document-identity {
    max-width: 34vw;
  }
}

/* 详细释义弹窗 */
.modal-overlay {
  position: fixed;
  top: 0;
  bottom: 0;
  left: 0;
  right: 0;
  background: rgba(0, 0, 0, 0.4);
  backdrop-filter: blur(4px);
  z-index: 1200;
  display: flex;
  align-items: center;
  justify-content: center;
}

.modal-card {
  width: 90%;
  max-width: 600px;
  height: 70vh;
  background: var(--bg-primary);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-md);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.modal-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 16px 24px;
  border-bottom: 1px solid var(--border-color);
  background-color: var(--bg-secondary);
}

.modal-close {
  display: grid;
  place-items: center;
  background: transparent;
  border: none;
  font-size: 1.5rem;
  cursor: pointer;
  color: var(--text-secondary);
  box-shadow: none;
}

.modal-body {
  flex: 1;
  overflow-y: auto;
  padding: 24px;
}

.no-defs {
  text-align: center;
  color: var(--text-muted);
  margin-top: 40px;
  font-style: italic;
}

.dict-section {
  margin-bottom: 24px;
  border-bottom: 1px dashed var(--border-color);
  padding-bottom: 16px;
}

.dict-section:last-child {
  border-bottom: none;
}

.dict-name {
  display: flex;
  align-items: center;
  gap: 6px;
  font-weight: bold;
  color: var(--accent-color);
  margin-bottom: 10px;
  font-size: 1.05rem;
}

.dict-content {
  font-size: 0.9rem;
  line-height: 1.6;
  color: var(--text-secondary);
}

.empty-line-placeholder {
  user-select: none;
  visibility: hidden;
}

/* Modal 渐变过渡 */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

@media (prefers-reduced-motion: reduce) {
  .book-analysis-cover::before {
    display: none;
  }

  .book-analysis-copy,
  .book-analysis-progress,
  .book-analysis-view > .error-message,
  .book-analysis-view > .backend-status {
    animation: none;
  }

  .analysis-view-enter-active {
    transition: none;
  }
}
</style>

<style>
::view-transition-group(book-cover) {
  z-index: 40;
  animation-duration: 300ms;
  animation-timing-function: cubic-bezier(0.4, 0, 0.2, 1);
}

::view-transition-old(book-cover),
::view-transition-new(book-cover) {
  height: 100%;
  overflow: clip;
  border-radius: 4px;
  mix-blend-mode: normal;
}

/* MDict 渲染的富文本样式过滤 */
.html-content span,
.html-content div {
  background-color: transparent !important;
}
.html-content {
  font-family: sans-serif;
}
</style>
