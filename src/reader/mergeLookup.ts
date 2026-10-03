import type { UnifiedDocument } from "../types/nlp";

export function selectionExcludedRanges(document: UnifiedDocument, offset = 0): [number, number][] {
  return document.morphemes.filter((token) =>
    ["記号", "補助記号", "空白"].includes(token.pos[0] ?? "") || token.pos.includes("数詞"))
    .map((token) => [token.char_range[0] + offset, token.char_range[1] + offset]);
}

export function constrainMergeRange(
  characters: string[],
  range: [number, number],
  anchor: number,
  excluded: [number, number][],
): [number, number] | null {
  const allowed = (index: number) => /^[\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}ー々〆〻]$/u.test(characters[index] ?? "")
    && !excluded.some(([start, end]) => start <= index && index < end);
  if (range[0] >= range[1]) return null;
  const origin = Math.max(range[0], Math.min(anchor, range[1] - 1));
  if (!allowed(origin)) return null;
  let start = origin;
  let end = origin + 1;
  while (start > range[0] && allowed(start - 1)) start--;
  while (end < range[1] && allowed(end)) end++;
  return [start, end];
}

export function mergedLookupTarget(document: UnifiedDocument) {
  const characters = Array.from(document.text);
  const roots = document.morphology.chains.filter((chain) => chain.role === "lexical" && !chain.parent_chain_id);
  const ending = roots.filter((root) => {
    const descendants = new Set([root.chain_id]);
    let end = root.char_range[1];
    for (let changed = true; changed;) {
      changed = false;
      for (const child of document.morphology.chains) {
        if (!child.parent_chain_id || !descendants.has(child.parent_chain_id) || descendants.has(child.chain_id)) continue;
        descendants.add(child.chain_id);
        if (child.char_range[0] <= end) end = Math.max(end, child.char_range[1]);
        changed = true;
      }
    }
    return end === characters.length;
  }).sort((left, right) => right.char_range[0] - left.char_range[0])[0];
  const base = ending?.lookup_form || ending?.lemma_form || ending?.dictionary_form;
  const canonical = base ? characters.slice(0, ending!.char_range[0]).join("") + base : document.text;
  const reading = canonical === document.text
    ? document.morphemes.map((token) => token.reading || "").join("") || null
    : null;
  return {
    morpheme_ids: document.morphemes.map((token) => token.id),
    lexical_core_ids: roots.map((chain) => chain.chain_id),
    source_formation_ids: [],
    lookup_forms: [{ kind: canonical === document.text ? "observed" : "canonical", form: canonical, reading }],
    reading_evidence: reading ? [reading] : [],
  };
}
