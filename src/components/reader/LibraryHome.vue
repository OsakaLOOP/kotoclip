<script setup lang="ts">
import { computed, ref } from "vue";
import { BookOpen, Clock3, FileText, FileUp, FolderOpen, Search, Trash2 } from "@lucide/vue";
import type { LibraryBookSummary } from "../../reader/library";

const props = defineProps<{
  books: LibraryBookSummary[];
  loading: boolean;
  importing: boolean;
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
const filteredBooks = computed(() => {
  const query = searchQuery.value.trim().toLocaleLowerCase();
  if (!query) return props.books;
  return props.books.filter((book) =>
    [book.title, book.author, book.currentChapter, book.sourceName]
      .some((value) => value?.toLocaleLowerCase().includes(query))
  );
});

function progressLabel(book: LibraryBookSummary): string {
  const percent = Math.round(book.progressPercent * 100);
  return percent > 0 ? `${percent}%` : "未开始";
}

function dateLabel(value?: string | null): string {
  if (!value) return "尚未阅读";
  const date = new Date(value);
  return Number.isNaN(date.getTime())
    ? "最近读过"
    : new Intl.DateTimeFormat("zh-CN", { month: "short", day: "numeric" }).format(date);
}
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
      <div class="library-actions">
        <button class="secondary-command" type="button" @click="emit('input')">
          <FileText :size="17" aria-hidden="true" /> 粘贴 Markdown
        </button>
        <button class="primary-command" type="button" :disabled="importing" @click="emit('import')">
          <FileUp :size="17" aria-hidden="true" />
          {{ importing ? '正在导入…' : '导入 EPUB' }}
        </button>
      </div>
    </div>

    <p v-if="error" class="library-error" role="alert">{{ error }}</p>

    <div v-if="loading" class="library-loading" role="status">正在读取书架…</div>

    <section v-else-if="books.length === 0" class="empty-library">
      <BookOpen :size="42" stroke-width="1.4" aria-hidden="true" />
      <h2>书架还是空的</h2>
      <p>导入 EPUB 后，原书、清理后的 Markdown、图片和阅读进度会统一保存在上方书库目录。</p>
      <button class="primary-command" type="button" @click="emit('import')">
        <FileUp :size="17" aria-hidden="true" /> 导入第一本书
      </button>
    </section>

    <template v-else>
      <section v-if="books[0].lastOpenedAt" class="continue-section" aria-labelledby="continue-title">
        <div class="section-heading">
          <h2 id="continue-title">继续阅读</h2>
          <span>{{ books.length }} 本书</span>
        </div>
        <button class="continue-book" type="button" @click="emit('open', books[0].id)">
          <div class="continue-cover">
            <img v-if="props.coverUrl(books[0])" :src="props.coverUrl(books[0])" alt="" />
            <BookOpen v-else :size="30" aria-hidden="true" />
          </div>
          <div class="continue-copy">
            <strong>{{ books[0].title }}</strong>
            <span>{{ books[0].author }}</span>
            <span v-if="books[0].currentChapter" class="continue-chapter">{{ books[0].currentChapter }}</span>
          </div>
          <div class="continue-progress">
            <span>{{ progressLabel(books[0]) }}</span>
            <div><i :style="{ width: `${books[0].progressPercent * 100}%` }"></i></div>
          </div>
        </button>
      </section>

      <section class="all-books" aria-labelledby="all-books-title">
        <div class="section-heading">
          <h2 id="all-books-title">全部书籍</h2>
          <div class="shelf-tools">
            <label class="shelf-search">
              <Search :size="15" aria-hidden="true" />
              <input v-model="searchQuery" type="search" placeholder="搜索书名或作者" />
            </label>
            <button type="button" class="icon-command" title="导入 EPUB" @click="emit('import')">
              <FileUp :size="17" aria-hidden="true" />
            </button>
          </div>
        </div>
        <div class="book-grid">
          <article v-for="book in filteredBooks" :key="book.id" class="book-card">
            <button class="book-open" type="button" @click="emit('open', book.id)">
              <div class="book-cover">
                <img v-if="props.coverUrl(book)" :src="props.coverUrl(book)" alt="" />
                <BookOpen v-else :size="32" aria-hidden="true" />
              </div>
              <strong>{{ book.title }}</strong>
              <span class="book-author">{{ book.author }}</span>
              <div class="book-meta">
                <span><Clock3 :size="13" aria-hidden="true" />{{ dateLabel(book.lastOpenedAt) }}</span>
                <span>{{ progressLabel(book) }}</span>
              </div>
              <div class="book-progress"><i :style="{ width: `${book.progressPercent * 100}%` }"></i></div>
            </button>
            <button class="book-menu" type="button" title="从书架移除" @click="emit('remove', book)">
              <Trash2 :size="16" aria-hidden="true" />
              <span class="sr-only">移除 {{ book.title }}</span>
            </button>
          </article>
        </div>
        <p v-if="filteredBooks.length === 0" class="no-search-results">没有匹配的书籍</p>
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
.library-actions,
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

.library-actions {
  gap: 10px;
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

.primary-command,
.secondary-command,
.icon-command {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
  min-height: 38px;
  border-radius: var(--radius-sm);
  cursor: pointer;
}

.primary-command {
  padding: 8px 15px;
  border: 1px solid var(--accent-color);
  background: var(--accent-color);
  color: white;
}

.secondary-command,
.icon-command {
  padding: 8px 13px;
  border: 1px solid var(--border-color);
  background: var(--bg-secondary);
  color: var(--text-secondary);
}

.icon-command {
  width: 36px;
  padding: 0;
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
}

.continue-cover,
.book-cover {
  display: grid;
  place-items: center;
  flex: 0 0 auto;
  overflow: hidden;
  background: var(--accent-light);
  color: var(--accent-color);
  border: 1px solid var(--border-color);
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

.continue-copy {
  display: flex;
  flex: 1;
  min-width: 0;
  flex-direction: column;
}

.continue-copy strong {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 1rem;
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
  width: min(220px, 24vw);
  color: var(--text-secondary);
  text-align: right;
  font-size: 0.8rem;
}

.continue-progress div,
.book-progress {
  height: 3px;
  overflow: hidden;
  margin-top: 7px;
  background: var(--border-color);
}

.continue-progress i,
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

.book-cover {
  width: 100%;
  aspect-ratio: 3 / 4;
  margin-bottom: 10px;
  border-radius: 4px;
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

.empty-library,
.library-loading {
  display: flex;
  min-height: 52vh;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  color: var(--text-muted);
  text-align: center;
}

.empty-library h2 {
  margin-top: 16px;
  color: var(--text-primary);
  font-size: 1.1rem;
}

.empty-library p {
  max-width: 520px;
  margin: 7px 0 18px;
  font-size: 0.84rem;
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

  .library-actions > * {
    flex: 1;
  }

  .continue-progress {
    display: none;
  }

  .book-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
</style>
