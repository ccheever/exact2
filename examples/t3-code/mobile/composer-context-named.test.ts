// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { mobileComposerTarget } from './composer-target';
import { mobileComposerContextCapture, mobileComposerContextCommit, mobileComposerContextCaptureTarget as capture,
  mobileComposerContextObserveTarget as observe, mobileComposerContextRead as read,
  mobileComposerContextsHydrate as hydrate, mobileComposerContextsPersisted as persisted } from './composer-command-context';
const link = '[a](t3-context://v1/mention/one)';
const record = { version: 1, kind: 'mention', contextId: 'one', label: 'a', path: 'a.ts' };
function fixture() {
  const client = new T3Client(); Object.assign(client, { origin: 'https://named.test', environmentId: 'e', threadId: 'a', generation: 4 });
  const target = mobileComposerTarget(client);
  expect(mobileComposerContextCommit(client, mobileComposerContextCapture(client, target)!, link, record)).toBe(true);
  client.threadId = 'b'; client.local.drafts['e:b'] = 'other';
  return { client, target };
}
test('named off-focus writes prune and restore their own context history without changing the focused slot', () => {
  const { client, target } = fixture(), guard = capture(client, target)!;
  client.local.drafts['e:a'] = ''; expect(observe(client, guard, '')).toBe(true);
  expect(read(client, 'e:a')).toMatchObject({ context: undefined }); expect(client.draft).toBe('other');
  const undo = capture(client, target)!; client.local.drafts['e:a'] = link;
  expect(observe(client, undo, link)).toBe(true); expect(read(client, 'e:a')).toMatchObject({ context: { records: [record] } });
  expect(observe(client, guard, link)).toBe(false); expect(client.draft).toBe('other');
});
test('new named seam refuses changed connection, wrong target, unobserved mismatch and replaced context incarnation', () => {
  const { client, target } = fixture(), guard = capture(client, target)!;
  expect(capture(client, { ...target, environmentId: 'wrong' })).toBeNull();
  expect(capture(client, { ...target, key: 'e:b' })).toBeNull();
  client.generation++; expect(capture(client, target)).toBeNull(); expect(observe(client, guard, link)).toBe(false); client.generation--;
  client.local.drafts['e:a'] = 'unobserved'; expect(capture(client, target)).toBeNull();
  client.local.drafts['e:a'] = link;
  const fresh = new T3Client(); Object.assign(fresh, { origin: client.origin, environmentId: 'e', threadId: 'b', generation: 4 });
  fresh.local.drafts['e:a'] = link;
  const beforeLoad = capture(fresh, target)!;
  hydrate(fresh, { mobileComposerContexts: persisted(client) });
  expect(observe(fresh, beforeLoad, link)).toBe(false);
});
