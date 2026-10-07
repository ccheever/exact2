// Lane r8-pointer (D14): a relaunch reconnects, as the reference does ("The environment is saved
// and will reconnect on app startup."). The transport restores the last focus but opens nothing
// until asked, and the fleet (settings-b-fleet.ts) would open every switched-on saved environment
// as a background transport while the focused one sat disconnected. On the first refresh of a
// process, a disconnected focus connects to:
//  - the primary ("This machine", local-primary.ts) when the transport's focus token says it was
//    the last focus (its port can change every launch, so it has no remembered origin);
//  - else the saved environment of the last origin (the credential Keychain keeps for it);
//  - else the primary while the Local environment switch is on (the reference's desktop always
//    opens its primary environment);
//  - else the first switched-on saved environment.
// The primary is connected once its server is ready with its bearer (20261005-local-primary-
// environment; until then the window shows the connecting state, decision U5 provisional, because
// the host opens the first window before the server is ready, exact2 #117 / issue X31). When the
// primary cannot come (switched off, a refused development build, no runtime), the launch falls
// back to the saved rule. Until the server names its environment, the fleet treats the one being
// opened as the focus, so it is not opened twice.
import { arr, obj, str, type Obj } from './domain';
import { bridgeReply, type Native } from './protocol';
import { environmentKey, trimOrigin, type FocusedHost } from './settings-b-fleet';
import { primary, withoutPrimaryDuplicates, type LocalPrimary } from './local-primary';

type Focus = { origin: string; environmentId: string; connection: string };
const asked = new WeakSet<object>();
const pending = new WeakMap<object, { origin: string; environmentId: string; primary?: boolean }>();
/** Clients whose launch waits for the primary's server. */
const awaitingPrimary = new WeakMap<object, Obj[]>();

/** The saved environment a relaunch opens when the primary does not: the last origin's, else the first switched on. */
export function relaunchTarget(saved: Obj[], lastOrigin: string): Obj | null {
  const enabled = saved.filter(entry => str(entry.origin) && str(entry.environmentId) && entry.enabled !== false);
  const last = trimOrigin(lastOrigin);
  return enabled.find(entry => last && trimOrigin(str(entry.origin)) === last) ?? enabled[0] ?? null;
}

/** What a launch opens: the primary, a saved environment, or nothing. */
export function launchChoice(saved: Obj[], status: Obj, source: LocalPrimary = primary): 'primary' | Obj | null {
  if (str(status.focus) === 'primary' && !source.disabled) return 'primary';
  const last = trimOrigin(str(status.origin));
  const byOrigin = last ? saved.find(entry => entry.enabled !== false && str(entry.environmentId) && trimOrigin(str(entry.origin)) === last) : undefined;
  if (byOrigin) return byOrigin;
  if (!source.disabled) return 'primary';
  return relaunchTarget(saved, '');
}

function open(client: object, native: Native, target: Obj): void {
  pending.set(client, { origin: str(target.origin), environmentId: str(target.environmentId) });
  void native.later({ op: 'connect', origin: str(target.origin), credential: '' }).catch(() => undefined);
}

/**
 * Once per client, on its first status: choose what a disconnected focus opens. Opening completes
 * when the socket opens; its progress arrives as t3.status changes, so this answer never waits on
 * the network. Each later refresh runs the primary's part: connect once its server is ready, or
 * fall back to the saved rule once it cannot come.
 */
export async function reconnectOnLaunch(client: Focus, native: Native, status: Obj, source: LocalPrimary = primary): Promise<boolean> {
  if (asked.has(client)) return primaryLaunch(client, native, source);
  asked.add(client);
  if (str(status.state) !== 'disconnected' || str(status.environmentId)) return false;
  let saved: Obj[] = [];
  try {
    const listed = await bridgeReply(native, { op: 'environments' });
    if (!listed.ok) return false;
    saved = withoutPrimaryDuplicates(arr(obj(listed.value).saved), source);
  } catch { return false; }
  const choice = launchChoice(saved, status, source);
  if (choice === null) return false;
  if (choice !== 'primary') { open(client, native, choice); return true; }
  awaitingPrimary.set(client, saved);
  return primaryLaunch(client, native, source);
}

/** The launch's wait for the primary: connect it once ready; fall back once it cannot come. */
function primaryLaunch(client: Focus, native: Native, source: LocalPrimary): boolean {
  const saved = awaitingPrimary.get(client);
  if (!saved) return false;
  // Something else connected meanwhile (a pairing, a thread opened from the fleet).
  if (client.environmentId || !['disconnected', 'error'].includes(client.connection)) { awaitingPrimary.delete(client); return false; }
  if (source.connectable && source.target) {
    awaitingPrimary.delete(client);
    const origin = trimOrigin(source.target.httpBaseUrl);
    pending.set(client, { origin, environmentId: source.target.environmentId, primary: true });
    void native.later({ op: 'connect', origin, primary: true }).catch(() => undefined);
    return true;
  }
  if (!source.unavailable) return false;
  awaitingPrimary.delete(client);
  const fallback = relaunchTarget(saved, '');
  if (!fallback) return false;
  open(client, native, fallback);
  return true;
}

/** The focus the fleet leaves alone: the client's, or the environment a launch is opening (the primary included). */
export function launchFocus<T extends Focus>(client: T, source: LocalPrimary = primary): T | FocusedHost {
  if (awaitingPrimary.has(client)) return { origin: trimOrigin(source.target?.httpBaseUrl ?? ''), environmentId: source.target?.environmentId ?? '', connection: 'connecting' };
  const opening = pending.get(client);
  if (!opening) return client;
  // The primary's address is not the client's until its socket reports it, so only a named environment or an error ends that wait.
  if (client.environmentId || client.connection === 'error' || (!opening.primary && client.connection === 'disconnected' && trimOrigin(client.origin) !== trimOrigin(opening.origin))) {
    pending.delete(client);
    return client;
  }
  return { origin: opening.origin, environmentId: opening.environmentId, connection: 'connecting' };
}

/** For tests: the key the fleet treats as focused while a launch opens. */
export const launchFocusKey = (client: Focus) => { const focus = launchFocus(client); return focus.environmentId ? environmentKey(focus.origin, focus.environmentId) : ''; };
