import { computed, nextTick, ref, watch } from "vue";
import type { AnnotatedToken, DictionaryLookup, DictionaryLookupRequest, GrammarTag } from "../types";
import { snapshotRect, type RectSnapshot } from "../explanation/geometry";
import { EXPLANATION_CLOSE_GRACE_MS, scheduleCloseGrace } from "../explanation/closeGrace";
import { floatDebug } from "../explanation/floatDebug";
import { deriveExplanationRenderGate } from "../explanation/interactionGate";
import { morphemeLookupTarget, type MorphemeLookupTarget } from "../utils/dictionaryTarget";

type LookupWord = (request: DictionaryLookupRequest) => Promise<DictionaryLookup | null>;

const HOVER_LOOKUP_DELAY_MS = 48;
const WHOLE_LOOKUP_DELAY_MS = 220;

interface SourceIdentity {
  paragraphId: number;
  tokenIndex: number;
  morphemeIndex: number;
}

export function useExplanationSession(lookupWord: LookupWord) {
  const visible = ref(false);
  const activeSource = ref<SourceIdentity | null>(null);
  const componentToken = ref<AnnotatedToken | null>(null);
  const componentLookup = ref<DictionaryLookup | null>(null);
  const componentLoading = ref(false);
  const componentHistory = ref<DictionaryLookup[]>([]);
  const componentLabel = ref("内部");
  const wholeToken = ref<AnnotatedToken | null>(null);
  const wholeLookup = ref<DictionaryLookup | null>(null);
  const wholeLoading = ref(false);
  const wholeHistory = ref<DictionaryLookup[]>([]);
  const anchorRect = ref<RectSnapshot | null>(null);
  const componentAnchorRect = ref<RectSnapshot | null>(null);
  const grammarTag = ref<GrammarTag | null>(null);
  const grammarAnchorRect = ref<RectSnapshot | null>(null);
  const grammarVisible = ref(false);

  let capsuleElement: HTMLElement | null = null;
  let morphemeElement: HTMLElement | null = null;
  let grammarElement: HTMLElement | null = null;
  let closeTimer: number | null = null;
  let closeStartedAt: number | null = null;
  let closeDeadline: number | null = null;
  let closeReason: string | null = null;
  let componentGeneration = 0;
  let wholeGeneration = 0;
  const resultCache = new Map<string, DictionaryLookup | null>();
  const inflightCache = new Map<string, Promise<DictionaryLookup | null>>();

  const hasWholePanel = computed(() => wholeLoading.value || Boolean(wholeLookup.value));
  const renderGate = computed(() => deriveExplanationRenderGate({
    dictionaryRequested: visible.value,
    grammarRequested: grammarVisible.value,
    hasComponentToken: Boolean(componentToken.value),
    hasComponentAnchor: Boolean(componentAnchorRect.value),
    hasWholeAnchor: Boolean(anchorRect.value),
    hasWholeLookup: Boolean(wholeLookup.value),
    wholeLoading: wholeLoading.value,
    hasGrammarTag: Boolean(grammarTag.value),
    hasGrammarAnchor: Boolean(grammarAnchorRect.value),
  }));

  function publishSession(action: string, outcome?: string) {
    floatDebug.snapshot("session", {
      action,
      outcome: outcome ?? null,
      visibleRequested: visible.value,
      grammarRequested: grammarVisible.value,
      activeSource: activeSource.value
        ? {
            paragraphId: activeSource.value.paragraphId,
            tokenIndex: activeSource.value.tokenIndex,
            morphemeIndex: activeSource.value.morphemeIndex,
          }
        : null,
      component: {
        loading: componentLoading.value,
        generation: componentGeneration,
        query: componentLookup.value?.query ?? null,
        history: componentHistory.value.length,
      },
      whole: {
        loading: wholeLoading.value,
        generation: wholeGeneration,
        query: wholeLookup.value?.query ?? null,
        history: wholeHistory.value.length,
      },
      grammar: grammarTag.value?.pattern_id ?? null,
    });
    floatDebug.record("session", "explanation-session", action, outcome, {
      visibleRequested: visible.value,
      grammarRequested: grammarVisible.value,
      componentGeneration,
      wholeGeneration,
    });
  }

  function publishTimer(
    action: string,
    outcome?: string,
    eventState?: { timerId: number | null; startedAt: number | null; deadline: number | null; reason: string | null },
  ) {
    floatDebug.snapshot("timer", {
      armed: closeTimer !== null,
      timerId: closeTimer,
      startedAt: closeStartedAt,
      deadline: closeDeadline,
      reason: closeReason,
      action,
      outcome: outcome ?? null,
    });
    floatDebug.record("timer", "close-grace", action, outcome, {
      timerId: eventState?.timerId ?? closeTimer,
      startedAt: eventState?.startedAt ?? closeStartedAt,
      deadline: eventState?.deadline ?? closeDeadline,
      reason: eventState?.reason ?? closeReason,
    });
  }

  watch(
    renderGate,
    (gate) => {
      floatDebug.snapshot("gate", {
        mode: gate.mode,
        dictionary: gate.dictionary,
        component: gate.component,
        whole: gate.whole,
        grammar: gate.grammar,
        blockers: [...gate.blockers],
      });
      floatDebug.record("gate", "render-gate", "evaluate", gate.mode, {
        dictionary: gate.dictionary,
        component: gate.component,
        whole: gate.whole,
        grammar: gate.grammar,
        blockers: [...gate.blockers],
      });
    },
    { immediate: true },
  );

  function cancelClose(reason = "unspecified") {
    const cancelledTimer = closeTimer;
    const cancelledState = {
      timerId: closeTimer,
      startedAt: closeStartedAt,
      deadline: closeDeadline,
      reason: closeReason,
    };
    if (cancelledTimer !== null) window.clearTimeout(cancelledTimer);
    closeTimer = null;
    closeStartedAt = null;
    closeDeadline = null;
    closeReason = null;
    publishTimer("cancel", cancelledTimer === null ? `${reason}:not-armed` : reason, cancelledState);
  }

  /** 只用于跨越正文与浮层之间的物理间隙。 */
  function scheduleClose(reason = "unspecified") {
    if (closeTimer !== null) {
      publishTimer("schedule-ignored", `${reason}:already-armed`);
      return;
    }
    closeStartedAt = performance.now();
    closeDeadline = closeStartedAt + EXPLANATION_CLOSE_GRACE_MS;
    closeReason = reason;
    closeTimer = scheduleCloseGrace(
      closeTimer,
      (callback, delay) => window.setTimeout(callback, delay),
      () => {
        publishTimer("expired", closeReason ?? "close-grace-expired");
        closeAll("close-grace-expired");
      },
    );
    publishTimer("scheduled", reason);
  }

  function closeAll(reason = "unspecified") {
    cancelClose(`close-all:${reason}`);
    visible.value = false;
    grammarVisible.value = false;
    activeSource.value = null;
    componentLookup.value = null;
    wholeLookup.value = null;
    componentLoading.value = false;
    wholeLoading.value = false;
    grammarTag.value = null;
    capsuleElement = null;
    morphemeElement = null;
    grammarElement = null;
    ++componentGeneration;
    ++wholeGeneration;
    publishSession("close-all", reason);
    publishScene("close-all");
  }

  function focusMorpheme(
    source: SourceIdentity,
    token: AnnotatedToken,
    capsule: HTMLElement,
    morpheme: HTMLElement,
  ) {
    cancelClose("focus-morpheme");
    grammarVisible.value = false;
    grammarTag.value = null;
    const previous = activeSource.value;
    const sameToken = previous?.paragraphId === source.paragraphId && previous.tokenIndex === source.tokenIndex;
    if (sameToken && previous?.morphemeIndex === source.morphemeIndex && visible.value) {
      publishSession("focus-morpheme", "same-source-refresh-anchor");
      refreshAnchor();
      publishScene("focus-morpheme:same-source");
      return;
    }

    activeSource.value = source;
    capsuleElement = capsule;
    morphemeElement = morpheme;
    refreshAnchor();
    visible.value = true;

    const focused = token.bunsetsu.morphemes[source.morphemeIndex];
    if (!focused) {
      closeAll("focused-morpheme-missing");
      return;
    }
    const lookupTarget = morphemeLookupTarget(token, focused);
    componentToken.value = tokenForMorphemeLookup(token, lookupTarget);
    componentLabel.value = lookupTarget.chain ? "词形" : "内部";
    componentHistory.value = [];
    resolveComponent(lookupTarget);

    if (!sameToken) {
      wholeToken.value = token;
      wholeHistory.value = [];
      resolveWhole(token, lookupTarget);
    }
    publishSession("focus-morpheme", sameToken ? "switch-component" : "new-token-session");
    publishScene("focus-morpheme");
  }

  function focusGrammar(tag: GrammarTag, badge: HTMLElement) {
    cancelClose("focus-grammar");
    visible.value = false;
    grammarTag.value = tag;
    grammarVisible.value = true;
    grammarElement = badge;
    grammarAnchorRect.value = snapshotRect(badge.getBoundingClientRect());
    publishSession("focus-grammar", tag.pattern_id);
    publishScene("focus-grammar");
  }

  function refreshAnchor() {
    if (grammarVisible.value) {
      if (!grammarElement?.isConnected) {
        floatDebug.record("gate", "anchor-gate", "close", "grammar-anchor-disconnected");
        closeAll("grammar-anchor-disconnected");
        return;
      }
      grammarAnchorRect.value = snapshotRect(grammarElement.getBoundingClientRect());
      floatDebug.snapshot("anchor", {
        mode: "grammar",
        grammar: rectDebugSnapshot(grammarAnchorRect.value),
      });
      floatDebug.record("layout", "anchor", "refresh", "grammar");
      publishScene("refresh-anchor:grammar");
      return;
    }
    if (!capsuleElement?.isConnected || !morphemeElement?.isConnected) {
      floatDebug.record("gate", "anchor-gate", "close", "dictionary-anchor-disconnected", {
        capsuleConnected: Boolean(capsuleElement?.isConnected),
        morphemeConnected: Boolean(morphemeElement?.isConnected),
      });
      closeAll("dictionary-anchor-disconnected");
      return;
    }
    anchorRect.value = snapshotRect(capsuleElement.getBoundingClientRect());
    componentAnchorRect.value = snapshotRect(morphemeElement.getBoundingClientRect());
    floatDebug.snapshot("anchor", {
      mode: "dictionary",
      whole: rectDebugSnapshot(anchorRect.value),
      component: rectDebugSnapshot(componentAnchorRect.value),
    });
    floatDebug.record("layout", "anchor", "refresh", "dictionary");
    publishScene("refresh-anchor:dictionary");
  }

  function publishScene(phase: string) {
    const source = activeSource.value;
    const token = wholeToken.value;
    const component = componentToken.value;
    floatDebug.snapshot("sessionScene", {
      phase,
      source: source
        ? {
            paragraphId: source.paragraphId,
            tokenIndex: source.tokenIndex,
            morphemeIndex: source.morphemeIndex,
          }
        : null,
      token: token
        ? {
            surface: token.bunsetsu.surface,
            headword: token.bunsetsu.head_word.base_form,
            reading: token.bunsetsu.head_word.reading,
          }
        : null,
      morpheme: component
        ? {
            surface: component.bunsetsu.head_word.surface,
            baseForm: component.bunsetsu.head_word.base_form,
            reading: component.bunsetsu.head_word.reading,
          }
        : null,
      visibleRequested: visible.value,
      grammarRequested: grammarVisible.value,
    });
    floatDebug.snapshot("textBoxes", {
      textCapsule: rectDebugSnapshot(anchorRect.value),
      textMorpheme: rectDebugSnapshot(componentAnchorRect.value),
      grammarBadge: rectDebugSnapshot(grammarAnchorRect.value),
    });
  }

  async function resolveComponent(target: MorphemeLookupTarget) {
    const resolveStartedAt = performance.now();
    const word = target.query;
    const generation = ++componentGeneration;
    const request = {
      word,
      observedForm: target.lemma,
      reading: target.lookupReading || undefined,
      pos: target.pos,
    } satisfies DictionaryLookupRequest;
    const requestKey = lookupKey(request);
    const cachedResult = resultCache.get(requestKey);
    const hasCachedResult = resultCache.has(requestKey);
    floatDebug.snapshot("request.component", {
      status: hasCachedResult ? "cache-hit" : "pending",
      generation,
      key: requestKey,
      word,
      reading: target.lookupReading,
    });
    floatDebug.record("request", "component", "resolve", hasCachedResult ? "cache-hit" : "pending", {
      generation,
      key: requestKey,
      word,
      reading: target.lookupReading,
    });
    if (hasCachedResult) {
      componentLookup.value = cachedResult ?? null;
      componentLoading.value = false;
      publishSession("component-resolved", "cache-hit");
      return;
    }
    componentLookup.value = null;
    componentLoading.value = true;
    await waitForHoverIntent(HOVER_LOOKUP_DELAY_MS);
    if (generation !== componentGeneration || !visible.value) {
      floatDebug.record("request", "component", "intent-cancelled", requestKey, {
        generation,
        currentGeneration: componentGeneration,
        visibleRequested: visible.value,
      });
      return;
    }
    const cached = cachedLookup(request);
    const invokeStartedAt = performance.now();
    const lookup = await cached.promise!;
    if (generation !== componentGeneration) {
      floatDebug.record("request", "component", "settle-discarded", "generation-mismatch", {
        generation,
        currentGeneration: componentGeneration,
        key: requestKey,
      });
      return;
    }
    componentLookup.value = lookup;
    componentLoading.value = false;
    await nextTick();
    const renderSettledAt = performance.now();
    floatDebug.snapshot("request.component", {
      status: "accepted",
      generation,
      key: requestKey,
      entries: lookup?.entries.length ?? 0,
    });
    floatDebug.record("request", "component", "settle-accepted", "generation-current", {
      generation,
      key: requestKey,
      entries: lookup?.entries.length ?? 0,
      intentDelayMs: Math.round(invokeStartedAt - resolveStartedAt),
      invokeAndTransferMs: Math.round(renderSettledAt - invokeStartedAt),
      totalUntilRenderMs: Math.round(renderSettledAt - resolveStartedAt),
      backend: lookup?.timing ? { ...lookup.timing } : null,
    });
    publishSession("component-resolved", "network-or-ipc");
  }

  async function resolveWhole(token: AnnotatedToken, focused: MorphemeLookupTarget) {
    const resolveStartedAt = performance.now();
    const lexical = token.bunsetsu.lexical_units[0];
    const sameAsComponent = lexical
      && lexical.base_form === focused.query
      && (!focused.lookupReading || lexical.reading === focused.lookupReading);
    if (!lexical || sameAsComponent) {
      ++wholeGeneration;
      wholeLookup.value = null;
      wholeLoading.value = false;
      floatDebug.snapshot("request.whole", {
        status: "skipped",
        generation: wholeGeneration,
        reason: !lexical ? "lexical-unit-missing" : "same-as-component",
      });
      floatDebug.record("request", "whole", "skip", !lexical ? "lexical-unit-missing" : "same-as-component");
      return;
    }
    const generation = ++wholeGeneration;
    const request = {
      word: lexical.base_form,
      observedForm: lexical.base_form,
      reading: lexical.reading || undefined,
      background: true,
      pos: lexical.output_pos,
    } satisfies DictionaryLookupRequest;
    const requestKey = lookupKey(request);
    const cachedResult = resultCache.get(requestKey);
    const hasCachedResult = resultCache.has(requestKey);
    floatDebug.snapshot("request.whole", {
      status: hasCachedResult ? "cache-hit" : "pending",
      generation,
      key: requestKey,
      word: lexical.base_form,
      reading: lexical.reading,
    });
    floatDebug.record("request", "whole", "resolve", hasCachedResult ? "cache-hit" : "pending", {
      generation,
      key: requestKey,
      word: lexical.base_form,
      reading: lexical.reading,
    });
    if (hasCachedResult) {
      wholeLookup.value = cachedResult?.forms.length ? cachedResult : null;
      wholeLoading.value = false;
      publishSession("whole-resolved", "cache-hit");
      return;
    }
    wholeLookup.value = null;
    wholeLoading.value = false;
    await waitForHoverIntent(WHOLE_LOOKUP_DELAY_MS);
    if (generation !== wholeGeneration || !visible.value) {
      floatDebug.record("request", "whole", "intent-cancelled", requestKey, {
        generation,
        currentGeneration: wholeGeneration,
        visibleRequested: visible.value,
      });
      return;
    }
    wholeLoading.value = true;
    const cached = cachedLookup(request);
    const invokeStartedAt = performance.now();
    const lookup = await cached.promise!;
    if (generation !== wholeGeneration) {
      floatDebug.record("request", "whole", "settle-discarded", "generation-mismatch", {
        generation,
        currentGeneration: wholeGeneration,
        key: requestKey,
      });
      return;
    }
    wholeLookup.value = lookup?.forms.length ? lookup : null;
    wholeLoading.value = false;
    await nextTick();
    const renderSettledAt = performance.now();
    floatDebug.snapshot("request.whole", {
      status: "accepted",
      generation,
      key: requestKey,
      entries: lookup?.entries.length ?? 0,
    });
    floatDebug.record("request", "whole", "settle-accepted", "generation-current", {
      generation,
      key: requestKey,
      entries: lookup?.entries.length ?? 0,
      intentDelayMs: Math.round(invokeStartedAt - resolveStartedAt),
      invokeAndTransferMs: Math.round(renderSettledAt - invokeStartedAt),
      totalUntilRenderMs: Math.round(renderSettledAt - resolveStartedAt),
      backend: lookup?.timing ? { ...lookup.timing } : null,
    });
    publishSession("whole-resolved", "network-or-ipc");
  }

  function cachedLookup(request: DictionaryLookupRequest) {
    const key = lookupKey(request);
    if (resultCache.has(key)) {
      floatDebug.record("request", "cache", "result-hit", key);
      return { immediate: true as const, value: resultCache.get(key) ?? null };
    }
    const inflightKey = `${key}\u001f${request.background ? "background" : "interactive"}`;
    let promise = inflightCache.get(inflightKey);
    if (!promise) {
      floatDebug.record("request", "cache", "start-inflight", key);
      promise = lookupWord(request).then((lookup) => {
        resultCache.set(key, lookup);
        inflightCache.delete(inflightKey);
        floatDebug.record("request", "cache", "store-result", key, {
          entries: lookup?.entries.length ?? 0,
        });
        return lookup;
      });
      inflightCache.set(inflightKey, promise);
    } else {
      floatDebug.record("request", "cache", "join-inflight", key);
    }
    return { immediate: false as const, promise };
  }

  async function navigateComponent(target: string) {
    if (componentLookup.value) componentHistory.value.push(componentLookup.value);
    const generation = ++componentGeneration;
    componentLoading.value = true;
    floatDebug.record("request", "component", "navigate", target, { generation });
    publishSession("component-navigate", target);
    const lookup = await lookupWord({ word: target, observedForm: target });
    if (generation === componentGeneration && visible.value) {
      componentLookup.value = lookup;
      componentLoading.value = false;
      floatDebug.record("request", "component", "navigate-accepted", target, {
        generation,
        entries: lookup?.entries.length ?? 0,
      });
      publishSession("component-navigate-resolved", target);
    } else {
      floatDebug.record("request", "component", "navigate-discarded", target, {
        generation,
        currentGeneration: componentGeneration,
        visibleRequested: visible.value,
      });
    }
  }

  async function navigateWhole(target: string) {
    if (wholeLookup.value) wholeHistory.value.push(wholeLookup.value);
    const generation = ++wholeGeneration;
    wholeLoading.value = true;
    floatDebug.record("request", "whole", "navigate", target, { generation });
    publishSession("whole-navigate", target);
    const lookup = await lookupWord({ word: target, observedForm: target });
    if (generation === wholeGeneration && visible.value) {
      wholeLookup.value = lookup;
      wholeLoading.value = false;
      floatDebug.record("request", "whole", "navigate-accepted", target, {
        generation,
        entries: lookup?.entries.length ?? 0,
      });
      publishSession("whole-navigate-resolved", target);
    } else {
      floatDebug.record("request", "whole", "navigate-discarded", target, {
        generation,
        currentGeneration: wholeGeneration,
        visibleRequested: visible.value,
      });
    }
  }

  async function selectComponentForm(formId: string) {
    if (!componentLookup.value) return;
    const generation = ++componentGeneration;
    componentLoading.value = true;
    floatDebug.record("request", "component", "select-form", formId, { generation });
    publishSession("component-select-form", formId);
    const current = componentLookup.value;
    const lookup = await lookupWord({
      word: current.query,
      observedForm: current.observed_form ?? current.query,
      reading: current.reading ?? undefined,
      pos: current.pos ?? undefined,
      selectedForm: formId,
    });
    if (generation === componentGeneration && visible.value) {
      componentLookup.value = lookup;
      componentLoading.value = false;
      floatDebug.record("request", "component", "select-form-accepted", formId, { generation });
      publishSession("component-select-form-resolved", formId);
    } else {
      floatDebug.record("request", "component", "select-form-discarded", formId, {
        generation,
        currentGeneration: componentGeneration,
        visibleRequested: visible.value,
      });
    }
  }

  async function selectWholeForm(formId: string) {
    if (!wholeLookup.value) return;
    const generation = ++wholeGeneration;
    wholeLoading.value = true;
    floatDebug.record("request", "whole", "select-form", formId, { generation });
    publishSession("whole-select-form", formId);
    const current = wholeLookup.value;
    const lookup = await lookupWord({
      word: current.query,
      observedForm: current.observed_form ?? current.query,
      reading: current.reading ?? undefined,
      pos: current.pos ?? undefined,
      selectedForm: formId,
    });
    if (generation === wholeGeneration && visible.value) {
      wholeLookup.value = lookup;
      wholeLoading.value = false;
      floatDebug.record("request", "whole", "select-form-accepted", formId, { generation });
      publishSession("whole-select-form-resolved", formId);
    } else {
      floatDebug.record("request", "whole", "select-form-discarded", formId, {
        generation,
        currentGeneration: wholeGeneration,
        visibleRequested: visible.value,
      });
    }
  }

  function backComponent() {
    const previous = componentHistory.value.pop();
    if (!previous) return;
    ++componentGeneration;
    componentLookup.value = previous;
    componentLoading.value = false;
    publishSession("component-back", previous.query);
  }

  function backWhole() {
    const previous = wholeHistory.value.pop();
    if (!previous) return;
    ++wholeGeneration;
    wholeLookup.value = previous;
    wholeLoading.value = false;
    publishSession("whole-back", previous.query);
  }

  return {
    visible,
    componentToken,
    componentLookup,
    componentLoading,
    componentHistory,
    componentLabel,
    wholeToken,
    wholeLookup,
    wholeLoading,
    wholeHistory,
    hasWholePanel,
    renderGate,
    anchorRect,
    componentAnchorRect,
    grammarTag,
    grammarVisible,
    grammarAnchorRect,
    cancelClose,
    scheduleClose,
    closeAll,
    focusMorpheme,
    focusGrammar,
    refreshAnchor,
    navigateComponent,
    navigateWhole,
    selectComponentForm,
    selectWholeForm,
    backComponent,
    backWhole,
  };
}

function lookupKey(request: DictionaryLookupRequest) {
  const posKey = request.pos
    ? `${request.pos.major}/${request.pos.sub1}/${request.pos.sub2}/${request.pos.sub3}`
    : "";
  return [
    request.word,
    request.observedForm ?? "",
    request.reading ?? "",
    posKey,
    request.selectedForm ?? "",
  ].join("\u001f");
}

function waitForHoverIntent(delay: number) {
  return new Promise<void>((resolve) => window.setTimeout(resolve, delay));
}

function rectDebugSnapshot(rect: RectSnapshot | null) {
  if (!rect) return null;
  return {
    left: Math.round(rect.left),
    top: Math.round(rect.top),
    right: Math.round(rect.right),
    bottom: Math.round(rect.bottom),
    width: Math.round(rect.width),
    height: Math.round(rect.height),
  };
}

function tokenForMorphemeLookup(
  token: AnnotatedToken,
  target: MorphemeLookupTarget,
): AnnotatedToken {
  return {
    ...token,
    bunsetsu: {
      ...token.bunsetsu,
      head_word: {
        surface: target.surface,
        base_form: target.query,
        reading: target.reading,
        pos: target.pos,
      },
      grammar_tags: [],
      morphology: { chains: target.chain ? [target.chain] : [] },
      word_formations: [],
      lexical_units: [],
    },
  };
}
