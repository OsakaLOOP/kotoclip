<script setup lang="ts">
import { BookOpenText } from "@lucide/vue";

withDefaults(defineProps<{ size?: "small" | "large" }>(), { size: "large" });
</script>

<template>
  <span class="reader-opening-mark" :class="`reader-opening-mark--${size}`" aria-hidden="true">
    <BookOpenText :size="size === 'small' ? 23 : 28" stroke-width="2.5" />
  </span>
</template>

<style scoped>
.reader-opening-mark {
  position: relative;
  z-index: 1;
  display: grid;
  width: 46px;
  height: 46px;
  place-items: center;
  border-radius: 7px;
  background: var(--accent-color);
  box-shadow: 0 4px 14px color-mix(in srgb, var(--text-primary) 20%, transparent);
  color: #fff;
  animation: reader-opening-mark 280ms cubic-bezier(.4, 0, .2, 1) both;
}

.reader-opening-mark::before {
  position: absolute;
  z-index: 0;
  width: 100%;
  aspect-ratio: 1;
  border-radius: 50%;
  background: color-mix(in srgb, var(--accent-color) 18%, transparent);
  content: "";
  animation: reader-opening-wave 280ms cubic-bezier(0, 0, .2, 1) both;
}

.reader-opening-mark--small {
  width: 38px;
  height: 38px;
  border-radius: 6px;
}

@keyframes reader-opening-mark {
  0% { opacity: 0; transform: translateY(8px) scale(.86); }
  62% { opacity: 1; transform: translateY(-2px) scale(1.03); }
  100% { opacity: 1; transform: translateY(0) scale(1); }
}

@keyframes reader-opening-wave {
  0% { opacity: 0; transform: scale(.25); }
  28% { opacity: .42; }
  100% { opacity: 0; transform: scale(7.5); }
}

@media (prefers-reduced-motion: reduce) {
  .reader-opening-mark,
  .reader-opening-mark::before { animation: none; }
}
</style>
