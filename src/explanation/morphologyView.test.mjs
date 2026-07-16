import assert from "node:assert/strict";
import test from "node:test";
import {
  morphologyChainForMorpheme,
  morphologyLemma,
  morphologyPosLabel,
  primaryMorphologyChain,
  readingForMorphologyLemma,
} from "./morphologyView.ts";
import { morphemeLookupTarget } from "../utils/dictionaryTarget.ts";

const pos = (major, sub1) => ({ major, sub1, sub2: "*", sub3: "*" });

function chain({ id, range, role = "lexical", surface, dictionary, lemma, lookup, anchor, operators = [] }) {
  return {
    chain_id: id,
    anchor_morpheme: 0,
    anchor_range: anchor,
    morpheme_range: [0, 1],
    char_range: range,
    role,
    base_lexeme: dictionary,
    surface_form: surface,
    dictionary_form: dictionary,
    lemma_form: lemma,
    lookup_form: lookup,
    source_ranges: [],
    operators,
    connection_forms: [],
    feature_candidates: [],
    evidence: [],
  };
}

function token(morphemes, chains, head) {
  for (const item of chains) {
    item.source_ranges = morphemes
      .filter((morpheme) => morpheme.char_range[0] >= item.char_range[0] && morpheme.char_range[1] <= item.char_range[1])
      .map((morpheme) => morpheme.char_range);
  }
  return {
    bunsetsu: {
      morphemes,
      surface: morphemes.map((item) => item.surface).join(""),
      head_word: head,
      grammar_tags: [],
      morphology: { chains },
      word_formations: [],
      lexical_units: [],
      char_range: [morphemes[0].char_range[0], morphemes.at(-1).char_range[1]],
    },
    novelty_score: 1,
    is_selected: false,
    is_known: false,
    inference_reason: null,
    expressions: [],
    display_class: "content",
  };
}

test("サ变词干、する与ます共享显示原型和黄色词汇范围", () => {
  const morphemes = [
    { surface: "説明", base_form: "説明", reading: "セツメイ", char_range: [0, 2], pos: pos("名詞", "サ変接続"), conjugation_type: "*", conjugation_form: "*" },
    { surface: "し", base_form: "する", reading: "シ", char_range: [2, 3], pos: pos("動詞", "自立"), conjugation_type: "サ変・スル", conjugation_form: "連用形" },
    { surface: "ます", base_form: "ます", reading: "マス", char_range: [3, 5], pos: pos("助動詞", "*"), conjugation_type: "特殊・マス", conjugation_form: "基本形" },
  ];
  const morphology = chain({
    id: "morph:0:5", range: [0, 5], surface: "説明します", dictionary: "説明する",
    lemma: "説明する", lookup: "説明", anchor: [2, 3],
  });
  const value = token(morphemes, [morphology], {
    surface: "説明します", base_form: "説明", reading: "セツメイ", pos: morphemes[0].pos,
  });

  assert.equal(primaryMorphologyChain(value), morphology);
  for (const morpheme of morphemes) {
    assert.equal(morphologyChainForMorpheme(value, morpheme, "lexical"), morphology);
    const target = morphemeLookupTarget(value, morpheme);
    assert.equal(target.lemma, "説明する");
    assert.equal(target.query, "説明");
    assert.equal(target.surface, "説明します");
    assert.equal(target.reading, "セツメイ");
  }
  assert.equal(morphologyPosLabel(morphology, morphemes[0].pos), "動詞 · サ変");
  assert.equal(readingForMorphologyLemma(morphology, "セツメイ"), "セツメイスル");
});

test("同一文节的やすく使用自己的原型和查询词，不回退到分かる", () => {
  const morphemes = [
    { surface: "分かり", base_form: "分かる", reading: "ワカリ", char_range: [0, 3], pos: pos("動詞", "自立"), conjugation_type: "五段・ラ行", conjugation_form: "連用形" },
    { surface: "やすく", base_form: "やすい", reading: "ヤスク", char_range: [3, 6], pos: pos("形容詞", "非自立"), conjugation_type: "形容詞・アウオ段", conjugation_form: "連用テ接続" },
  ];
  const main = chain({ id: "morph:0:3", range: [0, 3], surface: "分かり", dictionary: "分かる", lemma: "分かる", lookup: "分かる", anchor: [0, 3] });
  const suffix = chain({ id: "morph:3:6", range: [3, 6], surface: "やすく", dictionary: "やすい", lemma: "やすい", lookup: "やすい", anchor: [3, 6] });
  const value = token(morphemes, [main, suffix], {
    surface: "分かり", base_form: "分かる", reading: "ワカリ", pos: morphemes[0].pos,
  });

  const target = morphemeLookupTarget(value, morphemes[1]);
  assert.equal(target.chain, suffix);
  assert.equal(target.lemma, "やすい");
  assert.equal(target.query, "やすい");
  assert.equal(target.lookupReading, "");
  assert.equal(target.reading, "");
  assert.equal(target.pos.major, "形容詞");
});

test("ナ形容词以な作为显示原型，内部仍保留だ形", () => {
  const morphology = chain({
    id: "morph:0:3", range: [0, 3], surface: "静かな", dictionary: "静かだ",
    lemma: "静かな", lookup: "静か", anchor: [2, 3],
  });
  assert.equal(morphologyLemma(morphology), "静かな");
  assert.equal(morphologyPosLabel(morphology, pos("名詞", "形容動詞語幹")), "形容詞 · ナ形");
  assert.equal(readingForMorphologyLemma(morphology, "しずか"), "しずかな");
});

test("功能活用链保留自己的词形身份", () => {
  const morphemes = [
    { surface: "くださっ", base_form: "くださる", reading: "クダサッ", char_range: [0, 4], pos: pos("動詞", "非自立"), conjugation_type: "五段・ラ行特殊", conjugation_form: "連用タ接続" },
    { surface: "た", base_form: "た", reading: "タ", char_range: [4, 5], pos: pos("助動詞", "*"), conjugation_type: "特殊・タ", conjugation_form: "基本形" },
  ];
  const morphology = chain({ id: "morph:0:5", range: [0, 5], role: "functional", surface: "くださった", dictionary: "くださる", lemma: "くださる", lookup: "くださる", anchor: [0, 4] });
  const value = token(morphemes, [morphology], {
    surface: "くださっ", base_form: "くださる", reading: "クダサッ", pos: morphemes[0].pos,
  });
  assert.equal(morphologyChainForMorpheme(value, morphemes[1], "functional"), morphology);
  assert.equal(morphemeLookupTarget(value, morphemes[1]).query, "くださる");
});
