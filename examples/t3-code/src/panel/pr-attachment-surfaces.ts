// The right panel's "pull-request" and attachment surfaces,
// joined to panel-surfaces.ts's tab model (MIT reference, see LICENSE-T3:
// rightPanelStore.ts openPullRequest / openAttachment, RightPanelTabs.tsx
// surfaceTitle and SurfaceIcon, ChatView.tsx addPullRequestSurface and the
// right panel's pull-request content). Ops arrive as `surface-linked-*`.
import type { T3Client } from '../client/client';
import { obj, num, str } from '../client/domain';
import { bridgeReply, ClientError, type Files, type Native } from '../client/protocol';
import type { PanelState, Surface } from './panel-surfaces';
import { prLinksOpenInApp, prSurfaceId, prTabIcon, prTarget, prRowCommand, prRowLocal, prefetchPrTabs, selectionOf, threadPrTarget, togglePrRows, type PrTarget } from './pr-surface';
import { openLink as openLinkByTarget } from '../browser/browser-links';
import { attachmentLocal, attachmentToken, attachmentView, emptyAttachment, requireAttachment, type AttachmentMeta, type AttachmentView } from './attachment-surface';

export type LinkedSurface = Surface;
export type PrSurfaceView = { selected: string; number: number; label: string };
export const emptyPrSurface = (): PrSurfaceView => ({ selected: '', number: 0, label: '' });

function upsert(state: PanelState, surface: LinkedSurface): void {
  const index = state.surfaces.findIndex(entry => entry.id === surface.id);
  if (index < 0) state.surfaces.push(surface); else state.surfaces[index] = { ...state.surfaces[index]!, ...surface };
  state.active = surface.id; state.visible = true;
}
/** rightPanelStore.openPullRequest: one tab per reference; a known URL refreshes the tab's own. */
export function openPullRequestIn(state: PanelState, target: PrTarget): LinkedSurface {
  const surface: LinkedSurface = { id: prSurfaceId(target), kind: 'pull-request', path: '', line: 0, reveal: 0, pr: target };
  upsert(state, surface);
  return surface;
}
/** rightPanelStore.openAttachment: the attachment's own tab, replacing the standalone explorer. */
export function openAttachmentIn(state: PanelState, meta: AttachmentMeta): LinkedSurface {
  state.surfaces = state.surfaces.filter(entry => entry.kind !== 'files');
  const surface: LinkedSurface = { id: `attachment:${meta.id}`, kind: 'attachment', path: meta.name, line: 0, reveal: 0, attachment: meta };
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

/** `surface-linked-*` without the command gate: open a pull request link or an attachment, and the attachment's controls. */
export async function linkedLocal(client: T3Client, native: Native, state: PanelState, op: string, id: string, value: string): Promise<string> {
  if (op === 'pr-open') {
    if (id === 'held') { await bridgeReply(native, { op: 'remoteEditorsOpen', url: value }); return ''; }
    if (!prLinksOpenInApp(client)) { await openLinkByTarget(client, native, value); return ''; }
    // useOpenPrLink: a project on the link's host opens the surface; the caller passed its target.
    const target = prTarget(client, value) ?? (id ? (() => { const parsed = obj(JSON.parse(id)); return num(parsed.number) > 0 ? { projectId: str(parsed.projectId), host: str(parsed.host), repository: str(parsed.repository), number: num(parsed.number), url: str(parsed.url, value) } : null; })() : null);
    if (!target) { await openLinkByTarget(client, native, value); return ''; }
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
  if (op.startsWith('pr-') && prRowLocal(client, op.slice(3), id, value)) return ''; // Merge dialog, checks "Show all"
  if (op.startsWith('att-')) return attachmentLocal(client, native, op.slice(4), id, 0);
  throw new ClientError(`Unknown surface action: ${op}`);
}
/** `shell:surface-linked-*`: the details row's in-place pull request action. */
export async function linkedCommand(client: T3Client, native: Native, _storage: Files, op: string, id: string): Promise<string> {
  if (op.startsWith('pr-')) return prRowCommand(client, native, op.slice(3), id);
  throw new ClientError(`Unknown surface action: ${op}`);
}

/** surfaceTitle / SurfaceIcon for the two kinds: "#N" with its lifecycle glyph; the attachment's name with its file glyph. */
export function linkedTab(client: T3Client, surface: LinkedSurface): { title: string; icon: string; tone: string; fileToken: string } | null {
  if (surface.kind === 'pull-request' && surface.pr) { const icon = prTabIcon(client, surface.pr); return { title: `#${surface.pr.number}`, icon: icon.icon, tone: icon.state, fileToken: '' }; }
  if (surface.kind === 'attachment' && surface.attachment) return { title: surface.attachment.name, icon: '', tone: '', fileToken: attachmentToken(surface.attachment.name) };
  return null;
}

/** The pull request tabs' detail, read before their icons are drawn. */
export async function linkedPrefetch(client: T3Client, native: Native | null, surfaces: LinkedSurface[], now: number): Promise<void> {
  if (native) await prefetchPrTabs(client, native, surfaces.flatMap(surface => (surface.kind === 'pull-request' && surface.pr ? [surface.pr] : [])), now);
}
/** The active surface's body data: the pull request's selection for the window's `prDetail`, or the attachment preview. */
export async function linkedViews(client: T3Client, native: Native | null | undefined, active: LinkedSurface | null, open: boolean, now: number): Promise<{ pr: PrSurfaceView; attachment: AttachmentView }> {
  const pr = open && active?.kind === 'pull-request' && active.pr ? { selected: selectionOf(active.pr), number: active.pr.number, label: `#${active.pr.number}` } : emptyPrSurface();
  const attachment = open && active?.kind === 'attachment' && active.attachment ? await attachmentView(client, native, active.attachment, now) : emptyAttachment();
  return { pr, attachment };
}
export { threadPrTarget };
