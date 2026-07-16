import { invoke } from "@tauri-apps/api/core";
import type { GrammarConcept, GrammarConceptBundle } from "../types";

export interface GrammarCatalogQuery {
  query?: string;
  family?: string;
  jlptLevel?: number;
  auditStatus?: string;
  sourceRef?: string;
}

/** 主动浏览语法库时使用；正文浮层始终按 occurrence 精确解析。 */
export function searchGrammarCatalog(filters: GrammarCatalogQuery = {}) {
  return invoke<GrammarConcept[]>("search_grammar_catalog", {
    query: filters.query ?? null,
    family: filters.family ?? null,
    jlptLevel: filters.jlptLevel ?? null,
    auditStatus: filters.auditStatus ?? null,
    sourceRef: filters.sourceRef ?? null,
  });
}

/** 获取 concept、全部 sense 与默认讲解，不伪造正文 occurrence。 */
export function getGrammarConcept(conceptId: string) {
  return invoke<GrammarConceptBundle>("get_grammar_concept", { conceptId });
}
