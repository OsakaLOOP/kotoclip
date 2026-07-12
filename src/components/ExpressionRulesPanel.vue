<script setup lang="ts">
import type { ExpressionRule } from "../types";

defineProps<{
  show: boolean;
  rules: ExpressionRule[];
}>();

const emit = defineEmits<{
  (event: "close"): void;
  (event: "delete", id: number): void;
}>();

function patternPreview(rule: ExpressionRule): string {
  const formatPart = (part: any, index: number, total: number) => {
    const text = part.surface_hint || part.lemmas.join("+");
    let formatted = part.is_slot ? `{${text}}` : text;
    
    if (index === 0 && part.alignment === "suffix") {
      formatted = `～${formatted}`;
    }
    if (index === total - 1 && part.alignment === "prefix") {
      formatted = `${formatted}～`;
    }
    return formatted;
  };

  const len = rule.parts.length;
  const gap = rule.gap_after;
  if (gap !== undefined && gap !== null) {
    const head = rule.parts.slice(0, gap + 1).map((p, idx) => formatPart(p, idx, len)).join(" + ");
    const tail = rule.parts.slice(gap + 1).map((p, idx) => formatPart(p, idx + gap + 1, len)).join(" + ");
    return `${head}  ○  ${tail}`;
  } else {
    return rule.parts.map((p, idx) => formatPart(p, idx, len)).join(" + ");
  }
}

const typeLabels: Record<ExpressionRule["expression_type"], string> = {
  lexical_unit: "词汇单位",
  idiom: "固定惯用语",
  grammar_construction: "语法构式",
  correlative: "非连续呼应",
};
</script>

<template>
  <Transition name="slide-panel">
    <aside v-if="show" class="expression-panel" aria-label="跨文节表达规则">
      <header class="expression-panel-header">
        <div>
          <h2>表达规则</h2>
          <p>在正文中拖选实例，配置类型、范围和结构约束。</p>
        </div>
        <button class="panel-close" aria-label="关闭" @click="emit('close')">×</button>
      </header>

      <div v-if="rules.length === 0" class="expression-empty">
        暂无规则。拖拽选择常用表达后，会在这里显示并自动复用。
      </div>
      <ol v-else class="expression-rule-list">
        <li v-for="rule in rules" :key="rule.id" class="expression-rule-item">
          <div class="expression-rule-copy">
            <strong>{{ rule.label }}</strong>
            <small>{{ typeLabels[rule.expression_type] }} · 优先级 {{ rule.priority }} · {{ rule.boundary_effect === 'merge_lexical_unit' ? '合并边界' : '仅注解' }}</small>
            <span>{{ patternPreview(rule) }}</span>
            <small v-if="rule.description">{{ rule.description }}</small>
          </div>
          <button class="expression-delete" @click="emit('delete', rule.id)">删除</button>
        </li>
      </ol>
    </aside>
  </Transition>
</template>

<style scoped>
.expression-panel {
  position: fixed;
  z-index: 1100;
  top: 0;
  right: 0;
  width: min(390px, 92vw);
  height: 100vh;
  box-sizing: border-box;
  padding: 22px;
  overflow-y: auto;
  background: var(--bg-primary);
  border-left: 1px solid var(--border-color);
  box-shadow: -10px 0 30px rgba(37, 26, 49, 0.12);
}

.expression-panel-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 22px;
}

.expression-panel-header h2 { margin: 0; font-size: 1.15rem; }
.expression-panel-header p { margin: 5px 0 0; color: var(--text-secondary); font-size: 0.82rem; }
.panel-close { border: 0; background: transparent; font-size: 1.5rem; cursor: pointer; }
.expression-empty { color: var(--text-secondary); font-size: 0.9rem; line-height: 1.7; }
.expression-rule-list { display: grid; gap: 10px; margin: 0; padding: 0; list-style: none; }
.expression-rule-item { display: flex; align-items: center; gap: 12px; padding: 12px; border: 1px solid var(--border-color); border-radius: 8px; }
.expression-rule-copy { min-width: 0; display: grid; gap: 4px; flex: 1; }
.expression-rule-copy strong, .expression-rule-copy span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.expression-rule-copy span, .expression-rule-copy small { color: var(--text-secondary); font-size: 0.75rem; }
.expression-delete { flex: 0 0 auto; border: 0; background: transparent; color: #9a3f45; cursor: pointer; }
.slide-panel-enter-active, .slide-panel-leave-active { transition: transform 0.18s ease; }
.slide-panel-enter-from, .slide-panel-leave-to { transform: translateX(100%); }
</style>
