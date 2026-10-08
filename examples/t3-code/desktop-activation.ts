// `t3 app <dir>` opens a project in the running app (20261005-app-activation). Ported from T3 Code
// (MIT, see LICENSE-T3), reference 1e2ecbd975: apps/web/src/desktopAppActivation.ts
// (handleDesktopAppActivationRequest), components/desktop/DesktopAppActivationCoordinator.tsx (when
// the window is ready, and what each step does), packages/contracts/src/desktopAppActivation.ts
// (the protocol) and packages/shared/src/desktopAppControl.ts (the socket address).
//
// The socket and the broker are native (modules/apple/T3AppControl.swift); this file is the
// window's half, the reference's renderer:
//  - `activationPrepare` (each snapshot read) reports ready while the primary environment is
//    connected, its config is known and its shell snapshot is loaded, and not ready otherwise
//    (a disconnect, the Local environment switched off). A new client (a reload) reports with a new
//    token, so the native side drops what the old one was handed, as a renderer navigation does.
//  - The native side hands one request at a time to a ready window and announces
//    `t3.activation`. `activationOpen` (the shell view) turns a newly handed request into the
//    window's open request, beside a clicked notification's: the root's `shellOpen` task then
//    leaves Settings and the utility pages, as the reference's route change does, and sends
//    `activation:open`.
//  - `activationOps` runs that op: it reads the request back (a request the command line gave up
//    on, or that expired, is gone and opens nothing), finds or adds the project on the primary,
//    opens a new draft there, and answers with `activationComplete`.
// Where the reference's renderer reads every environment from one store, this client holds the
// focused environment and keeps the others in the fleet (settings-b-fleet.ts): a primary in the
// fleet gets the project through its own transport, then becomes the focus with the new draft.
import type { T3Client } from './client';
import { applyShell, obj, str, type Obj } from './domain';
import { ClientError, bridgeReply, type Files, type Native } from './protocol';
import type { OpOut } from './client-ops';
import { EnvironmentFleet, fleet, type FleetEntry } from './settings-b-fleet';
import { focusedOnPrimary, primary, type LocalPrimary } from './local-primary';
import { findProjectByPath, inferProjectTitleFromPath } from './project-paths';
import { ensureDraftThreadId } from './r7-handoff-thread';
import { settle } from './r10-connect-timing';
import { letGo } from './let-go';

// ── The protocol (packages/contracts/src/desktopAppActivation.ts) ─────────
export const DESKTOP_APP_ACTIVATION_PROTOCOL_VERSION = 1;
export type DesktopAppActivationPlatform = 'darwin' | 'linux' | 'win32';
export type DesktopAppActivationRequest = { version: 1; requestId: string; type: 'open-workspace'; workspaceRoot: string; platform: DesktopAppActivationPlatform };
export type DesktopAppActivationErrorCode = 'invalid-request' | 'renderer-unavailable' | 'environment-unavailable' | 'platform-mismatch'
  | 'project-create-failed' | 'thread-open-failed' | 'request-timeout' | 'internal-error';
export type DesktopAppActivationSuccess = { version: 1; requestId: string; ok: true; projectId: string; threadId: string };
export type DesktopAppActivationFailure = { version: 1; requestId: string; ok: false; code: DesktopAppActivationErrorCode; message: string };
export type DesktopAppActivationResponse = DesktopAppActivationSuccess | DesktopAppActivationFailure;

const PLATFORMS: readonly string[] = ['darwin', 'linux', 'win32'];
const nonEmpty = (value: unknown): value is string => typeof value === 'string' && value.length > 0;
/** Schema.is(DesktopAppActivationRequest). */
export function parseActivationRequest(value: unknown): DesktopAppActivationRequest | null {
  const raw = obj(value);
  if (raw.version !== DESKTOP_APP_ACTIVATION_PROTOCOL_VERSION || !nonEmpty(raw.requestId) || raw.type !== 'open-workspace'
    || !nonEmpty(raw.workspaceRoot) || !PLATFORMS.includes(str(raw.platform))) return null;
  return { version: 1, requestId: raw.requestId, type: 'open-workspace', workspaceRoot: raw.workspaceRoot, platform: raw.platform as DesktopAppActivationPlatform };
}

/**
 * resolveDesktopAppControlAddress: the socket the app listens on and the CLI dials. The state
 * directory (`<T3 home>/userdata`) is hashed so a long T3 home stays under the Unix socket path
 * limit. `sha256` is the digest's hex; the app computes the same address natively
 * (T3AppControlAddress), and both are held to the reference's vectors.
 */
export function resolveDesktopAppControlAddress(input: { stateDir: string; platform: string; tempDir: string; userId: number | undefined;
  joinPath: (...segments: string[]) => string; sha256: (text: string) => string }): { address: string; directory: string | null } {
  const stateHash = input.sha256(input.stateDir).slice(0, 24);
  if (input.platform === 'win32') return { address: `\\\\.\\pipe\\t3code-app-${stateHash}`, directory: null };
  const userKey = input.userId === undefined ? stateHash.slice(0, 12) : input.userId;
  const directory = input.joinPath(input.tempDir, `t3code-${userKey}`);
  return { address: input.joinPath(directory, `${stateHash}.sock`), directory };
}

// ── apps/web/src/desktopAppActivation.ts ─────────────────────────────────
export interface DesktopAppActivationProject { id: string; environmentId: string; workspaceRoot: string }
export interface DesktopAppActivationTarget { environmentId: string; platform: string }
export interface DesktopAppActivationDependencies {
  getTarget: () => DesktopAppActivationTarget | null;
  findProject: (environmentId: string, workspaceRoot: string) => DesktopAppActivationProject | null;
  createProject: (environmentId: string, workspaceRoot: string) => Promise<string>;
  waitForProject: (projectRef: { environmentId: string; projectId: string }) => Promise<void>;
  openThread: (projectRef: { environmentId: string; projectId: string }) => Promise<{ threadId: string } | null>;
}

export function activationFailure(requestId: string, code: DesktopAppActivationErrorCode, message: string): DesktopAppActivationFailure {
  return { version: 1, requestId, ok: false, code, message };
}
const environmentOs = (platform: DesktopAppActivationPlatform) => (platform === 'win32' ? 'windows' : platform);
const errorMessage = (error: unknown, fallback: string) => (error instanceof Error && error.message.trim().length > 0 ? error.message : fallback);

export async function handleDesktopAppActivationRequest(request: DesktopAppActivationRequest, dependencies: DesktopAppActivationDependencies): Promise<DesktopAppActivationResponse> {
  const target = dependencies.getTarget();
  if (target === null) return activationFailure(request.requestId, 'environment-unavailable', "The desktop app's primary local environment is not connected.");
  const requestPlatform = environmentOs(request.platform);
  if (requestPlatform !== target.platform) {
    return activationFailure(request.requestId, 'platform-mismatch',
      `The command path is for ${requestPlatform}, but the desktop app's primary environment uses ${target.platform}. Cross-platform path mapping is not supported.`);
  }
  let projectId = dependencies.findProject(target.environmentId, request.workspaceRoot)?.id ?? null;
  if (projectId === null) {
    try {
      projectId = await dependencies.createProject(target.environmentId, request.workspaceRoot);
      await dependencies.waitForProject({ environmentId: target.environmentId, projectId });
    } catch (error) {
      if (letGo(error)) throw error;
      return activationFailure(request.requestId, 'project-create-failed', errorMessage(error, 'T3 Code could not add the project.'));
    }
  }
  try {
    const opened = await dependencies.openThread({ environmentId: target.environmentId, projectId });
    if (opened === null) return activationFailure(request.requestId, 'thread-open-failed', 'T3 Code could not open a new thread for the project.');
    return { version: 1, requestId: request.requestId, ok: true, projectId, threadId: opened.threadId };
  } catch (error) {
    if (letGo(error)) throw error;
    return activationFailure(request.requestId, 'thread-open-failed', errorMessage(error, 'T3 Code could not open a new thread for the project.'));
  }
}

// ── The window's side (DesktopAppActivationCoordinator.tsx) ──────────────
type Focus = Pick<T3Client, 'environmentId' | 'origin' | 'connection' | 'config' | 'shellLoaded'>;
export type ActivationTarget = DesktopAppActivationTarget & { entry: FleetEntry | null };
const platformOf = (config: Obj) => str(obj(obj(config.environment).platform).os);
const knows = (config: Obj) => !!str(obj(config.environment).environmentId);

/**
 * The primary when it is connected with its config and shell snapshot: the focus, else its fleet
 * entry. Null while the Local environment is off, the primary is not connected, or not loaded yet.
 */
export function activationTarget(client: Focus, source: EnvironmentFleet = fleet, local: LocalPrimary = primary): ActivationTarget | null {
  if (local.disabled || !local.target) return null;
  if (client.connection === 'connected' && client.environmentId && focusedOnPrimary(client, local) && client.shellLoaded && knows(client.config)) {
    return { environmentId: client.environmentId, platform: platformOf(client.config), entry: null };
  }
  for (const entry of source.entries.values()) {
    if (entry.primary && entry.phase === 'connected' && entry.synchronized === entry.generation && knows(entry.config)) {
      return { environmentId: entry.environmentId, platform: platformOf(entry.config), entry };
    }
  }
  return null;
}

export const ACTIVATION_TOPIC = 't3.activation';
export const ACTIVATION_PREFIX = 'activation:';
export const ACTIVATION_OPEN = 'activation:open';
/** waitForProject's wait (state/entities.ts), polled every half second. */
export const PROJECT_WAIT_MS = 10_000, PROJECT_POLL_MS = 500;

type WindowState = { token: string; reported: boolean | null; seenOpened: string; seenRequest: string; serial: number; target: string };
const windows = new WeakMap<object, WindowState>();
/** This client's page token (Math.random is unavailable in data sources). */
const pageToken = () => Array.from(crypto.getRandomValues(new Uint8Array(8)), byte => byte.toString(16).padStart(2, '0')).join('');
function windowOf(client: object): WindowState {
  let state = windows.get(client);
  if (!state) windows.set(client, state = { token: pageToken(), reported: null, seenOpened: '', seenRequest: '', serial: 0, target: '' });
  return state;
}

/** Each snapshot read: tell the native side whether this window can open a project now (setReady). */
export async function activationPrepare(client: T3Client, native: Native | null | undefined, source: EnvironmentFleet = fleet, local: LocalPrimary = primary): Promise<void> {
  if (!native?.available) return;
  const state = windowOf(client), ready = activationTarget(client, source, local) !== null;
  if (state.reported === ready) return;
  try {
    const reply = await bridgeReply(native, { op: 'activationReady', ready, token: state.token });
    if (reply.ok) state.reported = ready;
  } catch (error) { if (letGo(error)) throw error; }
}

/**
 * The shell view's open request: a clicked notification's thread (T3Notifications `opened`) or a
 * request the native side handed this window, whichever came last, as `open:<n>` with its target.
 */
export async function activationOpen(client: object, native: Native | null | undefined, status: { opened: string; openedThread: string }): Promise<{ openRequest: string; openThreadId: string }> {
  const state = windowOf(client);
  let handed = '';
  if (native?.available) {
    native.watch(ACTIVATION_TOPIC);
    try {
      const reply = await bridgeReply(native, { op: 'activationStatus' });
      if (reply.ok) handed = str(obj(reply.value).dispatched);
    } catch (error) { if (letGo(error)) throw error; }
  }
  if (status.opened !== state.seenOpened) {
    state.seenOpened = status.opened;
    if (status.opened) { state.serial++; state.target = status.openedThread; }
  }
  if (handed && handed !== state.seenRequest) { state.seenRequest = handed; state.serial++; state.target = `${ACTIVATION_PREFIX}${handed}`; }
  return state.serial ? { openRequest: `open:${state.serial}`, openThreadId: state.target } : { openRequest: '', openThreadId: '' };
}

async function fleetCall(native: Native, entry: FleetEntry, request: Obj): Promise<Obj> {
  const generation = entry.generation;
  const reply = await bridgeReply(EnvironmentFleet.native(native, entry.key), { ...request, generation });
  if (!reply.ok) throw new ClientError(reply.error!.message, reply.error!.kind);
  if (reply.generation !== generation) throw new ClientError('The connection changed. Refresh before continuing.', 'stale');
  return obj(reply.value);
}

/** The reference's dependencies over this client: the primary focused, or in the fleet. */
export function activationDependencies(client: T3Client, native: Native, source: EnvironmentFleet = fleet, local: LocalPrimary = primary): DesktopAppActivationDependencies {
  let target: ActivationTarget | null = null;
  const projects = () => (target?.entry ? target.entry.shell.projects : client.shell.projects);
  const reread = async () => {
    if (target?.entry) target.entry.shell = applyShell(target.entry.shell, await fleetCall(native, target.entry, { op: 'http', path: '/api/orchestration/shell' }));
    else client.shell = applyShell(client.shell, await client.http(native, '/api/orchestration/shell'));
  };
  return {
    getTarget: () => (target = activationTarget(client, source, local)),
    findProject: (environmentId, workspaceRoot) => {
      const found = findProjectByPath(projects().map(project => ({ id: str(project.id), workspaceRoot: str(project.workspaceRoot) })), workspaceRoot);
      return found ? { id: found.id, environmentId, workspaceRoot: found.workspaceRoot } : null;
    },
    // projectEnvironment.create with reportFailure: false: a failure is the command's answer, not a toast.
    createProject: async (_environmentId, workspaceRoot) => {
      const [commandId, projectId] = await client.ids(native, 2);
      const payload = { type: 'project.create', commandId, projectId, title: inferProjectTitleFromPath(workspaceRoot), workspaceRoot,
        createWorkspaceRootIfMissing: false, defaultModelSelection: null };
      if (target?.entry) await fleetCall(native, target.entry, { op: 'request', method: 'projects.mutate', payload });
      else await client.rpc(native, 'projects.mutate', payload, true);
      return projectId!;
    },
    waitForProject: async ({ projectId }) => {
      const has = () => projects().some(project => project.id === projectId);
      for (let waited = 0; !has(); waited += PROJECT_POLL_MS) {
        if (waited >= PROJECT_WAIT_MS) throw new Error('The project did not appear in the desktop app.');
        if (waited > 0) await settle(native, PROJECT_POLL_MS);
        await reread().catch(error => { if (letGo(error)) throw error; });
      }
    },
    // useNewThreadHandler: the project's draft, focused, with its own thread id (newThreadId).
    openThread: async ({ environmentId, projectId }) => {
      if (!projects().some(project => project.id === projectId)) return null;
      const entry = target?.entry;
      if (entry) {
        client.local.selections[environmentId] = { projectId, threadId: '' };
        source.forget(entry.key);
        await native.later({ op: 'fleetStop', fleet: entry.key }).catch(() => undefined);
        const reply = await bridgeReply(native, { op: 'connect', origin: entry.origin, primary: true });
        if (!reply.ok) throw new Error(reply.error!.message);
        client.adoptStatus(obj(reply.value), reply.generation);
      } else await client.openProjectDraft(native, projectId);
      return { threadId: await ensureDraftThreadId(client, native, `${environmentId}:new:${projectId}`) };
    },
  };
}

/**
 * `activation:open` (the root's shellOpen task): the handed request, read back, handled and
 * answered. A request that is gone (cancelled, expired, already answered) opens nothing.
 */
export async function openActivation(client: T3Client, native: Native, requestId: string,
  dependencies: DesktopAppActivationDependencies = activationDependencies(client, native)): Promise<DesktopAppActivationResponse | null> {
  const handed = await bridgeReply(native, { op: 'activationRequest', requestId });
  const request = handed.ok ? parseActivationRequest(handed.value) : null;
  if (!request) return null;
  const response = await handleDesktopAppActivationRequest(request, dependencies);
  await bridgeReply(native, { op: 'activationComplete', response });
  return response;
}

/** The op group (client-ops.ts): `activation:open`, before the write check (it answers its own failures). */
export async function activationOps(this: T3Client, op: string, id: string, value: string, _n: number, native: Native, _storage: Files, out: OpOut): Promise<boolean> {
  if (op !== ACTIVATION_OPEN) return false;
  try { await openActivation(this, native, id.startsWith(ACTIVATION_PREFIX) ? id.slice(ACTIVATION_PREFIX.length) : id); }
  finally { Object.assign(out, { message: '', id, value }); }
  return true;
}
