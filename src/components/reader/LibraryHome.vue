<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import {
  BookOpen,
  BookOpenText,
  ChevronLeft,
  ChevronRight,
  Clock3,
  FileText,
  FileUp,
  FolderOpen,
  Search,
  Trash2,
} from "@lucide/vue";
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
type PageDirection = "next" | "previous";

interface PaginationPageItem {
  type: "page";
  page: number;
}

interface PaginationEllipsisItem {
  type: "ellipsis";
  key: string;
}

type PaginationItem = PaginationPageItem | PaginationEllipsisItem;

interface CoverRipple {
  key: number;
  target: string;
  left: number;
  top: number;
  size: number;
}

const coverRipple = ref<CoverRipple | null>(null);
const openingSurfaceTarget = ref<string | null>(null);
const recentPageSize = 2;
const recentPage = ref(0);
const recentDirection = ref<PageDirection>("next");
const recentNavigationSide = ref<PageDirection | null>(null);
const shelfPageSizes = [12, 24, 48] as const;
const shelfPageSize = ref<number>(shelfPageSizes[0]);
const shelfPage = ref(0);
let rippleKey = 0;
let rippleTimer: number | undefined;

function openedAtTimestamp(book: LibraryBookSummary): number {
  if (!book.lastOpenedAt) return 0;
  const timestamp = new Date(book.lastOpenedAt).getTime();
  return Number.isNaN(timestamp) ? 0 : timestamp;
}

const recentBooks = computed(() => [...props.books]
  .filter((book) => Boolean(book.lastOpenedAt))
  .sort((left, right) => openedAtTimestamp(right) - openedAtTimestamp(left)));

const recentTotalPages = computed(() => Math.max(1, Math.ceil(recentBooks.value.length / recentPageSize)));
const visibleRecentBooks = computed(() => {
  const start = recentPage.value * recentPageSize;
  return recentBooks.value.slice(start, start + recentPageSize);
});

const filteredBooks = computed(() => {
  const query = searchQuery.value.trim().toLocaleLowerCase();
  if (!query) return props.books;
  return props.books.filter((book) =>
    [book.title, book.author, book.currentChapter, book.sourceName]
      .some((value) => value?.toLocaleLowerCase().includes(query))
  );
});

const shelfTotalPages = computed(() => Math.max(1, Math.ceil(filteredBooks.value.length / shelfPageSize.value)));
const visibleShelfBooks = computed(() => {
  const start = shelfPage.value * shelfPageSize.value;
  const end = Math.min(start + shelfPageSize.value, filteredBooks.value.length);
  return filteredBooks.value.slice(start, end);
});
const showImportCard = computed(() => {
  return shelfPage.value === 0;
});
const shelfPaginationItems = computed<PaginationItem[]>(() => {
  const pageCount = shelfTotalPages.value;
  if (pageCount <= 7) {
    return Array.from({ length: pageCount }, (_, page) => ({ type: "page", page }));
  }

  const pages = [...new Set([
    0,
    pageCount - 1,
    shelfPage.value - 1,
    shelfPage.value,
    shelfPage.value + 1,
  ].filter((page) => page >= 0 && page < pageCount))].sort((left, right) => left - right);

  const items: PaginationItem[] = [];
  pages.forEach((page, index) => {
    const previous = pages[index - 1];
    if (index > 0 && page - previous > 1) {
      items.push({ type: "ellipsis", key: `ellipsis-${previous}-${page}` });
    }
    items.push({ type: "page", page });
  });
  return items;
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

function showRecentPrevious() {
  if (recentPage.value <= 0) return;
  recentDirection.value = "previous";
  recentPage.value -= 1;
  recentNavigationSide.value = null;
}

function showRecentNext() {
  if (recentPage.value >= recentTotalPages.value - 1) return;
  recentDirection.value = "next";
  recentPage.value += 1;
  recentNavigationSide.value = null;
}

function updateRecentNavigationSide(event: PointerEvent) {
  if (recentBooks.value.length <= recentPageSize || event.pointerType === "touch") {
    recentNavigationSide.value = null;
    return;
  }

  const bounds = (event.currentTarget as HTMLElement).getBoundingClientRect();
  const activationWidth = Math.min(84, Math.max(52, bounds.width * 0.12));
  const offset = event.clientX - bounds.left;
  if (offset <= activationWidth && recentPage.value > 0) {
    recentNavigationSide.value = "previous";
  } else if (offset >= bounds.width - activationWidth && recentPage.value < recentTotalPages.value - 1) {
    recentNavigationSide.value = "next";
  } else {
    recentNavigationSide.value = null;
  }
}

function goToShelfPage(page: number) {
  shelfPage.value = Math.min(Math.max(0, page), shelfTotalPages.value - 1);
}

onBeforeUnmount(() => {
  if (rippleTimer !== undefined) window.clearTimeout(rippleTimer);
});

watch(() => props.openingBookId, (bookId) => {
  if (!bookId) openingSurfaceTarget.value = null;
});

watch(recentTotalPages, (pageCount) => {
  recentPage.value = Math.min(recentPage.value, pageCount - 1);
});

watch(searchQuery, () => {
  shelfPage.value = 0;
});

watch(shelfPageSize, () => {
  shelfPage.value = 0;
});

watch(shelfTotalPages, (pageCount) => {
  shelfPage.value = Math.min(shelfPage.value, pageCount - 1);
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
      <section class="continue-section" aria-labelledby="continue-title">
        <div class="section-heading">
          <h2 id="continue-title">继续阅读</h2>
          <span v-if="recentBooks.length > recentPageSize" class="recent-page-status" aria-live="polite">
            {{ recentPage + 1 }}/{{ recentTotalPages }}
          </span>
        </div>
        <div v-if="recentBooks.length === 0" class="continue-skeleton" role="status" aria-label="暂无继续阅读的书籍">
          <span class="continue-skeleton__cover"></span>
          <span class="continue-skeleton__copy">
            <i></i>
            <i></i>
            <i></i>
          </span>
          <span class="continue-skeleton__progress"></span>
        </div>
        <div
          v-else
          class="recent-browser"
          @pointermove="updateRecentNavigationSide"
          @pointerleave="recentNavigationSide = null"
        >
          <Transition
            :name="recentDirection === 'next' ? 'recent-page-next' : 'recent-page-previous'"
            mode="out-in"
          >
            <div
              :key="`recent-${recentPage}`"
              class="continue-page"
              :class="{ 'is-single': visibleRecentBooks.length === 1 }"
            >
              <button
                v-for="book in visibleRecentBooks"
                :key="book.id"
                class="continue-book"
                type="button"
                :disabled="Boolean(openingBookId)"
                :aria-busy="isOpeningSurface('continue', book.id)"
                @pointerdown="startCoverRipple($event, book.id, 'continue')"
                @click="openBook($event, book.id, 'continue')"
              >
                <div
                  class="continue-cover"
                  :style="isOpeningSurface('continue', book.id) ? { viewTransitionName: 'book-cover' } : undefined"
                >
                  <img v-if="props.coverUrl(book)" :src="props.coverUrl(book)" alt="" />
                  <BookOpen v-else :size="30" aria-hidden="true" />
                  <span class="cover-state-layer" aria-hidden="true"></span>
                  <span
                    v-if="coverRipple?.target === rippleTarget('continue', book.id)"
                    :key="coverRipple.key"
                    class="cover-ripple"
                    :style="rippleStyle('continue', book.id)"
                    aria-hidden="true"
                  ></span>
                  <span v-if="isOpeningSurface('continue', book.id)" class="cover-opening" aria-hidden="true">
                    <span class="opening-mark"><BookOpenText :size="23" stroke-width="2.5" /></span>
                  </span>
                </div>
                <div class="continue-copy">
                  <strong>{{ book.title }}</strong>
                  <span>{{ book.author }}</span>
                  <span v-if="book.currentChapter" class="continue-chapter">{{ book.currentChapter }}</span>
                </div>
                <div
                  class="continue-progress"
                  :style="{ '--reading-progress': `${progressPercent(book) * 3.6}deg` }"
                  :aria-label="progressLabel(book)"
                >
                  <span>{{ progressPercent(book) }}%</span>
                </div>
              </button>
            </div>
          </Transition>

          <template v-if="recentBooks.length > recentPageSize">
            <button
              type="button"
              class="recent-browser__nav recent-browser__nav--previous"
              :class="{ 'is-visible': recentNavigationSide === 'previous' }"
              :disabled="recentPage === 0"
              aria-label="上一组继续阅读"
              @click="showRecentPrevious"
            >
              <ChevronLeft :size="24" aria-hidden="true" />
            </button>
            <button
              type="button"
              class="recent-browser__nav recent-browser__nav--next"
              :class="{ 'is-visible': recentNavigationSide === 'next' }"
              :disabled="recentPage === recentTotalPages - 1"
              aria-label="下一组继续阅读"
              @click="showRecentNext"
            >
              <ChevronRight :size="24" aria-hidden="true" />
            </button>
          </template>
        </div>
      </section>

      <section class="all-books" aria-labelledby="all-books-title">
        <div class="section-heading">
          <h2 id="all-books-title">全部书籍 <span>({{ books.length }})</span></h2>
          <div class="shelf-tools">
            <label class="shelf-search">
              <Search :size="15" aria-hidden="true" />
              <input v-model="searchQuery" type="search" placeholder="搜索书名或作者" />
            </label>
            <label class="shelf-page-size">
              <span>每页</span>
              <select v-model.number="shelfPageSize" aria-label="每页显示数量">
                <option v-for="size in shelfPageSizes" :key="size" :value="size">{{ size }} 本</option>
              </select>
            </label>
          </div>
        </div>
        <div class="book-grid">
          <article v-if="showImportCard" class="shelf-import-card" aria-label="添加阅读内容">
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
          <article v-for="book in visibleShelfBooks" :key="book.id" class="book-card">
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
        </div>
        <p v-if="searchQuery && filteredBooks.length === 0" class="no-search-results">没有匹配的书籍</p>
        <nav v-if="shelfTotalPages > 1" class="shelf-pagination" aria-label="书库分页">
          <button
            type="button"
            :disabled="shelfPage === 0"
            aria-label="上一页"
            @click="goToShelfPage(shelfPage - 1)"
          >
            <ChevronLeft :size="17" aria-hidden="true" />
          </button>
          <template v-for="item in shelfPaginationItems" :key="item.type === 'page' ? `page-${item.page}` : item.key">
            <button
              v-if="item.type === 'page'"
              type="button"
              :class="{ 'is-current': item.page === shelfPage }"
              :aria-current="item.page === shelfPage ? 'page' : undefined"
              :aria-label="`第 ${item.page + 1} 页`"
              @click="goToShelfPage(item.page)"
            >
              {{ item.page + 1 }}
            </button>
            <span v-else class="shelf-pagination__ellipsis" aria-hidden="true">…</span>
          </template>
          <button
            type="button"
            :disabled="shelfPage === shelfTotalPages - 1"
            aria-label="下一页"
            @click="goToShelfPage(shelfPage + 1)"
          >
            <ChevronRight :size="17" aria-hidden="true" />
          </button>
        </nav>
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
.shelf-page-size,
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
.shelf-search,
.shelf-page-size {
  gap: 7px;
}

.shelf-tools {
  flex-wrap: wrap;
  justify-content: flex-end;
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

.recent-page-status {
  font-variant-numeric: tabular-nums;
}

.recent-browser {
  position: relative;
  min-width: 0;
  overflow: hidden;
}

.continue-page {
  display: grid;
  min-width: 0;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 14px;
}

.continue-page.is-single {
  grid-template-columns: minmax(0, 1fr);
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

.recent-browser__nav {
  position: absolute;
  z-index: 5;
  inset-block: 0;
  display: flex;
  width: clamp(52px, 12%, 84px);
  align-items: center;
  border: 0;
  padding: 0 9px;
  color: color-mix(in srgb, var(--accent-color) 76%, var(--text-muted));
  opacity: 0.62;
  cursor: pointer;
  transition: color 140ms ease, opacity 140ms ease, background 160ms ease;
}

.recent-browser__nav--previous {
  left: 0;
  justify-content: flex-start;
  background: linear-gradient(90deg, color-mix(in srgb, var(--bg-secondary) 96%, transparent) 16%, transparent 100%);
}

.recent-browser__nav--next {
  right: 0;
  justify-content: flex-end;
  background: linear-gradient(270deg, color-mix(in srgb, var(--bg-secondary) 96%, transparent) 16%, transparent 100%);
}

.recent-browser__nav.is-visible:not(:disabled),
.recent-browser__nav:hover:not(:disabled),
.recent-browser__nav:focus-visible {
  color: var(--accent-color);
  opacity: 1;
}

.recent-browser__nav:disabled {
  opacity: 0;
  pointer-events: none;
}

.continue-skeleton {
  display: flex;
  width: 100%;
  min-height: 122px;
  align-items: center;
  gap: 18px;
  padding: 18px;
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  background: var(--bg-secondary);
}

.continue-skeleton__cover,
.continue-skeleton__copy i,
.continue-skeleton__progress {
  background: linear-gradient(
    100deg,
    color-mix(in srgb, var(--border-color) 80%, transparent) 30%,
    color-mix(in srgb, var(--text-muted) 13%, transparent) 46%,
    color-mix(in srgb, var(--border-color) 80%, transparent) 62%
  );
  background-size: 240% 100%;
  animation: library-skeleton 1.8s ease-in-out infinite;
}

.continue-skeleton__cover {
  width: 66px;
  flex: 0 0 auto;
  aspect-ratio: 3 / 4;
  border-radius: 3px;
}

.continue-skeleton__copy {
  display: grid;
  flex: 1;
  gap: 9px;
}

.continue-skeleton__copy i {
  width: min(330px, 78%);
  height: 10px;
  border-radius: 2px;
}

.continue-skeleton__copy i:nth-child(2) {
  width: min(220px, 56%);
  height: 8px;
}

.continue-skeleton__copy i:nth-child(3) {
  width: min(280px, 68%);
  height: 8px;
}

.continue-skeleton__progress {
  width: 58px;
  height: 58px;
  flex: 0 0 auto;
  border-radius: 50%;
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
  grid-template-columns: repeat(auto-fill, minmax(150px, 176px));
  gap: 22px 18px;
  justify-content: start;
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

.shelf-page-size {
  height: 34px;
  padding: 0 8px 0 10px;
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  background: var(--bg-secondary);
  color: var(--text-muted);
  font-size: 0.75rem;
  white-space: nowrap;
}

.shelf-page-size select {
  border: 0;
  outline: 0;
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  font: inherit;
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

.recent-page-next-enter-active,
.recent-page-next-leave-active,
.recent-page-previous-enter-active,
.recent-page-previous-leave-active {
  transition: opacity 160ms ease, transform 180ms cubic-bezier(.2, .7, .2, 1);
}

.recent-page-next-enter-from,
.recent-page-previous-leave-to {
  opacity: 0;
  transform: translateX(14px);
}

.recent-page-next-leave-to,
.recent-page-previous-enter-from {
  opacity: 0;
  transform: translateX(-14px);
}

@keyframes library-skeleton {
  from { background-position: 100% 0; }
  to { background-position: -100% 0; }
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

  .continue-skeleton__cover,
  .continue-skeleton__copy i,
  .continue-skeleton__progress {
    animation: none;
  }

  .recent-page-next-enter-active,
  .recent-page-next-leave-active,
  .recent-page-previous-enter-active,
  .recent-page-previous-leave-active {
    transition-duration: 1ms;
  }
}

@media (hover: none) {
  .recent-browser__nav:not(:disabled) {
    opacity: 0.8;
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

.shelf-pagination {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 5px;
  margin-top: 30px;
}

.shelf-pagination button {
  display: grid;
  width: 32px;
  height: 32px;
  place-items: center;
  border: 1px solid var(--border-color);
  border-radius: 4px;
  background: var(--bg-secondary);
  color: var(--text-secondary);
  cursor: pointer;
  font: 0.75rem/1 var(--font-ui);
  font-variant-numeric: tabular-nums;
}

.shelf-pagination button:hover:not(:disabled),
.shelf-pagination button:focus-visible {
  border-color: color-mix(in srgb, var(--accent-color) 58%, var(--border-color));
  color: var(--accent-color);
}

.shelf-pagination button.is-current {
  border-color: var(--accent-color);
  background: var(--accent-color);
  color: #fff;
}

.shelf-pagination button:disabled {
  opacity: 0.38;
  cursor: default;
}

.shelf-pagination__ellipsis {
  display: grid;
  width: 24px;
  height: 32px;
  place-items: center;
  color: var(--text-muted);
  font-size: 0.78rem;
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

  .all-books .section-heading {
    align-items: stretch;
    flex-direction: column;
    gap: 10px;
  }

  .shelf-tools {
    justify-content: stretch;
  }

  .shelf-search {
    flex: 1;
  }

  .shelf-search input {
    width: 100%;
  }

  .continue-book {
    gap: 10px;
    padding: 12px;
  }

  .continue-cover {
    width: 54px;
  }

  .continue-progress {
    width: 46px;
    height: 46px;
  }

  .recent-browser__nav {
    width: 48px;
    padding: 0 6px;
  }

  .book-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@media (max-width: 520px) {
  .continue-page {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
