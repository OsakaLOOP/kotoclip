export async function runSessionBoundOperation<T, R>(
  expectedSessionId: string,
  currentSessionId: () => string | null,
  operation: () => Promise<T>,
  apply: (value: T) => R,
): Promise<R | null> {
  if (currentSessionId() !== expectedSessionId) return null;
  const value = await operation();
  if (currentSessionId() !== expectedSessionId) return null;
  return apply(value);
}
