import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { AnnotatedToken, ExpressionBoundaryEffect, ExpressionRule, ExpressionRulePreview, ExpressionType, SegmentationCandidate } from "../types";

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
  | "expression_matching"
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

interface CompactAnalysis {
  s: string[];
  t: CompactToken[];
}

interface CompactAnalysisPatch extends CompactAnalysis {
  b: number;
}

interface AnalysisPatch {
  sessionId: string;
  baseRevision: number;
  revision: number;
  kind: "full_replace" | "range_replace" | "token_update";
  charRange: [number, number];
  removedTokenIds: string[];
  tokenIds: string[];
  orderedTokenIds: string[];
  analysis: CompactAnalysisPatch;
  fingerprint: {
    sessionSchemaVersion: number;
    pipelineArtifactVersion: number;
  };
  invalidation?: {
    reason: string;
    stages: string[];
    stageRanges: { stage: string; charRanges: [number, number][] }[];
    charRanges: [number, number][];
    recomputedCharacters: number;
    totalCharacters: number;
  };
}

interface DocumentResponse {
  patch: AnalysisPatch;
  backendDurationMs: number;
}

interface CompactToken {
  b: CompactBunsetsu;
  n: number;
  k: boolean;
  r?: number;
  x?: CompactExpression[];
  d: number;
}

interface CompactBunsetsu {
  m: CompactMorpheme[];
  s: number;
  h: CompactHeadWord;
  g?: CompactGrammarTag[];
  w?: CompactWordFormation[];
  v?: CompactLexicalUnit[];
  u?: CompactBunsetsuFunction;
  c: [number, number];
}

interface CompactMorpheme {
  s: number;
  p: [number, number, number, number];
  b: number;
  r: number;
  t: number;
  f: number;
  c: [number, number];
}

interface CompactHeadWord { s: number; b: number; r: number; p: [number, number, number, number]; }
interface CompactGrammarTag { i: number; j: number; e: number; l?: number; d: number; m: [number, number]; c: [number, number]; }
interface CompactWordFormation { i: number; k: number; s: number; b: number; r: number; o: [number, number, number, number]; m: [number, number]; c: [number, number]; h: number; p?: CompactWordFormationCapture[]; q: number; }
interface CompactWordFormationCapture { n: number; s: number; m: [number, number]; c: [number, number]; }
interface CompactDictionaryEntryRef { k: number; d: number; h: number; f: number; m: number; r: number[]; }
interface CompactLexicalUnit { s: number; b: number; r: number; o: [number, number, number, number]; m: [number, number]; c: [number, number]; h: number; k: number; d: CompactDictionaryEntryRef[]; a: number[]; q: number; e: number[]; }
interface CompactBunsetsuFunction { f: number; c: number; e: number[]; }
interface CompactExpression { m: number; i: number; l: number; d: number; o: number; t: number; p: number; b: number; c: number; q: number; r: [number, number]; a: [number, number]; z?: [number, number][]; s: number; }

/** 将热路径的字符串表 IPC 模型恢复为现有组件使用的 AnnotatedToken。 */
function decodeAnalysis(analysis: CompactAnalysis): AnnotatedToken[] {
  const stringAt = (index: number) => analysis.s[index] ?? "";
  const position = (indices: [number, number, number, number]) => ({
    major: stringAt(indices[0]), sub1: stringAt(indices[1]), sub2: stringAt(indices[2]), sub3: stringAt(indices[3]),
  });
  return analysis.t.map((token) => ({
    bunsetsu: {
      morphemes: token.b.m.map((morpheme) => ({
        surface: stringAt(morpheme.s), pos: position(morpheme.p), base_form: stringAt(morpheme.b),
        reading: stringAt(morpheme.r), conjugation_type: stringAt(morpheme.t), conjugation_form: stringAt(morpheme.f),
        char_range: morpheme.c,
      })),
      surface: stringAt(token.b.s),
      head_word: {
        surface: stringAt(token.b.h.s), base_form: stringAt(token.b.h.b), reading: stringAt(token.b.h.r), pos: position(token.b.h.p),
      },
      grammar_tags: (token.b.g ?? []).map((tag) => ({
        pattern_id: stringAt(tag.i), name_ja: stringAt(tag.j), name_en: stringAt(tag.e), jlpt_level: tag.l ?? null,
        description: stringAt(tag.d), morpheme_range: tag.m, char_range: tag.c,
      })),
      word_formations: (token.b.w ?? []).map((formation) => ({
        rule_id: stringAt(formation.i), category: stringAt(formation.k), surface: stringAt(formation.s),
        base_form: stringAt(formation.b), reading: stringAt(formation.r), output_pos: position(formation.o), morpheme_range: formation.m,
        char_range: formation.c, head_morpheme: formation.h, confidence: formation.q,
        captures: (formation.p ?? []).map((capture) => ({
          name: stringAt(capture.n), surface: stringAt(capture.s), morpheme_range: capture.m, char_range: capture.c,
        })),
      })),
      lexical_units: (token.b.v ?? []).map((unit) => ({
        surface: stringAt(unit.s), base_form: stringAt(unit.b), reading: stringAt(unit.r),
        output_pos: position(unit.o), morpheme_range: unit.m, char_range: unit.c,
        head_morpheme: unit.h, lexical_shape: stringAt(unit.k),
        dictionary_refs: unit.d.map((reference) => ({
          entry_key: stringAt(reference.k), dict_name: stringAt(reference.d),
          headword: stringAt(reference.h), matched_form: stringAt(reference.f),
          match_type: stringAt(reference.m) as "exact_form" | "headword",
          readings: reference.r.map(stringAt),
        })),
        reading_candidates: unit.a.map(stringAt), confidence: unit.q, evidence: unit.e.map(stringAt),
      })),
      function: token.b.u === undefined ? null : {
        function: stringAt(token.b.u.f) as import("../types").BunsetsuFunction,
        confidence: token.b.u.c,
        evidence: token.b.u.e.map(stringAt),
        syntax_evidence: [],
      },
      char_range: token.b.c,
    },
    novelty_score: token.n,
    is_selected: false,
    is_known: token.k,
    inference_reason: token.r === undefined ? null : stringAt(token.r),
    expressions: (token.x ?? []).map((expression) => ({
      match_id: stringAt(expression.m), rule_id: expression.i, label: stringAt(expression.l), description: stringAt(expression.d),
      origin: stringAt(expression.o), expression_type: stringAt(expression.t) as ExpressionType,
      priority: expression.p, boundary_effect: stringAt(expression.b) as ExpressionBoundaryEffect,
      confidence: expression.c, position: stringAt(expression.q) as "start" | "middle" | "end" | "single",
      token_range: expression.r, char_range: expression.a, matched_ranges: expression.z ?? [expression.a], surface: stringAt(expression.s),
    })),
    display_class: stringAt(token.d) as "content" | "punctuation" | "line_break",
  }));
}

const initialProgress = (): AnalysisProgress => ({
  requestId: "",
  phase: "preparing",
  completed: 0,
  total: 0,
  percent: 0,
  message: "等待分析",
});

function buildParagraphs(allTokens: AnnotatedToken[]): Paragraph[] {
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

  for (let index = 1; index < lines.length; index++) {
    const line = lines[index];
    let punctuationCount = 0;
    for (const token of line) {
      if (token.display_class !== "punctuation") break;
      const surface = token.bunsetsu.surface.trim();
      if (surface.startsWith("「") || surface.startsWith("『")) break;
      punctuationCount++;
    }
    if (punctuationCount > 0) {
      lines[index - 1].push(...line.splice(0, punctuationCount));
    }
  }

  const result: Paragraph[] = [];
  let blockTokens: AnnotatedToken[] = [];
  let blockIsDialogue = false;
  let paragraphId = 0;
  const isBlankToken = (token: AnnotatedToken) => /^\s+$/.test(token.bunsetsu.surface);
  const trimTokens = (tokens: AnnotatedToken[]) => {
    let start = 0;
    let end = tokens.length;
    while (start < end && isBlankToken(tokens[start])) start++;
    while (end > start && isBlankToken(tokens[end - 1])) end--;
    return tokens.slice(start, end);
  };
  const flush = () => {
    const tokens = trimTokens(blockTokens);
    if (tokens.length > 0) {
      result.push({ id: paragraphId++, tokens, isDialogue: blockIsDialogue });
      blockTokens = [];
    }
  };
  let emptyLines = 0;
  for (const line of lines) {
    const isEmpty = line.every((token) => /^\s*$/.test(token.bunsetsu.surface));
    if (isEmpty) {
      emptyLines++;
      flush();
      continue;
    }
    const first = line.find((token) => token.bunsetsu.surface.trim().length > 0);
    const text = first?.bunsetsu.surface.trim() ?? "";
    const isDialogue = text.startsWith("「") || text.startsWith("『");
    if (isDialogue) {
      flush();
      const tokens = trimTokens(line);
      if (tokens.length > 0) result.push({ id: paragraphId++, tokens, isDialogue: true });
      emptyLines = 0;
      continue;
    }
    if (blockIsDialogue || emptyLines > 0) flush();
    if (blockTokens.length > 0) {
      blockTokens.push({
        bunsetsu: {
          morphemes: [], surface: "\n",
          head_word: { surface: "\n", base_form: "\n", reading: "", pos: { major: "改行", sub1: "*", sub2: "*", sub3: "*" } },
          grammar_tags: [], word_formations: [], lexical_units: [], char_range: [0, 0],
        },
        novelty_score: 0, is_selected: false, is_known: true, inference_reason: null,
        expressions: [], display_class: "line_break",
      });
    }
    blockTokens.push(...line);
    blockIsDialogue = false;
    emptyLines = 0;
  }
  flush();
  if (result.length === 0) result.push({ id: 0, tokens: [], isDialogue: false });
  return result;
}

export function useTokenization() {
  const paragraphs = ref<Paragraph[]>([]);
  const isAnalyzing = ref(false);
  const errorMsg = ref<string | null>(null);
  const analysisProgress = ref<AnalysisProgress>(initialProgress());
  const activeRequestId = ref<string | null>(null);
  const frontendTiming = ref<FrontendAnalysisTiming | null>(null);
  const activeSessionId = ref<string | null>(null);
  const documentRevision = ref(0);
  const tokenCache = new Map<string, AnnotatedToken>();
  const sessionStrings: string[] = [];

  function mergePatch(patch: AnalysisPatch) {
    const openingNewSession = activeSessionId.value !== patch.sessionId;
    if (!openingNewSession && patch.baseRevision !== documentRevision.value) {
      throw new Error(`忽略过期文档 Patch：当前 ${documentRevision.value}，收到 ${patch.baseRevision}`);
    }
    if (openingNewSession) {
      tokenCache.clear();
      sessionStrings.length = 0;
    }
    if (patch.analysis.b !== sessionStrings.length) {
      throw new Error(`文档字符串表基址不匹配：当前 ${sessionStrings.length}，收到 ${patch.analysis.b}`);
    }
    sessionStrings.push(...patch.analysis.s);
    for (const tokenId of patch.removedTokenIds) tokenCache.delete(tokenId);
    const decoded = decodeAnalysis({ s: sessionStrings, t: patch.analysis.t });
    if (decoded.length !== patch.tokenIds.length) {
      throw new Error("文档 Patch 的 Token ID 与负载数量不一致");
    }
    decoded.forEach((token, index) => {
      const tokenId = patch.tokenIds[index];
      const existing = tokenCache.get(tokenId);
      if (existing) Object.assign(existing, token);
      else tokenCache.set(tokenId, token);
    });
    const ordered = patch.orderedTokenIds.map((tokenId) => tokenCache.get(tokenId));
    if (ordered.some((token) => token === undefined)) {
      throw new Error("文档 Patch 缺少有序 Token 所需的数据");
    }
    activeSessionId.value = patch.sessionId;
    documentRevision.value = patch.revision;
    return ordered as AnnotatedToken[];
  }

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
      // 创建后端规范文档会话；前端只应用带 revision 的 Patch。
      const invokeStartedAt = performance.now();
      const previousSessionId = activeSessionId.value;
      const response = await invoke<DocumentResponse>("open_document", {
        text,
        recordExposure,
        requestId,
      });
      const allTokens = mergePatch(response.patch);
      const backendDurationMs = response.backendDurationMs;
      invokeAndTransferMs = performance.now() - invokeStartedAt;
      if (activeRequestId.value !== requestId) return false;
      if (previousSessionId && previousSessionId !== activeSessionId.value) {
        void invoke("close_document", { sessionId: previousSessionId });
      }

      const paragraphBuildStartedAt = performance.now();
      paragraphs.value = buildParagraphs(allTokens);
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

  async function requestDocumentRange(charRange: [number, number]) {
    if (!activeSessionId.value) throw new Error("尚未打开文档会话");
    const patch = await invoke<AnalysisPatch>("request_document_range", {
      sessionId: activeSessionId.value,
      baseRevision: documentRevision.value,
      charRange,
    });
    paragraphs.value = buildParagraphs(mergePatch(patch));
    return patch;
  }

  async function replaceDocumentText(text: string, recordExposure = false) {
    if (!activeSessionId.value) return analyzeText(text, recordExposure);
    const response = await invoke<DocumentResponse>("apply_document_mutation", {
      sessionId: activeSessionId.value,
      baseRevision: documentRevision.value,
      mutation: { type: "replace_text", text, recordExposure },
    });
    paragraphs.value = buildParagraphs(mergePatch(response.patch));
    return true;
  }

  async function refreshDocumentExpressions() {
    if (!activeSessionId.value) throw new Error("尚未打开文档会话");
    const patch = await invoke<AnalysisPatch>("refresh_document_expressions", {
      sessionId: activeSessionId.value,
      baseRevision: documentRevision.value,
    });
    paragraphs.value = buildParagraphs(mergePatch(patch));
    return patch;
  }

  async function markDocumentKnown(baseForm: string, reading: string, known: boolean) {
    if (!activeSessionId.value) {
      await invoke(known ? "mark_known" : "mark_unknown", { baseForm, reading });
      return null;
    }
    const patch = await invoke<AnalysisPatch>("mark_document_known", {
      sessionId: activeSessionId.value,
      baseRevision: documentRevision.value,
      baseForm,
      reading,
      known,
    });
    paragraphs.value = buildParagraphs(mergePatch(patch));
    return patch;
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

  async function previewExpressionRule(
    tokens: AnnotatedToken[],
    bunsetsuStates: ('fixed' | 'slot' | 'any')[],
    morphemeMasks: boolean[][],
    gapAfter: number | null,
    expressionType: ExpressionType,
  ) {
    return await invoke<ExpressionRulePreview>("preview_expression_rule", {
      tokens,
      bunsetsuStates,
      morphemeMasks,
      gapAfter,
      expressionType,
      boundaryEffect: "annotate_only",
    });
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
    if (!activeSessionId.value) {
      await invoke("choose_segmentation", { source, candidate });
      return;
    }
    const patch = await invoke<AnalysisPatch>("choose_document_segmentation", {
      sessionId: activeSessionId.value,
      baseRevision: documentRevision.value,
      source,
      candidate,
    });
    paragraphs.value = buildParagraphs(mergePatch(patch));
  }

  return {
    paragraphs,
    isAnalyzing,
    errorMsg,
    analysisProgress,
    frontendTiming,
    activeSessionId,
    documentRevision,
    analyzeText,
    requestDocumentRange,
    replaceDocumentText,
    refreshDocumentExpressions,
    markDocumentKnown,
    mergeTokens,
    addExpressionRule,
    getExpressionRules,
    previewExpressionRule,
    refreshExpressionAnnotations,
    deleteExpressionRule,
    getCandidates,
    chooseSegmentation,
  };
}
