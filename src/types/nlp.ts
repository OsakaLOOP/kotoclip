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
  token_range: [number, number] | null; observed_reading: string | null; status: string;
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
}
export interface QueryOutput {
  analysis_id: string | null; token_id: string | null; dictionary_names: string[];
  groups: { form: QueryForm; entries: DictEntry[]; total: number }[];
}
