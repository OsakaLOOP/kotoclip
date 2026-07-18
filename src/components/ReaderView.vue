<script setup lang="ts">
import { ref, computed, watch, onMounted, onBeforeUnmount, nextTick } from "vue";
import { AlertTriangle, BookMarked, BookOpen, BriefcaseBusiness, Link2, Library, Moon, Plus, Settings2, X } from "@lucide/vue";
import type { ComponentPublicInstance } from "vue";
import { useVirtualizer } from "@tanstack/vue-virtual";
import { useTokenization } from "../composables/useTokenization";
import { useSelection } from "../composables/useSelection";
import { useDictionary } from "../composables/useDictionary";
import { useDragMerge } from "../composables/useDragMerge";
import { useScrollFocus } from "../composables/useScrollFocus";
import { DictEntry, ExpressionBoundaryEffect, ExpressionRule, ExpressionType, SegmentationCandidate, AnnotatedToken, GrammarDictionaryTarget } from "../types";

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
import { dictionaryTargetForToken } from "../utils/dictionaryTarget";
import { useExplanationSession } from "../composables/useExplanationSession";
import { useExplanationInteraction } from "../composables/useExplanationInteraction";

// 状态定义
const inputText = ref("");
const showInput = ref(true);
const einkMode = ref(false);
const showDevMetrics = import.meta.env.DEV || import.meta.env.VITE_SHOW_DEV_METRICS === "true";
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
  addExpressionRule,
  previewExpressionRule,
  getExpressionRules,
  deleteExpressionRule,
  refreshDocumentExpressions,
  markDocumentKnown,
  getCandidates,
  chooseSegmentation,
} = useTokenization();
const { selectedKeys, toggleSelect, markAsKnown, markAsUnknown, exportSelected, updateNote } = useSelection(paragraphs, markDocumentKnown);
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
    return paragraphs.value.find((item) => item.id === paragraphId)?.tokens[tokenIndex];
  },
  session: explanation,
});

function openGrammarDictionary(target: GrammarDictionaryTarget) {
  for (const paragraph of paragraphs.value) {
    for (let tokenIndex = 0; tokenIndex < paragraph.tokens.length; tokenIndex++) {
      const token = paragraph.tokens[tokenIndex];
      const morphemeIndex = token.bunsetsu.morphemes.findIndex((morpheme) =>
        morpheme.char_range[0] === target.char_range[0] && morpheme.char_range[1] === target.char_range[1],
      );
      if (morphemeIndex < 0) continue;
      const capsule = document.querySelector<HTMLElement>(
        `[data-paragraph-id="${paragraph.id}"][data-token-index="${tokenIndex}"]`,
      );
      const morpheme = capsule?.querySelector<HTMLElement>(`[data-morpheme-index="${morphemeIndex}"]`);
      if (!capsule || !morpheme) return;
      explanation.focusMorpheme({ paragraphId: paragraph.id, tokenIndex, morphemeIndex }, token, capsule, morpheme);
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
const expressionDraftMorphemeRange = ref({ startMorphemeIdx: 0, endMorphemeIdx: 0 });
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
  if (!showInput.value && inputText.value.trim()) {
    await refreshDocumentExpressions();
  }
}

async function saveExpressionDraft(
  label: string,
  description: string,
  bunsetsuStates: ('fixed' | 'slot' | 'any')[],
  morphemeMasks: boolean[][],
  gapAfter: number | null,
  expressionType: ExpressionType,
  priority: number,
  boundaryEffect: ExpressionBoundaryEffect
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
      boundaryEffect
    );
    showRuleWorkbench.value = false;
    expressionDraft.value = [];
    expressionDraftMorphemeRange.value = { startMorphemeIdx: 0, endMorphemeIdx: 0 };
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
} = useDragMerge(paragraphs, async (tokens, _paragraphId, startMorphemeIdx, endMorphemeIdx) => {
  expressionDraft.value = tokens.filter(t => t.display_class === "content");
  expressionDraftMorphemeRange.value = { startMorphemeIdx, endMorphemeIdx };
  ruleWorkbenchView.value = "editor";
  showRuleWorkbench.value = true;
  try {
    expressionRules.value = await getExpressionRules();
  } catch (error) {
    console.error("Rule catalog load error:", error);
  }
});

// 使用 @tanstack/vue-virtual 虚拟滚动
const virtualizer = useVirtualizer(
  computed(() => ({
    count: paragraphs.value.length,
    getScrollElement: () => scrollContainerRef.value,
    estimateSize: () => 70, // 估计的段落高度
    overscan: 5,
  }))
);

function measureVirtualRow(element: Element | ComponentPublicInstance | null) {
  if (element instanceof Element) {
    virtualizer.value.measureElement(element);
  }
}

// 新文档进入阅读态时归零；后台 Patch 只重新测量，不能打断用户滚动位置。
watch(
  showInput,
  async (visible) => {
    explanation.closeAll();
    if (visible) return;
    await nextTick();
    virtualizer.value.measure();
    virtualizer.value.scrollToOffset(0);
    triggerUpdate();
  },
  { flush: "post" }
);

watch(
  paragraphs,
  async () => {
    if (showInput.value) return;
    await nextTick();
    virtualizer.value.measure();
    triggerUpdate();
  },
  { flush: "post" }
);

let rangePrefetchPending = false;

async function prefetchNextMissingRange() {
  const container = scrollContainerRef.value;
  if (!container || documentComplete.value || rangePrefetchPending) return;
  const nearEnd = container.scrollTop + container.clientHeight >= container.scrollHeight - container.clientHeight * 2;
  if (!nearEnd) return;
  const loadedEnd = availableRanges.value.reduce(
    (end, range) => range[0] <= end ? Math.max(end, range[1]) : end,
    0
  );
  if (loadedEnd >= documentCharRange.value[1]) return;
  rangePrefetchPending = true;
  try {
    await requestDocumentRange([loadedEnd, Math.min(documentCharRange.value[1], loadedEnd + 4_000)]);
  } finally {
    rangePrefetchPending = false;
  }
}

async function handleReaderScroll() {
  explanation.refreshAnchor();
  void prefetchNextMissingRange().catch((error) => {
    console.error("Viewport range prefetch failed:", error);
  });
  await nextTick();
  explanation.refreshAnchor();
}

// 监听拖拽的鼠标松开事件 (挂载在 window 以防在胶囊外松开失效)
onMounted(() => {
  void initializeBackendStatus();
  window.addEventListener("mouseup", handleMouseUp);
  window.addEventListener("resize", explanation.refreshAnchor);
});

watch(backendReady, (ready) => {
  if (!ready) return;
  void loadDictionarySettings().catch((error) => {
    console.error("Dictionary settings load error:", error);
  });
}, { immediate: true });

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
  disposeBackendStatusListener();
  window.removeEventListener("mouseup", handleMouseUp);
  window.removeEventListener("resize", explanation.refreshAnchor);
  explanation.closeAll();
});

// 执行文本分析
async function triggerAnalysis(recordExposure = true) {
  if (!inputText.value.trim()) return;
  const sourceText = inputText.value;
  const startedAt = performance.now();
  const succeeded = await analyzeText(sourceText, recordExposure);
  if (succeeded) {
    const renderSetupStartedAt = performance.now();
    showInput.value = false;
    await nextTick();
    virtualizer.value.measure();
    triggerUpdate();
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

// 事件委托：段落内的点击 (切换选中/已知)
function handleParagraphClick(e: MouseEvent, paragraphId: number) {
  // 右键菜单显示或拖拽期间，不切换导出选择。
  if (contextMenuShow.value || isDragging.value) return;

  const target = e.target as HTMLElement;
  const capsuleEl = target.closest("[data-token-index]") as HTMLElement;
  if (!capsuleEl) return;

  const tokenIndex = parseInt(capsuleEl.getAttribute("data-token-index") || "", 10);
  if (isNaN(tokenIndex)) return;

  const p = paragraphs.value.find((para) => para.id === paragraphId);
  const token = p?.tokens[tokenIndex];
  const isPunc = token && (token.display_class === "punctuation" || token.display_class === "line_break");
  if (!token || isPunc) return;

  // 切换该 token 选中状态 (用于 Anki 导出)
  toggleSelect(paragraphId, tokenIndex);
}

// 右键仅保留词条操作和 N-best 分词候选；词汇与语素解释统一由悬浮进入。
function handleParagraphContextMenu(e: MouseEvent, paragraphId: number) {
  const target = e.target as HTMLElement;
  const capsuleEl = target.closest("[data-token-index]") as HTMLElement;
  if (!capsuleEl) return;
  const tokenIndex = parseInt(capsuleEl.getAttribute("data-token-index") || "", 10);
  if (isNaN(tokenIndex)) return;
  const p = paragraphs.value.find((para) => para.id === paragraphId);
  const token = p?.tokens[tokenIndex];
  const isPunc = token && (token.display_class === "punctuation" || token.display_class === "line_break");
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
    contextMenuCandidates.value = await getCandidates(contextMenuToken.value, 5);
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

  const lookup = await lookupWord(target.word, target.reading, false, target.pos);
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
    const jsonStr = await exportSelected(inputText.value, async (word, reading) => {
      return (await lookupWord(word, reading))?.entries ?? [];
    });

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
        <BookOpen class="logo-icon" :size="24" stroke-width="1.8" aria-hidden="true" />
        <span class="logo-text">Kotoclip</span>
        <span class="logo-sub">日文生词胶囊阅读器</span>
      </div>
      <div class="action-bar">
        <div
          v-if="showDevMetrics && analysisMetrics"
          class="dev-metrics"
          aria-label="开发者分析指标"
        >
          <span>{{ analysisMetrics.characterCount }} 字</span>
          <span>{{ lastOpenCacheHit ? '暖缓存' : (documentComplete ? '已补全' : '渐进') }}</span>
          <span>{{ availableRanges[availableRanges.length - 1]?.[1] ?? 0 }}/{{ documentCharRange[1] }} 字</span>
          <span>{{ Math.round(lastPatchBytes / 1024) }} KB</span>
          <span v-if="lastInvalidation">
            {{ lastInvalidation.reason }} {{ lastInvalidation.recomputedCharacters }}/{{ lastInvalidation.totalCharacters }}
          </span>
          <span :title="`监听 ${analysisMetrics.listenerSetupMs} ms；后端 ${analysisMetrics.backendDurationMs} ms；IPC/解析 ${analysisMetrics.ipcAndParseMs} ms；IPC传输+后端 ${analysisMetrics.invokeAndTransferMs} ms；组段 ${analysisMetrics.paragraphBuildMs} ms；首帧布局/绘制 ${analysisMetrics.renderSetupMs} ms`">
            {{ analysisMetrics.durationMs }} ms
          </span>
        </div>
        <button class="icon-btn" :class="{ active: showExportPanel }" @click="showExportPanel = !showExportPanel">
          <BriefcaseBusiness :size="16" aria-hidden="true" /> 导出本 ({{ selectedKeys.length }})
        </button>
        <button class="icon-btn" :class="{ active: showRuleWorkbench }" @click="openExpressionRules">
          <Link2 :size="16" aria-hidden="true" /> 规则
        </button>
        <button class="icon-btn" :class="{ active: showGrammarLibrary }" @click="showGrammarLibrary = true">
          <Library :size="16" aria-hidden="true" /> 文法库
        </button>
        <button class="icon-btn" :class="{ active: showDictionarySettings }" @click="showDictionarySettings = true">
          <Settings2 :size="16" aria-hidden="true" /> 词典
        </button>
        <button class="icon-btn" :class="{ active: einkMode }" @click="toggleEinkMode">
          <Moon :size="16" aria-hidden="true" /> 墨水屏
        </button>
        <button v-if="!showInput" class="icon-btn highlight" @click="showInput = true">
          <Plus :size="16" aria-hidden="true" /> 输入文本
        </button>
      </div>
    </header>

    <!-- 主布局 -->
    <div class="main-layout">
      <!-- 1. 文本输入模块 -->
      <div v-if="showInput" class="input-section">
        <textarea
          v-model="inputText"
          placeholder="在此粘贴整页日文文本..."
          class="raw-textarea"
          :disabled="isAnalyzing"
          :aria-busy="isAnalyzing"
        ></textarea>
        <div v-if="errorMsg" class="error-message">
          <AlertTriangle :size="16" aria-hidden="true" /> 分析出错: {{ errorMsg }}
        </div>
        <div v-if="backendError" class="error-message">
          <AlertTriangle :size="16" aria-hidden="true" /> 本地分析引擎启动失败: {{ backendError }}
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
            :disabled="isAnalyzing || !backendReady"
            @click="triggerAnalysis()"
          >
            {{ isAnalyzing ? analysisProgress.message : backendReady ? '解析生词胶囊' : '正在启动分析引擎…' }}
          </button>
        </div>
      </div>

      <AnalysisProgressPanel
        v-if="!showInput"
        class="reader-progress-overlay"
        :progress="analysisProgress"
        :active="isAnalyzing"
      />

      <!-- 2. 阅读展示区域 -->
      <div
        v-if="!showInput"
        ref="scrollContainerRef"
        class="reader-viewport no-scrollbar"
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
            :key="virtualRow.index"
            :style="{
              position: 'absolute',
              top: 0,
              left: 0,
              width: '100%',
              transform: `translateY(${virtualRow.start}px)`,
            }"
            :data-index="virtualRow.index"
            :ref="measureVirtualRow"
            :class="['paragraph-block', { 'dialogue-block': paragraphs[virtualRow.index].isDialogue }]"
            @pointerover="explanationInteraction.handleParagraphPointerOver"
            @pointerout="explanationInteraction.handleParagraphPointerOut"
            @mousedown="handleMouseDown($event, paragraphs[virtualRow.index].id)"
            @mousemove="handleMouseMove($event, paragraphs[virtualRow.index].id)"
            @click="handleParagraphClick($event, paragraphs[virtualRow.index].id)"
            @contextmenu.prevent="handleParagraphContextMenu($event, paragraphs[virtualRow.index].id)"
          >
            <template v-if="paragraphs[virtualRow.index].tokens.length > 0">
              <template v-for="(token, tokenIndex) in paragraphs[virtualRow.index].tokens" :key="tokenIndex">
                <BunsetsuCapsule
                  :token="token"
                  :paragraphId="paragraphs[virtualRow.index].id"
                  :tokenIndex="tokenIndex"
                  :isDragSelected="isTokenDragSelected(paragraphs[virtualRow.index].id, tokenIndex)"
                  :tokens="paragraphs[virtualRow.index].tokens"
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

    <!-- 3. 词典浮层组与独立语法说明 -->
    <ExplanationPopover
      :show="explanation.renderGate.value.dictionary"
      :anchor="explanation.anchorRect.value"
      :component-anchor="explanation.hasWholePanel.value ? explanation.anchorRect.value : explanation.componentAnchorRect.value"
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
      <div v-if="showDefinitionModal" class="modal-overlay" @click="showDefinitionModal = false">
        <div class="modal-card" @click.stop>
          <div class="modal-header">
            <h3>{{ activeWordForModal }} 完整词典释义</h3>
            <button class="modal-close" aria-label="关闭完整释义" @click="showDefinitionModal = false"><X :size="19" aria-hidden="true" /></button>
          </div>
          <div class="modal-body no-scrollbar">
            <div v-if="modalDefinitions.length === 0" class="no-defs">
              未检索到本地 SQLite 词库中关于该词的完整释义。
            </div>
            <div v-else class="modal-defs-container">
              <div v-for="(def, idx) in modalDefinitions" :key="idx" class="dict-section">
                <div class="dict-name"><BookMarked :size="15" aria-hidden="true" /> {{ def.dict_name }}</div>
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
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 24px;
  background: var(--glass-bg);
  backdrop-filter: var(--glass-filter);
  border-bottom: 1px solid var(--border-color);
  z-index: 10;
}

.logo-title {
  display: flex;
  align-items: center;
  gap: 8px;
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
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
  justify-content: flex-end;
}

.dev-metrics {
  display: flex;
  gap: 8px;
  color: var(--text-muted);
  font-size: 0.72rem;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.dev-metrics span + span::before {
  content: "/";
  margin-right: 8px;
  color: var(--border-color);
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
  max-width: clamp(600px, 75vw, 960px);
  width: 100%;
  overflow-y: auto;
  padding: 40px clamp(24px, 5vw, 64px);
  box-sizing: border-box;
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
</style>

<style>
/* MDict 渲染的富文本样式过滤 */
.html-content span, .html-content div {
  background-color: transparent !important;
}
.html-content {
  font-family: sans-serif;
}
</style>
