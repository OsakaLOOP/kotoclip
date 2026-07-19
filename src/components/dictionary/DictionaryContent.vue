<script setup lang="ts">
import type { DictEntry, DictionaryLink } from "../../types";
import DictionarySenseTree from "./DictionarySenseTree.vue";
import DictionaryExamples from "./DictionaryExamples.vue";
import "../../styles/dictionaries/generic.css";
import "../../styles/dictionaries/daijirin.css";
import "../../styles/dictionaries/shogakukan.css";
import "../../styles/dictionaries/crown.css";

defineProps<{ entry: DictEntry }>();

const emit = defineEmits<{
  navigate: [target: string];
}>();

function relationKey(link: DictionaryLink) {
  return `${link.relation}:${link.target}`;
}
</script>

<template>
  <div class="dictionary-content" :class="`dictionary-content--${entry.style_profile}`">
    <template v-if="entry.senses.length || entry.sections.length">
      <DictionarySenseTree v-if="entry.senses.length" :senses="entry.senses" @navigate="emit('navigate', $event)" />
      <section v-for="(section, sectionIndex) in entry.sections" :key="`${section.kind}:${sectionIndex}`" class="dictionary-section" :data-kind="section.kind">
        <h4 v-if="section.label" class="dictionary-section__title">{{ section.label }}</h4>
        <article v-for="(item, itemIndex) in section.items" :key="itemIndex" class="dictionary-section__item">
          <header v-if="item.label || item.reading || item.tags.length" class="dictionary-section__header">
            <strong v-if="item.label_html" lang="ja" v-html="item.label_html"></strong>
            <strong v-else-if="item.label" lang="ja">{{ item.label }}</strong>
            <span v-if="item.reading" class="dictionary-section__reading" lang="ja">【{{ item.reading }}】</span>
            <span v-for="tag in item.tags" :key="`${tag.kind}:${tag.label}`" class="dictionary-tag" :data-kind="tag.kind">{{ tag.label }}</span>
          </header>
          <div v-for="(content, contentIndex) in item.content" :key="`content:${contentIndex}`" class="dictionary-section__content" :lang="content.lang || undefined" v-html="content.html"></div>
          <DictionarySenseTree v-if="item.senses.length" :senses="item.senses" @navigate="emit('navigate', $event)" />
          <DictionaryExamples v-if="item.examples.length" :examples="item.examples" />
          <div v-if="item.relations.length" class="sense-relations">
            <button v-for="link in item.relations" :key="relationKey(link)" type="button" :data-relation="link.relation" @click="emit('navigate', link.target)">{{ link.label || link.target }}</button>
          </div>
        </article>
      </section>
    </template>
    <template v-else>
      <section v-for="(block, index) in entry.content_blocks" :key="`${block.kind}:${index}`" :class="['dictionary-module', `dictionary-module--${block.kind}`]">
        <h4 v-if="block.label" class="dictionary-module__label">{{ block.label }}</h4>
        <div v-if="block.html" class="dictionary-module__body" v-html="block.html"></div>
      </section>
    </template>
  </div>
</template>
