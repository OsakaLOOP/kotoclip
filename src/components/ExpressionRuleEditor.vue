<script setup lang="ts">
import { ref, watch } from "vue";
import type { AnnotatedToken } from "../types";

const props = defineProps<{
  show: boolean;
  tokens: AnnotatedToken[];
}>();

const emit = defineEmits<{
  (event: "cancel"): void;
  (event: "save", label: string, description: string, slotIndices: number[]): void;
}>();

const label = ref("");
const description = ref("");
const slots = ref<number[]>([]);

watch(
  () => [props.show, props.tokens] as const,
  () => {
    if (!props.show) return;
    label.value = props.tokens.map((token) => token.bunsetsu.surface).join("");
    description.value = "";
    slots.value = [];
  },
  { deep: true },
);

function signature(token: AnnotatedToken): string {
  return token.bunsetsu.morphemes
    .map((morpheme) => `${morpheme.base_form === "*" ? morpheme.surface : morpheme.base_form}/${morpheme.pos.major}`)
    .join(" + ");
}

function toggleSlot(index: number) {
  slots.value = slots.value.includes(index)
    ? slots.value.filter((value) => value !== index)
    : [...slots.value, index].sort((a, b) => a - b);
}
</script>

<template>
  <Transition name="fade">
    <div v-if="show" class="expression-editor-overlay" @click.self="emit('cancel')">
      <section class="expression-editor" role="dialog" aria-modal="true" aria-label="保存跨文节表达">
        <header>
          <div>
            <h2>保存跨文节表达</h2>
            <p>保留文节边界；可变槽位只约束词性与助词结构。</p>
          </div>
          <button aria-label="关闭" @click="emit('cancel')">×</button>
        </header>

        <label class="label-field">
          <span>短标签</span>
          <input v-model="label" maxlength="28" />
        </label>

        <label class="label-field">
          <span>整体含义或使用条件</span>
          <textarea v-model="description" rows="3" placeholder="说明这个整体表达了什么，而不是逐词翻译。"></textarea>
        </label>

        <div class="part-list">
          <button
            v-for="(token, index) in tokens"
            :key="index"
            type="button"
            class="part-item"
            :class="{ slot: slots.includes(index) }"
            @click="toggleSlot(index)"
          >
            <span class="part-index">{{ index + 1 }}</span>
            <span class="part-copy">
              <strong>{{ token.bunsetsu.surface }}</strong>
              <small>{{ signature(token) }}</small>
            </span>
            <span class="slot-state">{{ slots.includes(index) ? '可变槽' : '固定' }}</span>
          </button>
        </div>

        <footer>
          <button class="secondary" @click="emit('cancel')">取消</button>
          <button class="primary" :disabled="!label.trim()" @click="emit('save', label.trim(), description.trim(), slots)">保存并应用</button>
        </footer>
      </section>
    </div>
  </Transition>
</template>

<style scoped>
.expression-editor-overlay { position: fixed; z-index: 1300; inset: 0; display: grid; place-items: center; padding: 20px; background: rgba(20, 17, 24, 0.35); }
.expression-editor { width: min(620px, 100%); max-height: min(760px, 90vh); overflow: auto; box-sizing: border-box; padding: 22px; border-radius: 12px; background: var(--bg-primary); box-shadow: 0 18px 55px rgba(20, 17, 24, 0.22); }
.expression-editor header { display: flex; justify-content: space-between; gap: 18px; }
.expression-editor h2 { margin: 0; font-size: 1.15rem; }
.expression-editor p { margin: 5px 0 0; color: var(--text-secondary); font-size: 0.82rem; }
.expression-editor header button { align-self: flex-start; border: 0; background: transparent; font-size: 1.5rem; cursor: pointer; }
.label-field { display: grid; gap: 6px; margin: 20px 0 14px; font-size: 0.82rem; color: var(--text-secondary); }
.label-field input, .label-field textarea { min-width: 0; box-sizing: border-box; padding: 9px 11px; border: 1px solid var(--border-color); border-radius: 7px; background: var(--bg-secondary); color: var(--text-primary); font: inherit; resize: vertical; }
.part-list { display: grid; gap: 8px; }
.part-item { display: flex; align-items: center; min-width: 0; gap: 10px; padding: 10px; text-align: left; border: 1px solid var(--border-color); border-radius: 8px; background: var(--bg-secondary); color: var(--text-primary); cursor: pointer; }
.part-item.slot { border-color: #7956a8; background: #f3eef9; }
.part-index { flex: 0 0 1.6rem; text-align: center; color: var(--text-secondary); }
.part-copy { display: grid; min-width: 0; flex: 1; }
.part-copy strong, .part-copy small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.part-copy small { color: var(--text-secondary); }
.slot-state { flex: 0 0 auto; font-size: 0.72rem; color: #65428f; }
.expression-editor footer { display: flex; justify-content: flex-end; gap: 10px; margin-top: 20px; }
.expression-editor footer button { padding: 8px 14px; border-radius: 7px; cursor: pointer; }
.secondary { border: 1px solid var(--border-color); background: transparent; }
.primary { border: 1px solid #65428f; background: #65428f; color: #fff; }
.primary:disabled { cursor: not-allowed; opacity: 0.45; }
</style>
