import { invoke, isTauri } from '@tauri-apps/api/core';

export async function readerRequest<T>(command: string, payload: Record<string, unknown> = {}): Promise<T> {
  if (!isTauri()) throw new Error('阅读器需要在 Kotoclip 桌面窗口中运行');
  return invoke<T>(command, payload);
}
