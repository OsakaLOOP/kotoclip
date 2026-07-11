<script setup lang="ts">
import { computed } from "vue";
import { AnnotatedToken } from "../types";

const props = defineProps<{
  token: AnnotatedToken;
  paragraphId: number;
  tokenIndex: number;
  isDragSelected: boolean;
}>();

// 根据生词得分和状态计算 CSS 类
const capsuleClasses = computed(() => {
  const t = props.token;
  const expressionClasses = t.expressions.length > 0
    ? { "has-expression": true, [`expression-${t.expressions[0].position}`]: true }
    : {};
  
  // 标点符号特殊处理
  const isPunc = t.bunsetsu.morphemes.length === 1 && t.bunsetsu.morphemes[0].pos.major === "記号";
  if (isPunc) {
    return {
      "bunsetsu-capsule": true,
      "punctuation": true,
      ...expressionClasses,
    };
  }

  // 已知词汇样式退化为普通正文
  if (t.is_known) {
    return {
      "bunsetsu-capsule": true,
      "is-known": true,
      ...expressionClasses,
    };
  }

  let noveltyClass = "novelty-low";
  if (t.novelty_score > 0.6) {
    noveltyClass = "novelty-high";
  } else if (t.novelty_score >= 0.2) {
    noveltyClass = "novelty-mid";
  }

  return {
    "bunsetsu-capsule": true,
    [noveltyClass]: true,
    "is-selected": t.is_selected,
    "drag-over": props.isDragSelected,
    ...expressionClasses,
  };
});

// 后端的 head_word 可能由多个形态素组成（如 警察 + 署）。
const headMorphemeIndices = computed(() => {
  const morphemes = props.token.bunsetsu.morphemes;
  const head = props.token.bunsetsu.head_word;

  for (let start = 0; start < morphemes.length; start++) {
    let surface = "";
    let baseForm = "";
    for (let end = start; end < morphemes.length; end++) {
      surface += morphemes[end].surface;
      baseForm += morphemes[end].base_form;
      if (surface === head.surface || baseForm === head.base_form) {
        return new Set(Array.from({ length: end - start + 1 }, (_, i) => start + i));
      }
      if (!head.surface.startsWith(surface) && !head.base_form.startsWith(baseForm)) {
        break;
      }
    }
  }

  return new Set<number>();
});

function isHeadMorpheme(index: number) {
  return headMorphemeIndices.value.has(index);
}

function isGrammarMorpheme(index: number) {
  const m = props.token.bunsetsu.morphemes[index];
  return props.token.bunsetsu.grammar_tags.some((tag) => m.char_range[0] >= tag.char_range[0] && m.char_range[1] <= tag.char_range[1]);
}

function isExpressionMorpheme(index: number) {
  const morpheme = props.token.bunsetsu.morphemes[index];
  return props.token.expressions.some((expression) =>
    morpheme.char_range[0] >= expression.char_range[0]
    && morpheme.char_range[1] <= expression.char_range[1]
  );
}
</script>

<template>
  <span
    :class="capsuleClasses"
    :data-paragraph-id="paragraphId"
    :data-token-index="tokenIndex"
  >
    <!-- 遍历渲染形态素，区分自立语与附属语 -->
    <span
      v-for="(m, idx) in token.bunsetsu.morphemes"
      :key="idx"
      :class="{ 'head-word-highlight': isHeadMorpheme(idx), 'helper-word': !isHeadMorpheme(idx), 'grammar-match': isGrammarMorpheme(idx), 'expression-anchor': isExpressionMorpheme(idx) }"
    >
      {{ m.surface }}
    </span>

    <!-- 跨文节表达使用细连接带，不改变原文节胶囊大小。 -->
    <span
      v-for="expression in token.expressions.filter((item) => item.position === 'start' || item.position === 'single')"
      :key="expression.match_id"
      class="expression-badge"
      :title="expression.description || expression.surface"
    >
      {{ expression.label }}
    </span>

    <!-- 渲染语法 Badge 徽章 -->
    <span
      v-for="tag in token.bunsetsu.grammar_tags"
      :key="tag.pattern_id"
      class="grammar-badge"
      :title="tag.description"
    >
      {{ tag.name_ja }}
    </span>
  </span>
</template>
