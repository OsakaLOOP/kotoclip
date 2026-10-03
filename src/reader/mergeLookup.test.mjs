import assert from "node:assert/strict";
import test from "node:test";
import { constrainMergeRange, mergedLookupTarget, selectionExcludedRanges } from "./mergeLookup.ts";
import { readerChains, readerMorphologyHits } from "./lookupPresentation.ts";

test("Shift 选区保留字符边界，正反向拖拽在标点处停止", () => {
  const characters = Array.from("取り戻した。読んだ");
  assert.deepEqual(constrainMergeRange(characters, [1, 4], 1, []), [1, 4]);
  assert.deepEqual(constrainMergeRange(characters, [1, 9], 1, []), [1, 5]);
  assert.deepEqual(constrainMergeRange(characters, [1, 9], 8, []), [6, 9]);
  assert.equal(constrainMergeRange(characters, [5, 6], 5, []), null);
  assert.equal(constrainMergeRange(characters, [2, 2], 2, []), null);
});

test("字母、数字、空白和符号分隔连续查词范围", () => {
  for (const separator of ["A", "ｚ", "α", "1", "９", " ", "\n", "！", "🙂"]) {
    const characters = Array.from(`読む${separator}書く`);
    assert.deepEqual(constrainMergeRange(characters, [0, characters.length], 0, []), [0, 2]);
    assert.deepEqual(constrainMergeRange(characters, [0, characters.length], characters.length - 1, []), [3, 5]);
  }
  assert.deepEqual(constrainMergeRange(Array.from("𠮷々スーパー"), [0, 6], 0, []), [0, 6]);
});

test("复用词性排除范围，活用附属成分可参与合并", () => {
  const document = { morphemes: [
    { pos: ["動詞"], char_range: [0, 2] },
    { pos: ["助動詞"], char_range: [2, 3] },
    { pos: ["名詞", "数詞"], char_range: [3, 4] },
    { pos: ["補助記号"], char_range: [4, 5] },
  ] };
  assert.deepEqual(selectionExcludedRanges(document, 10), [[13, 14], [14, 15]]);
  assert.deepEqual(constrainMergeRange(Array.from("読んだ一。"), [0, 5], 0, selectionExcludedRanges(document)), [0, 3]);
});

function inflectedDocument(prefix = "") {
  const offset = Array.from(prefix).length;
  const state = { conjugation_type: "五段", conjugation_form: "連用形", category: "動詞", form: "continuative" };
  return {
    text: `${prefix}読んでいた`,
    morphemes: [
      { id: "read", surface: "読ん", reading: "ヨン", pos: ["動詞"], char_range: [offset, offset + 2] },
      { id: "aux", surface: "でいた", reading: "デイタ", pos: ["助動詞"], char_range: [offset + 2, offset + 5] },
    ],
    formation: { nodes: [] },
    morphology: {
      chains: [
        { chain_id: "core", role: "lexical", parent_chain_id: null, anchor_range: [offset, offset + 2], char_range: [offset, offset + 2], surface_form: "読ん", lookup_form: "読む", lemma_form: "読む", dictionary_form: "読む", core_morpheme_indices: [0], morpheme_indices: [0], operators: [], connection_forms: [], final_state: state },
        { chain_id: "aux", role: "functional", parent_chain_id: "core", char_range: [offset + 2, offset + 5], surface_form: "でいた", lookup_form: "いる", lemma_form: "いる", dictionary_form: "いる", core_morpheme_indices: [1], morpheme_indices: [1], operators: [{ operator_id: "progressive", kind: "progressive", label: "进行", description: "动作持续", normalized_form: "ている" }], connection_forms: [], final_state: state },
      ],
      occurrences: [{ id: "progressive-hit", chain_id: "aux", operator_ids: ["progressive"], kind: "progressive", char_range: [offset + 2, offset + 5], hit_ranges: [[offset + 2, offset + 5]], candidates: [] }],
    },
  };
}

test("合并活用对应核心原形，临时目标生成相同的活用说明", () => {
  const document = inflectedDocument();
  const target = { id: "selection:10:15", char_range: [0, 5], ...mergedLookupTarget(document) };
  assert.equal(target.lookup_forms[0].form, "読む");
  assert.deepEqual(target.lexical_core_ids, ["core"]);
  assert.deepEqual(target.morpheme_ids, ["read", "aux"]);
  assert.equal(readerChains(target, document)[1].formName, "进行");
  const [hit] = readerMorphologyHits([target], document);
  assert.deepEqual(hit.range, [2, 5]);
  assert.equal(hit.detail.description, "动作持续");
  assert.equal(hit.targetId, target.id);
});

test("复合范围保留前部文本，词尾活用对应原形", () => {
  assert.equal(mergedLookupTarget(inflectedDocument("立ち")).lookup_forms[0].form, "立ち読む");
  const document = inflectedDocument();
  document.text += "猫";
  assert.equal(mergedLookupTarget(document).lookup_forms[0].form, "読んでいた猫");
});

test("普通合并文本保留整体表记，原有分析对象保持独立", () => {
  const document = { text: "茶屋酒", morphemes: [{ id: "word", reading: "チャヤザケ" }], morphology: { chains: [] } };
  const snapshot = structuredClone(document);
  assert.deepEqual(mergedLookupTarget(document).lookup_forms, [{ kind: "observed", form: "茶屋酒", reading: "チャヤザケ" }]);
  assert.deepEqual(document, snapshot);
});
