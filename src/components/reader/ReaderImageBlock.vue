<script setup lang="ts">
import { ref, watch } from "vue";

const props = defineProps<{ src?: string; alt: string; title?: string; width?: number; height?: number }>();
const emit = defineEmits<{ settled: [] }>();
const imageState = ref<"loading" | "ready" | "error">("loading");

watch(() => props.src, () => {
  imageState.value = "loading";
});

function handleLoad() {
  imageState.value = "ready";
  emit("settled");
}

function handleError() {
  imageState.value = "error";
  emit("settled");
}
</script>

<template>
  <figure class="reader-image-block" :data-image-state="src ? imageState : 'missing'">
    <img
      v-if="src && imageState !== 'error'"
      :src="src"
      :alt="alt"
      :width="width"
      :height="height"
      @load="handleLoad"
      @error="handleError"
    />
    <div v-else class="missing-image">图片资源不可用</div>
    <figcaption v-if="title || alt">{{ title || alt }}</figcaption>
  </figure>
</template>

<style scoped>
.reader-image-block {
  display: flex;
  flex-direction: column;
  align-items: center;
  margin: 0;
}

img {
  display: block;
  max-width: 100%;
  max-height: min(76vh, 900px);
  object-fit: contain;
}

figcaption,
.missing-image {
  margin-top: 8px;
  color: var(--text-muted);
  font-size: 0.72rem;
}

.missing-image {
  display: grid;
  width: 100%;
  min-height: 120px;
  place-items: center;
  border: 1px dashed var(--border-color);
}
</style>
