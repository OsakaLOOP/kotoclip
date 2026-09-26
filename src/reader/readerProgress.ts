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
      message: "准备正文",
    };
  }

  const { complete, total, failed, pending } = session.progress;
  const safeTotal = Math.max(1, total);
  const phase = complete + failed < total ? "processing" : "completed";
  const completed = complete;
  const message = failed > 0
    ? "部分单元失败"
    : session.paused
      ? "分析已暂停"
      : phase === "processing"
        ? `完整单元分析 · ${pending} 个待处理`
        : "分析完成";

  return {
    requestId: session.session_id,
    mode: "analysis",
    phase,
    completed,
    total,
    percent: Math.round((completed / safeTotal) * 100),
    message,
  };
}
