// Integrations › Devices › EAS Simulator (MIT T3 Code 74541bab17, see LICENSE-T3:
// components/settings/EasDeviceHostsSettings.tsx and easDeviceHostsSettings.logic.ts).
// EAS hosts are the environment's `easDeviceHosts` setting, offered only when its device
// state says `supportsEasHosts`; the editor's account check is `device.easAccount`. A saved
// access token arrives redacted, and sending the redacted value back keeps it.
// exact2: the editor's fields live in the dialog (settings-eas-hosts.contract). Each account
// check carries the dialog's check number, and an answer shows only while that number is the
// dialog's latest: a later check or a token edit makes it stale, as the reference's
// `latestCheck` does.
import type { T3Client } from '../client/client';
import { arr, decodeEasAccount, obj, str, type Obj } from '../client/domain';
import { ClientError, type Native } from '../client/protocol';
import { letGo } from '../client/let-go';
import { pushToast } from '../shell/toast';
import { randomId } from './settings-a-hosts';

export type EasHost = { kind: 'eas'; id: string; label: string; appId?: string; accessToken: string; maxSessionMinutes: number; idleMinutes: number; maxConcurrentSessions: number };

/** EasDeviceHostConfig's upper bounds; every limit starts at 1. */
export const EAS_LIMITS = { maxConcurrentSessions: 20, maxSessionMinutes: 115, idleMinutes: 60 } as const;

export const newEasDeviceHost = (id: string): EasHost => ({ kind: 'eas', id, label: 'EAS Simulator', accessToken: '', maxSessionMinutes: 30, idleMinutes: 10, maxConcurrentSessions: 2 });

const whole = (value: number, max: number) => Number.isInteger(value) && value >= 1 && value <= max;
/**
 * parseEasDeviceHostDraft over EasDeviceHostConfig, with the editor's reason when it fails: '' for a valid host.
 * The limits' ranges are on the fields; only idle minutes against session minutes has its own message.
 */
export function easHostProblem(host: EasHost): string {
  const valid = /^[a-zA-Z0-9][a-zA-Z0-9_-]*$/.test(host.id) && host.id !== 'local' && host.id.length <= 128 && host.label.trim() !== ''
    && whole(host.maxConcurrentSessions, EAS_LIMITS.maxConcurrentSessions) && whole(host.maxSessionMinutes, EAS_LIMITS.maxSessionMinutes)
    && whole(host.idleMinutes, EAS_LIMITS.idleMinutes) && host.idleMinutes < host.maxSessionMinutes;
  if (valid) return '';
  return host.maxSessionMinutes > 0 && host.idleMinutes >= host.maxSessionMinutes ? 'Idle minutes must be smaller than session minutes.'
    : 'Give the host a label and limits within the ranges shown.';
}

/** Adds, replaces, or removes one EAS host. */
export function updateEasDeviceHosts(hosts: EasHost[], host: EasHost, remove: boolean): EasHost[] {
  if (remove) return hosts.filter(candidate => candidate.id !== host.id);
  return hosts.some(candidate => candidate.id === host.id) ? hosts.map(candidate => candidate.id === host.id ? host : candidate) : [...hosts, host];
}

export function easSessionLimits(host: EasHost): string {
  const sessions = host.maxConcurrentSessions === 1 ? '1 session' : `${host.maxConcurrentSessions} sessions`;
  return `Up to ${sessions} at once, each stopped after ${host.maxSessionMinutes} min or ${host.idleMinutes} idle min`;
}

const count = (value: unknown, fallback: number) => typeof value === 'number' ? value : fallback;
/** The `easDeviceHosts` setting with EasDeviceHostConfig's decoding defaults. */
export const easHostsOf = (settings: Obj): EasHost[] => arr(settings.easDeviceHosts).map(host => ({
  kind: 'eas', id: str(host.id), label: str(host.label), ...(str(host.appId) ? { appId: str(host.appId) } : {}), accessToken: str(host.accessToken),
  maxSessionMinutes: count(host.maxSessionMinutes, 30), idleMinutes: count(host.idleMinutes, 10), maxConcurrentSessions: count(host.maxConcurrentSessions, 2) }));

// ── Views ──────────────────────────────────────────────────────────────────
type EasApp = { id: string; fullName: string };
type Account = { seq: number; status: 'checking' | 'signed-in' | 'failed'; message: string; apps: EasApp[] };
type Editor = { serial: number; isNew: boolean; host: EasHost; account: Account; mount: number };
type EasState = { editor: Editor | null; serial: number };
const states = new WeakMap<T3Client, EasState>();
const stateOf = (client: T3Client) => { let state = states.get(client); if (!state) states.set(client, state = { editor: null, serial: 0 }); return state; };

export type EasHostRow = { id: string; label: string; limits: string; first: boolean };
export type EasHostsView = { supported: boolean; disabled: boolean; editing: boolean; hosts: EasHostRow[] };
/** The row under Device hosts; `disabled` is the reference's `!canConfigure || projectScope || !environmentId`. */
export function easHostsView(client: T3Client, settings: Obj, deviceState: Obj | null, projectScope: boolean, writable: boolean, connected: boolean): EasHostsView {
  return { supported: deviceState?.supportsEasHosts === true, disabled: !writable || projectScope || !connected, editing: stateOf(client).editor !== null,
    hosts: easHostsOf(settings).map((host, index) => ({ id: host.id, label: host.label, limits: easSessionLimits(host), first: index === 0 })) };
}

export type EasEditorView = { open: boolean; serial: string; isNew: boolean; title: string; id: string; label: string; tokenSaved: boolean; appId: string;
  sessions: string; sessionMinutes: string; idleMinutes: string; accountSeq: number; accountStatus: string; accountMessage: string; apps: EasApp[] };
/** The Add/Edit EAS host dialog: the edited host's fields and the newest account answer. */
export function easEditorView(client: T3Client): EasEditorView {
  const editor = stateOf(client).editor;
  if (!editor) return { open: false, serial: '', isNew: false, title: '', id: '', label: '', tokenSaved: false, appId: '', sessions: '', sessionMinutes: '', idleMinutes: '',
    accountSeq: 0, accountStatus: '', accountMessage: '', apps: [] };
  const { host, account } = editor;
  return { open: true, serial: String(editor.serial), isNew: editor.isNew, title: editor.isNew ? 'Add EAS host' : 'Edit EAS host', id: host.id, label: host.label,
    tokenSaved: host.accessToken !== '', appId: host.appId ?? '', sessions: String(host.maxConcurrentSessions), sessionMinutes: String(host.maxSessionMinutes),
    idleMinutes: String(host.idleMinutes), accountSeq: account.seq, accountStatus: account.status, accountMessage: account.message,
    apps: account.status === 'signed-in' ? account.apps : [] };
}

// ── Commands ───────────────────────────────────────────────────────────────
const CREDENTIAL_SOURCES: Record<string, string> = { settings: 'the access token saved here', env: 'EXPO_TOKEN on the server', 'eas-login': "the server's eas login" };
const PROJECT_SCOPE = 'EAS hosts belong to the environment. Choose All projects to manage them.';
const failure = (error: unknown) => error instanceof Error ? error.message : String(error);

/** The dialog's draft as the host Save writes: a saved token goes back redacted, a cleared one as ''. */
export function easDraft(editor: { host: EasHost }, input: Record<string, string>): EasHost {
  const appId = str(input.appId).trim();
  return { kind: 'eas', id: editor.host.id, label: str(input.label).trim(), ...(appId ? { appId } : {}),
    accessToken: input.tokenKind === 'saved' ? editor.host.accessToken : input.tokenKind === 'new' ? str(input.token) : '',
    maxConcurrentSessions: Number(input.sessions || 0), maxSessionMinutes: Number(input.sessionMinutes || 0), idleMinutes: Number(input.idleMinutes || 0) };
}

/** restlocal:eas-check — device.easAccount for the dialog's credential; no pending state, as the reference checks while its fields stay open. */
export async function easAccountCheck(client: T3Client, native: Native, input: Record<string, string>): Promise<string> {
  const editor = stateOf(client).editor;
  if (!editor || String(editor.serial) !== input.serial) return '';
  const seq = Number(input.seq);
  // Check 1 is a dialog's first: one that mounts again (Settings closed and reopened over the open editor) counts
  // from 1, so its checks start over and answers to the earlier dialog's checks are dropped.
  if (seq === 1) { editor.mount++; editor.account = { seq: 0, status: 'checking', message: '', apps: [] }; }
  const mount = editor.mount;
  // A cleared token is sent as empty, so the check does not fall back to the saved one that Save is about to drop.
  const payload: Obj = { ...(input.tokenKind === 'saved' && !editor.isNew ? { hostId: editor.host.id } : {}),
    ...(input.tokenKind === 'new' ? { accessToken: str(input.token) } : input.tokenKind === 'cleared' ? { accessToken: '' } : {}) };
  let account: Account;
  try {
    const result = decodeEasAccount(await client.restAccess(native).request('device.easAccount', payload));
    account = { seq, status: 'signed-in', message: `Signed in as ${result.signedInAs} with ${CREDENTIAL_SOURCES[result.credentialSource] ?? result.credentialSource}.`, apps: result.apps };
  } catch (error) {
    if (letGo(error)) throw error;
    account = { seq, status: 'failed', message: failure(error), apps: [] };
  }
  const current = stateOf(client).editor;
  if (current === editor && current.mount === mount && seq >= current.account.seq) current.account = account;
  return '';
}

async function writeHosts(client: T3Client, native: Native, change: (hosts: EasHost[]) => EasHost[], failedTitle: string): Promise<boolean> {
  const access = client.restAccess(native);
  try {
    const settings = obj((await access.request('server.getConfig')).settings);
    const updated = await access.request('server.updateSettings', { patch: { easDeviceHosts: change(easHostsOf(settings)) } }, true);
    client.config = { ...client.config, settings: updated };
    return true;
  } catch (error) {
    if (letGo(error)) throw error;
    pushToast(client, { kind: 'error', title: failedTitle, description: failure(error) });
    return false;
  }
}

/** rest:eas-* — open / edit / close the editor, save, remove. */
export async function easHostsCommand(client: T3Client, native: Native, op: string, input: Record<string, string>, projectScope: boolean): Promise<string> {
  const state = stateOf(client);
  if (op === 'eas-close') { state.editor = null; return ''; }
  if (projectScope) throw new ClientError(PROJECT_SCOPE);
  if (op === 'eas-open' || op === 'eas-edit') {
    const host = op === 'eas-open' ? newEasDeviceHost(randomId()) : easHostsOf(obj(client.config.settings)).find(entry => entry.id === input.id);
    if (!host) throw new ClientError('That EAS host is no longer configured.');
    // The editor checks the account as it opens: its first answer is the check its dialog sends on mount (seq 1).
    state.editor = { serial: ++state.serial, isNew: op === 'eas-open', host, account: { seq: 0, status: 'checking', message: '', apps: [] }, mount: 0 };
    return '';
  }
  if (op === 'eas-save') {
    const editor = state.editor;
    if (!editor || String(editor.serial) !== input.serial) throw new ClientError('That EAS host editor is no longer open.');
    const host = easDraft(editor, input), problem = easHostProblem(host);
    if (problem) throw new ClientError(problem);
    if (await writeHosts(client, native, hosts => updateEasDeviceHosts(hosts, host, false), 'EAS host not saved') && state.editor === editor) state.editor = null;
    return '';
  }
  if (op === 'eas-remove') {
    const host = easHostsOf(obj(client.config.settings)).find(entry => entry.id === input.id);
    if (!host) throw new ClientError('That EAS host is no longer configured.');
    await writeHosts(client, native, hosts => updateEasDeviceHosts(hosts, host, true), 'EAS host not removed');
    return '';
  }
  throw new ClientError(`Unknown settings action: ${op}`);
}
