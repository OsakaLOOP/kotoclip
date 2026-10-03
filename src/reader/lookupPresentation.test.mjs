import assert from "node:assert/strict";
import test from "node:test";
import { dictionaryLookupFromSearch, readerCapsuleRanges, readerChains, readerMorphologyHits } from "./lookupPresentation.ts";

test("胶囊采用已确认整体词，文节和候选整体不改变边界", () => {
  const document = {
    text: "東京へ行く", morphemes: [],
    bunsetsu: { nodes: [{ id: "b1", status: "observed", char_range: [0, 5] }] },
    formation: { nodes: [
      { id: "whole", status: "observed", char_range: [0, 2], word: {} },
      { id: "candidate", status: "candidate", char_range: [2, 5], word: {} },
    ] },
    morphology: { chains: [
      { chain_id: "tokyo", role: "lexical", parent_chain_id: null, char_range: [0, 2] },
      { chain_id: "go", role: "lexical", parent_chain_id: null, char_range: [3, 5] },
    ] },
  };
  assert.deepEqual(readerCapsuleRanges(document), [{ id: "whole", range: [0, 2] }, { id: "go", range: [3, 5] }]);
});

test("词汇链与嵌套附属部分形成独立词边界", () => {
  const document = {
    text: "読んでいる", morphemes: [], formation: { nodes: [] },
    bunsetsu: { nodes: [] },
    morphology: { chains: [
      { chain_id: "core", role: "lexical", parent_chain_id: null, char_range: [0, 2] },
      { chain_id: "aux", role: "functional", parent_chain_id: "core", char_range: [2, 3] },
      { chain_id: "nested", role: "functional", parent_chain_id: "aux", char_range: [3, 5] },
    ] },
  };
  assert.deepEqual(readerCapsuleRanges(document), [{ id: "core", range: [0, 5] }]);
});

test("标点隔开词汇链，未确认整体不产生胶囊", () => {
  const document = {
    text: "読む。行く", morphemes: [{ surface: "。", pos: ["補助記号"], char_range: [2, 3] }],
    formation: { nodes: [{ id: "pending", status: "candidate", char_range: [0, 6], word: {} }] },
    morphology: { chains: [
      { chain_id: "read", role: "lexical", parent_chain_id: null, char_range: [0, 2] },
      { chain_id: "invalid", role: "functional", parent_chain_id: "read", char_range: [3, 5] },
      { chain_id: "go", role: "lexical", parent_chain_id: null, char_range: [3, 5] },
    ] },
  };
  assert.deepEqual(readerCapsuleRanges(document), [{ id: "read", range: [0, 2] }, { id: "go", range: [3, 5] }]);
});

test("已确认整体优先于重叠的词汇链，剩余词汇仍可显示", () => {
  const document = {
    text: "食べている猫", morphemes: [],
    formation: { nodes: [{ id: "whole", status: "observed", char_range: [0, 5], word: {} }] },
    morphology: { chains: [
      { chain_id: "eat", role: "lexical", parent_chain_id: null, char_range: [0, 2] },
      { chain_id: "aux", role: "functional", parent_chain_id: "eat", char_range: [2, 5] },
      { chain_id: "cat", role: "lexical", parent_chain_id: null, char_range: [5, 6] },
    ] },
  };
  assert.deepEqual(readerCapsuleRanges(document), [{ id: "whole", range: [0, 5] }, { id: "cat", range: [5, 6] }]);
});

test("按当前查询目标引用展示词形、连接和成员", () => {
  const document = {
    morphemes: [
      { id: "m0", surface: "読ん", pos: ["動詞"] },
      { id: "m1", surface: "で", pos: ["助詞"] },
    ],
    formation: { nodes: [] },
    morphology: { chains: [{
      chain_id: "chain-1", role: "lexical", surface_form: "読んで", lemma_form: "読む",
      dictionary_form: "読む", lookup_form: "読む", base_lexeme: "読む",
      final_state: { conjugation_type: "五段", conjugation_form: "連用形" },
      connection_forms: ["て接続"], morpheme_indices: [0, 1], core_morpheme_indices: [0],
      operators: [{ label: "音便", description: "撥音便", output_state: "連用形" }],
    }] },
  };
  const [chain] = readerChains({ lexical_core_ids: ["chain-1"], source_formation_ids: [], morpheme_ids: ["m0"] }, document);
  assert.equal(chain.surface, "読んで");
  assert.equal(chain.lemma, "読む");
  assert.equal(chain.conjugation, "五段活用 · 连用形");
  assert.deepEqual(chain.members, [{ surface: "読ん", role: "核心" }, { surface: "で", role: "助詞" }]);
  assert.deepEqual(chain.connections, ["て接続"]);
  assert.deepEqual(chain.steps, [{ label: "音便", description: "撥音便", normalizedForm: undefined }]);
});

test("构词来源关联活用链；搜索只展示所选表记的词条", () => {
  const document = {
    morphemes: [],
    formation: { nodes: [{ id: "word-1", word: { chain_ids: ["chain-1"] } }] },
    morphology: { chains: [{ chain_id: "chain-1", surface_form: "読んだ", lemma_form: "読む", lookup_form: "読む", final_state: {}, connection_forms: [], morpheme_indices: [], core_morpheme_indices: [], operators: [] }] },
  };
  assert.equal(readerChains({ lexical_core_ids: [], source_formation_ids: ["word-1"], morpheme_ids: [] }, document)[0].lemma, "読む");
  const output = {
    forms: [{ form_id: "a" }, { form_id: "b" }], selected_form_id: "b", dictionary_names: ["辞典"],
    groups: [{ form_id: "a", entries: [{ headword: "甲" }] }, { form_id: "b", entries: [{ headword: "乙" }] }],
  };
  assert.deepEqual(dictionaryLookupFromSearch(output, "読む").entries.map((entry) => entry.headword), ["乙"]);
});

test("活用部分的两个命中范围共用核心词查询目标", () => {
  const document = {
    text: "読んでいる", formation: { nodes: [] },
    morphology: {
      chains: [
        { chain_id: "core", parent_chain_id: null, anchor_range: [0, 2], operators: [] },
        { chain_id: "aux", parent_chain_id: "core", operators: [{ operator_id: "op", kind: "te_iru", label: "ている形式", description: "ている形式：いる", normalized_form: null }] },
      ],
      occurrences: [{ id: "occ", chain_id: "aux", kind: "te_iru", char_range: [2, 5], hit_ranges: [[2, 3], [3, 5]], operator_ids: ["op"], candidates: [] }],
    },
  };
  const hits = readerMorphologyHits([{ id: "word", char_range: [0, 2], lexical_core_ids: ["core"], source_formation_ids: [] }], document);
  assert.deepEqual(hits.map((hit) => [hit.range, hit.targetId]), [
    [[2, 3], "word"], [[3, 5], "word"],
  ]);
  assert.equal(hits[0].detail.title, "ている形式");
  assert.equal(hits[0].detail.surface, "でいる");
  const [preferred] = readerMorphologyHits([
    { id: "inner", char_range: [0, 2], lexical_core_ids: ["core"], source_formation_ids: [] },
    { id: "outer", char_range: [0, 5], lexical_core_ids: ["core"], source_formation_ids: [] },
  ], document);
  assert.equal(preferred.targetId, "inner");
});

test("所属补助链参与词典气泡的整体活用概要", () => {
  const document = {
    morphemes: [{ id: "core-m", surface: "読ん", pos: ["動詞"] }, { id: "aux-m", surface: "いる", pos: ["動詞"] }],
    formation: { nodes: [] },
    morphology: { chains: [
      { chain_id: "core", parent_chain_id: null, role: "lexical", surface_form: "読んで", lemma_form: "読む", lookup_form: "読む", final_state: { conjugation_type: "五段", conjugation_form: "連用形" }, connection_forms: [], morpheme_indices: [0], core_morpheme_indices: [0], operators: [] },
      { chain_id: "aux", parent_chain_id: "core", role: "functional", surface_form: "いる", lemma_form: "いる", lookup_form: "いる", final_state: {}, connection_forms: [], morpheme_indices: [1], core_morpheme_indices: [1], operators: [{ kind: "te_iru", label: "ている形式", description: "継続" }] },
    ] },
  };
  const chains = readerChains({ lexical_core_ids: ["core"], source_formation_ids: [], morpheme_ids: ["core-m"] }, document);
  assert.deepEqual(chains.map((chain) => [chain.surface, chain.formName]), [["読んで", ""], ["いる", "ている形式"]]);
});

test("普通词尾活用生成单体说明而保留词典目标", () => {
  const document = {
    text: "読ん", formation: { nodes: [] },
    morphology: {
      chains: [{ chain_id: "core", parent_chain_id: null, anchor_range: [0, 2], surface_form: "読ん", lemma_form: "読む", dictionary_form: "読む",
        final_state: { conjugation_type: "五段", conjugation_form: "連用形", category: "動詞", form: "continuative" },
        operators: [{ operator_id: "conj", kind: "conjugation", char_range: [0, 2] }] }],
      occurrences: [],
    },
  };
  const [hit] = readerMorphologyHits([{ id: "word", char_range: [0, 2], lexical_core_ids: ["core"], source_formation_ids: [] }], document);
  assert.equal(hit.targetId, "word");
  assert.equal(hit.detail.title, "五段活用 · 连用形");
  assert.equal(hit.detail.description, "読む → 読ん");
});
