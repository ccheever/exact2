// ThreadErrorBanner's variant and ChatView's timelineThreadError (T3 Code,
// MIT; see LICENSE-T3): a usage-limit failure of the latest run belongs to the
// composer's usage banner, so the thread banner stays quiet for it; any other
// server error of class usage_limit reads as a warning, not an error.
import { str, type Obj } from '../client/domain';
import type { T3Client } from '../client/client';
import { serverThreadError } from '../composer/requests';

export function threadErrorView<T extends { error: string; clientError: string }>(client: T3Client, base: T): T & { errorWarning: boolean } {
  const shell: Obj | undefined = client.shell.threads.find((thread: Obj) => thread.id === client.threadId);
  const server = client.threadId ? serverThreadError(client.projection) : '';
  const fromServer = !base.clientError && !!server && base.error === server;
  const usageLimit = fromServer && str(shell?.lastErrorClass) === 'usage_limit';
  const handledByComposer = usageLimit && shell?.status === 'failed' && !!(shell.latestRun || shell.latestRunId);
  return { ...base, error: handledByComposer ? '' : base.error, errorWarning: usageLimit && !handledByComposer };
}
