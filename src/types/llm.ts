export type DictionaryAssistantTask = "select_dictionary_definition";

export interface DictionaryAssistantAuthorizationRequest {
  task: DictionaryAssistantTask;
  reason: string;
  endpointOrigin: string;
  model: string;
  dataSummary: {
    sendsSentenceContext: boolean;
    candidateCount: number;
    resolvedNavigationCandidateCount: number;
    includesDocumentTitle: boolean;
    includesUserSelection: boolean;
  };
}

export interface DictionaryAssistantGrant {
  grantId: string;
  task: DictionaryAssistantTask;
  scope: "once" | "session";
  endpointOrigin: string;
  expiresAt: string | null;
}

export interface DictionaryAssistantRequest {
  schema_version: "kotoclip.dictionary-disambiguation.v1";
  request_id: string;
  task: DictionaryAssistantTask;
  context: Record<string, unknown>;
  analysis: Record<string, unknown>;
  candidates: DictionaryAssistantCandidate[];
  candidate_set_truncated: boolean;
  omitted_candidate_count: number;
  instructions: Record<string, boolean>;
}

export interface DictionaryAssistantCandidate {
  candidate_id: string;
  ordinal: number;
  dict_name: string;
  headword: string;
  reading: string | null;
  match_type: string;
  deterministic_preferred: boolean;
  content_markdown: string;
  content_truncated: boolean;
  navigation_targets: Array<{
    relation: string;
    label: string;
    target: string;
    resolution: "resolved" | "unresolved";
    resolved_candidates: Array<{
      candidate_id: string;
      dict_name: string;
      headword: string;
      reading: string | null;
      content_markdown: string;
      content_truncated: boolean;
    }>;
  }>;
}

export interface DictionaryAssistantDecision {
  schema_version: "kotoclip.dictionary-disambiguation.v1";
  request_id: string;
  status: "selected" | "ambiguous" | "insufficient_evidence";
  selected_candidate_id: string | null;
  ranked_candidate_ids: string[];
  confidence: number;
  needs_user_review: boolean;
  summary: string;
  ambiguity_reason: string | null;
  evidence: Array<{
    candidate_id: string;
    context_quote: string;
    definition_quote: string;
    supports: string;
  }>;
  navigation_recommendation: {
    from_candidate_id: string;
    relation: string;
    target: string;
    target_candidate_id: string;
    context_quote: string;
    target_content_quote: string;
    reason: string;
  } | null;
}

/**
 * UI 未来接入 LLM 时必须注入此端口。当前没有默认实现、按钮或自动调用。
 * 授权由 UI 发起，grant 只能用于同 task 与同 endpoint origin。
 */
export interface DictionaryAssistantPort {
  requestAuthorization(
    request: DictionaryAssistantAuthorizationRequest,
  ): Promise<DictionaryAssistantGrant | null>;
  invoke(
    request: DictionaryAssistantRequest,
    grant: DictionaryAssistantGrant,
  ): Promise<DictionaryAssistantDecision>;
  revoke(grantId: string): Promise<void>;
}
