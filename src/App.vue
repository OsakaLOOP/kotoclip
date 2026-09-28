<script setup lang="ts">
import { convertFileSrc, isTauri } from "@tauri-apps/api/core";
import { open as openFile } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { BookMarked } from "@lucide/vue";
import AnalysisProgressPanel from "./components/AnalysisProgressPanel.vue";
import AppHeader from "./components/common/AppHeader.vue";
import LibraryHome from "./components/reader/LibraryHome.vue";
import ReaderDocumentView from "./components/reader/ReaderDocumentView.vue";
import ReaderOpeningMark from "./components/reader/ReaderOpeningMark.vue";
import ReaderSurface from "./components/reader/ReaderSurface.vue";
import { useDocumentSession } from "./composables/useDocumentSession";
import { compileReaderDocumentAsync, type ReaderDocument } from "./reader/document";
import { DEFAULT_READER_APPEARANCE, type ReaderAppearance } from "./reader/reading";
import type { LibraryBook, LibraryBookSummary } from "./reader/library";
import { readerRequest } from "./services/reader";
import { nlpRequest } from "./services/nlp";
import type { SavedSelection } from "./types/reader";
import { readerAnalysisProgress } from "./reader/readerProgress";
import { createViewTransitionGuard, type ManagedViewTransition } from "./reader/viewTransition";
import "./styles/main.css";

type View = "library" | "reader";

const view = ref<View>("library");
const books = ref<LibraryBookSummary[]>([]);
const libraryPath = ref("");
const libraryLoading = ref(true);
const libraryError = ref("");
const importing = ref(false);
const openingBookId = ref<string | null>(null);
const currentBook = ref<LibraryBook | null>(null);
const currentDocument = ref<ReaderDocument | null>(null);
const readerPreparing = ref(false);
const textInputOpen = ref(false);
const textDraft = ref("");
const textError = ref("");
const appearance = ref<ReaderAppearance>({ ...DEFAULT_READER_APPEARANCE });
const selections = ref<SavedSelection[]>([]);
const session = useDocumentSession();
const sessionState = session.state;
const readerProgress = computed(() => readerAnalysisProgress(sessionState.value, readerPreparing.value));
const readerEntered = ref(false);
const timingEnabled = ref(true);
const timingSaving = ref(false);
const readerReady = computed(() => {
  const state = sessionState.value;
  const offset = Math.min(currentBook.value?.progressOffset || 0, Math.max(0, (state?.plan.prepared.mapping.origins.length || 1) - 1));
  return Boolean(currentDocument.value && state && (readerEntered.value || state.plan.units.some((unit) =>
    unit.anchor.char_range[0] <= offset && offset < unit.anchor.char_range[1]
      && state.units[unit.id]?.stage === "complete" && state.units[unit.id]?.document)));
});
watch(readerReady, (ready) => { if (ready) readerEntered.value = true; });
const readerAnalysisActive = computed(() => view.value === "reader" && (!readerReady.value || readerProgress.value.phase !== "completed"));
const readerViewTransition = createViewTransitionGuard();
let openGeneration = 0;

async function toggleTiming() {
  timingSaving.value = true;
  try {
    const result = await nlpRequest<{ analysis_timing_enabled: boolean }>({ command: "set_analysis_timing", enabled: !timingEnabled.value });
    timingEnabled.value = result.analysis_timing_enabled;
  } catch (error) {
    libraryError.value = error instanceof Error ? error.message : String(error);
  } finally {
    timingSaving.value = false;
  }
}

function exportTiming() {
  const state = sessionState.value;
  if (!state) return;
  const units = state.plan.units.map((plan) => ({
    unit_id: plan.id, char_range: plan.anchor.char_range,
    stage: state.units[plan.id]?.stage, cache_hit: state.units[plan.id]?.cache_hit, timing: state.units[plan.id]?.timing,
    providers: state.units[plan.id]?.providers,
  }));
  const payload = JSON.stringify({ document_id: state.plan.id, text_version: state.text_version, progress: state.progress, units }, null, 2);
  const link = document.createElement("a");
  link.href = URL.createObjectURL(new Blob([payload], { type: "application/json" }));
  link.download = `kotoclip-analysis-${state.plan.id.replace(/[^\w-]/g, "-")}.json`;
  link.click();
  window.setTimeout(() => URL.revokeObjectURL(link.href), 1000);
}

function waitForOpening(milliseconds: number): Promise<void> {
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches
    ? Promise.resolve()
    : new Promise((resolve) => window.setTimeout(resolve, milliseconds));
}

async function transitionToReader() {
  const transitionDocument = document as Document & { startViewTransition?: (update: () => Promise<void>) => ManagedViewTransition };
  if (!transitionDocument.startViewTransition || window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
    view.value = "reader";
    await nextTick();
    return;
  }
  const transition = transitionDocument.startViewTransition(async () => {
    view.value = "reader";
    await nextTick();
  });
  readerViewTransition.track(transition);
  await transition.updateCallbackDone;
}

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
  try {
    await session.close();
  } finally {
    try { await readerRequest("reader_close"); } catch { /* 宿主退出时会话已结束 */ }
  }
}

async function openBook(id: string) {
  const generation = ++openGeneration;
  openingBookId.value = id;
  libraryError.value = "";
  try {
    await session.close();
    readerEntered.value = false;
    const [book] = await Promise.all([
      readerRequest<LibraryBook>("reader_open_book", { id }),
      waitForOpening(300),
    ]);
    if (generation !== openGeneration) return;
    currentBook.value = book;
    currentDocument.value = null;
    readerPreparing.value = true;
    await transitionToReader();
    const [compiled] = await Promise.all([compileReaderDocumentAsync(book.markdown), waitForOpening(320)]);
    if (generation !== openGeneration) return;
    currentDocument.value = compiled;
    await session.open(currentDocument.value.analysisText, "auto", book.id, book.progressOffset);
    if (generation !== openGeneration) { await session.close(); return; }
    if (!sessionState.value) throw new Error(session.error.value || "文档分析启动失败");
    void session.control("continue_document");
    void loadSelections(sessionState.value.text_version, book.id);
    readerPreparing.value = false;
  } catch (error) {
    if (generation === openGeneration) {
      readerPreparing.value = false;
      view.value = "library";
      libraryError.value = error instanceof Error ? error.message : String(error);
    }
  } finally {
    if (generation === openGeneration) openingBookId.value = null;
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
    readerEntered.value = false;
    const inputText = textDraft.value;
    currentBook.value = null;
    currentDocument.value = null;
    textInputOpen.value = false;
    view.value = "reader";
    readerPreparing.value = true;
    currentDocument.value = await compileReaderDocumentAsync(inputText);
    await session.open(currentDocument.value.analysisText, "auto");
    if (!sessionState.value) throw new Error(session.error.value || "文档分析启动失败");
    void session.control("continue_document");
    void loadSelections(sessionState.value.text_version, "__text__");
    readerPreparing.value = false;
  } catch (error) {
    readerPreparing.value = false;
    textInputOpen.value = true;
    view.value = "library";
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
    readerPreparing.value = false;
    view.value = "library";
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
    readerPreparing.value = false;
    view.value = "library";
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
    readerPreparing.value = false;
    view.value = "library";
    libraryError.value = error instanceof Error ? error.message : String(error);
  }
}

async function removeBook(book: LibraryBookSummary) {
  if (!window.confirm(`确认删除《${book.title}》？`)) return;
  try {
    await readerRequest("reader_remove", { id: book.id });
    await loadLibrary();
  } catch (error) {
    readerPreparing.value = false;
    view.value = "library";
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
    readerPreparing.value = false;
    view.value = "library";
    libraryError.value = error instanceof Error ? error.message : String(error);
  }
}

async function leaveReader() {
  ++openGeneration;
  await readerViewTransition.finish();
  readerPreparing.value = false;
  openingBookId.value = null;
  currentBook.value = null;
  currentDocument.value = null;
  selections.value = [];
  view.value = "library";
  let closeError = "";
  try { await closeSession(); }
  catch (error) { closeError = error instanceof Error ? error.message : String(error); }
  await loadLibrary();
  if (closeError) libraryError.value = closeError;
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
  void nlpRequest<{ settings: { analysis_timing_enabled: boolean } }>({ command: "provider_status" })
    .then((result) => { timingEnabled.value = result.settings.analysis_timing_enabled; })
    .catch((error) => { libraryError.value = String(error); });
});

onBeforeUnmount(() => { readerViewTransition.dispose(); void closeSession(); });
</script>

<template>
  <div class="reader-app">
    <AppHeader v-if="view === 'library'" description="日语生肉阅读助手" />
    <AppHeader v-else-if="!readerReady" show-back collapse-brand back-label="返回书架" :title="currentBook?.title || '文本阅读'" :description="currentDocument?.metadata.author || ''" @back="void leaveReader()" />
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

    <Transition name="analysis-view">
    <ReaderDocumentView
      v-if="view === 'reader' && currentDocument && sessionState && readerReady"
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
      @range="void session.control('request_range', { range: $event })"
      @retry="void session.control('retry_document', { unit_id: $event || null })"
      @sync="void session.synchronize().catch(() => undefined)"
      @save-selection="saveSelection"
      @remove-selection="removeSelection"
      @clear-selections="clearSelections"
      @update-selection="updateSelection"
      @export-selections="exportSelections"
    />
    </Transition>

    <main v-if="view === 'reader'" class="reader-progress" :class="{ 'reader-progress--blocking': !readerReady }">
      <div v-if="!readerReady" class="reader-progress__identity">
        <div v-if="currentBook" class="reader-progress__cover">
          <img v-if="coverUrl(currentBook)" :src="coverUrl(currentBook)" alt="" />
          <BookMarked v-else :size="34" aria-hidden="true" />
        </div>
        <ReaderOpeningMark v-else />
        <div class="reader-progress__copy">
          <strong>{{ currentBook?.title || currentDocument?.metadata.title || "文本阅读" }}</strong>
          <span v-if="currentBook?.author">{{ currentBook.author }}</span>
          <small v-if="currentBook">{{ currentBook.chapterCount }} 章 · {{ currentBook.totalCharacters.toLocaleString("zh-CN") }} 字</small>
        </div>
      </div>
      <AnalysisProgressPanel :progress="readerProgress" :active="readerAnalysisActive" />
      <div v-if="sessionState" class="reader-progress__diagnostics">
        <button type="button" :disabled="timingSaving" @click="void toggleTiming()">{{ timingEnabled ? '关闭耗时记录' : '开启耗时记录' }}</button>
        <button type="button" @click="exportTiming">导出耗时</button>
      </div>
      <button v-if="!readerReady && sessionState?.progress.failed" class="reader-progress__retry" type="button" @click="void session.control('retry_document', { unit_id: null })">重试分析</button>
    </main>

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
.reader-app { display: flex; min-height: 0; height: 100vh; flex-direction: column; overflow: hidden; background: var(--bg-primary); }
.reader-progress { position: fixed; z-index: 50; top: 70px; right: 18px; width: min(560px, calc(100vw - 36px)); pointer-events: none; }
.reader-progress :deep(.analysis-progress-panel) { pointer-events: auto; }
.reader-progress__diagnostics { display: flex; justify-content: flex-end; gap: 8px; margin-top: 5px; pointer-events: auto; }
.reader-progress__diagnostics button { padding: 4px 8px; border: 1px solid var(--border-color); border-radius: 5px; background: var(--bg-primary); color: var(--text-secondary); cursor: pointer; font-size: .7rem; }
.reader-progress--blocking { position: relative; inset: auto; display: flex; width: 100%; min-height: 0; flex: 1; flex-direction: column; align-items: center; justify-content: center; gap: 26px; padding: 42px 24px 72px; background: var(--bg-primary); pointer-events: auto; }
.reader-progress--blocking > * { width: min(620px, 100%); }
.reader-progress__identity { display: flex; align-items: center; gap: 22px; color: var(--text-primary); }
.reader-progress__cover { position: relative; display: grid; width: 86px; aspect-ratio: 3 / 4; flex: 0 0 auto; place-items: center; border: 1px solid var(--border-color); border-radius: 4px; background: var(--accent-light); color: var(--accent-color); view-transition-name: book-cover; }
.reader-progress__cover::before { position: absolute; inset: -20px; border-radius: 50%; background: color-mix(in srgb, var(--accent-color) 12%, transparent); content: ""; animation: analysis-cover-halo 360ms cubic-bezier(0, 0, .2, 1) both; }
.reader-progress__cover img { position: relative; width: 100%; height: 100%; border-radius: inherit; object-fit: cover; }
.reader-progress__copy { display: flex; min-width: 0; flex-direction: column; animation: analysis-content-enter 220ms 60ms cubic-bezier(0, 0, .2, 1) both; }
.reader-progress__copy > * { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.reader-progress__copy strong { font-size: 1.08rem; }
.reader-progress__copy span { color: var(--text-secondary); font-size: .82rem; }
.reader-progress__copy small { color: var(--text-muted); font-size: .72rem; }
.reader-progress__retry { align-self: center; max-width: max-content; padding: 7px 14px; border: 1px solid var(--border-color); border-radius: var(--radius-sm); background: var(--bg-primary); color: var(--text-primary); cursor: pointer; }
.analysis-view-enter-active { transition: opacity 220ms ease, transform 260ms cubic-bezier(0, 0, .2, 1); }
.analysis-view-enter-from { opacity: 0; transform: translateY(12px); }
@keyframes analysis-cover-halo { from { opacity: .7; transform: scale(.55); } to { opacity: 0; transform: scale(1); } }
@keyframes analysis-content-enter { from { opacity: 0; transform: translateY(8px); } to { opacity: 1; transform: translateY(0); } }
@media (prefers-reduced-motion: reduce) { .reader-progress__cover::before { display: none; } .reader-progress__copy { animation: none; } .analysis-view-enter-active { transition: none; } }
@media (max-width: 640px) { .reader-progress__cover { width: 72px; } .reader-progress__identity { gap: 16px; } }
.text-input-panel { display: flex; gap: 12px; padding: 6px 18px 20px; flex-direction: column; }
.text-input-panel textarea { min-height: 260px; resize: vertical; padding: 12px; border: 1px solid var(--border-color); border-radius: var(--radius-sm); background: var(--bg-secondary); color: var(--text-primary); font: 1rem/1.7 var(--font-ja); }
.text-input-panel button { align-self: flex-end; padding: 8px 15px; border: 0; border-radius: var(--radius-sm); background: var(--accent-color); color: #fff; cursor: pointer; }
.text-input-panel__error { color: var(--novelty-high-text); font-size: .8rem; }
</style>

<style>
::view-transition-group(book-cover) { z-index: 40; animation-duration: 300ms; animation-timing-function: cubic-bezier(.4, 0, .2, 1); }
::view-transition-old(book-cover), ::view-transition-new(book-cover) { height: 100%; overflow: clip; border-radius: 4px; mix-blend-mode: normal; }
</style>
