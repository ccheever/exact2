// A side chat's own chat view (T3 Code 2359643cff, MIT, see LICENSE-T3: SideChatPane renders `ChatView` with the side
// chat's thread and `embedded`). The pane draws the window's timeline rows and composer from a snapshot of the side
// chat's thread (thread-scope.ts): the same presentation as the window's, over the client seen through that thread.
// What the pane presses goes back as `scope:<thread id>|<op>`, which runs the window's command on the scope.
import type { T3Client } from '../client/client';
import type { Files, Native } from '../client/protocol';
import { threadScope, windowClient } from '../client/thread-scope';

export const SCOPE_PREFIX = 'scope:';

/** `scope:<thread id>|<op>` as its thread and op; null for a window op. */
export function scopedOp(op: string): { threadId: string; op: string } | null {
  if (!op.startsWith(SCOPE_PREFIX)) return null;
  const bar = op.indexOf('|');
  return bar > SCOPE_PREFIX.length ? { threadId: op.slice(SCOPE_PREFIX.length, bar), op: op.slice(bar + 1) } : null;
}

/**
 * Ops that act on the window wherever they are pressed: they open a thread or a page, or change the window. A side
 * chat's row or notice that opens another thread opens it in the window, as ChatView's `onOpenThread` does there.
 */
const WINDOW_OPS = new Set(['select-thread', 'new-thread', 'sidebar', 'search', 'notification-open', 'device-setting', 'settings-core']);
const windowOp = (op: string) => WINDOW_OPS.has(op) || op.startsWith('shell:') || op.startsWith('shelllocal:') || op.startsWith('sidebar:')
  || op.startsWith('sidebarlocal:') || op.startsWith('terminallocal:') || op.startsWith('terminalpanellocal:') || op.startsWith('pageslocal:');

/** Runs `op`, scoped or not, on the client it names. */
export function scopedCommand(client: T3Client, op: string, id: string, value: string, n: number, native: Native | null | undefined, storage: Files) {
  const scoped = scopedOp(op);
  if (!scoped) return client.command(op, id, value, n, native, storage);
  const target = windowOp(scoped.op) ? client : threadScope(client, scoped.threadId);
  return target.command(scoped.op, id, value, n, native, storage);
}

/** The client a resource asked for `threadId` reads: the side chat's scope, or the window's for ''. */
export const sideChatClient = (client: T3Client, threadId: string): T3Client => threadScope(client, threadId);

/** The composer a chip's details popover serves (the root's `chipTarget`, as the `composerChip` resource last asked). */
const chipTargets = new WeakMap<T3Client, string>();
export function noteChipTarget(client: T3Client, threadId: string): void { chipTargets.set(windowClient(client), threadId); }
/**
 * A chip details' op the window draws (its close, View instructions, a citation's comment, a chip's action) runs on the
 * composer it serves: a side chat's scope. The image and file previews are the window's overlays.
 */
export function chipScopedOp(client: T3Client, op: string): string {
  const target = chipTargets.get(windowClient(client)) ?? '', chip = op.startsWith('editorlocal:') ? op.slice('editorlocal:'.length) : '';
  return target && (chip === 'chip-close' || chip === 'chip-instructions' || chip === 'chip-act' || chip.startsWith('chip-cite-')) ? `${SCOPE_PREFIX}${target}|${op}` : op;
}
