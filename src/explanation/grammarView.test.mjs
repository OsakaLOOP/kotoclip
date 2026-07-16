import assert from "node:assert/strict";
import test from "node:test";
import { grammarTagCoversRange, primaryGrammarIndex } from "./grammarView.ts";

function tag(kind, ranges) {
  return { occurrence_kind: kind, display_ranges: ranges };
}

test("非连续显示范围不会染色自由 gap", () => {
  const value = tag("grammar_construction", [[1, 2], [4, 5]]);
  assert.equal(grammarTagCoversRange(value, [1, 2]), true);
  assert.equal(grammarTagCoversRange(value, [2, 4]), false);
});

test("同一语素优先打开更具体的构式讲解", () => {
  const tags = [
    tag("functional_morpheme", [[2, 3]]),
    tag("grammar_construction", [[2, 5]]),
    tag("morphology_feature", [[2, 3]]),
  ];
  assert.equal(primaryGrammarIndex(tags, [2, 3]), 1);
});
