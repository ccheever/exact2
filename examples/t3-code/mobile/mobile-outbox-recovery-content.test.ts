import { expect, test } from 'bun:test';
import { mobileOutboxRecoveryKey as key, mobileOutboxRecoveryMergeContent as merge,
  type MobileOutboxRecoveryContent } from './mobile-outbox-recovery-content';
import type { MobileOutboxAttachment, MobileOutboxRecord } from './mobile-outbox-model';
import { contextLink } from './shared/composer-editor-menu';
const record = (patch: Partial<MobileOutboxRecord> = {}): MobileOutboxRecord => ({ schemaVersion: 1,
  origin: 'https://recovery.test', environmentId: 'env', threadId: 'thread', messageId: 'message', commandId: 'command',
  text: 'queued', attachments: [], createdAt: '2026-10-08T00:00:00.000Z', ...patch });
const empty = (): MobileOutboxRecoveryContent => ({ text: '', attachments: [] });
const image = (n: number): MobileOutboxAttachment => ({ kind: 'image', id: `00000000-0000-4000-a000-${String(n).padStart(12, '0')}`,
  name: `image-${n}`, mimeType: 'image/png', sizeBytes: 1, uploadId: '', status: 'staged' });
const terminal = (contextId: string, text = 'captured') => ({ version: 1, kind: 'terminal', contextId, label: 'Terminal',
  terminalId: 'pane', terminalLabel: 'Terminal', lineStart: 1, lineEnd: 1, text });
const link = (id: string) => contextLink('terminal', id, 'Terminal');
function content(result: ReturnType<typeof merge>): MobileOutboxRecoveryContent {
  if (!result.ok) throw new Error(result.reason); return result.content;
}
test('accepted creation edits target its thread while rejected creation has one deterministic separate draft', () => {
  const creation = record({ creation: { projectId: 'project', workspaceMode: 'local', branch: null, worktreePath: null } });
  expect(key(creation, 'accepted-edits')).toBe('env:thread');
  expect(key(creation, 'rejected')).toBe('new-task:restored-message');
  expect(key(record(), 'rejected')).toBe('env:thread');
});
test('merge preserves newer text and is idempotent for retry and empty input without trimming', () => {
  const incoming = record({ text: '  queued\n' }), original = { ...empty(), text: 'newer typing' };
  const first = content(merge(incoming, original, 'accepted-edits'));
  expect(first.text).toBe('newer typing\n\n  queued\n');
  expect(content(merge(incoming, first, 'accepted-edits'))).toEqual(first);
  expect(content(merge(record({ text: '' }), first, 'rejected'))).toEqual(first);
  expect(content(merge(incoming, empty(), 'rejected')).text).toBe(incoming.text);
  expect(original.text).toBe('newer typing');
});
test('incoming context replaces matching IDs, prunes unused records and returns detached payloads', () => {
  const existing = { ...empty(), text: link('old'), context: { version: 1, records: [terminal('old'), terminal('unused')] } };
  const incoming = record({ text: link('new'), context: { version: 1, records: [terminal('old', 'replacement'), terminal('new')] } });
  const merged = content(merge(incoming, existing, 'accepted-edits'));
  expect(merged.context).toEqual({ version: 1, records: [terminal('old', 'replacement'), terminal('new')] });
  (merged.context!.records as ReturnType<typeof terminal>[])[0].text = 'caller mutation';
  expect((incoming.context!.records as ReturnType<typeof terminal>[])[0].text).toBe('replacement');
  expect(existing.context.records[0].text).toBe('captured');
});
test('unknown context stays recoverable and preview screenshot dependency stays attached', () => {
  const screenshot = { version: 1, kind: 'image', contextId: 'shot', label: 'Image' };
  const note = { version: 1, kind: 'preview-annotation', contextId: 'note', label: 'Note', screenshotContextId: 'shot', futurePayload: { captured: true } };
  const incoming = record({ text: contextLink('preview-annotation', 'note', 'Note'), context: { version: 1, records: [screenshot, note] } });
  expect(content(merge(incoming, empty(), 'accepted-edits')).context).toEqual(incoming.context);
});
test('attachments preserve existing identity and order; ACK overflow retains every image', () => {
  const images = Array.from({ length: 100 }, (_, i) => image(i));
  const original = { ...empty(), attachments: images }, incoming = record({ attachments: [{ ...image(1), name: 'duplicate' }, image(100)] });
  expect(merge(incoming, original, 'rejected')).toEqual({ ok: false,
    reason: 'Remove attachments from the draft before restoring this message. Messages can contain at most 100 attachments.' });
  const first = content(merge(incoming, original, 'accepted-edits'));
  expect(first.attachments).toEqual([...images, image(100)]);
  expect(content(merge(incoming, first, 'accepted-edits'))).toEqual(first);
  first.attachments[0].name = 'caller'; expect(original.attachments[0].name).toBe('image-0');
});
test('absent settings preserve target choices; present queued settings replace only their own field', () => {
  const original: MobileOutboxRecoveryContent = { ...empty(), modelSelection: { instanceId: 'provider', model: 'chosen' },
    runtimeMode: 'auto', interactionMode: 'plan' };
  expect(content(merge(record(), original, 'rejected'))).toMatchObject({ ...original, text: 'queued' });
  expect(content(merge(record({ interactionMode: 'default' }), original, 'accepted-edits'))).toMatchObject({
    modelSelection: original.modelSelection, runtimeMode: 'auto', interactionMode: 'default' });
});
test('invalid target or queued content blocks before merge without losing either input', () => {
  const bad = { ...empty(), context: { version: 2, records: [] } }, incoming = record();
  expect(merge(incoming, bad, 'rejected').ok).toBe(false); expect(bad.context.version).toBe(2);
  expect(merge(record({ attachments: [image(1), image(1)] }), empty(), 'accepted-edits').ok).toBe(false);
  expect(merge(record({ attachments: [{ ...image(1), uploadId: 'wrong-home' }] }), empty(), 'rejected').ok).toBe(false);
  expect(incoming.text).toBe('queued');
});
