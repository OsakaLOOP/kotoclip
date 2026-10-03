import { invoke, isTauri } from '@tauri-apps/api/core';

export async function nlpRequest<T>(request: Record<string, unknown>): Promise<T> {
  const response: { result: T | null; error: string | null } = isTauri()
    ? await invoke('nlp_request', { request })
    : await fetch('/api/nlp', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(request) })
        .then(async response => {
          const value = await response.json();
          if (!response.ok) throw new Error(value.error || `服务错误 ${response.status}`);
          return value;
        });
  if (response.error || response.result === null) throw new Error(response.error || '服务未返回结果');
  return response.result;
}
