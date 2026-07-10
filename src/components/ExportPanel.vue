<script setup lang="ts">
import { ref, computed } from "vue";
import { Paragraph } from "../composables/useTokenization";

const props = defineProps<{
  show: boolean;
  selectedKeys: { paragraphId: number; tokenIndex: number }[];
  paragraphs: Paragraph[];
}>();

const emit = defineEmits<{
  (e: "close"): void;
  (e: "remove-key", paragraphId: number, tokenIndex: number): void;
  (e: "clear-all"): void;
  (e: "export"): void;
}>();

const isExporting = ref(false);

// 将选中的 Keys 映射为可展示的词条数据
const selectedTokens = computed(() => {
  const list: {
    paragraphId: number;
    tokenIndex: number;
    surface: string;
    baseForm: string;
    reading: string;
  }[] = [];

  for (const key of props.selectedKeys) {
    const p = props.paragraphs.find((para) => para.id === key.paragraphId);
    if (!p) continue;
    const token = p.tokens[key.tokenIndex];
    if (!token) continue;
    list.push({
      paragraphId: key.paragraphId,
      tokenIndex: key.tokenIndex,
      surface: token.bunsetsu.surface,
      baseForm: token.bunsetsu.head_word.base_form,
      reading: token.bunsetsu.head_word.reading,
    });
  }
  return list;
});

// 计算选中的词汇数量
const count = computed(() => selectedTokens.value.length);
</script>

<template>
  <Transition name="slide">
    <div v-if="show" class="export-panel">
      <div class="panel-header">
        <div class="header-title">
          <span>待导出词汇</span>
          <span class="badge">{{ count }}</span>
        </div>
        <button class="close-btn" @click="emit('close')">×</button>
      </div>

      <div class="panel-body no-scrollbar">
        <div v-if="count === 0" class="empty-state">
          <span class="empty-icon">🎒</span>
          <p>阅读时点击胶囊</p>
          <p>在此收集生词</p>
        </div>

        <div v-else class="token-list">
          <div
            v-for="item in selectedTokens"
            :key="`${item.paragraphId}-${item.tokenIndex}`"
            class="token-card"
          >
            <div class="card-content">
              <div class="card-word">{{ item.baseForm }}</div>
              <div class="card-reading">【{{ item.reading }}】</div>
              <div class="card-surface">来自: {{ item.surface }}</div>
            </div>
            <button
              class="card-remove"
              title="移出导出列表"
              @click="emit('remove-key', item.paragraphId, item.tokenIndex)"
            >
              ×
            </button>
          </div>
        </div>
      </div>

      <div v-if="count > 0" class="panel-footer">
        <button class="clear-btn" @click="emit('clear-all')">清空选择</button>
        <button
          class="export-btn"
          :disabled="isExporting"
          @click="$emit('export')"
        >
          {{ isExporting ? '处理中...' : '生成 Anki 导出' }}
        </button>
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.export-panel {
  position: fixed;
  right: 0;
  top: 0;
  bottom: 0;
  width: 320px;
  background: var(--bg-secondary);
  border-left: 1px solid var(--border-color);
  box-shadow: var(--shadow-md);
  z-index: 1000;
  display: flex;
  flex-direction: column;
}

.panel-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 16px;
  border-bottom: 1px solid var(--border-color);
  background: var(--bg-primary);
}

.header-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-weight: bold;
}

.badge {
  font-size: 0.75rem;
  background-color: var(--accent-color);
  color: white;
  padding: 2px 8px;
  border-radius: 10px;
}

.close-btn {
  background: transparent;
  border: none;
  font-size: 1.5rem;
  cursor: pointer;
  color: var(--text-secondary);
  box-shadow: none;
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
  font-size: 3rem;
  margin-bottom: 12px;
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

/* 滑动动画 */
.slide-enter-active,
.slide-leave-active {
  transition: transform 0.3s cubic-bezier(0.16, 1, 0.3, 1);
}

.slide-enter-from,
.slide-leave-to {
  transform: translateX(100%);
}
</style>
