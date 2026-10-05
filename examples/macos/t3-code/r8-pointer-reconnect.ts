// Lane r8-pointer (D14): a relaunch reconnects to the saved environment, as
// the reference does ("The environment is saved and will reconnect on app
// startup."). The transport restores the last origin but opens nothing until
// asked, and the fleet (settings-b-fleet.ts) would open every switched-on saved
// environment as a background transport while the focused one sat
// disconnected. On the first refresh of a process, a disconnected focus with a
// switched-on saved environment connects: the last origin's, else this
// machine's (loopback), else the first saved. The credential is the one
// Keychain keeps for it (an empty pairing code). Until the server names its
// environment, the fleet treats that one as the focus, so it is not opened twice.
import { arr, obj, str, type Obj } from './domain';
import { bridgeReply, type Native } from './protocol';
import { environmentKey, isLoopback, trimOrigin, type FocusedHost } from './settings-b-fleet';

type Focus = { origin: string; environmentId: string; connection: string };
const asked = new WeakSet<object>();
const pending = new WeakMap<object, { origin: string; environmentId: string }>();

/** The saved environment a relaunch opens, or null. */
export function relaunchTarget(saved: Obj[], lastOrigin: string): Obj | null {
  const enabled = saved.filter(entry => str(entry.origin) && str(entry.environmentId) && entry.enabled !== false);
  const last = trimOrigin(lastOrigin);
  return enabled.find(entry => last && trimOrigin(str(entry.origin)) === last)
    ?? enabled.find(entry => isLoopback(str(entry.origin))) ?? enabled[0] ?? null;
}

/**
 * Once per client, on its first status: connect a disconnected focus to its
 * saved environment. Opening completes when the socket opens; its progress
 * arrives as t3.status changes, so this answer never waits on the network.
 */
export async function reconnectOnLaunch(client: object, native: Native, status: Obj): Promise<boolean> {
  if (asked.has(client)) return false;
  asked.add(client);
  if (str(status.state) !== 'disconnected' || str(status.environmentId)) return false;
  let saved: Obj[] = [];
  try {
    const listed = await bridgeReply(native, { op: 'environments' });
    if (!listed.ok) return false;
    saved = arr(obj(listed.value).saved);
  } catch { return false; }
  const target = relaunchTarget(saved, str(status.origin));
  if (!target) return false;
  pending.set(client, { origin: str(target.origin), environmentId: str(target.environmentId) });
  void native.later({ op: 'connect', origin: str(target.origin), credential: '' }).catch(() => undefined);
  return true;
}

/** The focus the fleet leaves alone: the client's, or the environment a relaunch is opening. */
export function launchFocus<T extends Focus>(client: T): T | FocusedHost {
  const opening = pending.get(client);
  if (!opening) return client;
  if (client.environmentId || client.connection === 'error' || (client.connection === 'disconnected' && trimOrigin(client.origin) !== trimOrigin(opening.origin))) {
    pending.delete(client);
    return client;
  }
  return { origin: opening.origin, environmentId: opening.environmentId, connection: 'connecting' };
}

/** For tests: the key the fleet treats as focused while a relaunch opens. */
export const launchFocusKey = (client: Focus) => { const focus = launchFocus(client); return focus.environmentId ? environmentKey(focus.origin, focus.environmentId) : ''; };
