<script setup lang="ts">
defineProps<{
  show: boolean;
  x: number;
  y: number;
  width: number;
  maxHeight: number;
  title: string;
  surface: string;
}>();

const emit = defineEmits<{ enter: []; leave: [] }>();
</script>

<template>
  <Transition name="fade">
    <aside
      v-if="show"
      class="reader-explanation"
      :style="{ left: `${x}px`, top: `${y}px`, width: `${width}px`, maxHeight: `${maxHeight}px` }"
      role="dialog"
      :aria-label="title"
      @pointerenter="emit('enter')"
      @pointerleave="emit('leave')"
      @wheel.stop
    >
      <header>
        <div><strong>{{ title }}</strong><span>{{ surface }}</span></div>
      </header>
      <slot />
    </aside>
  </Transition>
</template>

<style scoped>
.reader-explanation { position: fixed; z-index: 1001; box-sizing: border-box; width: min(310px, calc(100vw - 24px)); overflow: auto; overflow-wrap: anywhere; overscroll-behavior: contain; padding: 14px; border: 1px solid var(--glass-border); border-radius: var(--radius-md); background: var(--glass-bg); backdrop-filter: var(--glass-filter); box-shadow: var(--shadow-md); color: var(--text-primary); font: .84rem/1.5 var(--font-ui); }
.reader-explanation header { display: flex; align-items: flex-start; justify-content: space-between; gap: 8px; margin-bottom: 10px; }
.reader-explanation header div { display: flex; min-width: 0; flex-wrap: wrap; align-items: baseline; gap: 4px 10px; }
.reader-explanation strong { color: var(--accent-color); }
.reader-explanation header span { font-family: var(--font-ja); font-size: 1.05rem; }
.fade-enter-active, .fade-leave-active { transition: opacity 120ms ease; }
.fade-enter-from, .fade-leave-to { opacity: 0; }
@media (prefers-reduced-motion: reduce) { .fade-enter-active, .fade-leave-active { transition: none; } }
</style>
