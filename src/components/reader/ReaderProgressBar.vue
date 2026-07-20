<script setup lang="ts">
defineProps<{
  percent: number;
  currentChapter: string;
  remainingLabel: string;
  completionLabel: string;
}>();
</script>

<template>
  <footer class="reading-progress" aria-label="阅读进度">
    <div class="progress-track"><i :style="{ width: `${Math.min(100, Math.max(0, percent * 100))}%` }"></i></div>
    <div class="progress-copy">
      <strong>{{ currentChapter || '正文' }}</strong>
      <span>{{ Math.round(percent * 100) }}%</span>
      <span>{{ remainingLabel }}</span>
      <span>预计 {{ completionLabel }} 完成</span>
    </div>
  </footer>
</template>

<style scoped>
.reading-progress {
  position: absolute;
  z-index: 35;
  right: 0;
  bottom: 0;
  left: 0;
  background: color-mix(in srgb, var(--bg-primary) 94%, transparent);
  backdrop-filter: blur(10px);
}

.progress-track {
  height: 2px;
  background: var(--border-color);
}

.progress-track i {
  display: block;
  height: 100%;
  background: var(--accent-color);
}

.progress-copy {
  display: flex;
  min-height: 30px;
  align-items: center;
  justify-content: center;
  gap: 10px;
  padding: 4px 16px;
  color: var(--text-muted);
  font-size: 0.7rem;
  font-variant-numeric: tabular-nums;
}

.progress-copy strong {
  max-width: min(360px, 32vw);
  overflow: hidden;
  color: var(--text-secondary);
  font-weight: 500;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.progress-copy span + span::before {
  content: "·";
  margin-right: 10px;
}

@media (max-width: 640px) {
  .progress-copy span:last-child {
    display: none;
  }
}
</style>
