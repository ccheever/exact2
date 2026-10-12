// Config-stream side effects (lane r3-protocol). Reference (MIT):
// apps/web/src/components/KeybindingsUpdateToast.logic.ts and routes/__root.tsx
// handleServerConfigUpdated: every live `keybindingsUpdated` event toasts once,
// "Keybindings updated" on success, or a warning naming the first keybindings
// issue with "Open keybindings.json". The reference's 2 s success cooldown needs a
// clock a data source does not have; one keyed success toast replaces itself
// instead, so a burst of reloads still shows a single toast. Each invalid reload
// stacks a new warning over it, as the reference's toast manager does.
import type { T3Client } from './client';
import { arr, str, type Obj } from './domain';
import { pushToast } from './toast';

export type KeybindingsToastDecision = { tag: 'Success' } | { tag: 'InvalidConfiguration'; message: string } | null;

/** createKeybindingsUpdateToastController().handle, without the clock. */
export function keybindingsToastDecision(event: Obj): KeybindingsToastDecision {
  if (event.type !== 'keybindingsUpdated') return null;
  const issue = arr((event.payload as Obj | undefined)?.issues).find(entry => str(entry.kind).startsWith('keybindings.'));
  return issue ? { tag: 'InvalidConfiguration', message: str(issue.message) } : { tag: 'Success' };
}

/** Called for each config-stream event the client folds (client.ts drain). */
export function configEventSideEffects(client: T3Client, event: Obj): void {
  const decision = keybindingsToastDecision(event);
  if (!decision) return;
  if (decision.tag === 'Success') {
    pushToast(client, { kind: 'success', title: 'Keybindings updated', description: 'Keybindings configuration reloaded successfully.', key: 'keybindings-updated' });
    return;
  }
  // stackedThreadToast({ type: "warning", actionVariant: "outline", actionProps: "Open keybindings.json" }).
  pushToast(client, { kind: 'warning', title: 'Invalid keybindings configuration', description: decision.message, stacked: true, actionVariant: 'outline',
    action: { label: 'Open keybindings.json', op: 'rest:keybinding-open', id: `${client.environmentId}:`, value: '' } });
}
