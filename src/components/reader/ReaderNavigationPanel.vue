<script setup lang="ts">
import { X } from "@lucide/vue";
import type { ReaderChapter } from "../../reader/document";

defineProps<{ show: boolean; chapters: ReaderChapter[]; currentId?: string }>();
const emit = defineEmits<{ close: []; navigate: [chapter: ReaderChapter] }>();
</script>

<template>
  <aside v-if="show" class="navigation-panel" aria-label="章节目录">
    <header>
      <strong>章节</strong>
      <button type="button" title="关闭章节目录" @click="emit('close')"><X :size="18" aria-hidden="true" /></button>
    </header>
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
  </aside>
</template>

<style scoped>
.navigation-panel {
  position: absolute;
  z-index: 70;
  top: 0;
  bottom: 0;
  left: 0;
  width: min(340px, 88vw);
  border-right: 1px solid var(--border-color);
  background: var(--bg-primary);
  box-shadow: var(--shadow-md);
}

header {
  display: flex;
  height: 54px;
  align-items: center;
  justify-content: space-between;
  padding: 0 14px 0 18px;
  border-bottom: 1px solid var(--border-color);
}

header strong {
  font-size: 0.9rem;
}

header button {
  display: grid;
  width: 30px;
  height: 30px;
  place-items: center;
  border: 0;
  background: transparent;
  color: var(--text-muted);
  cursor: pointer;
}

nav {
  height: calc(100% - 54px);
  overflow-y: auto;
  padding: 8px;
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
