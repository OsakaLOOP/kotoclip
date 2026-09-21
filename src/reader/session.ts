import type { UnifiedDocument } from '../types/nlp';

export interface AnalysisUnit {
  id: string;
  anchor: { document_id: string; text_version: string; char_range: [number, number]; text_sha256: string };
  context_range: [number, number]; source_range: [number, number];
}
export interface DocumentPlan {
  schema: string; id: string; text_version: string;
  prepared: { text: string; annotations: UnifiedDocument['author_ruby']; mapping: UnifiedDocument['preparation'] };
  routing: UnifiedDocument['routing']; units: AnalysisUnit[];
}
export interface UnitUpdate {
  unit_id: string; stage: 'pending' | 'analyzing' | 'basic' | 'enriching' | 'complete' | 'failed';
  artifact_revision: number; document: UnifiedDocument | null;
  providers: { id: string; status: string; error?: string; cache_hit?: boolean }[]; error: string | null;
}
export interface DocumentUpdate {
  schema: string; session_id: string; document_id: string; text_version: string; generation: number;
  base_revision: number | null; revision: number; snapshot: boolean; paused: boolean;
  progress: { total: number; basic: number; complete: number; failed: number; pending: number };
  changes: UnitUpdate[];
}
export interface DocumentSession {
  plan: DocumentPlan; session_id: string; text_version: string; generation: number; revision: number;
  paused: boolean; progress: DocumentUpdate['progress']; units: Record<string, UnitUpdate>;
}

export function openSession(plan: DocumentPlan, update: DocumentUpdate): DocumentSession {
  return { plan, session_id: update.session_id, text_version: update.text_version, generation: update.generation,
    revision: update.revision, paused: update.paused, progress: update.progress,
    units: Object.fromEntries(update.changes.map(unit => [unit.unit_id, unit])) };
}

/** 返回 null 时请求完整快照；迟到响应保持现有对象。 */
export function mergeSession(state: DocumentSession, update: DocumentUpdate): DocumentSession | null {
  if (update.session_id !== state.session_id || update.text_version !== state.text_version || update.generation < state.generation) return state;
  if (update.revision < state.revision) return state;
  if (!update.snapshot && (update.generation !== state.generation || update.base_revision !== state.revision)) return null;
  return { ...state, generation: update.generation, revision: update.revision, paused: update.paused, progress: update.progress,
    units: { ...(update.snapshot ? {} : state.units), ...Object.fromEntries(update.changes.map(unit => [unit.unit_id, unit])) } };
}
