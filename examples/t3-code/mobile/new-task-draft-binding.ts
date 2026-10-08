// One foreground ID per containing NewTask flow; no project-slot content transfer.
// @ref llp/1109.005-composer-and-transcript.decision.md#new-task-ownership
import type { T3Client } from './shared/client';
import { ClientError, type Native, type Files } from './shared/protocol';
import { isScratch, scratchRootOf } from './shared/r12-threads-scratch';
import { patchDraftContext } from './shared/composer-controls-branch';
import { composerNow } from './shared/composer-controls';
import { mobileQueuedEditOrigin } from './queued-edit-origin';
import { mobileNewTaskDraftCreate, mobileNewTaskDraftLookup, mobileNewTaskDraftRetarget, mobileNewTaskDraftBind,
  mobileNewTaskDraftChoicesRestore } from './mobile-new-task-drafts';

/** Selection is already the actual loaded server project. The outer native
 * scope rejects every stale await, including ID allocation and persistence. */
export async function mobileBindNewTaskDraft(client: T3Client, owner: string, previousKey: string, resumeKey: string,
  native: Native, storage: Files, current: () => boolean): Promise<string> {
  const captured = JSON.stringify([client.origin, client.generation, client.environmentId, client.projectId, client.threadId, client.threadEpoch]);
  const assertCurrent = () => {
    if (!current() || captured !== JSON.stringify([client.origin, client.generation, client.environmentId, client.projectId, client.threadId, client.threadEpoch]))
      throw new ClientError('The new task selection changed.', 'superseded');
  };
  assertCurrent();
  const stamp = { environmentId: client.environmentId, projectId: client.projectId, origin: mobileQueuedEditOrigin(client) };
  let key = resumeKey || previousKey;
  if (key) {
    const record = mobileNewTaskDraftLookup(client, key);
    if (!record) throw new ClientError('That saved draft is no longer available.');
    if (resumeKey) {
      if (record.environmentId !== stamp.environmentId || record.projectId !== stamp.projectId || record.origin !== stamp.origin)
        throw new ClientError('The saved draft belongs to a different environment or project.');
    } else if (!mobileNewTaskDraftRetarget(client, key, stamp)) throw new ClientError('The selected draft could not be moved.');
  } else {
    const now = composerNow(client);
    if (!Number.isFinite(now) || now <= 0) throw new ClientError('Wait for the app clock before creating this draft.');
    const [id] = await client.ids(native, 1); assertCurrent();
    key = mobileNewTaskDraftCreate(client, { ...stamp, id, createdAt: new Date(now).toISOString() }).key;
  }
  if (!mobileNewTaskDraftBind(client, key, owner)) throw new ClientError('The new task selection changed.', 'superseded');
  mobileNewTaskDraftChoicesRestore(client, key);
  const project = client.shell.projects.find(project => project.id === client.projectId);
  if (isScratch(project, scratchRootOf(client.ready, client.config))) patchDraftContext(client, { envMode: 'local', branch: '', worktreePath: '' }, key);
  await client.persist(storage); assertCurrent();
  return key;
}
