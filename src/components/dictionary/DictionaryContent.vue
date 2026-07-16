<script setup lang="ts">
import { DictEntry } from "../../types";
import "../../styles/dictionaries/generic.css";
import "../../styles/dictionaries/daijirin.css";
import "../../styles/dictionaries/shogakukan.css";
import "../../styles/dictionaries/crown.css";

defineProps<{ entry: DictEntry }>();
</script>

<template>
  <div class="dictionary-content" :class="`dictionary-content--${entry.style_profile}`">
    <section v-for="(block, index) in entry.content_blocks" :key="`${block.kind}:${index}`" :class="['dictionary-module', `dictionary-module--${block.kind}`]">
      <h4 v-if="block.label" class="dictionary-module__label">{{ block.label }}</h4>
      <div v-if="block.html" class="dictionary-module__body" v-html="block.html"></div>
    </section>
  </div>
</template>
