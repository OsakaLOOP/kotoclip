<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { AnnotatedToken, DictionaryLookup } from "../../types";
import { explanationPanelWidth, measureIntrinsicPanel, placeExplanationPanels, type PopoverPlacement, type RectSnapshot, type Size } from "../../explanation/geometry";
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

function intrinsicPanelSize(panelId: string): Size | undefined {
  const panel = document.getElementById(panelId);
  if (!panel) return undefined;
  return measureIntrinsicPanel(panel);
}

function place() {
  if (!props.show || !props.anchor || !props.componentAnchor) return;
  const componentSize = intrinsicPanelSize("explanation-component-panel");
  if (!componentSize) return;
  const wholeSize = intrinsicPanelSize("explanation-whole-panel");
  placement.value = placeExplanationPanels(
    props.anchor,
    props.componentAnchor,
    componentSize,
    { width: window.innerWidth, height: window.innerHeight },
    wholeSize,
  );
}

async function connectAndPlace() {
  if (!props.show) return;
  const hasWhole = Boolean(props.wholeLookup || props.wholeLoading);
  panelWidth.value = explanationPanelWidth(window.innerWidth, hasWhole);
  await nextTick();
  observer?.disconnect();
  observer = new ResizeObserver(place);
  const contents = document.querySelectorAll("[data-explanation-content]");
  contents.forEach((content) => observer?.observe(content));
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
