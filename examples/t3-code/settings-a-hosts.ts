// Integrations → Device hosts (lane settings-a). Reference DeviceHostsSettings.tsx,
// DeviceHostEditor.tsx, deviceHostsSettings.logic.ts, deviceHostConnectionChecks.ts
// and DeviceToolVersions.tsx. Hosts are the environment's `deviceHosts` setting;
// a connection check is `device.testHost`, a failed host's retry is
// `device.list { retryHostId }`, and the row's state is `device.list`'s
// hostStatuses. This client edits one environment (the connected one).
import type { T3Client } from './client';
import { arr, obj, str, type Obj } from './domain';
import { ClientError, type Native } from './protocol';
import { pushToast } from './toast';

export type HostConfig = { id: string; label: string; target: string; identityFile?: string; port?: number };
type Check = { status: 'pending' | 'local' | 'connected' | 'failed'; platforms: Obj[]; tools: Obj | null; error: string };
type Editor = { isNew: boolean; original: HostConfig | null; host: HostConfig; serial: number };
type HostsState = { editor: Editor | null; checks: Map<string, Check>; serial: number; retrying: string };
const states = new WeakMap<T3Client, HostsState>();
const stateOf = (client: T3Client) => { let state = states.get(client); if (!state) states.set(client, state = { editor: null, checks: new Map(), serial: 0, retrying: '' }); return state; };

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
const hostsOf = (settings: Obj): HostConfig[] => arr(settings.deviceHosts).map(host => ({ id: str(host.id), label: str(host.label), target: str(host.target),
  ...(str(host.identityFile) ? { identityFile: str(host.identityFile) } : {}), ...(typeof host.port === 'number' ? { port: host.port } : {}) }));
// randomUUID's shape from crypto.getRandomValues (Math.random and Date.now are unavailable in data sources).
const randomId = () => { const bytes = crypto.getRandomValues(new Uint8Array(16)); return Array.from(bytes, (byte, index) => `${[4, 6, 8, 10].includes(index) ? '-' : ''}${byte.toString(16).padStart(2, '0')}`).join(''); };
const envLabel = (client: T3Client) => str(obj(client.config.environment).label, 'this environment');

// ── Views ──────────────────────────────────────────────────────────────────
type Platform = { id: string; label: string };
type ToolLine = { id: string; name: string; running: string; required: string; installed: string };
export type HostRow = { id: string; label: string; target: string; platforms: Platform[]; versions: string; tools: ToolLine[]; toolsKnown: boolean; owner: string; toolError: string;
  local: boolean; error: string; progress: string; retry: boolean; retrying: boolean; menuId: string };
export type DeviceHostsView = { available: boolean; canAdd: boolean; message: string; environmentLabel: string; hosts: HostRow[] };
const toolLines = (tools: Obj | null): ToolLine[] => tools ? [['hub', 'Device hub'], ['agent', 'Agent device']].map(([id, name]) => {
  const tool = obj(tools[id!]);
  return { id: id!, name: name!, running: str(tool.runningVersion) || 'Not running', required: str(tool.requiredVersion), installed: (Array.isArray(tool.installedVersions) ? tool.installedVersions.map(String) : []).join(', ') || 'None' };
}) : [];
const platformsOf = (platforms: Obj[]): Platform[] => platforms.filter(platform => platform.available === true)
  .map(platform => str(platform.platform) === 'ios' ? { id: 'ios', label: 'iOS available' } : { id: 'android', label: 'Android available' });

export function deviceHostsView(client: T3Client, settings: Obj, deviceState: Obj | null, projectScope: boolean, connected: boolean): DeviceHostsView {
  const state = stateOf(client), label = envLabel(client);
  if (!connected) return { available: false, canAdd: false, message: 'Connect a selected environment to manage device hosts.', environmentLabel: label, hosts: [] };
  const summaries = arr(deviceState?.hosts), statuses = obj(deviceState?.hostStatuses);
  const hosts = hostsOf(settings).map(host => {
    const summary = summaries.find(entry => str(entry.id) === host.id) ?? null, status = obj(statuses[host.id]), check = state.checks.get(connectionKey(host)) ?? null;
    const platforms = check?.status === 'connected' ? check.platforms : arr(summary?.platforms);
    const progress = check?.status === 'pending' ? 'Checking connection…' : str(status.status) === 'installing' ? 'Installing device support…' : str(status.status) === 'starting' ? 'Connecting…' : '';
    const error = check?.status === 'failed' ? check.error : check?.status === 'local' ? '' : str(status.status) === 'failed' ? str(status.detail) : '';
    const tools = summary?.tools ? obj(summary.tools) : check?.status === 'connected' ? check.tools : null;
    return { id: host.id, label: host.label, target: host.target, platforms: platformsOf(platforms), versions: str(summary?.toolInspectionError) ? 'Versions unavailable' : 'Versions',
      tools: toolLines(tools), toolsKnown: tools !== null, owner: label, toolError: str(summary?.toolInspectionError), local: check?.status === 'local', error, progress,
      retry: str(status.status) === 'failed' && deviceState?.supportsHostRetry === true && str(deviceState?.hostStatus) !== 'disabled', retrying: state.retrying === host.id, menuId: `device-host-menu-${host.id}` };
  });
  return { available: true, canAdd: !projectScope && state.editor === null, message: '', environmentLabel: label, hosts };
}

export type HostCheckRow = { id: string; label: string; status: string; platforms: string; error: string };
export type HostEditorView = { open: boolean; title: string; description: string; serial: string; id: string; label: string; target: string; identityFile: string; port: string; optionsOpen: boolean;
  checkKey: string; checking: boolean; checked: boolean; summary: string; results: HostCheckRow[] };
/** The Add/Edit device host dialog; its fields start from the edited host (the draft lives in the dialog). */
export function hostEditorView(client: T3Client): HostEditorView {
  const editor = stateOf(client).editor, label = envLabel(client);
  const blank: HostEditorView = { open: false, title: '', description: '', serial: '', id: '', label: '', target: '', identityFile: '', port: '', optionsOpen: false, checkKey: '', checking: false, checked: false, summary: 'Check access before saving', results: [] };
  if (!editor) return blank;
  const host = editor.host;
  return { ...blank, open: true, title: editor.isNew ? 'Add device host' : 'Edit device host', description: `Connect from ${label}. Hosts on the same machine are skipped.`, serial: String(editor.serial),
    id: host.id, label: host.label, target: host.target, identityFile: host.identityFile ?? '', port: host.port === undefined ? '' : String(host.port), optionsOpen: host.port !== undefined || host.identityFile !== undefined };
}
/** The check shown under the dialog's Test connection for one connection key. */
export function hostChecks(client: T3Client): { key: string; checking: boolean; summary: string; results: HostCheckRow[] }[] {
  const label = envLabel(client), environmentId = client.environmentId;
  return [...stateOf(client).checks].map(([key, check]) => ({ key, checking: check.status === 'pending',
    summary: check.status === 'pending' ? 'Checking environments…' : check.status === 'failed' ? '1 of 1 failed' : 'Connection checks passed',
    results: [{ id: environmentId, label, status: check.status === 'pending' ? 'Checking…' : check.status === 'local' ? 'Already available locally' : check.status === 'failed' ? 'Failed' : 'Connected',
      platforms: check.status === 'connected' ? check.platforms.map(platform => `${str(platform.platform) === 'ios' ? 'iOS' : 'Android'} ${platform.available === true ? 'available' : 'unavailable'}`).join('   ') : '',
      error: check.status === 'failed' ? check.error : '' }] }));
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
  } catch (error) { check = { status: 'failed', platforms: [], tools: null, error: error instanceof Error ? error.message : String(error) }; }
  state.checks.set(key, check);
  return check;
}
const fields = (input: Record<string, string>) => ({ id: str(input.id), label: str(input.label), target: str(input.target), identityFile: str(input.identityFile), port: str(input.port) });

/** rest:hosts-* — open / edit / close the editor, test, save, remove, retry. */
export async function deviceHostsCommand(client: T3Client, native: Native, op: string, input: Record<string, string>, projectScope: boolean): Promise<string> {
  const state = stateOf(client), access = client.restAccess(native);
  if (op === 'hosts-close') { state.editor = null; return ''; }
  if (op === 'hosts-open' || op === 'hosts-edit') {
    if (projectScope) throw new ClientError('Device hosts belong to the environment. Choose All projects to manage them.');
    const settings = await access.request('server.getSettings');
    const host = op === 'hosts-edit' ? hostsOf(settings).find(entry => entry.id === input.id) : { id: randomId(), label: '', target: '' };
    if (!host) throw new ClientError('That device host is no longer configured.');
    state.serial++;
    state.editor = { isNew: op === 'hosts-open', original: op === 'hosts-edit' ? host : null, host, serial: state.serial };
    return '';
  }
  if (op === 'hosts-test' || op === 'hosts-test-row') {
    const parsed = op === 'hosts-test-row' ? hostsOf(await access.request('server.getSettings')).find(entry => entry.id === input.id) ?? 'That device host is no longer configured.'
      : parseHost({ ...fields(input), label: input.label?.trim() || str(input.target) });
    if (typeof parsed === 'string') throw new ClientError(parsed);
    const check = await test(client, native, parsed);
    if (op === 'hosts-test-row') {
      const failed = check.status === 'failed';
      pushToast(client, { kind: failed ? 'error' : 'success', title: failed ? `${parsed.label}: 1 of 1 environments failed` : `${parsed.label}: connection checks passed`,
        description: failed ? `Could not connect from ${envLabel(client)}.` : 'Connected or already available locally on each selected environment.' });
    }
    return '';
  }
  if (op === 'hosts-retry') {
    state.retrying = str(input.id);
    try { await access.request('device.list', { retryHostId: str(input.id) }, true); } finally { state.retrying = ''; }
    return '';
  }
  if (op === 'hosts-save' || op === 'hosts-remove') {
    if (projectScope) throw new ClientError('Device hosts belong to the environment. Choose All projects to manage them.');
    const settings = await access.request('server.getSettings'), hosts = hostsOf(settings);
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
    } catch {
      pushToast(client, { kind: 'error', title: 'Device hosts not saved on all environments', description: `Could not update ${envLabel(client)}.` });
      return '';
    }
    if (op === 'hosts-save') state.editor = null;
    return '';
  }
  throw new ClientError(`Unknown settings action: ${op}`);
}
