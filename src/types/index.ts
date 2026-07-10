/// 形态素定义
export interface Morpheme {
  surface: string;
  pos: PosTag;
  base_form: string;
  reading: string;
  conjugation_type: string;
  conjugation_form: string;
  char_range: [number, number];
}

/// 词性标签定义
export interface PosTag {
  major: string;
  sub1: string;
  sub2: string;
  sub3: string;
}

/// 语法模式标签定义
export interface GrammarTag {
  pattern_id: string;
  name_ja: string;
  name_en: string;
  jlpt_level: number | null;
  description: string;
}

/// 核心自立语定义
export interface HeadWord {
  surface: string;
  base_form: string;
  reading: string;
  pos: PosTag;
}

/// 文节 (Bunsetsu) 胶囊定义
export interface Bunsetsu {
  morphemes: Morpheme[];
  surface: string;
  head_word: HeadWord;
  grammar_tags: GrammarTag[];
  char_range: [number, number];
}

/// 带用户画像信息的最终渲染 Token 条目
export interface AnnotatedToken {
  bunsetsu: Bunsetsu;
  novelty_score: number; // 0.0 (熟词) 到 1.0 (生词)
  is_selected: boolean;
  is_known: boolean;
  inference_reason: string | null;
}

/// 词典释义条目
export interface DictEntry {
  dict_name: string;
  headword: string;
  definition_html: string;
}

/// 导出所选词条的数据格式
export interface ExportEntry {
  surface: string;
  base_form: string;
  reading: string;
  pos: string;
  grammar_tags: string[];
  context_sentence: string;
  context_highlight: [number, number];
  definitions: DictEntry[];
}
