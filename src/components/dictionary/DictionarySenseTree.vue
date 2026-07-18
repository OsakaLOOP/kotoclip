<script setup lang="ts">
import type { DictionaryLink, DictionarySense } from "../../types";

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
          <div v-if="sense.heading || sense.tags.length" class="sense-heading-row">
            <span v-if="sense.heading" class="sense-heading" lang="ja">{{ sense.heading }}</span>
            <span v-for="tag in sense.tags" :key="`${tag.kind}:${tag.label}`" class="dictionary-tag" :data-kind="tag.kind">{{ tag.label }}</span>
          </div>
          <div v-if="sense.glosses.length" class="sense-glosses">
            <span v-for="(gloss, index) in sense.glosses" :key="index" class="sense-gloss" :lang="gloss.lang || undefined">
              <small
                v-if="gloss.qualifier && (index === 0 || sense.glosses[index - 1]?.qualifier !== gloss.qualifier)"
                class="sense-gloss__qualifier"
                lang="ja"
              >{{ gloss.qualifier }}</small>
              <span v-html="gloss.html"></span>
            </span>
          </div>
          <div v-for="(definition, index) in sense.definitions" :key="`definition:${index}`" class="sense-definition" :lang="definition.lang || undefined" v-html="definition.html"></div>
          <div v-if="sense.examples.length" class="sense-examples">
            <blockquote v-for="(example, index) in sense.examples" :key="index" class="example-pair">
              <div class="example-source" :lang="example.source.lang || 'ja'" v-html="example.source.html"></div>
              <div v-if="example.translation" class="example-translation" :lang="example.translation.lang || 'zh-CN'" v-html="example.translation.html"></div>
              <div v-if="example.note" class="example-note" :lang="example.note.lang || undefined" v-html="example.note.html"></div>
            </blockquote>
          </div>
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
