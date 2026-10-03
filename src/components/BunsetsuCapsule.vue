<script setup lang="ts">
import { computed } from "vue";
import { AnnotatedToken } from "../types";
import {
  grammarIndexForMorpheme,
  readerMorphemeSegments,
} from "../reader/segmentColoring";

const props = defineProps<{
  token: AnnotatedToken;
  paragraphId: number;
  tokenIndex: number;
  isDragSelected: boolean;
  tokens?: AnnotatedToken[];
  characterHits?: boolean;
  activeCharacter?: number | null;
  changedRange?: [number, number] | null;
  changedRanges?: [number, number][];
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

// 没有活用链的名词、复合词仍使用既有词头范围作为回退。
const morphemeSegments = computed(() => readerMorphemeSegments(props.token));

function isExpressionMorpheme(index: number) {
  const morpheme = props.token.bunsetsu.morphemes[index];
  return props.token.expressions.some((expression) => {
    const ranges = expression.matched_ranges.length > 0 ? expression.matched_ranges : [expression.char_range];
    return ranges.some((range) => morpheme.char_range[0] >= range[0] && morpheme.char_range[1] <= range[1]);
  });
}

function morphemeCharacters(morpheme: AnnotatedToken["bunsetsu"]["morphemes"][number]) {
  return Array.from(morpheme.surface).map((character, index) => ({
    character,
    offset: morpheme.char_range[0] + index,
  }));
}

function characterChanged(offset: number) {
  const ranges = props.changedRanges?.length
    ? props.changedRanges
    : props.changedRange
      ? [props.changedRange]
      : [];
  return ranges.some((range) => range[0] <= offset && offset < range[1]);
}
</script>

<template>
  <span
    :class="[capsuleClasses, { 'has-headword': morphemeSegments.some((segment) => segment.kind === 'lexical') }]"
    :data-paragraph-id="paragraphId"
    :data-token-index="tokenIndex"
  >
    <!-- 遍历渲染形态素，区分自立语与附属语 -->
    <span
      v-for="(m, idx) in token.bunsetsu.morphemes"
      :key="idx"
      :data-morpheme-index="idx"
      :data-grammar-index="grammarIndexForMorpheme(token, idx)"
      :class="{
        'head-word-highlight': morphemeSegments[idx].kind === 'lexical',
        'head-word-tone-a': morphemeSegments[idx].kind === 'lexical' && morphemeSegments[idx].tone === 0,
        'head-word-tone-b': morphemeSegments[idx].kind === 'lexical' && morphemeSegments[idx].tone === 1,
        'helper-word': morphemeSegments[idx].kind === 'helper',
        'grammar-match': morphemeSegments[idx].kind === 'grammar',
        'grammar-tone-a': morphemeSegments[idx].kind === 'grammar' && morphemeSegments[idx].tone === 0,
        'grammar-tone-b': morphemeSegments[idx].kind === 'grammar' && morphemeSegments[idx].tone === 1,
        'expression-anchor': isExpressionMorpheme(idx),
      }"
    >
      <template v-if="characterHits">
        <span
          v-for="item in morphemeCharacters(m)"
          :key="item.offset"
          class="audit-character"
          :class="{
            'audit-character--active': activeCharacter === item.offset,
            'audit-character--changed': characterChanged(item.offset),
          }"
          :data-character-offset="item.offset"
        >{{ item.character }}</span>
      </template>
      <template v-else>{{ m.surface }}</template>
    </span>
  </span>
</template>

<style scoped>
.audit-character {
  border-radius: 3px;
  transition: background-color 100ms ease, box-shadow 100ms ease;
}

.audit-character--changed {
  background: color-mix(in srgb, #d9534f 22%, transparent);
  box-shadow: inset 0 -2px #c53a32;
}

.audit-character--active {
  background: color-mix(in srgb, var(--accent-color) 18%, transparent);
  box-shadow: inset 0 -2px var(--accent-color);
}

@media (prefers-reduced-motion: reduce) {
  .audit-character { transition: none; }
}
</style>
