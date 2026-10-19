// WebhookDeliveriesDialog (MIT T3 Code 9c161462c9, LICENSE-T3):
// list recent requests, then read one delivery's redacted request and rendered prompt.
import type { T3Client } from '../client/client';
import type { LiveEnvironment } from '../client/live-streams';
import { arr, obj, str, type Obj } from '../client/domain';
import { DELIVERY_OUTCOMES } from './scheduled-webhook';
import { relativeLabel } from './scheduled-tasks';
export const EMPTY_DELIVERIES = { open: false, title: '', selected: '', error: '', empty: false, items: [] as { id: string; label: string; method: string; age: string; missing: string }[], blocks: [] as { title: string; value: string }[], outcome: '', method: '', age: '', verified: false, missing: '' };
const visits = new WeakMap<T3Client, { key: string; list: Obj[]; details: Map<string, Obj> }>();
export const closeDeliveries = (client: T3Client) => visits.delete(client);
export async function deliveriesPage(client: T3Client, environment: LiveEnvironment, task: Obj, selected: string, refresh: number, now: number) {
  const base = { ...EMPTY_DELIVERIES, open: true, title: `Deliveries · ${str(task.title)}`, selected };
  try {
    const key = `${environment.environmentId}|${environment.key}|${str(task.id)}|${refresh}`;
    let visit = visits.get(client);
    if (!visit || visit.key !== key) {
      visit = { key, list: arr((await environment.request('scheduledTasks.listWebhookDeliveries', { id: str(task.id) })).deliveries), details: new Map() };
      visits.set(client, visit);
    }
    if (!selected) return { ...base, empty: visit.list.length === 0, items: visit.list.map(item => ({ id: str(item.id), label: DELIVERY_OUTCOMES[str(item.outcome)] ?? str(item.outcome), method: str(item.method), age: relativeLabel(item.receivedAt, now), missing: Array.isArray(item.missingFields) && item.missingFields.length ? `${item.missingFields.length} empty placeholder${item.missingFields.length === 1 ? '' : 's'}` : '' })) };
    let detail = visit.details.get(selected);
    if (!detail) { detail = obj((await environment.request('scheduledTasks.getWebhookDelivery', { id: str(task.id), deliveryId: selected })).delivery); visit.details.set(selected, detail); }
    return { ...base, error: str(detail.error), outcome: DELIVERY_OUTCOMES[str(detail.outcome)] ?? str(detail.outcome), method: str(detail.method), age: relativeLabel(detail.receivedAt, now), verified: detail.signatureVerified === true,
      missing: Array.isArray(detail.missingFields) && detail.missingFields.length ? `Empty placeholders: ${detail.missingFields.map(String).join(', ')}` : '',
      blocks: [{ title: 'Prompt sent to the agent', value: str(detail.renderedPrompt, 'No run was started for this request.') },
        { title: 'Headers', value: Object.entries(obj(detail.headers)).map(([name, value]) => `${name}: ${String(value)}`).join('\n') },
        ...(str(detail.query) ? [{ title: 'Query', value: str(detail.query) }] : []), { title: detail.bodyTruncated === true ? 'Body (truncated)' : 'Body', value: str(detail.body) || '(empty)' }] };
  } catch (error) { return { ...base, error: error instanceof Error ? error.message : 'Could not load deliveries.' }; }
}
