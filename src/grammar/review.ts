import { ref } from "vue";
import type { GrammarReviewStatus } from "../types";

export interface GrammarReviewOverride {
  status: GrammarReviewStatus;
  reviewer: string;
  reviewedAt: string;
}

const STORAGE_KEY = "kotoclip.grammar-review-overrides.v1";

function loadOverrides(): Record<string, GrammarReviewOverride> {
  try {
    const value = localStorage.getItem(STORAGE_KEY);
    return value ? JSON.parse(value) as Record<string, GrammarReviewOverride> : {};
  } catch {
    return {};
  }
}

export const grammarReviewOverrides = ref<Record<string, GrammarReviewOverride>>(loadOverrides());

export function setGrammarReviewOverride(
  conceptId: string,
  status: GrammarReviewStatus,
  reviewer = "",
) {
  grammarReviewOverrides.value = {
    ...grammarReviewOverrides.value,
    [conceptId]: {
      status,
      reviewer: reviewer.trim(),
      reviewedAt: new Date().toISOString().slice(0, 10),
    },
  };
  localStorage.setItem(STORAGE_KEY, JSON.stringify(grammarReviewOverrides.value));
}

export function clearGrammarReviewOverride(conceptId: string) {
  const next = { ...grammarReviewOverrides.value };
  delete next[conceptId];
  grammarReviewOverrides.value = next;
  localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
}
