import { computed, ref } from "vue";
import type { AnnotatedToken, DictionaryLookup, GrammarTag, Morpheme } from "../types";
import { snapshotRect, type RectSnapshot } from "../explanation/geometry";
import { scheduleCloseGrace } from "../explanation/closeGrace";

type LookupWord = (word: string, reading?: string) => Promise<DictionaryLookup | null>;
type ChooseTarget = (query: string, reading: string | null, target: string) => Promise<DictionaryLookup | null>;

interface SourceIdentity {
  paragraphId: number;
  tokenIndex: number;
  morphemeIndex: number;
}

export function useExplanationSession(lookupWord: LookupWord, chooseDictionaryTarget: ChooseTarget) {
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
  let componentGeneration = 0;
  let wholeGeneration = 0;
  const resultCache = new Map<string, DictionaryLookup | null>();
  const inflightCache = new Map<string, Promise<DictionaryLookup | null>>();

  const hasWholePanel = computed(() => wholeLoading.value || Boolean(wholeLookup.value));

  function cancelClose() {
    if (closeTimer !== null) window.clearTimeout(closeTimer);
    closeTimer = null;
  }

  /** 只用于跨越正文与浮层之间的物理间隙。 */
  function scheduleClose() {
    closeTimer = scheduleCloseGrace(
      closeTimer,
      (callback, delay) => window.setTimeout(callback, delay),
      closeAll,
    );
  }

  function closeAll() {
    cancelClose();
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
  }

  function focusMorpheme(
    source: SourceIdentity,
    token: AnnotatedToken,
    capsule: HTMLElement,
    morpheme: HTMLElement,
  ) {
    cancelClose();
    grammarVisible.value = false;
    grammarTag.value = null;
    const previous = activeSource.value;
    const sameToken = previous?.paragraphId === source.paragraphId && previous.tokenIndex === source.tokenIndex;
    if (sameToken && previous?.morphemeIndex === source.morphemeIndex && visible.value) {
      refreshAnchor();
      return;
    }

    activeSource.value = source;
    capsuleElement = capsule;
    morphemeElement = morpheme;
    refreshAnchor();
    visible.value = true;

    const focused = token.bunsetsu.morphemes[source.morphemeIndex];
    if (!focused) {
      closeAll();
      return;
    }
    componentToken.value = tokenForMorphemeLookup(token, focused);
    componentLabel.value = "内部";
    componentHistory.value = [];
    resolveComponent(focused);

    if (!sameToken) {
      wholeToken.value = token;
      wholeHistory.value = [];
      resolveWhole(token, focused);
    }
  }

  function focusGrammar(tag: GrammarTag, badge: HTMLElement) {
    cancelClose();
    visible.value = false;
    grammarTag.value = tag;
    grammarVisible.value = true;
    grammarElement = badge;
    grammarAnchorRect.value = snapshotRect(badge.getBoundingClientRect());
  }

  function refreshAnchor() {
    if (grammarVisible.value) {
      if (!grammarElement?.isConnected) {
        closeAll();
        return;
      }
      grammarAnchorRect.value = snapshotRect(grammarElement.getBoundingClientRect());
      return;
    }
    if (!capsuleElement?.isConnected || !morphemeElement?.isConnected) {
      closeAll();
      return;
    }
    anchorRect.value = snapshotRect(capsuleElement.getBoundingClientRect());
    componentAnchorRect.value = snapshotRect(morphemeElement.getBoundingClientRect());
  }

  async function resolveComponent(morpheme: Morpheme) {
    const word = lemma(morpheme);
    const generation = ++componentGeneration;
    const cached = cachedLookup(word, morpheme.reading);
    if (cached.immediate) {
      componentLookup.value = cached.value;
      componentLoading.value = false;
      return;
    }
    componentLookup.value = null;
    componentLoading.value = true;
    const lookup = await cached.promise;
    if (generation !== componentGeneration) return;
    componentLookup.value = lookup;
    componentLoading.value = false;
  }

  async function resolveWhole(token: AnnotatedToken, focused: Morpheme) {
    const lexical = token.bunsetsu.lexical_units[0];
    const sameAsComponent = lexical
      && lexical.base_form === lemma(focused)
      && lexical.reading === focused.reading;
    if (!lexical || sameAsComponent) {
      ++wholeGeneration;
      wholeLookup.value = null;
      wholeLoading.value = false;
      return;
    }
    const generation = ++wholeGeneration;
    const cached = cachedLookup(lexical.base_form, lexical.reading);
    if (cached.immediate) {
      wholeLookup.value = cached.value?.entries.length ? cached.value : null;
      wholeLoading.value = false;
      return;
    }
    wholeLookup.value = null;
    wholeLoading.value = true;
    const lookup = await cached.promise;
    if (generation !== wholeGeneration) return;
    wholeLookup.value = lookup?.entries.length ? lookup : null;
    wholeLoading.value = false;
  }

  function cachedLookup(word: string, reading: string) {
    const key = `${word}\u001f${reading}`;
    if (resultCache.has(key)) {
      return { immediate: true as const, value: resultCache.get(key) ?? null };
    }
    let promise = inflightCache.get(key);
    if (!promise) {
      promise = lookupWord(word, reading).then((lookup) => {
        resultCache.set(key, lookup);
        inflightCache.delete(key);
        return lookup;
      });
      inflightCache.set(key, promise);
    }
    return { immediate: false as const, promise };
  }

  async function navigateComponent(target: string) {
    if (componentLookup.value) componentHistory.value.push(componentLookup.value);
    const generation = ++componentGeneration;
    componentLoading.value = true;
    const lookup = await lookupWord(target);
    if (generation === componentGeneration && visible.value) {
      componentLookup.value = lookup;
      componentLoading.value = false;
    }
  }

  async function navigateWhole(target: string) {
    if (wholeLookup.value) wholeHistory.value.push(wholeLookup.value);
    const generation = ++wholeGeneration;
    wholeLoading.value = true;
    const lookup = await lookupWord(target);
    if (generation === wholeGeneration && visible.value) {
      wholeLookup.value = lookup;
      wholeLoading.value = false;
    }
  }

  async function selectComponent(target: string) {
    if (!componentLookup.value) return;
    const generation = ++componentGeneration;
    componentLoading.value = true;
    const lookup = await chooseDictionaryTarget(componentLookup.value.query, componentLookup.value.reading, target);
    if (generation === componentGeneration && visible.value) {
      componentLookup.value = lookup;
      componentLoading.value = false;
    }
  }

  async function selectWhole(target: string) {
    if (!wholeLookup.value) return;
    const generation = ++wholeGeneration;
    wholeLoading.value = true;
    const lookup = await chooseDictionaryTarget(wholeLookup.value.query, wholeLookup.value.reading, target);
    if (generation === wholeGeneration && visible.value) {
      wholeLookup.value = lookup;
      wholeLoading.value = false;
    }
  }

  function backComponent() {
    const previous = componentHistory.value.pop();
    if (!previous) return;
    ++componentGeneration;
    componentLookup.value = previous;
    componentLoading.value = false;
  }

  function backWhole() {
    const previous = wholeHistory.value.pop();
    if (!previous) return;
    ++wholeGeneration;
    wholeLookup.value = previous;
    wholeLoading.value = false;
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
    selectComponent,
    selectWhole,
    backComponent,
    backWhole,
  };
}

function lemma(morpheme: Morpheme) {
  return morpheme.base_form && morpheme.base_form !== "*" ? morpheme.base_form : morpheme.surface;
}

function tokenForMorphemeLookup(token: AnnotatedToken, morpheme: Morpheme): AnnotatedToken {
  return {
    ...token,
    bunsetsu: {
      ...token.bunsetsu,
      head_word: {
        surface: morpheme.surface,
        base_form: lemma(morpheme),
        reading: morpheme.reading,
        pos: morpheme.pos,
      },
      grammar_tags: [],
      word_formations: [],
      lexical_units: [],
    },
  };
}
