<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { ArrowLeft, ArrowRight, Check, X } from "@lucide/vue";
import WelcomeArt from "./WelcomeArt.vue";
import ReadingAscentArt from "./ReadingAscentArt.vue";
import ContentOrbitArt from "./ContentOrbitArt.vue";
import LearningSystemArt from "./LearningSystemArt.vue";

type SlideId = "welcome" | "ascent" | "interest" | "system";

interface Slide {
  id: SlideId;
  eyebrow: string;
  title: string;
  lead: string;
}

const emit = defineEmits<{
  complete: [];
}>();

const slides: Slide[] = [
  {
    id: "welcome",
    eyebrow: "KOTOCLIP · V1.0",
    title: "欢迎来到 KotoClip",
    lead: "你的日语整本书阅读智能助手",
  },
  {
    id: "ascent",
    eyebrow: "01 · 读懂整本",
    title: "水平还没到，也可以开始读整本书",
    lead: "把句子拆清楚，把解释放回语境，让阅读继续往前。",
  },
  {
    id: "interest",
    eyebrow: "02 · 兴趣激励",
    title: "奖励自己，也让喜欢的内容带你继续读",
    lead: "经典名著、网络文案、视频字幕与 ACGN 作品，都能进入同一套阅读流程。",
  },
  {
    id: "system",
    eyebrow: "03 · 建立体系",
    title: "读过的内容，逐渐连成自己的日语体系",
    lead: "分级、文法、卡片与复习提示互相衔接，把每次读懂都留下来。",
  },
];

const activeIndex = ref(0);
const direction = ref<1 | -1>(1);
const rewardPrompting = ref(false);
const current = computed(() => slides[activeIndex.value]);
const transitionName = computed(() => direction.value > 0 ? "slide-forward" : "slide-back");

function goTo(index: number) {
  if (index < 0 || index >= slides.length || index === activeIndex.value) return;
  direction.value = index > activeIndex.value ? 1 : -1;
  activeIndex.value = index;
  rewardPrompting.value = false;
}

function next() {
  if (current.value.id === "ascent") {
    rewardPrompting.value = false;
    requestAnimationFrame(() => {
      rewardPrompting.value = true;
    });
    return;
  }
  if (current.value.id === "system") {
    emit("complete");
    return;
  }
  goTo(activeIndex.value + 1);
}

function claimReward() {
  rewardPrompting.value = false;
  goTo(2);
}

function handleKeydown(event: KeyboardEvent) {
  const target = event.target as HTMLElement | null;
  if (target?.closest("button, a, input, textarea, select")) return;
  if (event.key === "ArrowLeft" && activeIndex.value > 0) {
    event.preventDefault();
    goTo(activeIndex.value - 1);
  }
  if (event.key === "ArrowRight" || event.key === "Enter") {
    event.preventDefault();
    next();
  }
}

onMounted(() => window.addEventListener("keydown", handleKeydown));
onBeforeUnmount(() => window.removeEventListener("keydown", handleKeydown));
</script>

<template>
  <main class="onboarding" :data-slide="current.id">
    <div class="ambient ambient-one" aria-hidden="true"></div>
    <div class="ambient ambient-two" aria-hidden="true"></div>
    <div class="grain" aria-hidden="true"></div>

    <header class="onboarding-header">
      <div class="brand" aria-label="KotoClip">
        <span class="brand-mark" aria-hidden="true">
          <span></span>
          <span></span>
        </span>
        <span>KotoClip</span>
      </div>

      <div class="progress" aria-label="引导进度">
        <button
          v-for="(slide, index) in slides"
          :key="slide.id"
          class="progress-dot"
          :class="{ active: index === activeIndex, passed: index < activeIndex }"
          :aria-label="`前往第 ${index + 1} 页`"
          :aria-current="index === activeIndex ? 'step' : undefined"
          @click="goTo(index)"
        >
          <span></span>
        </button>
      </div>

      <button class="skip-button" type="button" @click="emit('complete')">
        跳过介绍
        <X :size="15" aria-hidden="true" />
      </button>
    </header>

    <div class="stage" aria-live="polite">
      <Transition :name="transitionName" mode="out-in">
        <section :key="current.id" class="slide">
          <div class="copy-column">
            <p class="eyebrow">{{ current.eyebrow }}</p>
            <h1>{{ current.title }}</h1>
            <p class="lead">{{ current.lead }}</p>

            <p v-if="current.id === 'welcome'" class="body-copy">
              从一句一句看懂，走到真正读完一本书。KotoClip 会在你需要时提供恰到好处的帮助。
            </p>

            <div v-else-if="current.id === 'ascent'" class="feature-grid feature-grid--ascent">
              <article>
                <span class="feature-index">01</span>
                <div><strong>语素与文节拆分</strong><p>先看清句子由什么组成。</p></div>
              </article>
              <article>
                <span class="feature-index">02</span>
                <div><strong>权威词典解释</strong><p>用可靠来源确认本句含义。</p></div>
              </article>
              <article>
                <span class="feature-index">03</span>
                <div><strong>智能语法分析</strong><p>找到连接关系和句内作用。</p></div>
              </article>
              <article>
                <span class="feature-index">04</span>
                <div><strong>AI 语境问答</strong><p>带着当前段落继续追问。</p></div>
              </article>
            </div>

            <div v-else-if="current.id === 'interest'" class="interest-copy">
              <div class="material-list" aria-label="支持的内容类型">
                <span>经典名著</span>
                <span>网络文案</span>
                <span>视频字幕</span>
                <span>ACGN 作品</span>
              </div>
              <p class="conversion-line"><Check :size="17" aria-hidden="true" /> 一键成为你的阅读教材</p>
            </div>

            <div v-else class="feature-grid feature-grid--system">
              <article><span class="system-glyph">N</span><div><strong>N1–N5 严格分级</strong><p>清楚定位当前难度。</p></div></article>
              <article><span class="system-glyph">文</span><div><strong>完整文法与阅读技巧</strong><p>从规则走向真实语境。</p></div></article>
              <article><span class="system-glyph">卡</span><div><strong>ANKI 卡片生成</strong><p>把阅读现场直接带入复习。</p></div></article>
              <article><span class="system-glyph">曲</span><div><strong>遗忘曲线动态提示</strong><p>在需要的时候再次遇见。</p></div></article>
            </div>

            <div class="actions">
              <button
                v-if="activeIndex > 0"
                class="back-button"
                type="button"
                @click="goTo(activeIndex - 1)"
              >
                <ArrowLeft :size="17" aria-hidden="true" />
                上一页
              </button>

              <button
                v-if="current.id !== 'ascent'"
                class="primary-button"
                type="button"
                @click="next"
              >
                {{ current.id === 'welcome' ? '了解 KotoClip' : current.id === 'system' ? '开始使用 KotoClip' : '继续' }}
                <ArrowRight :size="17" aria-hidden="true" />
              </button>

              <p v-else class="reward-direction">
                点击右侧那一点奖励，继续前往“兴趣激励”。
              </p>
            </div>

            <p v-if="current.id === 'system'" class="release-note">
              v1.0 从核心阅读体验开始，相关能力会持续完善。
            </p>
          </div>

          <div class="art-column" aria-hidden="false">
            <WelcomeArt v-if="current.id === 'welcome'" />
            <ReadingAscentArt
              v-else-if="current.id === 'ascent'"
              :prompting="rewardPrompting"
              @claim="claimReward"
            />
            <ContentOrbitArt v-else-if="current.id === 'interest'" />
            <LearningSystemArt v-else />
          </div>
        </section>
      </Transition>
    </div>

    <footer class="onboarding-footer">
      <span>{{ String(activeIndex + 1).padStart(2, "0") }}</span>
      <span class="footer-line"></span>
      <span>{{ String(slides.length).padStart(2, "0") }}</span>
      <span class="keyboard-hint">方向键也可以翻页</span>
    </footer>
  </main>
</template>

<style scoped>
.onboarding {
  --page-bg: #f4f1ec;
  --page-surface: rgba(255, 255, 255, 0.62);
  --page-ink: #172036;
  --page-muted: #657087;
  --page-accent: #5b55d6;
  --page-accent-soft: rgba(91, 85, 214, 0.12);
  isolation: isolate;
  position: relative;
  width: 100%;
  height: 100%;
  min-height: 560px;
  overflow: clip;
  color: var(--page-ink);
  background: var(--page-bg);
  font-family: var(--font-ui);
  transition: background-color 650ms ease;
}

.onboarding[data-slide="ascent"] {
  --page-bg: #edf2f0;
  --page-accent: #315d73;
  --page-accent-soft: rgba(49, 93, 115, 0.12);
}

.onboarding[data-slide="interest"] {
  --page-bg: #f6efe9;
  --page-accent: #7b4aa0;
  --page-accent-soft: rgba(123, 74, 160, 0.12);
}

.onboarding[data-slide="system"] {
  --page-bg: #eef1ee;
  --page-accent: #286f68;
  --page-accent-soft: rgba(40, 111, 104, 0.12);
}

.grain {
  position: absolute;
  inset: 0;
  z-index: -1;
  opacity: 0.24;
  pointer-events: none;
  background-image: url("data:image/svg+xml,%3Csvg viewBox='0 0 180 180' xmlns='http://www.w3.org/2000/svg'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='.88' numOctaves='2' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)' opacity='.09'/%3E%3C/svg%3E");
  mix-blend-mode: multiply;
}

.ambient {
  position: absolute;
  z-index: -2;
  width: 36vw;
  aspect-ratio: 1;
  border-radius: 50%;
  filter: blur(12px);
  opacity: 0.42;
  transition: background 700ms ease, transform 1.1s cubic-bezier(.2,.8,.2,1);
}

.ambient-one {
  top: -23vw;
  right: -8vw;
  background: radial-gradient(circle, rgba(137, 116, 255, .48), rgba(137, 116, 255, 0) 70%);
}

.ambient-two {
  bottom: -27vw;
  left: -12vw;
  background: radial-gradient(circle, rgba(255, 183, 90, .38), rgba(255, 183, 90, 0) 70%);
}

[data-slide="ascent"] .ambient-one { transform: translate(-6vw, 6vw); background: radial-gradient(circle, rgba(91, 171, 190, .42), transparent 70%); }
[data-slide="interest"] .ambient-one { transform: translate(-12vw, 3vw); background: radial-gradient(circle, rgba(151, 96, 205, .42), transparent 70%); }
[data-slide="interest"] .ambient-two { transform: translate(8vw, -2vw); background: radial-gradient(circle, rgba(255, 180, 79, .45), transparent 70%); }
[data-slide="system"] .ambient-one { transform: translate(-8vw, 8vw); background: radial-gradient(circle, rgba(77, 161, 150, .42), transparent 70%); }

.onboarding-header,
.onboarding-footer {
  position: relative;
  z-index: 4;
  display: flex;
  align-items: center;
  width: 100%;
  padding-inline: clamp(24px, 4.8vw, 76px);
}

.onboarding-header {
  height: 76px;
  justify-content: space-between;
}

.brand {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: .88rem;
  font-weight: 760;
  letter-spacing: .02em;
}

.brand-mark {
  position: relative;
  display: block;
  width: 25px;
  height: 25px;
  border: 1px solid color-mix(in srgb, var(--page-accent) 42%, transparent);
  border-radius: 8px;
  transform: rotate(-5deg);
}

.brand-mark span {
  position: absolute;
  top: 6px;
  width: 7px;
  height: 12px;
  background: var(--page-accent);
}

.brand-mark span:first-child { left: 5px; border-radius: 4px 1px 1px 4px; }
.brand-mark span:last-child { right: 5px; border-radius: 1px 4px 4px 1px; opacity: .58; }

.progress {
  position: absolute;
  left: 50%;
  display: flex;
  align-items: center;
  gap: 12px;
  transform: translateX(-50%);
}

.progress-dot {
  display: grid;
  place-items: center;
  width: 22px;
  height: 22px;
  padding: 0;
  border: 0;
  border-radius: 50%;
  background: transparent;
  cursor: pointer;
}

.progress-dot span {
  width: 5px;
  height: 5px;
  border-radius: 999px;
  background: rgba(23, 32, 54, .22);
  transition: width 250ms ease, background 250ms ease, transform 250ms ease;
}

.progress-dot.passed span { background: color-mix(in srgb, var(--page-accent) 50%, white); }
.progress-dot.active span { width: 24px; background: var(--page-accent); }
.progress-dot:focus-visible { outline: 2px solid var(--page-accent); outline-offset: 2px; }

.skip-button {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 7px 4px 7px 10px;
  border: 0;
  background: transparent;
  color: var(--page-muted);
  font-size: .78rem;
  cursor: pointer;
  transition: color 180ms ease;
}

.skip-button:hover { color: var(--page-ink); }

.stage {
  height: calc(100% - 120px);
  min-height: 440px;
}

.slide {
  display: grid;
  grid-template-columns: minmax(360px, .88fr) minmax(460px, 1.12fr);
  align-items: center;
  gap: clamp(24px, 5vw, 88px);
  width: min(1380px, 100%);
  height: 100%;
  margin: 0 auto;
  padding: 10px clamp(28px, 6vw, 94px) 28px;
}

.copy-column {
  position: relative;
  z-index: 2;
  max-width: 580px;
}

.eyebrow {
  margin-bottom: 18px;
  color: var(--page-accent);
  font-size: .72rem;
  font-weight: 760;
  letter-spacing: .17em;
}

h1 {
  max-width: 640px;
  font-size: clamp(2.35rem, 4.15vw, 4.85rem);
  line-height: 1.07;
  letter-spacing: -.055em;
  text-wrap: balance;
}

[data-slide="ascent"] h1,
[data-slide="interest"] h1,
[data-slide="system"] h1 {
  font-size: clamp(2.05rem, 3.35vw, 3.8rem);
}

.lead {
  max-width: 590px;
  margin-top: 22px;
  color: var(--page-muted);
  font-size: clamp(1rem, 1.38vw, 1.22rem);
  line-height: 1.75;
}

[data-slide="welcome"] .lead {
  color: var(--page-ink);
  font-weight: 650;
}

.body-copy {
  max-width: 530px;
  margin-top: 22px;
  color: var(--page-muted);
  line-height: 1.85;
}

.feature-grid {
  display: grid;
  gap: 9px;
  margin-top: 25px;
}

.feature-grid article {
  display: flex;
  align-items: center;
  gap: 13px;
  min-width: 0;
  padding: 10px 12px;
  border: 1px solid rgba(23, 32, 54, .08);
  border-radius: 14px;
  background: var(--page-surface);
  backdrop-filter: blur(10px);
}

.feature-grid--ascent { grid-template-columns: repeat(2, minmax(0, 1fr)); }
.feature-grid--system { grid-template-columns: repeat(2, minmax(0, 1fr)); }

.feature-index,
.system-glyph {
  flex: 0 0 auto;
  display: grid;
  place-items: center;
  width: 33px;
  height: 33px;
  border-radius: 11px;
  color: var(--page-accent);
  background: var(--page-accent-soft);
  font-size: .62rem;
  font-weight: 800;
}

.system-glyph { font-family: var(--font-ja); font-size: .82rem; }
.feature-grid strong { display: block; font-size: .82rem; line-height: 1.35; }
.feature-grid p { margin-top: 3px; color: var(--page-muted); font-size: .69rem; line-height: 1.45; }

.interest-copy { margin-top: 28px; }
.material-list { display: flex; flex-wrap: wrap; gap: 9px; }
.material-list span {
  padding: 8px 13px;
  border: 1px solid rgba(123, 74, 160, .14);
  border-radius: 999px;
  color: #624078;
  background: rgba(255, 255, 255, .52);
  font-size: .78rem;
  font-weight: 650;
}

.conversion-line {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 17px;
  color: var(--page-accent);
  font-size: .86rem;
  font-weight: 720;
}

.actions {
  display: flex;
  align-items: center;
  gap: 12px;
  min-height: 48px;
  margin-top: 30px;
}

.primary-button,
.back-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 9px;
  min-height: 44px;
  border-radius: 999px;
  cursor: pointer;
  transition: transform 180ms ease, box-shadow 180ms ease, background 180ms ease;
}

.primary-button {
  padding: 11px 20px 11px 23px;
  border: 1px solid transparent;
  color: #fff;
  background: var(--page-accent);
  box-shadow: 0 11px 30px color-mix(in srgb, var(--page-accent) 23%, transparent);
  font-weight: 720;
}

.primary-button:hover { transform: translateY(-2px); box-shadow: 0 15px 34px color-mix(in srgb, var(--page-accent) 30%, transparent); }

.back-button {
  padding: 10px 14px;
  border: 1px solid rgba(23, 32, 54, .1);
  color: var(--page-muted);
  background: rgba(255, 255, 255, .42);
}

.back-button:hover { color: var(--page-ink); background: rgba(255, 255, 255, .7); }
.primary-button:focus-visible, .back-button:focus-visible, .skip-button:focus-visible { outline: 2px solid var(--page-accent); outline-offset: 3px; }

.reward-direction {
  max-width: 270px;
  color: var(--page-accent);
  font-size: .76rem;
  line-height: 1.55;
}

.release-note { margin-top: 12px; color: var(--page-muted); font-size: .69rem; }

.art-column {
  position: relative;
  display: grid;
  place-items: center;
  min-width: 0;
  height: min(65vh, 640px);
}

.onboarding-footer {
  position: absolute;
  bottom: 0;
  height: 44px;
  gap: 8px;
  color: var(--page-muted);
  font-size: .64rem;
  letter-spacing: .1em;
}

.footer-line { width: 42px; height: 1px; background: rgba(23, 32, 54, .18); }
.keyboard-hint { margin-left: auto; letter-spacing: 0; }

.slide-forward-enter-active,
.slide-forward-leave-active,
.slide-back-enter-active,
.slide-back-leave-active {
  transition: opacity 410ms ease, transform 560ms cubic-bezier(.2,.82,.2,1), filter 410ms ease;
}

.slide-forward-enter-from { opacity: 0; transform: translateX(70px); filter: blur(6px); }
.slide-forward-leave-to { opacity: 0; transform: translateX(-48px); filter: blur(4px); }
.slide-back-enter-from { opacity: 0; transform: translateX(-70px); filter: blur(6px); }
.slide-back-leave-to { opacity: 0; transform: translateX(48px); filter: blur(4px); }

@media (max-width: 980px) {
  .onboarding { min-height: 640px; overflow-x: clip; overflow-y: auto; }
  .stage { height: auto; min-height: calc(100% - 120px); }
  .slide { grid-template-columns: 1fr; align-content: start; gap: 20px; height: auto; min-height: calc(100vh - 120px); padding-top: 34px; padding-bottom: 70px; }
  .copy-column { max-width: 720px; }
  h1, [data-slide="ascent"] h1, [data-slide="interest"] h1, [data-slide="system"] h1 { max-width: 760px; font-size: clamp(2.1rem, 6vw, 3.8rem); }
  .art-column { grid-row: 1; height: min(42vh, 420px); overflow: clip; }
  .copy-column { grid-row: 2; }
  .onboarding-footer { position: fixed; }
}

@media (max-width: 620px) {
  .onboarding-header { height: 64px; padding-inline: 18px; }
  .brand > span:last-child, .keyboard-hint { display: none; }
  .progress { gap: 7px; }
  .skip-button { font-size: 0; }
  .slide { padding: 18px 20px 64px; }
  .art-column { height: min(35vh, 310px); }
  .eyebrow { margin-bottom: 12px; }
  h1, [data-slide="ascent"] h1, [data-slide="interest"] h1, [data-slide="system"] h1 { font-size: clamp(1.8rem, 9vw, 2.75rem); }
  .lead { margin-top: 14px; font-size: .95rem; }
  .feature-grid--ascent, .feature-grid--system { grid-template-columns: 1fr; }
  .slide { padding-bottom: 108px; }
  .onboarding-footer { display: none; }
  .actions { margin-top: 22px; flex-wrap: wrap; }
  .back-button { flex: 0 0 auto; white-space: nowrap; }
  .reward-direction { flex: 1 1 190px; min-width: 0; }
}

@media (prefers-reduced-motion: reduce) {
  .onboarding *, .onboarding *::before, .onboarding *::after {
    scroll-behavior: auto !important;
    animation-duration: .001ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: .001ms !important;
  }
}
</style>
