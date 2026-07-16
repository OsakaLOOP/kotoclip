<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { getGrammarConcept, searchGrammarCatalog } from "../grammar/catalog";
import {
  clearGrammarReviewOverride,
  grammarReviewOverrides,
  setGrammarReviewOverride,
} from "../grammar/review";
import aiCheckedIcon from "../assets/grammar-review/ai-checked.svg";
import trustedIcon from "../assets/grammar-review/trusted.svg";
import unverifiedIcon from "../assets/grammar-review/unverified.svg";
import GrammarTrustBadges from "./grammar/GrammarTrustBadges.vue";
import type {
  GrammarConcept,
  GrammarConceptBundle,
  GrammarExplanationDocument,
  GrammarSense,
  GrammarReviewStatus,
} from "../types";

const props = defineProps<{ show: boolean }>();
const emit = defineEmits<{ close: [] }>();

const query = ref("");
const family = ref("");
const jlptLevel = ref("");
const results = ref<GrammarConcept[]>([]);
const selected = ref<GrammarConceptBundle | null>(null);
const loading = ref(false);
const detailLoading = ref(false);
const error = ref("");
let searchGeneration = 0;
let detailGeneration = 0;
let searchTimer: number | null = null;

const selectedExplanation = computed(() => selected.value?.explanation ?? null);
const selectedReviewOverride = computed(() => {
  const conceptId = selected.value?.concept.concept_id;
  return conceptId ? grammarReviewOverrides.value[conceptId] : undefined;
});
const selectedReviewStatus = computed(() => (
  selectedReviewOverride.value?.status ?? selectedExplanation.value?.review_status ?? "unverified"
));
const selectedReviewer = computed(() => selectedReviewOverride.value?.reviewer ?? "");
const selectedReviewedAt = computed(() => selectedReviewOverride.value?.reviewedAt ?? "");
const reviewerDraft = ref("");
const reviewOptions: { status: GrammarReviewStatus; label: string; icon: string }[] = [
  { status: "unverified", label: "未核验", icon: unverifiedIcon },
  { status: "ai_checked", label: "AI 批量核验", icon: aiCheckedIcon },
  { status: "trusted", label: "成员权威核验", icon: trustedIcon },
];

function explanationForSense(sense: GrammarSense): GrammarExplanationDocument | null {
  return selected.value?.explanations.find((item) => item.explanation_id === sense.explanation_id) ?? null;
}

function updateReviewStatus(status: GrammarReviewStatus) {
  const conceptId = selected.value?.concept.concept_id;
  if (!conceptId) return;
  const reviewer = status === "trusted" ? (reviewerDraft.value.trim() || "本机成员") : "";
  setGrammarReviewOverride(conceptId, status, reviewer);
  if (status === "trusted" && !reviewerDraft.value.trim()) reviewerDraft.value = reviewer;
}

function updateReviewer() {
  if (selectedReviewStatus.value !== "trusted") return;
  updateReviewStatus("trusted");
}

function resetReviewStatus() {
  const conceptId = selected.value?.concept.concept_id;
  if (!conceptId) return;
  clearGrammarReviewOverride(conceptId);
  reviewerDraft.value = "";
}

async function selectConcept(conceptId: string) {
  const generation = ++detailGeneration;
  detailLoading.value = true;
  error.value = "";
  try {
    const detail = await getGrammarConcept(conceptId);
    if (generation === detailGeneration) selected.value = detail;
  } catch (reason) {
    if (generation === detailGeneration) error.value = String(reason);
  } finally {
    if (generation === detailGeneration) detailLoading.value = false;
  }
}

async function runSearch() {
  if (!props.show) return;
  const generation = ++searchGeneration;
  loading.value = true;
  error.value = "";
  try {
    const concepts = await searchGrammarCatalog({
      query: query.value.trim() || undefined,
      family: family.value || undefined,
      jlptLevel: jlptLevel.value ? Number(jlptLevel.value) : undefined,
      auditStatus: "verified",
    });
    if (generation !== searchGeneration) return;
    results.value = concepts;
    const selectedId = selected.value?.concept.concept_id;
    if (!selectedId || !concepts.some((item) => item.concept_id === selectedId)) {
      if (concepts[0]) await selectConcept(concepts[0].concept_id);
      else selected.value = null;
    }
  } catch (reason) {
    if (generation === searchGeneration) error.value = String(reason);
  } finally {
    if (generation === searchGeneration) loading.value = false;
  }
}

function scheduleSearch(immediate = false) {
  if (searchTimer !== null) window.clearTimeout(searchTimer);
  searchTimer = window.setTimeout(runSearch, immediate ? 0 : 180);
}

watch(
  () => props.show,
  (show) => {
    if (show) scheduleSearch(true);
    else {
      searchGeneration += 1;
      detailGeneration += 1;
    }
  },
  { immediate: true },
);
watch([query, family, jlptLevel], () => scheduleSearch());
watch(
  () => selected.value?.concept.concept_id,
  () => {
    reviewerDraft.value = selectedReviewOverride.value?.reviewer ?? "";
  },
);

onBeforeUnmount(() => {
  if (searchTimer !== null) window.clearTimeout(searchTimer);
});
</script>

<template>
  <Transition name="grammar-library-fade">
    <section v-if="show" class="grammar-library" role="dialog" aria-modal="true" aria-label="语法知识库">
      <header class="library-header">
        <div>
          <span>Grammar Catalog</span>
          <h1>语法知识库</h1>
          <p>按稳定 concept 浏览讲解；正文中的实际作用仍由 occurrence 精确解析。</p>
        </div>
        <button type="button" class="close-button" @click="emit('close')">关闭</button>
      </header>

      <div class="library-filters">
        <label>
          <span>搜索</span>
          <input v-model="query" type="search" placeholder="名称、形态、功能或语义" autofocus />
        </label>
        <label>
          <span>知识族</span>
          <select v-model="family">
            <option value="">全部</option>
            <option value="particle">助词</option>
            <option value="functional_morpheme">功能语素</option>
            <option value="construction">构式</option>
            <option value="formal_noun">形式名词</option>
            <option value="morphology_feature">活用特征</option>
          </select>
        </label>
        <label>
          <span>JLPT</span>
          <select v-model="jlptLevel">
            <option value="">全部</option>
            <option v-for="level in 5" :key="level" :value="level">N{{ level }}</option>
          </select>
        </label>
        <output>{{ loading ? "检索中" : `${results.length} 项` }}</output>
      </div>

      <p v-if="error" class="library-error">{{ error }}</p>

      <div class="library-body">
        <nav class="concept-list" aria-label="语法概念列表">
          <button
            v-for="concept in results"
            :key="concept.concept_id"
            type="button"
            :class="{ active: selected?.concept.concept_id === concept.concept_id }"
            @click="selectConcept(concept.concept_id)"
          >
            <strong>{{ concept.canonical_label }}</strong>
            <span>{{ concept.kind }}<template v-if="concept.jlpt_level"> · N{{ concept.jlpt_level }}</template></span>
            <small>{{ concept.function_tags.slice(0, 3).join(" · ") }}</small>
          </button>
          <p v-if="!loading && !results.length" class="empty-state">没有符合条件的语法概念。</p>
        </nav>

        <article class="concept-detail" :aria-busy="detailLoading">
          <div v-if="detailLoading && !selected" class="empty-state">正在读取讲解…</div>
          <template v-else-if="selected && selectedExplanation">
            <header class="concept-heading">
              <div>
                <span>{{ selected.concept.semantic_domains.slice(0, 3).join(" · ") || selected.concept.kind }}</span>
                <h2>{{ selectedExplanation.title }}</h2>
                <p>{{ selectedExplanation.compact_summary }}</p>
              </div>
              <GrammarTrustBadges
                :provenance="selectedExplanation.provenance"
                :review-status="selectedReviewStatus"
                :reviewer="selectedReviewer"
                :reviewed-at="selectedReviewedAt"
              />
            </header>

            <section class="review-editor" aria-label="核验等级">
              <div>
                <strong>核验等级</strong>
                <span>只覆盖本机显示，不改写内置目录。</span>
              </div>
              <div class="review-options">
                <button
                  v-for="option in reviewOptions"
                  :key="option.status"
                  type="button"
                  :class="{ active: selectedReviewStatus === option.status }"
                  :title="option.label"
                  @click="updateReviewStatus(option.status)"
                >
                  <img :src="option.icon" alt="" />
                  <span>{{ option.label }}</span>
                </button>
              </div>
              <input
                v-if="selectedReviewStatus === 'trusted'"
                v-model="reviewerDraft"
                type="text"
                placeholder="核验成员"
                aria-label="核验成员"
                @change="updateReviewer"
              />
              <button
                v-if="selectedReviewOverride"
                type="button"
                class="reset-review"
                @click="resetReviewStatus"
              >恢复目录值</button>
            </section>

            <dl class="concept-summary">
              <template v-if="selectedExplanation.function_summary">
                <dt>功能</dt><dd>{{ selectedExplanation.function_summary }}</dd>
              </template>
              <template v-if="selectedExplanation.connection">
                <dt>接续</dt><dd>{{ selectedExplanation.connection }}</dd>
              </template>
              <template v-if="selectedExplanation.formation">
                <dt>构成</dt><dd>{{ selectedExplanation.formation }}</dd>
              </template>
              <template v-if="selected.concept.aliases.length">
                <dt>别名</dt><dd>{{ selected.concept.aliases.join("、") }}</dd>
              </template>
              <template v-if="selected.concept.register.length">
                <dt>语域</dt><dd>{{ selected.concept.register.join("、") }}</dd>
              </template>
            </dl>

            <section v-if="selected.senses.length" class="detail-section">
              <h3>语义分支</h3>
              <div class="sense-grid">
                <article v-for="sense in selected.senses" :key="sense.sense_id">
                  <header><strong>{{ sense.label }}</strong></header>
                  <p>{{ explanationForSense(sense)?.function_summary || sense.function_summary }}</p>
                  <small v-if="sense.context_requirements.length">条件：{{ sense.context_requirements.join("；") }}</small>
                  <small v-if="sense.exclusion_conditions.length">排除：{{ sense.exclusion_conditions.join("；") }}</small>
                </article>
              </div>
            </section>

            <section v-if="selectedExplanation.body_blocks.length" class="detail-section">
              <h3>讲解</h3>
              <div
                v-for="(block, index) in selectedExplanation.body_blocks"
                :key="`${block.kind}-${index}`"
                :class="['library-block', `library-block-${block.kind}`]"
              >
                <strong v-if="block.label">{{ block.label }}</strong>
                <p>{{ block.text }}</p>
              </div>
            </section>

            <div class="detail-columns">
              <section v-if="selectedExplanation.examples.length" class="detail-section">
                <h3>正例</h3>
                <ul><li v-for="item in selectedExplanation.examples" :key="item">{{ item }}</li></ul>
              </section>
              <section v-if="selectedExplanation.counter_examples.length" class="detail-section">
                <h3>反例</h3>
                <ul><li v-for="item in selectedExplanation.counter_examples" :key="item">{{ item }}</li></ul>
              </section>
            </div>

            <section
              v-if="selected.concept.related_concept_ids.length || selected.concept.contrast_concept_ids.length"
              class="detail-section relation-section"
            >
              <h3>相关与对比</h3>
              <button
                v-for="conceptId in selected.concept.related_concept_ids"
                :key="`related-${conceptId}`"
                type="button"
                @click="selectConcept(conceptId)"
              >相关：{{ conceptId }}</button>
              <button
                v-for="conceptId in selected.concept.contrast_concept_ids"
                :key="`contrast-${conceptId}`"
                type="button"
                @click="selectConcept(conceptId)"
              >对比：{{ conceptId }}</button>
            </section>

            <footer class="concept-footer">
              <span>{{ selected.concept.concept_id }}</span>
            </footer>
          </template>
          <div v-else class="empty-state">从左侧选择一个语法概念。</div>
        </article>
      </div>
    </section>
  </Transition>
</template>

<style scoped>
.grammar-library { position: fixed; inset: 18px; z-index: 1200; display: grid; grid-template-rows: auto auto minmax(0, 1fr); overflow: hidden; border: 1px solid var(--border-color); border-radius: var(--radius-lg); background: color-mix(in srgb, var(--bg-primary) 96%, transparent); box-shadow: 0 24px 70px rgba(18, 28, 48, .2); backdrop-filter: blur(22px); }
.library-header { display: flex; justify-content: space-between; gap: 24px; align-items: flex-start; padding: 22px 26px 16px; border-bottom: 1px solid var(--border-color); }
.library-header span, .concept-heading > div > span { color: #1769aa; font: 800 .68rem/1.3 var(--font-ui); letter-spacing: .09em; text-transform: uppercase; }
.library-header h1 { margin-top: 3px; font-size: 1.45rem; }
.library-header p { margin-top: 3px; color: var(--text-muted); font-size: .78rem; }
button { border: 1px solid var(--border-color); background: transparent; color: var(--text-secondary); cursor: pointer; font: inherit; }
.close-button { padding: 7px 13px; border-radius: 999px; }
.library-filters { display: grid; grid-template-columns: minmax(220px, 1fr) minmax(130px, .3fr) minmax(100px, .2fr) auto; gap: 12px; align-items: end; padding: 13px 26px; border-bottom: 1px solid var(--border-color); background: var(--bg-secondary); }
.library-filters label { display: grid; gap: 4px; min-width: 0; }
.library-filters label span { color: var(--text-muted); font-size: .68rem; font-weight: 700; }
.library-filters input, .library-filters select { width: 100%; min-width: 0; height: 36px; padding: 0 10px; border: 1px solid var(--border-color); border-radius: 8px; background: var(--bg-primary); color: var(--text-primary); outline: none; }
.library-filters input:focus, .library-filters select:focus { border-color: #1769aa; box-shadow: 0 0 0 2px color-mix(in srgb, #1769aa 12%, transparent); }
.library-filters output { padding-bottom: 7px; color: var(--text-muted); font-size: .72rem; white-space: nowrap; }
.library-error { padding: 8px 26px; background: color-mix(in srgb, #a8323e 9%, var(--bg-primary)); color: #a8323e; font-size: .78rem; }
.library-body { display: grid; grid-template-columns: minmax(210px, 290px) minmax(0, 1fr); min-height: 0; }
.concept-list { overflow: auto; border-right: 1px solid var(--border-color); background: color-mix(in srgb, var(--bg-secondary) 70%, transparent); }
.concept-list > button { display: grid; width: 100%; gap: 2px; padding: 12px 16px; border: 0; border-bottom: 1px solid color-mix(in srgb, var(--border-color) 70%, transparent); border-radius: 0; text-align: left; }
.concept-list > button:hover, .concept-list > button.active { background: color-mix(in srgb, #1769aa 8%, var(--bg-primary)); }
.concept-list > button.active { box-shadow: inset 3px 0 #1769aa; }
.concept-list strong { color: var(--text-primary); }
.concept-list span, .concept-list small { overflow: hidden; color: var(--text-muted); font-size: .68rem; text-overflow: ellipsis; white-space: nowrap; }
.concept-detail { min-width: 0; overflow: auto; padding: 24px clamp(22px, 4vw, 52px) 36px; }
.concept-heading { display: flex; justify-content: space-between; gap: 24px; align-items: flex-start; }
.concept-heading h2 { margin-top: 4px; font-size: clamp(1.35rem, 2vw, 2rem); }
.concept-heading p { margin-top: 5px; color: var(--text-secondary); }
.review-editor { display: flex; flex-wrap: wrap; gap: 10px 14px; align-items: center; margin-top: 18px; padding: 11px 13px; border: 1px solid color-mix(in srgb, var(--border-color) 76%, transparent); border-radius: 11px; background: color-mix(in srgb, var(--bg-secondary) 64%, transparent); }
.review-editor > div:first-child { display: grid; margin-right: auto; }
.review-editor > div:first-child strong { font-size: .74rem; }
.review-editor > div:first-child span { color: var(--text-muted); font-size: .64rem; }
.review-options { display: flex; gap: 12px; align-items: flex-end; }
.review-options button { position: relative; display: grid; justify-items: center; gap: 2px; min-width: 46px; padding: 2px; border: 0; border-radius: 0; background: transparent; color: var(--text-muted); font-size: .61rem; }
.review-options button:hover, .review-options button.active { background: transparent; color: var(--text-primary); }
.review-options button:focus-visible { outline: 2px solid color-mix(in srgb, #1769aa 46%, transparent); outline-offset: 3px; border-radius: 6px; }
.review-options button.active::after { position: absolute; right: 8px; bottom: 14px; width: 5px; height: 5px; border: 2px solid var(--bg-secondary); border-radius: 50%; background: #2f83c6; content: ""; }
.review-options img { width: 36px; height: 36px; object-fit: contain; filter: grayscale(.18) opacity(.76); transition: filter .12s ease, transform .12s ease; }
.review-options button:hover img, .review-options button.active img { filter: none; transform: translateY(-1px); }
.review-editor input { width: 120px; height: 31px; padding: 0 8px; border: 1px solid var(--border-color); border-radius: 7px; background: var(--bg-primary); color: var(--text-primary); font-size: .7rem; }
.reset-review { padding: 4px 7px; border: 0; color: var(--text-muted); font-size: .64rem; }
.concept-summary { display: grid; grid-template-columns: max-content minmax(0, 1fr); gap: 7px 16px; margin-top: 22px; padding: 16px 18px; border: 1px solid var(--border-color); border-radius: var(--radius-md); }
.concept-summary dt { color: var(--text-muted); font-size: .72rem; }
.concept-summary dd { margin: 0; color: var(--text-secondary); }
.detail-section { margin-top: 24px; }
.detail-section h3 { margin-bottom: 9px; color: var(--text-primary); font-size: .82rem; }
.sense-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 8px; }
.sense-grid article { padding: 11px 13px; border: 1px solid var(--border-color); border-radius: 9px; }
.sense-grid header { display: flex; justify-content: space-between; gap: 8px; }
.sense-grid header span, .sense-grid small { color: var(--text-muted); font-size: .66rem; }
.sense-grid p { margin: 4px 0; color: var(--text-secondary); font-size: .78rem; }
.sense-grid small { display: block; }
.library-block { padding: 9px 0; border-top: 1px solid color-mix(in srgb, var(--border-color) 70%, transparent); }
.library-block:first-of-type { border-top: 0; }
.library-block strong { font-size: .74rem; }
.library-block p { margin-top: 2px; color: var(--text-secondary); }
.library-block-warning { margin-top: 7px; padding: 9px 11px; border-left: 2px solid #1769aa; background: color-mix(in srgb, #1769aa 5%, transparent); }
.detail-columns { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 24px; }
.detail-section ul { display: grid; gap: 5px; padding-left: 18px; color: var(--text-secondary); }
.relation-section { display: flex; flex-wrap: wrap; gap: 7px; align-items: center; }
.relation-section h3 { flex-basis: 100%; }
.relation-section button { padding: 5px 8px; border-radius: 999px; font-size: .7rem; }
.concept-footer { display: grid; gap: 3px; margin-top: 28px; padding-top: 12px; border-top: 1px solid var(--border-color); color: var(--text-muted); font-size: .68rem; }
.empty-state { padding: 30px 18px; color: var(--text-muted); text-align: center; }
.grammar-library-fade-enter-active, .grammar-library-fade-leave-active { transition: opacity .14s ease, transform .14s ease; }
.grammar-library-fade-enter-from, .grammar-library-fade-leave-to { opacity: 0; transform: translateY(4px); }
@media (max-width: 760px) { .grammar-library { inset: 8px; }.library-filters { grid-template-columns: 1fr 1fr; }.library-filters label:first-child { grid-column: 1 / -1; }.library-body { grid-template-columns: 1fr; }.concept-list { max-height: 33vh; border-right: 0; border-bottom: 1px solid var(--border-color); }.concept-heading { display: grid; }.detail-columns { grid-template-columns: 1fr; }.review-editor { align-items: flex-start; }.review-options { flex-wrap: wrap; } }
</style>
