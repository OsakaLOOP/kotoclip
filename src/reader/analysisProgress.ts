export type AnalysisPhase =
  | "preparing"
  | "tokenizing"
  | "chunking"
  | "grammar_matching"
  | "dictionary_matching"
  | "profile_scoring"
  | "expression_matching"
  | "recording_exposure"
  | "completed";

export type CacheProgressPhase =
  | "cache_reading"
  | "cache_decoding"
  | "cache_restoring"
  | "cache_finalizing"
  | "completed";

export type AnalysisProgressMode = "starting" | "analysis" | "cache";
export type ProgressPhase = AnalysisPhase | CacheProgressPhase;

export interface AnalysisProgress {
  requestId: string;
  mode: AnalysisProgressMode;
  phase: ProgressPhase;
  completed: number;
  total: number;
  percent: number;
  message: string;
}

export interface ProgressStage {
  phase: ProgressPhase;
  label: string;
}

export const analysisProgressStages: readonly ProgressStage[] = [
  { phase: "preparing", label: "准备" },
  { phase: "tokenizing", label: "形态素" },
  { phase: "dictionary_matching", label: "词典" },
  { phase: "chunking", label: "文节" },
  { phase: "grammar_matching", label: "语法" },
  { phase: "profile_scoring", label: "评分" },
  { phase: "expression_matching", label: "表达" },
];

export const cacheProgressStages: readonly ProgressStage[] = [
  { phase: "cache_reading", label: "读取" },
  { phase: "cache_decoding", label: "解析" },
  { phase: "cache_restoring", label: "恢复" },
  { phase: "cache_finalizing", label: "构建" },
];

export function progressStagesForMode(
  mode: AnalysisProgressMode,
): readonly ProgressStage[] {
  if (mode === "analysis") return analysisProgressStages;
  if (mode === "cache") return cacheProgressStages;
  return [];
}

export function progressPhaseIndex(progress: AnalysisProgress): number {
  const stages = progressStagesForMode(progress.mode);
  if (progress.phase === "completed") return stages.length;
  const index = stages.findIndex((stage) => stage.phase === progress.phase);
  if (index >= 0) return index;
  if (progress.mode === "analysis" && progress.phase === "recording_exposure") {
    return stages.length;
  }
  return -1;
}

export function progressCurrentLabel(progress: AnalysisProgress): string {
  if (progress.phase === "completed") return "完成";
  if (progress.mode === "starting") return "加载";
  const stage = progressStagesForMode(progress.mode).find(
    (item) => item.phase === progress.phase,
  );
  if (stage) return stage.label;
  return progress.mode === "cache" ? "缓存" : "分析";
}

export function isProgressIndeterminate(progress: AnalysisProgress): boolean {
  return progress.mode === "starting" || progress.mode === "cache";
}
