<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { AnnotatedToken, DictionaryLookup } from "../../types";
import { placeExplanationPanels, type PopoverPlacement, type RectSnapshot } from "../../explanation/geometry";
import TooltipPanel from "../TooltipPanel.vue";

const props = defineProps<{
  show: boolean;
  anchor: RectSnapshot | null;
  componentAnchor: RectSnapshot | null;
  wholeToken: AnnotatedToken | null;
  wholeLookup: DictionaryLookup | null;
  wholeLoading: boolean;
  wholeCanGoBack: boolean;
  componentToken: AnnotatedToken | null;
  componentLookup: DictionaryLookup | null;
  componentLoading: boolean;
  componentCanGoBack: boolean;
  componentLabel: string;
}>();

const emit = defineEmits<{
  enter: [event: PointerEvent];
  leave: [event: PointerEvent];
  navigateWhole: [target: string];
  navigateComponent: [target: string];
  selectWhole: [target: string];
  selectComponent: [target: string];
  backWhole: [];
  backComponent: [];
}>();

const placement = ref<PopoverPlacement>({
  component: { left: -10000, top: -10000, width: 420, height: 320, maxHeight: 620 },
});
const panelWidth = ref(420);
let observer: ResizeObserver | null = null;

function place() {
  if (!props.show || !props.anchor || !props.componentAnchor) return;
  const componentElement = document.getElementById("explanation-component-panel");
  if (!componentElement) return;
  const componentRect = componentElement.getBoundingClientRect();
  const wholeElement = document.getElementById("explanation-whole-panel");
  const wholeRect = wholeElement?.getBoundingClientRect();
  placement.value = placeExplanationPanels(
    props.anchor,
    props.componentAnchor,
    { width: componentRect.width, height: componentRect.height },
    { width: window.innerWidth, height: window.innerHeight },
    wholeRect ? { width: wholeRect.width, height: wholeRect.height } : undefined,
  );
}

async function connectAndPlace() {
  if (!props.show) return;
  panelWidth.value = Math.min(420, window.innerWidth - 24);
  await nextTick();
  observer?.disconnect();
  observer = new ResizeObserver(place);
  const component = document.getElementById("explanation-component-panel");
  const whole = document.getElementById("explanation-whole-panel");
  if (component) observer.observe(component);
  if (whole) observer.observe(whole);
  place();
}

watch(
  () => [props.show, props.anchor, props.componentAnchor, props.wholeLookup, props.wholeLoading, props.componentLookup, props.componentLoading],
  connectAndPlace,
  { flush: "post" },
);

onMounted(() => window.addEventListener("resize", connectAndPlace));
onBeforeUnmount(() => {
  observer?.disconnect();
  window.removeEventListener("resize", connectAndPlace);
});
</script>

<template>
  <TooltipPanel
    v-if="wholeLookup || wholeLoading"
    panel-id="explanation-whole-panel"
    :show="show"
    :x="placement.whole?.left ?? -10000"
    :y="placement.whole?.top ?? -10000"
    :width="panelWidth"
    :max-height="placement.whole?.maxHeight ?? 620"
    :token="wholeToken"
    :lookup="wholeLookup"
    :loading="wholeLoading"
    kind-label="整体"
    :can-go-back="wholeCanGoBack"
    @enter="emit('enter', $event)"
    @leave="emit('leave', $event)"
    @navigate="emit('navigateWhole', $event)"
    @select="emit('selectWhole', $event)"
    @back="emit('backWhole')"
  />
  <TooltipPanel
    panel-id="explanation-component-panel"
    :show="show"
    :x="placement.component.left"
    :y="placement.component.top"
    :width="panelWidth"
    :max-height="placement.component.maxHeight"
    :token="componentToken"
    :lookup="componentLookup"
    :loading="componentLoading"
    :kind-label="componentLabel"
    :can-go-back="componentCanGoBack"
    @enter="emit('enter', $event)"
    @leave="emit('leave', $event)"
    @navigate="emit('navigateComponent', $event)"
    @select="emit('selectComponent', $event)"
    @back="emit('backComponent')"
  />
</template>
