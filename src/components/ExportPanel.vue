<script setup lang="ts">
import { computed } from "vue";
import { Backpack, Download, Eraser, Trash2 } from "@lucide/vue";
import ReaderSurface from "./reader/ReaderSurface.vue";
import type { SavedSelection } from "../types/reader";

const props = defineProps<{
  show: boolean;
  selections: SavedSelection[];
}>();

const emit = defineEmits<{
  (e: "close"): void;
  (e: "remove", selection: SavedSelection): void;
  (e: "clear-all"): void;
  (e: "export"): void;
  (e: "update-note", selection: SavedSelection, note: string): void;
}>();

const count = computed(() => props.selections.length);
</script>

<template>
  <ReaderSurface :show="show" variant="side" title="待导出词汇" @close="emit('close')">
      <template #actions>
        <span class="badge">{{ count }}</span>
      </template>

      <div class="panel-body no-scrollbar">
        <div v-if="count === 0" class="empty-state">
          <Backpack class="empty-icon" :size="42" stroke-width="1.5" aria-hidden="true" />
          <p>阅读时点击胶囊</p>
          <p>在此收集生词</p>
        </div>

        <div v-else class="token-list">
          <div
            v-for="item in selections"
            :key="`${item.book_id}:${item.text_version}:${item.start}:${item.end}`"
            class="token-card"
          >
            <div class="card-content">
              <div class="card-word">{{ item.base_form || item.surface }}</div>
              <div v-if="item.reading" class="card-reading">【{{ item.reading }}】</div>
              <div class="card-surface">正文：{{ item.surface }}</div>
              <textarea class="card-note" :value="item.note" @input="emit('update-note', item, ($event.target as HTMLTextAreaElement).value)" />
            </div>
            <button
              class="card-remove"
              title="移出导出列表"
              @click="emit('remove', item)"
            >
              <Trash2 :size="16" aria-hidden="true" />
            </button>
          </div>
        </div>
      </div>

      <div v-if="count > 0" class="panel-footer">
        <button class="clear-btn" @click="emit('clear-all')"><Eraser :size="15" aria-hidden="true" /> 清空选择</button>
        <button
          class="export-btn"
          @click="$emit('export')"
        >
          <Download :size="15" aria-hidden="true" /> 导出 JSON
        </button>
      </div>
  </ReaderSurface>
</template>

<style scoped>
.badge {
  font-size: 0.75rem;
  background-color: var(--accent-color);
  color: white;
  padding: 2px 8px;
  border-radius: 10px;
}

.panel-body {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 200px;
  color: var(--text-muted);
  font-size: 0.9rem;
}

.empty-icon {
  display: block;
  margin-bottom: 12px;
  color: var(--accent-color);
  opacity: 0.5;
}

.token-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.token-card {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px;
  background-color: var(--bg-primary);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-md);
  transition: all 0.2s ease;
}

.token-card:hover {
  border-color: var(--accent-color);
  transform: translateY(-1px);
  box-shadow: var(--shadow-sm);
}

.card-content {
  flex: 1;
}

.card-word {
  font-weight: bold;
  color: var(--text-primary);
  font-size: 1.05rem;
}

.card-reading {
  font-size: 0.75rem;
  color: var(--text-secondary);
  margin-top: 2px;
}

.card-surface {
  font-size: 0.7rem;
  color: var(--text-muted);
  margin-top: 4px;
}

.card-remove {
  background: transparent;
  border: none;
  color: var(--text-muted);
  font-size: 1.25rem;
  cursor: pointer;
  padding: 4px;
  box-shadow: none;
}

.card-remove:hover {
  color: var(--novelty-high-text);
}

.panel-footer {
  padding: 16px;
  background: var(--bg-primary);
  border-top: 1px solid var(--border-color);
  display: flex;
  gap: 10px;
}

.clear-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  flex: 1;
  background-color: transparent;
  color: var(--text-secondary);
  border: 1px solid var(--border-color);
  padding: 10px;
  border-radius: var(--radius-sm);
  cursor: pointer;
}

.clear-btn:hover {
  background-color: var(--bg-secondary);
}

.export-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  flex: 2;
  background-color: var(--accent-color);
  color: white;
  border: none;
  padding: 10px;
  border-radius: var(--radius-sm);
  cursor: pointer;
  font-weight: bold;
}

.export-btn:hover {
  background-color: var(--accent-hover);
}

.card-note {
  width: 100%;
  margin-top: 6px;
  padding: 6px 8px;
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  background: var(--bg-secondary);
  color: var(--text-primary);
  font-family: var(--font-ja);
  font-size: 0.8rem;
  resize: vertical;
  min-height: 32px;
  max-height: 80px;
  outline: none;
}
.card-note:focus {
  border-color: var(--accent-color);
}
</style>
