import assert from "node:assert/strict";
import test from "node:test";
import { readerMorphemeSegments } from "./segmentColoring.ts";

function morpheme(surface, index, major = "名詞", sub1 = "一般") {
  return {
    surface,
    base_form: surface,
    reading: "*",
    pos: { major, sub1, sub2: "*", sub3: "*" },
    conjugation_type: "*",
    conjugation_form: "*",
    char_range: [index, index + Array.from(surface).length],
  };
}

function token({ morphemes, grammar_tags = [], word_formations = [], morphology = { chains: [] } }) {
  return {
    bunsetsu: {
      morphemes,
      surface: morphemes.map((item) => item.surface).join(""),
      head_word: { surface: morphemes[0].surface, base_form: morphemes[0].base_form, reading: "*", pos: morphemes[0].pos },
      grammar_tags,
      morphology,
      word_formations,
      lexical_units: [],
      char_range: [morphemes[0].char_range[0], morphemes.at(-1).char_range[1]],
    },
    novelty_score: 0.8,
    is_selected: false,
    is_known: false,
    inference_reason: null,
    expressions: [],
    display_class: "content",
  };
}

test("构词捕获将前缀、主干和接尾分为交替的黄色片段", () => {
  const value = token({
    morphemes: [morpheme("超", 0, "接頭詞", "名詞接続"), morpheme("能力", 1), morpheme("者", 3, "名詞", "接尾")],
    word_formations: [{
      rule_id: "prefix_noun_suffix",
      category: "productive_prefixed_noun",
      surface: "超能力者",
      base_form: "超能力者",
      reading: "*",
      output_pos: { major: "名詞", sub1: "一般", sub2: "*", sub3: "*" },
      morpheme_range: [0, 3],
      char_range: [0, 4],
      head_morpheme: 1,
      captures: [
        { name: "prefix", surface: "超", morpheme_range: [0, 1], char_range: [0, 1] },
        { name: "base", surface: "能力", morpheme_range: [1, 2], char_range: [1, 3] },
        { name: "suffix", surface: "者", morpheme_range: [2, 3], char_range: [3, 4] },
      ],
      confidence: 88,
    }],
  });

  assert.deepEqual(readerMorphemeSegments(value), [
    { kind: "lexical", tone: 1 },
    { kind: "lexical", tone: 0 },
    { kind: "lexical", tone: 1 },
  ]);
});

test("普通相邻名词与相邻语法 occurrence 各自交替，未覆盖助词保持灰色", () => {
  const value = token({
    morphemes: [
      morpheme("山", 0),
      morpheme("道", 1),
      morpheme("の", 2, "助詞", "連体化"),
      morpheme("に", 3, "助詞", "格助詞"),
      morpheme("は", 4, "助詞", "係助詞"),
      morpheme("を", 5, "助詞", "格助詞"),
    ],
    grammar_tags: [
      { pattern_id: "niwa", name_ja: "には", name_en: "niwa", jlpt_level: null, description: "", morpheme_range: [3, 5], char_range: [3, 5], occurrence_id: "niwa", concept_id: "niwa", occurrence_kind: "grammar_construction", status: "accepted", show_badge: true, display_ranges: [[3, 5]], selected_sense_id: null, sense_candidates: [], explanation: null },
      { pattern_id: "wo", name_ja: "を", name_en: "wo", jlpt_level: null, description: "", morpheme_range: [5, 6], char_range: [5, 6], occurrence_id: "wo", concept_id: "wo", occurrence_kind: "grammar_construction", status: "accepted", show_badge: true, display_ranges: [[5, 6]], selected_sense_id: null, sense_candidates: [], explanation: null },
    ],
  });

  assert.deepEqual(readerMorphemeSegments(value), [
    { kind: "lexical", tone: 1 },
    { kind: "lexical", tone: 0 },
    { kind: "helper", tone: null },
    { kind: "grammar", tone: 1 },
    { kind: "grammar", tone: 1 },
    { kind: "grammar", tone: 0 },
  ]);
});
