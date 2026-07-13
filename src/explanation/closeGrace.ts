export const EXPLANATION_CLOSE_GRACE_MS = 140;

type ScheduleTimer = (callback: () => void, delay: number) => number;

/** 同一次离开过程只允许启动一个关闭计时，外部事件不得续期。 */
export function scheduleCloseGrace(
  currentTimer: number | null,
  scheduleTimer: ScheduleTimer,
  close: () => void,
) {
  if (currentTimer !== null) return currentTimer;
  return scheduleTimer(close, EXPLANATION_CLOSE_GRACE_MS);
}
