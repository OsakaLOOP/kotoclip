import type { DictEntry } from './index';

export type Register = 'cwj' | 'csj';
export type RegisterPolicy = 'auto' | Register;
export interface RegisterRun { char_range: [number, number]; selected: Register; reason: string; }
export interface SourceRun { provider: { id: string; version: string; dictionary_sha256: string; field_schema: string }; char_range: [number, number]; token_range: [number, number]; }
export interface FeatureField { index: number; name: string; label: string; raw: string | null; value: string | null; }
export interface ProviderToken {
  index: number; surface: string; char_range: [number, number]; byte_range: [number, number];
  lexicon_type: string; left_id: number; right_id: number; word_cost: number; total_cost: number;
  raw_feature: string; fields: FeatureField[];
}
export interface QueryForm { kind: string; form: string; reading: string | null; reading_field: string | null; }
export interface RubyValidation {
  base: string; ruby_reading: string; expected_reading: string; char_range: [number, number];
  candidate_char_range: [number, number]; source_char_ranges: [number, number][];
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
  preparation: { schema: string; source_text: string; source_sha256: string; text_sha256: string; origins: number[]; removed: { source_range: [number, number]; text_offset: number; kind: string }[] };
  author_ruby: { base: string; reading: string; char_range: [number, number] }[];
  source: { runs: SourceRun[]; tokens: ProviderToken[] };
  routing: { version: string; requested: RegisterPolicy; selected: Register | null; runs: RegisterRun[] };
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
    nodes: { id: string; kind: string; char_range: [number, number]; morpheme_indices: number[]; status: string; evidence: { provider: string; source_id: string | null; reason: string }[]; word: FormationWord | null }[];
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
      id: string; kind: 'token' | 'compound' | 'morphology'; char_range: [number, number]; surface: string;
      morpheme_indices: number[]; query_forms: QueryForm[]; status: string; source_id: string;
    }[];
  };
  grammar: { schema: string; occurrences: GrammarOccurrence[] };
  expression: { schema: string; occurrences: ExpressionOccurrence[] };
  projection: { schema: string; targets: ProjectionTarget[] };
  morphology: MorphologyArtifact;
  application: ApplicationArtifact;
  external_sources: SourceArtifact[];
  structure_graph: StructureGraph;
  stage_timings: { stage: string; elapsed_ms: number }[];
  provider_token_alignments: { schema: string; source_id: string | null; unidic_provider: string; external_provider: string; groups: AlignmentGroup[]; alignments: TokenAlignment[] }[];
}

export interface TokenAlignment {
  provider: string; provider_token_id: string; provider_char_range: [number, number]; provider_surface: string | null;
  status: 'exact' | 'compound' | 'partial' | 'unmatched'; morpheme_indices: number[]; morpheme_ids: string[]; reason: string;
}
export interface AlignmentGroup {
  id: string; cardinality: '1:1' | '1:n' | 'n:1' | 'n:m' | 'unmatched'; status: 'complete' | 'partial' | 'unmatched';
  provider_tokens: { id: string; text_ranges: [number, number][]; source_ranges: [number, number][] | null }[];
  morpheme_ids: string[]; char_range: [number, number];
  intersections: { provider_token_id: string; morpheme_id: string; char_range: [number, number] }[];
  provider_gaps: [number, number][]; morpheme_gaps: [number, number][]; reason: string;
}
export interface SpanCoverage { morpheme_indices: number[]; intersections: [number, number][]; gaps: [number, number][]; complete: boolean; reason: string; }
export interface MappedEntity {
  id: string; provider: string; source_id: string; kind: SourceNode['kind']; text_ranges: [number, number][]; source_ranges: [number, number][];
  surface: string; coverage: SpanCoverage[]; alignment_group_ids: string[]; morpheme_ids: string[]; members: string[];
  head: string | null; head_morpheme_ids: string[]; complete: boolean; diagnostics: string[]; features: Record<string, unknown>;
}
export type MappedEndpoint = { kind: 'entity'; id: string } | { kind: 'root' } | { kind: 'exophora'; label: string };
export interface StructureGraph {
  schema: string; document_id: string; selection_version: string; entities: MappedEntity[];
  relations: { id: string; provider: string; source_id: string; kind: SourceRelation['kind']; source: MappedEndpoint; target: MappedEndpoint; label: string; complete: boolean; diagnostics: string[]; features: Record<string, unknown> }[];
  candidates: { id: string; kind: SourceNode['kind']; text_ranges: [number, number][]; evidence: string[]; preferred_entity: string | null; selected: boolean; reason: string; competing_ids: string[] }[];
}

export interface ProviderConfig { python: string; model: string; enabled: boolean; timeout_seconds: number; dictionary: string; }
export interface ProviderSettings { ginza: ProviderConfig; kwja: ProviderConfig; kwja_cache: string; hf_cache: string; }
export interface ProviderManifest {
  id: string; version: string; model: string; model_version: string; versions: Record<string, string>; tasks: string[]; capabilities: string[]; coordinate_system: string;
  resources: { role: string; name: string; path: string | null; sha256: string; bytes: number }[];
  resource_digest: string; execution: Record<string, unknown>;
}
export interface ProviderCheck { id: string; status: string; error?: string; manifest?: ProviderManifest; pid?: number; }
export interface SourceNode {
  id: string; kind: 'token' | 'compound' | 'bunsetsu' | 'basic_phrase' | 'sentence' | 'clause' | 'predicate' | 'entity';
  source_ranges: [number, number][]; source_surface: string; text_ranges: [number, number][]; surface: string;
  head: string | null; members: string[]; features: Record<string, unknown>;
}
export type SourceEndpoint = { kind: 'node'; id: string } | { kind: 'root' } | { kind: 'exophora'; label: string };
export interface SourceRelation {
  id: string; kind: 'dependency' | 'predicate_argument' | 'coreference' | 'bridging' | 'discourse';
  source: SourceEndpoint; target: SourceEndpoint; label: string; features: Record<string, unknown>;
}
export interface SourceArtifact {
  schema: string; provider: ProviderManifest;
  text_sha256: string; text_characters: number; normalized_text: string; normalization_map: [number, number][];
  deleted_ranges: [number, number][]; nodes: SourceNode[]; relations: SourceRelation[]; diagnostics: string[];
  raw: string | null; elapsed_ms: number;
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
  input_state: string; output_state: string; concept_id: string; confidence: number; evidence: string[]; candidates: string[];
  label: string; description: string;
  state_before: MorphologyState; state_after: MorphologyState;
  normalized_form: string | null;
}
export interface SourceEvidence { provider: string; node_id: string | null; relation_id: string | null; reason: string; }
export interface MorphologyState {
  category: string; form: 'stem' | 'irrealis' | 'continuative' | 'terminal' | 'attributive' | 'conditional' | 'imperative' | 'volitional' | 'te' | 'other';
  conjugation_type: string; conjugation_form: string;
}
export interface MorphologyOccurrence {
  id: string; chain_id: string; operator_ids: string[]; kind: string; char_range: [number, number]; context_range: [number, number];
  morpheme_indices: number[]; candidates: string[]; status: string; source_evidence: SourceEvidence[];
  hit_ranges: [number, number][];
}
export interface FormationWord {
  surface: string; head_morpheme: number | null; output_pos: (string | null)[]; core_morpheme_indices: number[];
  chain_ids: string[]; source_token_ids: string[]; source_relation_ids: string[]; component_candidate_ids: string[];
  query_forms: QueryForm[]; dictionary_status: string; reason: string;
}
export interface MorphologyChain {
  chain_id: string; anchor_morpheme: number; anchor_range: [number, number]; morpheme_range: [number, number];
  char_range: [number, number]; role: 'lexical' | 'functional'; base_lexeme: string; surface_form: string;
  dictionary_form: string; lemma_form: string; lookup_form: string; display_form: string; parent_chain_id: string | null; source_ranges: [number, number][];
  operators: MorphologyOperator[]; connection_forms: string[]; evidence: string[];
  morpheme_indices: number[]; core_morpheme_indices: number[]; final_state: MorphologyState;
  query_forms: QueryForm[]; source_evidence: SourceEvidence[]; status: string;
}
export interface MorphologyArtifact { schema: string; chains: MorphologyChain[]; occurrences: MorphologyOccurrence[]; diagnostics: string[]; }
export interface DictionaryBinding { dictionary: string; entry_key: string; occurrence_id: string; headword: string; reading: string; }
export interface LexicalDecision {
  id: string; char_range: [number, number]; members: number[]; candidate_ids: string[]; status: string; reason: string;
  bindings: DictionaryBinding[]; competing_ids: string[];
}
export interface ReadingUnit {
  id: string; char_range: [number, number]; members: number[]; lexical_id: string | null; chain_ids: string[];
  bunsetsu_ids: string[]; query_target_ids: string[];
}
export interface LanguageExplanation {
  id: string; layer: string; source_id: string; char_range: [number, number]; hit_ranges: [number, number][]; members: number[];
  captures: Record<string, number[]>; chain_ids: string[]; concept_id: string | null; sense_id: string | null;
  sense_candidates: string[]; status: string; reason: string; title: string; summary: string; evidence: string[];
}
export interface ApplicationArtifact { version: string; rules_version: number; lexical: LexicalDecision[]; reading_units: ReadingUnit[]; explanations: LanguageExplanation[]; }
export interface RuleAtom {
  surfaces: string[]; base_forms: string[]; pos_major: string[]; pos_sub1: string[]; conjugation_types: string[];
  conjugation_forms: string[]; morphology_features: string[]; capture: string | null; optional: boolean; gap_before: number;
}
export interface LanguageRule {
  id: string; label: string; description: string; kind: 'idiom' | 'grammar_construction' | 'correlative' | 'lexical_unit' | 'functional_morpheme' | 'morphology_feature';
  atoms: RuleAtom[]; priority: number; enabled: boolean; document_id: string | null; allow_whitespace: boolean;
  concept_id: string | null; sense_ids: string[]; display_from: number; display_to: number | null; source_refs: string[];
  gap_after_atom: number | null; gap_bunsetsu: [number, number] | null;
}
export interface LanguageRuleMatch { rule_id: string; members: number[]; char_range: [number, number]; hit_ranges: [number, number][]; captures: Record<string, number[]>; }
export interface RuleSnapshot { schema: string; version: number; rules: LanguageRule[]; }
export interface StructureSpan {
  id: string; kind: string; char_range: [number, number]; status: string; provider: string;
  source_id?: string | null; head_char_range?: [number, number] | null; labels?: string[];
}
export interface AlignmentDiagnostic {
  provider_span_id: string; status: string; reason: string; matched_range: [number, number] | null;
}
export interface QueryOutput {
  analysis_id: string | null; token_id: string | null; dictionary_names: string[];
  groups: { form: QueryForm; form_id?: string | null; entries: DictEntry[]; total: number }[];
  forms: { form_id: string; display_form: string; normalized_form: string; readings: string[]; evidence: string[]; score: number; variants: { surface_form: string; readings: string[]; evidence: string[]; score: number; dictionary_names: string[] }[]; dictionaries: { dictionary_name: string; available: boolean }[] }[];
  selected_form_id?: string | null; mode?: string; observed_form?: string | null; reading?: string | null;
}
