<script setup lang="ts">
import { computed, onBeforeUnmount, watch } from "vue";
import { ArrowLeft, BookOpen, X } from "@lucide/vue";

type ReaderSurfaceVariant = "side" | "modal" | "fullscreen";

const props = withDefaults(defineProps<{
  show: boolean;
  variant: ReaderSurfaceVariant;
  title: string;
  side?: "left" | "right";
  label?: string;
}>(), {
  side: "right",
  label: "",
});

const emit = defineEmits<{ close: [] }>();
const transitionName = computed(() => `reader-surface-${props.variant}`);

function close() {
  emit("close");
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === "Escape" && props.show) close();
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
      @click.self="variant !== 'fullscreen' && close()"
    >
      <section
        class="reader-surface__panel"
        :role="variant === 'fullscreen' ? undefined : 'dialog'"
        :aria-modal="variant === 'fullscreen' ? undefined : true"
        :aria-label="label || title"
      >
        <header class="reader-surface__header">
          <button
            v-if="variant === 'fullscreen'"
            class="reader-surface__back"
            type="button"
            :title="`返回阅读界面`"
            :aria-label="`返回阅读界面`"
            @click="close"
          >
            <ArrowLeft :size="19" aria-hidden="true" />
          </button>
          <div
            class="reader-surface__heading"
            :class="{ 'reader-surface__heading--brand': variant === 'fullscreen' }"
          >
            <template v-if="variant === 'fullscreen'">
              <BookOpen class="reader-surface__brand-icon" :size="24" stroke-width="1.8" aria-hidden="true" />
              <strong class="reader-surface__brand-name">Kotoclip</strong>
              <span class="reader-surface__section-name">{{ title }}</span>
            </template>
            <slot v-else name="title">
              <strong>{{ title }}</strong>
            </slot>
          </div>
          <div class="reader-surface__actions">
            <slot name="actions" />
            <button
              v-if="variant !== 'fullscreen'"
              class="reader-surface__close"
              type="button"
              :title="`关闭${title}`"
              :aria-label="`关闭${title}`"
              @click="close"
            >
              <X :size="18" aria-hidden="true" />
            </button>
          </div>
        </header>
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

.reader-surface__header {
  display: flex;
  min-height: 58px;
  flex: 0 0 auto;
  align-items: center;
  gap: 10px;
  padding: 10px 14px;
  border-bottom: 1px solid var(--border-color);
  background: color-mix(in srgb, var(--bg-primary) 88%, transparent);
  backdrop-filter: var(--glass-filter);
}

.reader-surface__heading {
  display: grid;
  min-width: 0;
  flex: 1 1 auto;
  gap: 1px;
}

.reader-surface__heading strong {
  overflow: hidden;
  color: var(--text-primary);
  font-size: .9rem;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.reader-surface__heading span {
  overflow: hidden;
  color: var(--text-muted);
  font-size: .7rem;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.reader-surface__heading--brand {
  display: flex;
  align-items: center;
  gap: 8px;
  overflow: hidden;
}

.reader-surface__brand-icon {
  flex: 0 0 auto;
  color: var(--accent-color);
}

.reader-surface__heading .reader-surface__brand-name {
  flex: 0 0 auto;
  color: var(--accent-color);
  font-size: 1.25rem;
  font-weight: 700;
}

.reader-surface__heading .reader-surface__section-name {
  min-width: 0;
  padding-left: 8px;
  border-left: 1px solid var(--border-color);
  color: var(--text-muted);
  font-size: .75rem;
  text-overflow: ellipsis;
}

.reader-surface__actions {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  gap: 4px;
}

.reader-surface__back,
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

.reader-surface__back:hover,
.reader-surface__close:hover,
.reader-surface__back:focus-visible,
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

.reader-surface--fullscreen .reader-surface__header {
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

  .reader-surface--fullscreen .reader-surface__header {
    padding-right: 12px;
    padding-left: 12px;
  }
}
</style>
