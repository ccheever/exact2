// Remote open-in-editor (MIT reference, see LICENSE-T3: apps/web/src/remoteOpen.ts,
// components/chat/OpenInPicker.tsx and OpenInPicker.logic.ts, ChatMarkdown.tsx
// canUseMarkdownFileShellActions, apps/desktop/src/electron/ElectronShell.ts
// parseSafeExternalUrl, at 1e2ecbd975). When this client is not on the environment's
// machine, "Open" hands the OS a `vscode://vscode-remote/ssh-remote+…` deep link (the
// local editor connects over SSH) instead of running an editor on the environment's host.
// Host precedence: the SSH alias of a saved SSH environment beats the server's advertised
// `remoteOpenTargets` (tailnet name first, then mDNS). The editors offered are the
// remote-capable ones this Mac has (T3RemoteEditors.swift probes them), else VS Code.
//
// The clone has no bundled server: a loopback environment stands in for the reference's
// primary (README), and an SSH environment is a loopback tunnel named in the saved SSH
// targets (T3Ssh.swift), so the target is read from those targets before the origin.
import { arr, obj, str } from './domain';
import { bridgeReply, type Native } from './protocol';
import { buildRemoteOpenUrl, REMOTE_CAPABLE_EDITOR_IDS, remoteSchemeForEditor } from './editors';
import { isLoopback, trimOrigin } from './settings-b-fleet';
import { sshTargets } from './settings-b-ssh';
import type { T3Client } from './client';

export type RemoteOpenHost = { kind: 'ssh-alias' | 'tailscale' | 'mdns'; host: string };
export type RemoteOpenState = { mode: 'local-exec' } | { mode: 'remote-links'; host: RemoteOpenHost } | { mode: 'remote-unavailable' };
export type RemoteOpenMode = RemoteOpenState['mode'];
/** ConnectionTarget's tags, as far as the clone has them (no relay, no WSL). */
export type RemoteTarget = { kind: 'primary'; httpBaseUrl: string } | { kind: 'ssh' } | { kind: 'bearer' } | { kind: 'relay' } | { kind: 'desktop-local' };

const LOCAL_EXEC: RemoteOpenState = { mode: 'local-exec' };
const REMOTE_UNAVAILABLE: RemoteOpenState = { mode: 'remote-unavailable' };

function hostnameOf(url: string): string | null {
  try { return new URL(url).hostname; } catch { return null; }
}
const loopbackHost = (hostname: string) => /^(localhost|127(?:\.\d{1,3}){3}|\[::1\]|::1)$/i.test(hostname);

/** resolveRemoteOpenState. */
export function resolveRemoteOpenState(input: { target: RemoteTarget | null; sshAlias: string | null; remoteOpenTargets: { kind: string; host: string }[] | undefined; isDesktopRenderer: boolean }): RemoteOpenState {
  const { target } = input;
  // No catalog entry: keep the exec behavior rather than guessing.
  if (target === null) return LOCAL_EXEC;
  if (target.kind === 'primary') {
    // The desktop app's own primary is always this machine; a browser on a loopback primary is too.
    if (input.isDesktopRenderer) return LOCAL_EXEC;
    const hostname = hostnameOf(target.httpBaseUrl);
    if (hostname !== null && loopbackHost(hostname)) return LOCAL_EXEC;
  } else if (target.kind === 'desktop-local') {
    return LOCAL_EXEC;
  }
  if (input.sshAlias !== null && input.sshAlias.length > 0) return { mode: 'remote-links', host: { kind: 'ssh-alias', host: input.sshAlias } };
  const advertised = input.remoteOpenTargets?.[0];
  if (advertised !== undefined) return { mode: 'remote-links', host: { kind: advertised.kind === 'mdns' ? 'mdns' : 'tailscale', host: advertised.host } };
  return REMOTE_UNAVAILABLE;
}

/** shouldShowOpenInPicker: the primary always; other environments only outside local-exec (a WSL backend keeps it hidden). */
export function shouldShowOpenInPicker(input: { activeProjectName: string | undefined; activeThreadEnvironmentId: string; primaryEnvironmentId: string | null; remoteOpenMode: RemoteOpenMode }): boolean {
  if (!input.activeProjectName) return false;
  if (input.primaryEnvironmentId !== null && input.activeThreadEnvironmentId === input.primaryEnvironmentId) return true;
  return input.remoteOpenMode !== 'local-exec';
}

/** canUseMarkdownFileShellActions: a Markdown file link gets editor actions only where Open runs on this machine. */
export function canUseMarkdownFileShellActions(environmentId: string | null, remoteOpenMode: RemoteOpenMode, isRemoteOpenResolved: boolean): boolean {
  return environmentId !== null && isRemoteOpenResolved && remoteOpenMode === 'local-exec';
}

// ── Safe external URLs (ElectronShell.ts parseSafeExternalUrl; Swift's T3RemoteEditors.safeExternalUrl is the gate) ──
const SAFE_WEB_PROTOCOLS = new Set(['http:', 'https:']);
const REMOTE_EDITOR_PROTOCOLS = new Set(REMOTE_CAPABLE_EDITOR_IDS.flatMap(id => { const scheme = remoteSchemeForEditor(id); return scheme ? [`${scheme}:`] : []; }));
// Zed's host sits in the first path segment, so it needs its own userinfo ban.
const ZED_SSH_PATHNAME = /^\/[^/@:]+\/.*$/;
function isRemoteEditorUrl(url: URL): boolean {
  return REMOTE_EDITOR_PROTOCOLS.has(url.protocol) && url.username.length === 0 && url.password.length === 0
    && (url.protocol === 'zed:' ? url.host === 'ssh' && ZED_SSH_PATHNAME.test(url.pathname)
      : url.host === 'vscode-remote' && url.pathname.startsWith('/ssh-remote+') && url.pathname.length > '/ssh-remote+'.length);
}
export function parseSafeExternalUrl(raw: unknown): string | null {
  if (typeof raw !== 'string') return null;
  try {
    const url = new URL(raw);
    return SAFE_WEB_PROTOCOLS.has(url.protocol) || isRemoteEditorUrl(url) ? url.href : null;
  } catch { return null; }
}

// ── Remote-capable editors (useRemoteCapableEditors) ──
export const REMOTE_FALLBACK_EDITORS = ['vscode'];
/** The probe's answer kept to remote-capable editors, VS Code when none resolve. */
export function remoteEditorsFrom(ids: unknown[]): string[] {
  const capable = ids.filter((id): id is string => typeof id === 'string' && REMOTE_CAPABLE_EDITOR_IDS.includes(id));
  return capable.length > 0 ? capable : REMOTE_FALLBACK_EDITORS;
}
let probed: string[] | null = null;
/** Probed once per process, as the reference caches it; a failed probe settles on the fallback. */
export async function remoteCapableEditors(native: Native | null | undefined): Promise<string[]> {
  if (probed) return probed;
  if (!native?.available) return REMOTE_FALLBACK_EDITORS;
  try {
    const reply = await bridgeReply(native, { op: 'remoteEditorsProbe' });
    const ids = obj(reply.value).editors;
    probed = reply.ok && Array.isArray(ids) ? remoteEditorsFrom(ids) : REMOTE_FALLBACK_EDITORS;
  } catch { probed = REMOTE_FALLBACK_EDITORS; }
  return probed;
}
export function resetRemoteEditorsForTests(): void { probed = null; aliases = null; hintSeen = null; }

// ── The one-time "Opens over SSH" hint (useRemoteOpenHint), kept by the module per device ──
let hintSeen: boolean | null = null;
async function remoteHintSeen(native: Native): Promise<boolean> {
  if (hintSeen !== null) return hintSeen;
  try { hintSeen = obj((await bridgeReply(native, { op: 'remoteEditorsHint' })).value).seen === true; } catch { hintSeen = false; }
  return hintSeen;
}
async function markRemoteHintSeen(native: Native): Promise<void> {
  hintSeen = true;
  await bridgeReply(native, { op: 'remoteEditorsHint', seen: true }).catch(() => undefined);
}

// ── The focused environment's target ──
let aliases: Record<string, string> | null = null;
/** The saved SSH environments' aliases by loopback origin; read again after an SSH environment is added. */
export async function loadSshAliases(native: Native | null | undefined, fresh = false): Promise<Record<string, string>> {
  if (aliases && !fresh) return aliases;
  const targets = await sshTargets(native);
  aliases = Object.fromEntries(Object.entries(targets).map(([origin, target]) => [trimOrigin(origin), target.alias]));
  return aliases;
}
export function rememberSshAlias(origin: string, alias: string): void {
  aliases = { ...(aliases ?? {}), [trimOrigin(origin)]: alias };
}

export type RemoteOpen = { state: RemoteOpenState; resolved: boolean; primary: boolean; label: string };
/** useRemoteOpenResolution for the focused environment, from what is known now (sync: the keyboard dispatch reads it). */
export function remoteOpenFor(client: T3Client): RemoteOpen {
  const origin = trimOrigin(str(client.origin));
  const alias = aliases?.[origin] ?? null;
  const target: RemoteTarget | null = !client.environmentId || !origin ? null : alias ? { kind: 'ssh' } : isLoopback(origin) ? { kind: 'primary', httpBaseUrl: origin } : { kind: 'bearer' };
  const advertised = Array.isArray(obj(client.config).remoteOpenTargets)
    ? arr(obj(client.config).remoteOpenTargets).map(entry => ({ kind: str(entry.kind), host: str(entry.host).trim() })).filter(entry => entry.host) : undefined;
  const state = resolveRemoteOpenState({ target, sshAlias: alias, remoteOpenTargets: advertised, isDesktopRenderer: true });
  const label = str(obj(obj(client.config).environment).label).trim() || 'this machine';
  return { state, resolved: aliases !== null && target !== null, primary: target?.kind === 'primary', label };
}

/** The editors Open offers here: the server's PATH probe on this machine, else this Mac's remote-capable ones. */
export function effectiveEditors(remote: RemoteOpen, serverEditors: string[], remoteEditors: string[]): string[] {
  return remote.state.mode === 'local-exec' ? serverEditors : remoteEditors;
}

export type OpenInView = { mode: RemoteOpenMode; show: boolean; label: string; hint: string; unavailable: string; empty: boolean; editors: string[] };
/** What the details card's Open row needs (OpenInPicker panel form). */
export async function openInView(client: T3Client, native: Native | null | undefined, serverEditors: string[], projectName: string): Promise<OpenInView> {
  await loadSshAliases(native);
  const remote = remoteOpenFor(client);
  const remoteEditors = remote.state.mode === 'remote-links' ? await remoteCapableEditors(native) : REMOTE_FALLBACK_EDITORS;
  const editors = effectiveEditors(remote, serverEditors, remoteEditors);
  const seen = remote.state.mode === 'remote-links' && native?.available ? await remoteHintSeen(native) : true;
  return {
    mode: remote.state.mode,
    show: shouldShowOpenInPicker({ activeProjectName: projectName || undefined, activeThreadEnvironmentId: client.environmentId, primaryEnvironmentId: remote.primary ? client.environmentId : null, remoteOpenMode: remote.state.mode }),
    label: remote.label,
    hint: remote.state.mode === 'remote-links' && !seen ? `Opens over SSH. Needs your key on ${remote.label}` : '',
    unavailable: remote.state.mode === 'remote-unavailable' ? `No SSH route to ${remote.label}` : '',
    empty: remote.state.mode !== 'remote-unavailable' && editors.length === 0,
    editors,
  };
}

/** The open-favorite shortcut is live only where the picker shows (ChatView useOpenFavoriteEditorShortcut `enabled`). */
export function openFavoriteEnabled(client: T3Client, projectName: string): boolean {
  const remote = remoteOpenFor(client);
  return shouldShowOpenInPicker({ activeProjectName: projectName || undefined, activeThreadEnvironmentId: client.environmentId, primaryEnvironmentId: remote.primary ? client.environmentId : null, remoteOpenMode: remote.state.mode });
}

/**
 * OpenInPicker openInEditor: on this machine the server runs the editor (shell.openInEditor); remotely the
 * deep link goes to the OS, and only an accepted link counts as the first remote open; with no SSH route
 * nothing opens. Returns whether the editor became the preferred one.
 */
export async function openInEditorHere(client: T3Client, native: Native, cwd: string, editor: string): Promise<boolean> {
  if (!cwd || !editor) return false;
  await loadSshAliases(native);
  const remote = remoteOpenFor(client);
  if (remote.state.mode === 'remote-unavailable') return false;
  if (remote.state.mode === 'remote-links') {
    const url = buildRemoteOpenUrl({ editor, host: remote.state.host.host, absolutePath: cwd });
    if (url === undefined) return false;
    const reply = await bridgeReply(native, { op: 'remoteEditorsOpen', url }).catch(() => null);
    if (!reply?.ok || obj(reply.value).opened !== true) return false;
    await markRemoteHintSeen(native);
    return true;
  }
  await client.restAccess(native).request('shell.openInEditor', { cwd, editor });
  return true;
}

/**
 * editor.openFavorite (OpenInPicker's shortcut handler): the preferred editor opens the workspace,
 * remotely the first remote-capable editor this Mac has. False when nothing could open.
 */
export async function openFavoriteHere(client: T3Client, native: Native, cwd: string, serverFavorite: string): Promise<boolean> {
  await loadSshAliases(native);
  const remote = remoteOpenFor(client);
  const editor = remote.state.mode === 'local-exec' ? serverFavorite : remote.state.mode === 'remote-links' ? (await remoteCapableEditors(native))[0] ?? '' : '';
  return openInEditorHere(client, native, cwd, editor);
}
