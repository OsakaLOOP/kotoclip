export interface Morpheme { surface: string; pos: PosTag; base_form: string; reading: string; conjugation_type: string; conjugation_form: string; char_range: [number, number]; }
export interface PosTag { major: string; sub1: string; sub2: string; sub3: string; }
export type GrammarOccurrenceKind = "morphology_feature" | "functional_morpheme" | "grammar_construction" | "bunsetsu_function" | "correlative_grammar" | "unknown";
export type GrammarOccurrenceStatus = "accepted" | "pending" | "rejected" | "unknown";
export interface GrammarSenseCandidate { sense_id: string; label: string; confidence: number; evidence: string[]; }
export interface GrammarCapture { name: string; surface: string; base_form: string; morpheme_range: [number, number]; char_range: [number, number]; }
export interface GrammarContentBlock { kind: string; label: string | null; text: string; }
export interface GrammarDictionaryTarget { label: string; base_form: string; reading: string; char_range: [number, number]; }
export type GrammarGenerationOrigin = "ai" | "human" | "builtin";
export type GrammarReviewStatus = "unverified" | "ai_checked" | "trusted";
export interface GrammarProvenance { origin: GrammarGenerationOrigin; author: string; date: string; version: string; }
export interface ResolvedGrammarExplanation {
  status: "resolved" | "partial" | "unavailable" | "error" | string;
  occurrence_summary: string;
  concept_id: string;
  title: string;
  compact_summary: string;
  function_summary: string;
  connection: string;
  actual_form: string;
  selected_sense: GrammarSenseCandidate | null;
  alternative_senses: GrammarSenseCandidate[];
  bound_captures: GrammarCapture[];
  morphology_chain: string[];
  content_blocks: GrammarContentBlock[];
  evidence: string[];
  related_concept_ids: string[];
  contrast_concept_ids: string[];
  dictionary_targets: GrammarDictionaryTarget[];
  source_refs: string[];
  provenance: GrammarProvenance;
  review_status: GrammarReviewStatus;
  available_depths: string[];
  content_version: number;
  audit_status: string;
}
export interface GrammarTag {
  pattern_id: string;
  name_ja: string;
  name_en: string;
  jlpt_level: number | null;
  description: string;
  morpheme_range: [number, number];
  char_range: [number, number];
  occurrence_id: string;
  concept_id: string;
  occurrence_kind: GrammarOccurrenceKind;
  status: GrammarOccurrenceStatus;
  show_badge: boolean;
  display_ranges: [number, number][];
  selected_sense_id: string | null;
  sense_candidates: GrammarSenseCandidate[];
  explanation: ResolvedGrammarExplanation | null;
}
export type MorphologyChainRole = "lexical" | "functional";
export interface MorphologyOperator {
  operator_id: string;
  kind: string;
  source_morpheme_range: [number, number];
  char_range: [number, number];
  output_state: string;
  concept_id: string;
  confidence: number;
  evidence: string[];
  candidates: string[];
  label: string;
  description: string;
}
export interface MorphologyChain {
  chain_id: string;
  anchor_morpheme: number;
  anchor_range: [number, number];
  morpheme_range: [number, number];
  char_range: [number, number];
  role: MorphologyChainRole;
  base_lexeme: string;
  surface_form: string;
  dictionary_form: string;
  lemma_form: string;
  lookup_form: string;
  source_ranges: [number, number][];
  operators: MorphologyOperator[];
  connection_forms: string[];
  evidence: string[];
}
export interface MorphologyArtifact { chains: MorphologyChain[]; }
export interface GrammarConcept {
  concept_id: string;
  kind: string;
  canonical_label: string;
  aliases: string[];
  semantic_domains: string[];
  function_tags: string[];
  jlpt_level: number | null;
  register: string[];
  related_concept_ids: string[];
  contrast_concept_ids: string[];
  default_explanation_id: string;
  source_refs: string[];
  audit_status: string;
  concept_version: number;
  enabled: boolean;
}
export interface GrammarSense {
  sense_id: string;
  concept_id: string;
  label: string;
  function_summary: string;
  semantic_features: Record<string, string>;
  context_requirements: string[];
  exclusion_conditions: string[];
  related_sense_ids: string[];
  contrast_sense_ids: string[];
  explanation_id: string;
  sense_version: number;
  audit_status: string;
}
export interface GrammarExplanationSourceBlock { kind: string; label: string | null; text: string; }
export interface GrammarExplanationDocument {
  explanation_id: string;
  concept_id: string;
  sense_id: string | null;
  language: string;
  title: string;
  compact_summary: string;
  function_summary: string;
  connection: string;
  formation: string;
  usage_notes: string[];
  semantic_constraints: string[];
  pragmatic_notes: string[];
  examples: string[];
  counter_examples: string[];
  source_refs: string[];
  authoring_status: string;
  content_version: number;
  provenance: GrammarProvenance;
  review_status: GrammarReviewStatus;
  body_blocks: GrammarExplanationSourceBlock[];
}
export interface GrammarConceptBundle {
  concept: GrammarConcept;
  senses: GrammarSense[];
  explanation: GrammarExplanationDocument;
  explanations: GrammarExplanationDocument[];
}
export interface WordFormationCapture { name: string; surface: string; morpheme_range: [number, number]; char_range: [number, number]; }
export interface WordFormationAnnotation { rule_id: string; category: string; surface: string; base_form: string; reading: string; output_pos: PosTag; morpheme_range: [number, number]; char_range: [number, number]; head_morpheme: number; captures: WordFormationCapture[]; confidence: number; }
export interface DictionaryEntryRef { entry_key: string; dict_name: string; headword: string; matched_form: string; match_type: "exact_form" | "headword"; readings: string[]; }
export interface DictionaryLexicalUnitAnnotation { surface: string; base_form: string; reading: string; output_pos: PosTag; morpheme_range: [number, number]; char_range: [number, number]; head_morpheme: number; lexical_shape: string; dictionary_refs: DictionaryEntryRef[]; reading_candidates: string[]; confidence: number; evidence: string[]; }
export type BunsetsuFunction = "predicate" | "case_phrase" | "adnominal" | "adverbial" | "conjunctive" | "nominal" | "standalone" | "unknown";
export interface BunsetsuFunctionAnnotation { function: BunsetsuFunction; confidence: number; evidence: string[]; syntax_evidence: string[]; }
export interface HeadWord { surface: string; base_form: string; reading: string; pos: PosTag; }
export interface Bunsetsu { morphemes: Morpheme[]; surface: string; head_word: HeadWord; grammar_tags: GrammarTag[]; morphology: MorphologyArtifact; word_formations: WordFormationAnnotation[]; lexical_units: DictionaryLexicalUnitAnnotation[]; function?: BunsetsuFunctionAnnotation | null; char_range: [number, number]; }
export type ExpressionType = "lexical_unit" | "idiom" | "grammar_construction" | "correlative";
export type ExpressionBoundaryEffect = "merge_lexical_unit" | "regroup_bunsetsu" | "annotate_only";
export interface ExpressionPatternPart { lemmas: string[]; pos: string[]; pos_details: PosTag[]; conjugation_types: string[]; conjugation_forms: string[]; surface_hint: string; is_slot: boolean; alignment?: "full" | "suffix" | "prefix"; is_any?: boolean; }
export interface ExpressionRule { id: number; schema_version: number; rule_version: number; enabled: boolean; requires_review: boolean; label: string; description: string; origin: string; expression_type: ExpressionType; priority: number; boundary_effect: ExpressionBoundaryEffect; parts: ExpressionPatternPart[]; created_at: string; gap_after?: number; gap_bunsetsu?: [number, number]; }
export interface ExpressionAnnotation { match_id: string; rule_id: number; label: string; description: string; origin: string; expression_type: ExpressionType; priority: number; boundary_effect: ExpressionBoundaryEffect; confidence: number; position: "start" | "middle" | "end" | "single"; token_range: [number, number]; char_range: [number, number]; matched_ranges: [number, number][]; surface: string; }
export interface ExpressionRulePreview { status: "accepted" | "pending" | "rejected"; expression_type: ExpressionType; surface: string; matched_ranges: [number, number][]; covered_token_range: [number, number]; evidence: string[]; counter_evidence: string[]; rejection_reason: string | null; }
export interface AnnotatedToken { bunsetsu: Bunsetsu; novelty_score: number; is_selected: boolean; is_known: boolean; inference_reason: string | null; expressions: ExpressionAnnotation[]; display_class: "content" | "punctuation" | "line_break"; }
export interface SegmentationCandidate { tokens: AnnotatedToken[]; total_cost: number; relative_cost: number; source: "vibrato_lattice"; vibrato_rank: number; rank_score: number; dictionary_evidence: string[]; }
export interface DictionaryLink { target: string; label: string; relation: "candidate" | "redirect" | "synonym" | "antonym" | "parent" | "child" | "phrase" | "reference" | "internal_reference" | "related"; }
export interface DictionaryContentBlock { kind: "rich_text" | "notice" | string; label: string | null; html: string; }
export interface DictionaryChoiceOption { key: string; label: string; active: boolean; preferred?: boolean; unavailable?: boolean; disabled?: boolean; title?: string; }
export interface DictionaryTag { kind: string; label: string; }
export interface DictionaryForm { form: string; reading?: string | null; kind: string; }
export interface DictionaryPronunciation { system: string; label: string; value: string; }
export interface DictionaryOccurrenceHeader { display_form: string; canonical_form?: string | null; reading?: string | null; historical_reading?: string | null; pronunciations: DictionaryPronunciation[]; scoped_forms: DictionaryForm[]; pos_tags: DictionaryTag[]; usage_tags: DictionaryTag[]; origin?: string | null; short_note?: string | null; }
export interface DictionaryText { lang: string; qualifier?: string | null; html: string; }
export interface DictionaryGlossClause { separator?: string | null; qualifier?: string | null; leading_tags: DictionaryTag[]; text: DictionaryText; trailing_tags: DictionaryTag[]; }
export interface DictionaryGlossGroup { heading?: string | null; clauses: DictionaryGlossClause[]; }
export interface DictionaryExample { source: DictionaryText; translation?: DictionaryText | null; note?: DictionaryText | null; }
export interface DictionarySense { sense_id: string; marker?: string | null; heading?: string | null; glosses: DictionaryText[]; gloss_groups: DictionaryGlossGroup[]; definitions: DictionaryText[]; tags: DictionaryTag[]; examples: DictionaryExample[]; notes: DictionaryText[]; relations: DictionaryLink[]; children: DictionarySense[]; }
export interface DictionarySectionItem { label?: string | null; label_html?: string | null; reading?: string | null; content: DictionaryText[]; tags: DictionaryTag[]; examples: DictionaryExample[]; senses: DictionarySense[]; relations: DictionaryLink[]; }
export interface DictionarySection { kind: string; label?: string | null; items: DictionarySectionItem[]; }
export interface DictionaryAdapterDiagnostics { coverage: string; warnings: string[]; omitted: string[]; }
export interface DictionaryMatchEvidence { kind: string; query_form: string; matched_form?: string | null; requested_reading?: string | null; reading_match: string; pos_match: string; dictionary_local: boolean; penalties: string[]; score: number; }
export interface DictEntry { entry_key: string; dict_name: string; headword: string; reading: string | null; is_preferred: boolean; definition_html: string; style_profile: string; content_blocks: DictionaryContentBlock[]; match_type: "exact_form" | "exact_headword" | "explicit_alias" | "compatibility_alias" | "reading_fallback" | "fuzzy" | string; links: DictionaryLink[]; occurrence_id: string; source_record_index: number; entry_kind: string; header: DictionaryOccurrenceHeader; senses: DictionarySense[]; sections: DictionarySection[]; adapter_diagnostics: DictionaryAdapterDiagnostics; match_evidence?: DictionaryMatchEvidence | null; raw_definition?: string | null; }
export interface DictionaryLookupTiming { resourceWaitMs: number; serviceMs: number; redirectMs: number; sqliteMs: number; definitionMs: number; presentationMs: number; definitionCacheHits: number; definitionCacheMisses: number; entries: number; }
export interface DictionaryFormAvailability { dictionary_name: string; available: boolean; }
export interface DictionaryFormVariant { surface_form: string; readings: string[]; evidence: string[]; score: number; dictionary_names: string[]; }
export interface DictionaryFormGroup { form_id: string; display_form: string; normalized_form: string; readings: string[]; evidence: string[]; score: number; variants: DictionaryFormVariant[]; dictionaries: DictionaryFormAvailability[]; }
export interface DictionaryLookup { query: string; observed_form?: string | null; reading: string | null; pos?: PosTag | null; selected_form_id?: string | null; mode: string; forms: DictionaryFormGroup[]; dictionary_names: string[]; entries: DictEntry[]; timing?: DictionaryLookupTiming; }
export interface DictionaryLookupRequest { word: string; observedForm?: string; reading?: string; background?: boolean; pos?: PosTag; selectedForm?: string; }
export interface DictionarySettings { available_dictionaries: string[]; default_dictionary: string | null; dictionary_order: string[]; }
export interface ExportEntry { surface: string; base_form: string; reading: string; pos: string; grammar_tags: string[]; jlpt_levels: number[]; context_sentence: string; context_highlight: [number, number]; definitions: DictEntry[]; user_note: string; char_range?: [number, number]; }
