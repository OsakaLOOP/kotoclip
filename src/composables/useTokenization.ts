import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { AnnotatedToken, SegmentationCandidate } from "../types";

export interface Paragraph {
  id: number;
  tokens: AnnotatedToken[];
}

export type AnalysisPhase =
  | "preparing"
  | "tokenizing"
  | "chunking"
  | "grammar_matching"
  | "dictionary_matching"
  | "profile_scoring"
  | "recording_exposure"
  | "completed";

export interface AnalysisProgress {
  requestId: string;
  phase: AnalysisPhase;
  completed: number;
  total: number;
  percent: number;
  message: string;
}

const initialProgress = (): AnalysisProgress => ({
  requestId: "",
  phase: "preparing",
  completed: 0,
  total: 0,
  percent: 0,
  message: "等待分析",
});

export function useTokenization() {
  const paragraphs = ref<Paragraph[]>([]);
  const isAnalyzing = ref(false);
  const errorMsg = ref<string | null>(null);
  const analysisProgress = ref<AnalysisProgress>(initialProgress());
  const activeRequestId = ref<string | null>(null);

  /**
   * 分析整页文本
   */
  async function analyzeText(text: string, recordExposure = true) {
    if (!text.trim()) {
      paragraphs.value = [];
      analysisProgress.value = initialProgress();
      return false;
    }
    if (isAnalyzing.value) return false;

    isAnalyzing.value = true;
    errorMsg.value = null;
    const requestId = globalThis.crypto?.randomUUID?.()
      ?? `analysis-${Date.now()}-${Math.random().toString(16).slice(2)}`;
    activeRequestId.value = requestId;
    analysisProgress.value = {
      ...initialProgress(),
      requestId,
      message: "准备分析",
    };
    let unlisten: UnlistenFn | undefined;

    try {
      // 先建立监听再调用 IPC，避免丢失最早的阶段事件。
      unlisten = await listen<AnalysisProgress>("analysis-progress", ({ payload }) => {
        if (payload.requestId === activeRequestId.value) {
          analysisProgress.value = payload;
        }
      });
      // 调用 Tauri 命令进行分词与用户画像评分标注
      const allTokens = await invoke<AnnotatedToken[]>("analyze_text", {
        text,
        recordExposure,
        requestId,
      });
      if (activeRequestId.value !== requestId) return false;
      
      // 根据换行符切分为段落结构，方便虚拟列表高效渲染
      const tempParagraphs: Paragraph[] = [];
      let currentTokens: AnnotatedToken[] = [];
      let paragraphId = 0;

      const trimWhitespaceTokens = (tokens: AnnotatedToken[]): AnnotatedToken[] => {
        const isBlank = (t: AnnotatedToken) => /^\s+$/.test(t.bunsetsu.surface);
        let start = 0;
        while (start < tokens.length && isBlank(tokens[start])) {
          start++;
        }
        let end = tokens.length;
        while (end > start && isBlank(tokens[end - 1])) {
          end--;
        }
        return tokens.slice(start, end);
      };

      for (const token of allTokens) {
        // 判断是否为换行符
        if (token.bunsetsu.surface === "\n" || token.bunsetsu.surface === "\r\n") {
          tempParagraphs.push({
            id: paragraphId++,
            tokens: trimWhitespaceTokens(currentTokens),
          });
          currentTokens = [];
        } else {
          currentTokens.push(token);
        }
      }

      // 将剩余的最后一组 tokens 压入
      if (currentTokens.length > 0 || tempParagraphs.length === 0) {
        tempParagraphs.push({
          id: paragraphId++,
          tokens: trimWhitespaceTokens(currentTokens),
        });
      }

      paragraphs.value = tempParagraphs;
      return true;
    } catch (err: any) {
      if (activeRequestId.value === requestId) {
        errorMsg.value = err.toString() || "分词分析失败";
        analysisProgress.value = {
          ...analysisProgress.value,
          message: "分析失败",
        };
      }
      console.error("Tokenization Error:", err);
      return false;
    } finally {
      unlisten?.();
      if (activeRequestId.value === requestId) {
        isAnalyzing.value = false;
        activeRequestId.value = null;
      }
    }
  }

  /**
   * 合并两个或多个文节 (添加自定义合并规则)
   */
  async function mergeTokens(surfaces: string[]) {
    try {
      await invoke("add_merge_rule", { parts: surfaces });
    } catch (err: any) {
      console.error("Merge Rule Register Error:", err);
      throw err;
    }
  }

  async function splitToken(token: AnnotatedToken) {
    return await invoke<AnnotatedToken[]>("split_token", { token });
  }

  async function getCandidates(token: AnnotatedToken, topN = 5) {
    return await invoke<SegmentationCandidate[]>("get_candidates", { token, topN });
  }

  return {
    paragraphs,
    isAnalyzing,
    errorMsg,
    analysisProgress,
    analyzeText,
    mergeTokens,
    splitToken,
    getCandidates,
  };
}
