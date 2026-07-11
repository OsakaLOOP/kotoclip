use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Morpheme { pub surface: String, pub pos: PosTag, pub base_form: String, pub reading: String, pub conjugation_type: String, pub conjugation_form: String, pub char_range: (usize, usize) }
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct PosTag { pub major: String, pub sub1: String, pub sub2: String, pub sub3: String }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bunsetsu { pub morphemes: Vec<Morpheme>, pub surface: String, pub head_word: HeadWord, pub grammar_tags: Vec<GrammarTag>, pub char_range: (usize, usize) }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadWord { pub surface: String, pub base_form: String, pub reading: String, pub pos: PosTag }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrammarTag {
    pub pattern_id: String, pub name_ja: String, pub name_en: String, pub jlpt_level: Option<u8>,
    pub description: String, pub morpheme_range: (usize, usize), pub char_range: (usize, usize),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotatedToken { pub bunsetsu: Bunsetsu, pub novelty_score: f32, pub is_selected: bool, pub is_known: bool, pub inference_reason: Option<String> }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentationCandidate { pub tokens: Vec<AnnotatedToken> }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportEntry {
    pub surface: String, pub base_form: String, pub reading: String, pub pos: String, pub grammar_tags: Vec<String>,
    pub context_sentence: String, pub context_highlight: (usize, usize), pub definitions: Vec<DictEntry>,
    pub jlpt_levels: Vec<u8>, pub user_note: String,
    pub char_range: Option<(usize, usize)>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictEntry { pub dict_name: String, pub headword: String, pub definition_html: String, pub match_type: String }
