<script setup lang="ts">
import { computed, ref, watch } from "vue";
import type { ReaderImageLayout, ReaderRowImage } from "../../reader/rows";

const props = defineProps<{ items: ReaderRowImage[]; layout: ReaderImageLayout }>();
const emit = defineEmits<{ settled: [] }>();
type ImageState = "loading" | "ready" | "error" | "missing";
const imageStates = ref<ImageState[]>([]);

watch(
  () => props.items.map((item) => item.resolvedSrc ?? "").join("\n"),
  () => {
    imageStates.value = props.items.map((item) => item.resolvedSrc ? "loading" : "missing");
  },
  { immediate: true },
);

const imageState = computed<ImageState>(() => {
  if (imageStates.value.includes("loading")) return "loading";
  if (imageStates.value.includes("ready")) return "ready";
  if (imageStates.value.includes("error")) return "error";
  return "missing";
});

function handleLoad(index: number) {
  imageStates.value[index] = "ready";
  emit("settled");
}

function handleError(index: number) {
  imageStates.value[index] = "error";
  emit("settled");
}
</script>

<template>
  <figure
    class="reader-image-block"
    :class="`layout-${layout}`"
    :data-image-state="imageState"
  >
    <div v-for="(item, index) in items" :key="item.image.id" class="reader-image-item">
      <img
        v-if="item.resolvedSrc && imageStates[index] !== 'error'"
        :src="item.resolvedSrc"
        :alt="item.image.alt"
        :width="item.intrinsicWidth"
        :height="item.intrinsicHeight"
        @load="handleLoad(index)"
        @error="handleError(index)"
      />
      <div v-else class="missing-image">图片资源不可用</div>
      <figcaption v-if="layout !== 'symbols' && (item.image.title || item.image.alt)">
        {{ item.image.title || item.image.alt }}
      </figcaption>
    </div>
  </figure>
</template>

<style scoped>
.reader-image-block {
  margin: 0;
}

.layout-single {
  display: flex;
  justify-content: center;
}

.layout-pair {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: clamp(8px, 1.6vw, 14px);
}

.layout-symbols {
  display: flex;
  min-height: calc(var(--reader-font-size, 19px) * 1.4);
  align-items: center;
  justify-content: center;
  gap: 1px;
}

.reader-image-item {
  display: flex;
  min-width: 0;
  flex-direction: column;
  align-items: center;
}

.layout-single .reader-image-item {
  width: 100%;
}

.reader-image-item img {
  display: block;
  max-width: 100%;
  max-height: min(76vh, 900px);
  object-fit: contain;
}

.layout-pair img {
  width: 100%;
  height: auto;
}

.layout-symbols .reader-image-item {
  flex: 0 0 auto;
}

.layout-symbols img {
  width: min(calc(var(--reader-font-size, 19px) * 1.25), 28px);
  height: min(calc(var(--reader-font-size, 19px) * 1.25), 28px);
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

.layout-symbols .missing-image {
  width: min(calc(var(--reader-font-size, 19px) * 1.25), 28px);
  min-height: min(calc(var(--reader-font-size, 19px) * 1.25), 28px);
  overflow: hidden;
  margin-top: 0;
  border: 0;
  font-size: 0;
}
</style>
