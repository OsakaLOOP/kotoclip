<script setup lang="ts">
import { computed, nextTick, ref, watch, type ComponentPublicInstance } from "vue";
import { invoke } from "@tauri-apps/api/core";
import {
  ArrowLeft,
  ArrowRight,
  ChevronDown,
  ChevronUp,
  RefreshCw,
  Search,
} from "@lucide/vue";
import ReaderSurface from "../reader/ReaderSurface.vue";
import QualityAuditPane from "./QualityAuditPane.vue";
import { useDictionary } from "../../composables/useDictionary";
import {
  moveQualityAuditUnit,
  restoreQualityAuditPosition,
  type QualityAuditPosition,
} from "../../quality/navigation";
import type {
  QualityAuditComparison,
  QualityAuditHistory,
  QualityAuditHistoryRecord,
  QualityReadingUnit,
} from "../../types/qualityAudit";

const props = defineProps<{ show: boolean }>();
const emit = defineEmits<{ close: [] }>();

type PaneSide = "before" | "after";
type PaneHandle = {
  focusCoordinate: (coordinate: number, kind: "character" | "grammar") => Promise<boolean>;
  close: () => void;
};

const history = ref<QualityAuditHistory | null>(null);
const selectedComparisonId = ref("");
const comparison = ref<QualityAuditComparison | null>(null);
const loadingHistory = ref(false);
const loadingComparison = ref(false);
const loadingPage = ref(false);
const pageError = ref("");
const pageUnitDetails = ref<QualityReadingUnit[]>([]);
const errorMessage = ref("");
const searchText = ref("");
const domainFilter = ref("");
const stageFilter = ref("");
const coordinateFilter = ref("");
const pageSize = ref(10);
const pageIndex = ref(0);
const selectedUnitId = ref<string | null>(null);
const activeUnitId = ref<string | null>(null);
const activeCharacter = ref<number | null>(null);
const activeSide = ref<PaneSide | null>(null);
const paneHandles = new Map<string, Partial<Record<PaneSide, PaneHandle>>>();
const unitElements = new Map<string, HTMLElement>();
const comparisonPositions = new Map<string, QualityAuditPosition>();
const { lookupWord, loadDictionarySettings } = useDictionary();
let comparisonGeneration = 0;
let pageGeneration = 0;
let pendingScrollUnitId: string | null = null;

const records = computed(() => history.value?.comparisons ?? []);
const currentRecord = computed(() =>
  records.value.find((record) => record.comparison_id === selectedComparisonId.value) ?? null,
);
const historyIndex = computed(() =>
  records.value.findIndex((record) => record.comparison_id === selectedComparisonId.value),
);
const units = computed(() => comparison.value?.readingIndex.units ?? []);
const summary = computed<Record<string, any>>(() => comparison.value?.summary ?? {});
const stageRows = computed<Record<string, any>[]>(() =>
  Array.isArray(summary.value.stages) ? summary.value.stages : [],
);
const domains = computed(() =>
  [...new Set(units.value.flatMap((unit) => Object.keys(unit.domains)))].sort(),
);
const stages = computed(() =>
  [
    ...new Set([
      ...stageRows.value.map((row) => String(row.stage)),
      ...units.value.flatMap((unit) => unit.stages),
    ]),
  ].sort(),
);
const filteredUnits = computed(() => {
  const needle = searchText.value.trim().toLocaleLowerCase();
  const coordinate = Number(coordinateFilter.value);
  const hasCoordinate = coordinateFilter.value.trim() !== "" && Number.isFinite(coordinate);
  return units.value.filter((unit) => {
    if (needle && !`${unit.unit_id}\n${unit.before.text}\n${unit.after.text}`.toLocaleLowerCase().includes(needle)) {
      return false;
    }
    if (domainFilter.value && !(domainFilter.value in unit.domains)) return false;
    if (stageFilter.value && !unit.stages.includes(stageFilter.value)) return false;
    if (hasCoordinate) {
      const ranges = [unit.before.char_range, unit.after.char_range];
      if (!ranges.some((range) => range[0] <= coordinate && coordinate < range[1])) return false;
    }
    return true;
  });
});
const filteredUnitIds = computed(() => filteredUnits.value.map((unit) => unit.unit_id));
const pageCount = computed(() => Math.max(1, Math.ceil(filteredUnits.value.length / pageSize.value)));
const pageUnits = computed(() => {
  const start = pageIndex.value * pageSize.value;
  return filteredUnits.value.slice(start, start + pageSize.value);
});
const pageUnitOrder = computed(() =>
  new Map(pageUnits.value.map((unit, index) => [unit.unit_id, index])),
);
const renderedPageUnits = computed(() =>
  [...pageUnitDetails.value].sort(
    (left, right) =>
      (pageUnitOrder.value.get(left.unit_id) ?? Number.MAX_SAFE_INTEGER) -
      (pageUnitOrder.value.get(right.unit_id) ?? Number.MAX_SAFE_INTEGER),
  ),
);
const pageStart = computed(() => filteredUnits.value.length ? pageIndex.value * pageSize.value + 1 : 0);
const pageEnd = computed(() => Math.min((pageIndex.value + 1) * pageSize.value, filteredUnits.value.length));
const selectedUnitIndex = computed(() =>
  selectedUnitId.value ? filteredUnitIds.value.indexOf(selectedUnitId.value) : -1,
);
const navigationUnitIndex = computed(() =>
  selectedUnitIndex.value >= 0 ? selectedUnitIndex.value : Math.max(0, pageStart.value - 1),
);
const comparisonConclusion = computed(() => {
  if (summary.value.changes === 0 && summary.value.evidence_changes > 0) return "仅证据变化";
  if (summary.value.changes === 0) return "无阅读结果变化";
  if (summary.value.quality_conclusion === "improved") return "整体改善";
  if (summary.value.quality_conclusion === "regressed") return "存在回退";
  return "需要逐条审查";
});
const coverageDescription = computed(() => {
  if (summary.value.complete_stage_coverage) return "全部阶段可比较";
  const missing = Array.isArray(summary.value.missing_stages)
    ? summary.value.missing_stages.join("、")
    : "未知阶段";
  return `覆盖不完整：${missing || "未知阶段"}`;
});
const viewDescription = computed(() => {
  const record = currentRecord.value;
  if (!record) return "提交级全量比较";
  return `${shortCommit(record.before.implementation?.git_commit)} → ${shortCommit(record.after.implementation?.git_commit)}`;
});

function shortCommit(value?: string | null) {
  return value ? value.slice(0, 12) : "未知提交";
}

function formatNumber(value: unknown) {
  return typeof value === "number" && Number.isFinite(value)
    ? value.toLocaleString("zh-CN")
    : "—";
}

function formatPercent(value: unknown) {
  return typeof value === "number" && Number.isFinite(value)
    ? `${(value * 100).toFixed(value < 0.001 ? 4 : 2)}%`
    : "—";
}

function formatBytes(value: unknown) {
  if (typeof value !== "number" || !Number.isFinite(value)) return "—";
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KiB`;
  if (value < 1024 * 1024 * 1024) return `${(value / 1024 / 1024).toFixed(1)} MiB`;
  return `${(value / 1024 / 1024 / 1024).toFixed(2)} GiB`;
}

function formatTime(value?: string | null) {
  if (!value) return "未知时间";
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime()) ? value : parsed.toLocaleString("zh-CN", { hour12: false });
}

function domainLabel(value: string) {
  const labels: Record<string, string> = {
    resource: "资源",
    preprocessing: "预处理",
    morpheme: "形态素",
    morphology: "形态结构",
    word_formation: "构词",
    lexical: "整体词",
    lexical_unit: "整体词",
    bunsetsu: "文节",
    grammar: "语法",
    expression: "表达",
    nbest: "N-best",
    personalization: "画像",
    ui_projection: "界面投影",
  };
  return labels[value] ?? value;
}

function gateLabel(value?: string | null) {
  if (value === "passed") return "通过";
  if (value === "review_required") return "需要审查";
  if (value === "blocked") return "已阻断";
  return value || "未运行";
}

function comparisonLabel(record: QualityAuditHistoryRecord) {
  const before = shortCommit(record.before.implementation?.git_commit);
  const after = shortCommit(record.after.implementation?.git_commit);
  const availability = record.reading_index
    ? `${formatNumber(record.summary.reading_units)} 条`
    : record.reading_diff
      ? "需重建分页索引"
      : "无原生条目";
  return `${before} → ${after} · ${availability} · ${formatTime(record.created_at)}`;
}

function resetInteraction() {
  for (const handles of paneHandles.values()) {
    handles.before?.close();
    handles.after?.close();
  }
  paneHandles.clear();
  activeUnitId.value = null;
  activeCharacter.value = null;
  activeSide.value = null;
}

function setPaneRef(
  unitId: string,
  side: PaneSide,
  value: Element | ComponentPublicInstance | null,
) {
  const handles = paneHandles.get(unitId) ?? {};
  if (value && "focusCoordinate" in value && "close" in value) {
    handles[side] = value as unknown as PaneHandle;
    paneHandles.set(unitId, handles);
    return;
  }
  delete handles[side];
  if (handles.before || handles.after) paneHandles.set(unitId, handles);
  else paneHandles.delete(unitId);
}

function setUnitRef(
  unitId: string,
  value: Element | ComponentPublicInstance | null,
) {
  if (value instanceof HTMLElement) unitElements.set(unitId, value);
  else unitElements.delete(unitId);
}

async function scrollToUnit(unitId: string) {
  await nextTick();
  unitElements.get(unitId)?.scrollIntoView({ behavior: "smooth", block: "center" });
}

function selectUnit(unitId: string, scroll = false) {
  selectedUnitId.value = unitId;
  if (scroll) void scrollToUnit(unitId);
}

function moveUnit(offset: -1 | 1) {
  const target = moveQualityAuditUnit(
    filteredUnitIds.value,
    pageSize.value,
    selectedUnitId.value,
    pageIndex.value,
    offset,
  );
  if (!target) return;
  selectedUnitId.value = target.unitId;
  if (target.pageIndex !== pageIndex.value) {
    pendingScrollUnitId = target.unitId;
    pageIndex.value = target.pageIndex;
  } else {
    void scrollToUnit(target.unitId);
  }
}

async function handleFocus(
  unit: QualityReadingUnit,
  focus: { side: PaneSide; coordinate: number; kind: "character" | "grammar" },
) {
  activeUnitId.value = unit.unit_id;
  activeCharacter.value = focus.coordinate;
  activeSide.value = focus.side;
  const counterpart = focus.side === "before" ? "after" : "before";
  await paneHandles.get(unit.unit_id)?.[counterpart]?.focusCoordinate(focus.coordinate, focus.kind);
}

function handleLeave(unit: QualityReadingUnit, side: PaneSide) {
  if (activeUnitId.value !== unit.unit_id || activeSide.value !== side) return;
  activeUnitId.value = null;
  activeCharacter.value = null;
  activeSide.value = null;
  const counterpart = side === "before" ? "after" : "before";
  paneHandles.get(unit.unit_id)?.[counterpart]?.close();
}

async function loadHistory() {
  loadingHistory.value = true;
  errorMessage.value = "";
  try {
    history.value = await invoke<QualityAuditHistory>("quality_audit_history");
    const previousComparisonId = selectedComparisonId.value;
    const stillExists = records.value.some((record) => record.comparison_id === selectedComparisonId.value);
    selectedComparisonId.value = stillExists
      ? selectedComparisonId.value
      : records.value[0]?.comparison_id ?? "";
    if (!selectedComparisonId.value) comparison.value = null;
    else if (selectedComparisonId.value === previousComparisonId) {
      await loadComparison(selectedComparisonId.value);
    }
  } catch (error) {
    history.value = null;
    comparison.value = null;
    errorMessage.value = String(error);
  } finally {
    loadingHistory.value = false;
  }
}

async function loadComparison(comparisonId: string) {
  const generation = ++comparisonGeneration;
  const savedPosition = comparisonPositions.get(comparisonId);
  resetInteraction();
  comparison.value = null;
  pageUnitDetails.value = [];
  pageIndex.value = 0;
  selectedUnitId.value = null;
  if (!comparisonId) return;
  loadingComparison.value = true;
  errorMessage.value = "";
  try {
    const value = await invoke<QualityAuditComparison>("quality_audit_comparison", {
      comparisonId,
    });
    if (generation === comparisonGeneration) {
      comparison.value = value;
      if (domainFilter.value && !domains.value.includes(domainFilter.value)) domainFilter.value = "";
      if (stageFilter.value && !stages.value.includes(stageFilter.value)) stageFilter.value = "";
      const restored = restoreQualityAuditPosition(
        filteredUnitIds.value,
        pageSize.value,
        savedPosition,
      );
      pageIndex.value = restored.pageIndex;
      selectedUnitId.value = restored.unitId;
      pendingScrollUnitId = savedPosition?.unitId === restored.unitId
        ? restored.unitId
        : null;
    }
  } catch (error) {
    if (generation === comparisonGeneration) errorMessage.value = String(error);
  } finally {
    if (generation === comparisonGeneration) loadingComparison.value = false;
  }
}

async function loadReadingPage() {
  const generation = ++pageGeneration;
  resetInteraction();
  pageUnitDetails.value = [];
  pageError.value = "";
  const unitIds = pageUnits.value.map((unit) => unit.unit_id);
  if (!selectedComparisonId.value || !unitIds.length) return;
  loadingPage.value = true;
  try {
    const value = await invoke<QualityReadingUnit[]>("quality_audit_reading_units", {
      comparisonId: selectedComparisonId.value,
      unitIds,
    });
    if (generation === pageGeneration) {
      pageUnitDetails.value = value;
      const target = pendingScrollUnitId;
      if (target && unitIds.includes(target)) {
        pendingScrollUnitId = null;
        await scrollToUnit(target);
      }
    }
  } catch (error) {
    if (generation === pageGeneration) pageError.value = String(error);
  } finally {
    if (generation === pageGeneration) loadingPage.value = false;
  }
}

function moveHistory(offset: number) {
  const index = historyIndex.value + offset;
  const record = records.value[index];
  if (record) selectedComparisonId.value = record.comparison_id;
}

function previousPage() {
  pageIndex.value = Math.max(0, pageIndex.value - 1);
  const unit = filteredUnits.value[pageIndex.value * pageSize.value];
  selectedUnitId.value = unit?.unit_id ?? null;
  pendingScrollUnitId = unit?.unit_id ?? null;
}

function nextPage() {
  pageIndex.value = Math.min(pageCount.value - 1, pageIndex.value + 1);
  const unit = filteredUnits.value[pageIndex.value * pageSize.value];
  selectedUnitId.value = unit?.unit_id ?? null;
  pendingScrollUnitId = unit?.unit_id ?? null;
}

watch(
  () => props.show,
  (show) => {
    if (show) {
      void loadHistory();
      void loadDictionarySettings().catch((error) => {
        console.error("加载审计词典设置失败", error);
      });
    }
    else resetInteraction();
  },
  { immediate: true },
);
watch(selectedComparisonId, (value, previousValue) => {
  if (previousValue) {
    comparisonPositions.set(previousValue, {
      pageIndex: pageIndex.value,
      unitId: selectedUnitId.value,
    });
  }
  void loadComparison(value);
});
watch([searchText, domainFilter, stageFilter, coordinateFilter], () => {
  resetInteraction();
  pageIndex.value = 0;
  selectedUnitId.value = filteredUnits.value[0]?.unit_id ?? null;
});
watch(pageSize, () => {
  const restored = restoreQualityAuditPosition(
    filteredUnitIds.value,
    pageSize.value,
    { pageIndex: pageIndex.value, unitId: selectedUnitId.value },
  );
  pageIndex.value = restored.pageIndex;
  selectedUnitId.value = restored.unitId;
});
watch(pageIndex, resetInteraction);
watch(
  () => pageUnits.value.map((unit) => unit.unit_id).join("\0"),
  () => void loadReadingPage(),
  { immediate: true },
);
</script>

<template>
  <ReaderSurface
    :show="show"
    variant="fullscreen"
    title="语言质量审计"
    :description="viewDescription"
    @back="emit('close')"
    @close="emit('close')"
  >
    <template #actions>
      <button
        class="quality-audit__icon-button"
        type="button"
        title="刷新历史轮次"
        aria-label="刷新历史轮次"
        :disabled="loadingHistory"
        @click="loadHistory"
      >
        <RefreshCw :size="17" :class="{ 'is-spinning': loadingHistory }" aria-hidden="true" />
      </button>
    </template>

    <main class="quality-audit">
      <nav v-if="records.length" class="quality-audit__history-nav" aria-label="对比历史轮次">
        <button
          type="button"
          title="更新的轮次"
          aria-label="更新的轮次"
          :disabled="historyIndex <= 0"
          @click="moveHistory(-1)"
        >
          <ArrowLeft :size="16" aria-hidden="true" />
        </button>
        <label>
          <span>历史轮次</span>
          <select v-model="selectedComparisonId">
            <option v-for="record in records" :key="record.comparison_id" :value="record.comparison_id">
              {{ comparisonLabel(record) }}
            </option>
          </select>
        </label>
        <span>{{ historyIndex + 1 }} / {{ records.length }}</span>
        <button
          type="button"
          title="更早的轮次"
          aria-label="更早的轮次"
          :disabled="historyIndex < 0 || historyIndex >= records.length - 1"
          @click="moveHistory(1)"
        >
          <ArrowRight :size="16" aria-hidden="true" />
        </button>
      </nav>
      <div v-if="errorMessage" class="quality-audit__message quality-audit__message--error" role="alert">
        {{ errorMessage }}
      </div>
      <div v-else-if="loadingHistory || loadingComparison" class="quality-audit__message" role="status">
        正在读取语言质量产物…
      </div>
      <div v-else-if="!records.length" class="quality-audit__message">
        尚无可查看的对比轮次。
      </div>
      <template v-else-if="comparison && currentRecord">
        <section class="quality-audit__overview" aria-label="当前比较摘要">
          <div class="quality-audit__revision">
            <div>
              <span>Before</span>
              <strong>{{ shortCommit(currentRecord.before.implementation?.git_commit) }}</strong>
              <small>{{ currentRecord.before.implementation?.git_subject || currentRecord.before.label || "基准实现" }}</small>
              <small>{{ currentRecord.before.implementation?.git_author || "未知作者" }} · {{ formatTime(currentRecord.before.implementation?.git_author_time) }}</small>
            </div>
            <ArrowRight :size="18" aria-hidden="true" />
            <div>
              <span>After</span>
              <strong>{{ shortCommit(currentRecord.after.implementation?.git_commit) }}</strong>
              <small>{{ currentRecord.after.implementation?.git_subject || currentRecord.after.label || "候选实现" }}</small>
              <small>{{ currentRecord.after.implementation?.git_author || "未知作者" }} · {{ formatTime(currentRecord.after.implementation?.git_author_time) }}</small>
            </div>
          </div>
          <dl class="quality-audit__metrics">
            <div><dt>聚合条目</dt><dd>{{ formatNumber(comparison.readingIndex.unit_count) }}</dd></div>
            <div><dt>主变化</dt><dd>{{ formatNumber(summary.changes) }}</dd></div>
            <div><dt>根变化（含证据）</dt><dd>{{ formatNumber(summary.root_changes) }}</dd></div>
            <div><dt>证据变化</dt><dd>{{ formatNumber(summary.evidence_changes) }}</dd></div>
            <div><dt>Churn</dt><dd>{{ formatPercent(summary.churn?.rate ?? currentRecord.summary.churn_rate) }}</dd></div>
            <div><dt>门禁</dt><dd>{{ gateLabel(currentRecord.gate_status) }}</dd></div>
          </dl>
          <div
            class="quality-audit__status"
            :class="{ 'quality-audit__status--warning': !summary.complete_stage_coverage }"
          >
            <strong>{{ comparisonConclusion }}</strong>
            <span>{{ coverageDescription }}</span>
            <span v-if="summary.evidence_changes && !summary.changes">
              {{ formatNumber(summary.evidence_changes) }} 条证据变化未改变阅读结果
            </span>
          </div>
          <details class="quality-audit__metadata">
            <summary>轮次元数据</summary>
            <dl>
              <div><dt>Comparison</dt><dd><code>{{ comparison.comparisonId }}</code></dd></div>
              <div><dt>Diff schema</dt><dd><code>{{ comparison.manifest.schema_version || "—" }}</code></dd></div>
              <div><dt>Reading schema</dt><dd><code>{{ comparison.readingIndex.reading_schema_version }}</code></dd></div>
              <div><dt>语料</dt><dd>{{ currentRecord.after.corpus?.id || currentRecord.before.corpus?.id || "—" }}</dd></div>
              <div><dt>分析字符</dt><dd>{{ formatNumber(currentRecord.after.corpus?.analysis_characters ?? currentRecord.after.corpus?.selected_characters) }}</dd></div>
              <div><dt>生成时间</dt><dd>{{ formatTime(currentRecord.created_at) }}</dd></div>
              <div><dt>运行分支</dt><dd><code>{{ comparison.lifecycle?.git?.branch || "—" }}</code></dd></div>
              <div><dt>运行 HEAD</dt><dd><code>{{ shortCommit(comparison.lifecycle?.git?.head) }}</code></dd></div>
              <div><dt>工作树</dt><dd>{{ comparison.lifecycle?.git?.dirty ? "有未提交修改" : "干净" }}</dd></div>
              <div><dt>生命周期</dt><dd>{{ comparison.lifecycle?.status || "—" }}</dd></div>
              <div><dt>差分峰值内存</dt><dd>{{ formatBytes(comparison.memoryProfile?.observed_peak_rss_bytes) }}</dd></div>
              <div><dt>差分耗时</dt><dd>{{ comparison.memoryProfile?.elapsed_seconds ? `${comparison.memoryProfile.elapsed_seconds.toFixed(1)} 秒` : "—" }}</dd></div>
            </dl>
          </details>
          <details class="quality-audit__stages">
            <summary>分层统计 · {{ stageRows.length }} 个阶段</summary>
            <div class="quality-audit__stage-scroll">
              <table>
                <thead>
                  <tr>
                    <th>阶段</th>
                    <th>覆盖</th>
                    <th>Before 实体</th>
                    <th>After 实体</th>
                    <th>主变化</th>
                    <th>证据变化</th>
                    <th>根变化</th>
                    <th>传播候选</th>
                    <th>Churn</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="row in stageRows" :key="row.stage">
                    <td><code>{{ row.stage }}</code></td>
                    <td>{{ row.coverage }}</td>
                    <td>{{ formatNumber(row.before_entities) }}</td>
                    <td>{{ formatNumber(row.after_entities) }}</td>
                    <td>{{ formatNumber(row.changes) }}</td>
                    <td>{{ formatNumber(row.evidence_changes) }}</td>
                    <td>{{ formatNumber(row.root_changes) }}</td>
                    <td>{{ formatNumber(row.propagated_candidates) }}</td>
                    <td>{{ formatPercent(row.churn?.rate) }}</td>
                  </tr>
                </tbody>
              </table>
            </div>
          </details>
        </section>

        <section class="quality-audit__controls" aria-label="差异筛选">
          <label class="quality-audit__search">
            <Search :size="15" aria-hidden="true" />
            <input v-model="searchText" type="search" placeholder="搜索句子或条目 ID" />
          </label>
          <label>
            <span>领域</span>
            <select v-model="domainFilter">
              <option value="">全部</option>
              <option v-for="domain in domains" :key="domain" :value="domain">{{ domainLabel(domain) }}</option>
            </select>
          </label>
          <label>
            <span>阶段</span>
            <select v-model="stageFilter">
              <option value="">全部</option>
              <option v-for="stage in stages" :key="stage" :value="stage">{{ stage }}</option>
            </select>
          </label>
          <label class="quality-audit__coordinate">
            <span>字符坐标</span>
            <input v-model="coordinateFilter" type="number" min="0" placeholder="任意" />
          </label>
          <label>
            <span>每页</span>
            <select v-model.number="pageSize">
              <option :value="5">5</option>
              <option :value="10">10</option>
              <option :value="20">20</option>
            </select>
          </label>
        </section>

        <div class="quality-audit__result-bar">
          <span>显示 {{ pageStart }}–{{ pageEnd }} / {{ formatNumber(filteredUnits.length) }} 条</span>
          <div class="quality-audit__navigation">
            <div class="quality-audit__unit-nav" aria-label="逐条导航">
              <button
                type="button"
                title="上一条"
                aria-label="上一条"
                :disabled="!filteredUnits.length || navigationUnitIndex <= 0"
                @click="moveUnit(-1)"
              >
                <ChevronUp :size="16" aria-hidden="true" />
              </button>
              <span>条目 {{ filteredUnits.length ? navigationUnitIndex + 1 : 0 }} / {{ filteredUnits.length }}</span>
              <button
                type="button"
                title="下一条"
                aria-label="下一条"
                :disabled="!filteredUnits.length || navigationUnitIndex >= filteredUnits.length - 1"
                @click="moveUnit(1)"
              >
                <ChevronDown :size="16" aria-hidden="true" />
              </button>
            </div>
            <div class="quality-audit__pager" aria-label="分页导航">
              <button type="button" title="上一页" aria-label="上一页" :disabled="pageIndex === 0" @click="previousPage">
                <ArrowLeft :size="16" aria-hidden="true" />
              </button>
              <span>页 {{ pageIndex + 1 }} / {{ pageCount }}</span>
              <button type="button" title="下一页" aria-label="下一页" :disabled="pageIndex >= pageCount - 1" @click="nextPage">
                <ArrowRight :size="16" aria-hidden="true" />
              </button>
            </div>
          </div>
        </div>

        <div v-if="loadingPage" class="quality-audit__message" role="status">正在读取当前页…</div>
        <div v-else-if="pageError" class="quality-audit__message quality-audit__message--error" role="alert">{{ pageError }}</div>
        <section v-else-if="renderedPageUnits.length" class="quality-audit__units" aria-label="聚合差异条目">
          <article
            v-for="unit in renderedPageUnits"
            :key="unit.unit_id"
            :ref="(value) => setUnitRef(unit.unit_id, value)"
            class="quality-audit-unit"
            :class="{ 'quality-audit-unit--selected': selectedUnitId === unit.unit_id }"
          >
            <header class="quality-audit-unit__header" @click="selectUnit(unit.unit_id)">
              <div>
                <strong>#{{ pageIndex * pageSize + (pageUnitOrder.get(unit.unit_id) ?? 0) + 1 }}</strong>
                <code>{{ unit.changed_range[0] }}–{{ unit.changed_range[1] }}</code>
                <span v-for="domain in Object.keys(unit.domains)" :key="domain">{{ domainLabel(domain) }}</span>
              </div>
              <small>{{ unit.primary_change_count }} 主变化 · {{ unit.evidence_change_count }} 条证据 · {{ unit.stages.join(" / ") }}</small>
            </header>
            <div class="quality-audit-unit__panes">
              <QualityAuditPane
                :ref="(value) => setPaneRef(unit.unit_id, 'before', value)"
                :unit="unit"
                side="before"
                :title="`Before · ${shortCommit(currentRecord.before.implementation?.git_commit)} · 句 ${unit.sentence_index + 1}`"
                :active-character="activeUnitId === unit.unit_id ? activeCharacter : null"
                :shortcuts-enabled="activeUnitId === unit.unit_id && activeSide === 'before'"
                :lookup-word="lookupWord"
                @focus="handleFocus(unit, $event)"
                @leave="handleLeave(unit, $event)"
              />
              <QualityAuditPane
                :ref="(value) => setPaneRef(unit.unit_id, 'after', value)"
                :unit="unit"
                side="after"
                :title="`After · ${shortCommit(currentRecord.after.implementation?.git_commit)} · 句 ${unit.sentence_index + 1}`"
                :active-character="activeUnitId === unit.unit_id ? activeCharacter : null"
                :shortcuts-enabled="activeUnitId === unit.unit_id && activeSide === 'after'"
                :lookup-word="lookupWord"
                @focus="handleFocus(unit, $event)"
                @leave="handleLeave(unit, $event)"
              />
            </div>
          </article>
        </section>
        <div v-else-if="!units.length" class="quality-audit__message">
          本轮没有影响阅读结果的主变化。可在上方查看证据变化、覆盖状态和分层统计。
        </div>
        <div v-else class="quality-audit__message">当前筛选没有匹配的差异条目。</div>
      </template>
    </main>
  </ReaderSurface>
</template>

<style scoped>
.quality-audit {
  min-width: 0;
  min-height: 0;
  flex: 1 1 auto;
  overflow-y: auto;
  background: var(--bg-secondary);
}

.quality-audit__icon-button,
.quality-audit__navigation button {
  display: grid;
  width: 32px;
  height: 32px;
  place-items: center;
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  background: var(--bg-primary);
  color: var(--text-secondary);
  cursor: pointer;
}

.quality-audit__icon-button:hover,
.quality-audit__navigation button:hover:not(:disabled) {
  border-color: var(--accent-color);
  color: var(--accent-color);
}

.quality-audit__icon-button:disabled,
.quality-audit__navigation button:disabled { cursor: default; opacity: .45; }
.is-spinning { animation: quality-audit-spin 800ms linear infinite; }

.quality-audit__overview {
  border-bottom: 1px solid var(--border-color);
  background: var(--bg-primary);
}

.quality-audit__history-nav {
  display: grid;
  grid-template-columns: 32px minmax(240px, 680px) auto 32px;
  align-items: end;
  justify-content: center;
  gap: 8px;
  padding: 9px 16px;
  border-bottom: 1px solid var(--border-color);
  background: var(--bg-primary);
}

.quality-audit__history-nav label { display: grid; min-width: 0; gap: 2px; color: var(--text-muted); font-size: .65rem; }
.quality-audit__history-nav select { width: 100%; height: 32px; min-width: 0; border: 1px solid var(--border-color); border-radius: var(--radius-sm); padding: 0 8px; background: var(--bg-primary); color: var(--text-primary); }
.quality-audit__history-nav > span { align-self: center; color: var(--text-muted); font-size: .7rem; font-variant-numeric: tabular-nums; }
.quality-audit__history-nav button { display: grid; width: 32px; height: 32px; place-items: center; border: 1px solid var(--border-color); border-radius: var(--radius-sm); background: var(--bg-primary); color: var(--text-secondary); cursor: pointer; }
.quality-audit__history-nav button:hover:not(:disabled) { border-color: var(--accent-color); color: var(--accent-color); }
.quality-audit__history-nav button:disabled { cursor: default; opacity: .4; }

.quality-audit__revision {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
  align-items: center;
  gap: 16px;
  width: min(1480px, 100%);
  margin: 0 auto;
  padding: 18px 24px 14px;
}

.quality-audit__revision > div { display: grid; min-width: 0; gap: 1px; }
.quality-audit__revision > div:last-child { text-align: right; }
.quality-audit__revision span { color: var(--text-muted); font-size: .68rem; font-weight: 700; text-transform: uppercase; }
.quality-audit__revision strong { font-family: "Cascadia Mono", Consolas, monospace; font-size: .9rem; }
.quality-audit__revision small { overflow: hidden; color: var(--text-secondary); font-size: .75rem; text-overflow: ellipsis; white-space: nowrap; }
.quality-audit__revision small + small { color: var(--text-muted); font-size: .67rem; }

.quality-audit__metrics {
  display: grid;
  grid-template-columns: repeat(6, minmax(0, 1fr));
  border-top: 1px solid var(--border-color);
}

.quality-audit__metrics > div { min-width: 0; padding: 10px 14px; border-right: 1px solid var(--border-color); text-align: center; }
.quality-audit__metrics > div:last-child { border-right: 0; }
.quality-audit__metrics dt { color: var(--text-muted); font-size: .68rem; }
.quality-audit__metrics dd { margin-top: 2px; font-size: .96rem; font-weight: 700; font-variant-numeric: tabular-nums; }

.quality-audit__status {
  display: flex;
  min-width: 0;
  align-items: center;
  justify-content: center;
  gap: 12px;
  padding: 8px 18px;
  border-top: 1px solid var(--border-color);
  background: color-mix(in srgb, var(--accent-light) 52%, var(--bg-primary));
  color: var(--text-secondary);
  font-size: .72rem;
}
.quality-audit__status strong { color: var(--text-primary); font-size: .76rem; }
.quality-audit__status span + span { padding-left: 12px; border-left: 1px solid var(--border-color); }
.quality-audit__status--warning { background: color-mix(in srgb, #c58a2c 9%, var(--bg-primary)); }

.quality-audit__metadata { border-top: 1px solid var(--border-color); }
.quality-audit__metadata summary { width: min(1480px, 100%); margin: 0 auto; padding: 7px 24px; color: var(--text-muted); font-size: .7rem; cursor: pointer; }
.quality-audit__metadata dl { display: grid; grid-template-columns: repeat(5, minmax(0, 1fr)); border-top: 1px solid var(--border-color); }
.quality-audit__metadata dl > div { min-width: 0; padding: 8px 12px; border-right: 1px solid var(--border-color); border-bottom: 1px solid var(--border-color); }
.quality-audit__metadata dl > div:nth-child(5n) { border-right: 0; }
.quality-audit__metadata dt { color: var(--text-muted); font-size: .63rem; }
.quality-audit__metadata dd { overflow: hidden; color: var(--text-secondary); font-size: .7rem; text-overflow: ellipsis; white-space: nowrap; }
.quality-audit__metadata code { font-family: "Cascadia Mono", Consolas, monospace; }

.quality-audit__stages { border-top: 1px solid var(--border-color); }
.quality-audit__stages summary { width: min(1480px, 100%); margin: 0 auto; padding: 7px 24px; color: var(--text-muted); font-size: .7rem; cursor: pointer; }
.quality-audit__stage-scroll { overflow-x: auto; border-top: 1px solid var(--border-color); }
.quality-audit__stages table { width: 100%; min-width: 900px; border-collapse: collapse; font-size: .69rem; font-variant-numeric: tabular-nums; }
.quality-audit__stages th,
.quality-audit__stages td { padding: 6px 10px; border-right: 1px solid var(--border-color); border-bottom: 1px solid var(--border-color); text-align: right; white-space: nowrap; }
.quality-audit__stages th:first-child,
.quality-audit__stages td:first-child,
.quality-audit__stages th:nth-child(2),
.quality-audit__stages td:nth-child(2) { text-align: left; }
.quality-audit__stages th { color: var(--text-muted); font-weight: 600; }
.quality-audit__stages td { color: var(--text-secondary); }
.quality-audit__stages code { color: var(--text-primary); font-family: "Cascadia Mono", Consolas, monospace; }

.quality-audit__controls {
  position: sticky;
  z-index: 5;
  top: 0;
  display: flex;
  min-width: 0;
  align-items: end;
  gap: 9px;
  padding: 11px max(18px, calc((100% - 1480px) / 2));
  border-bottom: 1px solid var(--border-color);
  background: color-mix(in srgb, var(--bg-primary) 94%, transparent);
  backdrop-filter: blur(12px);
}

.quality-audit__controls label { display: grid; min-width: 0; gap: 3px; color: var(--text-muted); font-size: .67rem; }
.quality-audit__controls input,
.quality-audit__controls select {
  height: 34px;
  min-width: 0;
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  padding: 0 9px;
  background: var(--bg-primary);
  color: var(--text-primary);
  outline: 0;
}
.quality-audit__controls input:focus,
.quality-audit__controls select:focus { border-color: var(--accent-color); }
.quality-audit__search { position: relative; flex: 1 1 260px; }
.quality-audit__search svg { position: absolute; z-index: 1; bottom: 9px; left: 9px; }
.quality-audit__search input { width: 100%; padding-left: 31px; }
.quality-audit__coordinate { width: 100px; }

.quality-audit__result-bar {
  display: flex;
  width: min(1480px, calc(100% - 36px));
  margin: 14px auto 9px;
  align-items: center;
  justify-content: space-between;
  color: var(--text-secondary);
  font-size: .76rem;
}
.quality-audit__navigation { display: flex; align-items: center; gap: 14px; }
.quality-audit__unit-nav,
.quality-audit__pager { display: flex; align-items: center; gap: 8px; }
.quality-audit__unit-nav span { min-width: 104px; font-variant-numeric: tabular-nums; text-align: center; }
.quality-audit__pager span { min-width: 68px; font-variant-numeric: tabular-nums; text-align: center; }

.quality-audit__units { display: grid; width: min(1480px, calc(100% - 36px)); margin: 0 auto 42px; gap: 12px; }
.quality-audit-unit { overflow: visible; border: 1px solid var(--border-color); border-radius: var(--radius-sm); background: var(--bg-primary); transition: border-color 120ms ease, box-shadow 120ms ease; }
.quality-audit-unit--selected { border-color: var(--accent-color); box-shadow: 0 0 0 1px color-mix(in srgb, var(--accent-color) 34%, transparent); }
.quality-audit-unit__header { display: flex; min-width: 0; align-items: center; justify-content: space-between; gap: 16px; padding: 8px 12px; border-bottom: 1px solid var(--border-color); cursor: pointer; }
.quality-audit-unit__header:hover { background: var(--bg-secondary); }
.quality-audit-unit__header > div { display: flex; min-width: 0; align-items: center; gap: 7px; }
.quality-audit-unit__header strong { font-size: .78rem; }
.quality-audit-unit__header code { color: var(--text-secondary); font-size: .7rem; }
.quality-audit-unit__header span { padding: 1px 5px; border: 1px solid var(--border-color); border-radius: 3px; color: var(--text-secondary); font-size: .65rem; }
.quality-audit-unit__header small { min-width: 0; overflow: hidden; color: var(--text-muted); font-size: .68rem; text-overflow: ellipsis; white-space: nowrap; }
.quality-audit-unit__panes { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); }
.quality-audit-unit__panes > :last-child { border-left: 1px solid var(--border-color); }

.quality-audit__message { width: min(960px, calc(100% - 36px)); margin: 64px auto; color: var(--text-secondary); text-align: center; }
.quality-audit__message--error { color: #a52a24; overflow-wrap: anywhere; }

@keyframes quality-audit-spin { to { transform: rotate(360deg); } }

@media (max-width: 980px) {
  .quality-audit__controls { flex-wrap: wrap; }
  .quality-audit__search { flex-basis: calc(50% - 5px); }
  .quality-audit__metrics { grid-template-columns: repeat(3, minmax(0, 1fr)); }
  .quality-audit__metrics > div:nth-child(3) { border-right: 0; }
  .quality-audit__metrics > div:nth-child(-n+3) { border-bottom: 1px solid var(--border-color); }
  .quality-audit__metadata dl { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .quality-audit__metadata dl > div:nth-child(5n) { border-right: 1px solid var(--border-color); }
  .quality-audit__metadata dl > div:nth-child(2n) { border-right: 0; }
}

@media (max-width: 720px) {
  .quality-audit__revision { padding-right: 14px; padding-left: 14px; }
  .quality-audit__controls { padding-right: 12px; padding-left: 12px; }
  .quality-audit__search { flex-basis: 100%; }
  .quality-audit__history-nav { grid-template-columns: 32px minmax(0, 1fr) 32px; }
  .quality-audit__history-nav > span { display: none; }
  .quality-audit__units, .quality-audit__result-bar { width: calc(100% - 20px); }
  .quality-audit__status { align-items: flex-start; flex-direction: column; gap: 4px; }
  .quality-audit__status span + span { padding-left: 0; border-left: 0; }
  .quality-audit__result-bar { align-items: flex-start; flex-direction: column; gap: 8px; }
  .quality-audit__navigation { width: 100%; justify-content: space-between; gap: 8px; }
  .quality-audit-unit__header { align-items: flex-start; flex-direction: column; gap: 4px; }
  .quality-audit-unit__header small { width: 100%; }
  .quality-audit-unit__panes { grid-template-columns: minmax(0, 1fr); }
  .quality-audit-unit__panes > :last-child { border-top: 1px solid var(--border-color); border-left: 0; }
}

@media (prefers-reduced-motion: reduce) { .is-spinning { animation-duration: 1.5s; } }
</style>
