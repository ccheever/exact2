// MIT T3 Code 74541bab17 (LICENSE-T3): scheduledTasksSettings.logic.ts,
// scheduledTaskWebhook.ts, webhookAddress.ts and the webhook schedule checks of
// packages/contracts/src/scheduledTask.ts. Secrets are write-only.
import { arr, obj, str, type Obj } from '../client/domain';
export const DEFAULT_WEBHOOK_PROMPT = 'Handle this webhook:\n{{body}}';
export const MAX_WEBHOOK_DELIVERY_AGE_MINUTES = 1440;
export const MAX_WEBHOOK_FILTERS = 16;
export function parseMaxDeliveryAge(value: string): number | null | undefined {
  if (value.trim() === '') return null;
  const minutes = Number(value.trim());
  return Number.isInteger(minutes) && minutes > 0 && minutes <= MAX_WEBHOOK_DELIVERY_AGE_MINUTES ? minutes : undefined;
}

/**
 * One editable filter row. `in` takes a comma-separated list, which is lossless because list
 * values can't contain commas; `exists` ignores the value. `id` is the row's identity in the form.
 */
export type WebhookFilterDraft = { id: string; path: string; op: string; value: string };
export const WEBHOOK_FILTER_OPS = [
  { op: 'equals', label: 'equals' },
  { op: 'notEquals', label: 'does not equal' },
  { op: 'in', label: 'is one of' },
  { op: 'startsWith', label: 'starts with' },
  { op: 'exists', label: 'exists' },
];
const filterOp = (op: unknown) => WEBHOOK_FILTER_OPS.some(option => option.op === op) ? String(op) : 'equals';

export function webhookFilterDrafts(filters: unknown): WebhookFilterDraft[] {
  return arr(filters).map((filter, index) => ({ id: `filter-${index}`, path: str(filter.path), op: filterOp(filter.op),
    value: filter.op === 'exists' ? '' : filter.op === 'in' ? (Array.isArray(filter.value) ? filter.value.map(String).join(', ') : '') : str(filter.value) }));
}

/**
 * The editor's rows as its save sends them (settings-scheduled.contract `encodeFilters`): rows
 * joined by ",", each `path:op:value` with path and value URI-encoded, so neither separator
 * appears inside a field.
 */
export function decodeWebhookFilterRows(text: string): WebhookFilterDraft[] {
  if (!text) return [];
  return text.split(',').map((row, index) => {
    const [path = '', op = '', value = ''] = row.split(':');
    return { id: `filter-${index}`, path: decodeURIComponent(path), op: filterOp(op), value: decodeURIComponent(value) };
  });
}

const FILTER_PATH = /^(body|headers|query)(\.|$)/;
const FILTER_LIST_VALUE = /^[^,\s](?:[^,]*[^,\s])?$/;
/** ScheduledTaskWebhookFilter's decode: the server's checks, so a row that passes here also saves. */
function decodeFilter(path: string, op: string, value: string): Obj | null {
  if (!path.trim() || !FILTER_PATH.test(path)) return null;
  if (op === 'exists') return { path, op };
  if (op !== 'in') return { path, op, value };
  const values = value.split(',').map(item => item.trim()).filter(item => item !== '');
  return values.length > 0 && values.every(item => FILTER_LIST_VALUE.test(item)) ? { path, op, value: values } : null;
}

/** The filters the server would accept, or the first row it would refuse. */
export function parseWebhookFilterDrafts(drafts: WebhookFilterDraft[]): { filters: Obj[] } | { error: string } {
  if (drafts.length > MAX_WEBHOOK_FILTERS) return { error: `Use at most ${MAX_WEBHOOK_FILTERS} filters.` };
  const filters: Obj[] = [];
  for (let index = 0; index < drafts.length; index++) {
    const draft = drafts[index]!;
    const filter = decodeFilter(draft.path.trim(), draft.op, draft.value);
    if (!filter) return { error: `Filter ${index + 1} needs a path under body, headers or query${draft.op === 'exists' ? '.' : draft.op === 'in' ? ', and at least one value.' : ', and a value.'}` };
    filters.push(filter);
  }
  return { filters };
}

/**
 * Whether the rows differ from the ones the form opened with. A save leaves `filters` out unless
 * they did, so editing only the title can't overwrite filters another device changed meanwhile.
 */
export function webhookFilterDraftsChanged(saved: WebhookFilterDraft[], current: WebhookFilterDraft[]): boolean {
  return saved.length !== current.length || saved.some((filter, index) => filter.path !== current[index]?.path || filter.op !== current[index]?.op
    || (filter.op !== 'exists' && filter.value !== current[index]?.value));
}

const DEDUPE_PLACEHOLDERS = /\{\{\s*(?:(?:body|headers|query)(?:\.[^{}.\s]+)*|request)\s*\}\}/g;
/** A replay or coalesce key: at least one request placeholder and no stray braces. */
export function isWebhookDedupeTemplate(template: string): boolean {
  const matches = template.match(DEDUPE_PLACEHOLDERS) ?? [];
  const remainder = template.replace(DEDUPE_PLACEHOLDERS, '');
  return matches.length > 0 && !remainder.includes('{{') && !remainder.includes('}}');
}

/** The age limit's error alone, else one error per invalid dedupe field. */
export function webhookDraftErrors(input: Record<string, string>): { title: string; description: string }[] {
  if (parseMaxDeliveryAge(str(input.maxDeliveryAgeMinutes)) === undefined) return [{ title: 'Invalid age limit', description: `Enter whole minutes from 1 to ${MAX_WEBHOOK_DELIVERY_AGE_MINUTES}, or leave it blank.` }];
  const errors: { title: string; description: string }[] = [];
  const keys: [string, string][] = [['Replay key', str(input.replayKey)], ['Coalesce key', str(input.coalesceKey)]];
  for (const [label, value] of keys) {
    if (value.trim() && !isWebhookDedupeTemplate(value.trim())) errors.push({ title: 'Invalid webhook settings', description: `${label} must contain a valid request placeholder.` });
  }
  const window = str(input.coalesceWindowSeconds).trim();
  if (window && (!Number.isInteger(Number(window)) || Number(window) < 10 || Number(window) > 3600)) errors.push({ title: 'Invalid webhook settings', description: 'Coalesce window must be whole seconds from 10 to 3600.' });
  return errors;
}

export function webhookDraft(task?: Obj) {
  const schedule = obj(task?.schedule), signature = obj(schedule.signature), webhook = schedule.type === 'webhook', dedupe = webhook ? obj(schedule.dedupe) : {};
  return { signatureEnabled: webhook && schedule.signature != null,
    signatureHeader: str(signature.header, 'x-hub-signature-256'), signatureEncoding: str(signature.encoding, 'hex'),
    signaturePrefix: str(signature.prefix, 'sha256='), signatureSecret: '',
    maxDeliveryAgeMinutes: webhook && schedule.maxDeliveryAgeMinutes != null ? String(schedule.maxDeliveryAgeMinutes) : '',
    replayKey: str(dedupe.replayKey), coalesceKey: str(dedupe.coalesceKey),
    coalesceWindowSeconds: dedupe.coalesceWindowSeconds !== undefined ? String(dedupe.coalesceWindowSeconds) : '',
    filters: webhookFilterDrafts(webhook ? schedule.filters : undefined),
    hasSecret: obj(task?.webhook).hasSecret === true };
}

/** submit() and scheduleFromDraft for a webhook trigger: `filters` and `savedFilters` are the editor's encoded rows. */
export function webhookSchedule(input: Record<string, string>, existing?: Obj): Obj {
  const rows = decodeWebhookFilterRows(str(input.filters));
  const parsed = parseWebhookFilterDrafts(rows);
  if ('error' in parsed) throw new Error(`Invalid filter: ${parsed.error}`);
  const errors = webhookDraftErrors(input);
  // One `Title: description` line per error; settingsFailure (shell-commands.ts) toasts each line.
  if (errors.length > 0) throw new Error(errors.map(error => `${error.title}: ${error.description}`).join('\n'));
  const maxDeliveryAgeMinutes = parseMaxDeliveryAge(str(input.maxDeliveryAgeMinutes));
  const secret = str(input.signatureSecret).trim(), header = str(input.signatureHeader).trim();
  const enabled = input.signatureEnabled === 'true';
  if (enabled && (!header || (!secret && !(obj(existing?.schedule).type === 'webhook' && obj(existing?.webhook).hasSecret === true)))) throw new Error('Signing secret is required: Enter the signature header and secret.');
  const replayKey = str(input.replayKey).trim(), coalesceKey = str(input.coalesceKey).trim(), window = str(input.coalesceWindowSeconds).trim();
  return { type: 'webhook', maxDeliveryAgeMinutes, signature: enabled ? { header, encoding: input.signatureEncoding === 'base64' ? 'base64' : 'hex', prefix: str(input.signaturePrefix), ...(secret ? { secret } : {}) } : null,
    // An empty object clears the stored dedupe settings.
    dedupe: { ...(replayKey ? { replayKey } : {}), ...(coalesceKey ? { coalesceKey } : {}), ...(window ? { coalesceWindowSeconds: Number(window) } : {}) },
    // Only edited filters are sent; the server keeps the stored ones otherwise.
    ...(webhookFilterDraftsChanged(decodeWebhookFilterRows(str(input.savedFilters)), rows) ? { filters: parsed.filters } : {}) };
}
export function webhookAddress(endpoint: Obj | undefined, httpBaseUrl: string) {
  if (!endpoint) return { address: '', copyable: false, note: 'The URL appears after you save.' };
  if (str(endpoint.url)) return { address: str(endpoint.url), copyable: true, note: '' };
  if (!httpBaseUrl) return { address: str(endpoint.path), copyable: false, note: 'Link this environment to T3 Connect for a public URL.' };
  const url = new URL(str(endpoint.path), httpBaseUrl), loopback = ['localhost', '127.0.0.1', '[::1]', '::1'].includes(url.hostname);
  return { address: url.href, copyable: true, note: loopback ? 'Only this computer can call this address. Link T3 Connect for a public URL.' : "Works wherever this environment's address is reachable, for example over Tailscale or your own proxy. Link T3 Connect for a public URL." };
}

const DELIVERY_OUTCOMES: Record<string, string> = { dispatch_failed: 'Run failed', duplicate: 'Duplicate', coalesced: 'Coalesced', filtered: 'Filtered',
  rejected_signature: 'Bad signature', disabled: 'Task paused', rate_limited: 'Rate limited', expired: 'Too old' };
/** A newer server can report an outcome this client has no label for. */
export function webhookDeliveryLabel(delivery: Obj): string {
  const outcome = str(delivery.outcome);
  if (outcome === 'accepted') return delivery.pendingWindow === true ? 'Opened window' : 'Ran';
  return Object.prototype.hasOwnProperty.call(DELIVERY_OUTCOMES, outcome) ? DELIVERY_OUTCOMES[outcome]! : outcome.replaceAll('_', ' ');
}
/** deliveryOutcomeVariant: an open coalescing window reads as coalesced. */
export function webhookDeliveryTone(delivery: Obj): string {
  const outcome = delivery.pendingWindow === true ? 'coalesced' : str(delivery.outcome);
  if (outcome === 'accepted') return 'success';
  if (['duplicate', 'coalesced', 'disabled', 'rate_limited', 'expired'].includes(outcome)) return 'warning';
  if (outcome === 'dispatch_failed' || outcome === 'rejected_signature') return 'error';
  return 'outline';
}
