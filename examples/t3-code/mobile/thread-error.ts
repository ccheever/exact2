import type { T3Client } from './shared/client';
import { threadErrorView } from './shared/timeline-errors';

interface ThreadErrorBase { error: string; clientError: string }

/** T3's ThreadErrorBanner shows provider JSON messages while keeping the raw text available to Copy/AX. */
export function threadErrorBannerMessage(error: string): string {
  let payload: unknown;
  try { payload = JSON.parse(error); } catch { return error; }
  if (typeof payload !== 'object' || payload === null || !('error' in payload)) return error;
  const nested = (payload as { error: unknown }).error;
  if (typeof nested !== 'object' || nested === null || !('message' in nested)) return error;
  const message = (nested as { message: unknown }).message;
  return typeof message === 'string' && message.trim().length > 0 ? message : error;
}

export function threadErrorExpandable(error: string, summary: string): boolean {
  return summary !== error || error.length > 180 || error.includes('\n');
}

export function mobileThreadErrorView<T extends ThreadErrorBase>(client: T3Client, base: T): T & { errorWarning: boolean; errorSummary: string; errorExpandable: boolean } {
  const source = threadErrorView(client, { ...base, errorWarning: false });
  const summary = threadErrorBannerMessage(source.error);
  return { ...source, errorSummary: summary, errorExpandable: threadErrorExpandable(source.error, summary) };
}
