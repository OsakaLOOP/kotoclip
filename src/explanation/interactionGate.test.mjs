import assert from "node:assert/strict";
import test from "node:test";
import {
  decidePointerEnter,
  decidePointerLeave,
  deriveExplanationRenderGate,
} from "./interactionGate.ts";

const morpheme = (key = "morpheme:1:2:0") => ({
  kind: "morpheme",
  paragraphId: 1,
  tokenIndex: 2,
  morphemeIndex: 0,
  key,
});

test("相同语义目标不会重新进入会话", () => {
  assert.deepEqual(decidePointerEnter(morpheme(), morpheme()), {
    action: "ignore",
    reason: "same-semantic-target",
  });
});

test("面板与正文都取消关闭，只有 outside 安排关闭", () => {
  assert.equal(decidePointerEnter({ kind: "panel", panel: "whole", key: "panel:whole" }, morpheme()).action, "cancel-close");
  assert.equal(decidePointerLeave({ kind: "panel", panel: "whole", key: "panel:whole" }).action, "cancel-close");
  assert.equal(decidePointerLeave({ kind: "outside", key: "outside" }).action, "schedule-close");
});

test("最终渲染门集中报告可见面板与阻断原因", () => {
  assert.deepEqual(deriveExplanationRenderGate({
    dictionaryRequested: true,
    grammarRequested: false,
    hasComponentToken: true,
    hasComponentAnchor: true,
    hasWholeAnchor: true,
    hasWholeLookup: true,
    wholeLoading: false,
    hasGrammarTag: false,
    hasGrammarAnchor: false,
  }), {
    mode: "dictionary",
    dictionary: true,
    component: true,
    whole: true,
    grammar: false,
    blockers: [],
  });

  const blocked = deriveExplanationRenderGate({
    dictionaryRequested: true,
    grammarRequested: false,
    hasComponentToken: true,
    hasComponentAnchor: false,
    hasWholeAnchor: true,
    hasWholeLookup: false,
    wholeLoading: false,
    hasGrammarTag: false,
    hasGrammarAnchor: false,
  });
  assert.equal(blocked.mode, "closed");
  assert.deepEqual(blocked.blockers, ["component-anchor-missing"]);
});
