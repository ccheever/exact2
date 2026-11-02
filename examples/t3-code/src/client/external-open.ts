// Opening a link outside the app (MIT reference, see LICENSE-T3: apps/desktop/src/electron/ElectronShell.ts
// parseSafeExternalUrl and openExternal, at 74541bab17). Only an http(s) URL or a remote editor's SSH deep link
// with no URL user info is opened; anything else is refused here. The open itself is Contract's `openURL`: the
// snapshot carries the latest request (`externalOpen`), and app.contract's `externalOpen` task opens it once per
// serial. app.json's `open_url.schemes` lists the editors' schemes, which `openURL` refuses unless declared. An
// agent run journals the open (`openURL vscode: … (journaled, not opened under the agent)`) instead of opening it.
// As Electron's `shell.openExternal` resolves whether the system opened the URL, `openURL(url, id)` answers
// `opened` or `failed`, which app-window.contract hands back (`external-open-answer`, `answerExternalOpen`).
import { REMOTE_CAPABLE_EDITOR_IDS, remoteSchemeForEditor } from '../shared/editors';

const SAFE_WEB_PROTOCOLS = new Set(['http:', 'https:']);
const REMOTE_EDITOR_PROTOCOLS = new Set(REMOTE_CAPABLE_EDITOR_IDS.flatMap(id => {
  const scheme = remoteSchemeForEditor(id);
  return scheme === undefined ? [] : [`${scheme}:`];
}));
// Zed embeds [user@]host in the path; reject passwords and ports there.
const ZED_SSH_PATHNAME = /^\/(?:[^/@:]+@)?[^/@:]+\/.*$/;

const isRemoteEditorUrl = (url: URL) =>
  REMOTE_EDITOR_PROTOCOLS.has(url.protocol) && url.username.length === 0 && url.password.length === 0 &&
  (url.protocol === 'zed:'
    ? url.host === 'ssh' && ZED_SSH_PATHNAME.test(url.pathname)
    : url.protocol === 'jetbrains:'
      ? url.host === 'gateway' && url.pathname === '/ssh/environment' && (url.searchParams.get('h') ?? '').length > 0
      : url.host === 'vscode-remote' && url.pathname.startsWith('/ssh-remote+') && url.pathname.length > '/ssh-remote+'.length);

/** parseSafeExternalUrl: the URL to hand the system, or null when it is refused. */
export function parseSafeExternalUrl(rawUrl: unknown): string | null {
  if (typeof rawUrl !== 'string') return null;
  try {
    const url = new URL(rawUrl);
    return SAFE_WEB_PROTOCOLS.has(url.protocol) || isRemoteEditorUrl(url) ? url.href : null;
  } catch { return null; }
}

export type ExternalOpen = { serial: number; url: string };
let request: ExternalOpen = { serial: 0, url: '' };
const opened: string[] = [];
// The opens waiting for Contract's answer, by serial. An open waits once a plan has answered: each boot's plan answers
// the serial standing at its first answer `opened` (server-browser-members.contract), so an open View › Reload's new plan
// will not perform is not left waiting, and one before any plan, or in a test with none, is taken as opened.
const waiting = new Map<number, (opened: boolean) => void>();
let answered = false;

/** ElectronShell.openExternal: whether the system opened the URL. A refused URL is false at once. */
export function openExternal(rawUrl: unknown): Promise<boolean> {
  const url = parseSafeExternalUrl(rawUrl);
  if (url === null) return Promise.resolve(false);
  request = { serial: request.serial + 1, url };
  opened.push(url);
  if (!answered) return Promise.resolve(true);
  const serial = request.serial;
  return new Promise(resolve => { waiting.set(serial, resolve); });
}

/** Tests: whether an open waits for an answer, as it does once a plan has answered. */
export function awaitExternalOpenAnswersForTests(on: boolean): void { answered = on; }

/**
 * `openURL(url, id)`'s answer for the open at `serial`, `opened` or `failed`. Only the latest request opens, so one it
 * replaced before its task ran shares its answer.
 */
export function answerExternalOpen(serial: number, value: string): void {
  answered = true;
  for (const [at, resolve] of waiting) if (at <= serial) { waiting.delete(at); resolve(value === 'opened'); }
}

/** The snapshot's `externalOpen`: the latest accepted URL and its serial (0 before the first). */
export const externalOpenRequest = (): ExternalOpen => request;

/** Tests: the URLs accepted since the last reset, in order. */
export const externalOpensForTests = (): readonly string[] => opened;
export function resetExternalOpensForTests(): void { opened.length = 0; }
