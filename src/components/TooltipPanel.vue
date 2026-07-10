<script setup lang="ts">
import { computed } from "vue";
import { AnnotatedToken } from "../types";

const props = defineProps<{
  show: boolean;
  x: number;
  y: number;
  placement: "above" | "below";
  token: AnnotatedToken | null;
  definitionPreview: string | null;
}>();

// 拼装读音和词性标签
const formattedPos = computed(() => {
  if (!props.token) return "";
  const hw = props.token.bunsetsu.head_word;
  const parts = [hw.pos.major];
  if (hw.pos.sub1 && hw.pos.sub1 !== "*") parts.push(hw.pos.sub1);
  return parts.join(" · ");
});
</script>

<template>
  <Transition name="fade">
    <div
      v-if="show && token"
      class="tooltip-panel"
      :class="`tooltip-${placement}`"
      :style="{ left: x + 'px', top: y + 'px' }"
    >
      <div class="tooltip-header">
        <span class="base-form">{{ token.bunsetsu.head_word.base_form }}</span>
        <span class="reading">【{{ token.bunsetsu.head_word.reading || '无读音' }}】</span>
      </div>
      
      <div class="tooltip-pos">{{ formattedPos }}</div>

      <!-- 语法模式标签解释 -->
      <div v-if="token.bunsetsu.grammar_tags.length > 0" class="tooltip-grammar-section">
        <div
          v-for="tag in token.bunsetsu.grammar_tags"
          :key="tag.pattern_id"
          class="grammar-desc"
        >
          <span class="grammar-name">「{{ tag.name_ja }}」</span>
          <span class="grammar-detail">{{ tag.description }}</span>
        </div>
      </div>

      <!-- 词典释义预览 -->
      <div class="tooltip-definition">
        <div class="def-title">词典释义</div>
        <div
          v-if="definitionPreview"
          class="def-body html-content"
          v-html="definitionPreview"
        ></div>
        <div v-else class="def-none">暂无本地词典释义</div>
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.tooltip-panel {
  position: fixed;
  z-index: 1000;
  width: min(320px, calc(100vw - 24px));
  max-height: min(320px, calc(100vh - 24px));
  overflow-y: auto;
  padding: 12px;
  background: var(--glass-bg);
  backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
  border: 1px solid var(--glass-border);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-md);
  color: var(--text-primary);
  font-size: 0.85rem;
  pointer-events: none; /* 穿透鼠标，防止遮挡胶囊触发离开 */
  overflow-wrap: anywhere;
}

.tooltip-above {
  transform: translate(-50%, -100%) translateY(-10px);
}

.tooltip-below {
  transform: translate(-50%, 10px);
}

.tooltip-header {
  display: flex;
  align-items: baseline;
  margin-bottom: 4px;
}

.base-form {
  font-size: 1.15rem;
  font-weight: 600;
  color: var(--accent-color);
}

.reading {
  font-size: 0.8rem;
  color: var(--text-secondary);
  margin-left: 4px;
}

.tooltip-pos {
  font-size: 0.75rem;
  color: var(--text-muted);
  margin-bottom: 8px;
}

.tooltip-grammar-section {
  border-top: 1px solid var(--border-color);
  padding-top: 8px;
  margin-top: 8px;
}

.grammar-desc {
  margin-bottom: 4px;
}

.grammar-name {
  font-weight: 600;
  color: var(--novelty-high-text);
}

.grammar-detail {
  color: var(--text-secondary);
  font-size: 0.8em;
}

.tooltip-definition {
  border-top: 1px solid var(--border-color);
  padding-top: 8px;
  margin-top: 8px;
}

.def-title {
  font-size: 0.75rem;
  font-weight: bold;
  color: var(--text-muted);
  margin-bottom: 4px;
}

.def-body {
  max-height: 120px;
  overflow-y: hidden;
  text-overflow: ellipsis;
  display: -webkit-box;
  -webkit-line-clamp: 4;
  -webkit-box-orient: vertical;
  color: var(--text-secondary);
  font-size: 0.8rem;
  line-height: 1.4;
}

.def-body :deep(*) {
  max-width: 100% !important;
  white-space: normal !important;
}

.def-none {
  color: var(--text-muted);
  font-style: italic;
  font-size: 0.8rem;
}

/* 渐变过渡动画 */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.15s ease, transform 0.15s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
