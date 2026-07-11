<script setup lang="ts">
import { computed } from "vue";
import { AnnotatedToken, DictEntry, DictionaryLink, DictionaryLookup } from "../types";
import DictionaryContent from "./dictionary/DictionaryContent.vue";

const props = defineProps<{
  show: boolean;
  x: number;
  y: number;
  placement: "above" | "below";
  token: AnnotatedToken | null;
  lookup: DictionaryLookup | null;
  loading: boolean;
  canGoBack: boolean;
}>();

const emit = defineEmits<{
  enter: [];
  leave: [];
  navigate: [target: string];
  select: [target: string];
  back: [];
}>();

const formattedPos = computed(() => {
  if (!props.token) return "";
  const head = props.token.bunsetsu.head_word;
  return [head.pos.major, head.pos.sub1].filter((part) => part && part !== "*").join(" · ");
});

const dictionaryGroups = computed(() => {
  const groups = new Map<string, NonNullable<typeof props.lookup>["entries"]>();
  const candidateTargets = new Set(props.lookup?.candidates.map((candidate) => candidate.target) ?? []);
  for (const entry of props.lookup?.entries ?? []) {
    const hasManagedRelation = entry.links.some((link) => !candidateTargets.has(link.target));
    if (!entry.content_blocks.length && !hasManagedRelation) continue;
    const group = groups.get(entry.dict_name) ?? [];
    group.push(entry);
    groups.set(entry.dict_name, group);
  }
  return [...groups.entries()].map(([name, entries]) => ({ name, entries }));
});

const activeHeadword = computed(() =>
  props.lookup?.selected_target
  ?? props.lookup?.entries[0]?.headword
  ?? props.lookup?.query
  ?? props.token?.bunsetsu.head_word.base_form
  ?? "",
);

const isSourceQuery = computed(() =>
  !props.lookup || props.lookup.query === props.token?.bunsetsu.head_word.base_form,
);

const activeReading = computed(() => {
  const preferred = props.lookup?.entries.find((entry) => entry.is_preferred)?.reading;
  if (preferred) return preferred;
  if (props.lookup?.reading) return props.lookup.reading;
  return isSourceQuery.value ? props.token?.bunsetsu.head_word.reading : null;
});

const panelMaxHeight = computed(() => {
  const viewportHeight = typeof window === "undefined" ? 800 : window.innerHeight;
  const available = props.placement === "above" ? props.y - 20 : viewportHeight - props.y - 20;
  return `${Math.max(160, Math.min(Math.round(viewportHeight * 0.7), 620, available))}px`;
});

function relationLabel(relation: string) {
  return ({ candidate: "表记", antonym: "反义", synonym: "近义", parent: "亲项目", child: "子项目", phrase: "惯用句", reference: "参照", related: "关联", redirect: "转至" } as Record<string, string>)[relation] ?? "关联";
}

function candidateLabel(candidate: DictionaryLink) {
  const match = candidate.target.match(/[【〖（](.*?)[】〗）]$/u);
  return match?.[1] || candidate.label || candidate.target;
}

function managedLinkGroups(entry: DictEntry) {
  const candidateTargets = new Set(props.lookup?.candidates.map((candidate) => candidate.target) ?? []);
  const groups = new Map<string, DictionaryLink[]>();
  for (const link of entry.links) {
    if (candidateTargets.has(link.target)) continue;
    const group = groups.get(link.relation) ?? [];
    group.push(link);
    groups.set(link.relation, group);
  }
  return [...groups.entries()].map(([relation, links]) => ({ relation, links }));
}

function handleDefinitionClick(event: MouseEvent) {
  const anchor = (event.target as HTMLElement).closest("a") as HTMLAnchorElement | null;
  if (!anchor) return;
  const marker = "https://kotoclip.invalid/entry/";
  if (anchor.href.startsWith(marker)) {
    event.preventDefault();
    emit("navigate", decodeURIComponent(anchor.href.slice(marker.length)));
  }
}
</script>

<template>
  <Transition name="fade">
    <section
      v-if="show && token"
      class="tooltip-panel"
      :class="`tooltip-${placement}`"
      :style="{ left: x + 'px', top: y + 'px', maxHeight: panelMaxHeight }"
      role="dialog"
      aria-label="词典释义"
      @mouseenter="emit('enter')"
      @mouseleave="emit('leave')"
      @wheel.stop
    >
      <header class="tooltip-header">
        <button v-if="canGoBack" type="button" class="back-button" aria-label="返回上一词条" @click="emit('back')">‹</button>
        <div class="headword-block">
          <span class="base-form">{{ activeHeadword }}</span>
          <span v-if="activeReading" class="reading">【{{ activeReading }}】</span>
        </div>
        <span v-if="isSourceQuery" class="tooltip-pos">{{ formattedPos }}</span>
      </header>

      <div v-if="isSourceQuery && token.bunsetsu.grammar_tags.length" class="tooltip-section grammar-list">
        <div v-for="tag in token.bunsetsu.grammar_tags" :key="tag.pattern_id" class="grammar-desc">
          <strong>「{{ tag.name_ja }}」</strong><span>{{ tag.description }}</span>
        </div>
      </div>

      <details v-if="lookup?.candidates.length" class="tooltip-section candidate-section" :open="lookup.candidates.length <= 8">
        <summary><span class="section-title">表记候选</span><span>{{ lookup.candidates.length }} 项</span></summary>
        <div class="candidate-list">
          <button
            v-for="candidate in lookup.candidates"
            :key="candidate.target"
            type="button"
            :class="{ active: lookup.selected_target === candidate.target }"
            @click="emit('select', candidate.target)"
          >{{ candidateLabel(candidate) }}</button>
        </div>
      </details>

      <div v-if="loading || dictionaryGroups.length || !lookup?.candidates.length" class="tooltip-section definitions" @click="handleDefinitionClick">
        <div class="section-title">词典释义</div>
        <div v-if="loading" class="empty-state">正在载入释义…</div>
        <div v-else-if="!dictionaryGroups.length" class="empty-state">暂无本地词典释义</div>
        <section v-for="group in dictionaryGroups" :key="group.name" class="dictionary-group">
          <h3>{{ group.name }}</h3>
          <details v-for="(entry, entryIndex) in group.entries" :key="entry.entry_key" class="dictionary-entry" :open="entry.is_preferred || (!group.entries.some(item => item.is_preferred) && entryIndex === 0)">
            <summary class="entry-meta">
              <strong><span v-if="entry.is_preferred" class="preferred-mark" title="读音匹配">★</span>{{ entry.headword }}</strong>
              <span>{{ entry.reading || (group.entries.length > 1 ? `词条 ${entryIndex + 1}` : entry.match_type) }}</span>
            </summary>
            <div class="entry-body">
              <DictionaryContent :entry="entry" />
            <div v-if="managedLinkGroups(entry).length" class="managed-relations">
              <details v-for="relationGroup in managedLinkGroups(entry)" :key="relationGroup.relation" class="relation-group" :open="relationGroup.links.length <= 6">
                <summary><span>{{ relationLabel(relationGroup.relation) }}</span><span>{{ relationGroup.links.length }} 项</span></summary>
                <div class="relation-list">
                  <button v-for="link in relationGroup.links" :key="link.target" type="button" :data-relation="link.relation" @click="emit('navigate', link.target)">{{ link.label || link.target }}</button>
                </div>
              </details>
            </div>
            </div>
          </details>
        </section>
      </div>
    </section>
  </Transition>
</template>

<style scoped>
.tooltip-panel { position: fixed; z-index: 1000; width: min(460px, calc(100vw - 24px)); overflow: auto; overscroll-behavior: contain; padding: 14px; background: var(--glass-bg); backdrop-filter: var(--glass-filter); border: 1px solid var(--glass-border); border-radius: var(--radius-md); box-shadow: var(--shadow-md); color: var(--text-primary); font: .88rem/1.55 var(--font-ja); overflow-wrap: anywhere; pointer-events: auto; scrollbar-gutter: stable; }
.tooltip-above { transform: translate(-50%, -100%) translateY(-8px); }
.tooltip-below { transform: translate(-50%, 8px); }
.tooltip-header { position: sticky; top: -14px; z-index: 3; display: flex; gap: 8px; align-items: baseline; padding: 10px 0 8px; background: var(--glass-bg); }
.headword-block { flex: 1; min-width: 0; }
.back-button { flex: 0 0 28px; padding: 0; font-size: 1.35rem; line-height: 26px; }
.base-form { color: var(--accent-color); font-size: 1.25rem; font-weight: 700; }
.reading, .tooltip-pos { color: var(--text-muted); font-size: .78rem; }
.tooltip-section { border-top: 1px solid var(--border-color); padding-top: 10px; margin-top: 6px; }
.section-title { margin-bottom: 7px; color: var(--text-muted); font: 700 .72rem var(--font-ui); letter-spacing: .04em; }
.grammar-desc { display: grid; grid-template-columns: auto 1fr; gap: 8px; }
.grammar-desc strong { color: var(--novelty-high-text); }
.grammar-desc span { color: var(--text-secondary); }
.candidate-section summary { display: flex; justify-content: space-between; cursor: pointer; color: var(--text-muted); font: .75rem var(--font-ui); }
.candidate-section summary .section-title { margin: 0; }
.candidate-list { display: grid; grid-template-columns: repeat(auto-fit, minmax(130px, 1fr)); gap: 6px; max-height: 180px; overflow: auto; margin-top: 8px; padding-right: 3px; }
.relation-list { display: flex; flex-wrap: wrap; gap: 6px; }
button { border: 1px solid var(--border-color); border-radius: 999px; padding: 3px 9px; background: var(--bg-card); color: var(--accent-color); cursor: pointer; }
button:hover, button.active { border-color: var(--accent-color); background: var(--accent-light); }
.dictionary-entry + .dictionary-entry { border-top: 1px dashed var(--border-color); margin-top: 12px; padding-top: 12px; }
.dictionary-group + .dictionary-group { margin-top: 14px; }
.dictionary-group > h3 { position: sticky; top: 45px; z-index: 1; margin: 0 -4px 8px; padding: 4px; background: var(--glass-bg); color: var(--text-muted); font: 700 .72rem var(--font-ui); }
.entry-meta { display: flex; justify-content: space-between; gap: 12px; margin-bottom: 6px; }
.entry-meta { cursor: pointer; }
.entry-body { padding-top: 3px; }
.entry-meta span, .empty-state { color: var(--text-muted); font: .75rem var(--font-ui); }
.entry-meta .preferred-mark { margin-right: 4px; color: var(--accent-color); }
.relation-list { margin-top: 8px; }
.managed-relations { display: grid; gap: 8px; margin-top: 10px; padding-top: 9px; border-top: 1px dotted var(--border-color); }
.relation-group summary { display: flex; justify-content: space-between; margin-bottom: 4px; color: var(--text-muted); font: 700 .7rem var(--font-ui); cursor: pointer; }
.relation-list button[data-relation="antonym"] { color: var(--novelty-high-text); }
.relation-list button[data-relation="parent"], .relation-list button[data-relation="child"] { color: var(--text-secondary); }
.fade-enter-active, .fade-leave-active { transition: opacity .12s ease; }
.fade-enter-from, .fade-leave-to { opacity: 0; }
</style>
