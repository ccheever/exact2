// A tool call's file change as the diff panel shows it (T3 Code 6d450a4284, MIT, see LICENSE-T3: apps/web/src/lib/
// toolCallDiff.ts and diffPanelStore.ts ToolCallDiffTarget / resolveToolCallDiffTarget). The panel reads the item in
// full (`orchestration.getTurnItem`) and makes a patch of what the provider recorded: a complete patch as is; added or
// deleted file contents, or bare hunks, wrapped in a file header. A record with content and no diff is shown as that
// content. Changes: the recorded patch is checked with the clone's parse (renderablePatch) where the reference uses
// Pierre's.
import { arr, obj, str, type Obj } from '../client/domain';
import { renderablePatch } from '../pull-requests/pages-pr-code-logic';
import { turnItemDetailRevision } from '../timeline/timeline-item-detail';

export type ToolCallDiffTarget = { threadId: string; itemId: string; revision: string };
export type ToolCallPreview = { patch: string | undefined; content: { label: string; text: string }[]; truncated: boolean; recordedFiles: number };

const optional = (value: unknown): string | undefined => (typeof value === 'string' ? value : undefined);

/** The visible row a target or a row id names (a source thread's item keeps its own id). */
export function toolCallRow(items: readonly Obj[], match: { rowId?: string; threadId?: string; itemId?: string }, threadId: string): Obj | undefined {
  return items.find(row => {
    const item = obj(row.item), source = str(row.sourceThreadId) || threadId, id = str(row.sourceItemId) || str(item.id);
    return match.rowId !== undefined ? str(item.id) === match.rowId || JSON.stringify([row.sourceThreadId, row.sourceItemId]) === match.rowId
      : source === match.threadId && id === match.itemId;
  });
}
/** The target a file change's row opens, or null when the row is not a file change. */
export function toolCallTarget(items: readonly Obj[], rowId: string, threadId: string): ToolCallDiffTarget | null {
  const row = toolCallRow(items, { rowId }, threadId), item = obj(row?.item);
  if (!row || item.type !== 'file_change') return null;
  return { threadId: str(row.sourceThreadId) || threadId, itemId: str(row.sourceItemId) || str(item.id), revision: turnItemDetailRevision(item) };
}
/** resolveToolCallDiffTarget: a change still streaming is read again at its newer revision. */
export function resolveToolCallDiffTarget(target: ToolCallDiffTarget, items: readonly Obj[], threadId: string): ToolCallDiffTarget {
  const item = obj(toolCallRow(items, target, threadId)?.item);
  if (item.type !== 'file_change') return target;
  const revision = turnItemDetailRevision(item);
  return revision === target.revision ? target : { ...target, revision };
}

function toolCallPatch(item: Obj): string | undefined {
  let patch = optional(item.diffStr);
  if (!patch) return patch;
  const changes = arr(item.changes), change = changes[0];
  const rawFileOperation = change?.operation === 'add' || change?.operation === 'delete';
  if (patch.startsWith('diff --git ') || patch.startsWith('--- ')) {
    if (!rawFileOperation) return patch;
    const parsed = renderablePatch(patch, false);
    if (parsed?.kind === 'files' && parsed.files.every(file => {
      const path = file.path.replaceAll('\\', '/');
      return file.hunks.length > 0 && changes.some(recorded => {
        const recordedPath = str(recorded.path).replaceAll('\\', '/');
        return (recorded.operation === 'add' ? file.deletionLines.length === 0 : recorded.operation === 'delete' ? file.additionLines.length === 0 : true)
          && (recordedPath === path || recordedPath.endsWith(`/${path}`));
      });
    })) return patch;
  }
  // Codex can record added/deleted file contents or bare hunks instead of a complete patch.
  if (rawFileOperation || !patch.trimStart().startsWith('@@ ')) {
    if (!rawFileOperation) return patch;
    const lines = patch.split('\n'), hasFinalNewline = patch.endsWith('\n');
    if (hasFinalNewline) lines.pop();
    const added = change!.operation === 'add', range = `1,${lines.length}`;
    patch = `@@ -${added ? '0,0' : range} +${added ? range : '0,0'} @@\n${lines.map(line => `${added ? '+' : '-'}${line}`).join('\n')}\n${hasFinalNewline ? '' : '\\ No newline at end of file\n'}`;
  }
  const oldPath = str(change?.oldPath) || str(item.fileName), newPath = str(change?.path) || str(item.fileName);
  const before = change?.operation === 'add' ? '/dev/null' : JSON.stringify(`a/${oldPath}`);
  const after = change?.operation === 'delete' ? '/dev/null' : JSON.stringify(`b/${newPath}`);
  return `diff --git ${JSON.stringify(`a/${oldPath}`)} ${JSON.stringify(`b/${newPath}`)}\n--- ${before}\n+++ ${after}\n${patch.trimStart()}`;
}

function recordedText(value: string | undefined) {
  const suffix = '\n… output truncated for transport';
  const truncated = value !== undefined && value.endsWith(suffix);
  return { text: truncated ? value.slice(0, -suffix.length) : value, truncated };
}

/** toolCallDiff: the patch to show, the recorded content when there is no diff, and whether the transport cut it. */
export function toolCallDiff(item: Obj): ToolCallPreview {
  const diff = recordedText(optional(item.diffStr));
  const operation = obj(arr(item.changes)[0]).operation;
  const emptyContent = diff.text === '' ? '' : undefined;
  const before = recordedText(optional(item.oldStr) ?? (operation === 'delete' ? emptyContent : undefined));
  const after = recordedText(optional(item.newStr) ?? (operation === 'add' ? emptyContent : undefined));
  return {
    patch: toolCallPatch({ ...item, ...(diff.text === undefined ? {} : { diffStr: diff.text }) }),
    content: diff.text?.trim() ? [] : [{ label: 'Recorded content before', text: before.text }, { label: 'Recorded content after', text: after.text }]
      .filter((entry): entry is { label: string; text: string } => entry.text !== undefined),
    truncated: diff.truncated || before.truncated || after.truncated,
    recordedFiles: arr(item.changes).length,
  };
}
