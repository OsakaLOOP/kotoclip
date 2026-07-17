<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { GripVertical, MoveDown, MoveUp, X } from "@lucide/vue";
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
const dragTarget = ref<string | null>(null);
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

function handleDragStart(dictionary: string) {
  draggedDictionary.value = dictionary;
}

function handleDrop(target: string) {
  if (draggedDictionary.value) moveDictionary(draggedDictionary.value, target);
  draggedDictionary.value = null;
  dragTarget.value = null;
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
        <ol v-else class="dictionary-list" aria-label="词典优先级">
          <li
            v-for="(dictionary, index) in orderedDictionaries"
            :key="dictionary"
            :class="{ dragging: draggedDictionary === dictionary, 'drag-target': dragTarget === dictionary && draggedDictionary !== dictionary }"
            draggable="true"
            @dragstart="handleDragStart(dictionary)"
            @dragend="draggedDictionary = null; dragTarget = null"
            @dragover.prevent="dragTarget = dictionary"
            @drop.prevent="handleDrop(dictionary)"
          >
            <GripVertical class="drag-handle" :size="17" aria-hidden="true" />
            <span class="priority">{{ index + 1 }}</span>
            <strong>{{ dictionary }}</strong>
            <span v-if="index === 0" class="default-badge">默认</span>
            <div class="move-actions">
              <button type="button" :disabled="index === 0" :aria-label="`上移 ${dictionary}`" @click="moveByOffset(dictionary, -1)"><MoveUp :size="14" aria-hidden="true" /></button>
              <button type="button" :disabled="index === orderedDictionaries.length - 1" :aria-label="`下移 ${dictionary}`" @click="moveByOffset(dictionary, 1)"><MoveDown :size="14" aria-hidden="true" /></button>
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
.dictionary-list li { display: grid; grid-template-columns: auto 24px minmax(0, 1fr) auto auto; align-items: center; gap: 9px; min-height: 44px; padding: 8px 9px; border: 1px solid var(--border-color); border-radius: var(--radius-sm); background: var(--bg-card); cursor: grab; transition: border-color .12s ease, background-color .12s ease, transform .12s ease; }
.dictionary-list li:active { cursor: grabbing; }
.dictionary-list li.dragging { opacity: .46; transform: scale(.985); }
.dictionary-list li.drag-target { border-color: var(--accent-color); background: var(--accent-light); }
.drag-handle { color: var(--text-muted); font-size: 1.1rem; letter-spacing: -2px; }
.priority { display: grid; width: 20px; height: 20px; place-items: center; border-radius: 50%; background: var(--bg-secondary); color: var(--text-muted); font: 700 .68rem var(--font-ui); }
strong { overflow: hidden; color: var(--text-primary); font-size: .85rem; text-overflow: ellipsis; white-space: nowrap; }
.default-badge { padding: 2px 7px; border-radius: 999px; background: var(--accent-light); color: var(--accent-color); font: 700 .68rem var(--font-ui); }
.move-actions { display: flex; gap: 3px; }
.move-actions button { width: 24px; height: 24px; border: 1px solid var(--border-color); border-radius: 5px; background: transparent; color: var(--text-secondary); cursor: pointer; }
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
