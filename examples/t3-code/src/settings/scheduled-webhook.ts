// MIT T3 Code 9c161462c9 (LICENSE-T3): scheduledTasksSettings.logic.ts,
// scheduledTaskWebhook.ts and webhookAddress.ts. Secrets are write-only.
import { obj, str, type Obj } from '../client/domain';
export const DEFAULT_WEBHOOK_PROMPT = 'Handle this webhook:\n{{body}}';
export const MAX_WEBHOOK_DELIVERY_AGE_MINUTES = 1440;
export function parseMaxDeliveryAge(value: string): number | null | undefined {
  if (value.trim() === '') return null;
  const minutes = Number(value.trim());
  return Number.isInteger(minutes) && minutes > 0 && minutes <= MAX_WEBHOOK_DELIVERY_AGE_MINUTES ? minutes : undefined;
}
export function webhookDraft(task?: Obj) {
  const schedule = obj(task?.schedule), signature = obj(schedule.signature);
  return { signatureEnabled: schedule.type === 'webhook' && schedule.signature != null,
    signatureHeader: str(signature.header, 'x-hub-signature-256'), signatureEncoding: str(signature.encoding, 'hex'),
    signaturePrefix: str(signature.prefix, 'sha256='), signatureSecret: '',
    maxDeliveryAgeMinutes: schedule.type === 'webhook' && schedule.maxDeliveryAgeMinutes != null ? String(schedule.maxDeliveryAgeMinutes) : '',
    hasSecret: obj(task?.webhook).hasSecret === true };
}
export function webhookSchedule(input: Record<string, string>, existing?: Obj): Obj {
  const maxDeliveryAgeMinutes = parseMaxDeliveryAge(str(input.maxDeliveryAgeMinutes));
  if (maxDeliveryAgeMinutes === undefined) throw new Error(`Invalid age limit: Enter whole minutes from 1 to ${MAX_WEBHOOK_DELIVERY_AGE_MINUTES}, or leave it blank.`);
  const secret = str(input.signatureSecret).trim(), header = str(input.signatureHeader).trim();
  const enabled = input.signatureEnabled === 'true';
  if (enabled && (!header || (!secret && !(obj(existing?.schedule).type === 'webhook' && obj(existing?.webhook).hasSecret === true)))) throw new Error('Signing secret is required: Enter the signature header and secret.');
  return { type: 'webhook', maxDeliveryAgeMinutes, signature: enabled ? { header, encoding: input.signatureEncoding === 'base64' ? 'base64' : 'hex', prefix: str(input.signaturePrefix), ...(secret ? { secret } : {}) } : null };
}
export function webhookAddress(endpoint: Obj | undefined, httpBaseUrl: string) {
  if (!endpoint) return { address: '', copyable: false, note: 'The URL appears after you save.' };
  if (str(endpoint.url)) return { address: str(endpoint.url), copyable: true, note: '' };
  if (!httpBaseUrl) return { address: str(endpoint.path), copyable: false, note: 'Link this environment to T3 Connect for a public URL.' };
  const url = new URL(str(endpoint.path), httpBaseUrl), loopback = ['localhost', '127.0.0.1', '[::1]', '::1'].includes(url.hostname);
  return { address: url.href, copyable: true, note: loopback ? 'Only this computer can call this address. Link T3 Connect for a public URL.' : "Works wherever this environment's address is reachable, for example over Tailscale or your own proxy. Link T3 Connect for a public URL." };
}
export const DELIVERY_OUTCOMES: Record<string, string> = { accepted: 'Ran', dispatch_failed: 'Run failed', rejected_signature: 'Bad signature', disabled: 'Task paused', rate_limited: 'Rate limited', expired: 'Too old' };
