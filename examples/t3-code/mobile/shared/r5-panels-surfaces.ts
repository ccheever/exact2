// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/r5-panels-surfaces.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// Lane r5-panels: the right panel's "pull-request" and attachment surfaces,
// joined to r4-surfaces-panel.ts's tab model (MIT reference, see LICENSE-T3:
// rightPanelStore.ts openPullRequest / openAttachment, RightPanelTabs.tsx
// surfaceTitle and SurfaceIcon, ChatView.tsx addPullRequestSurface and the
// right panel's pull-request content). Ops arrive as `surface-r5-*`.
import type { T3Client } from './client';
import { obj, num, str } from './domain';
import { ClientError, type Files, type Native } from './protocol';
import type { PanelState, Surface } from './r4-surfaces-panel';
import { prSurfaceId, prTabIcon, prTarget, prRowCommand, prRowLocal, prefetchPrTabs, selectionOf, threadPrTarget, togglePrRows, type PrTarget } from './r5-panels-pr';
import { attachmentLocal, attachmentToken, attachmentView, emptyAttachment, requireAttachment, type AttachmentMeta, type AttachmentView } from './r5-panels-attach';

export type R5Surface = Surface;
export type PrSurfaceView = { selected: string; number: number; label: string };
export const emptyPrSurface = (): PrSurfaceView => ({ selected: '', number: 0, label: '' });

function upsert(state: PanelState, surface: R5Surface): void {
  const index = state.surfaces.findIndex(entry => entry.id === surface.id);
  if (index < 0) state.surfaces.push(surface); else state.surfaces[index] = { ...state.surfaces[index]!, ...surface };
  state.active = surface.id; state.visible = true;
}
/** rightPanelStore.openPullRequest: one tab per reference; a known URL refreshes the tab's own. */
export function openPullRequestIn(state: PanelState, target: PrTarget): R5Surface {
  const surface: R5Surface = { id: prSurfaceId(target), kind: 'pull-request', path: '', line: 0, reveal: 0, pr: target };
  upsert(state, surface);
  return surface;
}
/** rightPanelStore.openAttachment: the attachment's own tab, replacing the standalone explorer. */
export function openAttachmentIn(state: PanelState, meta: AttachmentMeta): R5Surface {
  state.surfaces = state.surfaces.filter(entry => entry.kind !== 'files');
  const surface: R5Surface = { id: `attachment:${meta.id}`, kind: 'attachment', path: meta.name, line: 0, reveal: 0, attachment: meta };
  upsert(state, surface);
  return surface;
}

/** The chooser's "Pull request" (P): ChatView addPullRequestSurface. Null when the value names another surface. */
export function openThreadPullRequest(client: T3Client, state: PanelState, value: string): boolean | null {
  if (!['pull-request', 'p'].includes(value.toLowerCase())) return null;
  const target = threadPrTarget(client);
  if (!target) return false;
  client.diffOpen = false;
  openPullRequestIn(state, target);
  return true;
}

/** `surface-r5-*` without the command gate: open a pull request link or an attachment, and the attachment's controls. */
export async function r5Local(client: T3Client, native: Native, state: PanelState, op: string, id: string, value: string): Promise<string> {
  if (op === 'pr-open') {
    // useOpenPrLink: a project on the link's host opens the surface; the caller passed its target.
    const target = prTarget(client, value) ?? (id ? (() => { const parsed = obj(JSON.parse(id)); return num(parsed.number) > 0 ? { projectId: str(parsed.projectId), host: str(parsed.host), repository: str(parsed.repository), number: num(parsed.number), url: str(parsed.url, value) } : null; })() : null);
    if (!target) throw new ClientError('That pull request cannot be opened here.');
    client.diffOpen = false;
    openPullRequestIn(state, target);
    return '';
  }
  if (op === 'attachment') {
    const meta = requireAttachment(client, id);
    client.diffOpen = false;
    openAttachmentIn(state, meta);
    return '';
  }
  if (op === 'pr-more') { togglePrRows(client); return ''; }
  if (op.startsWith('pr-') && prRowLocal(client, op.slice(3), id, value)) return ''; // lane r6-pr: Merge dialog, checks "Show all"
  if (op.startsWith('att-')) return attachmentLocal(client, native, op.slice(4), id, 0);
  throw new ClientError(`Unknown surface action: ${op}`);
}
/** `shell:surface-r5-*`: the details row's in-place pull request action. */
export async function r5Command(client: T3Client, native: Native, _storage: Files, op: string, id: string): Promise<string> {
  if (op.startsWith('pr-')) return prRowCommand(client, native, op.slice(3), id);
  throw new ClientError(`Unknown surface action: ${op}`);
}

/** surfaceTitle / SurfaceIcon for the two kinds: "#N" with its lifecycle glyph; the attachment's name with its file glyph. */
export function r5Tab(client: T3Client, surface: R5Surface): { title: string; icon: string; tone: string; fileToken: string } | null {
  if (surface.kind === 'pull-request' && surface.pr) { const icon = prTabIcon(client, surface.pr); return { title: `#${surface.pr.number}`, icon: icon.icon, tone: icon.state, fileToken: '' }; }
  if (surface.kind === 'attachment' && surface.attachment) return { title: surface.attachment.name, icon: '', tone: '', fileToken: attachmentToken(surface.attachment.name) };
  return null;
}

/** Lane r6-pr: the pull request tabs' detail, read before their icons are drawn. */
export async function r5Prefetch(client: T3Client, native: Native | null, surfaces: R5Surface[], now: number): Promise<void> {
  if (native) await prefetchPrTabs(client, native, surfaces.flatMap(surface => (surface.kind === 'pull-request' && surface.pr ? [surface.pr] : [])), now);
}
/** The active surface's body data: the pull request's selection for the window's `prDetail`, or the attachment preview. */
export async function r5Views(client: T3Client, native: Native | null | undefined, active: R5Surface | null, open: boolean, now: number): Promise<{ pr: PrSurfaceView; attachment: AttachmentView }> {
  const pr = open && active?.kind === 'pull-request' && active.pr ? { selected: selectionOf(active.pr), number: active.pr.number, label: `#${active.pr.number}` } : emptyPrSurface();
  const attachment = open && active?.kind === 'attachment' && active.attachment ? await attachmentView(client, native, active.attachment, now) : emptyAttachment();
  return { pr, attachment };
}
export { threadPrTarget };
