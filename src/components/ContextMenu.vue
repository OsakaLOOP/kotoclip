<script setup lang="ts">
import { onMounted, onUnmounted } from "vue";
import { AnnotatedToken } from "../types";

const props = defineProps<{
  show: boolean;
  x: number;
  y: number;
  token: AnnotatedToken | null;
  paragraphId: number;
  tokenIndex: number;
}>();

const emit = defineEmits<{
  (e: "close"): void;
  (e: "mark-known", paragraphId: number, tokenIndex: number): void;
  (e: "mark-unknown", paragraphId: number, tokenIndex: number): void;
  (e: "view-definition", paragraphId: number, tokenIndex: number): void;
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
  width: 180px;
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

.menu-item .icon {
  margin-right: 8px;
  font-size: 1rem;
  display: inline-block;
  width: 16px;
  text-align: center;
}
</style>
