export interface ReaderAppearance {
  fontSize: number;
  lineHeight: number;
  paragraphGap: number;
  contentWidth: number;
}

export const DEFAULT_READER_APPEARANCE: ReaderAppearance = {
  fontSize: 19,
  lineHeight: 2.05,
  paragraphGap: 20,
  contentWidth: 760,
};

export function normalizeAppearance(value: Partial<ReaderAppearance>): ReaderAppearance {
  return {
    fontSize: clampNumber(value.fontSize, 14, 28, DEFAULT_READER_APPEARANCE.fontSize),
    lineHeight: clampNumber(value.lineHeight, 1.5, 2.8, DEFAULT_READER_APPEARANCE.lineHeight),
    paragraphGap: clampNumber(value.paragraphGap, 8, 40, DEFAULT_READER_APPEARANCE.paragraphGap),
    contentWidth: clampNumber(value.contentWidth, 520, 1040, DEFAULT_READER_APPEARANCE.contentWidth),
  };
}

export function readingEstimate(current: number, total: number, now = new Date()) {
  const boundedTotal = Math.max(0, total);
  const boundedCurrent = Math.min(Math.max(0, current), boundedTotal);
  const remainingCharacters = boundedTotal - boundedCurrent;
  const remainingMinutes = Math.ceil(remainingCharacters / 400);
  const completion = new Date(now.getTime() + remainingMinutes * 60_000);
  return {
    percent: boundedTotal === 0 ? 0 : boundedCurrent / boundedTotal,
    remainingCharacters,
    remainingMinutes,
    completionLabel: completion.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
  };
}

export function formatReadingDuration(minutes: number): string {
  if (minutes < 1) return "不足 1 分钟";
  if (minutes < 60) return `约 ${minutes} 分钟`;
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  return rest === 0 ? `约 ${hours} 小时` : `约 ${hours} 小时 ${rest} 分钟`;
}

function clampNumber(value: number | undefined, min: number, max: number, fallback: number): number {
  if (!Number.isFinite(value)) return fallback;
  return Math.min(max, Math.max(min, value as number));
}
