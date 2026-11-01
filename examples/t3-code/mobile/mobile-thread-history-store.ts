import { bridgeReply, ClientError, type Native } from './shared/protocol';
import { arr, obj, str, type Obj } from './shared/domain';

/**
 * The first residency unit stores fetched history ranges separately from the
 * whole-thread display snapshot. It deliberately has no client or UI owner:
 * callers must reconcile the returned revision and keep their existing route,
 * generation, and interaction guards.
 */
export interface MobileThreadHistoryItem extends Obj { key: string; ordinal: number; payload: string }
export interface MobileThreadHistoryAnchor extends Obj { key: string; ordinal: number }
export interface MobileThreadHistoryRange {
  revision: number; beforeCursor: string | null; afterCursor: string | null; hasMore: boolean;
  items: MobileThreadHistoryItem[];
}
export interface MobileThreadHistoryIdentity { environmentId: string; threadId: string }

const maxItems = 128, maxKeyUnits = 512, maxPayloadBytes = 256 * 1_024, maxCommitBytes = 256 * 1_024;
const invalid = () => new ClientError('The local history store returned an invalid reply.', 'Cache');
const identity = (value: MobileThreadHistoryIdentity) => {
  if (!value.environmentId || value.environmentId.trim() !== value.environmentId || value.environmentId.length > 512
    || value.environmentId.includes('\0') || !value.threadId || value.threadId.trim() !== value.threadId
    || value.threadId.length > 8_192 || value.threadId.includes('\0')) throw invalid();
  return value;
};
const cursor = (value: unknown, field: string): string | null => {
  if (value === null) return null;
  if (typeof value === 'string' && value.length > 0 && value.length <= 8_192 && value.trim() === value && !value.includes('\0')) return value;
  throw new ClientError(`Invalid local history ${field}.`, 'Cache');
};
const anchor = (value: unknown): MobileThreadHistoryAnchor => {
  const row = obj(value), key = str(row.key), ordinal = row.ordinal;
  if (!key || key.length > maxKeyUnits || key.trim() !== key || key.includes('\0') || !Number.isSafeInteger(ordinal) || Number(ordinal) < 0) throw invalid();
  return { key, ordinal: Number(ordinal) };
};
const item = (value: unknown): MobileThreadHistoryItem => {
  const row = obj(value), key = str(row.key), ordinal = row.ordinal, payload = str(row.payload);
  if (!key || key.length > maxKeyUnits || key.trim() !== key || key.includes('\0') || !Number.isSafeInteger(ordinal) || Number(ordinal) < 0
    || !payload || new TextEncoder().encode(payload).length > maxPayloadBytes) throw invalid();
  try {
    const decoded = JSON.parse(payload);
    if (decoded === null || typeof decoded !== 'object' || Array.isArray(decoded)) throw invalid();
  } catch { throw invalid(); }
  return { key, ordinal: Number(ordinal), payload };
};
const items = (value: unknown): MobileThreadHistoryItem[] => {
  if (!Array.isArray(value) || value.length > maxItems) throw invalid();
  const result = value.map(item), keys = new Set<string>();
  let previous: MobileThreadHistoryItem | null = null;
  for (const row of result) {
    if (keys.has(row.key) || previous && (row.ordinal < previous.ordinal || row.ordinal === previous.ordinal && row.key <= previous.key)) throw invalid();
    keys.add(row.key); previous = row;
  }
  return result;
};
async function call(native: Native, request: Obj): Promise<Obj> {
  const reply = await bridgeReply(native, { op: 'mobileClientCache', ...request });
  if (!reply.ok) throw new ClientError(reply.error!.message, reply.error!.kind);
  return obj(reply.value);
}
function bytes(value: unknown): number { return new TextEncoder().encode(JSON.stringify(value)).length; }

export async function mobileThreadHistoryTicket(native: Native, identityValue: MobileThreadHistoryIdentity): Promise<string> {
  const value = await call(native, { action: 'historyTicket', ...identity(identityValue) });
  const token = str(value.token);
  if (!token || token.length > 256) throw invalid();
  return token;
}

export async function mobileThreadHistoryCommit(native: Native, identityValue: MobileThreadHistoryIdentity,
  token: string, revision: number, beforeCursor: string | null, afterCursor: string | null,
  hasMore: boolean, rangeItems: MobileThreadHistoryItem[]): Promise<boolean> {
  identity(identityValue);
  if (!token || token.length > 256 || !Number.isSafeInteger(revision) || revision < 0 || typeof hasMore !== 'boolean') throw invalid();
  const normalized = items(rangeItems);
  if (bytes({ ...identityValue, token, revision, beforeCursor, afterCursor, hasMore, items: normalized }) > maxCommitBytes
    || (beforeCursor !== null && cursor(beforeCursor, 'before cursor') === null)
    || (afterCursor !== null && cursor(afterCursor, 'after cursor') === null)) throw invalid();
  const value = await call(native, { action: 'historyCommit', ...identityValue, token, revision,
    beforeCursor, afterCursor, hasMore, items: normalized });
  if (typeof value.committed !== 'boolean' || typeof value.stale !== 'boolean' || value.committed === value.stale) throw invalid();
  return value.committed;
}

export async function mobileThreadHistoryRead(native: Native, identityValue: MobileThreadHistoryIdentity,
  direction: 'older' | 'newer', limit = maxItems, anchorValue: MobileThreadHistoryAnchor | null = null): Promise<MobileThreadHistoryRange> {
  identity(identityValue);
  if (!['older', 'newer'].includes(direction) || !Number.isSafeInteger(limit) || limit < 1 || limit > maxItems) throw invalid();
  const value = await call(native, { action: 'historyRead', ...identityValue, direction, limit,
    ...(anchorValue ? { anchor: anchor(anchorValue) } : {}) });
  const revision = value.revision, hasMore = value.hasMore;
  if (!Number.isSafeInteger(revision) || Number(revision) < 0 || typeof hasMore !== 'boolean' || !Array.isArray(value.items)
    || value.items.length > limit) throw invalid();
  return { revision: Number(revision), beforeCursor: cursor(value.beforeCursor, 'before cursor'),
    afterCursor: cursor(value.afterCursor, 'after cursor'), hasMore, items: items(value.items) };
}

export async function mobileThreadHistoryClear(native: Native, identityValue: MobileThreadHistoryIdentity): Promise<number> {
  const value = await call(native, { action: 'historyClear', ...identity(identityValue) });
  if (!Number.isSafeInteger(value.removed) || Number(value.removed) < 0) throw invalid();
  return Number(value.removed);
}

export const mobileThreadHistoryLimits = { maxItems, maxKeyUnits, maxPayloadBytes, maxCommitBytes } as const;
