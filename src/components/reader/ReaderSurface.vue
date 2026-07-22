<script setup lang="ts">
import { computed, onBeforeUnmount, useAttrs, watch } from "vue";
import { X } from "@lucide/vue";
import AppHeader from "../common/AppHeader.vue";

type ReaderSurfaceVariant = "side" | "modal" | "fullscreen";

const props = withDefaults(defineProps<{
  show: boolean;
  variant: ReaderSurfaceVariant;
  title: string;
  description?: string;
  side?: "left" | "right";
  label?: string;
}>(), {
  description: "",
  side: "right",
  label: "",
});

const emit = defineEmits<{
  close: [];
  back: [];
}>();

const attrs = useAttrs();
const transitionName = computed(() => `reader-surface-${props.variant}`);

function isInputElement(target: EventTarget | null): boolean {
  if (!target || !(target instanceof HTMLElement)) return false;
  const tagName = target.tagName.toUpperCase();
  return tagName === "INPUT" || tagName === "TEXTAREA" || tagName === "SELECT" || target.isContentEditable;
}

function handleBack() {
  if (attrs.onBack) {
    emit("back");
  } else {
    close();
  }
}

function close() {
  emit("close");
}

function handleKeydown(event: KeyboardEvent) {
  if (!props.show) return;

  // 1. Esc 键
  if (event.key === "Escape") {
    event.preventDefault();
    handleBack();
    return;
  }

  // 2. Backspace 键 (非输入框焦点时触发返回)
  if (event.key === "Backspace" && !isInputElement(event.target)) {
    event.preventDefault();
    handleBack();
    return;
  }

  // 3. Alt + Left 方向键 (标准返回快捷键)
  if (event.altKey && event.key === "ArrowLeft") {
    event.preventDefault();
    handleBack();
    return;
  }
}

watch(
  () => props.show,
  (show) => {
    if (show) document.addEventListener("keydown", handleKeydown);
    else document.removeEventListener("keydown", handleKeydown);
  },
  { immediate: true },
);

onBeforeUnmount(() => document.removeEventListener("keydown", handleKeydown));
</script>

<template>
  <Transition :name="transitionName">
    <div
      v-if="show"
      class="reader-surface"
      :class="[
        `reader-surface--${variant}`,
        variant === 'side' && `reader-surface--${side}`,
      ]"
      @click.self="variant !== 'fullscreen' && handleBack()"
    >
      <section
        class="reader-surface__panel"
        :role="variant === 'fullscreen' ? undefined : 'dialog'"
        :aria-modal="variant === 'fullscreen' ? undefined : true"
        :aria-label="label || title"
      >
        <AppHeader
          :show-back="variant === 'fullscreen' || Boolean(attrs.onBack)"
          collapse-brand
          :title="title"
          :description="variant === 'fullscreen' ? description : ''"
          @back="handleBack"
        >
          <template #actions>
            <slot name="actions" />
            <button
              v-if="variant !== 'fullscreen'"
              class="reader-surface__close"
              type="button"
              :title="`关闭${title} (Esc)`"
              :aria-label="`关闭${title}`"
              @click="handleBack"
            >
              <X :size="18" aria-hidden="true" />
            </button>
          </template>
        </AppHeader>
        <div class="reader-surface__body">
          <slot />
        </div>
      </section>
    </div>
  </Transition>
</template>

<style scoped>
.reader-surface {
  position: fixed;
  z-index: 1300;
  inset: 0;
  min-width: 0;
}

.reader-surface--side,
.reader-surface--modal {
  background: color-mix(in srgb, var(--text-primary) 12%, transparent);
}

.reader-surface__panel {
  display: flex;
  min-width: 0;
  min-height: 0;
  flex-direction: column;
  overflow: hidden;
  border: 1px solid var(--border-color);
  background: var(--bg-primary);
  box-shadow: 0 18px 50px color-mix(in srgb, var(--text-primary) 16%, transparent);
  will-change: transform, opacity;
}

.reader-surface__close {
  display: grid;
  width: 32px;
  height: 32px;
  flex: 0 0 auto;
  place-items: center;
  border: 0;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
}

.reader-surface__close:hover,
.reader-surface__close:focus-visible {
  outline: 0;
  background: var(--accent-light);
  color: var(--accent-color);
}

.reader-surface__body {
  display: flex;
  min-width: 0;
  min-height: 0;
  flex: 1 1 auto;
  flex-direction: column;
  overflow: hidden;
}

.reader-surface--side .reader-surface__panel {
  position: absolute;
  top: 70px;
  bottom: 18px;
  width: min(360px, calc(100vw - 24px));
  border-radius: var(--radius-sm);
}

.reader-surface--left .reader-surface__panel { left: 12px; }
.reader-surface--right .reader-surface__panel { right: 12px; }

.reader-surface--modal .reader-surface__panel {
  position: absolute;
  top: 70px;
  right: 12px;
  width: min(420px, calc(100vw - 24px));
  max-height: calc(100vh - 88px);
  border-radius: var(--radius-sm);
  transform-origin: top right;
}

.reader-surface--fullscreen .reader-surface__panel {
  width: 100%;
  height: 100%;
  border: 0;
  border-radius: 0;
  box-shadow: none;
}

.reader-surface--fullscreen .app-header {
  min-height: 58px;
  padding: 8px 24px;
}

.reader-surface--fullscreen .reader-surface__body {
  overflow: hidden;
}

.reader-surface-side-enter-active,
.reader-surface-side-leave-active,
.reader-surface-modal-enter-active,
.reader-surface-modal-leave-active,
.reader-surface-fullscreen-enter-active,
.reader-surface-fullscreen-leave-active {
  transition: opacity 150ms ease;
}

.reader-surface-side-enter-active .reader-surface__panel,
.reader-surface-side-leave-active .reader-surface__panel,
.reader-surface-modal-enter-active .reader-surface__panel,
.reader-surface-modal-leave-active .reader-surface__panel,
.reader-surface-fullscreen-enter-active .reader-surface__panel,
.reader-surface-fullscreen-leave-active .reader-surface__panel {
  transition: transform 180ms cubic-bezier(.2, 0, 0, 1), opacity 150ms ease;
}

.reader-surface-side-enter-from,
.reader-surface-side-leave-to,
.reader-surface-modal-enter-from,
.reader-surface-modal-leave-to,
.reader-surface-fullscreen-enter-from,
.reader-surface-fullscreen-leave-to {
  opacity: 0;
}

.reader-surface--right.reader-surface-side-enter-from .reader-surface__panel,
.reader-surface--right.reader-surface-side-leave-to .reader-surface__panel {
  transform: translateX(24px);
}

.reader-surface--left.reader-surface-side-enter-from .reader-surface__panel,
.reader-surface--left.reader-surface-side-leave-to .reader-surface__panel {
  transform: translateX(-24px);
}

.reader-surface-modal-enter-from .reader-surface__panel,
.reader-surface-modal-leave-to .reader-surface__panel {
  transform: translate(10px, -8px) scale(.98);
}

.reader-surface-fullscreen-enter-from .reader-surface__panel,
.reader-surface-fullscreen-leave-to .reader-surface__panel {
  transform: translateX(16px);
}

@media (prefers-reduced-motion: reduce) {
  .reader-surface,
  .reader-surface__panel {
    transition-duration: 1ms !important;
  }

  .reader-surface__panel {
    transform: none !important;
  }
}

@media (max-width: 600px) {
  .reader-surface--side .reader-surface__panel,
  .reader-surface--modal .reader-surface__panel {
    top: 66px;
    right: 8px;
    bottom: 8px;
    left: 8px;
    width: auto;
    max-height: none;
  }

  .reader-surface--fullscreen .app-header {
    padding-right: 12px;
    padding-left: 12px;
  }
}
</style>
