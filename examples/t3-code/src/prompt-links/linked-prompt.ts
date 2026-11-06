// T3 Code 9c161462c9 (MIT, LICENSE-T3): components/promptLinks/linkedPrompt.ts.
// Draft targets are the clone's environment-qualified keys; toast actions return through shellLocal.
import type { T3Client } from '../client/client';
import { dismissToast, pushToast, toasts, updateToast } from '../shell/toast';
import type { LinkedPrompt } from './prompt-links';

type Replacement = { key: string; prompt: LinkedPrompt; notes: readonly string[] };
const replacements = new WeakMap<T3Client, Map<number, Replacement>>();

export function warnAboutHiddenCharacters(client: T3Client, prompt: LinkedPrompt): void {
  if (prompt.hiddenCharacters === 0) return;
  pushToast(client, { kind: 'warning', title: 'This prompt had hidden characters, it may be malicious',
    description: `T3 Code removed ${prompt.hiddenCharacters} hidden character${prompt.hiddenCharacters === 1 ? '' : 's'}. Read the prompt before you send it.`, timeoutMs: 0 });
}

function place(client: T3Client, key: string, prompt: LinkedPrompt, notes: readonly string[]): void {
  client.local.drafts[key] = prompt.text;
  pushToast(client, { kind: 'info', title: 'Prompt added from a link', description: ['Read it before you send.', ...notes].join(' ') });
}

export function placeLinkedPrompt(client: T3Client, key: string, prompt: LinkedPrompt, notes: readonly string[] = []): void {
  warnAboutHiddenCharacters(client, prompt);
  const existing = client.local.drafts[key] ?? '';
  if (!existing.trim() || existing === prompt.text) { place(client, key, prompt, notes); return; }
  const pending = replacements.get(client) ?? new Map<number, Replacement>();
  replacements.set(client, pending);
  const visible = new Set(toasts(client).map(toast => toast.id));
  for (const id of pending.keys()) if (!visible.has(id)) pending.delete(id);
  const id = pushToast(client, { kind: 'info', title: 'Linked prompt ready',
    description: 'This thread already has a draft. Replace it with the linked prompt?', timeoutMs: 0,
    onClose: () => pending.delete(id) });
  pending.set(id, { key, prompt, notes });
  updateToast(client, id, { action: { label: 'Replace draft', op: 'shelllocal:prompt-link-replace', id: String(id) } });
}

export function replaceLinkedPrompt(client: T3Client, id: number): void {
  const pending = replacements.get(client), replacement = pending?.get(id);
  if (!replacement) return;
  pending!.delete(id);
  dismissToast(client, id);
  place(client, replacement.key, replacement.prompt, replacement.notes);
}
