// The handoff stream's inbox entries (managed-codex-chatgpt; CodexSetupSection.tsx :219-222,
// :366-387): `provider.chatgpt.handoff.subscribe` runs on the primary's connection, which is the
// focused one or a background one, so both inboxes route its entries here (client.ts drain,
// settings-b-fleet.ts drain). Kept small: the fleet imports it.
import { obj, str, type Obj } from './domain';
import { codexFlows } from './codex-setup';

export const HANDOFF_KEY = 'chatgpt-handoff:';
/** The connections whose setups have a handoff open (codex-setup-ops.ts signIn adds them). */
export const handoffHosts = new Set<object>();

/**
 * One `provider.chatgpt.handoff.subscribe` inbox entry on the primary's connection. A stream
 * failure ends the handoff with its error; the stream is never subscribed again on its own
 * (a reconnect must not replay a sign-in, X21).
 */
export function codexHandoffEvent(event: Obj): boolean {
  const key = str(event.key);
  if (!key.startsWith(HANDOFF_KEY)) return false;
  const instanceId = key.slice(HANDOFF_KEY.length);
  for (const host of handoffHosts) {
    const flow = codexFlows(host).get(instanceId), handoff = flow?.handoff;
    if (!flow || !handoff || !handoff.subscription || handoff.subscription !== str(event.subscriptionId)) continue;
    const item = obj(event.value);
    if (item._retryDue || item._transportError || item._streamEnded) { handoff.error = 'ChatGPT sign-in on the primary environment was interrupted. Try again.'; continue; }
    handoff.state = item;
  }
  return true;
}

