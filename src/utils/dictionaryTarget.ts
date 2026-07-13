import type { AnnotatedToken } from "../types";

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
        word: morpheme.base_form && morpheme.base_form !== "*" ? morpheme.base_form : morpheme.surface,
        reading: morpheme.reading,
      };
    }
  }
  return {
    word: token.bunsetsu.head_word.base_form,
    reading: token.bunsetsu.head_word.reading,
  };
}
