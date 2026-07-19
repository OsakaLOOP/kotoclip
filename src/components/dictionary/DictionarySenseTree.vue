<script setup lang="ts">
import type { DictionaryLink, DictionarySense } from "../../types";
import DictionaryExamples from "./DictionaryExamples.vue";

withDefaults(defineProps<{
  senses: DictionarySense[];
  depth?: number;
}>(), { depth: 0 });

const emit = defineEmits<{
  navigate: [target: string];
}>();

function relationLabel(relation: string) {
  return ({ antonym: "反义", synonym: "近义", parent: "亲项", child: "子项", phrase: "惯用", reference: "参照", internal_reference: "同义项", related: "关联", redirect: "转至" } as Record<string, string>)[relation] ?? "关联";
}

function relationKey(link: DictionaryLink) {
  return `${link.relation}:${link.target}`;
}
</script>

<template>
  <ol class="sense-tree" :class="`sense-tree--depth-${Math.min(depth, 2)}`">
    <li v-for="sense in senses" :key="sense.sense_id" class="sense-node">
      <div class="sense-main" :class="{ 'sense-main--unmarked': !sense.marker }">
        <span v-if="sense.marker" class="sense-marker">{{ sense.marker }}</span>
        <div class="sense-content">
          <div v-if="sense.gloss_groups.length" class="sense-gloss-groups">
            <div v-for="(group, groupIndex) in sense.gloss_groups" :key="groupIndex" class="sense-gloss-group">
              <span v-if="group.heading" class="sense-heading" lang="ja">{{ group.heading }}</span>
              <span class="sense-gloss-clauses">
                <template v-for="(clause, clauseIndex) in group.clauses" :key="clauseIndex">
                  <span v-if="clause.separator" class="sense-gloss-separator">{{ clause.separator }}</span>
                  <span class="sense-gloss-clause" :lang="clause.text.lang || undefined">
                    <span v-if="clause.qualifier" class="sense-gloss__qualifier" lang="ja">{{ clause.qualifier }}</span>
                    <span v-for="tag in clause.leading_tags" :key="`leading:${tag.kind}:${tag.label}`" class="dictionary-tag" :data-kind="tag.kind">{{ tag.label }}</span>
                    <span v-if="clause.text.html" v-html="clause.text.html"></span>
                    <span v-for="tag in clause.trailing_tags" :key="`trailing:${tag.kind}:${tag.label}`" class="dictionary-tag" :data-kind="tag.kind">{{ tag.label }}</span>
                  </span>
                </template>
              </span>
            </div>
          </div>
          <div v-else-if="sense.heading || sense.tags.length || sense.glosses.length" class="sense-gloss-group">
            <span v-if="sense.heading" class="sense-heading" lang="ja">{{ sense.heading }}</span>
            <span v-for="tag in sense.tags" :key="`${tag.kind}:${tag.label}`" class="dictionary-tag" :data-kind="tag.kind">{{ tag.label }}</span>
            <span class="sense-gloss-clauses">
              <template v-for="(gloss, index) in sense.glosses" :key="index">
                <span v-if="index" class="sense-gloss-separator">，</span>
                <span class="sense-gloss-clause" :lang="gloss.lang || undefined">
                  <span
                    v-if="gloss.qualifier && (index === 0 || sense.glosses[index - 1]?.qualifier !== gloss.qualifier)"
                    class="sense-gloss__qualifier"
                    lang="ja"
                  >{{ gloss.qualifier }}</span>
                  <span v-html="gloss.html"></span>
                </span>
              </template>
            </span>
          </div>
          <div v-for="(definition, index) in sense.definitions" :key="`definition:${index}`" class="sense-definition" :lang="definition.lang || undefined" v-html="definition.html"></div>
          <DictionaryExamples v-if="sense.examples.length" :examples="sense.examples" />
          <div v-for="(note, index) in sense.notes" :key="`note:${index}`" class="sense-note" :lang="note.lang || undefined" v-html="note.html"></div>
          <div v-if="sense.relations.length" class="sense-relations">
            <template v-for="link in sense.relations" :key="relationKey(link)">
              <span v-if="link.relation === 'internal_reference'" class="sense-relation-static" :data-relation="link.relation">
                <span>{{ relationLabel(link.relation) }}</span>{{ link.label || link.target }}
              </span>
              <button v-else type="button" :data-relation="link.relation" @click="emit('navigate', link.target)">
                <span>{{ relationLabel(link.relation) }}</span>{{ link.label || link.target }}
              </button>
            </template>
          </div>
        </div>
      </div>
      <DictionarySenseTree v-if="sense.children.length" :senses="sense.children" :depth="depth + 1" @navigate="emit('navigate', $event)" />
    </li>
  </ol>
</template>
