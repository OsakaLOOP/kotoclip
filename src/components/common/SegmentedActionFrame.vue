<script setup lang="ts">
import { computed, onBeforeUnmount, ref, type Component, type CSSProperties } from "vue";
import {
  frameMotionRatio,
  minimumFrameCenterGap,
  segmentedFrameLayout,
  type FrameCornerDefinition,
  type FrameDividerDefinition,
  type SegmentedActionLayout,
} from "./segmentedActionFrameGeometry";

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
  layout?: SegmentedActionLayout;
  hoverExpansion?: number;
  centerPlateau?: number;
  falloffPower?: number;
  centerGap?: number;
  maxStateOpacity?: number;
  ariaLabel?: string;
}>(), {
  width: "100%",
  minHeight: "328px",
  layout: "2x1",
  hoverExpansion: 6,
  centerPlateau: .2,
  falloffPower: .72,
  centerGap: 28,
  maxStateOpacity: .075,
  ariaLabel: "可选操作",
});

const emit = defineEmits<{
  select: [id: string];
}>();

const layoutDefinition = computed(() => segmentedFrameLayout(props.layout));
const visibleActions = computed(() => props.actions.slice(0, layoutDefinition.value.capacity));
const frameElement = ref<HTMLElement | null>(null);
const activeIndex = ref<number | null>(null);
const pressedIndex = ref<number | null>(null);
const releaseLocked = ref(false);
const ripple = ref<{
  key: number;
  index: number;
  left: number;
  top: number;
  size: number;
} | null>(null);
let rippleKey = 0;
let rippleTimer: number | undefined;
let pressReleaseTimer: number | undefined;
let settleTimer: number | undefined;
let pointerFrame: number | undefined;
let pressStartedAt = 0;
let pointerInside = false;
let currentExpansion = 0;
let pendingPointerMotion: { index: number; expansion: number } | null = null;

const PRESS_EXPANSION = 3;
const PRESS_DURATION = 85;
const RELEASE_DURATION = 420;
const FRAME_LINE_WIDTH = 7;
const effectiveCenterGap = computed(() => Math.max(
  props.centerGap,
  minimumFrameCenterGap(props.hoverExpansion, PRESS_EXPANSION, FRAME_LINE_WIDTH),
));

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

function cornerStyle(corner: FrameCornerDefinition): CSSProperties {
  return {
    "--corner-color": themeAt(corner.actionIndex).color,
  } as CSSProperties;
}

function dividerStyle(divider: FrameDividerDefinition): CSSProperties {
  const before = themeAt(divider.beforeIndex).color;
  const after = themeAt(divider.afterIndex).color;
  return {
    "--split": divider.split,
    "--divider-start": divider.start,
    "--divider-end": divider.end,
    "--tone-a": before,
    "--tone-b": after,
  } as CSSProperties;
}

function actionPosition(index: number): Record<string, boolean> {
  const row = Math.floor(index / layoutDefinition.value.columns);
  const column = index % layoutDefinition.value.columns;
  return {
    "is-first-row": row === 0,
    "is-last-row": row === layoutDefinition.value.rows - 1,
    "is-first-column": column === 0,
    "is-last-column": column === layoutDefinition.value.columns - 1,
  };
}

function isVisuallyActive(index: number): boolean {
  return activeIndex.value === index && !releaseLocked.value;
}

function applyFrameMotion(nextExpansion: number, duration: number, force = false) {
  const next = Math.max(0, nextExpansion);
  if (!force && Math.abs(next - currentExpansion) < .001) return;
  currentExpansion = next;

  const ratio = props.hoverExpansion <= 0
    ? 0
    : Math.min(1, Math.max(0, next / props.hoverExpansion));
  const style = frameElement.value?.style;
  if (!style) return;
  style.setProperty("--frame-motion-duration", `${duration}ms`);
  style.setProperty("--frame-motion", `${next}px`);
  style.setProperty("--frame-motion-ratio", `${ratio}`);
  style.setProperty("--state-opacity", `${ratio * props.maxStateOpacity}`);
}

function cancelPointerMotion() {
  pendingPointerMotion = null;
  if (pointerFrame !== undefined) {
    window.cancelAnimationFrame(pointerFrame);
    pointerFrame = undefined;
  }
}

function flushPointerMotion() {
  pointerFrame = undefined;
  const pending = pendingPointerMotion;
  pendingPointerMotion = null;
  if (!pending || pressedIndex.value !== null || releaseLocked.value || !pointerInside) return;
  activeIndex.value = pending.index;
  // 指针本身已经提供连续时间轴；此处直接提交该帧，避免反复重启 CSS transition。
  applyFrameMotion(pending.expansion, 0);
}

function updatePointerExpansion(event: PointerEvent, index: number, immediate = false) {
  if (pressedIndex.value !== null || releaseLocked.value) return;
  const bounds = (event.currentTarget as HTMLElement).getBoundingClientRect();
  const dx = (event.clientX - bounds.left - bounds.width / 2) / Math.max(1, bounds.width / 2);
  const dy = (event.clientY - bounds.top - bounds.height / 2) / Math.max(1, bounds.height / 2);
  const nextExpansion = props.hoverExpansion * frameMotionRatio(
    dx,
    dy,
    props.centerPlateau,
    props.falloffPower,
  );
  activeIndex.value = index;
  if (immediate) {
    cancelPointerMotion();
    applyFrameMotion(nextExpansion, 90, true);
    return;
  }
  if (Math.abs(nextExpansion - currentExpansion) < .001) return;
  pendingPointerMotion = { index, expansion: nextExpansion };
  if (pointerFrame === undefined) pointerFrame = window.requestAnimationFrame(flushPointerMotion);
}

function handlePointerEnter(event: PointerEvent, index: number) {
  pointerInside = true;
  if (settleTimer !== undefined) {
    window.clearTimeout(settleTimer);
    settleTimer = undefined;
  }
  releaseLocked.value = false;
  updatePointerExpansion(event, index, true);
}

function handlePointerLeave() {
  pointerInside = false;
  cancelPointerMotion();
  if (pressedIndex.value !== null) {
    releasePress();
    return;
  }
  if (releaseLocked.value) {
    scheduleSettle();
    return;
  }
  activeIndex.value = null;
  applyFrameMotion(0, 220, true);
}

function handlePointerDown(event: PointerEvent, index: number) {
  cancelPointerMotion();
  if (pressReleaseTimer !== undefined) {
    window.clearTimeout(pressReleaseTimer);
    pressReleaseTimer = undefined;
  }
  if (settleTimer !== undefined) {
    window.clearTimeout(settleTimer);
    settleTimer = undefined;
  }
  activeIndex.value = index;
  pressedIndex.value = index;
  releaseLocked.value = false;
  pressStartedAt = performance.now();
  applyFrameMotion(currentExpansion + PRESS_EXPANSION, PRESS_DURATION, true);
  startRipple(event, index);
}

function releasePress() {
  if (pressedIndex.value === null) return;
  const remaining = Math.max(0, PRESS_DURATION - (performance.now() - pressStartedAt));
  if (pressReleaseTimer !== undefined) window.clearTimeout(pressReleaseTimer);
  pressReleaseTimer = window.setTimeout(startRelease, remaining);
}

function startRelease() {
  pressReleaseTimer = undefined;
  pressedIndex.value = null;
  releaseLocked.value = true;
  applyFrameMotion(0, RELEASE_DURATION, true);
  if (!pointerInside) scheduleSettle();
}

function scheduleSettle() {
  if (settleTimer !== undefined) window.clearTimeout(settleTimer);
  settleTimer = window.setTimeout(() => {
    activeIndex.value = null;
    releaseLocked.value = false;
    settleTimer = undefined;
  }, RELEASE_DURATION);
}

function handleFocus(index: number) {
  if (pressedIndex.value !== null) return;
  cancelPointerMotion();
  activeIndex.value = index;
  releaseLocked.value = false;
  applyFrameMotion(props.hoverExpansion, 180, true);
}

function handleBlur() {
  cancelPointerMotion();
  activeIndex.value = null;
  pressedIndex.value = null;
  releaseLocked.value = false;
  applyFrameMotion(0, 220, true);
}

function startRipple(event: PointerEvent | MouseEvent, index: number, centered = false) {
  const button = event.currentTarget as HTMLElement | null;
  if (!button) return;
  const bounds = button.getBoundingClientRect();
  const x = centered ? bounds.width / 2 : Math.min(bounds.width, Math.max(0, event.clientX - bounds.left));
  const y = centered ? bounds.height / 2 : Math.min(bounds.height, Math.max(0, event.clientY - bounds.top));
  const radius = Math.hypot(Math.max(x, bounds.width - x), Math.max(y, bounds.height - y));
  ripple.value = {
    key: ++rippleKey,
    index,
    left: x - radius,
    top: y - radius,
    size: radius * 2,
  };
  if (rippleTimer !== undefined) window.clearTimeout(rippleTimer);
  rippleTimer = window.setTimeout(() => {
    ripple.value = null;
    rippleTimer = undefined;
  }, 480);
}

function handleClick(event: MouseEvent, action: SegmentedAction, index: number) {
  if (event.detail === 0) startRipple(event, index, true);
  emit("select", action.id);
}

function rippleStyle(): CSSProperties | undefined {
  if (!ripple.value) return undefined;
  return {
    width: `${ripple.value.size}px`,
    height: `${ripple.value.size}px`,
    left: `calc(${ripple.value.left}px + var(--state-origin-x))`,
    top: `calc(${ripple.value.top}px + var(--state-origin-y))`,
  };
}

onBeforeUnmount(() => {
  cancelPointerMotion();
  if (rippleTimer !== undefined) window.clearTimeout(rippleTimer);
  if (pressReleaseTimer !== undefined) window.clearTimeout(pressReleaseTimer);
  if (settleTimer !== undefined) window.clearTimeout(settleTimer);
});
</script>

<template>
  <section
    ref="frameElement"
    class="segmented-action-frame"
    :style="{
      '--frame-height': minHeight,
      '--frame-width': width,
      '--frame-rows': layoutDefinition.rows,
      '--frame-columns': layoutDefinition.columns,
      '--frame-center-gap': `${effectiveCenterGap}px`,
    }"
    :data-layout="layout"
    :aria-label="ariaLabel"
  >
    <span
      v-for="corner in layoutDefinition.corners"
      :key="corner.position"
      class="segmented-action-frame__corner"
      :class="[
        `segmented-action-frame__corner--${corner.position}`,
        {
          'is-active': isVisuallyActive(corner.actionIndex),
          'is-top': activeIndex === corner.actionIndex,
        },
      ]"
      :style="cornerStyle(corner)"
      aria-hidden="true"
    ></span>
    <span
      v-for="divider in layoutDefinition.dividers"
      :key="divider.id"
      class="segmented-action-frame__divider"
      :class="[
        `segmented-action-frame__divider--${divider.orientation}`,
        {
          'is-active-before': isVisuallyActive(divider.beforeIndex),
          'is-active-after': isVisuallyActive(divider.afterIndex),
          'is-top-before': activeIndex === divider.beforeIndex,
          'is-top-after': activeIndex === divider.afterIndex,
          'is-start-outer': divider.startOuter,
          'is-end-outer': divider.endOuter,
        },
      ]"
      :style="dividerStyle(divider)"
      aria-hidden="true"
    >
      <span class="segmented-action-frame__stroke-group segmented-action-frame__stroke-group--before">
        <i class="segmented-action-frame__rail segmented-action-frame__rail--before"></i>
        <i class="segmented-action-frame__connector segmented-action-frame__connector--left-before"></i>
        <i class="segmented-action-frame__connector segmented-action-frame__connector--right-before"></i>
        <i class="segmented-action-frame__endpoint segmented-action-frame__endpoint--left-before"></i>
        <i class="segmented-action-frame__endpoint segmented-action-frame__endpoint--right-before"></i>
      </span>
      <span class="segmented-action-frame__stroke-group segmented-action-frame__stroke-group--after">
        <i class="segmented-action-frame__rail segmented-action-frame__rail--after"></i>
        <i class="segmented-action-frame__connector segmented-action-frame__connector--left-after"></i>
        <i class="segmented-action-frame__connector segmented-action-frame__connector--right-after"></i>
        <i class="segmented-action-frame__endpoint segmented-action-frame__endpoint--left-after"></i>
        <i class="segmented-action-frame__endpoint segmented-action-frame__endpoint--right-after"></i>
      </span>
    </span>
    <button
      v-for="(action, index) in visibleActions"
      :key="action.id"
      class="segmented-action-frame__action"
      :class="{
        'is-active': isVisuallyActive(index),
        'is-pressed': pressedIndex === index,
        'is-top': activeIndex === index,
        ...actionPosition(index),
      }"
      :style="actionStyle(index)"
      type="button"
      :disabled="action.disabled"
      @pointerenter="handlePointerEnter($event, index)"
      @pointermove="updatePointerExpansion($event, index)"
      @pointerleave="handlePointerLeave"
      @focus="handleFocus(index)"
      @blur="handleBlur"
      @pointerdown="handlePointerDown($event, index)"
      @pointerup="releasePress"
      @pointercancel="releasePress"
      @click="handleClick($event, action, index)"
    >
      <span class="segmented-action-frame__state-layer" aria-hidden="true">
        <span
          v-if="ripple?.index === index"
          :key="ripple.key"
          class="segmented-action-frame__ripple"
          :style="rippleStyle()"
        ></span>
      </span>
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
  grid-template-columns: repeat(var(--frame-columns), minmax(0, 1fr));
  overflow: visible;
  --frame-line-width: 7px;
  --frame-half-line-width: 3.5px;
  --frame-line-radius: 12px;
  --frame-corner-length: 38px;
  --frame-endpoint-length: 25px;
  --frame-motion: 0px;
  --frame-motion-ratio: 0;
  --frame-motion-duration: 180ms;
  --frame-motion-curve: cubic-bezier(.2, .8, .2, 1);
  --frame-center-gap: 28px;
  --state-opacity: 0;
}

.segmented-action-frame__corner {
  position: absolute;
  z-index: 3;
  width: var(--frame-corner-length);
  height: var(--frame-corner-length);
  border-color: var(--corner-color);
  border-style: solid;
  border-width: var(--frame-line-width);
  color: var(--corner-color);
  pointer-events: none;
  transition: filter var(--frame-motion-duration) var(--frame-motion-curve), transform var(--frame-motion-duration) var(--frame-motion-curve);
}

.segmented-action-frame__corner.is-top { z-index: 5; }

.segmented-action-frame__corner--top-left { top: 0; left: 0; border-right: 0; border-bottom: 0; border-top-left-radius: var(--frame-line-radius); }
.segmented-action-frame__corner--top-right { top: 0; right: 0; border-bottom: 0; border-left: 0; border-top-right-radius: var(--frame-line-radius); }
.segmented-action-frame__corner--bottom-left { bottom: 0; left: 0; border-top: 0; border-right: 0; border-bottom-left-radius: var(--frame-line-radius); }
.segmented-action-frame__corner--bottom-right { right: 0; bottom: 0; border-top: 0; border-left: 0; border-bottom-right-radius: var(--frame-line-radius); }
.segmented-action-frame__corner--top-left.is-active { transform: translate(calc(-1 * var(--frame-motion)), calc(-1 * var(--frame-motion))); }
.segmented-action-frame__corner--top-right.is-active { transform: translate(var(--frame-motion), calc(-1 * var(--frame-motion))); }
.segmented-action-frame__corner--bottom-left.is-active { transform: translate(calc(-1 * var(--frame-motion)), var(--frame-motion)); }
.segmented-action-frame__corner--bottom-right.is-active { transform: translate(var(--frame-motion), var(--frame-motion)); }
.segmented-action-frame__corner.is-active { filter: brightness(calc(1 + var(--frame-motion-ratio) * .08)) drop-shadow(0 0 7px color-mix(in srgb, currentColor calc(var(--frame-motion-ratio) * 34%), transparent)); }

.segmented-action-frame__divider {
  position: absolute;
  z-index: 3;
  background: transparent;
  isolation: isolate;
  pointer-events: none;
  --start-axis-motion: 0px;
  --end-axis-motion: 0px;
}

.segmented-action-frame__divider.is-start-outer { --start-axis-motion: var(--frame-motion); }
.segmented-action-frame__divider.is-end-outer { --end-axis-motion: var(--frame-motion); }
.segmented-action-frame__divider--horizontal { top: var(--split); right: var(--divider-end); left: var(--divider-start); height: var(--frame-line-width); transform: translateY(-50%); }
.segmented-action-frame__divider--vertical { top: var(--divider-start); bottom: var(--divider-end); left: var(--split); width: var(--frame-line-width); transform: translateX(-50%); }

.segmented-action-frame__stroke-group {
  position: absolute;
  z-index: 1;
  inset: 0;
  transition: filter var(--frame-motion-duration) var(--frame-motion-curve);
}

.segmented-action-frame__divider.is-top-before .segmented-action-frame__stroke-group--before,
.segmented-action-frame__divider.is-top-after .segmented-action-frame__stroke-group--after { z-index: 2; }

.segmented-action-frame__divider.is-active-before .segmented-action-frame__stroke-group--before {
  filter: brightness(calc(1 + var(--frame-motion-ratio) * .08)) drop-shadow(0 0 6px color-mix(in srgb, var(--tone-a) calc(var(--frame-motion-ratio) * 32%), transparent));
}

.segmented-action-frame__divider.is-active-after .segmented-action-frame__stroke-group--after {
  filter: brightness(calc(1 + var(--frame-motion-ratio) * .08)) drop-shadow(0 0 6px color-mix(in srgb, var(--tone-b) calc(var(--frame-motion-ratio) * 32%), transparent));
}

.segmented-action-frame__rail {
  position: absolute;
  z-index: 1;
  transition: filter var(--frame-motion-duration) var(--frame-motion-curve), transform var(--frame-motion-duration) var(--frame-motion-curve);
}

.segmented-action-frame__divider--horizontal .segmented-action-frame__rail { inset: 0 var(--frame-line-width); }
.segmented-action-frame__divider--vertical .segmented-action-frame__rail { inset: var(--frame-line-width) 0; }
.segmented-action-frame__divider--horizontal .segmented-action-frame__rail--before {
  background: repeating-linear-gradient(90deg, var(--tone-a) 0 18px, transparent 18px 36px);
}

.segmented-action-frame__divider--horizontal .segmented-action-frame__rail--after {
  background: repeating-linear-gradient(90deg, transparent 0 18px, var(--tone-b) 18px 36px);
}

.segmented-action-frame__divider--vertical .segmented-action-frame__rail--before {
  background: repeating-linear-gradient(180deg, var(--tone-a) 0 18px, transparent 18px 36px);
}

.segmented-action-frame__divider--vertical .segmented-action-frame__rail--after {
  background: repeating-linear-gradient(180deg, transparent 0 18px, var(--tone-b) 18px 36px);
}

.segmented-action-frame__endpoint {
  position: absolute;
  z-index: 3;
  transition: filter var(--frame-motion-duration) var(--frame-motion-curve), transform var(--frame-motion-duration) var(--frame-motion-curve);
}

.segmented-action-frame__connector {
  position: absolute;
  z-index: 2;
  width: var(--frame-line-width);
  height: var(--frame-line-width);
  opacity: 0;
  transition:
    top var(--frame-motion-duration) var(--frame-motion-curve),
    bottom var(--frame-motion-duration) var(--frame-motion-curve),
    left var(--frame-motion-duration) var(--frame-motion-curve),
    right var(--frame-motion-duration) var(--frame-motion-curve),
    transform var(--frame-motion-duration) var(--frame-motion-curve),
    width var(--frame-motion-duration) var(--frame-motion-curve),
    height var(--frame-motion-duration) var(--frame-motion-curve),
    opacity 0ms linear var(--frame-motion-duration);
}

.segmented-action-frame__divider--horizontal .segmented-action-frame__endpoint { width: var(--frame-line-width); height: var(--frame-endpoint-length); }
.segmented-action-frame__divider--horizontal .segmented-action-frame__endpoint--left-before,
.segmented-action-frame__divider--horizontal .segmented-action-frame__endpoint--right-before { bottom: 50%; background: var(--tone-a); border-radius: var(--frame-line-radius) var(--frame-line-radius) 0 0; }
.segmented-action-frame__divider--horizontal .segmented-action-frame__endpoint--left-after,
.segmented-action-frame__divider--horizontal .segmented-action-frame__endpoint--right-after { top: 50%; background: var(--tone-b); border-radius: 0 0 var(--frame-line-radius) var(--frame-line-radius); }
.segmented-action-frame__divider--horizontal .segmented-action-frame__endpoint--left-before,
.segmented-action-frame__divider--horizontal .segmented-action-frame__endpoint--left-after,
.segmented-action-frame__divider--horizontal .segmented-action-frame__connector--left-before,
.segmented-action-frame__divider--horizontal .segmented-action-frame__connector--left-after { left: 0; }
.segmented-action-frame__divider--horizontal .segmented-action-frame__endpoint--right-before,
.segmented-action-frame__divider--horizontal .segmented-action-frame__endpoint--right-after,
.segmented-action-frame__divider--horizontal .segmented-action-frame__connector--right-before,
.segmented-action-frame__divider--horizontal .segmented-action-frame__connector--right-after { right: 0; }
.segmented-action-frame__divider--horizontal .segmented-action-frame__connector { top: 0; }

.segmented-action-frame__divider--vertical .segmented-action-frame__endpoint { width: var(--frame-endpoint-length); height: var(--frame-line-width); }
.segmented-action-frame__divider--vertical .segmented-action-frame__endpoint--left-before,
.segmented-action-frame__divider--vertical .segmented-action-frame__endpoint--right-before { right: 50%; background: var(--tone-a); border-radius: var(--frame-line-radius) 0 0 var(--frame-line-radius); }
.segmented-action-frame__divider--vertical .segmented-action-frame__endpoint--left-after,
.segmented-action-frame__divider--vertical .segmented-action-frame__endpoint--right-after { left: 50%; background: var(--tone-b); border-radius: 0 var(--frame-line-radius) var(--frame-line-radius) 0; }
.segmented-action-frame__divider--vertical .segmented-action-frame__endpoint--left-before,
.segmented-action-frame__divider--vertical .segmented-action-frame__endpoint--left-after,
.segmented-action-frame__divider--vertical .segmented-action-frame__connector--left-before,
.segmented-action-frame__divider--vertical .segmented-action-frame__connector--left-after { top: 0; }
.segmented-action-frame__divider--vertical .segmented-action-frame__endpoint--right-before,
.segmented-action-frame__divider--vertical .segmented-action-frame__endpoint--right-after,
.segmented-action-frame__divider--vertical .segmented-action-frame__connector--right-before,
.segmented-action-frame__divider--vertical .segmented-action-frame__connector--right-after { bottom: 0; }
.segmented-action-frame__divider--vertical .segmented-action-frame__connector { left: 0; }

.segmented-action-frame__connector--left-before,
.segmented-action-frame__connector--right-before { background: var(--tone-a); }
.segmented-action-frame__connector--left-after,
.segmented-action-frame__connector--right-after { background: var(--tone-b); }

.segmented-action-frame__divider--horizontal.is-active-before .segmented-action-frame__rail--before {
  transform: translateY(var(--frame-motion));
}

.segmented-action-frame__divider--horizontal.is-active-before .segmented-action-frame__endpoint--left-before {
  transform: translate(calc(-1 * var(--start-axis-motion)), var(--frame-motion));
}

.segmented-action-frame__divider--horizontal.is-active-before .segmented-action-frame__endpoint--right-before {
  transform: translate(var(--end-axis-motion), var(--frame-motion));
}

.segmented-action-frame__divider--horizontal.is-active-before .segmented-action-frame__connector--left-before,
.segmented-action-frame__divider--horizontal.is-active-before .segmented-action-frame__connector--right-before {
  opacity: 1;
  transform: translateY(var(--frame-motion));
  transition-delay: 0ms;
}

.segmented-action-frame__divider--horizontal.is-active-before .segmented-action-frame__connector--left-before {
  left: calc(-1 * var(--start-axis-motion));
  width: calc(var(--frame-line-width) + var(--start-axis-motion));
}

.segmented-action-frame__divider--horizontal.is-active-before .segmented-action-frame__connector--right-before {
  right: calc(-1 * var(--end-axis-motion));
  width: calc(var(--frame-line-width) + var(--end-axis-motion));
}

.segmented-action-frame__divider--horizontal.is-active-after .segmented-action-frame__rail--after {
  transform: translateY(calc(-1 * var(--frame-motion)));
}

.segmented-action-frame__divider--horizontal.is-active-after .segmented-action-frame__endpoint--left-after {
  transform: translate(calc(-1 * var(--start-axis-motion)), calc(-1 * var(--frame-motion)));
}

.segmented-action-frame__divider--horizontal.is-active-after .segmented-action-frame__endpoint--right-after {
  transform: translate(var(--end-axis-motion), calc(-1 * var(--frame-motion)));
}

.segmented-action-frame__divider--horizontal.is-active-after .segmented-action-frame__connector--left-after,
.segmented-action-frame__divider--horizontal.is-active-after .segmented-action-frame__connector--right-after {
  opacity: 1;
  transform: translateY(calc(-1 * var(--frame-motion)));
  transition-delay: 0ms;
}

.segmented-action-frame__divider--horizontal.is-active-after .segmented-action-frame__connector--left-after {
  left: calc(-1 * var(--start-axis-motion));
  width: calc(var(--frame-line-width) + var(--start-axis-motion));
}

.segmented-action-frame__divider--horizontal.is-active-after .segmented-action-frame__connector--right-after {
  right: calc(-1 * var(--end-axis-motion));
  width: calc(var(--frame-line-width) + var(--end-axis-motion));
}

.segmented-action-frame__divider--vertical.is-active-before .segmented-action-frame__rail--before { transform: translateX(var(--frame-motion)); }
.segmented-action-frame__divider--vertical.is-active-after .segmented-action-frame__rail--after { transform: translateX(calc(-1 * var(--frame-motion))); }
.segmented-action-frame__divider--vertical.is-active-before .segmented-action-frame__endpoint--left-before { transform: translate(var(--frame-motion), calc(-1 * var(--start-axis-motion))); }
.segmented-action-frame__divider--vertical.is-active-before .segmented-action-frame__endpoint--right-before { transform: translate(var(--frame-motion), var(--end-axis-motion)); }
.segmented-action-frame__divider--vertical.is-active-after .segmented-action-frame__endpoint--left-after { transform: translate(calc(-1 * var(--frame-motion)), calc(-1 * var(--start-axis-motion))); }
.segmented-action-frame__divider--vertical.is-active-after .segmented-action-frame__endpoint--right-after { transform: translate(calc(-1 * var(--frame-motion)), var(--end-axis-motion)); }

.segmented-action-frame__divider--vertical.is-active-before .segmented-action-frame__connector--left-before,
.segmented-action-frame__divider--vertical.is-active-before .segmented-action-frame__connector--right-before { opacity: 1; transform: translateX(var(--frame-motion)); transition-delay: 0ms; }
.segmented-action-frame__divider--vertical.is-active-after .segmented-action-frame__connector--left-after,
.segmented-action-frame__divider--vertical.is-active-after .segmented-action-frame__connector--right-after { opacity: 1; transform: translateX(calc(-1 * var(--frame-motion))); transition-delay: 0ms; }
.segmented-action-frame__divider--vertical.is-active-before .segmented-action-frame__connector--left-before,
.segmented-action-frame__divider--vertical.is-active-after .segmented-action-frame__connector--left-after { top: calc(-1 * var(--start-axis-motion)); height: calc(var(--frame-line-width) + var(--start-axis-motion)); }
.segmented-action-frame__divider--vertical.is-active-before .segmented-action-frame__connector--right-before,
.segmented-action-frame__divider--vertical.is-active-after .segmented-action-frame__connector--right-after { bottom: calc(-1 * var(--end-axis-motion)); height: calc(var(--frame-line-width) + var(--end-axis-motion)); }

.segmented-action-frame__action {
  position: relative;
  z-index: 1;
  --item-motion-ratio: 0;
  display: flex;
  min-width: 0;
  min-height: 0;
  align-items: center;
  justify-content: center;
  gap: 13px;
  padding: 24px 22px;
  border: 0;
  overflow: visible;
  background: transparent;
  color: var(--action-text-color);
  cursor: pointer;
  font: inherit;
  text-align: left;
  transition: color var(--frame-motion-duration) var(--frame-motion-curve);
}

.segmented-action-frame__action.is-top {
  z-index: 2;
  --item-motion-ratio: var(--frame-motion-ratio);
}

.segmented-action-frame__state-layer {
  position: absolute;
  z-index: 0;
  --state-origin-x: var(--frame-motion);
  --state-origin-y: var(--frame-motion);
  inset: calc(-1 * var(--frame-motion));
  overflow: hidden;
  border-radius: 0;
  background: transparent;
  pointer-events: none;
  transition: inset var(--frame-motion-duration) var(--frame-motion-curve), border-radius var(--frame-motion-duration) var(--frame-motion-curve);
}

.segmented-action-frame__state-layer::before {
  position: absolute;
  inset: 0;
  background: var(--action-color);
  content: "";
  opacity: 0;
  transition: opacity var(--frame-motion-duration) var(--frame-motion-curve);
}

.segmented-action-frame__action:not(.is-first-row) .segmented-action-frame__state-layer {
  --state-origin-y: calc(var(--frame-motion) + var(--frame-half-line-width));
  top: calc(-1 * (var(--frame-motion) + var(--frame-half-line-width)));
}

.segmented-action-frame__action:not(.is-last-row) .segmented-action-frame__state-layer {
  bottom: calc(-1 * (var(--frame-motion) + var(--frame-half-line-width)));
}

.segmented-action-frame__action:not(.is-first-column) .segmented-action-frame__state-layer {
  --state-origin-x: calc(var(--frame-motion) + var(--frame-half-line-width));
  left: calc(-1 * (var(--frame-motion) + var(--frame-half-line-width)));
}

.segmented-action-frame__action:not(.is-last-column) .segmented-action-frame__state-layer {
  right: calc(-1 * (var(--frame-motion) + var(--frame-half-line-width)));
}

.segmented-action-frame__action.is-first-row.is-first-column .segmented-action-frame__state-layer { border-top-left-radius: calc(var(--frame-line-radius) + var(--frame-motion)); }
.segmented-action-frame__action.is-first-row.is-last-column .segmented-action-frame__state-layer { border-top-right-radius: calc(var(--frame-line-radius) + var(--frame-motion)); }
.segmented-action-frame__action.is-last-row.is-first-column .segmented-action-frame__state-layer { border-bottom-left-radius: calc(var(--frame-line-radius) + var(--frame-motion)); }
.segmented-action-frame__action.is-last-row.is-last-column .segmented-action-frame__state-layer { border-bottom-right-radius: calc(var(--frame-line-radius) + var(--frame-motion)); }

.segmented-action-frame__action:hover:not(:disabled) .segmented-action-frame__state-layer,
.segmented-action-frame__action:focus-visible .segmented-action-frame__state-layer,
.segmented-action-frame__action.is-active .segmented-action-frame__state-layer {
  opacity: 1;
}
.segmented-action-frame__action:hover:not(:disabled) .segmented-action-frame__state-layer::before,
.segmented-action-frame__action:focus-visible .segmented-action-frame__state-layer::before,
.segmented-action-frame__action.is-active .segmented-action-frame__state-layer::before,
.segmented-action-frame__action.is-pressed .segmented-action-frame__state-layer::before {
  opacity: var(--state-opacity);
}
.segmented-action-frame__icon, .segmented-action-frame__copy { position: relative; z-index: 2; }
.segmented-action-frame__icon { flex: 0 0 auto; color: color-mix(in srgb, var(--action-text-color), var(--action-color) calc(var(--item-motion-ratio) * 100%)); transition: color var(--frame-motion-duration) var(--frame-motion-curve); }
.segmented-action-frame__copy { display: grid; min-width: 0; gap: 3px; }
.segmented-action-frame__copy strong { color: color-mix(in srgb, var(--action-text-color), var(--action-color) calc(var(--item-motion-ratio) * 100%)); font-size: .88rem; font-weight: 800; transition: color var(--frame-motion-duration) var(--frame-motion-curve); }
.segmented-action-frame__copy small { color: color-mix(in srgb, color-mix(in srgb, var(--action-text-color) 70%, var(--text-muted)), var(--action-color) calc(var(--item-motion-ratio) * 72%)); font-size: .72rem; transition: color var(--frame-motion-duration) var(--frame-motion-curve); }
.segmented-action-frame__action:hover:not(:disabled), .segmented-action-frame__action:focus-visible { color: var(--action-color); outline: 0; }
.segmented-action-frame__action:disabled { cursor: default; opacity: .5; }

.segmented-action-frame__ripple {
  position: absolute;
  z-index: 1;
  border-radius: 50%;
  background: color-mix(in srgb, var(--action-color) 28%, transparent);
  pointer-events: none;
  transform: scale(0);
  animation: segmented-action-ripple 460ms cubic-bezier(.2, .75, .2, 1);
}

@keyframes segmented-action-ripple {
  0% { opacity: .58; transform: scale(0); }
  65% { opacity: .22; }
  100% { opacity: 0; transform: scale(1); }
}

@media (prefers-reduced-motion: reduce) {
  .segmented-action-frame__corner,
  .segmented-action-frame__stroke-group,
  .segmented-action-frame__rail,
  .segmented-action-frame__endpoint,
  .segmented-action-frame__connector,
  .segmented-action-frame__action,
  .segmented-action-frame__state-layer,
  .segmented-action-frame__state-layer::before,
  .segmented-action-frame__icon,
  .segmented-action-frame__copy strong,
  .segmented-action-frame__copy small { transition: none; }
  .segmented-action-frame__ripple { display: none; }
}
</style>
