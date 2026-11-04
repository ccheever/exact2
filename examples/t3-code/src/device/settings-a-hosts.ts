// Settings › Devices › Device sources: the SSH hosts' editor and commands. Reference T3 Code
// 5d3d78ab93 DeviceHostEditor.tsx, deviceHostsSettings.logic.ts, deviceHostConnectionChecks.ts and
// DeviceSourcesSettings.tsx (saveSsh, testSsh, retry, Local simulators' Refresh). Hosts are the
// environment's `deviceHosts` setting; a connection check is `device.testHost`, a failed host's
// retry is `device.list { retryHostId }`. Scope targets retain their own lists, ids and checks.
import type { T3Client } from '../client/client';
import { arr, obj, str, type Obj } from '../client/domain';
import { ClientError, type Native } from '../client/protocol';
import { pushToast } from '../shell/toast';
import { letGo } from '../client/let-go';
import { liveEnvironments } from '../client/live-streams';
import { environmentClient } from '../settings/projects-fleet';
import { settingsScopeEnvironmentLabels } from '../settings/settings-scope';
import type { ScopeEnvironment } from '../settings/settings-scope-sources';
import { randomBytes } from '../transport/transport-wire'; // drawn ahead: code resumed by a server reply may not draw

export type HostConfig = { id: string; label: string; target: string; identityFile?: string; port?: number };
type Check = { status: 'pending' | 'local' | 'connected' | 'failed'; platforms: Obj[]; tools: Obj | null; error: string };
type Editor = { isNew: boolean; original: HostConfig | null; host: HostConfig; serial: number };
export type HostTarget = { client: T3Client; label: string; secondary: string; state: Obj | null };
type HostsState = { targets: HostTarget[]; editingTargets: HostTarget[]; editor: Editor | null; checks: Map<string, Check>; serial: number; retrying: string };
const states = new WeakMap<T3Client, HostsState>();
const stateOf = (client: T3Client) => { let state = states.get(client); if (!state) states.set(client, state = { targets: [], editingTargets: [], editor: null, checks: new Map(), serial: 0, retrying: '' }); return state; };

/** deviceHostConnectionKey: target, port and identity file. The contract spells the same key. */
export const connectionKey = (host: { target: string; port?: number | string; identityFile?: string }) =>
  `${host.target.trim()}|${host.port === undefined || host.port === '' ? '' : String(host.port)}|${(host.identityFile ?? '').trim()}`;
/** parseDeviceHostDraft over SshDeviceHostConfig: an id, a label, a target without spaces or a leading dash, an optional identity file and port. */
export function parseHost(input: { id: string; label: string; target: string; identityFile?: string; port?: string }): HostConfig | string {
  const label = input.label.trim(), target = input.target.trim(), identity = (input.identityFile ?? '').trim(), portText = (input.port ?? '').trim();
  if (!/^[a-zA-Z0-9][a-zA-Z0-9_-]*$/.test(input.id) || input.id === 'local') return 'That host id is not valid.';
  if (!label) return 'Enter a name for this host.';
  if (!target || !/^[^\s-][^\s]*$/.test(target)) return 'Enter an SSH target such as user@host or an SSH alias.';
  let port: number | undefined;
  if (portText) { port = Number(portText); if (!Number.isInteger(port) || port < 1 || port > 65535) return 'Use a port between 1 and 65535.'; }
  return { id: input.id, label, target, ...(identity ? { identityFile: identity } : {}), ...(port !== undefined ? { port } : {}) };
}
/** updateDeviceHosts: one host change without replacing another environment's list. */
export function updateDeviceHosts(hosts: HostConfig[], host: HostConfig, remove: boolean, original: HostConfig = host): HostConfig[] {
  const same = (a: HostConfig, b: HostConfig) => a.target === b.target && a.port === b.port && a.identityFile === b.identityFile;
  const find = (destination: HostConfig) => {
    const matches = hosts.filter(candidate => same(candidate, destination));
    if (matches.length > 1) throw new ClientError('Multiple hosts match this SSH destination. Select the environment to edit its hosts.');
    return matches[0];
  };
  const existing = hosts.find(candidate => candidate.id === original.id) ?? find(original) ?? (remove ? undefined : find(host));
  if (remove) return hosts.filter(candidate => candidate.id !== existing?.id);
  return existing ? hosts.map(candidate => candidate.id === existing.id ? { ...host, id: existing.id } : candidate) : [...hosts, host];
}
export const hostsOf = (settings: Obj): HostConfig[] => arr(settings.deviceHosts).map(host => ({ id: str(host.id), label: str(host.label), target: str(host.target),
  ...(str(host.identityFile) ? { identityFile: str(host.identityFile) } : {}), ...(typeof host.port === 'number' ? { port: host.port } : {}) }));
// randomUUID's shape (Math.random and Date.now are unavailable in data sources).
export const randomId = () => Array.from(randomBytes(16), (byte, index) => `${[4, 6, 8, 10].includes(index) ? '-' : ''}${byte.toString(16).padStart(2, '0')}`).join('');
const envLabel = (client: T3Client) => str(obj(client.config.environment).label, 'this environment');

// ── Views ──────────────────────────────────────────────────────────────────
/** Whether a connection check for this destination is running on the client (a row's Test connection). */
export const hostTesting = (client: T3Client, key: string) => stateOf(client).checks.get(key)?.status === 'pending';

/** Only configurations with unambiguous identities can share a row (deviceHostsSettings.logic.ts). */
export function groupDeviceHosts(environments: readonly { environmentId: string; label: string; hosts: HostConfig[] }[]) {
  const groups = new Map<string, { host: HostConfig; members: { environmentId: string; label: string; host: HostConfig }[] }>();
  const identities = new Map<string, string>(), configurations = new Map<string, string>();
  for (const environment of environments) {
    const destinations = new Set<string>();
    for (const host of environment.hosts) {
      const destination = JSON.stringify([host.target, host.port, host.identityFile]);
      const configuration = JSON.stringify([host.label, host.target, host.port, host.identityFile]);
      if (destinations.has(destination) || (identities.has(host.id) && identities.get(host.id) !== configuration)
        || (configurations.has(host.target) && configurations.get(host.target) !== configuration)) return null;
      destinations.add(destination); identities.set(host.id, configuration); configurations.set(host.target, configuration);
      const member = { environmentId: environment.environmentId, label: environment.label, host };
      const group = groups.get(destination);
      if (group) group.members.push(member); else groups.set(destination, { host, members: [member] });
    }
  }
  return [...groups.values()];
}
const targetClients = new WeakMap<T3Client, Map<string, { generation: number; client: T3Client }>>();
/**
 * The scope's connected environments as the rows and commands act on them. Each background client keeps its
 * check and editor cache across answers; its connection generation invalidates it.
 */
export function syncHostTargets(client: T3Client, native: Native, environments: ScopeEnvironment[], all: ScopeEnvironment[], deviceState: Obj | null): HostTarget[] {
  let cache = targetClients.get(client);
  if (!cache) targetClients.set(client, cache = new Map());
  const targets = environments.filter(environment => environment.connection.phase === 'connected' && environment.serverConfig).map(environment => {
    let cached = cache!.get(environment.environmentId);
    const remote = environmentClient(client, environment.environmentId, native);
    if (!cached || cached.generation !== remote.generation) cache!.set(environment.environmentId, cached = { generation: remote.generation, client: remote });
    const labels = settingsScopeEnvironmentLabels(environment, all);
    return { client: cached.client, label: labels.primary, secondary: labels.secondary, state: environment.environmentId === client.environmentId ? deviceState : liveEnvironments(client, native).find(entry => entry.environmentId === environment.environmentId)?.devices.value?.[0] ?? null };
  });
  stateOf(client).targets = targets;
  return targets;
}
/** The targets the last page answer named: the commands' fan-out, and the EAS editor's account check. */
export const hostTargets = (client: T3Client): HostTarget[] => stateOf(client).targets;

export type HostCheckRow = { id: string; label: string; status: string; platforms: string; error: string };
export type HostEditorView = { open: boolean; title: string; description: string; serial: string; id: string; label: string; target: string; identityFile: string; port: string; optionsOpen: boolean;
  checkKey: string; checking: boolean; checked: boolean; summary: string; results: HostCheckRow[] };
/** The Add/Edit device host dialog; its fields start from the edited host (the draft lives in the dialog). */
export function hostEditorView(client: T3Client): HostEditorView {
  const editor = stateOf(client).editor, label = envLabel(client);
  const blank: HostEditorView = { open: false, title: '', description: '', serial: '', id: '', label: '', target: '', identityFile: '', port: '', optionsOpen: false, checkKey: '', checking: false, checked: false, summary: 'Check access before saving', results: [] };
  if (!editor) return blank;
  const host = editor.host;
  return { ...blank, open: true, title: editor.isNew ? 'Add device host' : 'Edit device host', description: stateOf(client).editingTargets.length > 1 ? `Connect from each of ${stateOf(client).editingTargets.length} machines. Hosts on the same machine are skipped.` : `Connect from ${label}. Hosts on the same machine are skipped.`, serial: String(editor.serial),
    id: host.id, label: host.label, target: host.target, identityFile: host.identityFile ?? '', port: host.port === undefined ? '' : String(host.port), optionsOpen: host.port !== undefined || host.identityFile !== undefined };
}
/** The check shown under the dialog's Test connection for one connection key. */
export function hostChecks(client: T3Client): { key: string; checking: boolean; summary: string; results: HostCheckRow[] }[] {
  const state = stateOf(client), targets = state.editingTargets.length ? state.editingTargets : state.targets.length ? state.targets : [{ client, label: envLabel(client) }];
  const keys = [...new Set(targets.flatMap(target => [...stateOf(target.client).checks.keys()]))];
  return keys.map(key => {
    const results = targets.flatMap(target => {
      const check = stateOf(target.client).checks.get(key);
      return check ? [{ id: target.client.environmentId, label: target.label, status: check.status === 'pending' ? 'Checking…' : check.status === 'local' ? 'Already available locally' : check.status === 'failed' ? 'Failed' : 'Connected',
        platforms: check.status === 'connected' ? check.platforms.map(platform => `${str(platform.platform) === 'ios' ? 'iOS' : 'Android'} ${platform.available === true ? 'available' : 'unavailable'}`).join('   ') : '', error: check.status === 'failed' ? check.error : '' }] : [];
    });
    const failed = results.filter(result => result.status === 'Failed').length, checking = results.some(result => result.status === 'Checking…');
    return { key, checking, summary: checking ? 'Checking environments…' : failed ? `${failed} of ${targets.length} failed` : 'Connection checks passed', results };
  });
}

// ── Commands ───────────────────────────────────────────────────────────────
async function test(client: T3Client, native: Native, host: HostConfig): Promise<Check> {
  const state = stateOf(client), key = connectionKey(host);
  if (state.checks.get(key)?.status === 'pending') return state.checks.get(key)!;
  state.checks.set(key, { status: 'pending', platforms: [], tools: null, error: '' });
  let check: Check;
  try {
    if (!client.ready) throw new Error('Environment disconnected');
    const result = await client.restAccess(native).request('device.testHost', host as unknown as Obj);
    check = str(result.kind) === 'local' ? { status: 'local', platforms: [], tools: null, error: '' } : { status: 'connected', platforms: arr(result.platforms), tools: result.tools ? obj(result.tools) : null, error: '' };
  } catch (error) { if (letGo(error)) throw error; check = { status: 'failed', platforms: [], tools: null, error: error instanceof Error ? error.message : String(error) }; }
  state.checks.set(key, check);
  return check;
}
const fields = (input: Record<string, string>) => ({ id: str(input.id), label: str(input.label), target: str(input.target), identityFile: str(input.identityFile), port: str(input.port) });

/**
 * rest:hosts-* on one environment — open / edit / close the editor, test, save, remove, retry, refresh.
 * A save or remove answers the write's failure message, '' when it was written.
 */
async function singleDeviceHostsCommand(client: T3Client, native: Native, op: string, input: Record<string, string>, projectScope: boolean): Promise<string> {
  const state = stateOf(client), access = client.restAccess(native);
  if (op === 'hosts-close') { state.editor = null; return ''; }
  if (op === 'hosts-open' || op === 'hosts-edit') {
    if (projectScope) throw new ClientError('Device hosts belong to the environment. Choose All projects to manage them.');
    const settings = obj((await access.request('server.getConfig')).settings);
    const host = op === 'hosts-edit' ? hostsOf(settings).find(entry => entry.id === input.id) : { id: randomId(), label: '', target: '' };
    if (!host) throw new ClientError('That device host is no longer configured.');
    state.serial++;
    state.editor = { isNew: op === 'hosts-open', original: op === 'hosts-edit' ? host : null, host, serial: state.serial };
    return '';
  }
  if (op === 'hosts-test' || op === 'hosts-test-row') {
    const parsed = op === 'hosts-test-row' ? hostsOf(obj((await access.request('server.getConfig')).settings)).find(entry => entry.id === input.id) ?? 'That device host is no longer configured.'
      : parseHost({ ...fields(input), label: input.label?.trim() || str(input.target) });
    if (typeof parsed === 'string') throw new ClientError(parsed);
    await test(client, native, parsed);
    return '';
  }
  if (op === 'hosts-retry') {
    state.retrying = str(input.id);
    try { await access.request('device.list', { retryHostId: str(input.id) }, true); } finally { state.retrying = ''; }
    return '';
  }
  // Local simulators' Refresh: `device.list {}`, its failure left to the row's status (reportFailure: false).
  if (op === 'hosts-refresh') {
    try { await access.request('device.list', {}, true); } catch (error) { if (letGo(error)) throw error; }
    return '';
  }
  if (op === 'hosts-save' || op === 'hosts-remove') {
    if (projectScope) throw new ClientError('Device hosts belong to the environment. Choose All projects to manage them.');
    const settings = obj((await access.request('server.getConfig')).settings), hosts = hostsOf(settings);
    let next: HostConfig[];
    if (op === 'hosts-remove') {
      const host = hosts.find(entry => entry.id === input.id);
      if (!host) throw new ClientError('That device host is no longer configured.');
      next = updateDeviceHosts(hosts, host, true);
    } else {
      const parsed = parseHost(fields(input));
      if (typeof parsed === 'string') throw new ClientError(parsed);
      next = updateDeviceHosts(hosts, parsed, false, state.editor?.original ?? parsed);
    }
    try {
      const updated = await access.request('server.updateSettings', { patch: { deviceHosts: next } }, true);
      client.config = { ...client.config, settings: updated };
    } catch (error) {
      if (letGo(error)) throw error;
      return error instanceof Error ? error.message : String(error);
    }
    return '';
  }
  throw new ClientError(`Unknown settings action: ${op}`);
}

/** One scoped action fans out without replacing the other machines' host ids or unrelated hosts. */
export async function deviceHostsCommand(client: T3Client, native: Native, op: string, input: Record<string, string>, projectScope: boolean): Promise<string> {
  const state = stateOf(client);
  const available = state.targets.length ? state.targets : [{ client, label: envLabel(client), secondary: '', state: null }];
  const targets = input.environment ? available.filter(target => target.client.environmentId === input.environment)
    : state.editingTargets.length && !['hosts-open', 'hosts-edit', 'hosts-test-row', 'hosts-remove', 'hosts-retry'].includes(op) ? state.editingTargets : available;
  if (!targets.length) throw new ClientError('That environment is no longer selected.');
  if (op === 'hosts-close') { state.editor = null; state.editingTargets = []; return ''; }
  if (op === 'hosts-open' || op === 'hosts-edit') {
    await singleDeviceHostsCommand(targets[0]!.client, native, op, input, projectScope);
    state.editor = stateOf(targets[0]!.client).editor; state.editingTargets = targets;
    return '';
  }
  const original = state.editor?.original;
  const outcomes = await Promise.all(targets.map(async target => {
    let mapped = { ...input };
    if (original && (op === 'hosts-save' || op === 'hosts-remove')) {
      const host = hostsOf(obj(target.client.config.settings)).find(host => connectionKey(host) === connectionKey(original));
      if (host) mapped.id = host.id;
    } else if (targets.length > 1 && ['hosts-test-row', 'hosts-remove', 'hosts-retry'].includes(op)) {
      const firstHost = hostsOf(obj(targets[0]!.client.config.settings)).find(host => host.id === input.id);
      const host = firstHost && hostsOf(obj(target.client.config.settings)).find(host => connectionKey(host) === connectionKey(firstHost));
      if (!host) return null;
      mapped.id = host.id;
    }
    if (state.editor) stateOf(target.client).editor = state.editor;
    const failure = await singleDeviceHostsCommand(target.client, native, op, mapped, projectScope);
    return { target, host: hostsOf(obj(target.client.config.settings)).find(host => host.id === mapped.id), failure };
  }));
  const reached = outcomes.filter(outcome => outcome !== null);
  const named = (target: HostTarget) => target.secondary ? `${target.label} · ${target.secondary}` : target.label;
  if (op === 'hosts-test-row') {
    const host = reached.find(outcome => outcome.host)?.host;
    if (!host) return '';
    const failed = reached.flatMap(({ target, host }) => {
      const check = host ? stateOf(target.client).checks.get(connectionKey(host)) : undefined;
      return check?.status === 'failed' ? [`${named(target)}: ${check.error}`] : [];
    });
    pushToast(client, { kind: failed.length ? 'error' : 'success', title: failed.length ? `${host.label}: ${failed.length} of ${targets.length} environments failed` : `${host.label}: connection checks passed`,
      description: failed.length ? failed.join('\n') : 'Connected or already available locally on each selected environment.' });
    return '';
  }
  if (op === 'hosts-save' || op === 'hosts-remove') {
    const failed = reached.filter(outcome => outcome.failure).map(({ target, failure }) => `${named(target)}: ${failure}`);
    if (failed.length) {
      pushToast(client, { kind: 'error', title: op === 'hosts-remove' ? 'Device host not removed everywhere' : 'Device host not saved everywhere', description: failed.join('\n') });
      return '';
    }
  }
  if (op === 'hosts-save') { state.editor = null; state.editingTargets = []; for (const target of targets) stateOf(target.client).editor = null; }
  return '';
}
