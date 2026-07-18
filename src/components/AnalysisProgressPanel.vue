<script setup lang="ts">
import { computed } from "vue";
import type { AnalysisPhase, AnalysisProgress } from "../composables/useTokenization";

const props = defineProps<{
  progress: AnalysisProgress;
  active: boolean;
}>();

const stages: Array<{ phase: AnalysisPhase; label: string }> = [
  { phase: "preparing", label: "准备" },
  { phase: "tokenizing", label: "形态素" },
  { phase: "dictionary_matching", label: "词典" },
  { phase: "chunking", label: "文节" },
  { phase: "grammar_matching", label: "语法" },
  { phase: "profile_scoring", label: "评分" },
  { phase: "expression_matching", label: "表达" },
  { phase: "recording_exposure", label: "记录" },
];

const phaseIndex = computed(() => {
  if (props.progress.phase === "completed") return stages.length;
  return stages.findIndex((stage) => stage.phase === props.progress.phase);
});

const currentLabel = computed(() => {
  if (props.progress.phase === "completed") return "完成";
  return stages.find((stage) => stage.phase === props.progress.phase)?.label ?? "分析";
});

const countText = computed(() => {
  if (props.progress.total <= 0) return "";
  return `${props.progress.completed.toLocaleString()} / ${props.progress.total.toLocaleString()}`;
});
</script>

<template>
  <Transition name="analysis-progress">
    <section
      v-if="active"
      class="analysis-progress-panel"
      aria-live="polite"
      aria-atomic="true"
    >
      <div class="progress-summary">
        <span class="progress-pulse" aria-hidden="true"></span>
        <strong>{{ currentLabel }}</strong>
        <span class="progress-message">{{ progress.message }}</span>
        <span v-if="countText" class="progress-count">{{ countText }}</span>
        <span class="progress-percent">{{ progress.percent }}%</span>
      </div>

      <div
        class="progress-track"
        role="progressbar"
        :aria-valuenow="progress.percent"
        aria-valuemin="0"
        aria-valuemax="100"
        :aria-label="progress.message"
      >
        <span class="progress-fill" :style="{ width: `${progress.percent}%` }"></span>
      </div>

      <ol class="stage-list" aria-label="NLP 分析阶段">
        <li
          v-for="(stage, index) in stages"
          :key="stage.phase"
          :class="{
            complete: index < phaseIndex,
            current: index === phaseIndex,
          }"
        >
          <span class="stage-dot" aria-hidden="true"></span>
          <span>{{ stage.label }}</span>
        </li>
      </ol>
    </section>
  </Transition>
</template>

<style scoped>
.analysis-progress-panel {
  width: 100%;
  padding: 14px 16px 12px;
  border: 1px solid var(--border-color);
  border-radius: var(--radius-md);
  background: var(--bg-card);
  box-shadow: var(--shadow-sm);
}

.progress-summary {
  display: grid;
  grid-template-columns: auto auto minmax(0, 1fr) auto auto;
  align-items: center;
  gap: 8px;
  margin-bottom: 9px;
  color: var(--text-primary);
  font-size: 0.84rem;
}

.progress-pulse {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--accent-color);
  animation: progress-pulse 1.25s ease-in-out infinite;
}

.progress-message {
  min-width: 0;
  overflow: hidden;
  color: var(--text-secondary);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.progress-count,
.progress-percent {
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.progress-percent {
  min-width: 3ch;
  color: var(--accent-color);
  font-weight: 700;
  text-align: right;
}

.progress-track {
  position: relative;
  height: 7px;
  overflow: hidden;
  border-radius: 999px;
  background: var(--accent-light);
}

.progress-fill {
  position: relative;
  display: block;
  height: 100%;
  overflow: hidden;
  border-radius: inherit;
  background: var(--accent-color);
  transition: width 180ms ease-out;
}

.progress-fill::after {
  position: absolute;
  inset: 0;
  background: linear-gradient(100deg, transparent 20%, rgba(255, 255, 255, 0.45) 50%, transparent 80%);
  content: "";
  transform: translateX(-100%);
  animation: progress-sheen 1.4s linear infinite;
}

.stage-list {
  display: grid;
  grid-template-columns: repeat(7, minmax(0, 1fr));
  gap: 4px;
  margin-top: 10px;
  list-style: none;
}

.stage-list li {
  display: flex;
  min-width: 0;
  align-items: center;
  justify-content: center;
  gap: 4px;
  color: var(--text-muted);
  font-size: 0.7rem;
  white-space: nowrap;
}

.stage-dot {
  width: 5px;
  height: 5px;
  flex: 0 0 auto;
  border: 1px solid currentColor;
  border-radius: 50%;
}

.stage-list li.complete,
.stage-list li.current {
  color: var(--accent-color);
}

.stage-list li.complete .stage-dot,
.stage-list li.current .stage-dot {
  background: currentColor;
}

.stage-list li.current {
  font-weight: 700;
}

.analysis-progress-enter-active,
.analysis-progress-leave-active {
  transition: opacity 160ms ease;
}

.analysis-progress-enter-from,
.analysis-progress-leave-to {
  opacity: 0;
}

@keyframes progress-pulse {
  50% { opacity: 0.38; transform: scale(0.72); }
}

@keyframes progress-sheen {
  to { transform: translateX(100%); }
}

@media (max-width: 640px) {
  .progress-summary {
    grid-template-columns: auto auto minmax(0, 1fr) auto;
  }

  .progress-count {
    display: none;
  }

  .stage-list li {
    font-size: 0.64rem;
  }
}

@media (prefers-reduced-motion: reduce) {
  .progress-pulse,
  .progress-fill::after {
    animation: none;
  }

  .progress-fill,
  .analysis-progress-enter-active,
  .analysis-progress-leave-active {
    transition: none;
  }
}

:global(body.eink-mode) .analysis-progress-panel {
  border: 1px solid #000;
  border-radius: 0;
  background: #fff;
}

:global(body.eink-mode) .progress-track {
  border: 1px solid #000;
  border-radius: 0;
  background: #fff;
}

:global(body.eink-mode) .progress-fill,
:global(body.eink-mode) .stage-dot,
:global(body.eink-mode) .progress-pulse {
  border-radius: 0;
  background: #000;
}
</style>
