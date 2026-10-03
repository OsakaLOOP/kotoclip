import type { DocumentSession } from "./session";
import type { AnalysisProgress } from "./analysisProgress";

export function readerAnalysisProgress(
  session: DocumentSession | null,
  preparing: boolean,
): AnalysisProgress {
  if (preparing || !session) {
    return {
      requestId: session?.session_id || "reader",
      mode: "analysis",
      phase: "preparing",
      completed: 0,
      total: 0,
      percent: 0,
      message: "准备分析",
    };
  }

  const { analysis, cache, failed } = session.progress;
  const completed = analysis.complete + cache.complete;
  const total = analysis.total + cache.total;
  const remaining = total - completed;
  const weightedComplete = 3 + analysis.complete + cache.complete / 4;
  const weightedTotal = 3 + analysis.total + cache.total / 4;
  const phase = remaining > failed ? "processing" : "completed";
  const message = failed > 0
    ? "部分单元失败"
    : session.paused
      ? "分析已暂停"
      : phase === "processing"
        ? `剩余 ${remaining} 个单元`
        : "分析完成";

  return {
    requestId: session.session_id,
    mode: "analysis",
    phase,
    completed,
    total,
    percent: Math.floor((weightedComplete / weightedTotal) * 1000) / 10,
    message,
    work: { analysis, cache },
  };
}
