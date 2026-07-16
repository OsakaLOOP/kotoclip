import type { AnnotatedToken } from "../types";
import { floatDebug, type FloatDebugValue } from "../explanation/floatDebug";
import { decidePointerEnter, decidePointerLeave } from "../explanation/interactionGate";
import { belongsToSameToken, explanationHitFromTarget, type ExplanationHit } from "../explanation/hitTest";

interface ExplanationInteractionSession {
  cancelClose: (reason?: string) => void;
  scheduleClose: (reason?: string) => void;
  closeAll: (reason?: string) => void;
  focusMorpheme: (
    source: { paragraphId: number; tokenIndex: number; morphemeIndex: number },
    token: AnnotatedToken,
    capsule: HTMLElement,
    morpheme: HTMLElement,
  ) => void;
  focusGrammar: (tag: AnnotatedToken["bunsetsu"]["grammar_tags"][number], badge: HTMLElement) => void;
}

interface ExplanationInteractionOptions {
  findToken: (paragraphId: number, tokenIndex: number) => AnnotatedToken | null | undefined;
  session: ExplanationInteractionSession;
}

/**
 * DOM 委托只在这里解析为领域交互。ReaderView 不再自行组合命中、
 * token 校验、关闭宽限和聚焦规则。
 */
export function useExplanationInteraction(options: ExplanationInteractionOptions) {
  function handleParagraphPointerOver(event: PointerEvent) {
    const hit = explanationHitFromTarget(event.target);
    const previous = explanationHitFromTarget(event.relatedTarget);
    const decision = decidePointerEnter(hit, previous);
    const detail = {
      hit: hitSnapshot(hit),
      previous: hitSnapshot(previous),
      x: event.clientX,
      y: event.clientY,
    };
    floatDebug.snapshot("interaction", {
      origin: "paragraph:pointerover",
      decision: decision.action,
      reason: decision.reason,
      ...detail,
    });
    floatDebug.record("hit", "paragraph", "pointerover", `${previous.key} -> ${hit.key}`, detail);
    floatDebug.record("decision", "interaction-gate", decision.action, decision.reason, detail);
    if (decision.action === "ignore") return;

    options.session.cancelClose(`pointerover:${decision.reason}`);
    if (decision.action === "cancel-close") return;

    const sourceHit = hit as Exclude<ExplanationHit, { kind: "outside" | "panel" }>;
    const token = options.findToken(sourceHit.paragraphId, sourceHit.tokenIndex);
    if (!token || token.display_class !== "content") {
      floatDebug.record("decision", "interaction-controller", "close", "invalid-content-token", {
        hit: hitSnapshot(sourceHit),
      });
      options.session.closeAll("pointerover:invalid-content-token");
      return;
    }

    const eventTarget = event.target;
    if (!(eventTarget instanceof Element)) return;
    const capsule = eventTarget.closest<HTMLElement>("[data-token-index]");
    if (!capsule) {
      floatDebug.record("decision", "interaction-controller", "ignore", "capsule-element-missing");
      return;
    }

    floatDebug.snapshot("interactionScene", {
      phase: "pointerover",
      pointer: { x: event.clientX, y: event.clientY },
      previous: hitSnapshot(previous),
      hit: hitSnapshot(hit),
      token: {
        paragraphId: sourceHit.paragraphId,
        tokenIndex: sourceHit.tokenIndex,
        surface: token.bunsetsu.surface,
        displayClass: token.display_class,
      },
      capsule: elementSnapshot(capsule),
    });

    if (sourceHit.kind === "grammar") {
      const tag = token.bunsetsu.grammar_tags[sourceHit.grammarIndex];
      const badge = eventTarget.closest<HTMLElement>("[data-grammar-index]");
      if (tag && badge) {
        floatDebug.record("decision", "interaction-controller", "focus-grammar", tag.pattern_id, {
          hit: hitSnapshot(sourceHit),
        });
        options.session.focusGrammar(tag, badge);
        floatDebug.snapshot("interactionScene", {
          phase: "focus-grammar",
          pointer: { x: event.clientX, y: event.clientY },
          hit: hitSnapshot(sourceHit),
          token: {
            paragraphId: sourceHit.paragraphId,
            tokenIndex: sourceHit.tokenIndex,
            surface: token.bunsetsu.surface,
          },
          grammar: { patternId: tag.pattern_id, name: tag.name_ja },
          capsule: elementSnapshot(capsule),
          badge: elementSnapshot(badge),
        });
      } else {
        floatDebug.record("decision", "interaction-controller", "ignore", "grammar-target-missing");
      }
      return;
    }

    if (sourceHit.kind === "token" && belongsToSameToken(sourceHit, previous)) {
      floatDebug.record("decision", "interaction-controller", "ignore", "token-shell-within-same-token");
      return;
    }
    const morphemeIndex = sourceHit.kind === "morpheme"
      ? sourceHit.morphemeIndex
      : Math.min(token.bunsetsu.lexical_units[0]?.head_morpheme ?? 0, token.bunsetsu.morphemes.length - 1);
    const morpheme = capsule.querySelector<HTMLElement>(`[data-morpheme-index="${morphemeIndex}"]`);
    if (!morpheme) {
      floatDebug.record("decision", "interaction-controller", "ignore", "morpheme-element-missing", {
        morphemeIndex,
      });
      return;
    }
    const focusedMorpheme = token.bunsetsu.morphemes[morphemeIndex];
    floatDebug.snapshot("interactionScene", {
      phase: "focus-morpheme",
      pointer: { x: event.clientX, y: event.clientY },
      previous: hitSnapshot(previous),
      hit: hitSnapshot(sourceHit),
      token: {
        paragraphId: sourceHit.paragraphId,
        tokenIndex: sourceHit.tokenIndex,
        surface: token.bunsetsu.surface,
        displayClass: token.display_class,
      },
      morpheme: {
        morphemeIndex,
        surface: focusedMorpheme?.surface ?? null,
        baseForm: focusedMorpheme?.base_form ?? null,
        reading: focusedMorpheme?.reading ?? null,
      },
      capsule: elementSnapshot(capsule),
      morphemeElement: elementSnapshot(morpheme),
    });
    floatDebug.record("decision", "interaction-controller", "focus-morpheme", "accepted", {
      paragraphId: sourceHit.paragraphId,
      tokenIndex: sourceHit.tokenIndex,
      morphemeIndex,
      surface: token.bunsetsu.morphemes[morphemeIndex]?.surface ?? null,
    });
    options.session.focusMorpheme(
      { paragraphId: sourceHit.paragraphId, tokenIndex: sourceHit.tokenIndex, morphemeIndex },
      token,
      capsule,
      morpheme,
    );
  }

  function handlePointerLeave(event: PointerEvent, origin: "paragraph" | "popover") {
    const current = explanationHitFromTarget(event.target);
    const next = explanationHitFromTarget(event.relatedTarget);
    const decision = decidePointerLeave(next);
    const detail = {
      current: hitSnapshot(current),
      next: hitSnapshot(next),
      x: event.clientX,
      y: event.clientY,
    };
    floatDebug.snapshot("interaction", {
      origin: `${origin}:pointerleave`,
      decision: decision.action,
      reason: decision.reason,
      ...detail,
    });
    floatDebug.snapshot("interactionScene", {
      phase: `${origin}:pointerleave`,
      pointer: { x: event.clientX, y: event.clientY },
      current: hitSnapshot(current),
      next: hitSnapshot(next),
      currentElement: event.target instanceof Element ? elementSnapshot(event.target) : null,
    });
    floatDebug.record("hit", origin, "pointerleave", next.key, detail);
    floatDebug.record("decision", "interaction-gate", decision.action, decision.reason, detail);
    if (decision.action === "cancel-close") {
      options.session.cancelClose(`${origin}:${decision.reason}`);
      return;
    }
    options.session.scheduleClose(`${origin}:${decision.reason}`);
  }

  function handleParagraphPointerOut(event: PointerEvent) {
    handlePointerLeave(event, "paragraph");
  }

  function handlePopoverEnter(event: PointerEvent) {
    const hit = explanationHitFromTarget(event.target);
    floatDebug.snapshot("interaction", {
      origin: "popover:pointerenter",
      decision: "cancel-close",
      reason: "panel-entered",
      hit: hitSnapshot(hit),
    });
    floatDebug.snapshot("interactionScene", {
      phase: "popover:pointerenter",
      pointer: { x: event.clientX, y: event.clientY },
      hit: hitSnapshot(hit),
      panel: event.target instanceof Element ? elementSnapshot(event.target.closest<HTMLElement>("[data-explanation-panel]") ?? event.target) : null,
    });
    floatDebug.record("hit", "popover", "pointerenter", hit.key, {
      hit: hitSnapshot(hit),
      x: event.clientX,
      y: event.clientY,
    });
    options.session.cancelClose("popover:pointerenter");
  }

  function handlePopoverLeave(event: PointerEvent) {
    handlePointerLeave(event, "popover");
  }

  return {
    handleParagraphPointerOver,
    handleParagraphPointerOut,
    handlePopoverEnter,
    handlePopoverLeave,
  };
}

function hitSnapshot(hit: ExplanationHit): Record<string, FloatDebugValue> {
  const base: Record<string, FloatDebugValue> = { kind: hit.kind, key: hit.key };
  if ("paragraphId" in hit) {
    base.paragraphId = hit.paragraphId;
    base.tokenIndex = hit.tokenIndex;
  }
  if (hit.kind === "morpheme") base.morphemeIndex = hit.morphemeIndex;
  if (hit.kind === "grammar") base.grammarIndex = hit.grammarIndex;
  if (hit.kind === "panel") base.panel = hit.panel;
  return base;
}

function elementSnapshot(element: Element | null): Record<string, FloatDebugValue> | null {
  if (!element) return null;
  const rect = element.getBoundingClientRect();
  const html = element as HTMLElement;
  return {
    tag: element.tagName.toLowerCase(),
    id: html.id || null,
    className: typeof html.className === "string" ? html.className : null,
    panel: html.dataset.explanationPanel ?? null,
    paragraphId: html.dataset.paragraphId ?? null,
    tokenIndex: html.dataset.tokenIndex ?? null,
    morphemeIndex: html.dataset.morphemeIndex ?? null,
    grammarIndex: html.dataset.grammarIndex ?? null,
    rect: rectSnapshot(rect),
  };
}

function rectSnapshot(rect: DOMRect) {
  return {
    left: Math.round(rect.left),
    top: Math.round(rect.top),
    right: Math.round(rect.right),
    bottom: Math.round(rect.bottom),
    width: Math.round(rect.width),
    height: Math.round(rect.height),
  };
}
