<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { Plus, Save, Trash2, X } from '@lucide/vue';
import { nlpRequest } from '../services/nlp';
import type { LanguageRule, LanguageRuleMatch, MorphemeToken, RuleSnapshot } from '../types/nlp';

const props = defineProps<{
  show: boolean;
  session: { session_id: string; text_version: string; generation: number } | null;
  unit: { unit_id: string; artifact_revision: number } | null;
  tokens: MorphemeToken[];
  analysisId: string | null;
}>();
const emit = defineEmits<{ (event: 'close'): void }>();

const snapshot = ref<RuleSnapshot | null>(null);
const selected = ref('');
const label = ref('');
const description = ref('');
const kind = ref<LanguageRule['kind']>('grammar_construction');
const priority = ref(60);
const memberText = ref('');
const gap = ref(0);
const scope = ref<'all' | 'document'>('all');
const selectedBase = ref<LanguageRule | null>(null);
const preview = ref<LanguageRuleMatch[]>([]);
const error = ref('');
const busy = ref(false);
const rules = computed(() => snapshot.value?.rules ?? []);

function emptyAtom() {
  return { surfaces: [], base_forms: [], pos_major: [], pos_sub1: [], conjugation_types: [], conjugation_forms: [], morphology_features: [], capture: null, optional: false, gap_before: 0 };
}

function draft(): LanguageRule {
  const indices = memberText.value.split(',').map(value => Number(value.trim())).filter(index => Number.isInteger(index) && index >= 0 && index < props.tokens.length);
  const atoms = indices.length ? [...new Set(indices)].sort((a, b) => a - b).map((index, atomIndex) => ({
    ...emptyAtom(), base_forms: props.tokens[index].query_forms.map(form => form.form), surfaces: [props.tokens[index].surface],
    pos_major: props.tokens[index].pos[0] ? [props.tokens[index].pos[0]!] : [], capture: `member_${atomIndex}`,
    gap_before: atomIndex === 0 ? 0 : gap.value,
  })) : structuredClone(selectedBase.value?.atoms ?? []);
  return { id: selected.value, label: label.value.trim(), description: description.value.trim(), kind: kind.value, atoms, priority: priority.value,
    enabled: selectedBase.value?.enabled ?? true, document_id: scope.value === 'document' ? props.analysisId : null, allow_whitespace: false, concept_id: selectedBase.value?.concept_id ?? null, sense_ids: selectedBase.value?.sense_ids ?? [], display_from: 0, display_to: null,
    source_refs: ['user-rule'], gap_after_atom: kind.value === 'correlative' && atoms.length > 1 ? 0 : null,
    gap_bunsetsu: kind.value === 'correlative' && atoms.length > 1 ? [0, 10] : null };
}

function edit(rule?: LanguageRule) {
  selectedBase.value = rule ? structuredClone(rule) : null;
  selected.value = rule?.id ?? '';
  label.value = rule?.label ?? '';
  description.value = rule?.description ?? '';
  kind.value = rule?.kind ?? 'grammar_construction';
  priority.value = rule?.priority ?? 60;
  memberText.value = '';
  gap.value = rule?.atoms[1]?.gap_before ?? 0;
  scope.value = rule?.document_id ? 'document' : 'all';
  preview.value = [];
}

async function load() {
  snapshot.value = await nlpRequest<RuleSnapshot>({ command: 'list_rules' });
}

async function runPreview() {
  if (!props.session || !props.unit) return;
  error.value = '';
  try {
    const value = await nlpRequest<{ matches: LanguageRuleMatch[] }>({ command: 'preview_rule', ...props.session,
      unit_id: props.unit.unit_id, artifact_revision: props.unit.artifact_revision, rule: draft() });
    preview.value = value.matches;
  } catch (reason) { error.value = String(reason); preview.value = []; }
}

async function save() {
  busy.value = true; error.value = '';
  try { snapshot.value = await nlpRequest<RuleSnapshot>({ command: 'save_rule', rule: draft() }); edit(); }
  catch (reason) { error.value = String(reason); }
  finally { busy.value = false; }
}

async function toggle(rule: LanguageRule) {
  snapshot.value = await nlpRequest<RuleSnapshot>({ command: 'set_rule_enabled', id: rule.id, enabled: !rule.enabled });
}

async function remove(rule: LanguageRule) {
  snapshot.value = await nlpRequest<RuleSnapshot>({ command: 'delete_rule', id: rule.id });
  if (selected.value === rule.id) edit();
}

watch(() => props.show, show => { if (show) void load().catch(reason => { error.value = String(reason); }); });
</script>

<template>
  <div v-if="show" class="rule-panel" role="dialog" aria-modal="true" aria-label="语言规则">
    <header><div><strong>语言规则</strong><span>规则修改后会刷新已分析正文</span></div><button title="关闭" aria-label="关闭" @click="emit('close')"><X :size="18" /></button></header>
    <div class="rule-body">
      <nav>
        <button class="new-rule" @click="edit()"><Plus :size="15" />新规则</button>
        <article v-for="rule in rules" :key="rule.id" :class="{ active: selected === rule.id }">
          <button class="rule-name" @click="edit(rule)"><strong>{{ rule.label }}</strong><span>{{ rule.kind }} · {{ rule.priority }}</span></button>
          <button :title="rule.enabled ? '停用' : '启用'" @click="toggle(rule)">{{ rule.enabled ? '启用' : '停用' }}</button>
          <button title="删除" aria-label="删除" @click="remove(rule)"><Trash2 :size="14" /></button>
        </article>
        <p v-if="!rules.length">尚无用户规则</p>
      </nav>
      <form @submit.prevent="save">
        <label>名称<input v-model="label" required maxlength="80" /></label>
        <label>说明<textarea v-model="description" rows="2" maxlength="400" /></label>
        <div class="fields"><label>类型<select v-model="kind"><option value="idiom">惯用语</option><option value="grammar_construction">语法构式</option><option value="correlative">非连续呼应</option></select></label><label>优先级<input v-model.number="priority" type="number" min="-1000" max="1000" /></label></div>
        <label>作用域<select v-model="scope"><option value="all">全部正文</option><option value="document" :disabled="!analysisId">当前分析单元</option></select></label>
        <label>词元序号<input v-model="memberText" placeholder="例如 3, 4, 7" @input="preview = []" /></label>
        <label v-if="kind === 'correlative'">最大间隔词元<input v-model.number="gap" type="number" min="1" max="128" /></label>
        <p class="hint">词元序号从 0 开始，按正文当前分析结果填写。固定表记、辞书形和词性会共同参与匹配。</p>
        <p v-if="error" class="error">{{ error }}</p>
        <output v-if="preview.length">命中 {{ preview.length }} 处：{{ preview.map(item => `[${item.char_range.join(', ')})`).join('、') }}</output>
        <div class="actions"><button type="button" :disabled="!label || (!memberText && !selectedBase)" @click="runPreview">预演</button><button class="primary" :disabled="busy || !label || (!memberText && !selectedBase)"><Save :size="15" />保存</button></div>
      </form>
    </div>
  </div>
</template>

<style scoped>
.rule-panel { position: fixed; inset: 7vh 7vw; z-index: 50; display: grid; grid-template-rows: auto minmax(0, 1fr); border: 1px solid var(--border-color); border-radius: 8px; background: var(--bg-primary); box-shadow: 0 18px 60px #0004; color: var(--text-primary); }
header { display: flex; justify-content: space-between; align-items: center; padding: 13px 16px; border-bottom: 1px solid var(--border-color); } header div { display: grid; } header span, .hint, nav p { color: var(--text-muted); font-size: .72rem; }
button, input, select, textarea { font: inherit; } button { display: inline-flex; align-items: center; gap: 5px; border: 1px solid var(--border-color); border-radius: 6px; background: transparent; color: inherit; cursor: pointer; } header button { width: 34px; height: 34px; justify-content: center; }
.rule-body { display: grid; grid-template-columns: minmax(220px, 300px) minmax(0, 1fr); min-height: 0; } nav { overflow: auto; padding: 12px; border-right: 1px solid var(--border-color); } .new-rule { width: 100%; justify-content: center; padding: 8px; margin-bottom: 8px; }
nav article { display: grid; grid-template-columns: minmax(0, 1fr) auto auto; gap: 4px; margin-top: 4px; } nav article.active { background: color-mix(in srgb, #1769aa 8%, transparent); } nav article > button:not(.rule-name) { padding: 5px 7px; } .rule-name { display: grid; min-width: 0; padding: 7px 9px; border: 0; text-align: left; } .rule-name span { overflow: hidden; color: var(--text-muted); font-size: .68rem; text-overflow: ellipsis; }
form { display: grid; align-content: start; gap: 13px; overflow: auto; padding: 22px clamp(20px, 4vw, 48px); } label { display: grid; gap: 5px; color: var(--text-secondary); font-size: .75rem; } input, select, textarea { width: 100%; min-width: 0; padding: 8px 10px; border: 1px solid var(--border-color); border-radius: 6px; background: var(--bg-secondary); color: var(--text-primary); } textarea { resize: vertical; } .fields { display: grid; grid-template-columns: 1fr 140px; gap: 12px; } .actions { display: flex; justify-content: flex-end; gap: 8px; } .actions button { padding: 8px 12px; } .primary { background: #1769aa; color: white; border-color: #1769aa; } .error { color: #a8323e; } output { color: #1769aa; font-size: .75rem; }
@media (max-width: 720px) { .rule-panel { inset: 2vh 2vw; }.rule-body { grid-template-columns: 1fr; grid-template-rows: minmax(120px, 32vh) minmax(0, 1fr); } nav { border-right: 0; border-bottom: 1px solid var(--border-color); }.fields { grid-template-columns: 1fr; } }
</style>
