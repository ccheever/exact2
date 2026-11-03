// Config-stream side effects. Reference (MIT, 5d3d78ab93):
// apps/web/src/components/KeybindingsUpdateToast.logic.ts and routes/__root.tsx
// handleServerConfigUpdated: every live `keybindingsUpdated` event toasts once,
// "Keybindings updated" on success, or a warning naming the first keybindings
// issue with "Open keybindings.json". A save and the server's file watcher each
// publish the same snapshot, so the reference drops an identical toast (the
// success, or a warning with the same message) inside a 2 s cooldown. That needs a
// clock a data source does not have; each toast is keyed by what it says instead,
// so an identical one replaces the one showing. A different warning stacks over
// it, as the reference's toast manager does.
import type { T3Client } from './client';
import { arr, str, type Obj } from './domain';
import { pushToast } from '../shell/toast';

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
  pushToast(client, { kind: 'warning', title: 'Invalid keybindings configuration', description: decision.message, stacked: true, actionVariant: 'outline', key: `keybindings-invalid:${decision.message}`,
    action: { label: 'Open keybindings.json', op: 'rest:keybinding-open', id: `${client.environmentId}:`, value: '' } });
}
