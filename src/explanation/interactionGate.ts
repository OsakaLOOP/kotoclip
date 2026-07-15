import type { ExplanationHit } from "./hitTest";

export type PointerEnterDecision =
  | { action: "ignore"; reason: "same-semantic-target" }
  | { action: "cancel-close"; reason: "interactive-region" }
  | { action: "focus-source"; reason: "new-source-target" };

export type PointerLeaveDecision =
  | { action: "cancel-close"; reason: "next-target-keeps-open" }
  | { action: "schedule-close"; reason: "left-interactive-region" };

export interface ExplanationRenderInput {
  dictionaryRequested: boolean;
  grammarRequested: boolean;
  hasComponentToken: boolean;
  hasComponentAnchor: boolean;
  hasWholeAnchor: boolean;
  hasWholeLookup: boolean;
  wholeLoading: boolean;
  hasGrammarTag: boolean;
  hasGrammarAnchor: boolean;
}

export interface ExplanationRenderGate {
  mode: "closed" | "dictionary" | "grammar";
  dictionary: boolean;
  component: boolean;
  whole: boolean;
  grammar: boolean;
  blockers: string[];
}

/** 将 pointerover 的 DOM 变化压缩为有限的交互动作。 */
export function decidePointerEnter(hit: ExplanationHit, previous: ExplanationHit): PointerEnterDecision {
  if (hit.key === previous.key) {
    return { action: "ignore", reason: "same-semantic-target" };
  }
  if (hit.kind === "outside" || hit.kind === "panel") {
    return { action: "cancel-close", reason: "interactive-region" };
  }
  return { action: "focus-source", reason: "new-source-target" };
}

/** 正文、词典面板和语法面板共同组成一个连续交互区域。 */
export function decidePointerLeave(next: ExplanationHit): PointerLeaveDecision {
  if (next.kind !== "outside") {
    return { action: "cancel-close", reason: "next-target-keeps-open" };
  }
  return { action: "schedule-close", reason: "left-interactive-region" };
}

/** 统一计算组件实际可消费的最终显隐门。 */
export function deriveExplanationRenderGate(input: ExplanationRenderInput): ExplanationRenderGate {
  const blockers: string[] = [];
  if (input.grammarRequested) {
    if (!input.hasGrammarTag) blockers.push("grammar-tag-missing");
    if (!input.hasGrammarAnchor) blockers.push("grammar-anchor-missing");
    const grammar = blockers.length === 0;
    return {
      mode: grammar ? "grammar" : "closed",
      dictionary: false,
      component: false,
      whole: false,
      grammar,
      blockers,
    };
  }

  if (!input.dictionaryRequested) blockers.push("dictionary-not-requested");
  if (!input.hasComponentToken) blockers.push("component-token-missing");
  if (!input.hasComponentAnchor) blockers.push("component-anchor-missing");
  if (!input.hasWholeAnchor) blockers.push("whole-anchor-missing");
  const dictionary = blockers.length === 0;
  const whole = dictionary && (input.wholeLoading || input.hasWholeLookup);
  return {
    mode: dictionary ? "dictionary" : "closed",
    dictionary,
    component: dictionary,
    whole,
    grammar: false,
    blockers,
  };
}
