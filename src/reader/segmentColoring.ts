import type { AnnotatedToken, Bunsetsu } from "../types";

export type ReaderSegmentKind = "lexical" | "grammar" | "helper";

export interface ReaderMorphemeSegment {
  kind: ReaderSegmentKind;
  tone: 0 | 1 | null;
}

function containsMorpheme(range: [number, number], index: number) {
  return range[0] <= index && index < range[1];
}

function containsRange(container: [number, number], inner: [number, number]) {
  return inner[0] >= container[0] && inner[1] <= container[1];
}

function morphologyChainForMorpheme(
  token: AnnotatedToken,
  index: number,
  role: "lexical" | "functional",
) {
  const morpheme = token.bunsetsu.morphemes[index];
  return token.bunsetsu.morphology.chains.find((chain) => (
    chain.role === role
    && chain.source_ranges.some((range) => containsRange(range, morpheme.char_range))
  )) ?? null;
}

function formationCaptureKey(bunsetsu: Bunsetsu, index: number) {
  for (const formation of bunsetsu.word_formations) {
    for (const capture of formation.captures) {
      if (containsMorpheme(capture.morpheme_range, index)) {
        return `${formation.rule_id}:${capture.name}:${capture.morpheme_range.join("-")}`;
      }
    }
    if (containsMorpheme(formation.morpheme_range, index)) {
      return `${formation.rule_id}:formation:${formation.morpheme_range.join("-")}`;
    }
  }
  return null;
}

function lexicalSegmentKey(token: AnnotatedToken, index: number, fallbackHeadIndices: ReadonlySet<number>) {
  const formation = formationCaptureKey(token.bunsetsu, index);
  if (formation) return `formation:${formation}`;

  const lexicalUnit = token.bunsetsu.lexical_units.find((unit) => (
    containsMorpheme(unit.morpheme_range, index)
  ));
  if (lexicalUnit) return `lexical-unit:${lexicalUnit.morpheme_range.join("-")}`;

  const morpheme = token.bunsetsu.morphemes[index];
  const chain = morphologyChainForMorpheme(token, index, "lexical");
  if (chain) return `morphology:${chain.chain_id}`;
  if (fallbackHeadIndices.has(index)) return `head:${index}`;
  if (["名詞", "動詞", "形容詞", "副詞", "連体詞", "感動詞", "接続詞"].includes(morpheme.pos.major)) {
    return `lexical:${index}`;
  }
  return null;
}

function grammarSegmentKey(token: AnnotatedToken, index: number) {
  const grammarIndex = grammarIndexForMorpheme(token, index);
  if (grammarIndex !== undefined) return `tag:${grammarIndex}`;

  const chain = morphologyChainForMorpheme(token, index, "functional");
  return chain ? `morphology:${chain.chain_id}` : null;
}

export function fallbackHeadMorphemeIndices(bunsetsu: Bunsetsu) {
  const { morphemes, head_word: head } = bunsetsu;
  for (let start = 0; start < morphemes.length; start++) {
    let surface = "";
    let baseForm = "";
    for (let end = start; end < morphemes.length; end++) {
      surface += morphemes[end].surface;
      baseForm += morphemes[end].base_form;
      if (surface === head.surface || baseForm === head.base_form) {
        return new Set(Array.from({ length: end - start + 1 }, (_, offset) => start + offset));
      }
      if (!head.surface.startsWith(surface) && !head.base_form.startsWith(baseForm)) break;
    }
  }
  return new Set<number>();
}

export function grammarIndexForMorpheme(token: AnnotatedToken, index: number) {
  const range = token.bunsetsu.morphemes[index].char_range;
  const priority = { grammar_construction: 3, functional_morpheme: 2, morphology_feature: 1 } as const;
  let bestIndex: number | undefined;
  let bestPriority = -1;
  token.bunsetsu.grammar_tags.forEach((tag, tagIndex) => {
    const displayRanges = tag.display_ranges.length > 0 ? tag.display_ranges : [tag.char_range];
    const covered = displayRanges.some((display) => containsRange(display, range));
    const currentPriority = priority[tag.occurrence_kind as keyof typeof priority] ?? 0;
    if (covered && currentPriority > bestPriority) {
      bestIndex = tagIndex;
      bestPriority = currentPriority;
    }
  });
  return bestIndex;
}

export function readerMorphemeSegments(token: AnnotatedToken): ReaderMorphemeSegment[] {
  const fallbackIndices = fallbackHeadMorphemeIndices(token.bunsetsu);
  let lexicalTone: 0 | 1 = 0;
  let grammarTone: 0 | 1 = 0;
  let previousLexicalKey: string | null = null;
  let previousGrammarKey: string | null = null;

  return token.bunsetsu.morphemes.map((_, index) => {
    const grammar = grammarSegmentKey(token, index);
    if (grammar) {
      if (grammar !== previousGrammarKey) grammarTone = grammarTone === 0 ? 1 : 0;
      previousGrammarKey = grammar;
      previousLexicalKey = null;
      return { kind: "grammar", tone: grammarTone };
    }

    const lexical = lexicalSegmentKey(token, index, fallbackIndices);
    if (lexical) {
      if (lexical !== previousLexicalKey) lexicalTone = lexicalTone === 0 ? 1 : 0;
      previousLexicalKey = lexical;
      previousGrammarKey = null;
      return { kind: "lexical", tone: lexicalTone };
    }

    previousLexicalKey = null;
    previousGrammarKey = null;
    return { kind: "helper", tone: null };
  });
}
