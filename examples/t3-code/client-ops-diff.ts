// The changes panel's client.command() ops (client-ops.ts): opening it on a
// turn's checkpoint or a scope, refetching, the whitespace and view options,
// copying a file path, and closing it, and the Changes scope's comparison target
// (diff-base-ref.ts). A stale answer never replaces a newer one.
import type { T3Client } from './client';
import type { OpOut } from './client-ops';
import { message } from './client-shared';
import { adoptDiff, currentSelection, diffPaths, diffRequest, diffView, selectBaseRef, selectCheckpoint, selectScope, turnSummaries } from './diff';
import { loadBaseRefs } from './diff-base-ref';
import { rememberDiffLayout } from './settings-appearance-look';
import { requestDiff } from './r11-device-diff';
import { diffReview, loadDiffFiles } from './diff-review';
import { type Native, type Files } from './protocol';
import { letGo } from './let-go';

/** The changes panel: open (a turn's checkpoint, a scope), refresh, whitespace, view options, copy a path, close. */
export async function diffOps(this: T3Client, op: string, id: string, value: string, n: number, native: Native, storage: Files, out: OpOut): Promise<boolean> {
  let resultMessage = '';
  try {
    if (op === 'close-diff') { this.diffOpen = false; this.diffLoading = false; }
    else if (op === 'diff-view' && id === 'copy') { await this.call(native, { op: 'copyText', text: value }); resultMessage = 'Copied file path'; }
    else if (op === 'diff-view') { diffView(this, id, value, diffPaths(this)); if (id === 'layout') rememberDiffLayout(this, value); }
    else if (op === 'diffreview') resultMessage = await diffReview(this, native, id, value, n); // diff-review.ts: tree reveal, large diffs, hidden lines, line comments
    else if (op === 'diffbase') await baseRefs(this, native, id, value);
    else if (['diff', 'checkpoint-diff', 'diff-scope', 'diff-base', 'diff-refresh', 'diff-whitespace'].includes(op)) await diff.call(this, native, op, id, value, n);
    else return false;
    return true;
  } finally { Object.assign(out, { message: resultMessage, id, value }); }
}
/** The changes panel asks the server for the current selection; a stale answer never replaces a newer one. */
async function diff(this: T3Client, native: Native, op: string, id: string, value: string, n: number): Promise<void> {
  // Only opening commands open the panel; a refetch queued behind a close never reopens it.
  if (op !== 'diff' && op !== 'checkpoint-diff' && !this.diffOpen) {
    if (op === 'diff-whitespace') this.diffState.ignoreWhitespace = !this.diffState.ignoreWhitespace;
    return;
  }
  // The scope menu lists Latest turn with no turns too; choosing it then only closes the menu (DiffPanel selectScopeValue).
  if (op === 'diff-scope' && value === 'latest' && !turnSummaries(this.projection).length) { this.diffState.menu = ''; return; }
  this.diffOpen = true; this.diffError = '';
  let request;
  try {
    if (op === 'checkpoint-diff') selectCheckpoint(this, n, id);
    else if (op === 'diff-scope') selectScope(this, value);
    else if (op === 'diff-base') selectBaseRef(this, value); // the comparison target picker's choice (Automatic, a ref, its remote switch)
    else if (op === 'diff') selectScope(this, 'branch'); // generic opens show Changes (diff.ts, upstream d1034d62b2)
    else if (op === 'diff-whitespace') this.diffState.ignoreWhitespace = !this.diffState.ignoreWhitespace;
    request = diffRequest(this);
  } catch (error) { this.diffText = ''; this.diffError = message(error); return; }
  if (request.scope !== this.diffState.scopeKey) this.diffText = '';
  const epoch = this.threadEpoch, threadId = this.threadId, asked = JSON.stringify(request);
  this.diffLoading = true;
  try {
    const result = await requestDiff(this.config, request, (method, payload) => this.request(native, method, payload)); // r11-device: DiffPanel's server-cwd retry
    if (epoch === this.threadEpoch && threadId === this.threadId && this.diffOpen && JSON.stringify(diffRequest(this)) === asked) {
      // A named file stays collapsed, as in the reference (onOpenTurnDiff leaves 'Expand <file>' false).
      this.diffText = adoptDiff(this, request, result);
      await loadDiffFiles(this, native); // diff-review.ts: a large preview's first four files
    }
  } catch (error) { if (epoch === this.threadEpoch && !letGo(error)) this.diffError = message(error); }
  finally { if (epoch === this.threadEpoch) this.diffLoading = false; }
}
/** `diffbase`: the comparison target picker opened (`open`, a fresh query) or its search edited (`query`): both ref lists are read again at the preview's cwd. */
async function baseRefs(client: T3Client, native: Native, id: string, value: string): Promise<void> {
  const source = client.diffState.source, picker = client.diffState.baseRefs;
  picker.query = id === 'query' ? value : '';
  if (!source || source.kind !== 'branch-range' || currentSelection(client).kind !== 'branch') return;
  await loadBaseRefs(picker, source.cwd, (method, payload) => client.rpc(native, method, payload));
}
