<script setup lang="ts">
import { computed } from "vue";
import { AnnotatedToken, DictionaryLookup } from "../types";

const props = defineProps<{
  show: boolean;
  x: number;
  y: number;
  placement: "above" | "below";
  token: AnnotatedToken | null;
  lookup: DictionaryLookup | null;
  loading: boolean;
}>();

const emit = defineEmits<{
  enter: [];
  leave: [];
  navigate: [target: string];
  select: [target: string];
}>();

const formattedPos = computed(() => {
  if (!props.token) return "";
  const head = props.token.bunsetsu.head_word;
  return [head.pos.major, head.pos.sub1].filter((part) => part && part !== "*").join(" · ");
});

const dictionaryGroups = computed(() => {
  const groups = new Map<string, NonNullable<typeof props.lookup>["entries"]>();
  for (const entry of props.lookup?.entries ?? []) {
    const group = groups.get(entry.dict_name) ?? [];
    group.push(entry);
    groups.set(entry.dict_name, group);
  }
  return [...groups.entries()].map(([name, entries]) => ({ name, entries }));
});

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
      :style="{ left: x + 'px', top: y + 'px' }"
      role="dialog"
      aria-label="词典释义"
      @mouseenter="emit('enter')"
      @mouseleave="emit('leave')"
      @wheel.stop
    >
      <header class="tooltip-header">
        <div>
          <span class="base-form">{{ token.bunsetsu.head_word.base_form }}</span>
          <span class="reading">【{{ token.bunsetsu.head_word.reading || '无读音' }}】</span>
        </div>
        <span class="tooltip-pos">{{ formattedPos }}</span>
      </header>

      <div v-if="token.bunsetsu.grammar_tags.length" class="tooltip-section grammar-list">
        <div v-for="tag in token.bunsetsu.grammar_tags" :key="tag.pattern_id" class="grammar-desc">
          <strong>「{{ tag.name_ja }}」</strong><span>{{ tag.description }}</span>
        </div>
      </div>

      <div v-if="lookup?.candidates.length" class="tooltip-section">
        <div class="section-title">表记候选</div>
        <div class="candidate-list">
          <button
            v-for="candidate in lookup.candidates"
            :key="candidate.target"
            type="button"
            :class="{ active: lookup.selected_target === candidate.target }"
            @click="emit('select', candidate.target)"
          >{{ candidate.label || candidate.target }}</button>
        </div>
      </div>

      <div class="tooltip-section definitions" @click="handleDefinitionClick">
        <div class="section-title">词典释义</div>
        <div v-if="loading" class="empty-state">正在载入释义…</div>
        <div v-else-if="!lookup?.entries.length" class="empty-state">暂无本地词典释义</div>
        <section v-for="group in dictionaryGroups" :key="group.name" class="dictionary-group">
          <h3>{{ group.name }}</h3>
          <article v-for="entry in group.entries" :key="entry.entry_key" class="dictionary-entry">
            <div class="entry-meta"><strong>{{ entry.headword }}</strong><span>{{ entry.match_type }}</span></div>
            <div class="dictionary-html" v-html="entry.definition_html"></div>
            <div v-if="entry.links.some(link => link.relation !== 'related')" class="relation-list">
              <button v-for="link in entry.links.filter(link => link.relation !== 'related')" :key="`${link.relation}:${link.target}`" type="button" @click="emit('navigate', link.target)">{{ link.relation === 'antonym' ? '反义' : '近义' }} · {{ link.label }}</button>
            </div>
          </article>
        </section>
      </div>
    </section>
  </Transition>
</template>

<style scoped>
.tooltip-panel { position: fixed; z-index: 1000; width: min(460px, calc(100vw - 24px)); max-height: min(70vh, 620px); overflow: auto; overscroll-behavior: contain; padding: 14px; background: var(--glass-bg); backdrop-filter: var(--glass-filter); border: 1px solid var(--glass-border); border-radius: var(--radius-md); box-shadow: var(--shadow-md); color: var(--text-primary); font: .88rem/1.55 var(--font-ja); overflow-wrap: anywhere; pointer-events: auto; scrollbar-gutter: stable; }
.tooltip-above { transform: translate(-50%, -100%) translateY(-8px); }
.tooltip-below { transform: translate(-50%, 8px); }
.tooltip-header { position: sticky; top: -14px; z-index: 2; display: flex; justify-content: space-between; gap: 12px; align-items: baseline; padding: 10px 0 8px; background: var(--glass-bg); }
.base-form { color: var(--accent-color); font-size: 1.25rem; font-weight: 700; }
.reading, .tooltip-pos { color: var(--text-muted); font-size: .78rem; }
.tooltip-section { border-top: 1px solid var(--border-color); padding-top: 10px; margin-top: 6px; }
.section-title { margin-bottom: 7px; color: var(--text-muted); font: 700 .72rem var(--font-ui); letter-spacing: .04em; }
.grammar-desc { display: grid; grid-template-columns: auto 1fr; gap: 8px; }
.grammar-desc strong { color: var(--novelty-high-text); }
.grammar-desc span { color: var(--text-secondary); }
.candidate-list, .relation-list { display: flex; flex-wrap: wrap; gap: 6px; }
button { border: 1px solid var(--border-color); border-radius: 999px; padding: 3px 9px; background: var(--bg-card); color: var(--accent-color); cursor: pointer; }
button:hover, button.active { border-color: var(--accent-color); background: var(--accent-light); }
.dictionary-entry + .dictionary-entry { border-top: 1px dashed var(--border-color); margin-top: 12px; padding-top: 12px; }
.dictionary-group + .dictionary-group { margin-top: 14px; }
.dictionary-group > h3 { position: sticky; top: 45px; z-index: 1; margin: 0 -4px 8px; padding: 4px; background: var(--glass-bg); color: var(--text-muted); font: 700 .72rem var(--font-ui); }
.entry-meta { display: flex; justify-content: space-between; gap: 12px; margin-bottom: 6px; }
.entry-meta span, .empty-state { color: var(--text-muted); font: .75rem var(--font-ui); }
.dictionary-html { color: var(--text-secondary); }
.dictionary-html :deep(a) { color: var(--accent-color); text-decoration: underline; cursor: pointer; }
.dictionary-html :deep(*) { max-width: 100%; white-space: normal !important; }
.dictionary-html :deep(.bss) { color: var(--text-primary); font-size: 1.03em; font-weight: 700; letter-spacing: .03em; }
.dictionary-html :deep(.annot) { color: var(--text-muted); font-size: .78em; }
.dictionary-html :deep(.leftnull), .dictionary-html :deep(.lefta) { display: grid; gap: 4px; }
.dictionary-html :deep(.no) { float: left; min-width: 1.8em; color: var(--accent-color); font-weight: 700; }
.dictionary-html :deep(hy) { color: var(--text-primary); font-weight: 600; }
.dictionary-html :deep(table) { width: 100%; border-collapse: collapse; }
.dictionary-html :deep(th), .dictionary-html :deep(td) { border: 1px solid var(--border-color); padding: 4px 6px; vertical-align: top; }
.dictionary-html :deep(ul), .dictionary-html :deep(ol) { padding-inline-start: 1.4em; }
.relation-list { margin-top: 8px; }
.fade-enter-active, .fade-leave-active { transition: opacity .12s ease; }
.fade-enter-from, .fade-leave-to { opacity: 0; }
</style>
