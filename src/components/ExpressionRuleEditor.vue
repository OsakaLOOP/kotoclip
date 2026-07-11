<script setup lang="ts">
import { ref, watch, computed } from "vue";
import type { AnnotatedToken } from "../types";

const props = defineProps<{
  show: boolean;
  tokens: AnnotatedToken[];
  startMorphemeIdx: number;
  endMorphemeIdx: number;
}>();

const emit = defineEmits<{
  (event: "cancel"): void;
  (event: "save", label: string, description: string, bunsetsuStates: ('fixed' | 'slot' | 'any')[], morphemeMasks: boolean[][], gapAfter: number | null): void;
}>();

const description = ref("");
const bunsetsuStates = ref<('fixed' | 'slot' | 'any')[]>([]);
const morphemeMasks = ref<boolean[][]>([]);
const activeGapIndex = ref<number | null>(null); // 在第 i 个 token 后面插入了可变间隔 (0-based)

// 遮罩层防误触点击关闭
const overlayMouseDownTarget = ref<EventTarget | null>(null);

function handleOverlayMouseDown(e: MouseEvent) {
  overlayMouseDownTarget.value = e.target;
}

function handleOverlayMouseUp(e: MouseEvent) {
  if (overlayMouseDownTarget.value === e.currentTarget && e.target === e.currentTarget) {
    emit('cancel');
  }
  overlayMouseDownTarget.value = null;
}

// 归一化逻辑
function normalizedLemma(surface: string, base_form: string): string {
  return (!base_form || base_form.trim() === "" || base_form === "*") ? surface : base_form;
}

function structuralLemma(surface: string, base_form: string, majorPos: string): string | null {
  const lemma = normalizedLemma(surface, base_form);
  if (majorPos === "助詞" && ["か", "ね", "よ"].includes(lemma)) {
    return null;
  }
  if (majorPos !== "助動詞" && majorPos !== "助动词") {
    return lemma;
  }
  if (["た", "ます"].includes(lemma)) {
    return null;
  }
  if (["だ", "edit", "です"].includes(lemma)) {
    return "<copula>";
  }
  return lemma;
}

watch(
  () => [props.show, props.tokens, props.startMorphemeIdx, props.endMorphemeIdx] as const,
  () => {
    if (!props.show || !props.tokens || props.tokens.length === 0) return;
    
    bunsetsuStates.value = props.tokens.map(() => 'fixed');
    activeGapIndex.value = null;
    
    morphemeMasks.value = props.tokens.map((token, tokenIdx) => {
      return token.bunsetsu.morphemes.map((_, morphemeIdx) => {
        if (tokenIdx === 0) {
          return morphemeIdx >= props.startMorphemeIdx;
        } else if (tokenIdx === props.tokens.length - 1) {
          return morphemeIdx <= props.endMorphemeIdx;
        } else {
          return true;
        }
      });
    });

    description.value = "";
  },
  { deep: true, immediate: true }
);

// 校验约束
const validationError = computed<string | null>(() => {
  if (bunsetsuStates.value.length < 2) {
    return "跨文节表达至少需要两个文节";
  }

  for (let i = 0; i < bunsetsuStates.value.length; i++) {
    if (bunsetsuStates.value[i] !== 'any') {
      const activeCount = morphemeMasks.value[i]?.filter(Boolean).length || 0;
      if (activeCount === 0) {
        return `文节 ${i + 1} ("${props.tokens[i].bunsetsu.surface}") 至少需要保留一个参与匹配的语素`;
      }
    }
  }

  return null;
});

const hasLeftExclusions = computed(() => {
  if (morphemeMasks.value.length === 0) return false;
  const mask = morphemeMasks.value[0] || [];
  const token = props.tokens[0];
  let firstSelected = -1;
  token.bunsetsu.morphemes.forEach((morpheme, mIdx) => {
    if (morpheme.surface.trim() === "") return;
    if (mask[mIdx] && firstSelected === -1) {
      firstSelected = mIdx;
    }
  });
  return firstSelected > 0;
});

const hasRightExclusions = computed(() => {
  if (morphemeMasks.value.length === 0) return false;
  const lastIdx = props.tokens.length - 1;
  const mask = morphemeMasks.value[lastIdx] || [];
  const token = props.tokens[lastIdx];
  let lastSelected = -1;
  let totalNonEmpty = 0;
  token.bunsetsu.morphemes.forEach((morpheme, mIdx) => {
    if (morpheme.surface.trim() === "") return;
    if (mask[mIdx]) {
      lastSelected = totalNonEmpty;
    }
    totalNonEmpty++;
  });
  return lastSelected < totalNonEmpty - 1;
});

function toggleLeftTilde() {
  const mask = morphemeMasks.value[0];
  if (!mask) return;
  const token = props.tokens[0];
  if (hasLeftExclusions.value) {
    token.bunsetsu.morphemes.forEach((morpheme, mIdx) => {
      if (morpheme.surface.trim() !== "") {
        mask[mIdx] = true;
      }
    });
  } else {
    const nonEmpties = token.bunsetsu.morphemes
      .map((m, idx) => ({ m, idx }))
      .filter(x => x.m.surface.trim() !== "");
    let targetStart = props.startMorphemeIdx;
    if (targetStart === 0 && nonEmpties.length > 1) {
      targetStart = nonEmpties[nonEmpties.length - 1].idx;
    }
    nonEmpties.forEach(({ idx }) => {
      mask[idx] = idx >= targetStart;
    });
  }
}

function toggleRightTilde() {
  const lastIdx = props.tokens.length - 1;
  const mask = morphemeMasks.value[lastIdx];
  if (!mask) return;
  const token = props.tokens[lastIdx];
  if (hasRightExclusions.value) {
    token.bunsetsu.morphemes.forEach((morpheme, mIdx) => {
      if (morpheme.surface.trim() !== "") {
        mask[mIdx] = true;
      }
    });
  } else {
    const nonEmpties = token.bunsetsu.morphemes
      .map((m, idx) => ({ m, idx }))
      .filter(x => x.m.surface.trim() !== "");
    let targetEnd = props.endMorphemeIdx;
    if (targetEnd === token.bunsetsu.morphemes.length - 1 && nonEmpties.length > 1) {
      targetEnd = nonEmpties[0].idx;
    }
    nonEmpties.forEach(({ idx }) => {
      mask[idx] = idx <= targetEnd;
    });
  }
}



// 自动生成的表达式名称 (全角○和全角…连接，仅词性、任意和间隔合并为○)
const ruleLabelName = computed(() => {
  if (morphemeMasks.value.length === 0 || bunsetsuStates.value.length === 0) return "";

  const items: string[] = [];

  props.tokens.forEach((token, idx) => {
    const state = bunsetsuStates.value[idx];
    
    if (state === 'slot' || state === 'any') {
      items.push("○");
    } else {
      const mask = morphemeMasks.value[idx] || [];
      const surfs: string[] = [];
      token.bunsetsu.morphemes.forEach((morpheme, mIdx) => {
        if (morpheme.surface.trim() !== "" && mask[mIdx]) {
          surfs.push(morpheme.surface);
        }
      });
      if (surfs.length > 0) {
        items.push(surfs.join(""));
      } else {
        items.push("○");
      }
    }

    if (activeGapIndex.value === idx) {
      items.push("○");
    }
  });

  let result = "";
  items.forEach((item) => {
    if (item === "○") {
      if (result.endsWith("○")) return; // 合并连续的 ○
      if (result === "") {
        result = "○";
      } else {
        result += " ○";
      }
    } else {
      if (result.endsWith("○")) {
        result += " " + item;
      } else {
        result += item;
      }
    }
  });

  if (props.tokens.length > 0) {
    const firstState = bunsetsuStates.value[0];
    const lastState = bunsetsuStates.value[props.tokens.length - 1];

    if (firstState === 'fixed' && hasLeftExclusions.value) {
      result = "～" + result;
    }
    if (lastState === 'fixed' && hasRightExclusions.value) {
      result = result + "～";
    }
  }

  return result.trim();
});

// 匹配签名调试预览 (详细包含词性和 lemmas)
const previewSignature = computed(() => {
  if (morphemeMasks.value.length === 0 || bunsetsuStates.value.length === 0) return "";
  
  const getPartSignature = (idx: number) => {
    const token = props.tokens[idx];
    const mask = morphemeMasks.value[idx] || [];
    const state = bunsetsuStates.value[idx];
    
    const items: string[] = [];
    let firstSelected = -1;
    let lastSelected = -1;
    let totalNonEmpty = 0;

    token.bunsetsu.morphemes.forEach((morpheme, mIdx) => {
      if (morpheme.surface.trim() === "") return;
      
      const included = mask[mIdx];
      if (included) {
        const lemma = structuralLemma(morpheme.surface, morpheme.base_form, morpheme.pos.major);
        if (lemma !== null) {
          if (firstSelected === -1) firstSelected = totalNonEmpty;
          lastSelected = totalNonEmpty;
          
          if (state === 'slot') {
            items.push(`{${morpheme.pos.major}}`);
          } else if (state === 'any') {
            items.push(`{任意}`);
          } else {
            items.push(`${lemma}/${morpheme.pos.major}`);
          }
        }
      }
      totalNonEmpty++;
    });

    if (items.length === 0) return "";
    
    let sig = items.join(" + ");
    
    if (state === 'fixed') {
      if (idx === 0 && hasLeftExclusions.value) {
        sig = `～${sig}`;
      }
      if (idx === props.tokens.length - 1 && hasRightExclusions.value) {
        sig = `${sig}～`;
      }
    }
    
    return sig;
  };

  const partsSignatures = props.tokens.map((_, idx) => getPartSignature(idx));

  if (activeGapIndex.value !== null) {
    const gapIdx = activeGapIndex.value;
    const head = partsSignatures.slice(0, gapIdx + 1).filter(Boolean).join(" + ");
    const tail = partsSignatures.slice(gapIdx + 1).filter(Boolean).join(" + ");
    return `${head}  ○  ${tail}`;
  } else {
    return partsSignatures.filter(Boolean).join(" + ");
  }
});

function handleSave() {
  emit(
    'save', 
    ruleLabelName.value, // 使用自动生成的格式化日文名称作为规则标签
    description.value.trim(), 
    bunsetsuStates.value, 
    morphemeMasks.value, 
    activeGapIndex.value
  );
}
</script>

<template>
  <Transition name="fade">
    <div 
      v-if="show" 
      class="expression-editor-overlay" 
      @mousedown="handleOverlayMouseDown"
      @mouseup="handleOverlayMouseUp"
    >
      <section class="expression-editor" role="dialog" aria-modal="true" aria-label="跨文节表达编辑" @mousedown.stop @mouseup.stop>
        <header>
          <div>
            <h2>跨文节表达</h2>
            <p>快速点选配置文节的词形和词性约束；点击文节间的空隙可插入可变间隔。</p>
          </div>
          <button aria-label="关闭" @click="emit('cancel')">×</button>
        </header>

        <div class="editor-body no-scrollbar">
          <!-- 自动生成的签名与名称置顶显示 -->
          <div class="top-signature-area">
            <div class="signature-row">
              <span class="preview-title">表达式名称 (自动规则标签)</span>
              <div class="name-display-box">
                <strong>{{ ruleLabelName || '（未匹配任何有效内容）' }}</strong>
              </div>
            </div>
            <div class="signature-row">
              <span class="preview-title">表达式匹配规则 (签名)</span>
              <div class="signature-box">
                <code>{{ previewSignature || '（未匹配任何有效内容）' }}</code>
              </div>
            </div>
          </div>

          <label class="label-field">
            <span>整体含义或使用条件</span>
            <textarea v-model="description" rows="2" placeholder="说明这个整体表达了什么。"></textarea>
          </label>

          <div class="part-list">
            <template v-for="(token, index) in tokens" :key="index">
              <!-- 文节卡片 -->
              <div
                class="part-card"
                :class="{ 
                  'part-fixed': bunsetsuStates[index] === 'fixed',
                  'part-slot': bunsetsuStates[index] === 'slot',
                  'part-any': bunsetsuStates[index] === 'any'
                }"
              >
                <div class="part-card-header">
                  <button 
                    v-if="index === 0 && bunsetsuStates[index] === 'fixed'"
                    type="button" 
                    class="tilde-btn tilde-left"
                    :class="{ active: hasLeftExclusions }"
                    @click="toggleLeftTilde"
                    title="切换前接波浪号 (～) 松散匹配"
                  >
                    ～
                  </button>
                  <span class="part-index">#{{ index + 1 }}</span>
                  <span class="part-surface">{{ token.bunsetsu.surface }}</span>
                  <div class="state-segmented-control">
                    <button 
                      type="button" 
                      class="state-btn"
                      :class="{ active: bunsetsuStates[index] === 'fixed' }"
                      @click="bunsetsuStates[index] = 'fixed'"
                    >
                      固定
                    </button>
                    <button 
                      type="button" 
                      class="state-btn"
                      :class="{ active: bunsetsuStates[index] === 'slot' }"
                      @click="bunsetsuStates[index] = 'slot'"
                    >
                      仅词性
                    </button>
                    <button 
                      type="button" 
                      class="state-btn"
                      :class="{ active: bunsetsuStates[index] === 'any' }"
                      @click="bunsetsuStates[index] = 'any'"
                    >
                      任意
                    </button>
                  </div>
                  <button 
                    v-if="index === tokens.length - 1 && bunsetsuStates[index] === 'fixed'"
                    type="button" 
                    class="tilde-btn tilde-right"
                    :class="{ active: hasRightExclusions }"
                    @click="toggleRightTilde"
                    title="切换后接波浪号 (～) 松散匹配"
                  >
                    ～
                  </button>
                </div>
                
                <!-- 固定/仅词性时展示语素列表，任意时显示简易占位 -->
                <div v-if="bunsetsuStates[index] !== 'any'" class="morpheme-list">
                  <label
                    v-for="(morpheme, mIdx) in token.bunsetsu.morphemes"
                    :key="mIdx"
                    class="morpheme-item"
                    :class="{ 
                      'is-excluded': !morphemeMasks[index]?.[mIdx],
                      'is-normalized-out': structuralLemma(morpheme.surface, morpheme.base_form, morpheme.pos.major) === null
                    }"
                  >
                    <input 
                      type="checkbox" 
                      v-model="morphemeMasks[index][mIdx]"
                    />
                    <span class="m-surface">{{ morpheme.surface }}</span>
                    <span class="m-info">
                      {{ normalizedLemma(morpheme.surface, morpheme.base_form) }}/{{ morpheme.pos.major }}
                      <span 
                        v-if="structuralLemma(morpheme.surface, morpheme.base_form, morpheme.pos.major) === null" 
                        class="out-badge"
                      >
                        归一化排除
                      </span>
                    </span>
                  </label>
                </div>
                <div v-else class="any-placeholder">
                  任意文节（匹配该位置的任何词形或词性）
                </div>
              </div>

              <!-- 文节之间的可插入/已插入间隔区 -->
              <div v-if="index < tokens.length - 1" class="gap-connection-area">
                <div v-if="activeGapIndex === index" class="gap-card">
                  <span class="gap-icon">⋯</span>
                  <span class="gap-text">可变间隔 ( 0-10 个文节 )</span>
                  <button type="button" class="gap-delete-btn" @click="activeGapIndex = null">×</button>
                </div>
                <div v-else-if="activeGapIndex === null" class="gap-divider">
                  <button type="button" class="gap-insert-btn" @click="activeGapIndex = index">
                    ＋ 插入可变间隔 (○)
                  </button>
                </div>
              </div>
            </template>
          </div>
        </div>

        <footer>
          <span v-if="validationError" class="validation-message">
            ⚠️ {{ validationError }}
          </span>
          <div class="btn-group">
            <button class="secondary" @click="emit('cancel')">取消</button>
            <button 
              class="primary" 
              :disabled="!ruleLabelName || !!validationError" 
              @click="handleSave"
            >
              保存并应用
            </button>
          </div>
        </footer>
      </section>
    </div>
  </Transition>
</template>

<style scoped>
.expression-editor-overlay { position: fixed; z-index: 1300; inset: 0; display: grid; place-items: center; padding: 20px; background: rgba(15, 12, 20, 0.45); backdrop-filter: blur(4px); }
.expression-editor { width: min(650px, 100%); max-height: min(850px, 92vh); display: flex; flex-direction: column; overflow: hidden; box-sizing: border-box; padding: 24px; border-radius: 16px; background: var(--bg-primary); border: 1px solid var(--border-color); box-shadow: 0 24px 60px rgba(10, 8, 15, 0.3); }
.expression-editor header { display: flex; justify-content: space-between; gap: 18px; margin-bottom: 12px; }
.expression-editor h2 { margin: 0; font-size: 1.25rem; color: var(--accent-color); font-weight: 700; }
.expression-editor p { margin: 4px 0 0; color: var(--text-muted); font-size: 0.8rem; }
.expression-editor header button { align-self: flex-start; border: 0; background: transparent; font-size: 1.6rem; cursor: pointer; color: var(--text-muted); line-height: 1; }
.editor-body { flex: 1; overflow-y: auto; display: flex; flex-direction: column; gap: 14px; padding-right: 4px; }
.label-field { display: grid; gap: 6px; font-size: 0.82rem; color: var(--text-secondary); }
.label-field textarea { min-width: 0; box-sizing: border-box; padding: 10px 12px; border: 1px solid var(--border-color); border-radius: 8px; background: var(--bg-secondary); color: var(--text-primary); font: inherit; outline: none; transition: border-color 0.2s; resize: none; }
.label-field textarea:focus { border-color: var(--accent-color); }
.part-list { display: grid; gap: 4px; padding: 2px; }
.part-card { border: 1px solid var(--border-color); border-radius: 10px; background: var(--bg-secondary); overflow: hidden; transition: all 0.2s ease; }
.part-fixed { border-left: 4px solid var(--accent-color, #65428f); }
.part-slot { border-left: 4px solid #d9822b; background: rgba(217, 130, 43, 0.02); }
.part-any { border-left: 4px solid #2b82d9; background: rgba(43, 130, 217, 0.02); }
.part-card-header { display: flex; align-items: center; justify-content: space-between; padding: 10px 14px; background: rgba(0, 0, 0, 0.02); border-bottom: 1px solid var(--border-color); }
.part-index { font-weight: bold; font-variant-numeric: tabular-nums; font-size: 0.85rem; color: var(--text-muted); }
.part-surface { font-weight: 700; font-size: 0.95rem; color: var(--text-primary); flex: 1; margin-left: 10px; }

.tilde-btn { border: 1px solid var(--border-color); background: rgba(0, 0, 0, 0.02); border-radius: 6px; width: 26px; height: 26px; display: flex; align-items: center; justify-content: center; font-size: 1.1rem; font-weight: bold; cursor: pointer; color: var(--text-muted); transition: all 0.2s ease; outline: none; user-select: none; font-family: "BIZ UDPGothic", "Hiragino Sans", Meiryo, sans-serif; line-height: 1; }
.tilde-btn:hover { color: var(--text-primary); background: rgba(0, 0, 0, 0.05); border-color: var(--text-muted); }
.tilde-btn.active { background: var(--accent-color, #65428f); color: #fff; border-color: var(--accent-color); box-shadow: 0 2px 6px rgba(101, 66, 143, 0.2); }
.tilde-left { margin-right: 8px; }
.tilde-right { margin-left: 8px; }

.state-segmented-control { display: flex; border: 1px solid var(--border-color); border-radius: 6px; overflow: hidden; background: rgba(0, 0, 0, 0.02); }
.state-btn { border: 0; background: transparent; padding: 4px 10px; font-size: 0.78rem; font-weight: bold; cursor: pointer; color: var(--text-muted); transition: all 0.2s ease; outline: none; }
.state-btn:hover { color: var(--text-primary); background: rgba(0, 0, 0, 0.04); }
.state-btn.active { color: #fff; }
.state-segmented-control :nth-child(1).active { background: var(--accent-color, #65428f); }
.state-segmented-control :nth-child(2).active { background: #d9822b; }
.state-segmented-control :nth-child(3).active { background: #2b82d9; }

.morpheme-list { padding: 10px 14px; display: flex; flex-direction: column; gap: 8px; }
.morpheme-item { display: flex; align-items: center; gap: 10px; cursor: pointer; font-size: 0.82rem; user-select: none; }
.morpheme-item input[type="checkbox"] { accent-color: var(--accent-color); width: 15px; height: 15px; cursor: pointer; }
.m-surface { font-weight: bold; color: var(--text-primary); min-width: 60px; }
.m-info { color: var(--text-muted); flex: 1; display: flex; align-items: center; gap: 8px; }
.is-excluded { opacity: 0.55; }
.is-excluded .m-surface { text-decoration: line-through; }
.is-normalized-out { color: var(--text-muted); }
.out-badge { font-size: 0.7rem; padding: 1px 5px; border-radius: 4px; background: rgba(0, 0, 0, 0.05); color: var(--text-muted); border: 1px solid var(--border-color); }
.any-placeholder { padding: 14px; text-align: center; color: var(--text-muted); font-size: 0.8rem; font-style: italic; }

.gap-connection-area { display: flex; flex-direction: column; align-items: center; margin: 4px 0; }
.gap-card { width: 90%; display: flex; align-items: center; gap: 12px; padding: 8px 16px; border-radius: 8px; background: rgba(0, 0, 0, 0.02); border: 1px dashed var(--border-color); box-sizing: border-box; }
.gap-icon { font-size: 1.1rem; font-weight: bold; color: var(--text-muted); }
.gap-text { font-size: 0.8rem; font-weight: 500; color: var(--text-secondary); flex: 1; text-align: center; }
.gap-delete-btn { border: 0; background: transparent; font-size: 1.25rem; line-height: 1; cursor: pointer; color: #cc3f45; padding: 0 4px; }
.gap-divider { width: 100%; display: flex; justify-content: center; }
.gap-insert-btn { border: 0; background: transparent; color: var(--text-muted); font-size: 0.75rem; font-weight: bold; cursor: pointer; padding: 6px 12px; border-radius: 6px; transition: all 0.2s ease; }
.gap-insert-btn:hover { color: var(--accent-color); background: rgba(101, 66, 143, 0.06); }

.top-signature-area { margin-bottom: 6px; display: grid; gap: 10px; }
.signature-row { display: grid; gap: 4px; }
.preview-title { font-size: 0.8rem; color: var(--text-muted); font-weight: bold; display: block; }
.name-display-box { padding: 10px 14px; border-radius: 8px; background: var(--bg-secondary); border: 1px solid var(--border-color); display: flex; align-items: center; }
.name-display-box strong { font-family: "BIZ UDPGothic", "Hiragino Sans", "Hiragino Kaku Gothic ProN", Meiryo, "MS PGothic", system-ui, sans-serif; font-size: 1.15rem; color: var(--text-primary); font-weight: 700; }
.signature-box { padding: 12px 14px; border-radius: 8px; background: var(--bg-secondary); border: 1px solid var(--border-color); overflow-x: auto; }
.signature-box code { font-family: Consolas, Monaco, "BIZ UDPGothic", "Hiragino Sans", Meiryo, monospace; font-size: 0.85rem; color: var(--accent-color); white-space: pre; }
.validation-message { color: #d32f2f; font-size: 0.8rem; font-weight: bold; flex: 1; text-align: left; }
.expression-editor footer { display: flex; justify-content: space-between; align-items: center; gap: 14px; margin-top: 18px; border-top: 1px solid var(--border-color); padding-top: 14px; }
.btn-group { display: flex; gap: 10px; margin-left: auto; }
.expression-editor footer button { padding: 9px 18px; border-radius: 8px; cursor: pointer; font-size: 0.85rem; font-weight: 500; transition: all 0.2s; }
.secondary { border: 1px solid var(--border-color); background: transparent; color: var(--text-secondary); }
.secondary:hover { background: var(--bg-secondary); }
.primary { border: 1px solid var(--accent-color); background: var(--accent-color); color: #fff; }
.primary:hover { background: var(--accent-hover); border-color: var(--accent-hover); }
.primary:disabled { cursor: not-allowed; opacity: 0.45; }
</style>
