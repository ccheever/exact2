// The Diff file header's copy button (T3 Code 2359643cff, MIT, see LICENSE-T3: DiffFilePathCopyButton and
// ui/anchoredCopyToast.ts showAnchoredCopyErrorToast): a refused copy is an error toast with the reason.
import { expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { obj } from '../../src/client/domain';
import type { Files, Native } from '../../src/client/protocol';
import { toasts } from '../../src/shell/toast';

function rig(refuse: boolean) {
  const copied: string[] = [];
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input);
    if (request.op !== 'copyText') return { ok: true, generation: 1, value: {} };
    if (refuse) return { ok: false, generation: 1, error: { kind: 'Server', message: 'The pasteboard is unavailable.', uncertain: false } };
    copied.push(String(request.text));
    return { ok: true, generation: 1, value: {} };
  } };
  const storage: Files = { fs: { async mkdir() {}, async readFile() { throw new Error('missing'); }, async atomicWriteFile() {} } };
  const client = new T3Client();
  Object.assign(client, { available: true, generation: 1, connection: 'connected', environmentId: 'env', loaded: true });
  return { client, copied, copy: () => client.command('diff-view', 'copy', 'src/a.ts', 0, native, storage) };
}

test('a copied path says so; a refused one is "Failed to copy" with the reason', async () => {
  const done = rig(false);
  expect((await done.copy()).message).toBe('Copied file path');
  expect(done.copied).toEqual(['src/a.ts']);
  const refused = rig(true);
  expect((await refused.copy()).message).toBe('');
  expect(toasts(refused.client).at(-1)).toMatchObject({ kind: 'error', title: 'Failed to copy', description: 'The pasteboard is unavailable.' });
});
