import assert from "node:assert/strict";
import test from "node:test";
import { morphemeLookupTarget } from "../utils/dictionaryTarget.ts";

function token(role = "lexical") {
  const morphemes = [
    {
      surface: "見え", base_form: "見える", reading: "ミエ", char_range: [0, 2],
      pos: { major: "動詞", sub1: "自立", sub2: "*", sub3: "*" },
      conjugation_type: "一段", conjugation_form: "未然形",
    },
    {
      surface: "なかっ", base_form: "ない", reading: "ナカッ", char_range: [2, 5],
      pos: { major: "助動詞", sub1: "*", sub2: "*", sub3: "*" },
      conjugation_type: "特殊・ナイ", conjugation_form: "連用タ接続",
    },
  ];
  return {
    bunsetsu: {
      morphemes,
      surface: "見えなかっ",
      head_word: { surface: "見えなかっ", base_form: "見える", reading: "ミエ", pos: morphemes[0].pos },
      grammar_tags: [],
      morphology: {
        chains: [{
          chain_id: "morph:0:5", anchor_morpheme: 0, anchor_range: [0, 2],
          morpheme_range: [0, 2], char_range: [0, 5], role,
          base_lexeme: "見える", surface_form: "見えなかっ", dictionary_form: "見える", lookup_form: "見える",
          source_ranges: [[0, 2], [2, 5]], operators: [], connection_forms: [], evidence: [],
        }],
      },
      word_formations: [], lexical_units: [], char_range: [0, 5],
    },
    novelty_score: 1, is_selected: false, is_known: false, inference_reason: null,
    expressions: [], display_class: "content",
  };
}

test("词汇活用链内任一语素使用合并词形，但保持原查询词", () => {
  const value = token();
  const target = morphemeLookupTarget(value, value.bunsetsu.morphemes[1]);
  assert.equal(target.chain?.role, "lexical");
  assert.equal(target.morpheme.surface, "見えなかっ");
  assert.equal(target.morpheme.base_form, "見える");
});

test("功能用言活用链不劫持词汇查词目标", () => {
  const value = token("functional");
  const focused = value.bunsetsu.morphemes[1];
  const target = morphemeLookupTarget(value, focused);
  assert.equal(target.chain, null);
  assert.equal(target.morpheme, focused);
});
