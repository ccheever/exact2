// Subagent work: chat/NativeChildWorkDialog.tsx and ProviderSubagentBar's actions at T3 Code 5d3d78ab93 (MIT; see
// LICENSE-T3). An Antigravity subagent thread's bar offers a read-only view of the provider-native subagents its call
// started (orchestration.getNativeChildWork): each one's role and prompt, then its steps. It loads only while open,
// and Refresh reads it again, as running children change between opens.
import { getNativeChildWork, type NativeChildWork } from '../client/server-operations';
import type { Native } from '../client/protocol';
import type { T3Client } from '../client/client';

export type NativeChildStepView = { key: string; label: string; calls: string; text: string; hasText: boolean };
export type NativeChildView = { key: string; role: string; prompt: string; truncated: boolean; transcript: boolean; steps: NativeChildStepView[] };
export type NativeChildWorkView = { open: boolean; loading: boolean; error: string; children: NativeChildView[] };
type State = { open: boolean; threadId: string; scope: string; loading: boolean; error: string; children: NativeChildView[] | null; serial: number };
const states = new WeakMap<T3Client, State>();
const state = (client: T3Client): State => {
  let found = states.get(client);
  if (!found) states.set(client, found = { open: false, threadId: '', scope: '', loading: false, error: '', children: null, serial: 0 });
  return found;
};

const scope = (client: { threadId: string; environmentId?: string; generation?: number }) => `${client.environmentId}:${client.generation}:${client.threadId}`;

/** stepLabel: "TOOL_CALL" reads "Tool call". */
const stepLabel = (type: string) => { const words = type.toLowerCase().replaceAll('_', ' '); return words.charAt(0).toUpperCase() + words.slice(1); };

function child(value: NativeChildWork): NativeChildView {
  const steps = value.steps;
  return { key: value.conversationId, role: value.role || 'Subagent', prompt: value.prompt, truncated: value.truncated === true, transcript: steps !== null,
    steps: (steps ?? []).map(step => ({ key: String(step.index),
      label: `${stepLabel(step.type)}${typeof step.status === 'string' && step.status !== 'DONE' ? ` · ${stepLabel(step.status)}` : ''}`,
      calls: step.toolCalls.map(call => `${call.name} ${call.input}`).join('\n'),
      text: typeof step.text === 'string' ? step.text : '', hasText: typeof step.text === 'string' })) };
}

/** `cc:native-child-work` with `open`, `refresh` or `close`: the dialog for the open thread. */
export async function nativeChildWork(client: T3Client, native: Native, op: string): Promise<void> {
  const own = state(client);
  if (op === 'close') { own.open = false; own.children = null; own.error = ''; own.serial++; return; }
  if (op === 'open') { own.open = true; own.threadId = client.threadId; own.scope = scope(client); own.children = null; own.error = ''; }
  if (!own.open || !own.threadId || own.scope !== scope(client)) return;
  const serial = ++own.serial;
  own.loading = true;
  try {
    const children = await getNativeChildWork(client, native, own.threadId);
    if (serial === own.serial && own.scope === scope(client)) { own.children = children.map(child); own.error = ''; }
  } catch (error) {
    if (serial === own.serial && own.scope === scope(client)) own.error = error instanceof Error ? error.message : 'Could not read the subagent work.';
  } finally { if (serial === own.serial && own.scope === scope(client)) own.loading = false; }
}

/** The dialog as the composer view carries it; closed once the thread it opened for is no longer open. */
export function nativeChildWorkView(client: { threadId: string }): NativeChildWorkView {
  const own = states.get(client as T3Client);
  if (own?.open && own.scope !== scope(client)) { own.open = false; own.children = null; own.serial++; }
  if (!own?.open) return { open: false, loading: false, error: '', children: [] };
  return { open: true, loading: own.loading || (own.children === null && !own.error), error: own.error, children: own.children ?? [] };
}
