<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { GripVertical, MoveDown, MoveUp, X } from "@lucide/vue";
import type { DictionarySettings } from "../../types";

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
      </section>
    </div>
  </Transition>
</template>

<style scoped>
.settings-overlay { position: fixed; inset: 0; z-index: 1300; display: grid; place-items: center; padding: 20px; background: color-mix(in srgb, #000 32%, transparent); backdrop-filter: blur(3px); }
.settings-panel { width: min(460px, 100%); padding: 20px; border: 1px solid var(--border-color); border-radius: var(--radius-lg); background: var(--bg-primary); box-shadow: var(--shadow-md); }
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
.fade-enter-active, .fade-leave-active { transition: opacity .12s ease; }
.fade-enter-from, .fade-leave-to { opacity: 0; }
</style>
