<script setup lang="ts">
import { convertFileSrc, isTauri } from "@tauri-apps/api/core";
import { open as openFile } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { onBeforeUnmount, onMounted, ref } from "vue";
import LibraryHome from "./components/reader/LibraryHome.vue";
import ReaderDocumentView from "./components/reader/ReaderDocumentView.vue";
import ReaderSurface from "./components/reader/ReaderSurface.vue";
import { useDocumentSession } from "./composables/useDocumentSession";
import { compileReaderDocument, type ReaderDocument } from "./reader/document";
import { DEFAULT_READER_APPEARANCE, type ReaderAppearance } from "./reader/reading";
import type { LibraryBook, LibraryBookSummary } from "./reader/library";
import { readerRequest } from "./services/reader";
import type { DocumentPlan, DocumentUpdate } from "./reader/session";
import type { SavedSelection } from "./types/reader";
import "./styles/main.css";

type View = "library" | "reader";
type OpenBookResult = { book: LibraryBook; session: { plan: DocumentPlan; update: DocumentUpdate } };
type OpenTextResult = { plan: DocumentPlan; update: DocumentUpdate };

const view = ref<View>("library");
const books = ref<LibraryBookSummary[]>([]);
const libraryPath = ref("");
const libraryLoading = ref(true);
const libraryError = ref("");
const importing = ref(false);
const openingBookId = ref<string | null>(null);
const currentBook = ref<LibraryBook | null>(null);
const currentDocument = ref<ReaderDocument | null>(null);
const textInputOpen = ref(false);
const textDraft = ref("");
const textError = ref("");
const appearance = ref<ReaderAppearance>({ ...DEFAULT_READER_APPEARANCE });
const selections = ref<SavedSelection[]>([]);
const session = useDocumentSession();
const sessionState = session.state;

async function loadLibrary() {
  libraryLoading.value = true;
  libraryError.value = "";
  try {
    const result = await readerRequest<{ books: LibraryBookSummary[]; path: string }>("reader_library");
    books.value = result.books;
    libraryPath.value = result.path;
  } catch (error) {
    libraryError.value = error instanceof Error ? error.message : String(error);
  } finally {
    libraryLoading.value = false;
  }
}

async function closeSession() {
  await session.close();
  try { await readerRequest("reader_close"); } catch { /* 宿主退出时会话已结束 */ }
}

async function openBook(id: string) {
  openingBookId.value = id;
  libraryError.value = "";
  try {
    await session.close();
    const result = await readerRequest<OpenBookResult>("reader_open_book", { id });
    currentBook.value = result.book;
    currentDocument.value = compileReaderDocument(result.book.markdown);
    session.adopt(result.session.plan, result.session.update);
    await loadSelections(result.session.update.text_version, result.book.id);
    view.value = "reader";
  } catch (error) {
    libraryError.value = error instanceof Error ? error.message : String(error);
  } finally {
    openingBookId.value = null;
  }
}

function openTextInput() {
  textDraft.value = "";
  textError.value = "";
  textInputOpen.value = true;
}

async function startText() {
  if (!textDraft.value.trim()) {
    textError.value = "请输入正文";
    return;
  }
  textError.value = "";
  try {
    await session.close();
    const result = await readerRequest<OpenTextResult>("reader_open_text", { text: textDraft.value });
    currentBook.value = null;
    currentDocument.value = compileReaderDocument(textDraft.value);
    session.adopt(result.plan, result.update);
    await loadSelections(result.update.text_version, "__text__");
    textInputOpen.value = false;
    view.value = "reader";
  } catch (error) {
    textError.value = error instanceof Error ? error.message : String(error);
  }
}

async function loadSelections(version: string, bookId = currentBook.value?.id || "__text__") {
  try {
    selections.value = await readerRequest<SavedSelection[]>("reader_selections", { book: bookId, version });
  } catch {
    selections.value = [];
  }
}

async function saveSelection(draft: { start: number; end: number; surface: string; baseForm: string; reading: string }) {
  const version = sessionState.value?.text_version;
  if (!version) return;
  const selection: SavedSelection = {
    book_id: currentBook.value?.id || "__text__",
    text_version: version,
    start: draft.start,
    end: draft.end,
    surface: draft.surface,
    base_form: draft.baseForm,
    reading: draft.reading,
    note: "",
  };
  try {
    await readerRequest("reader_save_selection", { selection });
    await loadSelections(version, selection.book_id);
  } catch (error) {
    libraryError.value = error instanceof Error ? error.message : String(error);
  }
}

async function removeSelection(selection: SavedSelection) {
  try {
    await readerRequest("reader_delete_selection", { book: selection.book_id, version: selection.text_version, start: selection.start, end: selection.end });
    selections.value = selections.value.filter((item) => item.start !== selection.start || item.end !== selection.end);
  } catch (error) {
    libraryError.value = error instanceof Error ? error.message : String(error);
  }
}

async function clearSelections() {
  const version = sessionState.value?.text_version;
  if (!version) return;
  try {
    await readerRequest("reader_clear_selections", { book: currentBook.value?.id || "__text__", version });
    selections.value = [];
  } catch (error) {
    libraryError.value = error instanceof Error ? error.message : String(error);
  }
}

async function updateSelection(selection: SavedSelection, note: string) {
  try {
    await readerRequest("reader_save_selection", { selection: { ...selection, note } });
    selections.value = selections.value.map((item) => item.start === selection.start && item.end === selection.end ? { ...item, note } : item);
  } catch (error) {
    libraryError.value = error instanceof Error ? error.message : String(error);
  }
}

function exportSelections() {
  const payload = JSON.stringify({ title: currentBook.value?.title || "文本阅读", text_version: sessionState.value?.text_version || "", selections: selections.value }, null, 2);
  const link = document.createElement("a");
  link.href = URL.createObjectURL(new Blob([payload], { type: "application/json;charset=utf-8" }));
  link.download = `${currentBook.value?.title || "kotoclip-selections"}.json`;
  link.click();
  URL.revokeObjectURL(link.href);
}

async function importEpub() {
  if (!isTauri()) {
    libraryError.value = "导入 EPUB 需要在 Kotoclip 桌面窗口中运行";
    return;
  }
  const selected = await openFile({ multiple: false, filters: [{ name: "EPUB", extensions: ["epub"] }] });
  const path = Array.isArray(selected) ? selected[0] : selected;
  if (!path) return;
  importing.value = true;
  libraryError.value = "";
  try {
    const book = await readerRequest<LibraryBook>("reader_import", { path });
    await loadLibrary();
    await openBook(book.id);
  } catch (error) {
    libraryError.value = error instanceof Error ? error.message : String(error);
  } finally {
    importing.value = false;
  }
}

async function revealLibrary() {
  if (libraryPath.value) await revealItemInDir(libraryPath.value);
}

async function revealBook(book: LibraryBookSummary) {
  if (book.coverPath) await revealItemInDir(book.coverPath);
}

async function updateOrganization(book: LibraryBookSummary, accentColor: string | null, tags: string[]) {
  try {
    await readerRequest("reader_organize", { id: book.id, color: accentColor, tags });
    await loadLibrary();
  } catch (error) {
    libraryError.value = error instanceof Error ? error.message : String(error);
  }
}

function handleOrganization(book: LibraryBookSummary, accentColor: string | null, tags: string[]) {
  void updateOrganization(book, accentColor, tags);
}

async function resetProgress(book: LibraryBookSummary) {
  try {
    await readerRequest("reader_reset", { id: book.id });
    await loadLibrary();
  } catch (error) {
    libraryError.value = error instanceof Error ? error.message : String(error);
  }
}

async function removeBook(book: LibraryBookSummary) {
  if (!window.confirm(`确认删除《${book.title}》？`)) return;
  try {
    await readerRequest("reader_remove", { id: book.id });
    await loadLibrary();
  } catch (error) {
    libraryError.value = error instanceof Error ? error.message : String(error);
  }
}

function coverUrl(book: LibraryBookSummary) {
  return book.coverPath && isTauri() ? convertFileSrc(book.coverPath) : undefined;
}

async function saveProgress(offset: number, chapter: string | null, seconds: number) {
  if (!currentBook.value) return;
  try {
    const summary = await readerRequest<LibraryBookSummary>("reader_progress", {
      id: currentBook.value.id,
      offset,
      total: currentDocument.value ? Array.from(currentDocument.value.analysisText).length : currentBook.value.totalCharacters,
      chapter,
      seconds,
    });
    currentBook.value = { ...currentBook.value, ...summary };
  } catch (error) {
    libraryError.value = error instanceof Error ? error.message : String(error);
  }
}

async function leaveReader() {
  await closeSession();
  currentBook.value = null;
  currentDocument.value = null;
  selections.value = [];
  view.value = "library";
  await loadLibrary();
}

function appearanceUpdated(value: ReaderAppearance) {
  appearance.value = value;
  localStorage.setItem("kotoclip.reader.appearance", JSON.stringify(value));
}

onMounted(() => {
  try {
    const saved = localStorage.getItem("kotoclip.reader.appearance");
    if (saved) appearance.value = { ...DEFAULT_READER_APPEARANCE, ...JSON.parse(saved) };
  } catch { /* 使用默认排版 */ }
  void loadLibrary();
});

onBeforeUnmount(() => { void closeSession(); });
</script>

<template>
  <div class="reader-app">
    <LibraryHome
      v-if="view === 'library'"
      :books="books"
      :loading="libraryLoading"
      :importing="importing"
      :opening-book-id="openingBookId"
      :error="libraryError || null"
      :library-path="libraryPath"
      :cover-url="coverUrl"
      @import="void importEpub()"
      @open="void openBook($event)"
      @input="openTextInput"
      @reveal="void revealLibrary()"
      @reveal-book="void revealBook($event)"
      @remove="void removeBook($event)"
      @update-organization="handleOrganization"
      @reset-progress="void resetProgress($event)"
    />

    <ReaderDocumentView
      v-else-if="currentDocument && sessionState"
      :reader-document="currentDocument"
      :session="sessionState"
      :book="currentBook"
      :resources="currentBook?.resources || []"
      :appearance="appearance"
      :initial-offset="currentBook?.progressOffset || 0"
      :selections="selections"
      @back="void leaveReader()"
      @progress="saveProgress"
      @update-appearance="appearanceUpdated"
      @continue="void session.control('continue_document')"
      @retry="void session.control('retry_document', { unit_id: $event || null })"
      @save-selection="saveSelection"
      @remove-selection="removeSelection"
      @clear-selections="clearSelections"
      @update-selection="updateSelection"
      @export-selections="exportSelections"
    />

    <ReaderSurface :show="textInputOpen" variant="modal" title="打开文本" @close="textInputOpen = false">
      <div class="text-input-panel">
        <textarea v-model="textDraft" autofocus placeholder="粘贴日文正文或 Markdown" />
        <p v-if="textError" class="text-input-panel__error">{{ textError }}</p>
        <button type="button" @click="void startText()">开始阅读</button>
      </div>
    </ReaderSurface>
  </div>
</template>

<style scoped>
.reader-app { min-height: 100vh; height: 100vh; overflow: hidden; background: var(--bg-primary); }
.text-input-panel { display: flex; gap: 12px; padding: 6px 18px 20px; flex-direction: column; }
.text-input-panel textarea { min-height: 260px; resize: vertical; padding: 12px; border: 1px solid var(--border-color); border-radius: var(--radius-sm); background: var(--bg-secondary); color: var(--text-primary); font: 1rem/1.7 var(--font-ja); }
.text-input-panel button { align-self: flex-end; padding: 8px 15px; border: 0; border-radius: var(--radius-sm); background: var(--accent-color); color: #fff; cursor: pointer; }
.text-input-panel__error { color: var(--novelty-high-text); font-size: .8rem; }
</style>
