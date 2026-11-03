import type { T3Client } from './shared/client';
import { obj, str } from './shared/domain';
import { bridgeReply, ClientError, type Native } from './shared/protocol';
import { serverThreadError } from './shared/requests';
import { threadErrorView } from './shared/timeline-errors';
import { letGo } from './shared/let-go';
import { pushToast } from './shared/toast';
import { CHATGPT_USAGE_URL } from './settings-usage-limits';

interface ThreadErrorBase { error: string; clientError: string }

export const CHATGPT_USAGE_LIMIT_CODE = 'subscription_sharing_usage_limit_exceeded';
export const CHATGPT_WORKSPACE_USAGE_LIMIT_CODE = 'subscription_sharing_workplace_individual_usage_limit_exceeded';
export const CHATGPT_WORKSPACE_ADMIN_REQUEST = "I've reached the limit for using my ChatGPT plan in T3 Code. Could you increase our workspace's usage limits so I can continue?\n\nGo to admin.openai.com → Workspace → Usage limits. Thank you!";

export type ThreadErrorLimit = '' | 'workspace' | 'chatgpt';

/** Mobile-only recovery actions; the pinned shared command lanes remain source-identical. */
export async function mobileThreadErrorAction(client: T3Client, native: Native, op: string): Promise<boolean> {
  if (op === 'admin-request') {
    try { await client.restAccess(native).call({ op: 'copyText', text: CHATGPT_WORKSPACE_ADMIN_REQUEST }); }
    catch (error) {
      if (letGo(error)) throw error;
      pushToast(client, { kind: 'error', title: 'Could not copy the request', description: error instanceof Error ? error.message : 'Could not copy the request.' });
      return true;
    }
    pushToast(client, { kind: 'success', title: 'Request copied', description: 'Send it to your workspace admin.' });
    return true;
  }
  if (op === 'chatgpt-usage') {
    const reply = await bridgeReply(native, { op: 'mobileOpenURL', url: CHATGPT_USAGE_URL });
    if (!reply.ok || obj(reply.value).opened !== true) throw new ClientError('The usage URL was not opened.');
    return true;
  }
  return false;
}

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

function threadErrorLimit(client: T3Client, base: ThreadErrorBase, error: string): ThreadErrorLimit {
  const shell = client.shell.threads.find(thread => thread.id === client.threadId);
  const server = client.threadId ? serverThreadError(client.projection) : '';
  const fromServer = !base.clientError && !!server && error === server;
  if (!fromServer || !error) return '';
  const code = str(obj(shell).lastErrorCode);
  return code === CHATGPT_WORKSPACE_USAGE_LIMIT_CODE ? 'workspace' : code === CHATGPT_USAGE_LIMIT_CODE ? 'chatgpt' : '';
}

export function mobileThreadErrorView<T extends ThreadErrorBase>(client: T3Client, base: T): T & { errorWarning: boolean; errorSummary: string; errorExpandable: boolean; errorLimit: ThreadErrorLimit } {
  const source = threadErrorView(client, { ...base, errorWarning: false });
  const summary = threadErrorBannerMessage(source.error);
  const errorLimit = threadErrorLimit(client, base, source.error);
  return { ...source, errorWarning: source.errorWarning || errorLimit !== '', errorSummary: summary,
    errorExpandable: threadErrorExpandable(source.error, summary), errorLimit };
}
