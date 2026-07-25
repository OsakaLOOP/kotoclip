import type { AnnotatedToken } from "./index";

export interface QualityAuditHistorySide {
  label?: string | null;
  implementation?: {
    git_commit?: string | null;
    git_dirty?: boolean | null;
    git_subject?: string | null;
    git_author?: string | null;
    git_author_time?: string | null;
    git_committer_time?: string | null;
  };
  corpus?: {
    id?: string | null;
    selected_characters?: number | null;
    analysis_characters?: number | null;
  } | null;
}

export interface QualityAuditHistoryRecord {
  comparison_id: string;
  created_at?: string | null;
  adapter?: string;
  before: QualityAuditHistorySide;
  after: QualityAuditHistorySide;
  summary: Record<string, unknown> & {
    status?: string;
    changes?: number;
    evidence_changes?: number;
    root_changes?: number;
    churn_rate?: number;
    quality_conclusion?: string;
  };
  gate_status?: string | null;
  reading_diff?: QualityAuditArtifact | null;
  reading_index?: QualityAuditArtifact | null;
  reading_units?: QualityAuditArtifact | null;
  manifest?: QualityAuditArtifact | null;
  summary_artifact?: QualityAuditArtifact | null;
  diff?: QualityAuditArtifact | null;
  lifecycle?: QualityAuditArtifact | null;
  memory_profile?: QualityAuditArtifact | null;
}

export interface QualityAuditArtifact {
  url: string;
  bytes: number;
  sha256: string;
}

export interface QualityAuditHistory {
  schema_version: string;
  comparisons: QualityAuditHistoryRecord[];
}

export interface QualityReadingToken {
  surface: string;
  char_range: [number, number];
  head_word: AnnotatedToken["bunsetsu"]["head_word"];
  morphemes: AnnotatedToken["bunsetsu"]["morphemes"];
  morphology?: AnnotatedToken["bunsetsu"]["morphology"];
  word_formations?: AnnotatedToken["bunsetsu"]["word_formations"];
  lexical_units?: AnnotatedToken["bunsetsu"]["lexical_units"];
  grammar_tags?: AnnotatedToken["bunsetsu"]["grammar_tags"];
  function?: AnnotatedToken["bunsetsu"]["function"];
  expressions?: AnnotatedToken["expressions"];
  display_class?: AnnotatedToken["display_class"];
}

export interface QualityReadingSide {
  char_range: [number, number];
  text: string;
  tokens: QualityReadingToken[];
}

export interface QualityReadingUnit {
  unit_id: string;
  sentence_index: number;
  changed_range: [number, number];
  changed_ranges: [number, number][];
  primary_change_count: number;
  evidence_change_count: number;
  domains: Record<string, number>;
  stages: string[];
  change_ids?: string[];
  evidence_change_ids?: string[];
  before: QualityReadingSide;
  after: QualityReadingSide;
}

export interface QualityReadingIndexSide {
  char_range: [number, number];
  text: string;
}

export interface QualityReadingIndexUnit {
  unit_id: string;
  sentence_index: number;
  changed_range: [number, number];
  changed_ranges: [number, number][];
  primary_change_count: number;
  evidence_change_count: number;
  domains: Record<string, number>;
  stages: string[];
  before: QualityReadingIndexSide;
  after: QualityReadingIndexSide;
  offset: number;
  bytes: number;
}

export interface QualityReadingIndex {
  schema_version: string;
  reading_schema_version: string;
  unit_count: number;
  units: QualityReadingIndexUnit[];
}

export interface QualityReadingDiff {
  schema_version: string;
  unit_count: number;
  units: QualityReadingUnit[];
}

export interface QualityAuditComparison {
  comparisonId: string;
  manifest: Record<string, any>;
  summary: Record<string, any>;
  readingIndex: QualityReadingIndex;
  gate: Record<string, any> | null;
  lifecycle: Record<string, any> | null;
  memoryProfile: Record<string, any> | null;
}

export function annotatedTokenFromQuality(token: QualityReadingToken): AnnotatedToken {
  return {
    bunsetsu: {
      morphemes: token.morphemes ?? [],
      surface: token.surface,
      head_word: token.head_word,
      grammar_tags: token.grammar_tags ?? [],
      morphology: token.morphology ?? { chains: [] },
      word_formations: token.word_formations ?? [],
      lexical_units: token.lexical_units ?? [],
      function: token.function ?? null,
      char_range: token.char_range,
    },
    novelty_score: 0,
    is_selected: false,
    is_known: false,
    inference_reason: null,
    expressions: token.expressions ?? [],
    display_class: token.display_class ?? "content",
  };
}
