<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from "vue";
import {
  AlertCircle,
  Bell,
  Clock3,
  ExternalLink,
  GitCommit,
  LoaderCircle,
  MapPin,
} from "@lucide/vue";
import { changelog, CURRENT_VERSION, type ChangelogRelease, type ReleaseType } from "../../version/changelog";

const props = withDefaults(defineProps<{ currentVersion?: string }>(), {
  currentVersion: CURRENT_VERSION,
});

const isOpen = ref(false);
const loading = ref(false);
const error = ref(false);
let closeTimer: number | undefined;

const currentRelease = computed(() => changelog.releases.find((release) => release.version === props.currentVersion));
const viewedStorageKey = computed(() => `kotoclip:last-viewed-version:${props.currentVersion}`);
const showDot = ref(readViewedState());

function readViewedState(): boolean {
  try {
    return window.localStorage.getItem(viewedStorageKey.value) !== "viewed";
  } catch {
    return true;
  }
}

function openBadge() {
  if (closeTimer !== undefined) window.clearTimeout(closeTimer);
  isOpen.value = true;
  if (showDot.value) {
    showDot.value = false;
    try {
      window.localStorage.setItem(viewedStorageKey.value, "viewed");
    } catch {
      // 本地存储不可用时不影响更新日志阅读。
    }
  }
}

function scheduleClose() {
  if (closeTimer !== undefined) window.clearTimeout(closeTimer);
  closeTimer = window.setTimeout(() => {
    isOpen.value = false;
    closeTimer = undefined;
  }, 180);
}

function typeClass(type: ReleaseType): string {
  return `version-badge__release--${type}`;
}

function releaseUrl(release: ChangelogRelease): string {
  return release.source.url;
}

onBeforeUnmount(() => {
  if (closeTimer !== undefined) window.clearTimeout(closeTimer);
});
</script>

<template>
  <div
    class="version-badge"
    @mouseenter="openBadge"
    @mouseleave="scheduleClose"
    @focusin="openBadge"
    @focusout="scheduleClose"
  >
    <a
      class="version-badge__trigger"
      :href="currentRelease?.source.url || changelog.repositoryUrl"
      target="_blank"
      rel="noopener noreferrer"
      :aria-label="`Kotoclip ${currentVersion}，打开版本来源`"
    >
      <span>v{{ currentVersion }}</span>
      <ExternalLink class="version-badge__trigger-icon" :size="10" aria-hidden="true" />
      <span v-if="showDot" class="version-badge__dot" aria-hidden="true"></span>
    </a>

    <Transition name="version-popover">
      <section v-if="isOpen" class="version-badge__popover" aria-label="更新日志" @mouseenter="openBadge">
        <header class="version-badge__header">
          <div class="version-badge__heading">
            <Bell :size="14" aria-hidden="true" />
            <strong>更新日志</strong>
          </div>
          <span class="version-badge__date">{{ changelog.lastUpdated }}</span>
        </header>
        <div class="version-badge__body" :aria-busy="loading">
          <div v-if="loading" class="version-badge__state">
            <LoaderCircle :size="15" class="version-badge__spin" aria-hidden="true" />加载日志中…
          </div>
          <div v-else-if="error" class="version-badge__state version-badge__state--error">
            <AlertCircle :size="15" aria-hidden="true" />无法读取更新日志
          </div>
          <div v-else class="version-badge__timeline">
            <div v-for="(release, index) in changelog.releases" :key="release.version" class="version-badge__release">
              <div class="version-badge__rail" aria-hidden="true">
                <span :class="['version-badge__version', typeClass(release.type)]">v{{ release.version }}</span>
                <i v-if="index < changelog.releases.length - 1"></i>
              </div>
              <div class="version-badge__release-content">
                <a :href="releaseUrl(release)" target="_blank" rel="noopener noreferrer" class="version-badge__release-link">
                  <span>{{ release.title }}</span>
                  <ExternalLink :size="11" aria-hidden="true" />
                </a>
                <ul>
                  <li v-for="change in release.changes" :key="change">{{ change }}</li>
                </ul>
                <span class="version-badge__release-meta">
                  <Clock3 :size="10" aria-hidden="true" />{{ release.date }} · {{ release.source.kind === "commit" ? "commit" : "release" }}
                </span>
              </div>
            </div>
          </div>
        </div>
        <footer class="version-badge__footer">
          <span><MapPin :size="11" aria-hidden="true" />当前版本 v{{ changelog.currentVersion }}</span>
          <a :href="`${changelog.repositoryUrl}/commits`" target="_blank" rel="noopener noreferrer">完整记录 <GitCommit :size="11" aria-hidden="true" /></a>
        </footer>
      </section>
    </Transition>
  </div>
</template>

<style scoped>
.version-badge { position: relative; z-index: 20; display: inline-flex; flex: 0 0 auto; align-items: center; }
.version-badge__trigger { position: relative; display: inline-flex; align-items: center; gap: 3px; padding: 2px 6px; border: 1px solid color-mix(in srgb, var(--accent-color) 32%, var(--border-color)); border-radius: 5px; background: var(--accent-light); color: var(--accent-color); font: 700 .66rem/1.4 var(--font-ui); text-decoration: none; transition: color 140ms ease, background 140ms ease, border-color 140ms ease, transform 140ms ease; }
.version-badge__trigger:hover, .version-badge__trigger:focus-visible { border-color: var(--accent-color); background: color-mix(in srgb, var(--accent-color) 13%, transparent); outline: 0; transform: translateY(-1px); }
.version-badge__trigger-icon { opacity: .65; }
.version-badge__dot { position: absolute; top: -4px; right: -4px; width: 8px; height: 8px; border: 1px solid var(--bg-primary); border-radius: 50%; background: var(--novelty-high-text); animation: version-dot-pulse 1.8s ease-in-out infinite; }
.version-badge__popover { position: absolute; top: calc(100% + 10px); left: 0; width: min(370px, calc(100vw - 24px)); overflow: hidden; border: 1px solid var(--border-color); border-radius: 8px; background: color-mix(in srgb, var(--bg-primary) 97%, transparent); box-shadow: var(--shadow-md); backdrop-filter: blur(16px); }
.version-badge__header, .version-badge__footer { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
.version-badge__header { padding: 10px 12px; border-bottom: 1px solid var(--border-color); background: color-mix(in srgb, var(--bg-secondary) 72%, transparent); }
.version-badge__heading, .version-badge__footer span, .version-badge__footer a { display: inline-flex; align-items: center; gap: 5px; }
.version-badge__heading { color: var(--text-primary); font-size: .73rem; }
.version-badge__heading svg { color: var(--accent-color); }
.version-badge__date { color: var(--text-muted); font: .62rem/1 var(--font-ui); font-variant-numeric: tabular-nums; }
.version-badge__body { max-height: 320px; overflow-y: auto; padding: 11px 12px 8px; }
.version-badge__state { display: flex; min-height: 70px; align-items: center; justify-content: center; gap: 7px; color: var(--text-muted); font-size: .7rem; }
.version-badge__state--error { color: var(--novelty-high-text); }
.version-badge__spin { animation: version-spin 1s linear infinite; }
.version-badge__release { display: grid; grid-template-columns: 49px minmax(0, 1fr); gap: 9px; min-height: 66px; }
.version-badge__rail { position: relative; display: flex; flex-direction: column; align-items: center; }
.version-badge__rail i { width: 1px; flex: 1; margin-top: 4px; background: var(--border-color); }
.version-badge__version { position: relative; z-index: 1; padding: 2px 5px; border: 1px solid var(--border-color); border-radius: 4px; background: var(--bg-primary); color: var(--text-secondary); font: 700 .61rem/1.45 var(--font-ui); }
.version-badge__release--feat { border-color: color-mix(in srgb, #2d8a73 34%, var(--border-color)); background: color-mix(in srgb, #2d8a73 10%, var(--bg-primary)); color: #2d806c; }
.version-badge__release--fix { border-color: color-mix(in srgb, var(--accent-color) 34%, var(--border-color)); background: var(--accent-light); color: var(--accent-color); }
.version-badge__release--security { border-color: color-mix(in srgb, var(--novelty-high-text) 34%, var(--border-color)); background: var(--novelty-high-bg); color: var(--novelty-high-text); }
.version-badge__release-content { min-width: 0; padding-bottom: 11px; }
.version-badge__release-link { display: flex; align-items: flex-start; justify-content: space-between; gap: 7px; color: var(--text-primary); font-size: .7rem; font-weight: 650; line-height: 1.45; text-decoration: none; }
.version-badge__release-link:hover, .version-badge__release-link:focus-visible { color: var(--accent-color); outline: 0; }
.version-badge__release-link svg { flex: 0 0 auto; margin-top: 2px; color: var(--accent-color); opacity: .75; }
.version-badge__release-content ul { display: grid; gap: 2px; margin: 4px 0 4px 14px; color: var(--text-secondary); font-size: .65rem; line-height: 1.4; }
.version-badge__release-meta { display: inline-flex; align-items: center; gap: 4px; color: var(--text-muted); font: .59rem/1.3 var(--font-ui); }
.version-badge__footer { padding: 8px 12px; background: var(--text-primary); color: var(--bg-primary); font-size: .62rem; }
.version-badge__footer span { opacity: .76; }
.version-badge__footer a { margin-left: auto; color: #dce8ff; font-weight: 700; text-decoration: none; white-space: nowrap; }
.version-badge__footer a:hover, .version-badge__footer a:focus-visible { color: #fff; outline: 0; text-decoration: underline; text-underline-offset: 2px; }
.version-popover-enter-active, .version-popover-leave-active { transition: opacity 150ms ease, transform 150ms ease; transform-origin: top left; }
.version-popover-enter-from, .version-popover-leave-to { opacity: 0; transform: translateY(-4px) scale(.98); }
@keyframes version-dot-pulse { 0%, 100% { box-shadow: 0 0 0 0 color-mix(in srgb, var(--novelty-high-text) 28%, transparent); } 50% { box-shadow: 0 0 0 4px color-mix(in srgb, var(--novelty-high-text) 0%, transparent); } }
@keyframes version-spin { to { transform: rotate(360deg); } }
@media (max-width: 520px) { .version-badge__popover { position: fixed; top: 58px; left: 12px; width: calc(100vw - 24px); } }
@media (prefers-reduced-motion: reduce) { .version-badge__trigger, .version-popover-enter-active, .version-popover-leave-active { transition: none; } .version-badge__dot, .version-badge__spin { animation: none; } }
</style>
