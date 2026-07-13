<script setup lang="ts">
import { computed } from "vue";
import { AnnotatedToken } from "../types";

const props = defineProps<{
  token: AnnotatedToken;
  paragraphId: number;
  tokenIndex: number;
  isDragSelected: boolean;
  tokens?: AnnotatedToken[];
}>();

const hasSentencePause = computed(() => {
  if (!props.tokens || props.token.display_class !== "content") return false;
  const index = props.tokenIndex;
  let i = index - 1;
  let foundPuncs: AnnotatedToken[] = [];
  while (i >= 0 && props.tokens[i].display_class === "punctuation") {
    foundPuncs.unshift(props.tokens[i]);
    i--;
  }
  if (foundPuncs.length === 0) return false;
  const puncStr = foundPuncs.map(t => t.bunsetsu.surface).join("");
  return /[。！？…].*$/.test(puncStr);
});

// 根据生词得分和状态计算 CSS 类
const capsuleClasses = computed(() => {
  const t = props.token;
  const expressionClasses = t.expressions.length > 0
    ? {
        [`expression-${t.expressions[0].position}`]: true,
        [`expression-type-${t.expressions[0].expression_type}`]: true,
        [`expression-boundary-${t.expressions[0].boundary_effect}`]: true,
      }
    : {};
  
  // 换行符特殊处理
  if (t.display_class === "line_break") {
    return {
      "bunsetsu-capsule": true,
      "line-break": true,
      ...expressionClasses,
    };
  }

  // 标点符号特殊处理
  if (t.display_class === "punctuation") {
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
      "is-selected": t.is_selected,
      "drag-over": props.isDragSelected,
      "sentence-pause-before": hasSentencePause.value,
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
    "sentence-pause-before": hasSentencePause.value,
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
  return props.token.expressions.some((expression) => {
    const ranges = expression.matched_ranges.length > 0 ? expression.matched_ranges : [expression.char_range];
    return ranges.some((range) => morpheme.char_range[0] >= range[0] && morpheme.char_range[1] <= range[1]);
  });
}
</script>

<template>
  <span
    :class="[capsuleClasses, { 'has-headword': headMorphemeIndices.size > 0 }]"
    :data-paragraph-id="paragraphId"
    :data-token-index="tokenIndex"
  >
    <!-- 遍历渲染形态素，区分自立语与附属语 -->
    <span
      v-for="(m, idx) in token.bunsetsu.morphemes"
      :key="idx"
      :data-morpheme-index="idx"
      :class="{ 'head-word-highlight': isHeadMorpheme(idx), 'helper-word': !isHeadMorpheme(idx), 'grammar-match': isGrammarMorpheme(idx), 'expression-anchor': isExpressionMorpheme(idx) }"
    >
      {{ m.surface }}
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
