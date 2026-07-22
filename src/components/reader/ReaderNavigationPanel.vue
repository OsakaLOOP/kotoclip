<script setup lang="ts">
import type { ReaderChapter } from "../../reader/document";
import ReaderSurface from "./ReaderSurface.vue";

defineProps<{ show: boolean; chapters: ReaderChapter[]; currentId?: string }>();
const emit = defineEmits<{ close: []; navigate: [chapter: ReaderChapter] }>();
</script>

<template>
  <ReaderSurface :show="show" variant="side" side="left" title="章节" label="章节目录" @close="emit('close')">
    <nav>
      <button
        v-for="(chapter, index) in chapters"
        :key="chapter.id"
        type="button"
        :class="{ active: chapter.id === currentId }"
        :style="{ paddingLeft: `${14 + Math.max(0, chapter.level - 1) * 10}px` }"
        @click="emit('navigate', chapter)"
      >
        <span>{{ String(index + 1).padStart(2, '0') }}</span>
        <strong>{{ chapter.title }}</strong>
      </button>
    </nav>
  </ReaderSurface>
</template>

<style scoped>
nav {
  min-height: 0;
  padding: 8px;
  overflow-y: auto;
}

nav button {
  display: grid;
  width: 100%;
  grid-template-columns: 28px minmax(0, 1fr);
  gap: 7px;
  align-items: baseline;
  padding-top: 9px;
  padding-right: 10px;
  padding-bottom: 9px;
  border: 0;
  border-radius: 4px;
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  text-align: left;
}

nav button:hover,
nav button.active {
  background: var(--accent-light);
  color: var(--accent-color);
}

nav span {
  color: var(--text-muted);
  font-size: 0.68rem;
  font-variant-numeric: tabular-nums;
}

nav strong {
  overflow-wrap: anywhere;
  font-size: 0.8rem;
  font-weight: 500;
}
</style>
