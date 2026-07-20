export type ReaderRowKind = "text" | "image";

export interface ReaderRowEstimateInput {
  kind: ReaderRowKind;
  heading: boolean;
  viewportHeight: number;
  fontSize: number;
  lineHeight: number;
  contentWidth: number;
  imageWidth?: number;
  imageHeight?: number;
  hasCaption?: boolean;
}

export interface ReaderRowMeasurementInput {
  kind: ReaderRowKind;
  imageState?: string;
  cachedSize?: number;
  estimatedSize: number;
  observedSize?: number;
  elementSize: number;
}

export function estimateReaderRow(input: ReaderRowEstimateInput): number {
  if (input.kind === "image") {
    if (input.imageWidth && input.imageHeight) {
      const heightLimit = Math.min(input.viewportHeight * 0.76, 900);
      const scale = Math.min(1, input.contentWidth / input.imageWidth, heightLimit / input.imageHeight);
      return Math.round(input.imageHeight * scale + 20 + (input.hasCaption ? 25 : 0));
    }
    return Math.round(Math.min(760, Math.max(420, input.viewportHeight * 0.72)));
  }
  const textHeight = input.fontSize * input.lineHeight * 2;
  return Math.round(textHeight + (input.heading ? 42 : 0));
}

/** 图片解码完成前保留已有尺寸或估算值，不能用接近 0 的占位 DOM 覆盖缓存。 */
export function resolveReaderRowMeasurement(input: ReaderRowMeasurementInput): number {
  if (input.kind === "image" && input.imageState === "loading") {
    return input.cachedSize ?? input.estimatedSize;
  }
  // 虚拟行使用绝对定位；向下取整不足 1px 也可能让下一行侵入当前行的下沿。
  return Math.ceil(input.observedSize ?? input.elementSize);
}
