<script setup lang="ts">
import { ArrowLeft, BookOpen } from "@lucide/vue";

withDefaults(defineProps<{
  showBack?: boolean;
  backLabel?: string;
  collapseBrand?: boolean;
  title?: string;
  description?: string;
}>(), {
  showBack: false,
  backLabel: "返回",
  collapseBrand: false,
  title: "",
  description: "",
});

const emit = defineEmits<{ back: [] }>();
</script>

<template>
  <header
    class="app-header"
    :class="{
      'app-header--collapsible': collapseBrand,
      'app-header--has-title': Boolean(title),
    }"
  >
    <div class="app-header__identity">
      <button
        v-if="showBack"
        class="app-header__back"
        type="button"
        :title="backLabel"
        :aria-label="backLabel"
        @click="emit('back')"
      >
        <ArrowLeft :size="19" aria-hidden="true" />
      </button>
      <BookOpen class="app-header__brand-icon" :size="24" stroke-width="1.8" aria-hidden="true" />
      <svg
        class="app-header__brand-name"
        viewBox="275 65 507 130"
        role="img"
        aria-label="Kotoclip"
        focusable="false"
      >
        <g fill="none" stroke-linecap="round" stroke-linejoin="round" stroke-width="14">
          <g stroke="currentColor">
            <path d="M282 72v88m0-28 40-36m-21 20 27 44" />
            <circle cx="368" cy="128" r="32" />
            <path d="M425 78v67q0 15 15 15m-33-57h39" />
            <circle cx="489" cy="128" r="32" />
          </g>
          <g stroke="#39c5bb">
            <path d="M580 105c-8-8-18-12-29-12-21 0-35 15-35 35s14 35 35 35c11 0 21-4 29-12" />
            <path d="M619 72v73q0 15 15 15" />
            <path d="M665 103v57" />
            <path d="M713 101v87m0-77c9-12 22-17 34-15 18 3 28 16 28 32 0 18-12 32-30 32-13 0-24-6-32-17" />
          </g>
          <circle cx="665" cy="78" r="7" fill="#f5d547" stroke="none" />
        </g>
      </svg>
      <div v-if="title" class="app-header__page-identity">
        <strong>{{ title }}</strong>
        <span v-if="description">{{ description }}</span>
      </div>
      <span v-else-if="description" class="app-header__brand-description">{{ description }}</span>
      <slot name="version" />
    </div>
    <div class="app-header__actions">
      <slot name="actions" />
    </div>
  </header>
</template>

<style scoped>
.app-header {
  z-index: 10;
  display: flex;
  min-height: 58px;
  flex: 0 0 auto;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 12px 24px;
  border-bottom: 1px solid var(--border-color);
  background: var(--glass-bg);
  backdrop-filter: var(--glass-filter);
}

.app-header__identity {
  display: flex;
  min-width: 0;
  flex: 1 1 auto;
  align-items: center;
  gap: 8px;
  overflow: visible;
}

.app-header__back {
  display: grid;
  width: 32px;
  height: 32px;
  flex: 0 0 auto;
  place-items: center;
  border: 0;
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
}

.app-header__back:hover,
.app-header__back:focus-visible {
  outline: 0;
  color: var(--accent-color);
}

.app-header__brand-icon {
  flex: 0 0 auto;
  color: var(--accent-color);
}

.app-header__brand-name {
  display: block;
  flex: 0 0 auto;
  height: 22px;
  width: auto;
  color: var(--accent-color);
}

.app-header__brand-description {
  min-width: 0;
  overflow: hidden;
  padding-left: 8px;
  border-left: 1px solid var(--border-color);
  color: var(--text-muted);
  font-size: .75rem;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.app-header__page-identity {
  display: flex;
  min-width: 0;
  max-width: min(46vw, 760px);
  flex-direction: column;
  padding-left: 10px;
  border-left: 1px solid var(--border-color);
  line-height: 1.25;
}

.app-header__page-identity strong,
.app-header__page-identity span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.app-header__page-identity strong {
  color: var(--text-primary);
  font-size: .86rem;
}

.app-header__page-identity span {
  color: var(--text-muted);
  font-size: .72rem;
}

.app-header__actions {
  display: flex;
  min-width: 0;
  flex: 0 1 auto;
  align-items: center;
  overflow: hidden;
}

@media (max-width: 820px) {
  .app-header {
    padding-right: 12px;
    padding-left: 12px;
  }

  .app-header--collapsible .app-header__brand-name {
    display: none;
  }

  .app-header--collapsible .app-header__page-identity {
    max-width: 34vw;
  }
}
</style>
