// T3 Code 1e2ecbd975 terminalFocus.ts: native first-responder ownership replaces DOM containment.
// A blur only clears the terminal that emitted it, so a late blur cannot erase a newer focus.
import type { T3Client } from './client';
import { obj, str } from './domain';
import { draftThreadId } from './r7-handoff-thread';

export type TerminalFocus = { environmentId: string; threadId: string; terminalId: string; surface: string };
const focused = new WeakMap<object, TerminalFocus>();
export function recordTerminalFocus(owner: object, value: unknown): void {
  const message = obj(value);
  const next = { environmentId: str(message.environmentId), threadId: str(message.threadId), terminalId: str(message.terminalId), surface: str(message.surface, 'drawer') };
  if (!next.threadId || !next.terminalId) return;
  if (message.focused === true) focused.set(owner, next);
  else {
    const current = focused.get(owner);
    if (current?.environmentId === next.environmentId && current.threadId === next.threadId && current.terminalId === next.terminalId && current.surface === next.surface) focused.delete(owner);
  }
}
export function focusedTerminal(client: T3Client): TerminalFocus | null {
  const current = focused.get(client);
  if (!current || current.environmentId !== client.environmentId) return null;
  const threadId = client.threadId || (client.local.composerControls ? draftThreadId(client) : '');
  return current.threadId === threadId ? current : null;
}
export const terminalFocused = (client: T3Client): boolean => focusedTerminal(client) !== null;
export function clearTerminalFocus(client: object): void { focused.delete(client); }
