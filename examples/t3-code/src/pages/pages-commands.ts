// Local commands of the pages (pageslocal:*): page preferences, the
// clipboard actions of the Usage, Pull Requests and Welcome pages, and the
// pull request writes (pr-act-*), which report their own outcome as toasts.
import type { T3Client } from '../client/client';
import { ClientError, bridgeReply, type Files, type Native } from '../client/protocol';
import { usageLocal, forgetUsage } from '../usage/pages-usage';
import { usagePoolLocal } from '../usage/usage-environments';
import { usagePricesLocal, usagePricesSave } from '../usage/pages-usage-prices';
import { setOpenModel } from '../usage/pages-usage-detail';
import { prLocal } from '../pull-requests/pages-prs';
import { prQuickLocal } from '../pull-requests/pages-pr-quick';
import { prCommand, noteCopy } from '../pull-requests/pages-pr-detail';
import { pushToast } from '../shell/toast';
import { welcomeLocal } from './pages-welcome';
import { CHATGPT_USAGE_URL } from '../providers/chatgpt-plan';
import { pullRequestLinkPick } from '../panel/context-menu-actions';

export async function pagesLocal(client: T3Client, native: Native | null | undefined, _storage: Files, op: string, id: string, value: string): Promise<string> {
  if (op === 'usage-prices-save') {
    if (!native?.available) throw new ClientError('Open on macOS to change model prices.');
    return usagePricesSave(client, native, () => forgetUsage(client));
  }
  // Set price closes the model dialog and opens Model prices with that model's row (UsagePage).
  if (op === 'usage-prices-open') setOpenModel(client, '');
  if (op.startsWith('usage-prices-')) return usagePricesLocal(client, op.slice(13), id, value);
  // ChatGptUsageButton on the Usage page and in the model picker opens ChatGPT's usage settings.
  if (op === 'usage-chatgpt' || op === 'chatgpt-usage') {
    if (native?.available) await bridgeReply(native, { op: 'remoteEditorsOpen', url: CHATGPT_USAGE_URL });
    return '';
  }
  if (op.startsWith('usage-pool-')) return usagePoolLocal(client, native, op.slice(11), id); // environments, redeem, Cursor
  if (op.startsWith('usage-')) return usageLocal(client, op.slice(6), value);
  if (op.startsWith('welcome-')) {
    if (!native?.available) throw new ClientError('Open on macOS to set up T3 Code.');
    return welcomeLocal(client, native, op.slice(8), id, value);
  }
  // A pull request row number's menu choice (the detail header's goes through chatLocal).
  if (op === 'pr-link-pick') {
    if (native?.available) await pullRequestLinkPick(client, native, value);
    return '';
  }
  if (op.startsWith('pr-act-')) {
    if (!native?.available) throw new ClientError('Open on macOS to change pull requests.');
    return prCommand(client, native, op.slice(7), id, value, _storage);
  }
  if (op.startsWith('pr-quick-')) return prQuickLocal(client, op.slice(9), value); // a row's popovers
  if (op.startsWith('pr-')) return prLocal(client, op.slice(3), value);
  if (op === 'copy') {
    if (!native?.available) throw new ClientError('Open on macOS to copy.');
    const reply = await bridgeReply(native, { op: 'copyText', text: value });
    // useCopyToClipboard's onCopy/onError: "PR number copied", "Failed to copy PR link".
    if (!reply.ok) {
      if (id) { pushToast(client, { kind: 'error', title: `Failed to copy ${id}`, description: reply.error?.message || 'Could not copy.' }); return ''; }
      throw new ClientError(reply.error?.message || 'Could not copy.');
    }
    if (id) pushToast(client, { kind: 'success', title: `${id} copied` });
    else noteCopy(client, value); // PullRequestCopyableCode: no toast, the value reads "Copied" in place
    return '';
  }
  throw new ClientError(`Unknown action: ${op}`);
}
