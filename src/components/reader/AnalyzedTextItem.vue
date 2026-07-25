<script setup lang="ts">
import { ref, type Component } from "vue";
import type { AnnotatedToken } from "../../types";
import BunsetsuCapsule from "../BunsetsuCapsule.vue";

defineOptions({ inheritAttrs: false });

withDefaults(defineProps<{
  container?: string | Component;
  contentContainer?: string | Component;
  contentClass?: string | string[] | Record<string, boolean>;
  tokens: AnnotatedToken[];
  paragraphId: number;
  isDragSelected?: (tokenIndex: number, token: AnnotatedToken) => boolean;
  characterHits?: boolean;
  activeCharacter?: number | null;
  changedRange?: [number, number] | null;
  changedRanges?: [number, number][];
}>(), {
  container: "div",
  contentContainer: "div",
  contentClass: "",
  isDragSelected: () => false,
  characterHits: false,
  activeCharacter: null,
  changedRange: null,
  changedRanges: () => [],
});

const rootElement = ref<HTMLElement | null>(null);
defineExpose({ rootElement });
</script>

<template>
  <component
    :is="container"
    ref="rootElement"
    v-bind="$attrs"
    class="analyzed-text-item"
  >
    <slot name="controls" />
    <component
      :is="contentContainer"
      class="analyzed-text-item__content"
      :class="contentClass"
    >
      <template v-if="tokens.length">
        <BunsetsuCapsule
          v-for="(token, tokenIndex) in tokens"
          :key="`${token.bunsetsu.char_range[0]}-${tokenIndex}`"
          :token="token"
          :paragraph-id="paragraphId"
          :token-index="tokenIndex"
          :is-drag-selected="isDragSelected(tokenIndex, token)"
          :tokens="tokens"
          :character-hits="characterHits"
          :active-character="activeCharacter"
          :changed-range="changedRange"
          :changed-ranges="changedRanges"
        />
      </template>
      <slot v-else name="empty">
        <span class="empty-line-placeholder">&nbsp;</span>
      </slot>
    </component>
    <slot name="after" />
  </component>
</template>
