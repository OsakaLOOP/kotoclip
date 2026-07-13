export interface RectSnapshot {
  left: number;
  top: number;
  right: number;
  bottom: number;
  width: number;
  height: number;
}

export interface Size {
  width: number;
  height: number;
}

export interface PanelPlacement {
  left: number;
  top: number;
  width: number;
  height: number;
  maxHeight: number;
}

export interface PopoverPlacement {
  whole?: PanelPlacement;
  component: PanelPlacement;
}

const MARGIN = 12;
const GAP = 10;
const DEFAULT_PANEL_WIDTH = 420;
const MAX_PANEL_HEIGHT = 480;

export function snapshotRect(rect: DOMRect): RectSnapshot {
  return {
    left: rect.left,
    top: rect.top,
    right: rect.right,
    bottom: rect.bottom,
    width: rect.width,
    height: rect.height,
  };
}

/** 外框高度可能受 max-height 限制；scrollHeight 才是当前宽度下的内容固有高度。 */
export function measureIntrinsicPanel(panel: Pick<HTMLElement, "getBoundingClientRect" | "scrollHeight">): Size {
  return {
    width: panel.getBoundingClientRect().width,
    height: panel.scrollHeight,
  };
}

export function explanationPanelWidth(viewportWidth: number, paired: boolean) {
  const availableWidth = paired
    ? (viewportWidth - MARGIN * 2 - GAP) / 2
    : viewportWidth - MARGIN * 2;
  return Math.min(DEFAULT_PANEL_WIDTH, Math.max(0, availableWidth));
}

/**
 * 先确定浮层组位于锚点上方或下方，再整体进行水平平移。
 * 双面板始终左右捆绑，不把上下分置作为桌面布局候选。
 */
export function placeExplanationPanels(
  anchor: RectSnapshot,
  componentAnchor: RectSnapshot,
  componentSize: Size,
  viewport: Size,
  wholeSize?: Size,
): PopoverPlacement {
  let component = constrainedSize(componentSize, viewport);
  if (!wholeSize) {
    return {
      component: placeSinglePanel(componentAnchor, component, viewport),
    };
  }

  let whole = constrainedSize(wholeSize, viewport);
  const availableGroupWidth = Math.max(0, viewport.width - MARGIN * 2 - GAP);
  const combinedPanelWidth = whole.width + component.width;
  const widthScale = combinedPanelWidth > 0
    ? Math.min(1, availableGroupWidth / combinedPanelWidth)
    : 1;
  whole = { ...whole, width: whole.width * widthScale };
  component = { ...component, width: component.width * widthScale };
  const groupWidth = whole.width + GAP + component.width;
  const groupLeft = clamp(
    anchor.left + anchor.width / 2 - groupWidth / 2,
    MARGIN,
    Math.max(MARGIN, viewport.width - MARGIN - groupWidth),
  );
  const desiredHeight = Math.min(MAX_PANEL_HEIGHT, Math.max(whole.height, component.height));
  const vertical = chooseVerticalPlacement(anchor, desiredHeight, viewport);
  const top = vertical.side === "above"
    ? anchor.top - GAP - vertical.height
    : anchor.bottom + GAP;

  return {
    whole: placedPanel(groupLeft, top, whole, vertical.height),
    component: placedPanel(groupLeft + whole.width + GAP, top, component, vertical.height),
  };
}

function placeSinglePanel(anchor: RectSnapshot, size: Size, viewport: Size): PanelPlacement {
  const desiredHeight = Math.min(MAX_PANEL_HEIGHT, size.height);
  const vertical = chooseVerticalPlacement(anchor, desiredHeight, viewport);
  const left = clamp(
    anchor.left + anchor.width / 2 - size.width / 2,
    MARGIN,
    Math.max(MARGIN, viewport.width - MARGIN - size.width),
  );
  const top = vertical.side === "above"
    ? anchor.top - GAP - vertical.height
    : anchor.bottom + GAP;
  return placedPanel(left, top, size, vertical.height);
}

function constrainedSize(size: Size, viewport: Size): Size {
  return {
    width: Math.min(size.width, Math.max(0, viewport.width - MARGIN * 2)),
    height: Math.min(size.height, Math.max(0, viewport.height - MARGIN * 2)),
  };
}

function chooseVerticalPlacement(anchor: RectSnapshot, desiredHeight: number, viewport: Size) {
  const above = Math.max(0, anchor.top - GAP - MARGIN);
  const below = Math.max(0, viewport.height - anchor.bottom - GAP - MARGIN);
  const side = above >= desiredHeight && below < desiredHeight
    ? "above"
    : below >= desiredHeight && above < desiredHeight
      ? "below"
      : above >= below ? "above" : "below";
  return {
    side,
    height: Math.min(desiredHeight, side === "above" ? above : below),
  } as const;
}

function placedPanel(left: number, top: number, size: Size, maxHeight: number): PanelPlacement {
  return {
    left,
    top,
    width: size.width,
    height: Math.min(size.height, maxHeight),
    maxHeight,
  };
}

function clamp(value: number, minimum: number, maximum: number) {
  return Math.min(maximum, Math.max(minimum, value));
}
