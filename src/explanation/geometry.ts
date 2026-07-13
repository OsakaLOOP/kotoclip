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

/** 使用面板固有尺寸生成候选，并按溢出、遮挡锚点、距离依次评分。 */
export function placeExplanationPanels(
  anchor: RectSnapshot,
  componentAnchor: RectSnapshot,
  componentSize: Size,
  viewport: Size,
  wholeSize?: Size,
): PopoverPlacement {
  const component = constrainedSize(componentSize, viewport);
  if (!wholeSize) {
    return {
      component: bestSinglePlacement(componentAnchor, component, viewport),
    };
  }

  const whole = constrainedSize(wholeSize, viewport);
  const center = anchor.left + anchor.width / 2;
  const groupWidth = whole.width + GAP + component.width;
  const horizontalLeft = center - groupWidth / 2;
  const aboveHeight = Math.max(120, anchor.top - GAP - MARGIN);
  const belowHeight = Math.max(120, viewport.height - anchor.bottom - GAP - MARGIN);
  const candidates: PopoverPlacement[] = [
    {
      whole: abovePanel(horizontalLeft, anchor.top, whole, viewport, aboveHeight),
      component: abovePanel(horizontalLeft + whole.width + GAP, anchor.top, component, viewport, aboveHeight),
    },
    {
      whole: belowPanel(horizontalLeft, anchor.bottom, whole, viewport, belowHeight),
      component: belowPanel(horizontalLeft + whole.width + GAP, anchor.bottom, component, viewport, belowHeight),
    },
    {
      whole: abovePanel(center - whole.width / 2, anchor.top, whole, viewport, aboveHeight),
      component: belowPanel(center - component.width / 2, anchor.bottom, component, viewport, belowHeight),
    },
  ];
  return candidates.sort((left, right) => scoreGroup(left, anchor, viewport) - scoreGroup(right, anchor, viewport))[0];
}

function bestSinglePlacement(anchor: RectSnapshot, size: Size, viewport: Size): PanelPlacement {
  const centerX = anchor.left + anchor.width / 2;
  const centerY = anchor.top + anchor.height / 2;
  const aboveHeight = Math.max(120, anchor.top - GAP - MARGIN);
  const belowHeight = Math.max(120, viewport.height - anchor.bottom - GAP - MARGIN);
  const candidates = [
    abovePanel(centerX - size.width / 2, anchor.top, size, viewport, aboveHeight),
    belowPanel(centerX - size.width / 2, anchor.bottom, size, viewport, belowHeight),
    panel(anchor.left - GAP - size.width, centerY - size.height / 2, size, viewport, 620),
    panel(anchor.right + GAP, centerY - size.height / 2, size, viewport, 620),
  ];
  return candidates.sort((left, right) => scorePanel(left, anchor, viewport) - scorePanel(right, anchor, viewport))[0];
}

function constrainedSize(size: Size, viewport: Size): Size {
  return {
    width: Math.min(size.width, viewport.width - MARGIN * 2),
    height: Math.min(size.height, viewport.height - MARGIN * 2),
  };
}

function abovePanel(left: number, anchorTop: number, size: Size, viewport: Size, maxHeight: number) {
  const height = Math.min(size.height, maxHeight);
  return panel(left, anchorTop - GAP - height, { ...size, height }, viewport, maxHeight);
}

function belowPanel(left: number, anchorBottom: number, size: Size, viewport: Size, maxHeight: number) {
  const height = Math.min(size.height, maxHeight);
  return panel(left, anchorBottom + GAP, { ...size, height }, viewport, maxHeight);
}

function panel(left: number, top: number, size: Size, viewport: Size, maxHeight: number): PanelPlacement {
  return {
    left: clamp(left, MARGIN, Math.max(MARGIN, viewport.width - MARGIN - size.width)),
    top: clamp(top, MARGIN, Math.max(MARGIN, viewport.height - MARGIN - size.height)),
    width: size.width,
    height: size.height,
    maxHeight: Math.max(120, Math.min(maxHeight, viewport.height - MARGIN * 2)),
  };
}

function scoreGroup(group: PopoverPlacement, anchor: RectSnapshot, viewport: Size) {
  const wholeScore = group.whole ? scorePanel(group.whole, anchor, viewport) : 0;
  const componentScore = scorePanel(group.component, anchor, viewport);
  const overlap = group.whole ? intersectionArea(group.whole, group.component) * 50 : 0;
  return wholeScore + componentScore + overlap;
}

function scorePanel(panel: PanelPlacement, anchor: RectSnapshot, viewport: Size) {
  const overflow = Math.max(0, -panel.left) + Math.max(0, -panel.top)
    + Math.max(0, panel.left + panel.width - viewport.width)
    + Math.max(0, panel.top + panel.height - viewport.height);
  const anchorOverlap = intersectionArea(panel, anchor);
  const panelCenterX = panel.left + panel.width / 2;
  const panelCenterY = panel.top + panel.height / 2;
  const anchorCenterX = anchor.left + anchor.width / 2;
  const anchorCenterY = anchor.top + anchor.height / 2;
  const distance = Math.hypot(panelCenterX - anchorCenterX, panelCenterY - anchorCenterY);
  return overflow * 1000 + anchorOverlap * 100 + distance;
}

function intersectionArea(left: Pick<RectSnapshot, "left" | "top"> & Partial<RectSnapshot>, right: Pick<RectSnapshot, "left" | "top"> & Partial<RectSnapshot>) {
  const leftRight = left.right ?? left.left + (left.width ?? 0);
  const leftBottom = left.bottom ?? left.top + (left.height ?? 0);
  const rightRight = right.right ?? right.left + (right.width ?? 0);
  const rightBottom = right.bottom ?? right.top + (right.height ?? 0);
  return Math.max(0, Math.min(leftRight, rightRight) - Math.max(left.left, right.left))
    * Math.max(0, Math.min(leftBottom, rightBottom) - Math.max(left.top, right.top));
}

function clamp(value: number, minimum: number, maximum: number) {
  return Math.min(maximum, Math.max(minimum, value));
}
