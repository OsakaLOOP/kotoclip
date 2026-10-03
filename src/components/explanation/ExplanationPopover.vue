<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { AnnotatedToken, DictionaryLookup } from "../../types";
import { floatDebug } from "../../explanation/floatDebug";
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
  panelPrefix?: string;
  shortcutsEnabled?: boolean;
}>();

const emit = defineEmits<{
  enter: [event: PointerEvent];
  leave: [event: PointerEvent];
  navigateWhole: [target: string];
  navigateComponent: [target: string];
  selectWholeForm: [formId: string];
  selectComponentForm: [formId: string];
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

const panelPrefix = computed(() => props.panelPrefix || "explanation");
const wholePanelId = computed(() => `${panelPrefix.value}-whole-panel`);
const componentPanelId = computed(() => `${panelPrefix.value}-component-panel`);
const enableShortcuts = computed(() => props.shortcutsEnabled ?? true);

function place() {
  if (!props.show || !props.anchor || !props.componentAnchor) {
    floatDebug.record("layout", "explanation-popover", "place-skipped", "render-input-missing", {
      show: props.show,
      hasAnchor: Boolean(props.anchor),
      hasComponentAnchor: Boolean(props.componentAnchor),
    });
    return;
  }
  const componentSize = intrinsicPanelSize(componentPanelId.value);
  if (!componentSize) {
    floatDebug.record("layout", "explanation-popover", "place-skipped", "component-panel-unmounted");
    return;
  }
  const wholeSize = intrinsicPanelSize(wholePanelId.value);
  placement.value = placeExplanationPanels(
    props.anchor,
    props.componentAnchor,
    componentSize,
    { width: window.innerWidth, height: window.innerHeight },
    wholeSize,
  );
  floatDebug.snapshot("layout", {
    viewport: { width: window.innerWidth, height: window.innerHeight },
    panelWidth: panelWidth.value,
    componentIntrinsic: sizeSnapshot(componentSize),
    wholeIntrinsic: sizeSnapshot(wholeSize),
    component: placementSnapshot(placement.value.component),
    whole: placementSnapshot(placement.value.whole),
  });
  floatDebug.record("layout", "explanation-popover", "placed", wholeSize ? "dual-panel" : "single-panel", {
    panelWidth: panelWidth.value,
    component: placementSnapshot(placement.value.component),
    whole: placementSnapshot(placement.value.whole),
  });
  void nextTick().then(capturePanelBoxes);
}

async function connectAndPlace() {
  if (!props.show) {
    floatDebug.record("layout", "explanation-popover", "connect-skipped", "render-gate-closed");
    return;
  }
  const hasWhole = Boolean(props.wholeLookup || props.wholeLoading);
  panelWidth.value = explanationPanelWidth(window.innerWidth, hasWhole);
  await nextTick();
  observer?.disconnect();
  observer = new ResizeObserver(place);
  const contents = [
    document.querySelector(`[data-explanation-content="${wholePanelId.value}"]`),
    document.querySelector(`[data-explanation-content="${componentPanelId.value}"]`),
  ].filter((content): content is Element => Boolean(content));
  contents.forEach((content) => observer?.observe(content));
  floatDebug.record("layout", "explanation-popover", "observer-connected", `${contents.length} content nodes`);
  place();
}

function sizeSnapshot(size: Size | undefined) {
  return size ? { width: Math.round(size.width), height: Math.round(size.height) } : null;
}

function placementSnapshot(value: PopoverPlacement["component"] | undefined) {
  if (!value) return null;
  return {
    left: Math.round(value.left),
    top: Math.round(value.top),
    width: Math.round(value.width),
    height: Math.round(value.height),
    maxHeight: Math.round(value.maxHeight),
  };
}

function capturePanelBoxes() {
  const component = document.getElementById(componentPanelId.value);
  const whole = document.getElementById(wholePanelId.value);
  floatDebug.snapshot("panelBoxes", {
    component: elementRectSnapshot(component),
    whole: elementRectSnapshot(whole),
    plannedComponent: placementSnapshot(placement.value.component),
    plannedWhole: placementSnapshot(placement.value.whole),
  });
}

function elementRectSnapshot(element: HTMLElement | null) {
  if (!element) return null;
  const rect = element.getBoundingClientRect();
  return {
    id: element.id,
    left: Math.round(rect.left),
    top: Math.round(rect.top),
    right: Math.round(rect.right),
    bottom: Math.round(rect.bottom),
    width: Math.round(rect.width),
    height: Math.round(rect.height),
    scrollHeight: Math.round(element.scrollHeight),
  };
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
    :panel-id="wholePanelId"
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
    :shortcuts-enabled="false"
    @enter="emit('enter', $event)"
    @leave="emit('leave', $event)"
    @navigate="emit('navigateWhole', $event)"
    @select-form="emit('selectWholeForm', $event)"
    @back="emit('backWhole')"
  />
  <TooltipPanel
    :panel-id="componentPanelId"
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
    :shortcuts-enabled="enableShortcuts"
    @enter="emit('enter', $event)"
    @leave="emit('leave', $event)"
    @navigate="emit('navigateComponent', $event)"
    @select-form="emit('selectComponentForm', $event)"
    @back="emit('backComponent')"
  />
</template>
