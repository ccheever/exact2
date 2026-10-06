// Proposed plans as T3 Code presents them (MIT, see LICENSE-T3):
// apps/web/src/proposedPlan.ts and components/chat/ProposedPlanCard.tsx.
import { arr, obj, str, type Obj } from './domain';
import { ClientError, type Native } from './protocol';
import type { T3Client } from './client';
import { pushToast } from './toast';

export function proposedPlanTitle(markdown: string): string | null {
  const heading = /^\s{0,3}#{1,6}\s+(.+)$/m.exec(markdown)?.[1]?.trim();
  return heading ? heading : null;
}
export function stripDisplayedPlanMarkdown(markdown: string): string {
  const lines = markdown.replace(/\s+$/, '').split(/\r?\n/);
  const source = lines[0] && /^\s{0,3}#{1,6}\s+/.test(lines[0]) ? lines.slice(1) : [...lines];
  while (source.length && source[0]!.trim() === '') source.shift();
  if (/^\s{0,3}#{1,6}\s+(.+)$/.exec(source[0] ?? '')?.[1]?.trim().toLowerCase() === 'summary') {
    source.shift();
    while (source.length && source[0]!.trim() === '') source.shift();
  }
  return source.join('\n');
}
export function collapsedPlanPreview(markdown: string, maxLines = 10): string {
  const lines = stripDisplayedPlanMarkdown(markdown).replace(/\s+$/, '').split(/\r?\n/).map(line => line.replace(/\s+$/, ''));
  const preview: string[] = [];
  let visible = 0, more = false;
  for (const line of lines) {
    const shown = line.trim().length > 0;
    if (shown && visible >= maxLines) { more = true; break; }
    preview.push(line);
    if (shown) visible++;
  }
  while (preview.length && preview[preview.length - 1]!.trim() === '') preview.pop();
  if (!preview.length) return proposedPlanTitle(markdown) ?? 'Plan preview unavailable.';
  if (more) preview.push('', '...');
  return preview.join('\n');
}
export function planFilename(markdown: string): string {
  const segment = (proposedPlanTitle(markdown) ?? 'plan').toLowerCase().replace(/[`'".,!?()[\]{}]+/g, '')
    .replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '');
  return `${segment || 'plan'}.md`;
}
export const planExport = (markdown: string) => `${markdown.replace(/\s+$/, '')}\n`;
export const planCollapsible = (markdown: string) => markdown.length > 900 || markdown.split('\n').length > 20;

/**
 * The Markdown a plan row asks the renderer for: the displayed plan and, when
 * it is long enough to collapse, its 10-line preview after a U+0001.
 */
export function planBody(markdown: string): string {
  const shown = stripDisplayedPlanMarkdown(markdown);
  return planCollapsible(markdown) ? `${shown}\u0001${collapsedPlanPreview(markdown)}` : shown;
}

interface SaveState { threadId: string; rowId: string; path: string; saving: boolean }
const saves = new WeakMap<T3Client, SaveState | null>();
function planMarkdown(client: T3Client, rowId: string): string {
  const row = arr(client.projection.visibleTurnItems).find(row => JSON.stringify([row.sourceThreadId, row.sourceItemId]) === rowId);
  const item = obj(row?.item);
  if (item.type !== 'proposed_plan') throw new ClientError('That plan is no longer available.');
  return str(item.markdown);
}
export function planRoot(client: T3Client): string {
  const thread = obj(client.projection.thread);
  const project = client.shell.projects.find((project: Obj) => project.id === (thread.projectId ?? client.projectId));
  return str(thread.worktreePath) || str(project?.workspaceRoot);
}

/** Plan actions: Copy to clipboard, Download as markdown and Save to workspace. */
export async function planAction(client: T3Client, native: Native, op: string, id: string, value: string): Promise<string> {
  if (op === 'plan-copy') {
    try { await client.restAccess(native).call({ op: 'copyText', text: planExport(planMarkdown(client, id)) }); }
    catch (error) {
      pushToast(client, { kind: 'error', title: 'Could not copy plan', description: error instanceof Error && error.message ? error.message : 'An error occurred while copying.' });
      return '';
    }
    return 'Copied!';
  }
  if (op === 'plan-download') {
    const markdown = planMarkdown(client, id);
    const reply = obj(await client.restAccess(native).call({ op: 'saveFile', name: planFilename(markdown), text: planExport(markdown) }));
    return reply.saved === true ? 'Saved plan' : '';
  }
  if (op === 'plan-save-open') {
    if (!planRoot(client)) {
      pushToast(client, { kind: 'error', title: 'Workspace path is unavailable', description: 'This thread does not have a workspace path to save into.', stacked: true });
      return '';
    }
    const previous = saves.get(client);
    saves.set(client, { threadId: client.threadId, rowId: id, path: previous?.rowId === id && previous.path ? previous.path : planFilename(planMarkdown(client, id)), saving: false });
    return '';
  }
  const save = saves.get(client);
  if (op === 'plan-save-path') { if (save && !save.saving) save.path = value.slice(0, 1024); return ''; }
  if (op === 'plan-save-cancel') { if (save && !save.saving) saves.set(client, null); return ''; }
  if (op === 'plan-save') {
    if (!save || save.threadId !== client.threadId || save.saving) return '';
    const relativePath = save.path.trim(), cwd = planRoot(client);
    if (!cwd) return '';
    if (!relativePath) { pushToast(client, { kind: 'warning', title: 'Enter a workspace path' }); return ''; }
    save.saving = true;
    try {
      const result = obj(await client.rpc(native, 'projects.writeFile', { cwd, relativePath, contents: planExport(planMarkdown(client, save.rowId)) }, true));
      saves.set(client, null);
      pushToast(client, { kind: 'success', title: 'Plan saved to workspace', description: str(result.relativePath, relativePath) });
    } catch (error) {
      save.saving = false;
      pushToast(client, { kind: 'error', title: 'Could not save plan', description: error instanceof Error && error.message ? error.message : 'An error occurred while saving.', stacked: true });
    }
    return '';
  }
  throw new ClientError(`Unknown plan action: ${op}`);
}

/** The "Save plan to workspace" dialog, when one is open for this thread. */
export function planSaveView(client: T3Client) {
  const save = saves.get(client);
  const open = !!save && save.threadId === client.threadId;
  return { planSaveOpen: open, planSavePath: open ? save!.path : '', planSaveRoot: open ? planRoot(client) : '', planSaving: open && save!.saving };
}
