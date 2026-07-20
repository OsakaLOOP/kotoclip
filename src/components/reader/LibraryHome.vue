<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { BookOpen, BookOpenText, Clock3, FileText, FileUp, FolderOpen, Search, Trash2 } from "@lucide/vue";
import type { LibraryBookSummary } from "../../reader/library";

const props = defineProps<{
  books: LibraryBookSummary[];
  loading: boolean;
  importing: boolean;
  openingBookId: string | null;
  error: string | null;
  libraryPath: string;
  coverUrl: (book: LibraryBookSummary) => string | undefined;
}>();

const emit = defineEmits<{
  import: [];
  open: [id: string];
  input: [];
  reveal: [];
  remove: [book: LibraryBookSummary];
}>();
const searchQuery = ref("");
type BookSurface = "continue" | "shelf";

interface CoverRipple {
  key: number;
  target: string;
  left: number;
  top: number;
  size: number;
}

const coverRipple = ref<CoverRipple | null>(null);
const openingSurfaceTarget = ref<string | null>(null);
let rippleKey = 0;
let rippleTimer: number | undefined;

const filteredBooks = computed(() => {
  const query = searchQuery.value.trim().toLocaleLowerCase();
  if (!query) return props.books;
  return props.books.filter((book) =>
    [book.title, book.author, book.currentChapter, book.sourceName]
      .some((value) => value?.toLocaleLowerCase().includes(query))
  );
});

function progressLabel(book: LibraryBookSummary): string {
  const percent = progressPercent(book);
  return percent > 0 ? `${percent}%` : "未开始";
}

function progressPercent(book: LibraryBookSummary): number {
  return Math.round(Math.min(1, Math.max(0, book.progressPercent)) * 100);
}

function dateLabel(value?: string | null): string {
  if (!value) return "尚未阅读";
  const date = new Date(value);
  return Number.isNaN(date.getTime())
    ? "最近读过"
    : new Intl.DateTimeFormat("zh-CN", { month: "short", day: "numeric" }).format(date);
}

function rippleTarget(surface: BookSurface, bookId: string): string {
  return `${surface}:${bookId}`;
}

function startCoverRipple(
  event: PointerEvent | MouseEvent,
  bookId: string,
  surface: BookSurface,
  centered = false,
) {
  const button = event.currentTarget as HTMLElement | null;
  const cover = button?.querySelector<HTMLElement>(surface === "continue" ? ".continue-cover" : ".book-cover");
  if (!cover) return;
  const bounds = cover.getBoundingClientRect();
  const fromCover = event.target instanceof Node && cover.contains(event.target);
  const useCenter = centered || !fromCover;
  const x = useCenter ? bounds.width / 2 : Math.min(bounds.width, Math.max(0, event.clientX - bounds.left));
  const y = useCenter ? bounds.height / 2 : Math.min(bounds.height, Math.max(0, event.clientY - bounds.top));
  const radius = Math.hypot(Math.max(x, bounds.width - x), Math.max(y, bounds.height - y));
  coverRipple.value = {
    key: ++rippleKey,
    target: rippleTarget(surface, bookId),
    left: x - radius,
    top: y - radius,
    size: radius * 2,
  };
  if (rippleTimer !== undefined) window.clearTimeout(rippleTimer);
  rippleTimer = window.setTimeout(() => {
    coverRipple.value = null;
    rippleTimer = undefined;
  }, 320);
}

function openBook(event: MouseEvent, bookId: string, surface: BookSurface) {
  if (event.detail === 0) startCoverRipple(event, bookId, surface, true);
  openingSurfaceTarget.value = rippleTarget(surface, bookId);
  emit("open", bookId);
}

function isOpeningSurface(surface: BookSurface, bookId: string): boolean {
  return props.openingBookId === bookId
    && openingSurfaceTarget.value === rippleTarget(surface, bookId);
}

function rippleStyle(surface: BookSurface, bookId: string): Record<string, string> | undefined {
  const ripple = coverRipple.value;
  if (!ripple || ripple.target !== rippleTarget(surface, bookId)) return undefined;
  return {
    width: `${ripple.size}px`,
    height: `${ripple.size}px`,
    left: `${ripple.left}px`,
    top: `${ripple.top}px`,
  };
}

onBeforeUnmount(() => {
  if (rippleTimer !== undefined) window.clearTimeout(rippleTimer);
});

watch(() => props.openingBookId, (bookId) => {
  if (!bookId) openingSurfaceTarget.value = null;
});
</script>

<template>
  <main class="library-home">
    <div class="library-toolbar">
      <div>
        <h1>书架</h1>
        <button class="library-location" type="button" title="在资源管理器中显示书库" @click="emit('reveal')">
          <FolderOpen :size="14" aria-hidden="true" />
          <span>{{ libraryPath || '正在读取书库位置…' }}</span>
        </button>
      </div>
    </div>

    <p v-if="error" class="library-error" role="alert">{{ error }}</p>

    <div v-if="loading" class="library-loading" role="status">正在读取书架…</div>

    <template v-else>
      <section v-if="books[0]?.lastOpenedAt" class="continue-section" aria-labelledby="continue-title">
        <div class="section-heading">
          <h2 id="continue-title">继续阅读</h2>
        </div>
        <button
          class="continue-book"
          type="button"
          :disabled="Boolean(openingBookId)"
          :aria-busy="isOpeningSurface('continue', books[0].id)"
          @pointerdown="startCoverRipple($event, books[0].id, 'continue')"
          @click="openBook($event, books[0].id, 'continue')"
        >
          <div
            class="continue-cover"
            :style="isOpeningSurface('continue', books[0].id) ? { viewTransitionName: 'book-cover' } : undefined"
          >
            <img v-if="props.coverUrl(books[0])" :src="props.coverUrl(books[0])" alt="" />
            <BookOpen v-else :size="30" aria-hidden="true" />
            <span class="cover-state-layer" aria-hidden="true"></span>
            <span
              v-if="coverRipple?.target === rippleTarget('continue', books[0].id)"
              :key="coverRipple.key"
              class="cover-ripple"
              :style="rippleStyle('continue', books[0].id)"
              aria-hidden="true"
            ></span>
            <span v-if="isOpeningSurface('continue', books[0].id)" class="cover-opening" aria-hidden="true">
              <span class="opening-mark"><BookOpenText :size="23" stroke-width="2.5" /></span>
            </span>
          </div>
          <div class="continue-copy">
            <strong>{{ books[0].title }}</strong>
            <span>{{ books[0].author }}</span>
            <span v-if="books[0].currentChapter" class="continue-chapter">{{ books[0].currentChapter }}</span>
          </div>
          <div
            class="continue-progress"
            :style="{ '--reading-progress': `${progressPercent(books[0]) * 3.6}deg` }"
            :aria-label="progressLabel(books[0])"
          >
            <span>{{ progressPercent(books[0]) }}%</span>
          </div>
        </button>
      </section>

      <section class="all-books" aria-labelledby="all-books-title">
        <div class="section-heading">
          <h2 id="all-books-title">全部书籍 <span>({{ books.length }})</span></h2>
          <div class="shelf-tools">
            <label class="shelf-search">
              <Search :size="15" aria-hidden="true" />
              <input v-model="searchQuery" type="search" placeholder="搜索书名或作者" />
            </label>
          </div>
        </div>
        <div class="book-grid">
          <article v-for="book in filteredBooks" :key="book.id" class="book-card">
            <button
              class="book-open"
              type="button"
              :disabled="Boolean(openingBookId)"
              :aria-busy="isOpeningSurface('shelf', book.id)"
              @pointerdown="startCoverRipple($event, book.id, 'shelf')"
              @click="openBook($event, book.id, 'shelf')"
            >
              <div
                class="book-cover"
                :style="isOpeningSurface('shelf', book.id) ? { viewTransitionName: 'book-cover' } : undefined"
              >
                <img v-if="props.coverUrl(book)" :src="props.coverUrl(book)" alt="" />
                <BookOpen v-else :size="32" aria-hidden="true" />
                <span class="cover-state-layer" aria-hidden="true"></span>
                <span
                  v-if="coverRipple?.target === rippleTarget('shelf', book.id)"
                  :key="coverRipple.key"
                  class="cover-ripple"
                  :style="rippleStyle('shelf', book.id)"
                  aria-hidden="true"
                ></span>
                <span v-if="isOpeningSurface('shelf', book.id)" class="cover-opening" aria-hidden="true">
                  <span class="opening-mark"><BookOpenText :size="28" stroke-width="2.5" /></span>
                </span>
              </div>
              <strong>{{ book.title }}</strong>
              <span class="book-author">{{ book.author }}</span>
              <div class="book-meta">
                <span><Clock3 :size="13" aria-hidden="true" />{{ dateLabel(book.lastOpenedAt) }}</span>
                <span>{{ progressLabel(book) }}</span>
              </div>
              <div class="book-progress"><i :style="{ width: `${book.progressPercent * 100}%` }"></i></div>
            </button>
            <button
              class="book-menu"
              type="button"
              title="从书架移除"
              :disabled="Boolean(openingBookId)"
              @click="emit('remove', book)"
            >
              <Trash2 :size="16" aria-hidden="true" />
              <span class="sr-only">移除 {{ book.title }}</span>
            </button>
          </article>
          <article class="shelf-import-card" aria-label="添加阅读内容">
            <button
              type="button"
              :disabled="importing || Boolean(openingBookId)"
              @click="emit('input')"
            >
              <FileText :size="22" stroke-width="1.7" aria-hidden="true" />
              <span>粘贴 Markdown</span>
            </button>
            <button
              type="button"
              :disabled="importing || Boolean(openingBookId)"
              @click="emit('import')"
            >
              <FileUp :size="22" stroke-width="1.7" aria-hidden="true" />
              <span>{{ importing ? '正在导入…' : '导入 EPUB' }}</span>
            </button>
          </article>
        </div>
        <p v-if="searchQuery && filteredBooks.length === 0" class="no-search-results">没有匹配的书籍</p>
      </section>
    </template>
  </main>
</template>

<style scoped>
.library-home {
  width: 100%;
  height: 100%;
  overflow-y: auto;
  padding: 34px clamp(24px, 5vw, 72px) 64px;
  color: var(--text-primary);
}

.library-toolbar,
.section-heading,
.shelf-tools,
.shelf-search,
.book-meta,
.continue-book,
.library-location {
  display: flex;
  align-items: center;
}

.library-toolbar {
  justify-content: space-between;
  gap: 24px;
  margin: 0 auto 34px;
  max-width: 1180px;
}

h1 {
  font-size: 1.65rem;
  line-height: 1.2;
  letter-spacing: 0;
}

.library-location {
  gap: 6px;
  max-width: min(620px, 60vw);
  margin-top: 6px;
  padding: 0;
  border: 0;
  background: none;
  color: var(--text-muted);
  cursor: pointer;
  font-size: 0.76rem;
}

.library-location span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.shelf-tools,
.shelf-search {
  gap: 7px;
}

.shelf-search {
  height: 34px;
  padding: 0 10px;
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  background: var(--bg-secondary);
  color: var(--text-muted);
}

.shelf-search input {
  width: min(220px, 24vw);
  border: 0;
  outline: 0;
  background: transparent;
  color: var(--text-primary);
  font-size: 0.78rem;
}

.section-heading {
  justify-content: space-between;
  margin-bottom: 13px;
}

.section-heading h2 {
  font-size: 1rem;
  letter-spacing: 0;
}

.section-heading span {
  color: var(--text-muted);
  font-size: 0.78rem;
}

.continue-section,
.all-books {
  max-width: 1180px;
  margin: 0 auto 34px;
}

.continue-book {
  width: 100%;
  gap: 18px;
  padding: 18px;
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  background: var(--bg-secondary);
  color: inherit;
  cursor: pointer;
  text-align: left;
  transition: background-color 180ms cubic-bezier(.4, 0, .2, 1);
}

.continue-cover,
.book-cover {
  position: relative;
  display: grid;
  place-items: center;
  flex: 0 0 auto;
  overflow: hidden;
  background: var(--accent-light);
  color: var(--accent-color);
  border: 1px solid var(--border-color);
  transform: translateY(0);
  transition: box-shadow 180ms cubic-bezier(.4, 0, .2, 1), transform 180ms cubic-bezier(.4, 0, .2, 1);
  will-change: transform;
}

.continue-cover {
  width: 66px;
  aspect-ratio: 3 / 4;
}

.continue-cover img,
.book-cover img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.continue-book:hover:not(:disabled) .continue-cover,
.continue-book:focus-visible .continue-cover,
.book-open:hover:not(:disabled) .book-cover,
.book-open:focus-visible .book-cover {
  box-shadow: 0 8px 18px color-mix(in srgb, var(--text-primary) 11%, transparent);
  transform: translateY(-6px);
}

.continue-copy {
  display: flex;
  flex: 1;
  min-width: 0;
  flex-direction: column;
  justify-content: center;
}

.continue-copy strong {
  display: -webkit-box;
  overflow: hidden;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 3;
  overflow-wrap: anywhere;
  font-size: 1rem;
  line-height: 1.45;
}

.continue-copy span {
  color: var(--text-muted);
  font-size: 0.8rem;
}

.continue-copy .continue-chapter {
  margin-top: 8px;
  color: var(--text-secondary);
}

.continue-progress {
  position: relative;
  display: grid;
  width: 58px;
  height: 58px;
  flex: 0 0 auto;
  place-items: center;
  border-radius: 50%;
  background: conic-gradient(
    var(--accent-color) var(--reading-progress),
    var(--border-color) 0
  );
  color: var(--text-secondary);
  font-size: 0.8rem;
}

.continue-progress::before {
  position: absolute;
  inset: 5px;
  border-radius: 50%;
  background: var(--bg-secondary);
  content: "";
}

.continue-progress span {
  position: relative;
  z-index: 1;
  color: var(--text-secondary);
  font-size: 0.72rem;
  font-variant-numeric: tabular-nums;
}

.book-progress {
  height: 3px;
  overflow: hidden;
  margin-top: 7px;
  background: var(--border-color);
}

.book-progress i {
  display: block;
  height: 100%;
  background: var(--accent-color);
}

.book-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(162px, 1fr));
  gap: 22px 18px;
}

.book-card {
  position: relative;
  min-width: 0;
}

.shelf-import-card {
  display: grid;
  min-width: 0;
  aspect-ratio: 3 / 4;
  grid-template-rows: repeat(2, minmax(0, 1fr));
  overflow: hidden;
  border: 1px solid var(--border-color);
  border-radius: 4px;
  background: var(--bg-secondary);
}

.shelf-import-card button {
  display: flex;
  min-width: 0;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 12px 8px;
  border: 0;
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  font: inherit;
  font-size: 0.78rem;
}

.shelf-import-card button + button {
  border-top: 1px solid var(--border-color);
}

.shelf-import-card button:hover:not(:disabled),
.shelf-import-card button:focus-visible {
  background: color-mix(in srgb, var(--accent-color) 7%, transparent);
  color: var(--accent-color);
}

.shelf-import-card button:focus-visible {
  outline: 2px solid color-mix(in srgb, var(--accent-color) 72%, transparent);
  outline-offset: -2px;
}

.shelf-import-card button:disabled {
  opacity: 0.5;
  cursor: default;
}

.book-open {
  display: flex;
  width: 100%;
  min-width: 0;
  flex-direction: column;
  align-items: stretch;
  border: 0;
  background: transparent;
  color: inherit;
  cursor: pointer;
  text-align: left;
}

.book-open strong {
  transition: color 140ms ease;
}

.book-cover {
  width: 100%;
  aspect-ratio: 3 / 4;
  margin-bottom: 10px;
  border-radius: 4px;
}

.continue-book:focus-visible,
.book-open:focus-visible {
  outline: none;
}

.continue-book:focus-visible .continue-cover,
.book-open:focus-visible .book-cover {
  outline: 2px solid color-mix(in srgb, var(--accent-color) 72%, transparent);
  outline-offset: 3px;
}

.book-open:hover:not(:disabled) strong,
.book-open:focus-visible strong {
  color: var(--accent-color);
}

.continue-book:active:not(:disabled) .continue-cover,
.book-open:active:not(:disabled) .book-cover {
  box-shadow: 0 3px 9px color-mix(in srgb, var(--text-primary) 12%, transparent);
  transform: translateY(-2px);
  transition-duration: 90ms;
}

.continue-book:disabled,
.book-open:disabled {
  cursor: default;
}

.continue-book:disabled:not([aria-busy="true"]),
.book-open:disabled:not([aria-busy="true"]) {
  opacity: 0.62;
}

.cover-opening {
  position: absolute;
  z-index: 3;
  inset: 0;
  display: grid;
  place-items: center;
  overflow: hidden;
  pointer-events: none;
}

.cover-state-layer {
  position: absolute;
  z-index: 1;
  inset: 0;
  background: var(--accent-color);
  opacity: 0;
  pointer-events: none;
  transition: opacity 120ms ease;
}

.continue-book:hover:not(:disabled) .cover-state-layer,
.continue-book:focus-visible .cover-state-layer,
.book-open:hover:not(:disabled) .cover-state-layer,
.book-open:focus-visible .cover-state-layer {
  opacity: 0.055;
}

.continue-book:active:not(:disabled) .cover-state-layer,
.book-open:active:not(:disabled) .cover-state-layer {
  opacity: 0.11;
}

.cover-ripple {
  position: absolute;
  z-index: 4;
  border-radius: 50%;
  background: color-mix(in srgb, var(--accent-color) 22%, transparent);
  opacity: 0;
  pointer-events: none;
  transform: scale(0);
  animation: cover-ripple 300ms cubic-bezier(.4, 0, .2, 1);
}

.cover-opening::before {
  position: absolute;
  width: 22%;
  aspect-ratio: 1;
  border-radius: 50%;
  z-index: 0;
  background: color-mix(in srgb, var(--accent-color) 18%, transparent);
  content: "";
  animation: cover-opening-wave 280ms cubic-bezier(0, 0, .2, 1) both;
}

.opening-mark {
  position: relative;
  z-index: 1;
  display: grid;
  width: 46px;
  height: 46px;
  place-items: center;
  border-radius: 7px;
  background: var(--accent-color);
  box-shadow: 0 4px 14px color-mix(in srgb, var(--text-primary) 20%, transparent);
  color: #fff;
  animation: cover-opening-icon 280ms cubic-bezier(.4, 0, .2, 1) both;
}

.continue-cover .opening-mark {
  width: 38px;
  height: 38px;
  border-radius: 6px;
}

.book-open strong,
.book-author {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.book-open strong {
  padding-right: 26px;
  font-size: 0.88rem;
}

.book-author {
  color: var(--text-muted);
  font-size: 0.75rem;
}

.book-meta {
  justify-content: space-between;
  gap: 8px;
  margin-top: 8px;
  color: var(--text-muted);
  font-size: 0.7rem;
}

.book-meta span {
  display: inline-flex;
  align-items: center;
  gap: 3px;
}

.book-menu {
  position: absolute;
  right: 0;
  bottom: 42px;
  display: grid;
  width: 28px;
  height: 28px;
  place-items: center;
  border: 0;
  background: transparent;
  color: var(--text-muted);
  cursor: pointer;
}

.book-menu:hover {
  color: var(--novelty-high-text);
}

.book-menu:disabled {
  opacity: 0.45;
  cursor: default;
}

@keyframes cover-ripple {
  0% { opacity: 0.85; transform: scale(0); }
  70% { opacity: 0.25; }
  100% { opacity: 0; transform: scale(1); }
}

@keyframes cover-opening-wave {
  0% { opacity: 0; transform: scale(0.25); }
  28% { opacity: 0.42; }
  100% { opacity: 0; transform: scale(7.5); }
}

@keyframes cover-opening-icon {
  0% { opacity: 0; transform: translateY(8px) scale(0.86); }
  62% { opacity: 1; transform: translateY(-2px) scale(1.03); }
  100% { opacity: 1; transform: translateY(0) scale(1); }
}

@media (prefers-reduced-motion: reduce) {
  .continue-book,
  .continue-cover,
  .book-cover,
  .cover-state-layer,
  .book-open strong {
    transition: none;
  }

  .cover-ripple,
  .cover-opening::before {
    display: none;
  }

  .opening-mark {
    animation: none;
  }
}

.library-loading {
  display: flex;
  min-height: 52vh;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  color: var(--text-muted);
  text-align: center;
}

.library-error {
  max-width: 1180px;
  margin: -20px auto 24px;
  color: var(--novelty-high-text);
  font-size: 0.84rem;
}

.no-search-results {
  padding: 48px 0;
  color: var(--text-muted);
  text-align: center;
  font-size: 0.82rem;
}

.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
}

@media (max-width: 720px) {
  .library-toolbar {
    align-items: stretch;
    flex-direction: column;
  }

  .book-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
</style>
