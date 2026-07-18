import type { AnnotatedToken, Morpheme, MorphologyChain, PosTag } from "../types";
import {
  morphologyChainForMorpheme,
  morphologyDisplayReading,
  morphologyLemma,
  morphologyLookupReading,
  morphologyPos,
} from "../explanation/morphologyView.ts";

export interface MorphemeLookupTarget {
  chain: MorphologyChain | null;
  surface: string;
  lemma: string;
  query: string;
  reading: string;
  lookupReading: string;
  pos: PosTag;
  charRange: [number, number];
}

/** 合并词形只改变悬浮目标与显示，不改变现有词典查询词策略。 */
export function morphemeLookupTarget(token: AnnotatedToken, morpheme: Morpheme) {
  const chain = morphologyChainForMorpheme(token, morpheme);
  if (!chain) {
    const lemma = dictionaryLemma(morpheme);
    return {
      chain: null,
      surface: morpheme.surface,
      lemma,
      query: lemma,
      reading: morpheme.reading,
      lookupReading: morpheme.reading,
      pos: morpheme.pos,
      charRange: morpheme.char_range,
    } satisfies MorphemeLookupTarget;
  }
  const reading = morphologyLookupReading(token, chain);
  return {
    chain,
    surface: chain.surface_form,
    lemma: morphologyLemma(chain),
    query: chain.lookup_form || chain.dictionary_form,
    reading: morphologyDisplayReading(token, chain),
    lookupReading: reading,
    pos: morphologyPos(token, chain),
    charRange: chain.char_range,
  } satisfies MorphemeLookupTarget;
}

/** 词典查询只把独立词的 base_form 当作词头；功能成分保留实际表面。 */
export function dictionaryLemma(morpheme: Morpheme) {
  if (
    morpheme.pos.major === "助詞"
    || morpheme.pos.major === "助動詞"
    || morpheme.pos.major === "動詞" && morpheme.pos.sub1 === "接尾"
  ) {
    return morpheme.surface;
  }
  return morpheme.base_form && morpheme.base_form !== "*" ? morpheme.base_form : morpheme.surface;
}

/**
 * 选择胶囊级词典入口。精确词典整体优先；生产型构词没有整体词条时，
 * 使用规则声明的词头语素，禁止拿拼接读音做无表记回退。
 */
export function dictionaryTargetForToken(token: AnnotatedToken) {
  const lexical = token.bunsetsu.lexical_units[0];
  if (lexical) {
    return { word: lexical.base_form, reading: lexical.reading, pos: lexical.output_pos };
  }
  const formation = token.bunsetsu.word_formations[0];
  if (formation) {
    const morpheme = token.bunsetsu.morphemes[formation.head_morpheme];
    if (morpheme) {
      return {
        word: dictionaryLemma(morpheme),
        reading: morpheme.reading,
        pos: morpheme.pos,
      };
    }
  }
  return {
    word: dictionaryLemma({
      surface: token.bunsetsu.head_word.surface,
      base_form: token.bunsetsu.head_word.base_form,
      reading: token.bunsetsu.head_word.reading,
      pos: token.bunsetsu.head_word.pos,
      conjugation_type: "*",
      conjugation_form: "*",
      char_range: token.bunsetsu.char_range,
    }),
    reading: token.bunsetsu.head_word.reading,
    pos: token.bunsetsu.head_word.pos,
  };
}
