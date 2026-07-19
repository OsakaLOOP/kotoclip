<script setup lang="ts">
import { DictionaryChoiceOption } from "../../types";
import { Star } from "@lucide/vue";

defineProps<{
  label: string;
  options: DictionaryChoiceOption[];
  shortcutKeys?: string[];
}>();

const emit = defineEmits<{ select: [key: string] }>();
</script>

<template>
  <section v-if="options.length" class="dictionary-choice-bar">
    <div class="dictionary-choice-label">
      <span>{{ label }}</span>
      <span v-if="shortcutKeys?.length" class="shortcut-keys" aria-label="快捷键">
        <kbd v-for="key in shortcutKeys" :key="key">{{ key }}</kbd>
      </span>
    </div>
    <div class="dictionary-choice-viewport no-scrollbar">
      <div class="dictionary-choice-options" :class="{ 'is-dense': options.length > 8 }">
        <button
          v-for="option in options"
          :key="option.key"
          type="button"
          :class="{ active: option.active, unavailable: option.unavailable }"
          :aria-pressed="option.active"
          :aria-description="option.unavailable ? '当前选择下无词典释义' : undefined"
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
.dictionary-choice-label { display: flex; align-items: center; gap: 5px; padding-top: 5px; color: var(--text-muted); font: 700 .7rem var(--font-ui); letter-spacing: .04em; }
.shortcut-keys { display: inline-flex; align-items: center; gap: 2px; }
kbd { min-width: 18px; padding: 1px 4px 2px; border: 1px solid color-mix(in srgb, var(--border-color) 88%, var(--text-muted)); border-bottom-width: 2px; border-radius: 4px; background: color-mix(in srgb, var(--bg-card) 92%, transparent); color: var(--text-secondary); font: 700 .62rem/1.1 var(--font-ui); text-align: center; box-shadow: 0 1px 0 color-mix(in srgb, var(--border-color) 55%, transparent); letter-spacing: 0; }
.dictionary-choice-viewport { overflow-x: auto; overscroll-behavior-x: contain; padding-bottom: 2px; }
.dictionary-choice-options { display: flex; gap: 6px; width: max-content; min-width: 100%; }
.dictionary-choice-options.is-dense { display: grid; grid-auto-flow: column; grid-template-rows: repeat(2, auto); justify-content: start; }
button { display: inline-flex; align-items: center; justify-content: center; gap: 3px; min-height: 29px; max-width: 190px; border: 1px solid var(--border-color); border-radius: 999px; padding: 3px 10px; background: color-mix(in srgb, var(--bg-card) 88%, transparent); color: var(--accent-color); white-space: nowrap; cursor: pointer; }
button > span { min-width: 0; overflow: hidden; text-overflow: ellipsis; }
button:hover, button.active { border-color: var(--accent-color); background: var(--accent-light); }
button.active { box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent-color) 32%, transparent); }
button.unavailable { border-color: color-mix(in srgb, var(--border-color) 72%, transparent); background: color-mix(in srgb, var(--bg-card) 62%, transparent); color: var(--text-muted); opacity: .52; }
button.unavailable:hover, button.unavailable.active { border-color: color-mix(in srgb, var(--accent-color) 58%, var(--border-color)); background: color-mix(in srgb, var(--accent-light) 48%, var(--bg-card)); opacity: .76; }
.choice-star { color: var(--accent-color); font-size: .72em; }
</style>
