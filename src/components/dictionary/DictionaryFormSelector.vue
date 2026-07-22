<script setup lang="ts">
import { computed } from "vue";
import type { DictionaryFormGroup } from "../../types";

const props = defineProps<{
  forms: DictionaryFormGroup[];
  selectedFormId?: string | null;
}>();

const emit = defineEmits<{ select: [formId: string] }>();

const activeFormId = computed(() => props.selectedFormId ?? props.forms[0]?.form_id ?? "");

function formTitle(form: DictionaryFormGroup) {
  const variants = form.variants.map((variant) => variant.surface_form).join(" / ");
  const readings = form.readings.join(" / ");
  return [variants, readings].filter(Boolean).join("；");
}

function handleSelect(event: Event) {
  const formId = (event.target as HTMLSelectElement).value;
  if (formId && formId !== activeFormId.value) emit("select", formId);
}
</script>

<template>
  <section v-if="forms.length > 1" class="form-selector">
    <div class="form-selector-label">表记</div>
    <select
      v-if="forms.length > 8"
      class="form-select"
      :value="activeFormId"
      aria-label="表记"
      @change="handleSelect"
    >
      <option v-for="form in forms" :key="form.form_id" :value="form.form_id">
        {{ form.display_form }}
      </option>
    </select>
    <div v-else class="form-options">
      <button
        v-for="form in forms"
        :key="form.form_id"
        type="button"
        :class="{ active: form.form_id === activeFormId }"
        :aria-pressed="form.form_id === activeFormId"
        :title="formTitle(form)"
        @click="emit('select', form.form_id)"
      >
        {{ form.display_form }}
      </button>
    </div>
  </section>
</template>

<style scoped>
.form-selector { display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 9px; align-items: start; margin-top: 7px; padding-top: 9px; border-top: 1px solid var(--border-color); }
.form-selector-label { padding-top: 5px; color: var(--text-muted); font: 700 .7rem var(--font-ui); }
.form-options { display: flex; flex-wrap: wrap; gap: 6px; min-width: 0; }
button { min-width: 0; min-height: 29px; max-width: 100%; overflow: hidden; border: 1px solid var(--border-color); border-radius: 999px; padding: 3px 10px; background: color-mix(in srgb, var(--bg-card) 88%, transparent); color: var(--accent-color); font: inherit; text-overflow: ellipsis; white-space: nowrap; cursor: pointer; }
button:hover, button.active { border-color: var(--accent-color); background: var(--accent-light); }
button.active { box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent-color) 32%, transparent); }
.form-select { width: 100%; min-width: 0; min-height: 31px; border: 1px solid var(--border-color); border-radius: 5px; padding: 3px 8px; background: var(--bg-card); color: var(--text-primary); font: .78rem var(--font-ja); }
</style>
