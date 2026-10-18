import { requestComposerField } from './composer-field-request';
import type { T3Client } from '../client/client';
import type { Native } from '../client/protocol';
import { bridgeReply } from '../client/protocol';
import { obj, type Obj } from '../client/domain';
import { claimComposerOSRecord, composerOSRecord } from './composer-os-record';
import { pushToast } from '../shell/toast';
import { composerFieldOwner } from './auto-balance-owner';

/** OS extraction supplies folder paths; only the shared field edits source. */
export async function takeFolderDrop(client: T3Client, native: Native, value: Obj): Promise<void> {
  const generation = client.generation;
  const currentOwner = () => generation === client.generation && value.snapshotOwner === client.snapshotOwner && value.fieldOwner === composerFieldOwner(client);
  const paths = value.paths;
  const valid = Array.isArray(paths) && paths.length > 0 && paths.length <= 100
    && paths.every((path): path is string => typeof path === 'string' && path.startsWith('/') && !/[\r\n\0]/.test(path));
  const claimed = claimComposerOSRecord(client, value), fact = valid && claimed.kind === 'accepted' ? claimed.record : null;
  if (Number.isSafeInteger(value.id) && Number(value.id) > 0) await bridgeReply(native, { op: 'editorForgetFold', id: value.id });
  if (!fact || !valid || !composerOSRecord(client, value)) {
    if (currentOwner() && (claimed.kind === 'accepted' || claimed.kind === 'refused' && claimed.notify)) pushToast(client, { kind: 'info', title: 'Folders were not added', description: 'The prompt changed before the folders could be inserted.', hideCopy: true });
    return;
  }
  const text = paths.map(path => `@${/[\s"\\]/.test(path) ? `"${path.replace(/\\/g, '\\\\').replace(/"/g, '\\"')}"` : path}`).join(' ') + ' ';
  const reply = await requestComposerField(native, { op: 'editorEdit', snapshotOwner: fact.snapshotOwner, fieldOwner: fact.fieldOwner, value: fact.value,
    start: fact.start, end: fact.end, expect: fact.expect, text, pad: true });
  if (currentOwner() && !reply.accepted) client.error = 'The prompt changed before the folders could be added.';
}
