import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { AnnotatedToken, ExpressionBoundaryEffect, ExpressionRule, ExpressionType, SegmentationCandidate } from "../types";

export interface Paragraph {
  id: number;
  tokens: AnnotatedToken[];
  isDialogue: boolean;
}

export interface FrontendAnalysisTiming {
  listenerSetupMs: number;
  invokeAndTransferMs: number;
  paragraphBuildMs: number;
  totalBeforeRenderMs: number;
  backendDurationMs: number;
  ipcAndParseMs: number;
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
  const frontendTiming = ref<FrontendAnalysisTiming | null>(null);

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
    const totalStartedAt = performance.now();
    let listenerSetupMs = 0;
    let invokeAndTransferMs = 0;

    try {
      // 先建立监听再调用 IPC，避免丢失最早的阶段事件。
      const listenerStartedAt = performance.now();
      unlisten = await listen<AnalysisProgress>("analysis-progress", ({ payload }) => {
        if (payload.requestId === activeRequestId.value) {
          analysisProgress.value = payload;
        }
      });
      listenerSetupMs = performance.now() - listenerStartedAt;
      // 调用 Tauri 命令进行分词与用户画像评分标注
      const invokeStartedAt = performance.now();
      const response = await invoke<{ tokens: AnnotatedToken[]; backendDurationMs: number }>("analyze_text", {
        text,
        recordExposure,
        requestId,
      });
      const allTokens = response.tokens;
      const backendDurationMs = response.backendDurationMs;
      invokeAndTransferMs = performance.now() - invokeStartedAt;
      if (activeRequestId.value !== requestId) return false;
      
      const paragraphBuildStartedAt = performance.now();
      // 1. 根据 line_break 将 tokens 划分为源行
      const lines: AnnotatedToken[][] = [];
      let currentLine: AnnotatedToken[] = [];
      for (const token of allTokens) {
        if (token.display_class === "line_break") {
          lines.push(currentLine);
          currentLine = [];
        } else {
          currentLine.push(token);
        }
      }
      lines.push(currentLine);

      // 2. 标点跟随上一行调整逻辑 (避头尾合并)
      for (let idx = 1; idx < lines.length; idx++) {
        const line = lines[idx];
        let puncCount = 0;
        for (const token of line) {
          if (token.display_class === "punctuation") {
            const isOpener = token.bunsetsu.surface.trim().startsWith("「") || token.bunsetsu.surface.trim().startsWith("『");
            if (isOpener) {
              break;
            }
            puncCount++;
          } else {
            break;
          }
        }
        if (puncCount > 0) {
          const puncs = line.splice(0, puncCount);
          lines[idx - 1].push(...puncs);
        }
      }

      // 3. 构建阅读块 Paragraph 数组
      const tempParagraphs: Paragraph[] = [];
      let currentBlockTokens: AnnotatedToken[] = [];
      let currentBlockIsDialogue = false;
      let paragraphId = 0;

      const isLineEmpty = (l: AnnotatedToken[]) => {
        return l.every(t => /^\s*$/.test(t.bunsetsu.surface));
      };

      const isLineDialogue = (l: AnnotatedToken[]) => {
        const firstNonEmpty = l.find(t => t.bunsetsu.surface.trim().length > 0);
        if (!firstNonEmpty) return false;
        const text = firstNonEmpty.bunsetsu.surface.trim();
        return text.startsWith("「") || text.startsWith("『");
      };

      const trimWhitespaceTokens = (toks: AnnotatedToken[]): AnnotatedToken[] => {
        const isBlank = (t: AnnotatedToken) => /^\s+$/.test(t.bunsetsu.surface);
        let start = 0;
        while (start < toks.length && isBlank(toks[start])) {
          start++;
        }
        let end = toks.length;
        while (end > start && isBlank(toks[end - 1])) {
          end--;
        }
        return toks.slice(start, end);
      };

      const flushBlock = () => {
        const trimmed = trimWhitespaceTokens(currentBlockTokens);
        if (trimmed.length > 0) {
          tempParagraphs.push({
            id: paragraphId++,
            tokens: trimmed,
            isDialogue: currentBlockIsDialogue,
          });
          currentBlockTokens = [];
        }
      };

      let consecutiveEmptyLinesBefore = 0;

      for (let idx = 0; idx < lines.length; idx++) {
        const line = lines[idx];
        const isEmpty = isLineEmpty(line);

        if (isEmpty) {
          consecutiveEmptyLinesBefore++;
          flushBlock();
        } else {
          const isDialogue = isLineDialogue(line);

          if (isDialogue) {
            flushBlock();
            const trimmedDial = trimWhitespaceTokens(line);
            if (trimmedDial.length > 0) {
              tempParagraphs.push({
                id: paragraphId++,
                tokens: trimmedDial,
                isDialogue: true,
              });
            }
            consecutiveEmptyLinesBefore = 0;
          } else {
            if (currentBlockIsDialogue || consecutiveEmptyLinesBefore > 0) {
              flushBlock();
            }

            if (currentBlockTokens.length > 0) {
              currentBlockTokens.push({
                bunsetsu: {
                  morphemes: [],
                  surface: "\n",
                  head_word: {
                    surface: "\n",
                    base_form: "\n",
                    reading: "",
                    pos: { major: "改行", sub1: "*", sub2: "*", sub3: "*" }
                  },
                  grammar_tags: [],
                  char_range: [0, 0]
                },
                novelty_score: 0,
                is_selected: false,
                is_known: true,
                inference_reason: null,
                expressions: [],
                display_class: "line_break"
              });
            }

            currentBlockTokens.push(...line);
            currentBlockIsDialogue = false;
            consecutiveEmptyLinesBefore = 0;
          }
        }
      }
      flushBlock();

      if (tempParagraphs.length === 0) {
        tempParagraphs.push({
          id: paragraphId++,
          tokens: [],
          isDialogue: false,
        });
      }

      paragraphs.value = tempParagraphs;
      frontendTiming.value = {
        listenerSetupMs: Math.round(listenerSetupMs),
        invokeAndTransferMs: Math.round(invokeAndTransferMs),
        paragraphBuildMs: Math.round(performance.now() - paragraphBuildStartedAt),
        totalBeforeRenderMs: Math.round(performance.now() - totalStartedAt),
        backendDurationMs: backendDurationMs,
        ipcAndParseMs: Math.max(0, Math.round(invokeAndTransferMs - backendDurationMs)),
      };
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

  async function addExpressionRule(
    tokens: AnnotatedToken[],
    label?: string,
    description?: string,
    bunsetsuStates: ('fixed' | 'slot' | 'any')[] = [],
    morphemeMasks: boolean[][] = [],
    gapAfter: number | null = null,
    expressionType: ExpressionType = "grammar_construction",
    priority = 50,
    boundaryEffect: ExpressionBoundaryEffect = "annotate_only"
  ) {
    return await invoke<ExpressionRule>("add_expression_rule", {
      tokens,
      label,
      description,
      bunsetsuStates,
      morphemeMasks,
      gapAfter,
      expressionType,
      priority,
      boundaryEffect,
    });
  }

  async function getExpressionRules() {
    return await invoke<ExpressionRule[]>("get_expression_rules");
  }

  async function refreshExpressionAnnotations(tokens: AnnotatedToken[]) {
    return await invoke<AnnotatedToken[]>("refresh_expression_annotations", { tokens });
  }

  async function deleteExpressionRule(id: number) {
    return await invoke<boolean>("delete_expression_rule", { id });
  }

  async function getCandidates(token: AnnotatedToken, topN = 5) {
    return await invoke<SegmentationCandidate[]>("get_candidates", { token, topN });
  }

  async function chooseSegmentation(source: AnnotatedToken, candidate: SegmentationCandidate) {
    await invoke("choose_segmentation", { source, candidate });
  }

  return {
    paragraphs,
    isAnalyzing,
    errorMsg,
    analysisProgress,
    frontendTiming,
    analyzeText,
    mergeTokens,
    addExpressionRule,
    getExpressionRules,
    refreshExpressionAnnotations,
    deleteExpressionRule,
    getCandidates,
    chooseSegmentation,
  };
}
