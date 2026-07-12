<script setup lang="ts">
import { onMounted, onUnmounted } from "vue";
import { AnnotatedToken } from "../types";
import type { SegmentationCandidate } from "../types";

const props = defineProps<{
  show: boolean;
  x: number;
  y: number;
  token: AnnotatedToken | null;
  paragraphId: number;
  tokenIndex: number;
  candidates: SegmentationCandidate[];
  candidatesLoading: boolean;
}>();

const emit = defineEmits<{
  (e: "close"): void;
  (e: "mark-known", paragraphId: number, tokenIndex: number): void;
  (e: "mark-unknown", paragraphId: number, tokenIndex: number): void;
  (e: "view-definition", paragraphId: number, tokenIndex: number): void;
  (e: "load-candidates"): void;
  (e: "apply-candidate", candidate: SegmentationCandidate): void;
}>();

function handleMarkKnown() {
  emit("mark-known", props.paragraphId, props.tokenIndex);
  emit("close");
}

function handleMarkUnknown() {
  emit("mark-unknown", props.paragraphId, props.tokenIndex);
  emit("close");
}

function handleViewDefinition() {
  emit("view-definition", props.paragraphId, props.tokenIndex);
  emit("close");
}

// 点击外部关闭菜单
function handleOutsideClick(e: MouseEvent) {
  if (props.show) {
    const el = document.getElementById("context-menu");
    if (el && !el.contains(e.target as Node)) {
      emit("close");
    }
  }
}

onMounted(() => {
  document.addEventListener("mousedown", handleOutsideClick);
});

onUnmounted(() => {
  document.removeEventListener("mousedown", handleOutsideClick);
});
</script>

<template>
  <div
    v-if="show && token"
    id="context-menu"
    class="context-menu"
    :style="{ left: x + 'px', top: y + 'px' }"
  >
    <div class="menu-header">{{ token.bunsetsu.surface }}</div>
    <div class="menu-divider"></div>

    <button
      class="menu-item"
      :disabled="candidatesLoading"
      @click="emit('load-candidates')"
    >
      <span class="icon">候</span>
      {{ candidatesLoading ? '生成中...' : 'Top-N 分词候选' }}
    </button>

    <div v-if="candidates.length > 0" class="candidate-list">
      <button
        v-for="(candidate, index) in candidates"
        :key="index"
        class="candidate-item"
        @click="emit('apply-candidate', candidate)"
      >
        <span>{{ candidate.tokens.map((item) => item.bunsetsu.surface).join('｜') }}</span>
        <small :title="candidate.dictionary_evidence.length ? `词典：${candidate.dictionary_evidence.join('、')}` : '无多字词典证据'">
          {{ index === 0 ? `推荐 · V${candidate.vibrato_rank}` : `V${candidate.vibrato_rank} · Δ${candidate.relative_cost}` }}
        </small>
      </button>
    </div>
    <div class="menu-divider"></div>
    
    <button class="menu-item" @click="handleViewDefinition">
      <span class="icon">📖</span> 查看完整释义
    </button>
    
    <button
      v-if="!token.is_known"
      class="menu-item"
      @click="handleMarkKnown"
    >
      <span class="icon">✓</span> 标记为已知 (脱下胶囊)
    </button>
    
    <button
      v-else
      class="menu-item"
      @click="handleMarkUnknown"
    >
      <span class="icon">✗</span> 重新标为生词
    </button>
  </div>
</template>

<style scoped>
.context-menu {
  position: fixed;
  z-index: 1100;
  width: min(360px, calc(100vw - 24px));
  background: var(--bg-primary);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-md);
  padding: 6px 0;
  color: var(--text-primary);
  font-size: 0.85rem;
}

.menu-header {
  padding: 6px 14px;
  font-weight: 600;
  color: var(--text-secondary);
  font-size: 0.8rem;
  background-color: var(--bg-secondary);
  margin-bottom: 4px;
}

.menu-divider {
  height: 1px;
  background-color: var(--border-color);
  margin: 4px 0;
}

.menu-item {
  width: 100%;
  display: flex;
  align-items: center;
  padding: 8px 14px;
  background: transparent;
  border: none;
  text-align: left;
  cursor: pointer;
  color: var(--text-primary);
  transition: background-color 0.2s ease;
  box-shadow: none;
}

.menu-item:hover {
  background-color: var(--accent-light);
  color: var(--accent-color);
}

.menu-item:disabled {
  cursor: wait;
  opacity: 0.6;
}

.candidate-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 4px 8px;
}

.candidate-item {
  width: 100%;
  overflow: hidden;
  padding: 6px 8px;
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  background: var(--bg-secondary);
  color: var(--text-secondary);
  text-align: left;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  cursor: pointer;
}

.candidate-item span { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.candidate-item small { flex: 0 0 auto; font-variant-numeric: tabular-nums; color: var(--text-secondary); }

.candidate-item:hover {
  border-color: var(--accent-color);
  color: var(--accent-color);
}

.menu-item .icon {
  margin-right: 8px;
  font-size: 1rem;
  display: inline-block;
  width: 16px;
  text-align: center;
}
</style>
