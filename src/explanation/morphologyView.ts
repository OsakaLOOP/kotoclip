import type { AnnotatedToken, MorphologyChain, MorphologyOperator, PosTag } from "../types";

export function morphologyLemma(chain: MorphologyChain) {
  return chain.lemma_form || chain.dictionary_form || chain.lookup_form || chain.surface_form;
}

export function primaryMorphologyChain(token: AnnotatedToken) {
  const head = token.bunsetsu.head_word;
  const chains = token.bunsetsu.morphology.chains;
  const lexical = chains.filter((chain) => chain.role === "lexical");
  return lexical.find((chain) => chain.surface_form === head.surface)
    ?? lexical.find((chain) => chain.lookup_form === head.base_form)
    ?? lexical.find((chain) => morphologyLemma(chain) === head.base_form)
    ?? chains.find((chain) => chain.lookup_form === head.base_form)
    ?? null;
}

export function morphologyPosLabel(chain: MorphologyChain | null, pos: PosTag) {
  const lemma = chain ? morphologyLemma(chain) : "";
  if (chain && lemma.endsWith("する") && chain.lookup_form !== lemma) return "動詞 · サ変";
  if (chain && lemma.endsWith("な") && chain.dictionary_form.endsWith("だ")) return "形容詞 · ナ形";
  return [pos.major, pos.sub1].filter((part) => part && part !== "*").join(" · ");
}

export function morphologySteps(chain: MorphologyChain | null): MorphologyOperator[] {
  const seen = new Set<string>();
  return (chain?.operators ?? []).filter((operator) => {
    const key = operator.concept_id || operator.output_state;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

export function readingForMorphologyLemma(chain: MorphologyChain | null, reading: string | null | undefined) {
  if (!chain || !reading) return reading ?? null;
  const lemma = morphologyLemma(chain);
  if (!lemma.startsWith(chain.lookup_form)) return reading;
  const suffix = lemma.slice(chain.lookup_form.length);
  if (!suffix) return reading;
  const usesKatakana = /[ァ-ヶ]/u.test(reading);
  const normalizedSuffix = usesKatakana
    ? Array.from(suffix).map((character) => (
        character >= "ぁ" && character <= "ゖ"
          ? String.fromCharCode(character.charCodeAt(0) + 0x60)
          : character
      )).join("")
    : suffix;
  return `${reading}${normalizedSuffix}`;
}
