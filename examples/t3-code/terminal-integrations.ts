// T3 Code 1e2ecbd975 terminalContext.ts, composerContextRecords.ts and
// ThreadTerminalDrawer.tsx (MIT, LICENSE-T3). Shell eligibility is extracted
// from ChatMarkdown's CodeBlockActions; links honor the configured browser target.
import { obj, str, type Obj } from './domain';
import { contextId, contextLabel, contextLink, contextReferences } from './composer-editor-menu';
import type { T3Client } from './client';
import { pushToast } from './toast';
import type { Native } from './protocol';
import { ClientError } from './protocol';
import { favoriteEditor } from './keyboard-dispatch';
import { lastEditor } from './shell-details';
import { resolvePathLinkTarget } from './terminal-links';
import { letGo } from './let-go';


export interface TerminalContextSelection { terminalId: string; terminalLabel: string; lineStart: number; lineEnd: number; text: string }
export interface TerminalContextDraft extends TerminalContextSelection { id: string; threadId: string; createdAt: string }
export function normalizeTerminalContextText(text: string): string { return text.replace(/\r\n/g, '\n').replace(/^\n+|\n+$/g, ''); }
export function isTerminalContextExpired(context: { text: string }): boolean { return normalizeTerminalContextText(context.text).length === 0; }
export function formatTerminalContextLabel(context: Pick<TerminalContextSelection, 'terminalLabel' | 'lineStart' | 'lineEnd'>): string {
  return `${context.terminalLabel} ${context.lineStart === context.lineEnd ? `line ${context.lineStart}` : `lines ${context.lineStart}-${context.lineEnd}`}`;
}
export function terminalContextReference(context: Pick<TerminalContextDraft, 'id' | 'terminalLabel' | 'lineStart' | 'lineEnd'>) {
  return { kind: 'terminal', contextId: contextId('terminal', context.id), label: formatTerminalContextLabel(context) };
}
export function formatTerminalContextReference(context: Pick<TerminalContextDraft, 'id' | 'terminalLabel' | 'lineStart' | 'lineEnd'>): string {
  const reference = terminalContextReference(context); return contextLink(reference.kind, reference.contextId, reference.label);
}
export function terminalContextRecord(context: TerminalContextDraft): Obj {
  return { version: 1, ...terminalContextReference(context), label: contextLabel(formatTerminalContextLabel(context), 'terminal'),
    terminalId: context.terminalId, terminalLabel: context.terminalLabel, lineStart: context.lineStart, lineEnd: context.lineEnd, text: normalizeTerminalContextText(context.text) };
}
export function migrateLegacyTerminalContextPlaceholders(prompt: string, contexts: ReadonlyArray<Pick<TerminalContextDraft, 'id' | 'terminalLabel' | 'lineStart' | 'lineEnd'>>): string {
  let index = 0; return prompt.replaceAll('\uFFFC', () => { const context = contexts[index++]; return context ? formatTerminalContextReference(context) : ''; });
}
export function parseTerminalContext(value: unknown): TerminalContextDraft | null {
  const v = obj(value);
  if (!str(v.id) || !str(v.terminalId).trim() || !str(v.terminalLabel).trim() || typeof v.text !== 'string' ||
    typeof v.lineStart !== 'number' || !Number.isFinite(v.lineStart) || typeof v.lineEnd !== 'number' || !Number.isFinite(v.lineEnd)) return null;
  const lineStart = Math.max(1, Math.floor(v.lineStart));
  return { id: str(v.id), threadId: str(v.threadId), createdAt: str(v.createdAt), terminalId: str(v.terminalId).trim(), terminalLabel: str(v.terminalLabel).trim(),
    lineStart, lineEnd: Math.max(lineStart, Math.floor(v.lineEnd)), text: normalizeTerminalContextText(v.text) };
}
export function terminalSelectionLineRange(position: { start: { y: number }; end: { y: number } }) {
  return { lineStart: position.start.y + 1, lineEnd: Math.max(position.start.y + 1, position.end.y + 1) };
}
export function terminalSelectionMenuItems(options?: { canAddToChat?: boolean }) {
  return [...(options?.canAddToChat === false ? [] : [{ id: 'add-to-chat', label: 'Add to chat' }]), { id: 'copy', label: 'Copy' }];
}
export function terminalContextMenuItems(options: { hasSelection: boolean; canAddToChat?: boolean }) {
  return [...terminalSelectionMenuItems(options).map(item => ({ ...item, disabled: !options.hasSelection })), { id: 'paste', label: 'Paste' }];
}
export function shouldClearTerminalSelectionAction(options: { actionPending: boolean; openMenuRequestId: number | null; currentRequestId: number }): boolean {
  return options.actionPending || options.openMenuRequestId === options.currentRequestId;
}
export function canRunShellCommand(language: string, code: string, streaming: boolean, closedFence = true): boolean {
  const command = code.trim();
  return closedFence && !streaming && /^(?:sh|bash|zsh|fish|shell|powershell|pwsh)$/.test(language) && code.endsWith('\n') && command.length > 0 && !command.endsWith('\\') && !/[\p{Cc}\p{Cf}]/u.test(code.slice(0, -1));
}
export function buildExpiredTerminalContextToastCopy(count: number, variant: 'empty' | 'omitted') {
  const noun = Math.max(1, Math.floor(count)) === 1 ? 'Expired terminal context' : 'Expired terminal contexts';
  return variant === 'empty' ? { title: `${noun} won't be sent`, description: 'Remove it or re-add it to include terminal output.' } :
    { title: `${noun} omitted from message`, description: 'Re-add it if you want that terminal output included.' };
}
// Records are retained beside drafts; only references in the current prompt are sent.
// This also preserves their payload when the prompt is stashed or restored.
export function saveTerminalContext(client: T3Client, context: TerminalContextDraft): void {
  const local = obj(client.local), records = obj(local.terminalContexts);
  local.terminalContexts = { ...records, [contextId('terminal', context.id)]: terminalContextRecord(context) };
}
export function adoptTerminalContexts(next: object, saved: Obj): void {
  const records: Obj = {};
  for (const [id, value] of Object.entries(obj(saved.terminalContexts))) {
    const record = obj(value);
    if (record.kind === 'terminal' && record.contextId === id && typeof record.text === 'string') records[id] = record;
  }
  Object.assign(next, { terminalContexts: records });
}
export function terminalDraftRecords(client: T3Client, text: string): Obj[] {
  const saved = obj(obj(client.local).terminalContexts);
  return contextReferences(text).flatMap(reference => reference.kind === 'terminal' && obj(saved[reference.id]).kind === 'terminal' ? [obj(saved[reference.id])] : []);
}
export function terminalMessageRecords(client: T3Client, text: string): Obj[] { return terminalDraftRecords(client, text).filter(record => !isTerminalContextExpired({ text: str(record.text) })); }
export function omitExpiredTerminalContexts(client: T3Client, text: string, hasAttachments: boolean): { text: string; empty: boolean } {
  const records = new Map(terminalMessageRecords(client, text).map(record => [str(record.contextId), record]));
  const expired = contextReferences(text).filter(reference => reference.kind === 'terminal' && !records.has(reference.id));
  if (!expired.length) return { text, empty: false };
  const ids = new Set(expired.map(reference => reference.id));
  const cleaned = text.replace(/\[[^\]\n]*\]\(t3-context:\/\/v1\/terminal\/([a-z0-9_-]+)\)/gi, (source, id: string) => ids.has(id) ? '' : source);
  const empty = !cleaned.trim() && !hasAttachments;
  pushToast(client, { kind: 'warning', ...buildExpiredTerminalContextToastCopy(expired.length, empty ? 'empty' : 'omitted') });
  return { text: cleaned, empty };
}

/** The source span must have a matching closing fence (ChatMarkdown.isClosedCodeFence). */
export function isClosedCodeFence(source: string): boolean {
  const opening = /^(?:`{3,}|~{3,})/.exec(source)?.[0];
  const closing = /(?:^|\n)[ \t>]*(`{3,}|~{3,})[ \t\r]*$/.exec(source)?.[1];
  return opening !== undefined && closing !== undefined && opening[0] === closing[0] && closing.length >= opening.length;
}
export function runnableShellCommands(markdown: string, streaming: boolean): string[] {
  if (streaming) return [];
  const commands: string[] = [], lines = markdown.split('\n');
  for (let index = 0; index < lines.length; index++) {
    const opening = /^[ \t>]*(`{3,}|~{3,})([^\s`]*)[^\n]*$/.exec(lines[index] ?? '');
    if (!opening) continue;
    const fence = opening[1] ?? '', language = opening[2] ?? '';
    const body: string[] = []; let closed = false;
    for (index++; index < lines.length; index++) {
      const line = lines[index] ?? '';
      const closing = /^[ \t>]*(`{3,}|~{3,})[ \t\r]*$/.exec(line)?.[1];
      if (closing && closing[0] === fence[0] && closing.length >= fence.length) { closed = true; break; }
      body.push(line.replace(/^(?: {0,3}> ?)+/, ''));
    }
    const code = body.join('\n') + (body.length ? '\n' : '');
    if (canRunShellCommand(language, code, streaming, closed)) commands.push(`${language}:${code.replace(/\n+$/, '')}`);
  }
  return commands;
}

export function terminalLinkTarget(message: Obj, preference: string, hasThread: boolean): 'system' | 'app' | 'path' | 'unsupported' {
  const text = str(message.text);
  if (/^https?:\/\//i.test(text)) {
    try { const url = new URL(text); if (!['http:', 'https:'].includes(url.protocol)) return 'unsupported'; } catch { return 'unsupported'; }
    return message.metaKey === true || message.ctrlKey === true || preference !== 'app' || !hasThread ? 'system' : 'app';
  }
  return /^[a-z][a-z0-9+.-]*:/i.test(text) && !/^[a-z]:[\\/]/i.test(text) ? 'unsupported' : text ? 'path' : 'unsupported';
}
/** ThreadTerminalDrawer's link activation, including modifier escape and preferred editor. */
export async function terminalLinkAction(client: T3Client, native: Native, message: Obj): Promise<void> {
  const threadId = str(message.threadId) || client.threadId;
  const thread = client.shell.threads.find(thread => thread.id === threadId);
  const project = client.shell.projects.find(project => project.id === (str(thread?.projectId) || client.projectId));
  const target = terminalLinkTarget(message, str(obj(client.local.clientSettings).browserLinkTarget, 'system'), !!threadId);
  if (target === 'unsupported') return;
  if (target === 'app') {
    pushToast(client, { kind: 'error', title: 'Unable to open link', description: 'The in-app Browser is unavailable in this build. Command-click to open this URL in your system browser.' });
    return;
  }
  try {
    if (target === 'system') {
      const result = await client.call(native, { op: 'terminalOpenExternal', url: str(message.text) });
      if (result.opened !== true) throw new ClientError('Unable to open link');
    } else {
      const cwd = str(message.cwd) || str(thread?.worktreePath) || str(project?.workspaceRoot);
      const editor = favoriteEditor(client.config, lastEditor(client));
      if (!editor) throw new ClientError('Unable to open path: no preferred editor is available.');
      await client.restAccess(native).request('shell.openInEditor', { cwd: resolvePathLinkTarget(str(message.text), cwd), editor });
    }
  } catch (error) {
    if (letGo(error)) throw error;
    const text = error instanceof Error ? error.message : target === 'system' ? 'Unable to open link' : 'Unable to open path';
    await client.call(native, { op: 'terminalSystemMessage', environmentId: str(message.environmentId) || client.environmentId,
      threadId, terminalId: str(message.terminalId), message: text }).catch(() => {
      pushToast(client, { kind: 'error', title: 'Unable to open link', description: text });
    });
  }
}
