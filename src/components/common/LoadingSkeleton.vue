<script setup lang="ts">
withDefaults(defineProps<{ variant?: "dictionary" }>(), { variant: "dictionary" });
</script>

<template>
  <div class="loading-skeleton" :class="`loading-skeleton--${variant}`" role="status" aria-label="正在载入">
    <template v-if="variant === 'dictionary'">
      <div class="skeleton-line skeleton-source"></div>
      <div class="skeleton-line skeleton-headword"></div>
      <div v-for="index in 3" :key="index" class="skeleton-sense">
        <span class="skeleton-dot"></span>
        <span class="skeleton-line" :class="`skeleton-copy-${index}`"></span>
      </div>
    </template>
  </div>
</template>

<style scoped>
.loading-skeleton { display: grid; gap: 9px; padding: 3px 0 5px; overflow: hidden; }
.skeleton-line, .skeleton-dot { background: linear-gradient(100deg, var(--accent-light) 20%, color-mix(in srgb, var(--accent-light) 35%, white) 42%, var(--accent-light) 64%); background-size: 240% 100%; animation: skeleton-shimmer 1.25s ease-in-out infinite; }
.skeleton-line { height: 10px; border-radius: 999px; }
.skeleton-source { width: 34%; height: 9px; }
.skeleton-headword { width: 48%; height: 15px; margin-bottom: 2px; }
.skeleton-sense { display: grid; grid-template-columns: 18px minmax(0, 1fr); gap: 8px; align-items: center; }
.skeleton-dot { width: 18px; height: 18px; border-radius: 50%; }
.skeleton-copy-1 { width: 94%; }
.skeleton-copy-2 { width: 82%; }
.skeleton-copy-3 { width: 68%; }
@keyframes skeleton-shimmer { from { background-position: 100% 0; } to { background-position: -100% 0; } }
@media (prefers-reduced-motion: reduce) { .skeleton-line, .skeleton-dot { animation: none; } }
</style>
