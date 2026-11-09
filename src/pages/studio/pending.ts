// The idea typed on the Studio page, handed to the new project's conversation, which sends it
// once it is on screen. Kept in memory only (never in the address).

const pending = new Map<string, string>();

export function setPending(projectId: string, text: string): void {
  pending.set(projectId, text);
}

/** The waiting text, once. */
export function takePending(projectId: string): string | null {
  const text = pending.get(projectId) ?? null;
  pending.delete(projectId);
  return text;
}
