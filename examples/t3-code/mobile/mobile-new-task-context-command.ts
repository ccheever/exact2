// Source365aa87982 use-composer-command-menu; existing commands, one per-draft owner.
// @ref llp/1109.005-composer-and-transcript.decision.md#foreground-capture-facts
import type { T3Client } from './shared/client';
import { obj, str, type Obj } from './shared/domain';
import { ClientError, type Native, type Files } from './shared/protocol';
import { letGo } from './shared/let-go';
import { peekComposerContextPick, localId } from './shared/composer-editor';
import { contextLink } from './shared/composer-editor-menu';
import { mobileNewTaskDraftCurrent, mobileNewTaskDraftIsKey } from './mobile-new-task-drafts';
import { mobileNewTaskContextRead, mobileNewTaskContextGuard, mobileNewTaskContextWrite } from './mobile-new-task-context';
import { mobileThreadContext, mobilePullRequestContext, type MobilePullRequestMetadata } from './mobile-new-task-context-producers';
import { parseTerminalContext, terminalContextRecord } from './shared/terminal-integrations';

/** null delegates unrelated commands to the existing shared owner. */
export async function mobileNewTaskContextCommand(client: T3Client, op: string, id: string, value: string,
  native: Native | null | undefined, storage: Files): Promise<{ revision: number; message: string } | null> {
  const target = mobileNewTaskDraftCurrent(client);
  if (!mobileNewTaskDraftIsKey(client.draftKey) || !op.startsWith('editorlocal:')) return null;
  if (!target) return { revision: client.revision, message: 'The composer changed. Reopen the original draft.' };
  const pick = op === 'editorlocal:pick' ? peekComposerContextPick(client, id) : null;
  if (op === 'editorlocal:pick' && pick && !['thread', 'pull-request'].includes(pick.row.type)) return null;
  if (op !== 'editorlocal:pick' && !(op === 'editorlocal:insert-context' && ['thread', 'terminal'].includes(id))) return null;
  try {
    const guard = mobileNewTaskContextGuard(client, target.key);
    if (!native?.available || !guard || guard.createdAt !== target.createdAt) throw new ClientError('The composer changed.', 'superseded');
    const existing = mobileNewTaskContextRead(client, target.key);
    if (!existing.ok) throw new ClientError(existing.error);
    const before = client.local.drafts[target.key] ?? '';
    let record: Obj, reference: Obj | undefined, start = before.length, end = start;
    const kind = pick?.row.type ?? id;
    if (op === 'editorlocal:pick') {
      if (!pick) throw new ClientError('That suggestion is no longer available.');
      start = pick.trigger.start; end = pick.trigger.end;
    }
    if (kind === 'thread') {
      const threadId = pick ? pick.row.id.slice('thread:'.length) : value;
      const thread = client.shell.threads.find(thread => thread.id === threadId);
      if (!thread || pick && pick.trigger.kind !== 'path') throw new ClientError('That thread is no longer available.');
      record = mobileThreadContext(client.environmentId, threadId, str(thread.title));
      reference = record;
      // Pinned mobile keeps the first payload for a repeated thread identity.
      record = existing.context?.records.find(entry => entry.contextId === record.contextId) ?? record;
    } else if (kind === 'pull-request') {
      if (!pick || pick.trigger.kind !== 'pull-request') throw new ClientError('That pull request is no longer available.');
      const metadata = obj(JSON.parse(pick.row.insert));
      if (!Number.isSafeInteger(metadata.number) || Number(metadata.number) < 1 ||
        !['title', 'url', 'headBranch', 'baseBranch', 'state'].every(key => typeof metadata[key] === 'string') || typeof metadata.isDraft !== 'boolean')
        throw new ClientError('That pull request metadata is invalid.');
      record = mobilePullRequestContext(metadata as unknown as MobilePullRequestMetadata, localId());
    } else {
      const terminal = parseTerminalContext(JSON.parse(value));
      if (!terminal || !terminal.text.trim()) throw new ClientError('Select terminal output before attaching it.');
      record = terminalContextRecord(terminal);
    }
    const shown = reference ?? record;
    const link = contextLink(str(shown.kind), str(shown.contextId), str(shown.label));
    const insert = `${!pick && before && !/\s$/.test(before) ? ' ' : ''}${link} `;
    const text = before.slice(0, start) + insert + before.slice(end);
    if (!mobileNewTaskContextWrite(client, guard, text, record)) throw new ClientError('The draft context could not be changed.');
    await client.persist(storage);
    return { revision: client.revision, message: '' };
  } catch (error) {
    if (letGo(error)) throw error;
    return { revision: client.revision, message: error instanceof Error ? error.message : 'Could not attach context.' };
  }
}
