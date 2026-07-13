export type ExplanationHit =
  | { kind: "morpheme"; paragraphId: number; tokenIndex: number; morphemeIndex: number; key: string }
  | { kind: "token"; paragraphId: number; tokenIndex: number; key: string }
  | { kind: "grammar"; paragraphId: number; tokenIndex: number; grammarIndex: number; key: string }
  | { kind: "panel"; panel: string; key: string }
  | { kind: "outside"; key: "outside" };

/**
 * 将 DOM 节点归一化为阅读器交互区域。事件只负责通知节点变化，
 * 后续状态转换只比较这里生成的语义 key。
 */
export function explanationHitFromTarget(target: EventTarget | null): ExplanationHit {
  if (!(target instanceof Element)) return { kind: "outside", key: "outside" };

  const panel = target.closest<HTMLElement>("[data-explanation-panel]");
  if (panel) {
    const name = panel.dataset.explanationPanel || "panel";
    return { kind: "panel", panel: name, key: `panel:${name}` };
  }

  const capsule = target.closest<HTMLElement>("[data-token-index][data-paragraph-id]");
  if (!capsule) return { kind: "outside", key: "outside" };
  const paragraphId = Number.parseInt(capsule.dataset.paragraphId ?? "", 10);
  const tokenIndex = Number.parseInt(capsule.dataset.tokenIndex ?? "", 10);
  if (!Number.isFinite(paragraphId) || !Number.isFinite(tokenIndex)) {
    return { kind: "outside", key: "outside" };
  }

  const grammar = target.closest<HTMLElement>("[data-grammar-index]");
  if (grammar) {
    const grammarIndex = Number.parseInt(grammar.dataset.grammarIndex ?? "", 10);
    if (Number.isFinite(grammarIndex)) {
      return {
        kind: "grammar",
        paragraphId,
        tokenIndex,
        grammarIndex,
        key: `grammar:${paragraphId}:${tokenIndex}:${grammarIndex}`,
      };
    }
  }

  const morpheme = target.closest<HTMLElement>("[data-morpheme-index]");
  if (morpheme) {
    const morphemeIndex = Number.parseInt(morpheme.dataset.morphemeIndex ?? "", 10);
    if (Number.isFinite(morphemeIndex)) {
      return {
        kind: "morpheme",
        paragraphId,
        tokenIndex,
        morphemeIndex,
        key: `morpheme:${paragraphId}:${tokenIndex}:${morphemeIndex}`,
      };
    }
  }

  return {
    kind: "token",
    paragraphId,
    tokenIndex,
    key: `token:${paragraphId}:${tokenIndex}`,
  };
}

export function belongsToSameToken(left: ExplanationHit, right: ExplanationHit) {
  if (!("paragraphId" in left) || !("paragraphId" in right)) return false;
  return left.paragraphId === right.paragraphId && left.tokenIndex === right.tokenIndex;
}

export function keepsExplanationOpen(hit: ExplanationHit) {
  return hit.kind !== "outside";
}
