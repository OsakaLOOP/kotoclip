<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { ChevronDown, ChevronLeft, ChevronRight, ChevronUp } from "@lucide/vue";
import type { DictionaryExample } from "../../types";

const props = defineProps<{
  examples: DictionaryExample[];
}>();

const pageSize = 2;
const expanded = ref(false);
const page = ref(0);
const direction = ref<"next" | "previous">("next");
const totalPages = computed(() => Math.max(1, Math.ceil(props.examples.length / pageSize)));
const visibleExamples = computed(() => {
  if (expanded.value) return props.examples;
  const start = page.value * pageSize;
  return props.examples.slice(start, start + pageSize);
});
const transitionName = computed(() => {
  if (expanded.value) return "example-expand";
  return direction.value === "next" ? "example-page-next" : "example-page-previous";
});

watch(() => props.examples.length, () => {
  page.value = Math.min(page.value, totalPages.value - 1);
  if (props.examples.length <= pageSize) expanded.value = false;
});

function showPrevious() {
  if (page.value <= 0) return;
  direction.value = "previous";
  page.value -= 1;
}

function showNext() {
  if (page.value >= totalPages.value - 1) return;
  direction.value = "next";
  page.value += 1;
}

function toggleExpanded() {
  expanded.value = !expanded.value;
}
</script>

<template>
  <section class="example-browser" :class="{ 'is-expanded': expanded }" :aria-label="`例句，共 ${examples.length} 条`">
    <Transition :name="transitionName" mode="out-in">
      <div :key="expanded ? 'expanded' : `page-${page}`" class="example-browser__page">
        <blockquote v-for="(example, index) in visibleExamples" :key="`${expanded ? index : page * pageSize + index}:${example.source.html}`" class="example-pair">
          <div class="example-source" :lang="example.source.lang || 'ja'" v-html="example.source.html"></div>
          <div v-if="example.translation" class="example-translation" :lang="example.translation.lang || 'zh-CN'" v-html="example.translation.html"></div>
          <div v-if="example.note" class="example-note" :lang="example.note.lang || undefined" v-html="example.note.html"></div>
        </blockquote>
      </div>
    </Transition>

    <footer v-if="examples.length > pageSize" class="example-browser__controls">
      <div v-if="!expanded" class="example-browser__pager">
        <button type="button" :disabled="page === 0" aria-label="上一页例句" @click="showPrevious">
          <ChevronLeft :size="15" aria-hidden="true" />
        </button>
        <span class="example-browser__counter" aria-live="polite">{{ page + 1 }}/{{ totalPages }}</span>
        <button type="button" :disabled="page === totalPages - 1" aria-label="下一页例句" @click="showNext">
          <ChevronRight :size="15" aria-hidden="true" />
        </button>
      </div>
      <span v-else class="example-browser__total">共 {{ examples.length }} 条</span>
      <button type="button" class="example-browser__toggle" :aria-expanded="expanded" @click="toggleExpanded">
        <ChevronUp v-if="expanded" :size="14" aria-hidden="true" />
        <ChevronDown v-else :size="14" aria-hidden="true" />
        {{ expanded ? "折叠" : "展开" }}
      </button>
    </footer>
  </section>
</template>
