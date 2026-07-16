<script setup lang="ts">
import { DictionaryChoiceOption } from "../../types";
import { Star } from "@lucide/vue";

defineProps<{
  label: string;
  options: DictionaryChoiceOption[];
}>();

const emit = defineEmits<{ select: [key: string] }>();
</script>

<template>
  <section v-if="options.length" class="dictionary-choice-bar">
    <div class="dictionary-choice-label">{{ label }}</div>
    <div class="dictionary-choice-viewport no-scrollbar">
      <div class="dictionary-choice-options" :class="{ 'is-dense': options.length > 8 }">
        <button
          v-for="option in options"
          :key="option.key"
          type="button"
          :class="{ active: option.active }"
          :aria-pressed="option.active"
          :title="option.title"
          @click="emit('select', option.key)"
        >
          <Star v-if="option.preferred" class="choice-star" :size="12" fill="currentColor" aria-label="最佳匹配" />
          <span>{{ option.label }}</span>
        </button>
      </div>
    </div>
  </section>
</template>

<style scoped>
.dictionary-choice-bar { display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 9px; align-items: start; border-top: 1px solid var(--border-color); padding-top: 9px; margin-top: 7px; }
.dictionary-choice-label { padding-top: 5px; color: var(--text-muted); font: 700 .7rem var(--font-ui); letter-spacing: .04em; }
.dictionary-choice-viewport { overflow-x: auto; overscroll-behavior-x: contain; padding-bottom: 2px; }
.dictionary-choice-options { display: flex; gap: 6px; width: max-content; min-width: 100%; }
.dictionary-choice-options.is-dense { display: grid; grid-auto-flow: column; grid-template-rows: repeat(2, auto); justify-content: start; }
button { display: inline-flex; align-items: center; justify-content: center; gap: 3px; min-height: 29px; max-width: 190px; border: 1px solid var(--border-color); border-radius: 999px; padding: 3px 10px; background: color-mix(in srgb, var(--bg-card) 88%, transparent); color: var(--accent-color); white-space: nowrap; cursor: pointer; }
button:hover, button.active { border-color: var(--accent-color); background: var(--accent-light); }
button.active { box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent-color) 32%, transparent); }
.choice-star { color: var(--accent-color); font-size: .72em; }
</style>
