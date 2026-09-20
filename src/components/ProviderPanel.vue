<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { nlpRequest } from '../services/nlp';
import type { ProviderSettings, SourceArtifact, SourceEndpoint, StructureGraph, UnifiedDocument } from '../types/nlp';

const props = defineProps<{ sources: SourceArtifact[]; graph: StructureGraph | null; alignments: UnifiedDocument['provider_token_alignments']; pending: boolean; canAnalyze: boolean; diagnostics: { id: string; status: string; error?: string }[] }>();
const emit = defineEmits<{ retry: []; cancel: [] }>();
const settings = ref<ProviderSettings | null>(null);
const error = ref('');
const saved = ref(false);
const saving = ref(false);
const providerId = ref('ginza');
const kind = ref('bunsetsu');
const source = computed(() => props.sources.find(item => item.provider.id === providerId.value));
const nodes = computed(() => source.value?.nodes.filter(node => node.kind === kind.value) || []);
const mapped = computed(() => new Map(props.graph?.entities.filter(entity => entity.provider === providerId.value).map(entity => [entity.source_id, entity]) || []));
const alignment = computed(() => props.alignments.find(item => item.external_provider === providerId.value));
function choice(id: string) {
  const entity = mapped.value.get(id);
  if (!entity?.complete) return '对齐待定';
  const candidate = props.graph?.candidates.find(candidate => candidate.evidence.includes(entity.id));
  return candidate?.selected && candidate.preferred_entity === entity.id ? '默认结构' : candidate ? '保留候选' : '已对齐';
}
const kinds: Record<string, string> = { token: '词元', compound: '复合词', bunsetsu: '文节', basic_phrase: '基本句', sentence: '句子', clause: '小句', predicate: '谓语', entity: '实体' };
const states: Record<string, string> = { ready: '完成', failed: '失败', disabled: '已停用', cancelled: '已取消' };
function endpoint(value: SourceEndpoint): string {
  if (value.kind === 'root') return '句法根';
  if (value.kind === 'exophora') return value.label;
  return `${source.value?.nodes.find(node => node.id === value.id)?.surface || value.id} (${value.id})`;
}
async function save() {
  if (!settings.value) return;
  error.value = ''; saved.value = false; saving.value = true;
  try { await nlpRequest({ command: 'configure_providers', settings: settings.value }); saved.value = true; }
  catch (e) { error.value = String(e); }
  finally { saving.value = false; }
}
onMounted(async () => {
  try { settings.value = (await nlpRequest<{ settings: ProviderSettings }>({ command: 'provider_status' })).settings; }
  catch (e) { error.value = String(e); }
});
</script>

<template>
  <section class="provider-panel" aria-label="结构分析">
    <div class="provider-heading"><h2>结构分析</h2><button v-if="pending" @click="emit('cancel')">取消</button><button v-else :disabled="!canAnalyze" @click="emit('retry')">分析结构</button></div>
    <p v-if="pending" role="status">正在加载或分析本机模型</p>
    <p v-for="item in diagnostics" :key="item.id" :role="item.error ? 'alert' : undefined">{{ item.id }}：{{ states[item.status] || item.status }}<span v-if="item.error"> · {{ item.error }}</span></p>
    <details>
      <summary>本机模型设置</summary>
      <form v-if="settings" @submit.prevent="save">
        <fieldset v-for="id in (['ginza', 'kwja'] as const)" :key="id" :disabled="pending || saving">
          <legend>{{ id === 'ginza' ? 'GiNZA' : 'KWJA' }}</legend>
          <label class="provider-toggle"><input v-model="settings[id].enabled" type="checkbox" />启用</label>
          <label>Python 解释器<input v-model="settings[id].python" required /></label>
          <label>模型<input v-model="settings[id].model" required /></label>
          <label>超时（秒）<input v-model.number="settings[id].timeout_seconds" type="number" min="1" required /></label>
        </fieldset>
        <label>KWJA 模型目录<input v-model="settings.kwja_cache" :disabled="pending || saving" /></label>
        <label>Hugging Face 缓存目录<input v-model="settings.hf_cache" :disabled="pending || saving" /></label>
        <button type="submit" :disabled="pending || saving">{{ saving ? '保存中' : '保存设置' }}</button><span v-if="saved" role="status">已保存，下次分析使用新设置</span>
      </form>
    </details>
    <p v-if="error" role="alert">{{ error }}</p>
    <div class="provider-filters"><label>来源<select v-model="providerId"><option value="ginza">GiNZA</option><option value="kwja">KWJA</option></select></label><label>结构<select v-model="kind"><option v-for="(label, id) in kinds" :key="id" :value="id">{{ label }}</option></select></label></div>
    <template v-if="source">
      <p>{{ source.provider.model }} · {{ source.provider.version }} · {{ Math.round(source.elapsed_ms) }} ms</p>
      <div class="source-nodes">
        <details v-for="node in nodes" :key="node.id">
          <summary><span lang="ja">{{ node.surface }}</span> <small>{{ node.id }} · {{ node.text_ranges.map(range => `[${range.join(', ')})`).join('、') }}</small></summary>
          <p v-if="node.head">主辞：{{ endpoint({ kind: 'node', id: node.head }) }}</p>
          <p>{{ choice(node.id) }} · 词元：{{ mapped.get(node.id)?.morpheme_ids.join('、') || '无' }}<template v-if="mapped.get(node.id)?.head_morpheme_ids.length"> · 主辞词元：{{ mapped.get(node.id)?.head_morpheme_ids.join('、') }}</template></p>
          <p v-if="mapped.get(node.id)?.diagnostics.length">{{ mapped.get(node.id)?.diagnostics.join('、') }}</p>
          <p v-if="node.source_surface !== node.surface">模型表记：{{ node.source_surface }}</p>
          <pre>{{ JSON.stringify(node.features, null, 2) }}</pre>
        </details>
        <p v-if="!nodes.length">本次结果没有该类结构</p>
      </div>
      <details><summary>结构关系（{{ source.relations.length }}）</summary><div class="source-nodes"><p v-for="relation in source.relations" :key="relation.id"><span lang="ja">{{ endpoint(relation.source) }} → {{ endpoint(relation.target) }}</span> · {{ relation.kind }} / {{ relation.label }}</p></div></details>
      <details v-if="alignment"><summary>词元对齐（{{ alignment.groups.length }} 组）</summary><div class="source-nodes"><details v-for="group in alignment.groups" :key="group.id"><summary>{{ group.cardinality }} · [{{ group.char_range.join(', ') }}) · {{ group.status }}</summary><p>{{ group.provider_tokens.map(token => token.id).join('、') }} → {{ group.morpheme_ids.join('、') }}</p><pre>{{ JSON.stringify({ intersections: group.intersections, provider_gaps: group.provider_gaps, morpheme_gaps: group.morpheme_gaps }, null, 2) }}</pre></details></div></details>
      <details><summary>来源信息与正文映射</summary><pre>{{ JSON.stringify({ provider: source.provider, text_sha256: source.text_sha256, normalized_text: source.normalized_text, deleted_ranges: source.deleted_ranges }, null, 2) }}</pre></details>
    </template>
  </section>
</template>

<style scoped>
.provider-panel { border-top: 1px solid #d9dfe4; padding: 16px 0; font-size: 13px; min-width: 0; }
.provider-heading, .provider-filters { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }
.provider-heading h2 { font-size: 16px; margin-right: auto; }
.provider-panel p, summary { overflow-wrap: anywhere; }
.provider-panel label { display: grid; gap: 4px; margin: 8px 0; min-width: 0; }
.provider-panel input:not([type=checkbox]), select { width: 100%; box-sizing: border-box; min-width: 0; padding: 7px; border: 1px solid #cad2db; border-radius: 5px; }
.provider-panel fieldset { min-width: 0; margin: 10px 0; border: 1px solid #d9dfe4; }
.provider-panel .provider-toggle { display: flex; gap: 8px; }
.provider-panel button { padding: 7px 12px; border: 1px solid #cad2db; border-radius: 5px; cursor: pointer; }
.provider-panel details { margin: 8px 0; }
.provider-panel summary { cursor: pointer; }
.source-nodes { max-height: 360px; overflow: auto; }
.source-nodes details { padding: 6px 4px; border-bottom: 1px solid #e6e9ed; }
.provider-panel pre { white-space: pre-wrap; overflow-wrap: anywhere; max-height: 280px; overflow: auto; }
.provider-panel small { color: #657381; }
</style>
