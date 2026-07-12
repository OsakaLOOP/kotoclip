<script setup lang="ts">
import { ref, computed, watch, onMounted, onBeforeUnmount, nextTick } from "vue";
import type { ComponentPublicInstance } from "vue";
import { useVirtualizer } from "@tanstack/vue-virtual";
import { useTokenization } from "../composables/useTokenization";
import { useSelection } from "../composables/useSelection";
import { useDictionary } from "../composables/useDictionary";
import { useDragMerge } from "../composables/useDragMerge";
import { useScrollFocus } from "../composables/useScrollFocus";
import { DictEntry, DictionaryLookup, ExpressionAnnotation, ExpressionBoundaryEffect, ExpressionRule, ExpressionType, SegmentationCandidate, AnnotatedToken } from "../types";

import BunsetsuCapsule from "./BunsetsuCapsule.vue";
import TooltipPanel from "./TooltipPanel.vue";
import ContextMenu from "./ContextMenu.vue";
import ExportPanel from "./ExportPanel.vue";
import AnalysisProgressPanel from "./AnalysisProgressPanel.vue";
import ExpressionRulesPanel from "./ExpressionRulesPanel.vue";
import ExpressionRuleEditor from "./ExpressionRuleEditor.vue";
import DictionaryContent from "./dictionary/DictionaryContent.vue";

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
  addExpressionRule,
  getExpressionRules,
  deleteExpressionRule,
  splitToken,
  getCandidates,
  chooseSegmentation,
} = useTokenization();
const { selectedKeys, toggleSelect, markAsKnown, markAsUnknown, exportSelected, updateNote } = useSelection(paragraphs);
const { lookupWord, chooseDictionaryTarget } = useDictionary();

// 详细释义弹窗状态
const showDefinitionModal = ref(false);
const activeWordForModal = ref("");
const modalDefinitions = ref<DictEntry[]>([]);

// 语法释义 Tooltip 状态
const tooltipShow = ref(false);
const tooltipX = ref(0);
const tooltipY = ref(0);
const tooltipPlacement = ref<"above" | "below">("above");
const tooltipToken = ref<any | null>(null);
const tooltipLookup = ref<DictionaryLookup | null>(null);
const tooltipLoading = ref(false);
let tooltipTimeout: number | null = null;
let tooltipRequestId = 0;
let tooltipPanelHovered = false;
const tooltipHistory = ref<DictionaryLookup[]>([]);

function cancelTooltipClose() {
  if (tooltipTimeout) window.clearTimeout(tooltipTimeout);
  tooltipTimeout = null;
}

function scheduleTooltipClose(delay = 180) {
  cancelTooltipClose();
  tooltipTimeout = window.setTimeout(() => {
    if (!tooltipPanelHovered) {
      tooltipShow.value = false;
      tooltipLookup.value = null;
    }
  }, delay);
}

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
const showExpressionRules = ref(false);
const expressionRules = ref<ExpressionRule[]>([]);
const expressionDraft = ref<AnnotatedToken[]>([]);
const expressionDraftMorphemeRange = ref({ startMorphemeIdx: 0, endMorphemeIdx: 0 });
const showExpressionEditor = ref(false);

async function openExpressionRules() {
  expressionRules.value = await getExpressionRules();
  showExpressionRules.value = true;
}

async function removeExpressionRule(id: number) {
  await deleteExpressionRule(id);
  expressionRules.value = await getExpressionRules();
  if (!showInput.value && inputText.value.trim()) {
    await triggerAnalysis(false);
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
    showExpressionEditor.value = false;
    expressionDraft.value = [];
    expressionDraftMorphemeRange.value = { startMorphemeIdx: 0, endMorphemeIdx: 0 };
    await triggerAnalysis(false);
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
  showExpressionEditor.value = true;
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

// 结果视图由 v-if 延迟挂载。必须等滚动容器进入 DOM 后再测量，
// 否则首次分析时 virtualizer 会保留空的可见区间。
watch(
  [paragraphs, showInput],
  async () => {
    if (showInput.value) return;
    await nextTick();
    virtualizer.value.measure();
    virtualizer.value.scrollToOffset(0);
    triggerUpdate();
  },
  { flush: "post" }
);

// 监听拖拽的鼠标松开事件 (挂载在 window 以防在胶囊外松开失效)
onMounted(() => {
  window.addEventListener("mouseup", handleMouseUp);
});

onBeforeUnmount(() => {
  window.removeEventListener("mouseup", handleMouseUp);
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
  }
}

// 事件委托：段落级别的 mouseover 悬浮处理 (200ms 延迟 Tooltip 显示)
async function handleParagraphMouseOver(e: MouseEvent, paragraphId: number) {
  const target = e.target as HTMLElement;
  const capsuleEl = target.closest("[data-token-index]") as HTMLElement;
  const relatedCapsule = (e.relatedTarget as HTMLElement | null)?.closest?.("[data-token-index]");
  if (capsuleEl && relatedCapsule === capsuleEl) return;
  
  cancelTooltipClose();

  if (!capsuleEl) {
    // 鼠标离开了胶囊，100ms 后关闭
    scheduleTooltipClose();
    return;
  }

  const tokenIndex = parseInt(capsuleEl.getAttribute("data-token-index") || "", 10);
  if (isNaN(tokenIndex)) return;

  const p = paragraphs.value.find((para) => para.id === paragraphId);
  const token = p?.tokens[tokenIndex];

  const isPunc = token && (token.display_class === "punctuation" || token.display_class === "line_break");
  if (!token || token.is_known || isPunc) {
    ++tooltipRequestId;
    tooltipShow.value = false;
    return;
  }

  // 延时 200ms 显示 Tooltip
  tooltipTimeout = window.setTimeout(async () => {
    // 计算悬浮框展示坐标 (相对于视口，防止溢出)
    const rect = capsuleEl.getBoundingClientRect();
    const tooltipHalfWidth = Math.min(230, Math.max(0, window.innerWidth / 2 - 12));
    tooltipX.value = Math.min(
      window.innerWidth - tooltipHalfWidth,
      Math.max(tooltipHalfWidth, rect.left + rect.width / 2)
    );
    tooltipPlacement.value = rect.top >= 340 ? "above" : "below";
    tooltipY.value = tooltipPlacement.value === "above" ? rect.top : rect.bottom;

    tooltipToken.value = token;
    tooltipPanelHovered = false;
    tooltipShow.value = true;
    tooltipLookup.value = null;
    tooltipHistory.value = [];
    tooltipLoading.value = true;
    const requestId = ++tooltipRequestId;

    // 异步查询词典释义摘要
    const lookup = await lookupWord(token.bunsetsu.head_word.base_form, token.bunsetsu.head_word.reading);
    if (requestId === tooltipRequestId && tooltipToken.value === token) {
      tooltipLookup.value = lookup;
      tooltipLoading.value = false;
    }
  }, 200);
}

// 离开段落清除 hover 状态
function handleParagraphMouseLeave() {
  scheduleTooltipClose();
}

function handleTooltipEnter() {
  tooltipPanelHovered = true;
  cancelTooltipClose();
}

function handleTooltipLeave() {
  tooltipPanelHovered = false;
  scheduleTooltipClose(120);
}

async function lookupExpression(expression: ExpressionAnnotation, target: HTMLElement) {
  cancelTooltipClose();
  const rect = target.getBoundingClientRect();
  const tooltipHalfWidth = Math.min(230, Math.max(0, window.innerWidth / 2 - 12));
  tooltipX.value = Math.min(window.innerWidth - tooltipHalfWidth, Math.max(tooltipHalfWidth, rect.left + rect.width / 2));
  tooltipPlacement.value = rect.top >= 340 ? "above" : "below";
  tooltipY.value = tooltipPlacement.value === "above" ? rect.top : rect.bottom;
  tooltipToken.value = {
    bunsetsu: {
      head_word: {
        surface: expression.surface,
        base_form: expression.label,
        reading: "",
        pos: { major: "表达", sub1: expression.expression_type, sub2: "", sub3: "" },
      },
      grammar_tags: [],
    },
  };
  tooltipShow.value = true;
  tooltipLoading.value = true;
  tooltipHistory.value = [];
  const requestId = ++tooltipRequestId;
  const query = expression.origin === "dictionary" ? expression.label : expression.surface;
  const lookup = await lookupWord(query);
  if (requestId === tooltipRequestId) {
    tooltipLookup.value = lookup;
    tooltipLoading.value = false;
  }
}

async function navigateTooltip(target: string) {
  if (tooltipLookup.value) tooltipHistory.value.push(tooltipLookup.value);
  const requestId = ++tooltipRequestId;
  tooltipLoading.value = true;
  const lookup = await lookupWord(target);
  if (requestId === tooltipRequestId) {
    tooltipLookup.value = lookup;
    tooltipLoading.value = false;
  }
}

function backTooltip() {
  const previous = tooltipHistory.value.pop();
  if (previous) {
    ++tooltipRequestId;
    tooltipLookup.value = previous;
    tooltipLoading.value = false;
  }
}

async function selectTooltipTarget(target: string) {
  if (!tooltipLookup.value) return;
  const requestId = ++tooltipRequestId;
  tooltipLoading.value = true;
  const lookup = await chooseDictionaryTarget(
    tooltipLookup.value.query,
    tooltipLookup.value.reading,
    target,
  );
  if (requestId === tooltipRequestId) {
    tooltipLookup.value = lookup;
    tooltipLoading.value = false;
  }
}

// 事件委托：段落内的点击 (切换选中/已知)
function handleParagraphClick(e: MouseEvent, paragraphId: number) {
  // 如果是右键/双击菜单正在显示，或者正在拖拽，不触发点击
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

// 双击段落：弹出上下文菜单
function handleParagraphDblClick(e: MouseEvent, paragraphId: number) {
  const target = e.target as HTMLElement;
  const capsuleEl = target.closest("[data-token-index]") as HTMLElement;
  if (!capsuleEl) return;

  const tokenIndex = parseInt(capsuleEl.getAttribute("data-token-index") || "", 10);
  if (isNaN(tokenIndex)) return;

  const p = paragraphs.value.find((para) => para.id === paragraphId);
  const token = p?.tokens[tokenIndex];
  const isPunc = token && (token.display_class === "punctuation" || token.display_class === "line_break");
  if (!token || isPunc) return;

  // 取消 Tooltip 显示
  cancelTooltipClose();
  ++tooltipRequestId;
  tooltipShow.value = false;

  // 弹出右键菜单
  contextMenuX.value = e.clientX;
  contextMenuY.value = e.clientY;
  contextMenuToken.value = token;
  contextMenuParagraphId.value = paragraphId;
  contextMenuTokenIndex.value = tokenIndex;
  contextMenuCandidates.value = [];
  contextMenuShow.value = true;
}

function replaceContextToken(replacement: AnnotatedToken[]) {
  const paragraph = paragraphs.value.find(
    (item) => item.id === contextMenuParagraphId.value
  );
  if (!paragraph || replacement.length === 0) return;
  paragraph.tokens.splice(contextMenuTokenIndex.value, 1, ...replacement);
  clearAllSelections();
  contextMenuShow.value = false;
  virtualizer.value.measure();
}

async function applyContextCandidate(candidate: SegmentationCandidate) {
  if (!contextMenuToken.value) return;
  try {
    await chooseSegmentation(contextMenuToken.value, candidate);
    contextMenuShow.value = false;
    await triggerAnalysis(false);
  } catch (err) {
    console.error("Candidate Apply Error:", err);
    alert(`应用 N-best 候选失败：${String(err)}`);
  }
}

async function splitContextToken() {
  if (!contextMenuToken.value) return;
  try {
    replaceContextToken(await splitToken(contextMenuToken.value));
  } catch (err) {
    console.error("Split Token Error:", err);
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

  activeWordForModal.value = token.bunsetsu.head_word.base_form;
  showDefinitionModal.value = true;
  modalDefinitions.value = [];

  const lookup = await lookupWord(token.bunsetsu.head_word.base_form, token.bunsetsu.head_word.reading);
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
        <span class="logo-icon">📖</span>
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
          <span :title="`监听 ${analysisMetrics.listenerSetupMs} ms；后端 ${analysisMetrics.backendDurationMs} ms；IPC/解析 ${analysisMetrics.ipcAndParseMs} ms；IPC传输+后端 ${analysisMetrics.invokeAndTransferMs} ms；组段 ${analysisMetrics.paragraphBuildMs} ms；首帧布局/绘制 ${analysisMetrics.renderSetupMs} ms`">
            {{ analysisMetrics.durationMs }} ms
          </span>
        </div>
        <button class="icon-btn" :class="{ active: showExportPanel }" @click="showExportPanel = !showExportPanel">
          💼 导出本 ({{ selectedKeys.length }})
        </button>
        <button class="icon-btn" :class="{ active: showExpressionRules }" @click="openExpressionRules">
          ⛓ 表达式
        </button>
        <button class="icon-btn" :class="{ active: einkMode }" @click="toggleEinkMode">
          🕶 墨水屏
        </button>
        <button v-if="!showInput" class="icon-btn highlight" @click="showInput = true">
          ＋ 输入文本
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
          ⚠️ 分析出错: {{ errorMsg }}
        </div>
        <AnalysisProgressPanel
          :progress="analysisProgress"
          :active="isAnalyzing"
        />
        <div class="btn-group">
          <button
            class="analyze-btn"
            :disabled="isAnalyzing"
            @click="triggerAnalysis()"
          >
            {{ isAnalyzing ? analysisProgress.message : '解析生词胶囊' }}
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
            @mouseover="handleParagraphMouseOver($event, paragraphs[virtualRow.index].id)"
            @mouseleave="handleParagraphMouseLeave"
            @mousedown="handleMouseDown($event, paragraphs[virtualRow.index].id)"
            @mousemove="handleMouseMove($event, paragraphs[virtualRow.index].id)"
            @click="handleParagraphClick($event, paragraphs[virtualRow.index].id)"
            @dblclick="handleParagraphDblClick($event, paragraphs[virtualRow.index].id)"
          >
            <template v-if="paragraphs[virtualRow.index].tokens.length > 0">
              <template v-for="(token, tokenIndex) in paragraphs[virtualRow.index].tokens" :key="tokenIndex">
                <BunsetsuCapsule
                  :token="token"
                  :paragraphId="paragraphs[virtualRow.index].id"
                  :tokenIndex="tokenIndex"
                  :isDragSelected="isTokenDragSelected(paragraphs[virtualRow.index].id, tokenIndex)"
                  :tokens="paragraphs[virtualRow.index].tokens"
                  @lookup-expression="lookupExpression"
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

    <!-- 3. 全局 Tooltip 悬浮框 -->
    <TooltipPanel
      :show="tooltipShow"
      :x="tooltipX"
      :y="tooltipY"
      :placement="tooltipPlacement"
      :token="tooltipToken"
      :lookup="tooltipLookup"
      :loading="tooltipLoading"
      @enter="handleTooltipEnter"
      @leave="handleTooltipLeave"
      @navigate="navigateTooltip"
      @select="selectTooltipTarget"
      @back="backTooltip"
      :can-go-back="tooltipHistory.length > 0"
    />

    <!-- 4. 双击上下文操作菜单 -->
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
      @split="splitContextToken"
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

    <ExpressionRulesPanel
      :show="showExpressionRules"
      :rules="expressionRules"
      @close="showExpressionRules = false"
      @delete="removeExpressionRule"
    />

    <ExpressionRuleEditor
      :show="showExpressionEditor"
      :tokens="expressionDraft"
      :startMorphemeIdx="expressionDraftMorphemeRange.startMorphemeIdx"
      :endMorphemeIdx="expressionDraftMorphemeRange.endMorphemeIdx"
      @cancel="showExpressionEditor = false"
      @save="saveExpressionDraft"
    />

    <!-- 6. 详细词典释义弹窗 (Modal) -->
    <Transition name="fade">
      <div v-if="showDefinitionModal" class="modal-overlay" @click="showDefinitionModal = false">
        <div class="modal-card" @click.stop>
          <div class="modal-header">
            <h3>{{ activeWordForModal }} 完整词典释义</h3>
            <button class="modal-close" @click="showDefinitionModal = false">×</button>
          </div>
          <div class="modal-body no-scrollbar">
            <div v-if="modalDefinitions.length === 0" class="no-defs">
              未检索到本地 SQLite 词库中关于该词的完整释义。
            </div>
            <div v-else class="modal-defs-container">
              <div v-for="(def, idx) in modalDefinitions" :key="idx" class="dict-section">
                <div class="dict-name">✦ {{ def.dict_name }}</div>
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
  height: 100vh;
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
  font-size: 1.5rem;
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
  background-color: var(--novelty-high-bg);
  border: 1px solid var(--novelty-high-border);
  color: var(--novelty-high-text);
  padding: 12px;
  border-radius: var(--radius-sm);
  font-size: 0.9rem;
  margin-top: -10px;
  margin-bottom: 10px;
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
