import { onUnmounted, ref, shallowRef } from 'vue';
import { nlpRequest } from '../services/nlp';
import { isSessionGenerationError, mergeSession, openSession, type DocumentPlan, type DocumentSession, type DocumentUpdate } from '../reader/session';
import type { RegisterPolicy } from '../types/nlp';

export function useDocumentSession() {
  const state = shallowRef<DocumentSession | null>(null);
  const error = ref('');
  const opening = ref(false);
  const controlling = ref(0);
  let operation = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let controls = Promise.resolve();
  let synchronization: { sessionId: string; promise: Promise<void> } | null = null;

  function synchronize(): Promise<void> {
    const current = state.value;
    if (!current) return Promise.resolve();
    if (synchronization?.sessionId === current.session_id) return synchronization.promise;
    const pending = {
      sessionId: current.session_id,
      promise: nlpRequest<DocumentUpdate>({ command: 'sync_document', session_id: current.session_id }).then((snapshot) => {
        if (state.value?.session_id === current.session_id) {
          state.value = mergeSession(state.value, snapshot);
          error.value = '';
        }
      }).catch((cause) => {
        if (state.value?.session_id === current.session_id) error.value = String(cause);
        throw cause;
      }).finally(() => { if (synchronization === pending) synchronization = null; }),
    };
    synchronization = pending;
    return pending.promise;
  }

  async function apply(update: DocumentUpdate) {
    const current = state.value;
    if (!current) return;
    const merged = mergeSession(current, update);
    if (merged) { state.value = merged; return; }
    await synchronize();
  }

  async function poll() {
    const current = state.value;
    const expected = operation;
    if (!current) return;
    try {
      const update = await nlpRequest<DocumentUpdate>({ command: 'poll_document', session_id: current.session_id,
        text_version: current.text_version, generation: current.generation, after_revision: current.revision });
      if (expected === operation) { await apply(update); error.value = ''; }
    } catch (e) {
      if (expected === operation && state.value?.session_id === current.session_id) {
        try { await synchronize(); }
        catch { return; }
      }
    }
    if (expected === operation && state.value) timer = setTimeout(poll, state.value.progress.pending ? 200 : 1000);
  }

  async function close() {
    operation++; clearTimeout(timer);
    opening.value = false;
    const current = state.value; state.value = null;
    if (current) await nlpRequest({ command: 'close_document', session_id: current.session_id });
  }

  async function open(text: string, policy: RegisterPolicy, documentId?: string, initialOffset = 0) {
    const expected = ++operation;
    clearTimeout(timer);
    const previous = state.value; state.value = null;
    opening.value = true; error.value = '';
    try {
      if (previous) await nlpRequest({ command: 'close_document', session_id: previous.session_id });
      if (expected !== operation) return;
      const value = await nlpRequest<{ plan: DocumentPlan; update: DocumentUpdate }>({ command: 'open_document', text, policy, document_id: documentId ?? null, initial_offset: initialOffset });
      if (expected !== operation) { await nlpRequest({ command: 'close_document', session_id: value.update.session_id }); return; }
      state.value = openSession(value.plan, value.update);
      void poll();
    } catch (e) { if (expected === operation) error.value = String(e); }
    finally { if (expected === operation) opening.value = false; }
  }

  function adopt(plan: DocumentPlan, update: DocumentUpdate) {
    operation++;
    clearTimeout(timer);
    error.value = '';
    state.value = openSession(plan, update);
    opening.value = false;
    void poll();
  }

  function control(command: string, payload: Record<string, unknown> = {}) {
    const sessionId = state.value?.session_id;
    if (!sessionId) return Promise.resolve();
    controlling.value++; operation++; clearTimeout(timer);
    controls = controls.then(async () => {
      async function send() {
        const current = state.value;
        if (!current || current.session_id !== sessionId) return;
        const update = await nlpRequest<DocumentUpdate>({ command, ...payload, session_id: sessionId, text_version: current.text_version, generation: current.generation, after_revision: current.revision });
        if (state.value?.session_id === sessionId) {
          await apply(update);
          error.value = '';
        }
      }
      try {
        try { await send(); }
        catch (cause) {
          if (!isSessionGenerationError(cause) || state.value?.session_id !== sessionId) throw cause;
          await synchronize();
          await send();
        }
      } catch (e) {
        if (state.value?.session_id === sessionId) error.value = String(e);
      }
    }).finally(() => { controlling.value--; if (!controlling.value && state.value) void poll(); });
    return controls;
  }

  onUnmounted(() => { void close(); });
  return { state, error, opening, controlling, open, adopt, close, control, synchronize };
}
