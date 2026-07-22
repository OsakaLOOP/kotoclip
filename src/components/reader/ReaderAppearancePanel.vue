<script setup lang="ts">
import { RotateCcw } from "@lucide/vue";
import { DEFAULT_READER_APPEARANCE, type ReaderAppearance } from "../../reader/reading";
import ReaderSurface from "./ReaderSurface.vue";

defineProps<{ show: boolean; appearance: ReaderAppearance }>();
const emit = defineEmits<{
  close: [];
  update: [value: ReaderAppearance];
}>();

function patch(appearance: ReaderAppearance, key: keyof ReaderAppearance, value: string) {
  emit("update", { ...appearance, [key]: Number(value) });
}
</script>

<template>
  <ReaderSurface :show="show" variant="modal" title="阅读排版" @close="emit('close')">
    <template #actions>
        <button class="reset-button" type="button" title="恢复默认排版" aria-label="恢复默认排版" @click="emit('update', DEFAULT_READER_APPEARANCE)">
          <RotateCcw :size="16" aria-hidden="true" />
        </button>
    </template>
    <div class="appearance-settings">
      <label>
      <span>字号 <output>{{ appearance.fontSize }} px</output></span>
      <input type="range" min="14" max="28" step="1" :value="appearance.fontSize" @input="patch(appearance, 'fontSize', ($event.target as HTMLInputElement).value)" />
      </label>
      <label>
      <span>行距 <output>{{ appearance.lineHeight.toFixed(2) }}</output></span>
      <input type="range" min="1.5" max="2.8" step="0.05" :value="appearance.lineHeight" @input="patch(appearance, 'lineHeight', ($event.target as HTMLInputElement).value)" />
      </label>
      <label>
      <span>段距 <output>{{ appearance.paragraphGap }} px</output></span>
      <input type="range" min="8" max="40" step="2" :value="appearance.paragraphGap" @input="patch(appearance, 'paragraphGap', ($event.target as HTMLInputElement).value)" />
      </label>
      <label>
      <span>版心宽度 <output>{{ appearance.contentWidth }} px</output></span>
      <input type="range" min="520" max="1040" step="20" :value="appearance.contentWidth" @input="patch(appearance, 'contentWidth', ($event.target as HTMLInputElement).value)" />
      </label>
    </div>
  </ReaderSurface>
</template>

<style scoped>
.appearance-settings {
  padding: 4px 18px 20px;
}

label span {
  display: flex;
  align-items: center;
}

.reset-button {
  display: grid;
  width: 29px;
  height: 29px;
  place-items: center;
  border: 0;
  background: transparent;
  color: var(--text-muted);
  cursor: pointer;
}

label {
  display: block;
  margin-top: 16px;
}

label span {
  justify-content: space-between;
  color: var(--text-secondary);
  font-size: 0.78rem;
}

output {
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
}

input {
  width: 100%;
  margin-top: 7px;
  accent-color: var(--accent-color);
}
</style>
