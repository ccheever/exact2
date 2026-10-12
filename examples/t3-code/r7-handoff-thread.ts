// Lane r7-handoff: the thread id a draft launches as (MIT reference, see LICENSE-T3,
// upstream f90b77d809): useHandleNewThread gives every draft its own ThreadId up front
// (newThreadId), and the first send launches the thread under that id. A pull request
// hand-off (usePullRequestHandoffs.startHandoff) opens the thread before the checkout and
// passes that id to `git.preparePullRequestThread`, because the server runs the project's
// setup script (ProjectSetupScriptRunner.runForThread) only for a checkout that knows which
// thread it is for: the setup terminal is opened under the thread id, and the thread the
// draft later launches as is the one that terminal belongs to.
//
// This client allocates a draft's id when a hand-off needs it, keeps it with the draft's
// workspace preferences (so a relaunch still sends as that thread), launches with it and
// forgets it once the thread exists.
import type { T3Client } from './client';
import type { Obj } from './domain';
import type { Native } from './protocol';

type DraftThreads = Record<string, string>;
const store = (client: T3Client): DraftThreads => (client.local.composerControls.draftThreads ??= {});

/** The id this draft will launch as, or '' when it has none yet. */
export function draftThreadId(client: T3Client, draftKey = client.draftKey): string {
  return client.local.composerControls.draftThreads?.[draftKey] ?? '';
}

/** newThreadId for a draft that does not have one: allocated once, then kept with the draft. */
export async function ensureDraftThreadId(client: T3Client, native: Native, draftKey = client.draftKey): Promise<string> {
  const existing = draftThreadId(client, draftKey);
  if (existing) return existing;
  const [id] = await client.restAccess(native).ids(1);
  // The draft may have changed while the id was out: the id belongs to the draft that asked.
  return (store(client)[draftKey] ??= id!);
}

/** The launch's thread id: the draft's own when it has one, else the freshly allocated id. */
export function launchThreadId(client: T3Client, draftKey: string, fresh: string): string {
  return draftThreadId(client, draftKey) || fresh;
}

/** The thread exists now: the draft that held its id starts over with a new one next time. */
export function forgetDraftThreadId(client: T3Client, draftKey: string): void {
  const threads = client.local.composerControls.draftThreads;
  if (threads && draftKey in threads) delete threads[draftKey];
}

/** git.preparePullRequestThread's input with the draft's thread, so the server runs the setup script for it. */
export function withHandoffThread(payload: Obj, threadId: string): Obj {
  return threadId ? { ...payload, threadId } : payload;
}
