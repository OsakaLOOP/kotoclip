<script setup lang="ts">
import { ArrowLeft, BadgeInfo, BookOpen, X } from "@lucide/vue";
import { ref } from "vue";
import { CURRENT_VERSION } from "../../version/changelog";

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
const showAbout = ref(false);
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
      <button class="app-header__about" type="button" title="关于 Kotoclip" aria-label="关于 Kotoclip" @click="showAbout = true">
        <BadgeInfo :size="18" stroke-width="1.8" aria-hidden="true" />
      </button>
    </div>
  </header>
  <Teleport to="body">
    <div v-if="showAbout" class="about-dialog-backdrop" role="presentation" @click.self="showAbout = false" @keydown.esc="showAbout = false">
      <section class="about-dialog" role="dialog" aria-modal="true" aria-labelledby="about-dialog-title">
        <header class="about-dialog__header">
          <div>
            <h2 id="about-dialog-title">关于 Kotoclip</h2>
            <p>版本 {{ CURRENT_VERSION }}</p>
          </div>
          <button class="about-dialog__close" type="button" title="关闭" aria-label="关闭关于窗口" @click="showAbout = false">
            <X :size="18" aria-hidden="true" />
          </button>
        </header>
        <div class="about-dialog__body">
          <p>Kotoclip 是面向日语原文阅读与语言分析的本地桌面应用。</p>
          <h3>许可证</h3>
          <p>项目代码采用 MIT License。许可证文本见发行包中的 <code>LICENSE</code> 文件。</p>
          <h3>致谢</h3>
          <ul>
            <li><a href="https://clrd.ninjal.ac.jp/unidic/" target="_blank" rel="noreferrer">UniDic</a>：感谢国立国语研究所及相关维护团队提供日语词法资源。UniDic 资源遵循其官方发布的 GPL v2.0、LGPL v2.1 与 BSD 许可条件。</li>
            <li><a href="https://github.com/megagonlabs/ginza" target="_blank" rel="noreferrer">GiNZA</a>：感谢 Megagon Labs 提供基于 spaCy 与 Sudachi 的日语依存分析工具，GiNZA 项目采用 MIT License。</li>
            <li><a href="https://forum.freemdict.com/" target="_blank" rel="noreferrer">FreeMdict 论坛</a>：感谢 MDict 词典制作与交流社区提供的资料整理和技术讨论。</li>
          </ul>
          <p class="about-dialog__note">第三方程序、模型、词典资源及其派生文件遵循各自的许可和分发条件，项目 MIT License 不覆盖这些内容。</p>
        </div>
      </section>
    </div>
  </Teleport>
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
  gap: 4px;
  overflow: visible;
}

.app-header__about,
.about-dialog__close {
  display: grid;
  width: 32px;
  height: 32px;
  flex: 0 0 auto;
  place-items: center;
  border: 0;
  border-radius: 50%;
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
}

.app-header__about:hover,
.app-header__about:focus-visible,
.about-dialog__close:hover,
.about-dialog__close:focus-visible {
  outline: 0;
  background: var(--accent-light);
  color: var(--accent-color);
}

.about-dialog-backdrop {
  position: fixed;
  z-index: 100;
  inset: 0;
  display: grid;
  place-items: center;
  padding: 20px;
  background: color-mix(in srgb, #111 38%, transparent);
}

.about-dialog {
  width: min(560px, 100%);
  max-height: min(720px, calc(100vh - 40px));
  overflow: hidden;
  border: 1px solid var(--border-color);
  border-radius: 12px;
  background: var(--bg-primary);
  box-shadow: 0 18px 60px color-mix(in srgb, #000 22%, transparent);
}

.about-dialog__header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  padding: 18px 20px 14px;
  border-bottom: 1px solid var(--border-color);
}

.about-dialog__header h2,
.about-dialog__header p,
.about-dialog__body h3,
.about-dialog__body p {
  margin: 0;
}

.about-dialog__header h2 { color: var(--text-primary); font-size: 1rem; }
.about-dialog__header p { margin-top: 4px; color: var(--text-muted); font-size: .72rem; }
.about-dialog__body { max-height: calc(min(720px, 100vh - 40px) - 82px); overflow: auto; padding: 18px 20px 20px; color: var(--text-secondary); font-size: .8rem; line-height: 1.65; }
.about-dialog__body h3 { margin-top: 18px; margin-bottom: 6px; color: var(--text-primary); font-size: .82rem; }
.about-dialog__body ul { display: grid; gap: 9px; margin: 8px 0 0; padding-left: 20px; }
.about-dialog__body a { color: var(--accent-color); }
.about-dialog__body code { padding: 1px 4px; border-radius: 3px; background: var(--bg-secondary); font-size: .75rem; }
.about-dialog__note { margin-top: 18px !important; color: var(--text-muted); font-size: .72rem; }

@media (max-width: 520px) {
  .about-dialog-backdrop { padding: 10px; }
  .about-dialog { max-height: calc(100vh - 20px); }
  .about-dialog__body { max-height: calc(100vh - 102px); }
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
