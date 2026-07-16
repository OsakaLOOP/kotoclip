import type { GrammarTag } from "../types";

export function grammarTagCoversRange(tag: GrammarTag, range: [number, number]) {
  const displayRanges = tag.display_ranges.length > 0 ? tag.display_ranges : [tag.char_range];
  return displayRanges.some((display) => range[0] >= display[0] && range[1] <= display[1]);
}

export function primaryGrammarIndex(tags: GrammarTag[], range: [number, number]) {
  const priority = { grammar_construction: 3, functional_morpheme: 2, morphology_feature: 1 } as const;
  let bestIndex: number | undefined;
  let bestPriority = -1;
  tags.forEach((tag, index) => {
    const currentPriority = priority[tag.occurrence_kind as keyof typeof priority] ?? 0;
    if (grammarTagCoversRange(tag, range) && currentPriority > bestPriority) {
      bestIndex = index;
      bestPriority = currentPriority;
    }
  });
  return bestIndex;
}
