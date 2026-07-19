<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { ChevronDown, ChevronUp, GripVertical, X } from "@lucide/vue";
import type { DictionarySettings } from "../../types";
import {
  dictionaryShortcutKeyOptions,
  dictionaryShortcutSettings,
  setDictionaryShortcut,
  shortcutKeyLabel,
  type DictionaryShortcutSettings,
} from "../../composables/useDictionaryShortcuts";

const props = defineProps<{
  show: boolean;
  settings: DictionarySettings;
}>();

const emit = defineEmits<{
  close: [];
  reorder: [order: string[]];
}>();

const orderedDictionaries = ref<string[]>([]);
const draggedDictionary = ref<string | null>(null);
const dragInsertIndex = ref<number | null>(null);
watch(
  () => props.settings,
  (settings) => {
    orderedDictionaries.value = settings.dictionary_order.length
      ? [...settings.dictionary_order]
      : [...settings.available_dictionaries];
  },
  { immediate: true },
);

const hasDictionaries = computed(() => props.settings.available_dictionaries.length > 0);

function moveDictionary(dictionary: string, target: string) {
  if (dictionary === target) return;
  const order = [...orderedDictionaries.value];
  const from = order.indexOf(dictionary);
  const to = order.indexOf(target);
  if (from < 0 || to < 0) return;
  order.splice(from, 1);
  order.splice(to, 0, dictionary);
  orderedDictionaries.value = order;
  emit("reorder", order);
}

function moveByOffset(dictionary: string, offset: number) {
  const index = orderedDictionaries.value.indexOf(dictionary);
  const target = orderedDictionaries.value[index + offset];
  if (target) moveDictionary(dictionary, target);
}

function resetDrag() {
  draggedDictionary.value = null;
  dragInsertIndex.value = null;
}

function handleDragStart(dictionary: string, event: DragEvent) {
  draggedDictionary.value = dictionary;
  dragInsertIndex.value = orderedDictionaries.value.indexOf(dictionary);
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("text/plain", dictionary);
  }
}

function handleDragOver(index: number, event: DragEvent) {
  if (!draggedDictionary.value) return;
  const row = event.currentTarget as HTMLElement;
  const bounds = row.getBoundingClientRect();
  dragInsertIndex.value = event.clientY < bounds.top + bounds.height / 2 ? index : index + 1;
  if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
}

function handleListDragOver(event: DragEvent) {
  if (!draggedDictionary.value || event.target !== event.currentTarget) return;
  dragInsertIndex.value = orderedDictionaries.value.length;
  if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
}

function handleDrop() {
  const dictionary = draggedDictionary.value;
  const insertionIndex = dragInsertIndex.value;
  if (!dictionary || insertionIndex === null) {
    resetDrag();
    return;
  }
  const order = [...orderedDictionaries.value];
  const from = order.indexOf(dictionary);
  if (from < 0) {
    resetDrag();
    return;
  }
  order.splice(from, 1);
  const adjustedIndex = Math.max(0, Math.min(order.length, insertionIndex - (insertionIndex > from ? 1 : 0)));
  order.splice(adjustedIndex, 0, dictionary);
  resetDrag();
  if (order.every((item, index) => item === orderedDictionaries.value[index])) return;
  orderedDictionaries.value = order;
  emit("reorder", order);
}

function updateShortcut(name: keyof DictionaryShortcutSettings, event: Event) {
  setDictionaryShortcut(name, (event.target as HTMLSelectElement).value);
}
</script>

<template>
  <Transition name="fade">
    <div v-if="show" class="settings-overlay" @click.self="emit('close')">
      <section class="settings-panel" role="dialog" aria-modal="true" aria-label="词典设置">
        <header>
          <div>
            <h2>词典设置</h2>
            <p>拖动词典调整优先级。第一个词典为默认词典，查词浮层仍可切换到其他命中项。</p>
          </div>
          <button type="button" class="close-button" aria-label="关闭词典设置" @click="emit('close')"><X :size="19" aria-hidden="true" /></button>
        </header>
        <p v-if="!hasDictionaries" class="empty-state">尚未加载本地词典。</p>
        <ol v-else class="dictionary-list" aria-label="词典优先级" @dragover.prevent="handleListDragOver" @drop.prevent="handleDrop">
          <li
            v-for="(dictionary, index) in orderedDictionaries"
            :key="dictionary"
            :class="{
              dragging: draggedDictionary === dictionary,
              'drop-before': draggedDictionary && dragInsertIndex === index,
              'drop-after': draggedDictionary && dragInsertIndex === orderedDictionaries.length && index === orderedDictionaries.length - 1,
            }"
            @dragover.prevent.stop="handleDragOver(index, $event)"
            @drop.prevent.stop="handleDrop"
          >
            <span
              class="drag-handle"
              draggable="true"
              :title="`拖动 ${dictionary} 调整优先级`"
              aria-hidden="true"
              @dragstart.stop="handleDragStart(dictionary, $event)"
              @dragend="resetDrag"
            ><GripVertical :size="17" aria-hidden="true" /></span>
            <span class="priority">{{ index + 1 }}</span>
            <span class="dictionary-name">
              <strong>{{ dictionary }}</strong>
              <span v-if="index === 0" class="default-badge">默认</span>
            </span>
            <div class="move-actions">
              <button type="button" :disabled="index === 0" :aria-label="`上移 ${dictionary}`" @click="moveByOffset(dictionary, -1)"><ChevronUp :size="17" :stroke-width="2.25" aria-hidden="true" /></button>
              <button type="button" :disabled="index === orderedDictionaries.length - 1" :aria-label="`下移 ${dictionary}`" @click="moveByOffset(dictionary, 1)"><ChevronDown :size="17" :stroke-width="2.25" aria-hidden="true" /></button>
            </div>
          </li>
        </ol>
        <section class="shortcut-settings" aria-labelledby="dictionary-shortcut-heading">
          <div class="section-heading">
            <h3 id="dictionary-shortcut-heading">悬浮快捷键</h3>
            <p>保持鼠标位置不动时循环气泡选项；同时显示表记和读音时，表记使用 Shift 组合键。</p>
          </div>
          <label>
            <span>切换词典</span>
            <span class="shortcut-control">
              <kbd v-if="dictionaryShortcutSettings.dictionaryKey">{{ shortcutKeyLabel(dictionaryShortcutSettings.dictionaryKey) }}</kbd>
              <select :value="dictionaryShortcutSettings.dictionaryKey" @change="updateShortcut('dictionaryKey', $event)">
                <option
                  v-for="option in dictionaryShortcutKeyOptions"
                  :key="option.value"
                  :value="option.value"
                  :disabled="Boolean(option.value && option.value === dictionaryShortcutSettings.choiceKey)"
                >{{ option.label }}</option>
              </select>
            </span>
          </label>
          <label>
            <span>切换表记／读音</span>
            <span class="shortcut-control">
              <kbd v-if="dictionaryShortcutSettings.choiceKey">{{ shortcutKeyLabel(dictionaryShortcutSettings.choiceKey) }}</kbd>
              <select :value="dictionaryShortcutSettings.choiceKey" @change="updateShortcut('choiceKey', $event)">
                <option
                  v-for="option in dictionaryShortcutKeyOptions"
                  :key="option.value"
                  :value="option.value"
                  :disabled="Boolean(option.value && option.value === dictionaryShortcutSettings.dictionaryKey)"
                >{{ option.label }}</option>
              </select>
            </span>
          </label>
        </section>
      </section>
    </div>
  </Transition>
</template>

<style scoped>
.settings-overlay { position: fixed; inset: 0; z-index: 1300; display: grid; place-items: center; padding: 20px; background: color-mix(in srgb, #000 32%, transparent); backdrop-filter: blur(3px); }
.settings-panel { width: min(460px, 100%); max-height: min(720px, calc(100vh - 40px)); overflow-y: auto; padding: 20px; border: 1px solid var(--border-color); border-radius: var(--radius-lg); background: var(--bg-primary); box-shadow: var(--shadow-md); }
header { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; margin-bottom: 18px; }
h2 { color: var(--text-primary); font-size: 1.06rem; }
p { margin-top: 4px; color: var(--text-secondary); font-size: .82rem; }
.close-button { display: grid; place-items: center; flex: 0 0 auto; border: 0; background: transparent; color: var(--text-muted); line-height: 1; cursor: pointer; }
.dictionary-list { display: grid; gap: 8px; margin-top: 10px; padding: 0; list-style: none; }
.dictionary-list li { position: relative; display: grid; grid-template-columns: 24px 24px minmax(0, 1fr) 64px; align-items: center; gap: 9px; min-height: 48px; padding: 8px 9px; border: 1px solid var(--border-color); border-radius: var(--radius-sm); background: var(--bg-card); transition: border-color .12s ease, background-color .12s ease, transform .12s ease; }
.dictionary-list li.dragging { opacity: .46; transform: scale(.985); }
.dictionary-list li.drop-before::before,
.dictionary-list li.drop-after::after { content: ""; position: absolute; z-index: 2; right: 8px; left: 8px; height: 2px; border-radius: 999px; background: var(--accent-color); box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent-light) 82%, transparent); }
.dictionary-list li.drop-before::before { top: -5px; }
.dictionary-list li.drop-after::after { bottom: -5px; }
.drag-handle { display: grid; width: 24px; height: 30px; place-items: center; border-radius: 6px; color: var(--text-muted); cursor: grab; user-select: none; }
.drag-handle:hover { background: var(--bg-secondary); color: var(--accent-color); }
.drag-handle:active { cursor: grabbing; }
.priority { display: grid; width: 20px; height: 20px; place-items: center; border-radius: 50%; background: var(--bg-secondary); color: var(--text-muted); font: 700 .68rem var(--font-ui); }
.dictionary-name { display: flex; min-width: 0; align-items: center; gap: 8px; }
strong { min-width: 0; overflow: hidden; color: var(--text-primary); font-size: .85rem; text-overflow: ellipsis; white-space: nowrap; }
.default-badge { padding: 2px 7px; border-radius: 999px; background: var(--accent-light); color: var(--accent-color); font: 700 .68rem var(--font-ui); }
.move-actions { display: inline-grid; grid-template-columns: repeat(2, 30px); gap: 4px; }
.move-actions button { display: grid; width: 30px; height: 30px; place-items: center; border: 1px solid var(--border-color); border-radius: 7px; padding: 0; background: transparent; color: var(--text-secondary); line-height: 0; cursor: pointer; }
.move-actions button svg { display: block; }
.move-actions button:hover:not(:disabled) { border-color: var(--accent-color); color: var(--accent-color); }
.move-actions button:disabled { opacity: .35; cursor: not-allowed; }
.empty-state { margin-top: 10px; }
.shortcut-settings { display: grid; gap: 9px; margin-top: 18px; padding-top: 16px; border-top: 1px solid var(--border-color); }
.section-heading { margin-bottom: 2px; }
h3 { color: var(--text-primary); font-size: .9rem; }
.shortcut-settings label { display: flex; align-items: center; justify-content: space-between; gap: 14px; color: var(--text-secondary); font-size: .82rem; }
.shortcut-control { display: flex; align-items: center; gap: 7px; }
.shortcut-control select { min-width: 92px; height: 32px; border: 1px solid var(--border-color); border-radius: 6px; padding: 0 28px 0 9px; background: var(--bg-card); color: var(--text-primary); font: .78rem var(--font-ui); }
kbd { min-width: 22px; padding: 3px 6px 4px; border: 1px solid color-mix(in srgb, var(--border-color) 88%, var(--text-muted)); border-bottom-width: 2px; border-radius: 5px; background: var(--bg-card); color: var(--text-secondary); font: 700 .68rem/1 var(--font-ui); text-align: center; box-shadow: 0 1px 0 color-mix(in srgb, var(--border-color) 55%, transparent); }
@media (max-width: 420px) { .shortcut-settings label { align-items: flex-start; flex-direction: column; gap: 6px; } .shortcut-control { width: 100%; } .shortcut-control select { flex: 1; } }
.fade-enter-active, .fade-leave-active { transition: opacity .12s ease; }
.fade-enter-from, .fade-leave-to { opacity: 0; }
</style>
