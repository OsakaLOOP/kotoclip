<script setup lang="ts">
import { computed, nextTick, ref } from "vue";
import type {
  DictionaryLookup,
  DictionaryLookupRequest,
  GrammarDictionaryTarget,
} from "../../types";
import AnalyzedTextItem from "../reader/AnalyzedTextItem.vue";
import ExplanationPopover from "../explanation/ExplanationPopover.vue";
import GrammarPopover from "../explanation/GrammarPopover.vue";
import { useExplanationInteraction } from "../../composables/useExplanationInteraction";
import { useExplanationSession } from "../../composables/useExplanationSession";
import {
  annotatedTokenFromQuality,
  type QualityReadingUnit,
} from "../../types/qualityAudit";

const props = defineProps<{
  unit: QualityReadingUnit;
  side: "before" | "after";
  title: string;
  activeCharacter: number | null;
  shortcutsEnabled: boolean;
  lookupWord: (
    request: DictionaryLookupRequest,
  ) => Promise<DictionaryLookup | null>;
}>();

const emit = defineEmits<{
  focus: [value: { side: "before" | "after"; coordinate: number; kind: "character" | "grammar" }];
  leave: [side: "before" | "after"];
}>();

const itemRef = ref<InstanceType<typeof AnalyzedTextItem> | null>(null);
const rootElement = computed(() => itemRef.value?.rootElement ?? null);
const paragraphId = computed(() => props.unit.sentence_index);
const tokens = computed(() => props.unit[props.side].tokens.map(annotatedTokenFromQuality));
const explanation = useExplanationSession(props.lookupWord);
const interaction = useExplanationInteraction({
  findToken(id, tokenIndex) {
    return id === paragraphId.value ? tokens.value[tokenIndex] : null;
  },
  session: explanation,
});

function sourceFromTarget(target: EventTarget | null) {
  if (!(target instanceof Element)) return null;
  const capsule = target.closest<HTMLElement>("[data-token-index][data-paragraph-id]");
  if (!capsule) return null;
  const tokenIndex = Number(capsule.dataset.tokenIndex);
  const token = tokens.value[tokenIndex];
  if (!token) return null;
  const grammar = target.closest<HTMLElement>("[data-grammar-index]");
  if (grammar) {
    const grammarIndex = Number(grammar.dataset.grammarIndex);
    const tag = token.bunsetsu.grammar_tags[grammarIndex];
    if (tag) return { coordinate: tag.char_range[0], kind: "grammar" as const };
  }
  const character = target.closest<HTMLElement>("[data-character-offset]");
  const coordinate = Number(character?.dataset.characterOffset);
  return Number.isFinite(coordinate)
    ? { coordinate, kind: "character" as const }
    : null;
}

function handlePointerOver(event: PointerEvent) {
  interaction.handleParagraphPointerOver(event);
  const source = sourceFromTarget(event.target);
  if (source) emit("focus", { side: props.side, ...source });
}

function handlePointerOut(event: PointerEvent) {
  interaction.handleParagraphPointerOut(event);
  const next = event.relatedTarget;
  if (!(next instanceof Node) || !rootElement.value?.contains(next)) emit("leave", props.side);
}

async function focusCoordinate(coordinate: number, kind: "character" | "grammar") {
  await nextTick();
  const root = rootElement.value;
  if (!root) return false;
  const character = root.querySelector<HTMLElement>(`[data-character-offset="${coordinate}"]`);
  const capsule = character?.closest<HTMLElement>("[data-token-index][data-paragraph-id]");
  if (!capsule) {
    explanation.closeAll("quality-counterpart-missing");
    return false;
  }
  const tokenIndex = Number(capsule.dataset.tokenIndex);
  const token = tokens.value[tokenIndex];
  if (!token || token.display_class !== "content") return false;
  if (kind === "grammar") {
    const grammarIndex = token.bunsetsu.grammar_tags.findIndex((tag) => {
      const ranges = tag.display_ranges.length ? tag.display_ranges : [tag.char_range];
      return ranges.some((range) => range[0] <= coordinate && coordinate < range[1]);
    });
    const badge = capsule.querySelector<HTMLElement>(`[data-grammar-index="${grammarIndex}"]`);
    const tag = token.bunsetsu.grammar_tags[grammarIndex];
    if (badge && tag) {
      explanation.focusGrammar(tag, badge);
      return true;
    }
  }
  const morpheme = character?.closest<HTMLElement>("[data-morpheme-index]");
  const morphemeIndex = Number(morpheme?.dataset.morphemeIndex);
  if (!morpheme || !Number.isFinite(morphemeIndex)) return false;
  explanation.focusMorpheme(
    { paragraphId: paragraphId.value, tokenIndex, morphemeIndex },
    token,
    capsule,
    morpheme,
  );
  return true;
}

function openGrammarDictionary(target: GrammarDictionaryTarget) {
  const root = rootElement.value;
  if (!root) return;
  for (let tokenIndex = 0; tokenIndex < tokens.value.length; tokenIndex++) {
    const token = tokens.value[tokenIndex];
    const morphemeIndex = token.bunsetsu.morphemes.findIndex(
      (morpheme) =>
        morpheme.char_range[0] === target.char_range[0] &&
        morpheme.char_range[1] === target.char_range[1],
    );
    if (morphemeIndex < 0) continue;
    const capsule = root.querySelector<HTMLElement>(
      `[data-token-index="${tokenIndex}"]`,
    );
    const morpheme = capsule?.querySelector<HTMLElement>(
      `[data-morpheme-index="${morphemeIndex}"]`,
    );
    if (!capsule || !morpheme) return;
    explanation.focusMorpheme(
      { paragraphId: paragraphId.value, tokenIndex, morphemeIndex },
      token,
      capsule,
      morpheme,
    );
    return;
  }
}

function close() {
  explanation.closeAll("quality-unit-changed");
}

defineExpose({ focusCoordinate, close });
</script>

<template>
  <AnalyzedTextItem
    ref="itemRef"
    container="section"
    class="quality-audit-pane"
    :data-side="side"
    content-class="paragraph-block quality-audit-pane__text"
    :tokens="tokens"
    :paragraph-id="paragraphId"
    character-hits
    :active-character="activeCharacter"
    :changed-range="unit.changed_range"
    :changed-ranges="unit.changed_ranges"
    @pointerover="handlePointerOver"
    @pointerout="handlePointerOut"
  >
    <template #controls><header>{{ title }}</header></template>
  </AnalyzedTextItem>

  <ExplanationPopover
    :show="explanation.renderGate.value.dictionary"
    :anchor="explanation.anchorRect.value"
    :component-anchor="explanation.hasWholePanel.value ? explanation.anchorRect.value : explanation.componentAnchorRect.value"
    :whole-token="explanation.wholeToken.value"
    :whole-lookup="explanation.wholeLookup.value"
    :whole-loading="explanation.wholeLoading.value"
    :whole-can-go-back="explanation.wholeHistory.value.length > 0"
    :component-token="explanation.componentToken.value"
    :component-lookup="explanation.componentLookup.value"
    :component-loading="explanation.componentLoading.value"
    :component-can-go-back="explanation.componentHistory.value.length > 0"
    :component-label="explanation.componentLabel.value"
    :panel-prefix="`quality-${unit.unit_id}-${side}`"
    :shortcuts-enabled="shortcutsEnabled"
    @enter="interaction.handlePopoverEnter"
    @leave="interaction.handlePopoverLeave"
    @navigate-whole="explanation.navigateWhole"
    @navigate-component="explanation.navigateComponent"
    @select-whole-form="explanation.selectWholeForm"
    @select-component-form="explanation.selectComponentForm"
    @back-whole="explanation.backWhole"
    @back-component="explanation.backComponent"
  />
  <GrammarPopover
    :show="explanation.renderGate.value.grammar"
    :tag="explanation.grammarTag.value"
    :anchor="explanation.grammarAnchorRect.value"
    @enter="interaction.handlePopoverEnter"
    @leave="interaction.handlePopoverLeave"
    @open-dictionary="openGrammarDictionary"
  />
</template>

<style scoped>
.quality-audit-pane { min-width: 0; background: var(--bg-primary); }
.quality-audit-pane > header { min-height: 34px; padding: 8px 14px; border-bottom: 1px solid var(--border-color); color: var(--text-muted); font-size: .72rem; font-weight: 700; overflow-wrap: anywhere; }
.quality-audit-pane__text { min-height: 132px; padding: 18px 16px 22px; text-align: left; }
</style>
