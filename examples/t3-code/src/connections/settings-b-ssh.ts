// Add Environment → SSH (ConnectionsSettings.tsx renderSshFields,
// parseManualDesktopSshTarget, handleSelectSshHostSuggestion, connectSavedBackendSshTarget;
// state/desktopSshHosts.ts filterDiscoveredSshHosts, at 74541bab17; EnvironmentRow.tsx
// formatDesktopSshTarget; connection/platform.ts provisionDesktopSshEnvironment's pairing
// failure, at 7c3b3b3605ef). Discovery, `ssh -G` and the tunnel are T3Ssh.swift.
import { arr, obj, str, type Obj } from '../client/domain';
import { ClientError, bridgeReply, type Native } from '../client/protocol';
import { commandShortcut } from '../palette/palette';
import { pushToast } from '../shell/toast';
import type { T3Client } from '../client/client';
import { rememberSshAlias } from './remote-open';
import { withStandardScope } from './remote-scopes';
import { placeRoute, savedEntry, savedList } from './connection-routes-ops';
import { CLIENT_VERSION } from './version-skew';

export type SshTarget = { alias: string; hostname: string; username: string | null; port: number | null };
export type SshHost = SshTarget & { source: string };

/** formatDesktopSshTarget: user@host:port, each part only when known. */
export function formatSshTarget(target: SshTarget): string {
  const authority = target.username ? `${target.username}@${target.hostname}` : target.hostname;
  return target.port ? `${authority}:${target.port}` : authority;
}

/** parseManualDesktopSshTarget. */
export function parseManualSshTarget(input: { host: string; username: string; port: string }): SshTarget {
  const rawHost = input.host.trim();
  if (!rawHost) throw new ClientError('SSH host or alias is required.');
  let hostname = rawHost, username: string | null = input.username.trim() || null, port: number | null = null;
  const at = hostname.lastIndexOf('@');
  if (at > 0) {
    const inline = hostname.slice(0, at).trim();
    hostname = hostname.slice(at + 1).trim();
    if (!username && inline) username = inline;
  }
  const bracketed = /^\[([^\]]+)\](?::(\d+))?$/u.exec(hostname);
  if (bracketed) {
    hostname = bracketed[1]!.trim();
    if (bracketed[2]) port = Number.parseInt(bracketed[2], 10);
  } else {
    const segments = hostname.split(':');
    if (segments.length === 2 && /^\d+$/u.test(segments[1] ?? '')) { hostname = segments[0]!.trim(); port = Number.parseInt(segments[1]!, 10); }
  }
  if (input.port.trim()) port = Number.parseInt(input.port.trim(), 10);
  if (!hostname) throw new ClientError('SSH host or alias is required.');
  if (port !== null && (!Number.isInteger(port) || port <= 0 || port > 65_535)) throw new ClientError('SSH port must be between 1 and 65535.');
  return { alias: hostname, hostname, username, port };
}

/** filterDiscoveredSshHosts: alias prefix matches, then alias or hostname substring matches. */
export function filterSshHosts<T extends { alias: string; hostname: string }>(hosts: T[], query: string): T[] {
  const normalized = query.trim().toLowerCase();
  if (!normalized) return hosts;
  const prefix: T[] = [], substring: T[] = [];
  for (const host of hosts) {
    const alias = host.alias.toLowerCase();
    if (alias.startsWith(normalized)) prefix.push(host); else if (alias.includes(normalized) || host.hostname.toLowerCase().includes(normalized)) substring.push(host);
  }
  return [...prefix, ...substring];
}

/** formatDesktopSshConnectionError: strip IPC and tagged-error prefixes. */
export function formatSshError(error: unknown): string {
  const raw = error instanceof Error ? error.message : 'Failed to connect SSH host.';
  return raw.replace(/^Error invoking remote method 'desktop:ensure-ssh-environment':\s*/u, '').replace(/^Ssh[A-Za-z]+Error:\s*/u, '').trim() || 'Failed to connect SSH host.';
}

const decodeTarget = (value: unknown): SshTarget => {
  const entry = obj(value);
  return { alias: str(entry.alias), hostname: str(entry.hostname, str(entry.alias)), username: str(entry.username) || null, port: typeof entry.port === 'number' ? entry.port : null };
};

// One discovery per dialog opening (desktopSshHostsAtom refreshes when discovery turns active).
let discovery: { hosts: SshHost[]; targets: Record<string, SshTarget>; available: boolean; error: string } | null = null;
let discoveryOpen = false;
async function discover(native: Native): Promise<NonNullable<typeof discovery>> {
  if (discovery) return discovery;
  try {
    const reply = await bridgeReply(native, { op: 'sshHosts' });
    if (!reply.ok) throw new ClientError(reply.error!.message);
    const value = obj(reply.value);
    const targets: Record<string, SshTarget> = {};
    for (const [origin, target] of Object.entries(obj(value.targets))) targets[origin] = decodeTarget(target);
    discovery = { hosts: arr(value.hosts).map(host => ({ ...decodeTarget(host), source: str(host.source) })), targets, available: value.available === true, error: '' };
  } catch (error) { discovery = { hosts: [], targets: {}, available: false, error: formatSshError(error) }; }
  return discovery;
}
/** The saved SSH environments' targets by loopback origin (for their rows' "SSH user@host" label). */
export async function sshTargets(native: Native | null | undefined): Promise<Record<string, SshTarget>> {
  if (!native?.available) return {};
  try {
    const reply = await bridgeReply(native, { op: 'sshHosts' });
    const targets: Record<string, SshTarget> = {};
    if (reply.ok) for (const [origin, target] of Object.entries(obj(obj(reply.value).targets))) targets[origin] = decodeTarget(target);
    return targets;
  } catch { return {}; }
}

/** The SSH host field's autocomplete: unsaved discovered hosts, filtered, with ⌘1–⌘9. */
export async function sshHostsView(client: T3Client, native: Native | null | undefined, open: boolean, query: string) {
  const empty = { loading: false, hosts: [] as { index: number; alias: string; address: string; shortcut: string }[], noMatch: '', error: '', hasContent: false };
  if (!open || !native?.available) { discoveryOpen = false; return empty; }
  if (!discoveryOpen) { discovery = null; discoveryOpen = true; }
  const found = await discover(native);
  const saved = new Set(Object.values(found.targets).flatMap(target => [target.alias, formatSshTarget(target)]));
  // A configured alias stays offered when only its resolved address is saved: it may name another user or port.
  const unsaved = found.hosts.filter(host => !saved.has(host.alias) && (host.source === 'ssh-config' || !saved.has(formatSshTarget(host))));
  const filtered = filterSshHosts(unsaved, query);
  return { ...empty, error: found.error, hasContent: unsaved.length > 0,
    hosts: filtered.slice(0, 200).map((host, index) => ({ index, alias: host.alias, address: formatSshTarget(host) === host.alias ? '' : formatSshTarget(host),
      shortcut: index < 9 ? commandShortcut(client, `thread.jump.${index + 1}`) : '' })),
    noMatch: unsaved.length > 0 && filtered.length === 0 ? `No hosts match "${query.trim()}".` : '' };
}

type Result = { status: Obj | null; generation: number };
async function call(native: Native, request: Obj) {
  const reply = await bridgeReply(native, request);
  if (!reply.ok) throw new ClientError(reply.error!.message, reply.error!.kind, false, { reason: reply.error!.reason, context: reply.error!.context });
  return reply;
}

/**
 * A reused remote server on another build can fail to decode the grant this desktop just minted, and answers 500 on
 * every retry: the add then names both versions (provisionDesktopSshEnvironment). The transport ends such a pairing
 * with reason `exchange_http_500` and the server's descriptor label and version as its context. Any other failure,
 * or a server on this build, stays as it was.
 */
export function sshPairingVersionFailure(failure: unknown, clientVersion = CLIENT_VERSION): ClientError | null {
  if (!(failure instanceof ClientError) || failure.reason !== 'exchange_http_500') return null;
  const label = failure.context.label ?? '', serverVersion = failure.context.serverVersion ?? '';
  if (!label || !serverVersion || serverVersion === clientVersion) return null;
  return new ClientError(`${label} runs T3 Code ${serverVersion}, but this desktop is ${clientVersion}, and that server could not accept its pairing. Restart the T3 server on ${label} so it runs ${clientVersion}, then retry.`, 'configuration');
}
/** One pairing call; a version-skewed server's 500 reads as sshPairingVersionFailure. */
async function pair(native: Native, request: Obj) {
  try { return await call(native, request); } catch (error) { throw sshPairingVersionFailure(error) ?? error; }
}

/**
 * `environment-ssh-add` (the manual fields: id is the host, value `username=…&port=…`)
 * and `environment-ssh-pick` (a suggestion: id is its alias, resolved through `ssh -G`).
 * Both tunnel to the host, pair with the issued token and save the environment.
 */
export async function runSshOp(native: Native, op: string, id: string, value: string, connected: boolean, client?: T3Client): Promise<Result> {
  const fields = Object.fromEntries(value.split('&').filter(Boolean).map(part => { const at = part.indexOf('='); return [decodeURIComponent(at < 0 ? part : part.slice(0, at)), decodeURIComponent(at < 0 ? '' : part.slice(at + 1))]; }));
  try {
    const target = op === 'environment-ssh-pick'
      ? decodeTarget(obj((await call(native, { op: 'sshResolve', alias: id.trim() })).value))
      : parseManualSshTarget({ host: id, username: str(fields.username), port: str(fields.port) });
    const bootstrap = obj((await call(native, { op: 'sshConnect', alias: target.alias, hostname: target.hostname, ...(target.username ? { username: target.username } : {}), ...(target.port ? { port: target.port } : {}), pair: true })).value);
    const origin = str(bootstrap.origin), credential = str(bootstrap.credential);
    if (!origin || !credential) throw new ClientError('SSH pairing did not return a credential.');
    discovery = null;
    rememberSshAlias(origin, target.alias, target.username); // remote Open reads the SSH target of an SSH environment (remote-open.ts)
    let result: Result = { status: null, generation: -1 };
    // "Add a route to <label>" over SSH joins that machine's routes, or is refused before the code is spent.
    const expected = str(fields.expectedEnvironmentId);
    if (expected) {
      await pair(native, { op: 'pairEnvironment', ...withStandardScope({ origin, credential }), expectedEnvironmentId: expected, ssh: true });
      await placeRoute(native, expected, origin, target);
      const label = str(savedEntry(await savedList(native), expected)?.label, target.alias);
      if (client) pushToast(client, { kind: 'success', title: 'Route added', description: `${label} can now be reached over SSH ${target.alias}.` });
      return result;
    }
    // `ssh`: the pairing comes from an SSH host's bootstrap (ssh.provision, prepare), so a server on another build that
    // refuses it answers with the version sentence.
    if (connected) await pair(native, { op: 'pairEnvironment', ...withStandardScope({ origin, credential }), ssh: true });
    else { const reply = await pair(native, { op: 'connect', ...withStandardScope({ origin, credential }), ssh: true }); result = { status: obj(reply.value), generation: reply.generation }; }
    if (client) pushToast(client, { kind: 'success', title: 'Environment connected', description: `${target.alias} is ready over an SSH-managed tunnel.` });
    return result;
  } catch (error) {
    throw new ClientError(formatSshError(error));
  }
}
export const SSH_OPS = ['environment-ssh-add', 'environment-ssh-pick'];
