<script setup lang="ts">
import { computed, type Component, type CSSProperties } from "vue";

export interface SegmentedAction {
  id: string;
  label: string;
  description?: string;
  icon?: Component;
  disabled?: boolean;
  theme: SegmentedActionTheme;
}

export interface SegmentedActionTheme {
  color: string;
  textColor?: string;
}

const props = withDefaults(defineProps<{
  actions: readonly SegmentedAction[];
  width?: string;
  minHeight?: string;
  ariaLabel?: string;
}>(), {
  width: "100%",
  minHeight: "328px",
  ariaLabel: "可选操作",
});

const emit = defineEmits<{
  select: [id: string];
}>();

const visibleActions = computed(() => props.actions.slice(0, 4));

function themeAt(index: number): SegmentedActionTheme {
  return visibleActions.value[index]?.theme ?? { color: "#4b6286" };
}

function actionStyle(index: number): CSSProperties {
  const theme = themeAt(index);
  return {
    "--action-color": theme.color,
    "--action-text-color": theme.textColor ?? "var(--text-primary)",
  } as CSSProperties;
}

function dividerStyle(index: number): CSSProperties {
  const left = themeAt(index).color;
  const right = themeAt(index + 1).color;
  return {
    "--split": `${((index + 1) / visibleActions.value.length) * 100}%`,
    "--tone-a": left,
    "--tone-b": right,
  } as CSSProperties;
}
</script>

<template>
  <section
    class="segmented-action-frame"
    :style="{
      '--frame-height': minHeight,
      '--frame-width': width,
      '--frame-rows': visibleActions.length,
      '--corner-top': themeAt(0).color,
      '--corner-bottom': themeAt(visibleActions.length - 1).color,
    }"
    :aria-label="ariaLabel"
  >
    <span class="segmented-action-frame__corner segmented-action-frame__corner--tl" aria-hidden="true"></span>
    <span class="segmented-action-frame__corner segmented-action-frame__corner--tr" aria-hidden="true"></span>
    <span class="segmented-action-frame__corner segmented-action-frame__corner--bl" aria-hidden="true"></span>
    <span class="segmented-action-frame__corner segmented-action-frame__corner--br" aria-hidden="true"></span>
    <span
      v-for="(_, index) in visibleActions.slice(0, -1)"
      :key="`divider-${index}`"
      class="segmented-action-frame__divider"
      :style="dividerStyle(index)"
      aria-hidden="true"
    ></span>
    <button
      v-for="(action, index) in visibleActions"
      :key="action.id"
      class="segmented-action-frame__action"
      :style="actionStyle(index)"
      type="button"
      :disabled="action.disabled"
      @click="emit('select', action.id)"
    >
      <component :is="action.icon" v-if="action.icon" class="segmented-action-frame__icon" :size="25" stroke-width="2.2" aria-hidden="true" />
      <span class="segmented-action-frame__copy">
        <strong>{{ action.label }}</strong>
        <small v-if="action.description">{{ action.description }}</small>
      </span>
    </button>
  </section>
</template>

<style scoped>
.segmented-action-frame {
  position: relative;
  display: grid;
  min-width: 0;
  width: var(--frame-width);
  min-height: var(--frame-height);
  grid-template-rows: repeat(var(--frame-rows), minmax(0, 1fr));
  overflow: visible;
  --frame-line-width: 7px;
  --frame-line-radius: 4px;
  --frame-corner-length: 38px;
}

.segmented-action-frame__corner {
  position: absolute;
  z-index: 3;
  width: var(--frame-corner-length);
  height: var(--frame-corner-length);
  border-color: var(--corner-top);
  border-style: solid;
  border-width: var(--frame-line-width);
  pointer-events: none;
}

.segmented-action-frame__corner--tl { top: 0; left: 0; border-right: 0; border-bottom: 0; border-top-left-radius: var(--frame-line-radius); }
.segmented-action-frame__corner--tr { top: 0; right: 0; border-bottom: 0; border-left: 0; border-top-right-radius: var(--frame-line-radius); }
.segmented-action-frame__corner--bl { bottom: 0; left: 0; border-color: var(--corner-bottom); border-top: 0; border-right: 0; border-bottom-left-radius: var(--frame-line-radius); }
.segmented-action-frame__corner--br { right: 0; bottom: 0; border-color: var(--corner-bottom); border-top: 0; border-left: 0; border-bottom-right-radius: var(--frame-line-radius); }

.segmented-action-frame__divider {
  position: absolute;
  z-index: 3;
  top: var(--split);
  right: 0;
  left: 0;
  height: var(--frame-line-width);
  border-radius: var(--frame-line-radius);
  background: repeating-linear-gradient(
    90deg,
    var(--tone-a) 0 18px,
    var(--tone-b) 18px 36px
  );
  pointer-events: none;
  transform: translateY(-50%);
}

.segmented-action-frame__divider::before,
.segmented-action-frame__divider::after {
  position: absolute;
  top: 50%;
  width: var(--frame-line-width);
  height: 24px;
  border-radius: var(--frame-line-radius);
  background: linear-gradient(to bottom, var(--tone-a) 0 50%, var(--tone-b) 50% 100%);
  content: "";
  transform: translateY(-50%);
}

.segmented-action-frame__divider::before { left: 0; }
.segmented-action-frame__divider::after { right: 0; }

.segmented-action-frame__action {
  position: relative;
  z-index: 1;
  display: flex;
  min-width: 0;
  min-height: 0;
  align-items: center;
  justify-content: center;
  gap: 13px;
  padding: 24px 22px;
  border: 0;
  background: transparent;
  color: var(--action-text-color);
  cursor: pointer;
  font: inherit;
  text-align: left;
  transition: color 160ms ease, transform 180ms cubic-bezier(.2, .8, .2, 1);
}

.segmented-action-frame__icon { flex: 0 0 auto; color: var(--action-color); }
.segmented-action-frame__copy { display: grid; min-width: 0; gap: 3px; }
.segmented-action-frame__copy strong { color: var(--action-text-color); font-size: .88rem; font-weight: 800; }
.segmented-action-frame__copy small { color: color-mix(in srgb, var(--action-text-color) 70%, var(--text-muted)); font-size: .72rem; }
.segmented-action-frame__action:hover:not(:disabled), .segmented-action-frame__action:focus-visible { color: var(--action-color); outline: 0; transform: translateY(-4px); }
.segmented-action-frame__action:hover:not(:disabled) .segmented-action-frame__copy strong, .segmented-action-frame__action:focus-visible .segmented-action-frame__copy strong { color: var(--action-color); }
.segmented-action-frame__action:disabled { cursor: default; opacity: .5; }

@media (prefers-reduced-motion: reduce) {
  .segmented-action-frame__action { transition: none; }
}
</style>
