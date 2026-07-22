<script setup lang="ts">
import { computed, onBeforeUnmount, ref, type Component, type CSSProperties } from "vue";

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
  hoverExpansion?: number;
  maxStateOpacity?: number;
  ariaLabel?: string;
}>(), {
  width: "100%",
  minHeight: "328px",
  hoverExpansion: 6,
  maxStateOpacity: .075,
  ariaLabel: "可选操作",
});

const emit = defineEmits<{
  select: [id: string];
}>();

const visibleActions = computed(() => props.actions.slice(0, 4));
const activeIndex = ref<number | null>(null);
const pressedIndex = ref<number | null>(null);
const expansion = ref(0);
const motionDuration = ref(180);
const releaseLocked = ref(false);
const motionRatio = computed(() => {
  if (props.hoverExpansion <= 0) return 0;
  return Math.min(1, Math.max(0, expansion.value / props.hoverExpansion));
});
const stateOpacity = computed(() => {
  return motionRatio.value * props.maxStateOpacity;
});
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
let pressStartedAt = 0;
let pointerInside = false;

const PRESS_EXPANSION = 3;
const PRESS_DURATION = 85;
const RELEASE_DURATION = 420;

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

function isVisuallyActive(index: number): boolean {
  return activeIndex.value === index && !releaseLocked.value;
}

function updatePointerExpansion(event: PointerEvent, index: number) {
  if (pressedIndex.value !== null || releaseLocked.value) return;
  const bounds = (event.currentTarget as HTMLElement).getBoundingClientRect();
  const dx = (event.clientX - bounds.left - bounds.width / 2) / Math.max(1, bounds.width / 2);
  const dy = (event.clientY - bounds.top - bounds.height / 2) / Math.max(1, bounds.height / 2);
  const centerProximity = Math.max(0, 1 - Math.hypot(dx, dy));
  activeIndex.value = index;
  motionDuration.value = 90;
  expansion.value = props.hoverExpansion * Math.pow(centerProximity, .72);
}

function handlePointerEnter(event: PointerEvent, index: number) {
  pointerInside = true;
  if (settleTimer !== undefined) {
    window.clearTimeout(settleTimer);
    settleTimer = undefined;
  }
  releaseLocked.value = false;
  updatePointerExpansion(event, index);
}

function handlePointerLeave() {
  pointerInside = false;
  if (pressedIndex.value !== null) {
    releasePress();
    return;
  }
  if (releaseLocked.value) {
    scheduleSettle();
    return;
  }
  activeIndex.value = null;
  motionDuration.value = 220;
  expansion.value = 0;
}

function handlePointerDown(event: PointerEvent, index: number) {
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
  motionDuration.value = PRESS_DURATION;
  expansion.value += PRESS_EXPANSION;
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
  motionDuration.value = RELEASE_DURATION;
  expansion.value = 0;
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
  activeIndex.value = index;
  releaseLocked.value = false;
  motionDuration.value = 180;
  expansion.value = props.hoverExpansion;
}

function handleBlur() {
  activeIndex.value = null;
  pressedIndex.value = null;
  releaseLocked.value = false;
  motionDuration.value = 220;
  expansion.value = 0;
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
  if (rippleTimer !== undefined) window.clearTimeout(rippleTimer);
  if (pressReleaseTimer !== undefined) window.clearTimeout(pressReleaseTimer);
  if (settleTimer !== undefined) window.clearTimeout(settleTimer);
});
</script>

<template>
  <section
    class="segmented-action-frame"
    :style="{
      '--frame-height': minHeight,
      '--frame-width': width,
      '--frame-rows': visibleActions.length,
      '--frame-motion': `${expansion}px`,
      '--frame-motion-ratio': motionRatio,
      '--frame-motion-duration': `${motionDuration}ms`,
      '--state-opacity': stateOpacity,
      '--corner-top': themeAt(0).color,
      '--corner-bottom': themeAt(visibleActions.length - 1).color,
    }"
    :aria-label="ariaLabel"
  >
    <span class="segmented-action-frame__corner segmented-action-frame__corner--tl" :class="{ 'is-active': isVisuallyActive(0), 'is-top': activeIndex === 0 }" aria-hidden="true"></span>
    <span class="segmented-action-frame__corner segmented-action-frame__corner--tr" :class="{ 'is-active': isVisuallyActive(0), 'is-top': activeIndex === 0 }" aria-hidden="true"></span>
    <span class="segmented-action-frame__corner segmented-action-frame__corner--bl" :class="{ 'is-active': isVisuallyActive(visibleActions.length - 1), 'is-top': activeIndex === visibleActions.length - 1 }" aria-hidden="true"></span>
    <span class="segmented-action-frame__corner segmented-action-frame__corner--br" :class="{ 'is-active': isVisuallyActive(visibleActions.length - 1), 'is-top': activeIndex === visibleActions.length - 1 }" aria-hidden="true"></span>
    <span
      v-for="(_, index) in visibleActions.slice(0, -1)"
      :key="`divider-${index}`"
      class="segmented-action-frame__divider"
      :class="{
        'is-active-before': isVisuallyActive(index),
        'is-active-after': isVisuallyActive(index + 1),
        'is-top-before': activeIndex === index,
        'is-top-after': activeIndex === index + 1,
      }"
      :style="dividerStyle(index)"
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
        'is-first': index === 0,
        'is-last': index === visibleActions.length - 1,
        'is-only': visibleActions.length === 1,
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
}

.segmented-action-frame__corner {
  position: absolute;
  z-index: 3;
  width: var(--frame-corner-length);
  height: var(--frame-corner-length);
  border-color: var(--corner-top);
  border-style: solid;
  border-width: var(--frame-line-width);
  color: var(--corner-top);
  pointer-events: none;
  transition: filter var(--frame-motion-duration) var(--frame-motion-curve), transform var(--frame-motion-duration) var(--frame-motion-curve);
}

.segmented-action-frame__corner.is-top { z-index: 5; }

.segmented-action-frame__corner--tl { top: 0; left: 0; border-right: 0; border-bottom: 0; border-top-left-radius: var(--frame-line-radius); }
.segmented-action-frame__corner--tr { top: 0; right: 0; border-bottom: 0; border-left: 0; border-top-right-radius: var(--frame-line-radius); }
.segmented-action-frame__corner--bl { bottom: 0; left: 0; border-color: var(--corner-bottom); color: var(--corner-bottom); border-top: 0; border-right: 0; border-bottom-left-radius: var(--frame-line-radius); }
.segmented-action-frame__corner--br { right: 0; bottom: 0; border-color: var(--corner-bottom); color: var(--corner-bottom); border-top: 0; border-left: 0; border-bottom-right-radius: var(--frame-line-radius); }
.segmented-action-frame__corner--tl.is-active { transform: translate(calc(-1 * var(--frame-motion)), calc(-1 * var(--frame-motion))); }
.segmented-action-frame__corner--tr.is-active { transform: translate(var(--frame-motion), calc(-1 * var(--frame-motion))); }
.segmented-action-frame__corner--bl.is-active { transform: translate(calc(-1 * var(--frame-motion)), var(--frame-motion)); }
.segmented-action-frame__corner--br.is-active { transform: translate(var(--frame-motion), var(--frame-motion)); }
.segmented-action-frame__corner.is-active { filter: brightness(calc(1 + var(--frame-motion-ratio) * .08)) drop-shadow(0 0 7px color-mix(in srgb, currentColor calc(var(--frame-motion-ratio) * 34%), transparent)); }

.segmented-action-frame__divider {
  position: absolute;
  z-index: 3;
  top: var(--split);
  right: 0;
  left: 0;
  height: var(--frame-line-width);
  background: transparent;
  isolation: isolate;
  pointer-events: none;
  transform: translateY(-50%);
}

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
  inset: 0 var(--frame-line-width);
  transition: filter var(--frame-motion-duration) var(--frame-motion-curve), transform var(--frame-motion-duration) var(--frame-motion-curve);
}

.segmented-action-frame__rail--before {
  background: repeating-linear-gradient(90deg, var(--tone-a) 0 18px, transparent 18px 36px);
}

.segmented-action-frame__rail--after {
  background: repeating-linear-gradient(90deg, transparent 0 18px, var(--tone-b) 18px 36px);
}

.segmented-action-frame__endpoint {
  position: absolute;
  width: var(--frame-line-width);
  height: var(--frame-endpoint-length);
  z-index: 3;
  transition: filter var(--frame-motion-duration) var(--frame-motion-curve), transform var(--frame-motion-duration) var(--frame-motion-curve);
}

.segmented-action-frame__connector {
  position: absolute;
  z-index: 2;
  top: 0;
  width: var(--frame-line-width);
  height: var(--frame-line-width);
  opacity: 0;
  transition:
    left var(--frame-motion-duration) var(--frame-motion-curve),
    right var(--frame-motion-duration) var(--frame-motion-curve),
    transform var(--frame-motion-duration) var(--frame-motion-curve),
    width var(--frame-motion-duration) var(--frame-motion-curve),
    opacity 0ms linear var(--frame-motion-duration);
}

.segmented-action-frame__endpoint--left-before,
.segmented-action-frame__endpoint--right-before { bottom: 50%; background: var(--tone-a); border-radius: var(--frame-line-radius) var(--frame-line-radius) 0 0; }
.segmented-action-frame__endpoint--left-after,
.segmented-action-frame__endpoint--right-after { top: 50%; background: var(--tone-b); border-radius: 0 0 var(--frame-line-radius) var(--frame-line-radius); }
.segmented-action-frame__endpoint--left-before,
.segmented-action-frame__endpoint--left-after { left: 0; }
.segmented-action-frame__endpoint--right-before,
.segmented-action-frame__endpoint--right-after { right: 0; }
.segmented-action-frame__connector--left-before,
.segmented-action-frame__connector--left-after { left: 0; }
.segmented-action-frame__connector--right-before,
.segmented-action-frame__connector--right-after { right: 0; }
.segmented-action-frame__connector--left-before,
.segmented-action-frame__connector--right-before { background: var(--tone-a); }
.segmented-action-frame__connector--left-after,
.segmented-action-frame__connector--right-after { background: var(--tone-b); }

.segmented-action-frame__divider.is-active-before .segmented-action-frame__rail--before {
  transform: translateY(var(--frame-motion));
}

.segmented-action-frame__divider.is-active-before .segmented-action-frame__endpoint--left-before {
  transform: translate(calc(-1 * var(--frame-motion)), var(--frame-motion));
}

.segmented-action-frame__divider.is-active-before .segmented-action-frame__endpoint--right-before {
  transform: translate(var(--frame-motion), var(--frame-motion));
}

.segmented-action-frame__divider.is-active-before .segmented-action-frame__connector--left-before,
.segmented-action-frame__divider.is-active-before .segmented-action-frame__connector--right-before {
  width: calc(var(--frame-line-width) + var(--frame-motion));
  opacity: 1;
  transform: translateY(var(--frame-motion));
  transition-delay: 0ms;
}

.segmented-action-frame__divider.is-active-before .segmented-action-frame__connector--left-before {
  left: calc(-1 * var(--frame-motion));
}

.segmented-action-frame__divider.is-active-before .segmented-action-frame__connector--right-before {
  right: calc(-1 * var(--frame-motion));
}

.segmented-action-frame__divider.is-active-after .segmented-action-frame__rail--after {
  transform: translateY(calc(-1 * var(--frame-motion)));
}

.segmented-action-frame__divider.is-active-after .segmented-action-frame__endpoint--left-after {
  transform: translate(calc(-1 * var(--frame-motion)), calc(-1 * var(--frame-motion)));
}

.segmented-action-frame__divider.is-active-after .segmented-action-frame__endpoint--right-after {
  transform: translate(var(--frame-motion), calc(-1 * var(--frame-motion)));
}

.segmented-action-frame__divider.is-active-after .segmented-action-frame__connector--left-after,
.segmented-action-frame__divider.is-active-after .segmented-action-frame__connector--right-after {
  width: calc(var(--frame-line-width) + var(--frame-motion));
  opacity: 1;
  transform: translateY(calc(-1 * var(--frame-motion)));
  transition-delay: 0ms;
}

.segmented-action-frame__divider.is-active-after .segmented-action-frame__connector--left-after {
  left: calc(-1 * var(--frame-motion));
}

.segmented-action-frame__divider.is-active-after .segmented-action-frame__connector--right-after {
  right: calc(-1 * var(--frame-motion));
}

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

.segmented-action-frame__action:not(.is-first) .segmented-action-frame__state-layer {
  --state-origin-y: calc(var(--frame-motion) + var(--frame-half-line-width));
  top: calc(-1 * (var(--frame-motion) + var(--frame-half-line-width)));
}

.segmented-action-frame__action:not(.is-last) .segmented-action-frame__state-layer {
  bottom: calc(-1 * (var(--frame-motion) + var(--frame-half-line-width)));
}

.segmented-action-frame__action.is-first .segmented-action-frame__state-layer {
  border-radius: calc(var(--frame-line-radius) + var(--frame-motion)) calc(var(--frame-line-radius) + var(--frame-motion)) 0 0;
}

.segmented-action-frame__action.is-last .segmented-action-frame__state-layer {
  border-radius: 0 0 calc(var(--frame-line-radius) + var(--frame-motion)) calc(var(--frame-line-radius) + var(--frame-motion));
}

.segmented-action-frame__action.is-only .segmented-action-frame__state-layer {
  border-radius: calc(var(--frame-line-radius) + var(--frame-motion));
}

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
  .segmented-action-frame__action { transition: none; }
  .segmented-action-frame__ripple { display: none; }
}
</style>
