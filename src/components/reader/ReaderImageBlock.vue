<script setup lang="ts">
defineProps<{ src?: string; alt: string; title?: string }>();
const emit = defineEmits<{ load: [] }>();
</script>

<template>
  <figure class="reader-image-block">
    <img v-if="src" :src="src" :alt="alt" @load="emit('load')" @error="emit('load')" />
    <div v-else class="missing-image">图片资源不可用</div>
    <figcaption v-if="title || alt">{{ title || alt }}</figcaption>
  </figure>
</template>

<style scoped>
.reader-image-block {
  display: flex;
  flex-direction: column;
  align-items: center;
  margin: 10px 0 30px;
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
