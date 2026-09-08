<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { ArrowLeft, ArrowRight, Check, Copy, FileText, LoaderCircle, Play, Search, X } from '@lucide/vue';
import DictionaryContent from './components/dictionary/DictionaryContent.vue';
import { nlpRequest } from './services/nlp';
import type { MorphemeToken, QueryOutput, Register, UnifiedDocument } from './types/nlp';
import './styles/inspection.css';

const input = ref('七日は警察署へ向かった。\n「めっちゃすごいじゃん」と彼女は云う。');
const register = ref<Register>('cwj');
const document = ref<UnifiedDocument | null>(null);
const selected = ref<MorphemeToken | null>(null);
const result = ref<QueryOutput | null>(null);
const busy = ref(false);
const queryBusy = ref(false);
const error = ref('');
const queryError = ref('');
const tab = ref<'dictionary' | 'metadata'>('dictionary');
const formIndex = ref(0);
const dictionary = ref('');
const search = ref('');
const copied = ref(false);
const available = ref<Record<string, boolean>>({ cwj: true, csj: true });
let analysisGeneration = 0;
let queryGeneration = 0;
let hoverTimer: ReturnType<typeof setTimeout> | undefined;
const characters = computed(() => Array.from(input.value).length);
const sourceToken = computed(() => selected.value && document.value?.source.tokens[selected.value.source_index]);
const selectedRubies = computed(() => {
  if (!selected.value || !document.value) return [];
  return document.value.ruby_validations.filter((item) => item.token_range && selected.value!.source_index >= item.token_range[0] && selected.value!.source_index < item.token_range[1]);
});
const rubySummary = computed(() => {
  const validations = document.value?.ruby_validations || [];
  return {
    total: validations.length,
    matched: validations.filter((item) => item.status === 'matched').length,
    variant: validations.filter((item) => item.status === 'variant').length,
    pending: validations.filter((item) => item.status === 'unavailable' || item.status === 'unmatched').length,
  };
});
const group = computed(() => result.value?.groups[formIndex.value]);
const dictionaries = computed(() => [...new Set(group.value?.entries.map(e => e.dict_name) || [])]);
const entries = computed(() => group.value?.entries.filter(e => !dictionary.value || e.dict_name === dictionary.value) || []);
const segments = computed(() => {
  if (!document.value) return [];
  return [
    ...document.value.morphemes.map(token => ({ start: token.char_range[0], surface: token.surface, token })),
    ...document.value.gaps.map(gap => ({ start: gap.char_range[0], surface: gap.surface, token: null })),
  ].sort((a, b) => a.start - b.start);
});

function invalidate() {
  analysisGeneration++; queryGeneration++; clearTimeout(hoverTimer);
  document.value = null; selected.value = null; result.value = null; queryBusy.value = false;
}
async function analyze() {
  invalidate(); const generation = analysisGeneration;
  busy.value = true; error.value = ''; queryError.value = '';
  try {
    const value = await nlpRequest<UnifiedDocument>({ command: 'analyze', text: input.value, register: register.value });
    if (generation !== analysisGeneration) return;
    document.value = value;
  } catch (e) { if (generation === analysisGeneration) error.value = String(e); }
  finally { if (generation === analysisGeneration) busy.value = false; }
}
async function select(token: MorphemeToken) {
  clearTimeout(hoverTimer);
  if (!document.value) return;
  if (selected.value?.id === token.id && (queryBusy.value || (result.value?.token_id === token.id && result.value.analysis_id === document.value.id))) return;
  selected.value = token; search.value = token.surface;
  await query({ command: 'query', analysis_id: document.value.id, token_id: token.id });
}
async function query(request: Record<string, unknown>) {
  const generation = ++queryGeneration;
  queryBusy.value = true; queryError.value = ''; result.value = null; formIndex.value = 0; dictionary.value = '';
  try {
    const value = await nlpRequest<QueryOutput>(request);
    if (generation !== queryGeneration) return;
    result.value = value;
    const found = value.groups.findIndex(g => g.entries.length > 0);
    formIndex.value = Math.max(0, found);
    dictionary.value = value.groups[formIndex.value]?.entries[0]?.dict_name || '';
  } catch (e) { if (generation === queryGeneration) queryError.value = String(e); }
  finally { if (generation === queryGeneration) queryBusy.value = false; }
}
function preview(token: MorphemeToken) {
  clearTimeout(hoverTimer);
  hoverTimer = setTimeout(() => select(token), 250);
}
function cancelPreview() { clearTimeout(hoverTimer); }
function chooseForm(index: number) {
  formIndex.value = index; dictionary.value = result.value?.groups[index]?.entries[0]?.dict_name || '';
}
function navigate(word: string) { tab.value = 'dictionary'; search.value = word; query({ command: 'search', word }); }
function move(direction: number) {
  if (!document.value) return;
  const index = document.value.morphemes.findIndex(t => t.id === selected.value?.id);
  const token = document.value.morphemes[index + direction]; if (token) select(token);
}
async function copy() {
  if (!sourceToken.value) return;
  try { await navigator.clipboard.writeText(sourceToken.value.raw_feature); copied.value = true; setTimeout(() => copied.value = false, 1500); }
  catch { queryError.value = '复制失败'; }
}
async function openText(event: Event) {
  const file = (event.target as HTMLInputElement).files?.[0];
  if (!file) return;
  input.value = await file.text(); invalidate(); error.value = ''; (event.target as HTMLInputElement).value = '';
}
onMounted(async () => {
  try {
    const status = await nlpRequest<{ providers: { register: string; available: boolean }[] }>({ command: 'status' });
    available.value = Object.fromEntries(status.providers.map(p => [p.register, p.available]));
  } catch (e) { error.value = String(e); }
});
</script>

<template>
  <div class="inspection-app">
    <header class="app-header">
      <div class="brand"><img src="/kotoclip.png" alt="" /><strong>Kotoclip</strong><span>UniDic 2025.12</span></div>
      <div class="resource-state"><span class="status-dot"></span>本地分析</div>
    </header>
    <main class="workspace">
      <section class="reading-workspace">
        <div class="input-toolbar">
          <div class="segmented" aria-label="分析词典">
            <button v-for="mode in (['cwj', 'csj'] as const)" :key="mode" :class="{ active: register === mode }" :aria-pressed="register === mode" :disabled="busy" @click="register = mode; invalidate()">{{ mode === 'cwj' ? '书面语 CWJ' : '口语 CSJ' }}</button>
          </div>
          <div class="toolbar-actions">
            <label class="icon-button" title="打开文本"><FileText :size="18" /><input type="file" accept=".txt,.md" aria-label="打开文本" :disabled="busy" @change="openText" /></label>
            <button class="icon-button" title="清空" aria-label="清空" :disabled="busy" @click="input = ''; invalidate()"><X :size="18" /></button>
            <button class="primary-button" :disabled="busy || !input.trim() || characters > 20000 || !available[register]" @click="analyze"><LoaderCircle v-if="busy" class="spin" :size="16" /><Play v-else :size="16" />{{ busy ? '分析中' : '分析' }}</button>
          </div>
        </div>
        <textarea v-model="input" class="source-input" lang="ja" aria-label="日文正文" placeholder="日文正文" :disabled="busy" @input="invalidate" />
        <div class="input-footer"><span :class="{ invalid: characters > 20000 }">{{ characters.toLocaleString() }} / 20,000 字符</span><span v-if="!available[register]" class="invalid">词典资源未找到</span></div>
        <p v-if="error" class="error-message" role="alert">{{ error }}</p>
        <div class="result-toolbar"><h1>分词结果</h1><span v-if="document">{{ document.morphemes.length }} 词 · {{ Math.round(document.elapsed_ms) }} ms · {{ document.source.provider.id }}<template v-if="document.routing.reason === 'long_dialogue'"> · 长对话自动使用 CSJ</template><template v-if="rubySummary.total"> · 注音 {{ rubySummary.matched }}/{{ rubySummary.total }} 匹配<template v-if="rubySummary.variant"> · 读音差异 {{ rubySummary.variant }}</template><template v-if="rubySummary.pending"> · 待核验 {{ rubySummary.pending }}</template></template></span></div>
        <div v-if="busy" class="empty-state" role="status"><LoaderCircle class="spin" :size="22" />正在分析</div>
        <div v-else-if="!document" class="empty-state">暂无分析结果</div>
        <article v-else class="token-text" lang="ja" aria-label="分词结果"><template v-for="segment in segments" :key="segment.start"><button v-if="segment.token" class="word" :class="{ selected: selected?.id === segment.token.id, unknown: document.source.tokens[segment.token.source_index].lexicon_type === 'unknown' }" :aria-label="segment.surface" :aria-pressed="selected?.id === segment.token.id" @click="select(segment.token)" @mouseenter="preview(segment.token)" @mouseleave="cancelPreview" @focus="select(segment.token)">{{ segment.surface }}</button><span v-else>{{ segment.surface }}</span></template></article>
      </section>
      <aside class="inspector">
        <div class="inspector-heading"><div><h2 lang="ja">{{ selected?.surface || '词语详情' }}</h2><p v-if="selected">{{ selected.pos.filter(Boolean).join(' / ') }} · [{{ selected.char_range.join(', ') }})</p></div><div class="inspector-navigation"><button class="icon-button" title="前一词" aria-label="前一词" :disabled="!selected || selected.source_index === 0" @click="move(-1)"><ArrowLeft :size="17" /></button><button class="icon-button" title="后一词" aria-label="后一词" :disabled="!selected || selected.source_index === (document?.morphemes.length || 0) - 1" @click="move(1)"><ArrowRight :size="17" /></button></div></div>
        <div class="panel-tabs" role="tablist"><button role="tab" :aria-selected="tab === 'dictionary'" :class="{ active: tab === 'dictionary' }" @click="tab = 'dictionary'">词典</button><button role="tab" :aria-selected="tab === 'metadata'" :class="{ active: tab === 'metadata' }" @click="tab = 'metadata'">UniDic 元数据</button></div>
        <div v-show="tab === 'dictionary'" class="dictionary-panel">
          <form class="search-form" @submit.prevent="navigate(search)"><input v-model="search" aria-label="查询词" placeholder="查询词" maxlength="100" /><button class="icon-button" aria-label="查询" title="查询" :disabled="!search.trim()"><Search :size="18" /></button></form>
          <div v-if="queryBusy" class="empty-state" role="status"><LoaderCircle class="spin" :size="20" />查询中</div>
          <p v-else-if="queryError" class="error-message" role="alert">{{ queryError }}</p>
          <template v-else-if="result">
            <div class="query-forms"><button v-for="(item, index) in result.groups" :key="index" :class="{ active: formIndex === index }" @click="chooseForm(index)"><span lang="ja">{{ item.form.form }}</span><small>{{ ({ observed: '出现形', base: '基本形', lemma: '词元', search: '查询' } as Record<string,string>)[item.form.kind] }}</small></button></div>
            <p v-if="group?.form.reading" class="query-reading" lang="ja">{{ group.form.reading }} <span>{{ group.form.reading_field }}</span></p>
            <div v-if="dictionaries.length" class="dictionary-tabs"><button v-for="name in dictionaries" :key="name" :class="{ active: dictionary === name }" @click="dictionary = name">{{ name }}</button></div>
            <article v-for="entry in entries" :key="entry.occurrence_id + entry.entry_key" class="entry"><header><h3 lang="ja">{{ entry.header.display_form || entry.headword }}</h3><span lang="ja">{{ entry.reading }}</span></header><DictionaryContent :entry="entry" @navigate="navigate" /></article>
            <p v-if="!entries.length" class="empty-state">{{ result.dictionary_names.length ? '未找到词条' : '本地查询词典未就绪' }}</p>
            <p v-if="group && group.total > group.entries.length" class="query-reading">显示前 {{ group.entries.length }} 条，共 {{ group.total }} 条</p>
          </template>
          <div v-else class="empty-state">尚未选择词语</div>
        </div>
        <div v-show="tab === 'metadata'" class="metadata-panel">
          <template v-if="sourceToken">
            <dl class="metadata-summary"><dt>来源</dt><dd>{{ document?.source.provider.id }}</dd><dt>词典类别</dt><dd>{{ sourceToken.lexicon_type }}</dd><dt>连接 ID</dt><dd>{{ sourceToken.left_id }} / {{ sourceToken.right_id }}</dd><dt>词成本 / 累计成本</dt><dd>{{ sourceToken.word_cost }} / {{ sourceToken.total_cost }}</dd><dt>UTF-8 范围</dt><dd>[{{ sourceToken.byte_range.join(', ') }})</dd></dl>
            <div v-for="ruby in selectedRubies" :key="ruby.char_range.join(':')" class="ruby-validation" :class="`is-${ruby.status}`">
              <div class="ruby-validation-heading"><strong>书名号注音验证</strong><span>{{ ruby.status === 'matched' ? (ruby.reason === 'small_kana' ? '匹配（大小假名）' : '匹配') : ruby.status === 'variant' ? '读音差异' : '待核验' }}</span></div>
              <p><span lang="ja">{{ ruby.base }}</span> · 作者注音 {{ ruby.ruby_reading }}</p>
              <p>UniDic 出现假名：{{ ruby.observed_reading || (ruby.reason === 'missing_reading' ? '未提供' : '局部范围待确定') }} · 涉及 {{ ruby.token_range ? `${ruby.token_range[1] - ruby.token_range[0]} 词` : '无' }}</p>
            </div>
            <table class="metadata-table"><thead><tr><th>字段</th><th>原始值</th></tr></thead><tbody><tr v-for="field in sourceToken.fields" :key="field.index"><th><code>{{ field.index }} · {{ field.name }}</code><small>{{ field.label }}</small></th><td lang="ja">{{ field.raw === null ? '未提供' : field.raw === '' ? '(空值)' : field.raw }}</td></tr></tbody></table>
            <div class="raw-heading"><h3>原始 CSV</h3><button class="icon-button" title="复制原始 CSV" aria-label="复制原始 CSV" @click="copy"><Check v-if="copied" :size="17" /><Copy v-else :size="17" /></button></div><pre>{{ sourceToken.raw_feature }}</pre>
            <details><summary>资源信息</summary><p>{{ document?.schema }}</p><p>{{ document?.source.provider.field_schema }}</p><code>{{ document?.source.provider.dictionary_sha256 }}</code></details>
          </template><div v-else class="empty-state">尚未选择词语</div>
        </div>
      </aside>
    </main>
  </div>
</template>
