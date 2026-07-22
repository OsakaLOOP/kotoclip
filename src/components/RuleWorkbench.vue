<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { ArrowRight } from "@lucide/vue";
import ReaderSurface from "./reader/ReaderSurface.vue";
import type {
  AnnotatedToken,
  ExpressionBoundaryEffect,
  ExpressionRule,
  ExpressionRulePreview,
  ExpressionType,
} from "../types";

type WorkbenchView = "library" | "editor";
type MatchMode = "fixed" | "slot" | "any";

const props = defineProps<{
  show: boolean;
  initialView: WorkbenchView;
  rules: ExpressionRule[];
  tokens: AnnotatedToken[];
  startMorphemeIdx: number;
  endMorphemeIdx: number;
  previewRule: (
    tokens: AnnotatedToken[],
    bunsetsuStates: MatchMode[],
    morphemeMasks: boolean[][],
    gapAfter: number | null,
    expressionType: ExpressionType,
  ) => Promise<ExpressionRulePreview>;
}>();

const emit = defineEmits<{
  (event: "close"): void;
  (event: "delete", id: number): void;
  (
    event: "save",
    label: string,
    description: string,
    bunsetsuStates: MatchMode[],
    morphemeMasks: boolean[][],
    gapAfter: number | null,
    expressionType: ExpressionType,
    priority: number,
    boundaryEffect: ExpressionBoundaryEffect,
  ): void;
}>();

const view = ref<WorkbenchView>("library");
const expressionType = ref<Exclude<ExpressionType, "lexical_unit">>("grammar_construction");
const label = ref("");
const description = ref("");
const priority = ref(60);
const matchModes = ref<MatchMode[]>([]);
const masks = ref<boolean[][]>([]);
const allowLeftContext = ref(false);
const allowRightContext = ref(false);
const gapAfter = ref<number | null>(null);
const libraryFilter = ref<"all" | Exclude<ExpressionType, "lexical_unit">>("all");
const backendPreview = ref<ExpressionRulePreview | null>(null);
let previewTimer: number | null = null;
let previewRequest = 0;

const typeOptions: Array<{
  value: Exclude<ExpressionType, "lexical_unit">;
  title: string;
  description: string;
  priority: number;
}> = [
  { value: "idiom", title: "惯用语", description: "固定或半固定词汇表达，保留内部文节。", priority: 70 },
  { value: "grammar_construction", title: "语法构式", description: "由词形、词性和活用共同约束。", priority: 60 },
  { value: "correlative", title: "非连续呼应", description: "以前后锚点和有限间隔构成。", priority: 40 },
];

const typeLabels: Record<ExpressionType, string> = {
  lexical_unit: "待迁移词汇规则",
  idiom: "惯用语",
  grammar_construction: "语法构式",
  correlative: "非连续呼应",
};

function resetDraft() {
  expressionType.value = "grammar_construction";
  label.value = "";
  description.value = "";
  priority.value = 60;
  gapAfter.value = null;
  matchModes.value = props.tokens.map(() => "fixed");
  masks.value = props.tokens.map((token, tokenIndex) =>
    token.bunsetsu.morphemes.map((morpheme, morphemeIndex) => {
      if (!morpheme.surface.trim()) return false;
      if (tokenIndex === 0 && morphemeIndex < props.startMorphemeIdx) return false;
      if (tokenIndex === props.tokens.length - 1 && morphemeIndex > props.endMorphemeIdx) return false;
      return true;
    }),
  );
  allowLeftContext.value = hasLeftExclusion.value;
  allowRightContext.value = hasRightExclusion.value;
}

function nonEmptyIndices(tokenIndex: number): number[] {
  return props.tokens[tokenIndex]?.bunsetsu.morphemes
    .map((morpheme, index) => (morpheme.surface.trim() ? index : -1))
    .filter((index) => index >= 0) ?? [];
}

const hasLeftExclusion = computed(() => {
  const indices = nonEmptyIndices(0);
  return indices.some((index) => !masks.value[0]?.[index])
    && indices.some((index) => masks.value[0]?.[index]);
});

const hasRightExclusion = computed(() => {
  const tokenIndex = props.tokens.length - 1;
  const indices = nonEmptyIndices(tokenIndex);
  return indices.some((index) => !masks.value[tokenIndex]?.[index])
    && indices.some((index) => masks.value[tokenIndex]?.[index]);
});

watch(
  () => [props.show, props.initialView, props.tokens, props.startMorphemeIdx, props.endMorphemeIdx] as const,
  () => {
    if (!props.show) return;
    view.value = props.initialView;
    if (props.initialView === "editor") resetDraft();
  },
  { immediate: true, deep: true },
);

function selectType(option: (typeof typeOptions)[number]) {
  expressionType.value = option.value;
  priority.value = option.priority;
  if (option.value !== "correlative") gapAfter.value = null;
}

function toggleMorpheme(tokenIndex: number, morphemeIndex: number) {
  if (matchModes.value[tokenIndex] === "any") return;
  masks.value[tokenIndex][morphemeIndex] = !masks.value[tokenIndex][morphemeIndex];
}

function setMatchMode(tokenIndex: number, mode: MatchMode) {
  matchModes.value[tokenIndex] = mode;
}

function selectedMorphemes(tokenIndex: number) {
  return props.tokens[tokenIndex].bunsetsu.morphemes.filter(
    (morpheme, morphemeIndex) => morpheme.surface.trim() && masks.value[tokenIndex]?.[morphemeIndex],
  );
}

function isContiguous(tokenIndex: number): boolean {
  if (matchModes.value[tokenIndex] === "any") return true;
  const selected = nonEmptyIndices(tokenIndex).filter((index) => masks.value[tokenIndex]?.[index]);
  return selected.length > 0 && selected.every((value, index) => index === 0 || value === selected[index - 1] + 1);
}

const validationError = computed(() => {
  if (!props.tokens.length) return "请先在正文中拖选一个实例。";
  const invalid = props.tokens.findIndex((_, index) => !isContiguous(index));
  if (invalid >= 0) return `第 ${invalid + 1} 个文节的参与语素必须连续且非空。`;
  if (hasLeftExclusion.value && !allowLeftContext.value) return "选择从文节内部开始；请允许左侧上下文，或调整语素选择。";
  if (hasRightExclusion.value && !allowRightContext.value) return "选择在文节内部结束；请允许右侧上下文，或调整语素选择。";
  if (expressionType.value === "correlative") {
    if (props.tokens.length < 2) return "非连续呼应至少需要两个锚点。";
    if (gapAfter.value === null) return "请选择前后锚点之间的 gap 位置。";
  }
  return null;
});

const generatedLabel = computed(() => {
  const parts = props.tokens.map((_token, tokenIndex) => {
    const mode = matchModes.value[tokenIndex];
  if (mode === "any") return "任意";
    const selected = selectedMorphemes(tokenIndex);
    if (mode === "slot") return selected.map((morpheme) => `{${morpheme.pos.major}}`).join("");
    return selected.map((morpheme) => morpheme.surface).join("");
  });
  if (gapAfter.value !== null) parts.splice(gapAfter.value + 1, 0, "…");
  return parts.join("");
});

const signatureParts = computed(() => {
  const parts = props.tokens.map((_, tokenIndex) => {
    const mode = matchModes.value[tokenIndex];
    if (mode === "any") return "{任意文节}";
    return selectedMorphemes(tokenIndex).map((morpheme) => {
      const pos = [morpheme.pos.major, morpheme.pos.sub1, morpheme.pos.sub2, morpheme.pos.sub3]
        .filter((value) => value && value !== "*")
        .join("/");
      return mode === "slot" ? `{${pos}}` : `${morpheme.base_form}/${pos}`;
    }).join(" + ");
  });
  if (gapAfter.value !== null) parts.splice(gapAfter.value + 1, 0, "{有限 gap}");
  return parts;
});

const previewRanges = computed(() => props.tokens.flatMap((_, tokenIndex) => {
  const selected = selectedMorphemes(tokenIndex);
  if (!selected.length || matchModes.value[tokenIndex] === "any") return [];
  return [[selected[0].char_range[0], selected[selected.length - 1].char_range[1]] as [number, number]];
}));

const previewStatus = computed(() => validationError.value ? "rejected" : backendPreview.value?.status ?? "pending");
const previewMessage = computed(() => validationError.value
  || backendPreview.value?.rejection_reason
  || (backendPreview.value?.status === "accepted" ? "后端 schema v2 校验通过" : "正在校验草稿"));
const effectivePreviewRanges = computed(() => backendPreview.value?.matched_ranges.length
  ? backendPreview.value.matched_ranges
  : previewRanges.value);
const canSave = computed(() => !validationError.value && backendPreview.value?.status === "accepted");

watch(
  [() => props.show, view, expressionType, matchModes, masks, gapAfter],
  () => {
    if (!props.show || view.value !== "editor" || !props.tokens.length) return;
    if (previewTimer !== null) window.clearTimeout(previewTimer);
    const request = ++previewRequest;
    backendPreview.value = null;
    previewTimer = window.setTimeout(async () => {
      try {
        const result = await props.previewRule(
          props.tokens,
          matchModes.value,
          masks.value,
          gapAfter.value,
          expressionType.value,
        );
        if (request === previewRequest) backendPreview.value = result;
      } catch (error) {
        if (request !== previewRequest) return;
        backendPreview.value = {
          status: "rejected",
          expression_type: expressionType.value,
          surface: "",
          matched_ranges: [],
          covered_token_range: [0, props.tokens.length],
          evidence: [],
          counter_evidence: ["preview_transport_failed"],
          rejection_reason: String(error),
        };
      }
    }, 120);
  },
  { deep: true, immediate: true },
);

const filteredRules = computed(() => props.rules.filter((rule) =>
  libraryFilter.value === "all" || rule.expression_type === libraryFilter.value,
));

function rulePreview(rule: ExpressionRule): string {
  const parts = rule.parts.map((part) => {
    if (part.is_any) return "任意";
    const body = part.surface_hint || part.lemmas.join("");
    return part.is_slot ? `{${part.pos.join("+") || body}}` : body;
  });
  if (rule.gap_after !== undefined && rule.gap_after !== null) {
    parts.splice(rule.gap_after + 1, 0, "…");
  }
  return parts.join("");
}

function save() {
  if (!canSave.value) return;
  emit(
    "save",
    label.value.trim() || generatedLabel.value,
    description.value.trim(),
    matchModes.value,
    masks.value,
    gapAfter.value,
    expressionType.value,
    priority.value,
    "annotate_only",
  );
}
</script>

<template>
  <ReaderSurface :show="show" variant="fullscreen" title="表达规则" description="查看和管理表达规则" @close="emit('close')">
    <template #actions>
      <button v-if="view === 'editor'" class="quiet-button" @click="view = 'library'">规则库</button>
    </template>
    <div class="workbench-shell" aria-label="表达规则">

      <main v-if="view === 'library'" class="library-layout">
        <aside class="layer-rail">
          <span class="section-label">表达层</span>
          <button :class="{ active: libraryFilter === 'all' }" @click="libraryFilter = 'all'">
            <strong>全部规则</strong><small>{{ rules.length }}</small>
          </button>
          <button v-for="option in typeOptions" :key="option.value" :class="{ active: libraryFilter === option.value }" @click="libraryFilter = option.value">
            <strong>{{ option.title }}</strong>
            <small>{{ rules.filter((rule) => rule.expression_type === option.value).length }}</small>
          </button>
          <div class="layer-note">
            <strong>构词与文节</strong>
            <p>由 schema v2 内置目录管理；表达规则不能修改其边界。</p>
          </div>
        </aside>

        <section class="library-content">
          <div class="section-heading">
            <div><span class="section-label">USER CATALOG</span><h2>用户规则</h2></div>
            <p>在正文中拖选实例即可新建规则。</p>
          </div>
          <div v-if="!filteredRules.length" class="empty-state">
            <strong>此分类暂无规则</strong>
            <span>关闭工作台，在正文中拖选一个完整实例开始创建。</span>
          </div>
          <ol v-else class="rule-grid">
            <li v-for="rule in filteredRules" :key="rule.id" class="rule-card">
              <div class="rule-card-top">
                <span class="type-chip">{{ typeLabels[rule.expression_type] }}</span>
                <span v-if="rule.requires_review" class="status-chip review">需迁移</span>
                <span v-else-if="!rule.enabled" class="status-chip">已停用</span>
                <span v-else class="status-chip enabled">已启用</span>
              </div>
              <h3>{{ rule.label }}</h3>
              <code>{{ rulePreview(rule) }}</code>
              <p>{{ rule.description || "未填写说明" }}</p>
              <footer>
                <span>v{{ rule.rule_version }} · 优先级 {{ rule.priority }}</span>
                <button class="danger-button" @click="emit('delete', rule.id)">删除</button>
              </footer>
            </li>
          </ol>
        </section>
      </main>

      <main v-else class="editor-layout">
        <section class="editor-main">
          <div class="section-heading">
            <div><span class="section-label">NEW RULE</span><h2>从实例创建</h2></div>
            <span class="selection-count">{{ tokens.length }} 个文节</span>
          </div>

          <fieldset class="type-picker">
            <legend>1 · 分类</legend>
            <button v-for="option in typeOptions" :key="option.value" :class="{ active: expressionType === option.value }" type="button" @click="selectType(option)">
              <strong>{{ option.title }}</strong><span>{{ option.description }}</span>
            </button>
          </fieldset>

          <section class="atom-section">
            <div class="step-heading"><span>2 · 原子与匹配</span><small>语素选择与匹配模式互不改写</small></div>
            <div class="atom-list">
              <template v-for="(token, tokenIndex) in tokens" :key="`${token.bunsetsu.char_range[0]}-${tokenIndex}`">
                <article class="atom-card">
                  <header>
                    <div><span>文节 {{ tokenIndex + 1 }}</span><strong>{{ token.bunsetsu.surface }}</strong></div>
                    <div class="mode-switch" aria-label="匹配模式">
                      <button v-for="mode in (['fixed', 'slot', 'any'] as MatchMode[])" :key="mode" :class="{ active: matchModes[tokenIndex] === mode }" @click="setMatchMode(tokenIndex, mode)">
                        {{ mode === 'fixed' ? '词形' : mode === 'slot' ? '词性槽' : '任意' }}
                      </button>
                    </div>
                  </header>
                  <div v-if="matchModes[tokenIndex] === 'any'" class="any-atom">匹配一个任意文节；当前语素选择保留但暂不参与。</div>
                  <div v-else class="morpheme-row">
                    <button v-for="(morpheme, morphemeIndex) in token.bunsetsu.morphemes" :key="`${morpheme.char_range[0]}-${morphemeIndex}`" v-show="morpheme.surface.trim()" class="morpheme-chip" :class="{ selected: masks[tokenIndex]?.[morphemeIndex] }" @click="toggleMorpheme(tokenIndex, morphemeIndex)">
                      <strong>{{ morpheme.surface }}</strong>
                      <span>{{ morpheme.base_form }}</span>
                      <small>{{ [morpheme.pos.major, morpheme.pos.sub1, morpheme.pos.sub2].filter((value) => value && value !== '*').join(' / ') }}</small>
                    </button>
                  </div>
                </article>
                <div v-if="tokenIndex < tokens.length - 1" class="connection-row">
                  <span></span>
                  <button v-if="expressionType === 'correlative'" :class="{ active: gapAfter === tokenIndex }" @click="gapAfter = gapAfter === tokenIndex ? null : tokenIndex">
                    {{ gapAfter === tokenIndex ? '已设有限 gap' : '在此设置 gap' }}
                  </button>
                </div>
              </template>
            </div>
          </section>

          <section class="context-section">
            <div class="step-heading"><span>3 · 上下文与输出</span><small>开关不会补选或取消语素</small></div>
            <div class="context-grid">
              <label><input v-model="allowLeftContext" type="checkbox" /><span><strong>允许左侧上下文</strong><small>规则可从首文节内部开始</small></span></label>
              <label><input v-model="allowRightContext" type="checkbox" /><span><strong>允许右侧上下文</strong><small>规则可在末文节内部结束</small></span></label>
              <div class="output-card"><strong>输出</strong><span>仅生成语义注解</span><small>使用精确 matched ranges，不改变文节。</small></div>
            </div>
          </section>

          <section class="metadata-section">
            <label><span>规则名称</span><input v-model="label" :placeholder="generatedLabel || '输入简短名称'" /></label>
            <label><span>优先级</span><input v-model.number="priority" type="number" min="0" max="100" /></label>
            <label class="description-field"><span>含义或使用条件</span><textarea v-model="description" rows="3" placeholder="说明接受条件和整体含义。"></textarea></label>
          </section>
        </section>

        <aside class="preview-panel">
          <div><span class="section-label">LIVE PREVIEW</span><h2>候选预览</h2></div>
          <div class="preview-status" :class="previewStatus"><span>{{ previewStatus }}</span><strong>{{ previewMessage }}</strong></div>
          <section><span>名称</span><strong class="preview-name">{{ label.trim() || generatedLabel || '未命名规则' }}</strong></section>
          <section><span>匹配签名</span><code v-if="signatureParts.length"><template v-for="(part, index) in signatureParts" :key="`${part}-${index}`"><span>{{ part }}</span><ArrowRight v-if="index < signatureParts.length - 1" :size="14" aria-hidden="true" /></template></code><code v-else>尚无有效原子</code></section>
          <section><span>精确范围</span><ol><li v-for="range in effectivePreviewRanges" :key="range.join('-')">char {{ range[0] }}..{{ range[1] }}</li></ol></section>
          <section><span>证据</span><ul><li v-for="evidence in backendPreview?.evidence ?? []" :key="evidence">{{ evidence }}</li><li v-if="allowLeftContext || allowRightContext">允许文节内部锚点</li></ul></section>
          <footer><button class="quiet-button" @click="emit('close')">取消</button><button class="primary-button" :disabled="!canSave" @click="save">保存规则</button></footer>
        </aside>
      </main>
    </div>
  </ReaderSurface>
</template>

<style scoped>
.workbench-shell { min-height: 0; flex: 1; display: flex; flex-direction: column; overflow: hidden; color: var(--text-primary); background: color-mix(in srgb, var(--bg-primary) 96%, #e9e4d9); }
.section-heading h2, .preview-panel h2 { margin: 3px 0 0; font-size: clamp(1.25rem, 2vw, 1.8rem); letter-spacing: -0.025em; }
.section-heading p { margin: 5px 0 0; color: var(--text-secondary); font-size: .84rem; }
.eyebrow, .section-label { color: var(--accent-color); font-size: .67rem; font-weight: 800; letter-spacing: .14em; }
.header-actions, .preview-panel footer { display: flex; align-items: center; gap: 9px; }
button { font: inherit; }
.quiet-button, .close-button, .primary-button, .danger-button { border-radius: 8px; padding: 9px 14px; cursor: pointer; }
.quiet-button, .close-button { border: 1px solid var(--border-color); color: var(--text-primary); background: var(--bg-primary); }
.primary-button { border: 1px solid var(--accent-color); color: #fff; background: var(--accent-color); font-weight: 700; }
.primary-button:disabled { cursor: not-allowed; opacity: .42; }
.danger-button { border: 0; color: #9d343d; background: transparent; }
.library-layout { min-height: 0; flex: 1; display: grid; grid-template-columns: 240px minmax(0, 1fr); }
.layer-rail { padding: 26px 18px; border-right: 1px solid var(--border-color); background: var(--bg-secondary); }
.layer-rail > button { width: 100%; display: flex; justify-content: space-between; gap: 10px; margin-top: 8px; padding: 11px 12px; border: 1px solid transparent; border-radius: 8px; color: var(--text-secondary); background: transparent; cursor: pointer; text-align: left; }
.layer-rail > button.active { color: var(--text-primary); border-color: var(--border-color); background: var(--bg-primary); box-shadow: 0 5px 15px rgba(30, 22, 35, .05); }
.layer-note { margin-top: 28px; padding: 14px; border-top: 1px solid var(--border-color); color: var(--text-secondary); }
.layer-note p { margin: 6px 0 0; font-size: .76rem; line-height: 1.6; }
.library-content, .editor-main { min-width: 0; overflow: auto; padding: 28px clamp(20px, 3vw, 44px) 50px; }
.section-heading, .step-heading { display: flex; align-items: flex-end; justify-content: space-between; gap: 20px; }
.empty-state { display: grid; place-items: center; gap: 7px; min-height: 300px; margin-top: 24px; border: 1px dashed var(--border-color); border-radius: 14px; color: var(--text-secondary); text-align: center; }
.rule-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 14px; padding: 0; margin: 24px 0 0; list-style: none; }
.rule-card { min-width: 0; padding: 17px; border: 1px solid var(--border-color); border-radius: 12px; background: var(--bg-primary); }
.rule-card-top, .rule-card footer { display: flex; align-items: center; justify-content: space-between; gap: 10px; }
.type-chip, .status-chip { padding: 4px 7px; border-radius: 999px; color: var(--text-secondary); background: var(--bg-secondary); font-size: .68rem; }
.status-chip.enabled { color: #27654c; background: #e5f3eb; }.status-chip.review { color: #8c5b16; background: #f8edd8; }
.rule-card h3 { margin: 15px 0 8px; font-size: 1rem; }.rule-card code { display: block; overflow: hidden; color: var(--accent-color); text-overflow: ellipsis; white-space: nowrap; }.rule-card p { min-height: 2.7em; color: var(--text-secondary); font-size: .78rem; line-height: 1.5; }.rule-card footer { padding-top: 10px; border-top: 1px solid var(--border-color); color: var(--text-muted); font-size: .7rem; }
.editor-layout { min-height: 0; flex: 1; display: grid; grid-template-columns: minmax(0, 1fr) minmax(300px, 360px); }
.selection-count { color: var(--text-secondary); font-size: .78rem; }
fieldset { min-width: 0; margin: 28px 0 0; padding: 0; border: 0; } legend, .step-heading > span { margin-bottom: 10px; font-size: .76rem; font-weight: 800; }
.type-picker { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 10px; }.type-picker legend { grid-column: 1 / -1; }
.type-picker button { min-width: 0; display: grid; gap: 5px; padding: 13px; border: 1px solid var(--border-color); border-radius: 10px; color: var(--text-secondary); background: var(--bg-primary); cursor: pointer; text-align: left; }.type-picker button.active { color: var(--text-primary); border-color: var(--accent-color); box-shadow: inset 0 0 0 1px var(--accent-color); }.type-picker span { font-size: .72rem; line-height: 1.45; }
.atom-section, .context-section { margin-top: 28px; }.step-heading small { color: var(--text-muted); font-size: .7rem; font-weight: 400; }.atom-list { display: grid; gap: 0; }
.atom-card { overflow: hidden; border: 1px solid var(--border-color); border-radius: 11px; background: var(--bg-primary); }.atom-card header { display: flex; align-items: center; justify-content: space-between; gap: 15px; padding: 11px 13px; border-bottom: 1px solid var(--border-color); background: var(--bg-secondary); }.atom-card header > div:first-child { display: grid; gap: 2px; }.atom-card header span { color: var(--text-muted); font-size: .65rem; }.atom-card header strong { font-size: .9rem; }
.mode-switch { flex: 0 0 auto; display: flex; overflow: hidden; border: 1px solid var(--border-color); border-radius: 7px; }.mode-switch button { padding: 5px 8px; border: 0; border-right: 1px solid var(--border-color); color: var(--text-secondary); background: var(--bg-primary); cursor: pointer; font-size: .68rem; }.mode-switch button:last-child { border-right: 0; }.mode-switch button.active { color: #fff; background: var(--accent-color); }
.morpheme-row { display: flex; flex-wrap: wrap; gap: 8px; padding: 13px; }.morpheme-chip { min-width: 92px; display: grid; gap: 2px; padding: 8px 10px; border: 1px solid var(--border-color); border-radius: 8px; color: var(--text-secondary); background: var(--bg-primary); cursor: pointer; text-align: left; opacity: .55; }.morpheme-chip.selected { color: var(--text-primary); border-color: color-mix(in srgb, var(--accent-color) 65%, var(--border-color)); background: color-mix(in srgb, var(--accent-color) 7%, var(--bg-primary)); opacity: 1; }.morpheme-chip span, .morpheme-chip small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: .65rem; }.any-atom { padding: 16px; color: var(--text-secondary); font-size: .75rem; text-align: center; }
.connection-row { height: 38px; display: grid; place-items: center; }.connection-row span { height: 100%; border-left: 1px solid var(--border-color); }.connection-row button { position: absolute; padding: 5px 10px; border: 1px dashed var(--border-color); border-radius: 999px; color: var(--text-muted); background: var(--bg-primary); cursor: pointer; font-size: .66rem; }.connection-row button.active { color: var(--accent-color); border-style: solid; border-color: var(--accent-color); }
.context-grid { display: grid; grid-template-columns: 1fr 1fr 1.15fr; gap: 9px; }.context-grid label, .output-card { min-width: 0; display: flex; align-items: flex-start; gap: 9px; padding: 12px; border: 1px solid var(--border-color); border-radius: 9px; background: var(--bg-primary); }.context-grid label span, .output-card { display: grid; gap: 3px; }.context-grid small, .output-card small { color: var(--text-muted); font-size: .68rem; line-height: 1.4; }.output-card span { color: var(--accent-color); font-size: .76rem; }
.metadata-section { display: grid; grid-template-columns: minmax(0, 1fr) 110px; gap: 10px; margin-top: 28px; }.metadata-section label { display: grid; gap: 5px; color: var(--text-secondary); font-size: .72rem; }.metadata-section input, .metadata-section textarea { width: 100%; box-sizing: border-box; padding: 10px 11px; border: 1px solid var(--border-color); border-radius: 8px; color: var(--text-primary); background: var(--bg-primary); font: inherit; }.description-field { grid-column: 1 / -1; }
.preview-panel code svg { display: inline-block; margin: 0 5px; vertical-align: -3px; }
@media (max-width: 900px) { .editor-layout { grid-template-columns: 1fr; overflow: auto; }.editor-main, .preview-panel { overflow: visible; }.preview-panel { border-top: 1px solid var(--border-color); border-left: 0; }.context-grid { grid-template-columns: 1fr; }.type-picker { grid-template-columns: 1fr; } }
@media (max-width: 680px) { .library-layout { grid-template-columns: 1fr; overflow: auto; }.layer-rail { display: flex; gap: 6px; overflow-x: auto; padding: 10px 14px; border-right: 0; border-bottom: 1px solid var(--border-color); }.layer-rail .section-label, .layer-note { display: none; }.layer-rail > button { flex: 0 0 auto; width: auto; margin: 0; }.library-content { overflow: visible; }.atom-card header { align-items: flex-start; flex-direction: column; }.mode-switch { width: 100%; }.mode-switch button { flex: 1; }.metadata-section { grid-template-columns: 1fr; }.description-field { grid-column: auto; } }
</style>
