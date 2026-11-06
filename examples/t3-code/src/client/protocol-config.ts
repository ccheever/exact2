// Config-stream side effects. Reference (MIT, 5d3d78ab93):
// apps/web/src/components/KeybindingsUpdateToast.logic.ts and routes/__root.tsx
// handleServerConfigUpdated: every live `keybindingsUpdated` event toasts once,
// "Keybindings updated" on success, or a warning naming the first keybindings
// issue with "Open keybindings.json". A save and the server's file watcher each
// publish the same snapshot, so the reference drops an identical toast (the
// success, or a warning with the same message) inside a 2 s cooldown.
import type { T3Client } from './client';
import { arr, str, type Obj } from './domain';
import { composerNow } from '../composer/composer-controls';
import { pushToast } from '../shell/toast';

export type KeybindingsToastDecision = { tag: 'Success' } | { tag: 'InvalidConfiguration'; message: string } | null;

/** The decision for one keybindings event. */
export function keybindingsToastDecision(event: Obj): KeybindingsToastDecision {
  if (event.type !== 'keybindingsUpdated') return null;
  const issue = arr((event.payload as Obj | undefined)?.issues).find(entry => str(entry.kind).startsWith('keybindings.'));
  return issue ? { tag: 'InvalidConfiguration', message: str(issue.message) } : { tag: 'Success' };
}

const lastToasts = new WeakMap<T3Client, { key: string; at: number }>();

/** Called for each config-stream event the client folds (client.ts drain). */
export function configEventSideEffects(client: T3Client, event: Obj): void {
  const decision = keybindingsToastDecision(event);
  if (!decision) return;
  const key = decision.tag === 'Success' ? 'success' : `invalid:${decision.message}`;
  const at = composerNow(client), last = lastToasts.get(client);
  if (last?.key === key && at - last.at < 2000) return;
  lastToasts.set(client, { key, at });
  if (decision.tag === 'Success') {
    pushToast(client, { kind: 'success', title: 'Keybindings updated', description: 'Keybindings configuration reloaded successfully.' });
    return;
  }
  // stackedThreadToast({ type: "warning", actionVariant: "outline", actionProps: "Open keybindings.json" }).
  pushToast(client, { kind: 'warning', title: 'Invalid keybindings configuration', description: decision.message, stacked: true, actionVariant: 'outline',
    action: { label: 'Open keybindings.json', op: 'rest:keybinding-open', id: `${client.environmentId}:`, value: '' } });
}
