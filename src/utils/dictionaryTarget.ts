import type { AnnotatedToken, Morpheme, MorphologyChain } from "../types";

export function morphologyChainForMorpheme(
  token: AnnotatedToken,
  morpheme: Morpheme,
  role?: MorphologyChain["role"],
) {
  return token.bunsetsu.morphology.chains.find((chain) => (
    (!role || chain.role === role)
    && chain.source_ranges.some((range) => (
      morpheme.char_range[0] >= range[0] && morpheme.char_range[1] <= range[1]
    ))
  ));
}

/** 合并词形只改变悬浮目标与显示，不改变现有词典查询词策略。 */
export function morphemeLookupTarget(token: AnnotatedToken, morpheme: Morpheme) {
  const chain = morphologyChainForMorpheme(token, morpheme, "lexical");
  if (!chain) return { morpheme, chain: null };
  return {
    chain,
    morpheme: {
      ...morpheme,
      surface: chain.surface_form,
      base_form: token.bunsetsu.head_word.base_form,
      reading: token.bunsetsu.head_word.reading,
      pos: token.bunsetsu.head_word.pos,
      char_range: chain.char_range,
    } satisfies Morpheme,
  };
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
    return { word: lexical.base_form, reading: lexical.reading };
  }
  const formation = token.bunsetsu.word_formations[0];
  if (formation) {
    const morpheme = token.bunsetsu.morphemes[formation.head_morpheme];
    if (morpheme) {
      return {
        word: dictionaryLemma(morpheme),
        reading: morpheme.reading,
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
  };
}
