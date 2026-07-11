export interface Morpheme { surface: string; pos: PosTag; base_form: string; reading: string; conjugation_type: string; conjugation_form: string; char_range: [number, number]; }
export interface PosTag { major: string; sub1: string; sub2: string; sub3: string; }
export interface GrammarTag { pattern_id: string; name_ja: string; name_en: string; jlpt_level: number | null; description: string; morpheme_range: [number, number]; char_range: [number, number]; }
export interface HeadWord { surface: string; base_form: string; reading: string; pos: PosTag; }
export interface Bunsetsu { morphemes: Morpheme[]; surface: string; head_word: HeadWord; grammar_tags: GrammarTag[]; char_range: [number, number]; }
export interface AnnotatedToken { bunsetsu: Bunsetsu; novelty_score: number; is_selected: boolean; is_known: boolean; inference_reason: string | null; }
export interface SegmentationCandidate { tokens: AnnotatedToken[]; }
export interface DictEntry { dict_name: string; headword: string; definition_html: string; match_type: "headword" | "reading" | "fuzzy"; }
export interface ExportEntry { surface: string; base_form: string; reading: string; pos: string; grammar_tags: string[]; jlpt_levels: number[]; context_sentence: string; context_highlight: [number, number]; definitions: DictEntry[]; user_note: string; char_range?: [number, number]; }
