<script setup lang="ts">
import { computed } from "vue";
import aiCheckedIcon from "../../assets/grammar-review/ai-checked.svg";
import sourceAiIcon from "../../assets/grammar-review/source-ai.svg";
import sourceBuiltinIcon from "../../assets/grammar-review/source-builtin.svg";
import sourceHumanIcon from "../../assets/grammar-review/source-human.svg";
import trustedIcon from "../../assets/grammar-review/trusted.svg";
import unverifiedIcon from "../../assets/grammar-review/unverified.svg";
import type { GrammarProvenance, GrammarReviewStatus } from "../../types";

const props = defineProps<{
  provenance: GrammarProvenance;
  reviewStatus: GrammarReviewStatus;
  reviewer?: string;
  reviewedAt?: string;
}>();

const originLabel = computed(() => ({
  ai: "AI",
  human: "人工",
  builtin: "内置",
})[props.provenance.origin] ?? "来源");

const originIcon = computed(() => ({
  ai: sourceAiIcon,
  human: sourceHumanIcon,
  builtin: sourceBuiltinIcon,
})[props.provenance.origin] ?? sourceBuiltinIcon);

const reviewMeta = computed(() => ({
  unverified: { label: "未核验", icon: unverifiedIcon },
  ai_checked: { label: "AI 批量核验", icon: aiCheckedIcon },
  trusted: { label: props.reviewer ? `${props.reviewer} 核验` : "成员权威核验", icon: trustedIcon },
})[props.reviewStatus]);

const sourceText = computed(() => [
  props.provenance.author,
  props.provenance.date,
  `v${props.provenance.version}`,
].filter(Boolean).join(" · "));

const sourceTitle = computed(() => `${originLabel.value}生成 · ${sourceText.value}`);

const reviewTitle = computed(() => [
  reviewMeta.value.label,
  props.reviewedAt,
].filter(Boolean).join(" · "));
</script>

<template>
  <div class="grammar-trust-badges" aria-label="内容来源与核验状态">
    <span class="source-badge" :title="sourceTitle">
      <img :src="originIcon" alt="" />
      {{ sourceText }}
    </span>
    <span
      class="review-badge"
      :class="`review-${reviewStatus}`"
      :title="reviewTitle"
      :aria-label="reviewTitle"
    >
      <img :src="reviewMeta.icon" alt="" />
    </span>
  </div>
</template>

<style scoped>
.grammar-trust-badges { display: flex; flex-wrap: wrap; gap: 5px; align-items: center; }
.source-badge, .review-badge { display: inline-flex; align-items: center; min-height: 24px; padding: 2px 7px 2px 4px; border: 1px solid color-mix(in srgb, var(--border-color) 78%, transparent); border-radius: 999px; background: color-mix(in srgb, var(--bg-primary) 86%, transparent); color: var(--text-muted); font: 700 .62rem/1.35 var(--font-ui); white-space: nowrap; }
.source-badge img { width: 18px; height: 18px; margin-right: 4px; object-fit: contain; }
.review-badge { width: 26px; justify-content: center; padding: 2px; color: var(--text-secondary); }
.review-badge img { width: 19px; height: 19px; object-fit: contain; }
.review-ai_checked { border-color: color-mix(in srgb, #7762c8 28%, var(--border-color)); background: color-mix(in srgb, #7762c8 6%, var(--bg-primary)); color: #6551ae; }
.review-trusted { border-color: color-mix(in srgb, #42a66b 30%, var(--border-color)); background: color-mix(in srgb, #f6a8c6 7%, var(--bg-primary)); color: #347c52; }
</style>
