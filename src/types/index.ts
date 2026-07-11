export interface Morpheme { surface: string; pos: PosTag; base_form: string; reading: string; conjugation_type: string; conjugation_form: string; char_range: [number, number]; }
export interface PosTag { major: string; sub1: string; sub2: string; sub3: string; }
export interface GrammarTag { pattern_id: string; name_ja: string; name_en: string; jlpt_level: number | null; description: string; morpheme_range: [number, number]; char_range: [number, number]; }
export interface HeadWord { surface: string; base_form: string; reading: string; pos: PosTag; }
export interface Bunsetsu { morphemes: Morpheme[]; surface: string; head_word: HeadWord; grammar_tags: GrammarTag[]; char_range: [number, number]; }
export interface ExpressionPatternPart { lemmas: string[]; pos: string[]; surface_hint: string; is_slot: boolean; alignment?: "full" | "suffix" | "prefix"; is_any?: boolean; }
export interface ExpressionRule { id: number; label: string; description: string; origin: string; parts: ExpressionPatternPart[]; created_at: string; gap_after?: number; gap_bunsetsu?: [number, number]; }
export interface ExpressionAnnotation { match_id: string; rule_id: number; label: string; description: string; origin: string; position: "start" | "middle" | "end" | "single"; token_range: [number, number]; char_range: [number, number]; surface: string; }
export interface AnnotatedToken { bunsetsu: Bunsetsu; novelty_score: number; is_selected: boolean; is_known: boolean; inference_reason: string | null; expressions: ExpressionAnnotation[]; display_class: "content" | "punctuation" | "line_break"; }
export interface SegmentationCandidate { tokens: AnnotatedToken[]; total_cost: number; relative_cost: number; source: "vibrato_lattice"; vibrato_rank: number; rank_score: number; dictionary_evidence: string[]; }
export interface DictionaryLink { target: string; label: string; relation: "candidate" | "redirect" | "synonym" | "antonym" | "parent" | "child" | "phrase" | "reference" | "related"; }
export interface DictionaryContentBlock { kind: "rich_text" | "notice" | string; label: string | null; html: string; }
export interface DictEntry { entry_key: string; dict_name: string; headword: string; reading: string | null; is_preferred: boolean; definition_html: string; style_profile: string; content_blocks: DictionaryContentBlock[]; match_type: "headword" | "reading" | "fuzzy"; links: DictionaryLink[]; }
export interface DictionaryLookup { query: string; reading: string | null; selected_target: string | null; candidates: DictionaryLink[]; entries: DictEntry[]; }
export interface ExportEntry { surface: string; base_form: string; reading: string; pos: string; grammar_tags: string[]; jlpt_levels: number[]; context_sentence: string; context_highlight: [number, number]; definitions: DictEntry[]; user_note: string; char_range?: [number, number]; }
