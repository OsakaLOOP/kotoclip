import type { DictEntry } from './index';

export type Register = 'cwj' | 'csj';
export interface FeatureField { index: number; name: string; label: string; raw: string | null; value: string | null; }
export interface ProviderToken {
  index: number; surface: string; char_range: [number, number]; byte_range: [number, number];
  lexicon_type: string; left_id: number; right_id: number; word_cost: number; total_cost: number;
  raw_feature: string; fields: FeatureField[];
}
export interface QueryForm { kind: string; form: string; reading: string | null; reading_field: string | null; }
export interface RubyValidation {
  base: string; ruby_reading: string; expected_reading: string; char_range: [number, number];
  original_char_range: [number, number];
  token_range: [number, number] | null; observed_reading: string | null;
  status: 'matched' | 'variant' | 'unavailable' | 'unmatched';
  reason: 'exact' | 'small_kana' | 'reading_difference' | 'missing_reading' | 'ambiguous_alignment' | 'source_gap';
}
export interface MorphemeToken {
  id: string; source_index: number; surface: string; char_range: [number, number];
  pos: (string | null)[]; lemma: string | null; reading: string | null; query_forms: QueryForm[];
}
export interface UnifiedDocument {
  schema: string; id: string; text: string; characters: number; elapsed_ms: number;
  source: { provider: { id: string; version: string; dictionary_sha256: string; field_schema: string }; tokens: ProviderToken[] };
  routing: { requested: Register; selected: Register; reason: string | null };
  ruby_validations: RubyValidation[];
  morphemes: MorphemeToken[]; gaps: { char_range: [number, number]; surface: string }[];
  structure: {
    schema: string; provider: string; provider_version: string | null;
    paragraphs: StructureSpan[];
    sentences: StructureSpan[];
    clauses: StructureSpan[];
    bunsetsu: StructureSpan[];
    compounds: StructureSpan[];
  };
  structure_diagnostics: AlignmentDiagnostic[];
  formation: {
    schema: string;
    nodes: { id: string; kind: string; char_range: [number, number]; morpheme_indices: number[]; status: string; evidence: { provider: string; source_id: string | null; reason: string }[] }[];
    conflicts: { id: string; node_ids: string[]; char_range: [number, number]; reason: string }[];
  };
  bunsetsu: {
    schema: string;
    nodes: {
      id: string; char_range: [number, number]; morpheme_indices: number[]; formation_node_ids: string[];
      status: string; provider: string; source_id: string | null; head_char_range: [number, number] | null;
      head_morpheme_index: number | null; labels: string[];
    }[];
    conflict_groups: { id: string; node_ids: string[]; char_range: [number, number]; providers: string[]; reason: string }[];
  };
  clause: {
    schema: string;
    sentences: { id: string; char_range: [number, number]; status: string; provider: string; source_id: string | null; labels: string[] }[];
    clauses: { id: string; char_range: [number, number]; sentence_ids: string[]; morpheme_indices: number[]; status: string; provider: string; source_id: string | null; labels: string[] }[];
    conflicts: { id: string; node_ids: string[]; char_range: [number, number]; providers: string[]; reason: string }[];
  };
  dictionary_candidates: {
    schema: string;
    candidates: {
      id: string; kind: 'token' | 'compound'; char_range: [number, number]; surface: string;
      morpheme_indices: number[]; query_forms: QueryForm[]; status: string; source_id: string;
    }[];
  };
  grammar: { schema: string; occurrences: GrammarOccurrence[] };
  expression: { schema: string; occurrences: ExpressionOccurrence[] };
  projection: { schema: string; targets: ProjectionTarget[] };
  morphology: MorphologyArtifact;
}
export interface GrammarOccurrence {
  id: string; concept_id: string | null; char_range: [number, number]; morpheme_indices: number[];
  status: 'observed' | 'candidate' | 'pending' | 'rejected'; provider: string; source_id: string | null;
  labels: string[]; evidence: string[];
}
export interface ExpressionOccurrence {
  id: string; rule_id: string | null; char_range: [number, number]; morpheme_indices: number[];
  status: 'observed' | 'candidate' | 'pending' | 'rejected'; provider: string; source_id: string | null;
  expression_type: string; labels: string[]; evidence: string[];
}
export interface ProjectionTarget { id: string; char_range: [number, number]; layer: string; source_id: string; status: string; }
export interface MorphologyOperator {
  operator_id: string; kind: string; source_morpheme_range: [number, number]; char_range: [number, number];
  output_state: string; concept_id: string; confidence: number; evidence: string[]; candidates: string[];
  label: string; description: string;
}
export interface MorphologyChain {
  chain_id: string; anchor_morpheme: number; anchor_range: [number, number]; morpheme_range: [number, number];
  char_range: [number, number]; role: 'lexical' | 'functional'; base_lexeme: string; surface_form: string;
  dictionary_form: string; lemma_form: string; lookup_form: string; source_ranges: [number, number][];
  operators: MorphologyOperator[]; connection_forms: string[]; evidence: string[];
}
export interface MorphologyArtifact { schema: string; chains: MorphologyChain[]; }
export interface StructureSpan {
  id: string; kind: string; char_range: [number, number]; status: string; provider: string;
  source_id?: string | null; head_char_range?: [number, number] | null; labels?: string[];
}
export interface AlignmentDiagnostic {
  provider_span_id: string; status: string; reason: string; matched_range: [number, number] | null;
}
export interface QueryOutput {
  analysis_id: string | null; token_id: string | null; dictionary_names: string[];
  groups: { form: QueryForm; entries: DictEntry[]; total: number }[];
}
