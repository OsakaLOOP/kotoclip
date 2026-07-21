<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import {
  BookOpen,
  BookOpenText,
  ChevronLeft,
  ChevronRight,
  Clock3,
  FileText,
  FileUp,
  FolderOpen,
  Info,
  MoreHorizontal,
  Palette,
  RotateCcw,
  Search,
  Tags,
  Trash2,
} from "@lucide/vue";
import type { LibraryBookSummary } from "../../reader/library";
import SegmentedActionFrame from "../common/SegmentedActionFrame.vue";

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
  revealBook: [book: LibraryBookSummary];
  updateOrganization: [book: LibraryBookSummary, accentColor: string | null, tags: string[]];
  resetProgress: [book: LibraryBookSummary];
}>();
const searchQuery = ref("");
const importActions = computed(() => [
  {
    id: "markdown",
    label: "粘贴 Markdown",
    description: "从纯文本开始阅读",
    icon: FileText,
    disabled: props.importing || Boolean(props.openingBookId),
    theme: { color: "#4d79b8", textColor: "#28486f" },
  },
  {
    id: "epub",
    label: props.importing ? "正在导入…" : "导入 EPUB",
    description: "保留封面、章节与插图",
    icon: FileUp,
    disabled: props.importing || Boolean(props.openingBookId),
    theme: { color: "#b9ce32", textColor: "#52620f" },
  },
]);
type ShelfSort = "recent" | "title" | "progress" | "added";
const shelfSort = ref<ShelfSort>("recent");
const contextMenu = ref<{ book: LibraryBookSummary; x: number; y: number } | null>(null);
const contextMenuElement = ref<HTMLElement | null>(null);
const detailsBook = ref<LibraryBookSummary | null>(null);
const tagDraft = ref("");
const menuColor = ref<string | null>(null);
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
      .some((value) => value?.toLocaleLowerCase().includes(query)) ||
    book.tags.some((tag) => tag.toLocaleLowerCase().includes(query))
  );
});

const sortedBooks = computed(() => [...filteredBooks.value].sort((left, right) => {
  if (shelfSort.value === "title") return left.title.localeCompare(right.title, "zh-CN");
  if (shelfSort.value === "progress") return right.progressPercent - left.progressPercent;
  if (shelfSort.value === "added") return new Date(right.createdAt).getTime() - new Date(left.createdAt).getTime();
  return openedAtTimestamp(right) - openedAtTimestamp(left) || new Date(right.createdAt).getTime() - new Date(left.createdAt).getTime();
}));

const shelfTotalPages = computed(() => Math.max(1, Math.ceil(sortedBooks.value.length / shelfPageSize.value)));
const visibleShelfBooks = computed(() => {
  const start = shelfPage.value * shelfPageSize.value;
  const end = Math.min(start + shelfPageSize.value, sortedBooks.value.length);
  return sortedBooks.value.slice(start, end);
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
  return openingSurfaceTarget.value === rippleTarget(surface, bookId);
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

function handleImportAction(id: string) {
  if (id === "markdown") emit("input");
  if (id === "epub") emit("import");
}

function openBookContextMenu(event: MouseEvent, book: LibraryBookSummary) {
  event.preventDefault();
  event.stopPropagation();
  const menuWidth = 276;
  const menuHeight = 420;
  const trigger = event.currentTarget as HTMLElement | null;
  const triggerBounds = trigger?.getBoundingClientRect();
  const requestedX = event.clientX || triggerBounds?.right || 10;
  const requestedY = event.clientY || triggerBounds?.top || 10;
  contextMenu.value = {
    book,
    x: Math.max(10, Math.min(requestedX, window.innerWidth - menuWidth - 10)),
    y: Math.max(10, Math.min(requestedY, window.innerHeight - menuHeight - 10)),
  };
  menuColor.value = book.accentColor ?? null;
  tagDraft.value = book.tags.join(", ");
  void nextTick(() => contextMenuElement.value?.querySelector<HTMLElement>("[role='menuitem']")?.focus());
}

function closeBookContextMenu() {
  contextMenu.value = null;
  menuColor.value = null;
}

function handleDocumentPointerDown(event: PointerEvent) {
  const target = event.target as HTMLElement | null;
  if (!target?.closest(".book-context-menu")) closeBookContextMenu();
}

function handleDocumentKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    closeBookContextMenu();
    detailsBook.value = null;
  }
}

function handleMenuKeydown(event: KeyboardEvent) {
  if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return;
  const items = [...(contextMenuElement.value?.querySelectorAll<HTMLElement>("[role='menuitem']") ?? [])];
  if (!items.length) return;
  event.preventDefault();
  const current = items.indexOf(document.activeElement as HTMLElement);
  const next = event.key === "Home" ? 0
    : event.key === "End" ? items.length - 1
      : event.key === "ArrowDown" ? (current + 1 + items.length) % items.length
        : (current - 1 + items.length) % items.length;
  items[next].focus();
}

function showDetails(book: LibraryBookSummary) {
  detailsBook.value = book;
  closeBookContextMenu();
}

function submitOrganization(book: LibraryBookSummary, color: string | null = menuColor.value) {
  const tags = tagDraft.value.split(/[，,\n]/).map((tag) => tag.trim().replace(/^#/, "")).filter(Boolean);
  emit("updateOrganization", book, color, [...new Set(tags)].slice(0, 12));
}

function selectColor(book: LibraryBookSummary, color: string | null) {
  menuColor.value = color;
  submitOrganization(book, color);
}

function requestRemove(book: LibraryBookSummary) {
  closeBookContextMenu();
  emit("remove", book);
}

function requestReset(book: LibraryBookSummary) {
  closeBookContextMenu();
  emit("resetProgress", book);
}

function requestReveal(book: LibraryBookSummary) {
  closeBookContextMenu();
  emit("revealBook", book);
}

function openFromMenu(book: LibraryBookSummary) {
  closeBookContextMenu();
  openingSurfaceTarget.value = rippleTarget("shelf", book.id);
  emit("open", book.id);
}

function openFromDetails(book: LibraryBookSummary) {
  detailsBook.value = null;
  openingSurfaceTarget.value = rippleTarget("shelf", book.id);
  emit("open", book.id);
}

onMounted(() => {
  document.addEventListener("pointerdown", handleDocumentPointerDown);
  document.addEventListener("keydown", handleDocumentKeydown);
});

onBeforeUnmount(() => {
  if (rippleTimer !== undefined) window.clearTimeout(rippleTimer);
  document.removeEventListener("pointerdown", handleDocumentPointerDown);
  document.removeEventListener("keydown", handleDocumentKeydown);
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

watch(shelfSort, () => {
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
  <main class="library-home" @contextmenu.prevent>
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
                @contextmenu="openBookContextMenu($event, book)"
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
              <input v-model="searchQuery" type="search" placeholder="搜索书名、作者或标签" />
            </label>
            <label class="shelf-sort">
              <span>排列</span>
              <select v-model="shelfSort" aria-label="排列书籍">
                <option value="recent">最近阅读</option>
                <option value="title">书名</option>
                <option value="progress">阅读进度</option>
                <option value="added">导入时间</option>
              </select>
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
          <SegmentedActionFrame
            v-if="showImportCard"
            class="shelf-import-card"
            :actions="importActions"
            min-height="328px"
            aria-label="添加阅读内容"
            @select="handleImportAction"
          />
          <article
            v-for="book in visibleShelfBooks"
            :key="book.id"
            class="book-card"
            :class="book.accentColor ? `book-card--${book.accentColor}` : undefined"
            @contextmenu="openBookContextMenu($event, book)"
          >
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
              <div v-if="book.tags.length" class="book-tags" aria-label="标签">
                <span v-for="tag in book.tags.slice(0, 2)" :key="tag">#{{ tag }}</span>
              </div>
              <div class="book-progress"><i :style="{ width: `${book.progressPercent * 100}%` }"></i></div>
            </button>
            <button
              class="book-menu"
              type="button"
              title="书本操作"
              aria-label="打开书本操作菜单"
              :disabled="Boolean(openingBookId)"
              @click="openBookContextMenu($event, book)"
            >
              <MoreHorizontal :size="17" aria-hidden="true" />
            </button>
          </article>
        </div>
        <p v-if="searchQuery && sortedBooks.length === 0" class="no-search-results">没有匹配的书籍</p>
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

    <Transition name="book-context-fade">
      <div
        v-if="contextMenu"
        ref="contextMenuElement"
        class="book-context-menu"
        :style="{ left: `${contextMenu.x}px`, top: `${contextMenu.y}px` }"
        role="menu"
        :aria-label="`${contextMenu.book.title} 的操作`"
        @pointerdown.stop
        @keydown="handleMenuKeydown"
      >
        <div class="book-context-menu__heading">
          <strong>{{ contextMenu.book.title }}</strong>
          <small>{{ contextMenu.book.author }}</small>
        </div>
        <button class="book-context-menu__item" type="button" role="menuitem" @click="openFromMenu(contextMenu.book)">
          <BookOpen :size="16" aria-hidden="true" />打开阅读
        </button>
        <button class="book-context-menu__item" type="button" role="menuitem" @click="showDetails(contextMenu.book)">
          <Info :size="16" aria-hidden="true" />查看详情
        </button>
        <button class="book-context-menu__item" type="button" role="menuitem" @click="requestReveal(contextMenu.book)">
          <FolderOpen :size="16" aria-hidden="true" />在文件夹中显示
        </button>
        <div class="book-context-menu__rule"></div>
        <div class="book-context-menu__label"><Palette :size="14" aria-hidden="true" />颜色标记</div>
        <div class="book-color-picker" role="group" aria-label="颜色标记">
          <button
            v-for="color in ['red', 'amber', 'lime', 'teal', 'blue', 'violet']"
            :key="color"
            type="button"
            class="book-color-swatch"
            :class="[`book-color-swatch--${color}`, { 'is-selected': menuColor === color }]"
            :aria-label="`${color}标记`"
            @click="selectColor(contextMenu.book, color)"
          ></button>
          <button type="button" class="book-color-swatch book-color-swatch--clear" aria-label="清除颜色标记" @click="selectColor(contextMenu.book, null)">×</button>
        </div>
        <div class="book-context-menu__label"><Tags :size="14" aria-hidden="true" />标签分类</div>
        <div class="book-tag-editor">
          <input v-model="tagDraft" type="text" placeholder="输入标签，用逗号分隔" @keydown.enter.prevent="submitOrganization(contextMenu.book)" />
          <button type="button" @click="submitOrganization(contextMenu.book)">保存</button>
        </div>
        <div class="book-context-menu__rule"></div>
        <button class="book-context-menu__item" type="button" role="menuitem" @click="requestReset(contextMenu.book)">
          <RotateCcw :size="16" aria-hidden="true" />重置阅读进度
        </button>
        <button class="book-context-menu__item book-context-menu__item--danger" type="button" role="menuitem" @click="requestRemove(contextMenu.book)">
          <Trash2 :size="16" aria-hidden="true" />从书架删除
        </button>
      </div>
    </Transition>

    <Transition name="book-details-slide">
      <aside v-if="detailsBook" class="book-details" role="dialog" aria-modal="true" aria-label="书籍详情">
        <button class="book-details__close" type="button" aria-label="关闭详情" @click="detailsBook = null">×</button>
        <div class="book-details__cover">
          <img v-if="props.coverUrl(detailsBook)" :src="props.coverUrl(detailsBook)" alt="" />
          <BookOpen v-else :size="30" aria-hidden="true" />
        </div>
        <p class="book-details__eyebrow">书籍详情</p>
        <h2>{{ detailsBook.title }}</h2>
        <p class="book-details__author">{{ detailsBook.author || '未知作者' }}</p>
        <dl class="book-details__stats">
          <div><dt>章节</dt><dd>{{ detailsBook.chapterCount }}</dd></div>
          <div><dt>字数</dt><dd>{{ detailsBook.totalCharacters.toLocaleString('zh-CN') }}</dd></div>
          <div><dt>进度</dt><dd>{{ progressLabel(detailsBook) }}</dd></div>
          <div><dt>格式</dt><dd>{{ detailsBook.sourceName.split('.').pop()?.toUpperCase() || 'EPUB' }}</dd></div>
        </dl>
        <div class="book-details__section">
          <span>标签</span>
          <div v-if="detailsBook.tags.length" class="book-details__tags"><span v-for="tag in detailsBook.tags" :key="tag">#{{ tag }}</span></div>
          <em v-else>尚未分类</em>
        </div>
        <button class="book-details__action" type="button" @click="openFromDetails(detailsBook)">打开阅读</button>
      </aside>
    </Transition>
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
.shelf-sort,
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
.shelf-sort,
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
  background: transparent;
  color: var(--text-muted);
}

.shelf-sort {
  height: 34px;
  gap: 6px;
  padding: 0 9px;
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-muted);
  font-size: 0.75rem;
  white-space: nowrap;
}

.shelf-sort select {
  max-width: 112px;
  border: 0;
  outline: 0;
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  font: inherit;
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
  margin-top: auto;
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
  align-items: stretch;
}

.book-card {
  position: relative;
  min-width: 0;
  min-height: 328px;
  display: flex;
}

.shelf-import-card {
  min-width: 0;
  min-height: 328px;
}

.shelf-page-size {
  height: 34px;
  padding: 0 8px 0 10px;
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  background: transparent;
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
  height: 100%;
}

.book-open strong {
  transition: color 140ms ease;
}

.book-cover {
  width: 100%;
  aspect-ratio: 3 / 4;
  margin-bottom: 10px;
  border-radius: 4px;
  background: transparent;
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

.book-menu:hover,
.book-menu:focus-visible {
  color: var(--accent-color);
  transform: translateY(-2px);
}

.book-menu:disabled {
  opacity: 0.45;
  cursor: default;
}

.book-tags {
  display: flex;
  gap: 4px;
  min-width: 0;
  margin-top: 6px;
  overflow: hidden;
}

.book-tags span {
  max-width: 82px;
  overflow: hidden;
  padding: 1px 5px;
  border: 1px solid color-mix(in srgb, var(--accent-color) 28%, var(--border-color));
  border-radius: 999px;
  color: var(--text-muted);
  font-size: .62rem;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.book-card--red .book-cover { border-color: #d87878; }
.book-card--amber .book-cover { border-color: #d39a4b; }
.book-card--lime .book-cover { border-color: #a2b329; }
.book-card--teal .book-cover { border-color: #43a6a0; }
.book-card--blue .book-cover { border-color: #5685cf; }
.book-card--violet .book-cover { border-color: #9270c5; }

.book-context-menu {
  position: fixed;
  z-index: 1200;
  width: min(276px, calc(100vw - 20px));
  padding: 8px;
  border: 1px solid color-mix(in srgb, var(--accent-color) 24%, var(--border-color));
  border-radius: 10px;
  background: color-mix(in srgb, var(--bg-primary) 96%, transparent);
  box-shadow: 0 14px 34px color-mix(in srgb, var(--text-primary) 13%, transparent);
  backdrop-filter: blur(4px);
}

.book-context-menu__heading {
  display: grid;
  gap: 1px;
  padding: 7px 9px 8px;
  border-bottom: 1px solid var(--border-color);
}

.book-context-menu__heading strong,
.book-context-menu__heading small {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.book-context-menu__heading strong { font-size: .78rem; }
.book-context-menu__heading small { color: var(--text-muted); font-size: .68rem; }

.book-context-menu__item {
  display: flex;
  width: 100%;
  align-items: center;
  gap: 9px;
  min-height: 34px;
  padding: 6px 9px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  font: inherit;
  font-size: .76rem;
  text-align: left;
  transition: color 140ms ease, transform 140ms ease;
}

.book-context-menu__item:hover,
.book-context-menu__item:focus-visible {
  color: var(--accent-color);
  outline: none;
  transform: translateX(3px);
}

.book-context-menu__item--danger:hover,
.book-context-menu__item--danger:focus-visible { color: var(--novelty-high-text); }
.book-context-menu__rule { height: 1px; margin: 6px 4px; background: var(--border-color); }
.book-context-menu__label { display: flex; align-items: center; gap: 6px; padding: 5px 9px 4px; color: var(--text-muted); font-size: .68rem; }

.book-color-picker { display: flex; gap: 8px; padding: 2px 9px 8px; }
.book-color-swatch {
  display: grid;
  width: 21px;
  height: 21px;
  place-items: center;
  border: 2px solid transparent;
  border-radius: 50%;
  cursor: pointer;
  transition: transform 140ms ease, border-color 140ms ease;
}
.book-color-swatch:hover, .book-color-swatch:focus-visible { outline: none; transform: scale(1.18); }
.book-color-swatch.is-selected { border-color: var(--text-primary); box-shadow: 0 0 0 2px var(--bg-primary); }
.book-color-swatch--red { background: #d87878; }
.book-color-swatch--amber { background: #d39a4b; }
.book-color-swatch--lime { background: #a8b82f; }
.book-color-swatch--teal { background: #43a6a0; }
.book-color-swatch--blue { background: #5685cf; }
.book-color-swatch--violet { background: #9270c5; }
.book-color-swatch--clear { border-color: var(--border-color); background: transparent; color: var(--text-muted); font-size: .85rem; }

.book-tag-editor { display: flex; gap: 5px; padding: 2px 9px 7px; }
.book-tag-editor input { min-width: 0; flex: 1; height: 29px; padding: 0 7px; border: 1px solid var(--border-color); border-radius: 5px; outline: 0; background: transparent; color: var(--text-primary); font: inherit; font-size: .7rem; }
.book-tag-editor input:focus { border-color: var(--accent-color); }
.book-tag-editor button { border: 0; background: transparent; color: var(--accent-color); cursor: pointer; font: inherit; font-size: .7rem; }

.book-details {
  position: fixed;
  z-index: 1190;
  top: 0;
  right: 0;
  display: flex;
  width: min(360px, 100vw);
  height: 100%;
  flex-direction: column;
  gap: 8px;
  overflow-y: auto;
  padding: 32px 26px 30px;
  border-left: 1px solid var(--border-color);
  background: color-mix(in srgb, var(--bg-primary) 97%, transparent);
  box-shadow: -14px 0 36px color-mix(in srgb, var(--text-primary) 10%, transparent);
  backdrop-filter: blur(3px);
}
.book-details__close { position: absolute; top: 14px; right: 16px; border: 0; background: transparent; color: var(--text-muted); cursor: pointer; font-size: 1.4rem; line-height: 1; }
.book-details__cover { width: 142px; aspect-ratio: 3 / 4; display: grid; place-items: center; overflow: hidden; margin-bottom: 14px; border: 1px solid var(--border-color); border-radius: 5px; background: transparent; color: var(--accent-color); }
.book-details__cover img { width: 100%; height: 100%; object-fit: cover; }
.book-details__eyebrow { color: var(--accent-color); font-size: .68rem; font-weight: 700; letter-spacing: .08em; }
.book-details h2 { font-size: 1.3rem; line-height: 1.35; overflow-wrap: anywhere; }
.book-details__author { color: var(--text-muted); font-size: .8rem; }
.book-details__stats { display: grid; grid-template-columns: repeat(4, 1fr); gap: 7px; margin: 15px 0 10px; }
.book-details__stats div { display: grid; gap: 2px; padding-top: 7px; border-top: 1px solid var(--border-color); }
.book-details__stats dt { color: var(--text-muted); font-size: .66rem; }
.book-details__stats dd { color: var(--text-secondary); font-size: .78rem; font-variant-numeric: tabular-nums; }
.book-details__section { display: grid; gap: 7px; margin-top: 8px; color: var(--text-muted); font-size: .72rem; }
.book-details__section em { color: var(--text-muted); font-style: normal; }
.book-details__tags { display: flex; flex-wrap: wrap; gap: 5px; }
.book-details__tags span { padding: 3px 7px; border: 1px solid color-mix(in srgb, var(--accent-color) 28%, var(--border-color)); border-radius: 999px; color: var(--text-secondary); font-size: .68rem; }
.book-details__action { margin-top: 18px; padding: 10px 12px; border: 1px solid var(--accent-color); border-radius: 6px; background: transparent; color: var(--accent-color); cursor: pointer; font: inherit; font-size: .78rem; }
.book-details__action:hover, .book-details__action:focus-visible { background: var(--accent-light); outline: none; }

.book-context-fade-enter-active, .book-context-fade-leave-active { transition: opacity 130ms ease, transform 130ms ease; }
.book-context-fade-enter-from, .book-context-fade-leave-to { opacity: 0; transform: translateY(-4px) scale(.98); }
.book-details-slide-enter-active, .book-details-slide-leave-active { transition: transform 180ms cubic-bezier(.2,.8,.2,1), opacity 180ms ease; }
.book-details-slide-enter-from, .book-details-slide-leave-to { opacity: 0; transform: translateX(24px); }

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
  background: transparent;
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
  background: transparent;
  color: var(--accent-color);
  font-weight: 800;
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
