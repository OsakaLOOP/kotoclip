export type SegmentedActionLayout = "1x2" | "2x1" | "2x2";

export type FrameCornerPosition = "top-left" | "top-right" | "bottom-left" | "bottom-right";
export type FrameDividerOrientation = "horizontal" | "vertical";

export interface FrameCornerDefinition {
  position: FrameCornerPosition;
  actionIndex: number;
}

export interface FrameDividerDefinition {
  id: string;
  orientation: FrameDividerOrientation;
  beforeIndex: number;
  afterIndex: number;
  split: string;
  start: string;
  end: string;
  startOuter: boolean;
  endOuter: boolean;
}

export interface SegmentedFrameLayoutDefinition {
  rows: number;
  columns: number;
  capacity: number;
  corners: readonly FrameCornerDefinition[];
  dividers: readonly FrameDividerDefinition[];
}

const OUTER_START = "0px";
const CENTER_START = "calc(50% + var(--frame-center-gap) / 2)";
const CENTER_END = "calc(50% + var(--frame-center-gap) / 2)";

const layoutDefinitions: Record<SegmentedActionLayout, SegmentedFrameLayoutDefinition> = {
  "1x2": {
    rows: 1,
    columns: 2,
    capacity: 2,
    corners: [
      { position: "top-left", actionIndex: 0 },
      { position: "bottom-left", actionIndex: 0 },
      { position: "top-right", actionIndex: 1 },
      { position: "bottom-right", actionIndex: 1 },
    ],
    dividers: [
      {
        id: "column",
        orientation: "vertical",
        beforeIndex: 0,
        afterIndex: 1,
        split: "50%",
        start: OUTER_START,
        end: OUTER_START,
        startOuter: true,
        endOuter: true,
      },
    ],
  },
  "2x1": {
    rows: 2,
    columns: 1,
    capacity: 2,
    corners: [
      { position: "top-left", actionIndex: 0 },
      { position: "top-right", actionIndex: 0 },
      { position: "bottom-left", actionIndex: 1 },
      { position: "bottom-right", actionIndex: 1 },
    ],
    dividers: [
      {
        id: "row",
        orientation: "horizontal",
        beforeIndex: 0,
        afterIndex: 1,
        split: "50%",
        start: OUTER_START,
        end: OUTER_START,
        startOuter: true,
        endOuter: true,
      },
    ],
  },
  "2x2": {
    rows: 2,
    columns: 2,
    capacity: 4,
    corners: [
      { position: "top-left", actionIndex: 0 },
      { position: "top-right", actionIndex: 1 },
      { position: "bottom-left", actionIndex: 2 },
      { position: "bottom-right", actionIndex: 3 },
    ],
    dividers: [
      {
        id: "row-left",
        orientation: "horizontal",
        beforeIndex: 0,
        afterIndex: 2,
        split: "50%",
        start: OUTER_START,
        end: CENTER_END,
        startOuter: true,
        endOuter: false,
      },
      {
        id: "row-right",
        orientation: "horizontal",
        beforeIndex: 1,
        afterIndex: 3,
        split: "50%",
        start: CENTER_START,
        end: OUTER_START,
        startOuter: false,
        endOuter: true,
      },
      {
        id: "column-top",
        orientation: "vertical",
        beforeIndex: 0,
        afterIndex: 1,
        split: "50%",
        start: OUTER_START,
        end: CENTER_END,
        startOuter: true,
        endOuter: false,
      },
      {
        id: "column-bottom",
        orientation: "vertical",
        beforeIndex: 2,
        afterIndex: 3,
        split: "50%",
        start: CENTER_START,
        end: OUTER_START,
        startOuter: false,
        endOuter: true,
      },
    ],
  },
};

export function segmentedFrameLayout(layout: SegmentedActionLayout): SegmentedFrameLayoutDefinition {
  return layoutDefinitions[layout];
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(maximum, Math.max(minimum, value));
}

function smootherstep(value: number): number {
  return value * value * value * (value * (value * 6 - 15) + 10);
}

export function frameMotionRatio(
  normalizedX: number,
  normalizedY: number,
  centerPlateau = 0.2,
  falloffPower = 0.72,
): number {
  const plateau = clamp(centerPlateau, 0, 0.95);
  const power = Math.max(0.01, falloffPower);
  const distance = Math.hypot(normalizedX, normalizedY);
  if (distance <= plateau) return 1;
  if (distance >= 1) return 0;

  const progress = (distance - plateau) / (1 - plateau);
  return Math.pow(1 - smootherstep(progress), power);
}

export function minimumFrameCenterGap(
  hoverExpansion: number,
  pressExpansion: number,
  lineWidth: number,
): number {
  const maximumExpansion = Math.max(0, hoverExpansion) + Math.max(0, pressExpansion);
  return maximumExpansion * 2 + Math.max(0, lineWidth);
}
