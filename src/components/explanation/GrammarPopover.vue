<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import type { GrammarTag } from "../../types";
import { floatDebug } from "../../explanation/floatDebug";
import type { RectSnapshot } from "../../explanation/geometry";

const props = defineProps<{
  show: boolean;
  tag: GrammarTag | null;
  anchor: RectSnapshot | null;
}>();

const emit = defineEmits<{
  enter: [event: PointerEvent];
  leave: [event: PointerEvent];
}>();
const panelRef = ref<HTMLElement | null>(null);

const style = computed(() => {
  const anchor = props.anchor;
  if (!anchor) return { left: "-10000px", top: "-10000px" };
  const width = Math.min(360, window.innerWidth - 24);
  const left = Math.min(window.innerWidth - 12 - width, Math.max(12, anchor.left + anchor.width / 2 - width / 2));
  const above = anchor.top > window.innerHeight / 2;
  return {
    left: `${left}px`,
    top: above ? `${anchor.top - 8}px` : `${anchor.bottom + 8}px`,
    width: `${width}px`,
    transform: above ? "translateY(-100%)" : undefined,
  };
});

watch(
  () => [props.show, props.tag, props.anchor],
  async () => {
    if (!props.show) return;
    await nextTick();
    const panel = panelRef.value;
    if (!panel) return;
    const rect = panel.getBoundingClientRect();
    floatDebug.snapshot("panelBoxes", {
      whole: null,
      component: null,
      grammar: {
        left: Math.round(rect.left),
        top: Math.round(rect.top),
        right: Math.round(rect.right),
        bottom: Math.round(rect.bottom),
        width: Math.round(rect.width),
        height: Math.round(rect.height),
      },
    });
  },
  { flush: "post" },
);
</script>

<template>
  <aside
    v-if="show && tag"
    ref="panelRef"
    class="grammar-popover"
    data-explanation-panel="grammar"
    :style="style"
    role="dialog"
    aria-label="语法说明"
    @pointerenter="emit('enter', $event)"
    @pointerleave="emit('leave', $event)"
  >
    <header>
      <strong>{{ tag.name_ja }}</strong>
      <span v-if="tag.jlpt_level">JLPT N{{ tag.jlpt_level }}</span>
    </header>
    <p>{{ tag.description }}</p>
  </aside>
</template>

<style scoped>
.grammar-popover {
  position: fixed;
  z-index: 1010;
  box-sizing: border-box;
  padding: 14px 16px;
  border: 1px solid color-mix(in srgb, #1769aa 35%, var(--border-color));
  border-radius: var(--radius-md);
  background: var(--glass-bg);
  box-shadow: var(--shadow-md);
  backdrop-filter: var(--glass-filter);
  color: var(--text-primary);
  font: .88rem/1.55 var(--font-ja);
}
header { display: flex; align-items: baseline; justify-content: space-between; gap: 12px; }
strong { color: #1769aa; font-size: 1rem; }
span { color: var(--text-muted); font: 700 .7rem var(--font-ui); }
p { margin: 8px 0 0; color: var(--text-secondary); }
</style>
