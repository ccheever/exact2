// REST requests as data, and the rate limiter and history pager they pass through (llp/0001 §2).
// The caller does the `fetch` and reports each response to `onResponse`; time is an argument.
export { makeNonce, type NonceState } from './ids.ts';

export interface RestRequest {
  method: 'GET' | 'POST' | 'PATCH' | 'PUT' | 'DELETE';
  url: string;
  headers: Record<string, string>;
  body?: string;
  route: string;
}
export interface RestContext { origin: string; headers: Record<string, string> }
export interface HistoryQuery { before?: string; after?: string; around?: string; limit?: number }
// `mention` false sends the reply without pinging its original's author, as Discord's @ OFF does.
export interface ReplyTo { messageId: string; channelId?: string; guildId?: string; mention?: boolean }
/** A file uploaded for a message: the slot's id and the name Discord's upload gave it. */
export interface Uploaded { id: string; filename: string; uploaded_filename: string }
export interface SendMessage { content: string; nonce: string; replyTo?: ReplyTo; attachments?: Uploaded[] }
/** A file a message will carry, as the upload slots ask for it. */
export interface UploadFile { id: string; filename: string; file_size: number }

function pathOnly(path: string): string {
  return path.replace(/^https?:\/\/[^/]+/, '').split('?')[0].replace(/^\/api\/v\d+/, '');
}

function major(path: string): string {
  const parts = pathOnly(path).split('/');
  if (parts[1] === 'webhooks') return parts.slice(1, 4).join('/');
  return ['channels', 'guilds'].includes(parts[1]) ? parts.slice(1, 3).join('/') : '';
}

/** Keep Discord's major parameters; message ids and reaction emoji do not identify a bucket. */
export function routeKey(method: string, path: string): string {
  const parts = pathOnly(path).split('/');
  const reaction = parts.indexOf('reactions');
  return `${method.toUpperCase()} ${parts.map((part, index) => {
    if (index === 2 && ['channels', 'guilds', 'webhooks'].includes(parts[1])) return part;
    if (index === 3 && parts[1] === 'webhooks') return part;
    if (reaction >= 0 && index === reaction + 1) return ':emoji';
    if (reaction >= 0 && index === reaction + 2) return ':user';
    return /^\d+$/.test(part) ? ':id' : part;
  }).join('/')}`;
}

function request(ctx: RestContext, method: RestRequest['method'], path: string, body?: unknown): RestRequest {
  const headers = { ...ctx.headers };
  if (body !== undefined && !Object.keys(headers).some(key => key.toLowerCase() === 'content-type')) {
    headers['Content-Type'] = 'application/json';
  }
  return {
    method, url: `${ctx.origin.replace(/\/$/, '')}/api/v9${path}`, headers,
    route: routeKey(method, path), ...(body === undefined ? {} : { body: JSON.stringify(body) }),
  };
}

function pageQuery(query: HistoryQuery, limit = query.limit ?? 50): string {
  if (!Number.isInteger(limit) || limit < 1 || limit > 50) throw new RangeError('History limit must be 1–50');
  const cursors = (['before', 'after', 'around'] as const).filter(key => query[key] !== undefined);
  if (cursors.length > 1) throw new Error('History takes only one cursor');
  return `limit=${limit}${cursors.map(key => `&${key}=${encodeURIComponent(query[key]!)}`).join('')}`;
}

/** Build one history page, bounded to the account's 50-message request budget. */
export function history(ctx: RestContext, channelId: string, query: HistoryQuery = {}, limit = query.limit ?? 50): RestRequest {
  return request(ctx, 'GET', `/channels/${channelId}/messages?${pageQuery(query, limit)}`);
}

/** Build a nonce-enforced send, with the referenced channel defaulting to the destination. */
export function sendMessage(ctx: RestContext, channelId: string, message: SendMessage): RestRequest {
  const reply = message.replyTo;
  return request(ctx, 'POST', `/channels/${channelId}/messages`, {
    content: message.content, nonce: message.nonce, tts: false, flags: 0, enforce_nonce: true,
    ...(message.attachments?.length ? { attachments: message.attachments } : {}),
    ...(reply ? { message_reference: {
      ...(reply.guildId ? { guild_id: reply.guildId } : {}),
      channel_id: reply.channelId ?? channelId, message_id: reply.messageId,
    } } : {}),
    ...(reply?.mention === false ? { allowed_mentions: { parse: ['users', 'roles', 'everyone'], replied_user: false } } : {}),
  });
}

/** Build the request for upload slots: one URL per file, which the file is PUT to before the message
 * names it (as Discord's clients upload). */
export function uploadSlots(ctx: RestContext, channelId: string, files: UploadFile[]): RestRequest {
  return request(ctx, 'POST', `/channels/${channelId}/attachments`,
    { files: files.map(f => ({ id: f.id, filename: f.filename, file_size: f.file_size, is_clip: false })) });
}

/** Build a content edit without replacing embeds or attachments. */
export function editMessage(ctx: RestContext, channelId: string, messageId: string, content: string): RestRequest {
  return request(ctx, 'PATCH', `/channels/${channelId}/messages/${messageId}`, { content });
}

/** Build deletion of one message. */
export function deleteMessage(ctx: RestContext, channelId: string, messageId: string): RestRequest {
  return request(ctx, 'DELETE', `/channels/${channelId}/messages/${messageId}`);
}

/** Build the current user's reaction using a Unicode name or a custom name:id. */
export function addReaction(ctx: RestContext, channelId: string, messageId: string, emoji: string): RestRequest {
  return request(ctx, 'PUT', `/channels/${channelId}/messages/${messageId}/reactions/${encodeURIComponent(emoji)}/@me`);
}

/** Build removal of the current user's reaction. */
export function removeReaction(ctx: RestContext, channelId: string, messageId: string, emoji: string): RestRequest {
  return request(ctx, 'DELETE', `/channels/${channelId}/messages/${messageId}/reactions/${encodeURIComponent(emoji)}/@me`);
}

/** Build the current user's vote on a poll: the answers chosen, none to take the vote back. */
export function votePoll(ctx: RestContext, channelId: string, messageId: string, answerIds: string[]): RestRequest {
  return request(ctx, 'PUT', `/channels/${channelId}/polls/${messageId}/answers/@me`, { answer_ids: answerIds });
}

/** Build an explicit read acknowledgement; lastViewed is Discord's day counter, not milliseconds. */
export function ack(ctx: RestContext, channelId: string, messageId: string, opts: { token: string | null; lastViewed: number }): RestRequest {
  return request(ctx, 'POST', `/channels/${channelId}/messages/${messageId}/ack`, {
    token: opts.token, last_viewed: opts.lastViewed,
  });
}

/** Build a typing notification for a person currently composing. */
export function typing(ctx: RestContext, channelId: string): RestRequest {
  return request(ctx, 'POST', `/channels/${channelId}/typing`);
}

/** Build the gateway discovery request. */
export function gateway(ctx: RestContext): RestRequest { return request(ctx, 'GET', '/gateway'); }

/** Build the current-user request. */
export function me(ctx: RestContext): RestRequest { return request(ctx, 'GET', '/users/@me'); }

interface Bucket {
  limit: number | null;
  remaining: number | null;
  resetAt: number;
  blockedUntil: number;
  inFlight: number;
  parent?: Bucket;
  scope?: string;
}
export interface Limiter {
  globalUntil: number;
  routes: Map<string, Bucket>;
  buckets: Map<string, Bucket>;
  flights: Map<RestRequest, Bucket[]>;
}

/** Create an isolated limiter for one authenticated account. */
export function limiterInit(): Limiter {
  return { globalUntil: 0, routes: new Map(), buckets: new Map(), flights: new Map() };
}

function root(bucket: Bucket): Bucket {
  if (bucket.parent) bucket.parent = root(bucket.parent);
  return bucket.parent ?? bucket;
}

function bucketFor(l: Limiter, req: RestRequest): Bucket {
  let bucket = l.routes.get(req.route);
  if (!bucket) {
    bucket = { limit: null, remaining: null, resetAt: 0, blockedUntil: 0, inFlight: 0 };
    l.routes.set(req.route, bucket);
  }
  return root(bucket);
}

/** Reserve a request; an infinite wait means a response must release an unknown bucket first. */
export function admit(l: Limiter, req: RestRequest, nowMs: number): { ok: true } | { ok: false; waitMs: number } {
  const bucket = bucketFor(l, req);
  const blocked = Math.max(l.globalUntil, bucket.blockedUntil);
  if (nowMs < blocked) return { ok: false, waitMs: blocked - nowMs };
  if (bucket.resetAt > 0 && nowMs >= bucket.resetAt) {
    bucket.remaining = bucket.limit === null ? null : Math.max(0, bucket.limit - bucket.inFlight);
    bucket.resetAt = 0;
  }
  if (bucket.remaining === null && bucket.inFlight > 0) return { ok: false, waitMs: Infinity };
  if (bucket.remaining !== null && bucket.remaining <= 0) {
    return { ok: false, waitMs: bucket.resetAt > nowMs ? bucket.resetAt - nowMs : Infinity };
  }
  if (bucket.remaining !== null) bucket.remaining--;
  bucket.inFlight++;
  const flights = l.flights.get(req) ?? [];
  flights.push(bucket);
  l.flights.set(req, flights);
  return { ok: true };
}

function numeric(value: string | null): number | null {
  if (value === null || value.trim() === '') return null;
  const n = Number(value);
  return Number.isFinite(n) && n >= 0 ? n : null;
}

/** Release a reservation and apply Discord's response limits; status 0 also releases a failed fetch. */
export function onResponse(
  l: Limiter, req: RestRequest, status: number, headers: (name: string) => string | null,
  body: unknown, nowMs: number,
): { retryAfterMs?: number; global?: boolean } {
  let bucket = bucketFor(l, req);
  const hash = headers('X-RateLimit-Bucket');
  if (hash) {
    // A bucket hash excludes major parameters: the same hash in two channels is two budgets.
    const key = `${hash}:${major(req.url)}`;
    const shared = l.buckets.get(key);
    if (shared && root(shared) !== bucket) {
      const target = root(shared);
      target.inFlight += bucket.inFlight;
      target.blockedUntil = Math.max(target.blockedUntil, bucket.blockedUntil);
      target.resetAt = Math.max(target.resetAt, bucket.resetAt);
      if (bucket.remaining !== null) target.remaining = Math.min(target.remaining ?? Infinity, bucket.remaining);
      bucket.parent = target;
      bucket = target;
    }
    l.buckets.set(key, bucket);
    l.routes.set(req.route, bucket);
  }
  const flights = l.flights.get(req);
  const flight = flights?.shift();
  if (flight) root(flight).inFlight = Math.max(0, root(flight).inFlight - 1);
  if (!flights?.length) l.flights.delete(req);
  const limit = numeric(headers('X-RateLimit-Limit'));
  const remaining = numeric(headers('X-RateLimit-Remaining'));
  const after = numeric(headers('X-RateLimit-Reset-After'));
  const reset = numeric(headers('X-RateLimit-Reset'));
  const resetAt = after !== null ? nowMs + after * 1000 : reset !== null ? reset * 1000 : null;
  const fresh = bucket.resetAt === 0 || bucket.resetAt <= nowMs;
  if (limit !== null) bucket.limit = limit;
  if (remaining !== null) {
    const available = Math.max(0, remaining - bucket.inFlight);
    bucket.remaining = fresh ? available : Math.min(bucket.remaining ?? Infinity, available);
  }
  if (resetAt !== null) bucket.resetAt = Math.max(bucket.resetAt, resetAt);
  bucket.scope = headers('X-RateLimit-Scope') ?? bucket.scope;
  if (status !== 429) return {};
  const data = body && typeof body === 'object' ? body as Record<string, unknown> : {};
  const seconds = typeof data.retry_after === 'number' && Number.isFinite(data.retry_after) && data.retry_after >= 0
    ? data.retry_after : numeric(headers('Retry-After')) ?? after ?? 1;
  const retryAfterMs = seconds * 1000;
  const global = data.global === true || headers('X-RateLimit-Global') === 'true' || bucket.scope === 'global';
  bucket.blockedUntil = Math.max(bucket.blockedUntil, nowMs + retryAfterMs);
  bucket.resetAt = Math.max(bucket.resetAt, bucket.blockedUntil);
  bucket.remaining = 0;
  if (global) l.globalUntil = Math.max(l.globalUntil, bucket.blockedUntil);
  return { retryAfterMs, global };
}

export interface Pager { inFlight: Map<string, string> }
export type PageResult = { pager: Pager; request: RestRequest } | {
  pager: Pager; request: null; reason: 'duplicate-query' | 'channel-busy';
};

/** Create a pager whose channels may load independently. */
export function pagerInit(): Pager { return { inFlight: new Map() }; }

/** Reserve one channel page; repeat queries are refused until pageDone releases the reservation. */
export function pageRequest(pager: Pager, ctx: RestContext, channelId: string, query: HistoryQuery = {}): PageResult {
  const key = pageQuery(query);
  const active = pager.inFlight.get(channelId);
  if (active) return { pager, request: null, reason: active === key ? 'duplicate-query' : 'channel-busy' };
  const req = history(ctx, channelId, query);
  pager.inFlight.set(channelId, key);
  return { pager, request: req };
}

/** Release a completed or failed page; supplying its query protects against a stale completion. */
export function pageDone(pager: Pager, channelId: string, query?: HistoryQuery): Pager {
  if (!query || pager.inFlight.get(channelId) === pageQuery(query)) pager.inFlight.delete(channelId);
  return pager;
}
