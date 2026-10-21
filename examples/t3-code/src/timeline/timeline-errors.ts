// ThreadErrorBanner's variant and message, and ChatView's timelineThreadError (T3 Code 74541bab17,
// MIT; see LICENSE-T3): a usage-limit failure of the latest run belongs to the
// composer's usage banner, so the thread banner stays quiet for it; any other
// server error of class usage_limit reads as a warning, not an error.
import { str, type Obj } from '../client/domain';
import type { T3Client } from '../client/client';
import { serverThreadError } from '../composer/requests';

/**
 * getThreadErrorBannerMessage: a provider's JSON error (`{"error":{"message":…}}`) reads as its message, whitespace kept;
 * anything else, a blank message included, stays as it is. The banner expands to the exact text.
 */
export function threadErrorBannerMessage(error: string): string {
  let payload: unknown;
  try { payload = JSON.parse(error); } catch { return error; }
  if (typeof payload !== 'object' || payload === null || !('error' in payload)) return error;
  const nested = (payload as { error: unknown }).error;
  if (typeof nested !== 'object' || nested === null || !('message' in nested)) return error;
  const message = (nested as { message: unknown }).message;
  return typeof message === 'string' && message.trim().length > 0 ? message : error;
}
/** ExpandableText's canExpand: a summary that differs from the text, or a long or multi-line text. */
export function errorExpandable(error: string, summary: string): boolean {
  return summary !== error || error.length > 180 || error.includes('\n');
}

export function threadErrorView<T extends { error: string; clientError: string }>(client: T3Client, base: T): T & { errorWarning: boolean; errorSummary: string; errorExpandable: boolean } {
  const shell: Obj | undefined = client.shell.threads.find((thread: Obj) => thread.id === client.threadId);
  const server = client.threadId ? serverThreadError(client.projection) : '';
  const fromServer = !base.clientError && !!server && base.error === server;
  const usageLimit = fromServer && str(shell?.lastErrorClass) === 'usage_limit';
  const handledByComposer = usageLimit && shell?.status === 'failed' && !!(shell.latestRun || shell.latestRunId);
  const error = handledByComposer ? '' : base.error, summary = threadErrorBannerMessage(error);
  return { ...base, error, errorWarning: usageLimit && !handledByComposer, errorSummary: summary, errorExpandable: errorExpandable(error, summary) };
}
