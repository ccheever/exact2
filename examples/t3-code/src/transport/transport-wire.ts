// The Effect RPC wire T3's server speaks over its socket, and the HTTP pieces every
// connection shares (T3 Code 7c3b3b3605ef, MIT, see LICENSE-T3: packages/client-runtime/src/rpc/
// protocol.ts and session.ts frame JSON with effect's RpcSerialization.layerJson;
// connection/compatibility.ts, supervisor.ts retryDelayMs, rpc/client.ts's expected-failure
// retry; authorization/remote.ts bootstrapRemoteBearerSession; connection/errors.ts
// mapRemoteEnvironmentError). Effect itself is not imported: its runtime reads the clock, which a data module refuses.

export type Json = null | boolean | number | string | Json[] | { [key: string]: Json };
export type Frame = Record<string, unknown>;

/**
 * exact2's rejection of a fetch whose answer it let go (a resource asked again, a newer send): passed on as it is,
 * as the native bridge's call would have been rejected, so the client reads it as superseded (client/let-go.ts).
 */
export function letGoAbort(error: unknown): boolean {
  const fields = error !== null && typeof error === 'object' ? error as { name?: unknown; kind?: unknown } : {};
  return fields.name === 'FetchError' && fields.kind === 'Aborted';
}

/** A failure as the native bridge reported it (`{kind, message, uncertain, reason, detail, context}`). */
export class TransportFailure extends Error {
  /**
   * The request never reached a live session, or its session ended before the reply (EnvironmentRpcUnavailableError's
   * place here). A read that fails so is asked again once the connection is back, so it shows no failure.
   */
  unavailable = false;
  /** A refused HTTP request's status; 0 for anything else. */
  status = 0;
  constructor(readonly kind: string, message: string, readonly uncertain = false, readonly reason = '', readonly detail = '', readonly context: Record<string, string> = {}) {
    super(message);
  }
  json() {
    return { kind: this.kind, message: this.message, uncertain: this.uncertain, reason: this.reason, detail: this.detail,
      ...(Object.keys(this.context).length ? { context: this.context } : {}), ...(this.unavailable ? { unavailable: true } : {}) };
  }
  with(fields: { message?: string; uncertain?: boolean; unavailable?: boolean }) {
    const next = new TransportFailure(this.kind, fields.message ?? this.message, fields.uncertain ?? this.uncertain, this.reason, this.detail, this.context);
    next.unavailable = fields.unavailable ?? this.unavailable; next.status = this.status;
    return next;
  }
}
/** A request refused because its session is not connected. */
export const notConnected = () => new TransportFailure('Disconnected', 'The server is not connected.').with({ unavailable: true });

/**
 * failRemoteRequest (rpc/http.ts) for a request that got no answer: past its deadline, the timeout; otherwise
 * the transport error, as effect's HttpClientError prints it (a refused port, an unknown host).
 */
export function remoteRequestFailure(url: string, method: string, error: unknown, timeoutMs: number): string {
  const kind = error !== null && typeof error === 'object' ? (error as { kind?: unknown }).kind : undefined;
  if (kind === 'Timeout') return `Remote environment endpoint ${url} timed out after ${timeoutMs}ms.`;
  return `Failed to fetch remote environment endpoint ${url} (HttpClientError: Transport error (${method} ${url})).`;
}

/** A typed error's context fields a client reads (ProjectReadFileError's failure, operation and the paths it tried). */
const CONTEXT_FIELDS = ['failure', 'operation', 'cwd', 'relativePath', 'resolvedPath', 'operationPath'];

const isObject = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value);
const text = (value: unknown) => typeof value === 'string' ? value : '';

/** The largest frame either side sends (the clone's own bound). */
export const MAXIMUM_BYTES = 16 * 1024 * 1024;
export const PROTOCOL_VERSION = 2;

export function utf8Length(value: string): number {
  let bytes = 0;
  for (let index = 0; index < value.length; index++) {
    const code = value.charCodeAt(index);
    if (code < 0x80) bytes += 1;
    else if (code < 0x800) bytes += 2;
    else if (code >= 0xd800 && code <= 0xdbff && index + 1 < value.length) { bytes += 4; index++; }
    else bytes += 3;
  }
  return bytes;
}

export function encode(message: unknown): string {
  const encoded = JSON.stringify(message);
  if (utf8Length(encoded) > MAXIMUM_BYTES) throw new TransportFailure('Limit', 'The request is too large.');
  return encoded;
}

/** One socket message: a frame, or a batch of frames, each tagged. */
export function decode(data: string): Frame[] {
  if (utf8Length(data) > MAXIMUM_BYTES) throw new TransportFailure('Limit', 'The server frame is too large.');
  let parsed: unknown;
  try { parsed = JSON.parse(data); } catch { throw new TransportFailure('Protocol', 'The server sent an invalid RPC frame.'); }
  const frames = Array.isArray(parsed) ? parsed : [parsed];
  if (!frames.every(isObject)) throw new TransportFailure('Protocol', 'The server sent an invalid RPC frame.');
  if (!frames.every(frame => typeof frame._tag === 'string')) throw new TransportFailure('Protocol', 'The server sent an untagged RPC frame.');
  return frames as Frame[];
}

export function identifier(value: unknown): string | null {
  if (typeof value === 'string') return value;
  if (typeof value === 'number' && Number.isFinite(value)) return String(value);
  return null;
}

/**
 * Where a WebSocket ticket is minted (reference 7c3b3b3605ef, client-runtime authorization/remote.ts
 * `webSocketTicketPath`): `rpc` for an RPC socket, `media` for the device hub and the preview stream, which share one
 * ticket across many requests; none, the bare path every server has, whose tickets open either.
 */
export const ticketPath = (use?: 'rpc' | 'media') => use ? `/api/auth/websocket-ticket/${use}` : '/api/auth/websocket-ticket';

/**
 * issueWebSocketTicketWithFallback: an RPC ticket from a server that predates ticket routes (it answers 404) comes
 * from the bare path. A media ticket has no fallback, so such a server opens no device or preview stream.
 */
export async function mintTicket<T>(use: 'rpc' | 'media', issue: (path: string) => Promise<T>): Promise<T> {
  try { return await issue(ticketPath(use)); }
  catch (error) {
    if (use !== 'rpc' || !(error instanceof TransportFailure) || error.status !== 404) throw error;
    return issue(ticketPath());
  }
}

/** A request carrying the orchestration protocol header. */
export function request(id: string, method: string, payload: unknown): Frame {
  return { _tag: 'Request', id, tag: method, payload, headers: [['x-t3-orchestration-protocol', String(PROTOCOL_VERSION)]] };
}

/**
 * PullRequestUnavailableError's message, a getter rather than a wire field (reference 7c3b3b3605ef, contracts
 * pullRequest.ts): what the host needs on the server in its own words (`requirement`, which an older server does not
 * send), else a generic sentence for the reason.
 */
export function pullRequestUnavailable(reason: string, requirement = ''): string {
  if (reason === 'cli-missing') return requirement || 'The tool this host is read through is not installed or set up.';
  if (reason === 'cli-unauthenticated') return requirement || 'This host has no working credentials.';
  if (reason === 'provider-unsupported') return "Change requests cannot be browsed for this project's host yet.";
  return '';
}

/**
 * An Exit failure as the reference squashes it (Cause.squash): the first typed `Fail`
 * reason's error, else a `Die` defect, else an interrupt. A typed error keeps its own
 * `_tag` and message; its `cause` only fills a missing message.
 */
export function failure(value: unknown): TransportFailure {
  if (Array.isArray(value)) {
    const reasons = value.filter(isObject);
    const picked = reasons.find(reason => reason._tag === 'Fail') ?? reasons.find(reason => reason._tag === 'Die');
    if (picked) return failure(picked);
    if (value.length) return failure(value[0]);
    return new TransportFailure('RPC', 'The server request failed.');
  }
  if (!isObject(value)) return new TransportFailure('RPC', typeof value === 'string' && value ? value : 'The server request failed.');
  switch (value._tag) {
    case 'Exit': if ('exit' in value) return failure(value.exit); break;
    case 'Failure': if ('cause' in value) return failure(value.cause); break;
    case 'Fail': if ('error' in value) return typed(value.error); break;
    case 'Die': case 'Defect': if ('defect' in value) return failure(value.defect); break;
    case 'ClientProtocolError': if ('error' in value) return failure(value.error); break;
    case 'Interrupt': return new TransportFailure('Interrupt', 'The server interrupted the request.');
    case undefined:
      // Untagged envelopes: { cause }, { error }, { defect }, or a plain Error ({ name, message }).
      if (!('message' in value)) {
        if ('error' in value) return failure(value.error);
        if ('cause' in value) return failure(value.cause);
        if ('defect' in value) return failure(value.defect);
      }
      break;
  }
  return typed(value);
}

// packages/contracts/src/secretRequest.ts at 7c3b3b3605ef (MIT, LICENSE-T3).
const SECRET_REQUEST_FAILURE_MESSAGES: Record<string, string> = {
  load_failed: "Could not load the secret request.",
  not_found: "This secret request no longer exists.",
  already_answered: "This secret request was already answered.",
  agent_stopped: "The agent that asked has stopped, so this secret can't be used.",
  store_failed: "Could not store the secret.",
  record_failed: "Saved the secret, but could not update the request.",
  invalid_ref: "That secretRef is not valid.",
  read_failed: "Could not read the secret.",
  ref_unavailable:
    "That secretRef was already used or does not exist. Ask the user again with request_secret.",
  ref_expired: "That secretRef expired. Ask the user again with request_secret.",
  consume_failed: "Could not use that secretRef. Try again.",
  not_a_file_request:
    'The user did not save this secret for shell use. Ask again with request_secret and delivery: "file".',
};

/** GitCommandFailureReason (contracts git.ts): a reason this build does not know decodes as absent. */
const GIT_FAILURE_REASONS = ['authentication_failed', 'branch_already_exists', 'branch_checked_out_in_worktree', 'host_key_unverified', 'not_a_repository',
  'path_already_exists', 'push_rejected_by_remote', 'push_rejected_non_fast_forward', 'ref_not_found', 'remote_unreachable', 'tag_would_be_clobbered'];

/** AssetWorkspaceAssetNotFoundError's resource and, from a server that sends it, the host path it checked. */
function assetNotFoundContext(value: Record<string, unknown>): Record<string, string> {
  const resource = isObject(value.resource) ? value.resource : {};
  return Object.fromEntries(Object.entries({ resourceTag: text(resource._tag), resourcePath: text(resource.path), checkedPath: text(value.checkedPath) }).filter(([, field]) => field));
}

function typed(value: unknown): TransportFailure {
  if (!isObject(value)) return failure(value);
  const tag = text(value._tag) || text(value.name) || 'RPC';
  const field = (key: string) => text(value[key]).trim();
  if (tag === 'SecretRequestError') return new TransportFailure(tag, SECRET_REQUEST_FAILURE_MESSAGES[field('reason')] ?? 'Could not answer the request. Try again.', false, field('reason'));
  if (tag === 'GitCommandError') {
    // Its message is a getter, not a wire field.
    const reason = GIT_FAILURE_REASONS.includes(field('reason')) ? field('reason') : '';
    const message = field('message') || `Git command failed in ${field('operation')} (${field('cwd')}): ${field('detail')}${reason ? ` (${reason})` : ''}`;
    return new TransportFailure(tag, message, false, reason, field('detail'));
  }
  let message = field('message');
  // PullRequestOperationError's and PullRequestUnavailableError's messages are getters, not wire fields.
  if (!message && tag === 'PullRequestOperationError' && field('detail')) message = `Pull request operation ${field('operation')} failed: ${field('detail')}`;
  if (!message && tag === 'PullRequestUnavailableError') message = pullRequestUnavailable(field('reason'), field('requirement'));
  // AssetWorkspaceAssetNotFoundError's message is a getter too (contracts assets.ts).
  if (!message && tag === 'AssetWorkspaceAssetNotFoundError') {
    const kind = isObject(value.resource) ? text(value.resource._tag) : '';
    message = kind === 'media-file' ? 'Media file was not found.' : kind === 'tool-output-image' ? 'Tool output image was not found.' : 'Workspace asset was not found.';
  }
  // The tag predates its rename to AssetSourceControlMediaUrlValidationError, whose message is a getter (contracts assets.ts).
  if (!message && tag === 'AssetGitHubMediaUrlValidationError') message = 'Only media a source control host keeps behind a credential can be fetched with one.';
  // DeviceEasAccountError's message is its reason, through a getter (contracts device.ts).
  if (!message && tag === 'DeviceEasAccountError') message = field('reason');
  if (!message && field('detail')) message = field('detail');
  if (!message && 'cause' in value) message = failure(value.cause).message;
  if (!message || message === 'The server request failed.') message = `The server refused the request (${tag}).`;
  const context = tag === 'ProjectReadFileError' ? Object.fromEntries(CONTEXT_FIELDS.filter(key => text(value[key])).map(key => [key, text(value[key])]))
    : tag === 'AssetWorkspaceAssetNotFoundError' ? assetNotFoundContext(value) : {};
  return new TransportFailure(tag, message, false, field('reason'), field('detail'), context);
}

// MARK: Addresses

/** An http(s) origin, lowercased, with no path, query or credentials (packages/shared/src/remote.ts accepts both schemes). */
export function origin(input: string): string {
  let url: URL;
  try { url = new URL(input.trim()); } catch { throw new TransportFailure('Address', 'Enter an http:// or https:// server address.'); }
  const scheme = url.protocol.toLowerCase();
  if (!['http:', 'https:'].includes(scheme) || !url.hostname || url.username || url.password) {
    throw new TransportFailure('Address', 'Enter an http:// or https:// server address.');
  }
  return `${scheme}//${url.host.toLowerCase()}`;
}

/** A path on the connected origin; anything that would leave it is refused. */
export function path(target: string, at: string): string {
  if (!target.startsWith('/') || target.startsWith('//') || target.includes('#')) {
    throw new TransportFailure('Address', 'The request must stay on the connected server.');
  }
  let url: URL;
  try { url = new URL(target, at); } catch { throw new TransportFailure('Address', 'The request must stay on the connected server.'); }
  if (origin(url.href) !== at) throw new TransportFailure('Address', 'The request must stay on the connected server.');
  return url.href;
}

/** A pairing token: pasted as is, or read from a pairing link to this origin (`#token=` or `?token=`). */
export function credential(input: string, at: string): string {
  const value = input.trim();
  if (!value.includes('://')) return value;
  let url: URL;
  try { url = new URL(value); } catch { throw new TransportFailure('Credential', 'Use a pairing link from this server, or paste its pairing token.'); }
  let linked = '';
  try { linked = origin(value); } catch { /* refused below */ }
  if (linked !== at) throw new TransportFailure('Credential', 'Use a pairing link from this server, or paste its pairing token.');
  const token = new URLSearchParams(url.hash.replace(/^#/, '')).get('token') || url.searchParams.get('token') || '';
  if (!token) throw new TransportFailure('Credential', 'The pairing link has no token.');
  return token;
}

export const trimmed = (value: string) => value.trim().replace(/\/+$/, '');

/** application/x-www-form-urlencoded, keys sorted; a literal '+' stays a plus. */
export function form(values: Record<string, string>): string {
  return Object.keys(values).sort().map(key => `${encodeURIComponent(key)}=${encodeURIComponent(values[key]!)}`).join('&');
}

/** `clientMetadataTokenExchangeFields`: what this app sends for a paired environment. */
export function exchangeForm(credential: string, scope = ''): string {
  const fields: Record<string, string> = {
    grant_type: 'urn:ietf:params:oauth:grant-type:token-exchange',
    subject_token: credential,
    subject_token_type: 'urn:t3:params:oauth:token-type:environment-bootstrap',
    requested_token_type: 'urn:ietf:params:oauth:token-type:access_token',
    client_label: 'Exact T3 for Mac', client_device_type: 'desktop', client_os: 'macos',
  };
  if (scope) fields.scope = scope;
  return form(fields);
}

/** The `reason` of a refused HTTP request's body (`invalid_scope`, `invalid_credential`, …), or ''. */
export function failureReason(decoded: unknown): string {
  return isObject(decoded) && typeof decoded.reason === 'string' ? decoded.reason : '';
}

/**
 * A pairing's token exchange: the requested scopes, then `legacyScope` when the server answers
 * `invalid_scope`. A server older than the permission split refuses the new scope names before it
 * spends the one-time credential, so the second request may use it (remote-scopes.ts).
 */
export async function exchangeScopes<T>(scope: string, legacyScope: string, exchange: (scope: string) => Promise<T>): Promise<T> {
  try { return await exchange(scope); }
  catch (error) {
    if (!legacyScope || legacyScope === scope || !(error instanceof TransportFailure) || error.reason !== 'invalid_scope') throw error;
    return exchange(legacyScope);
  }
}

/**
 * A refused HTTP request's text: the server's own message, then mapRemoteEnvironmentError's detail for an invalid
 * credential or a scope the link does not grant, then a pull request error's own sentence, then the status.
 */
export function failureMessage(decoded: unknown, status: number): string {
  const body = isObject(decoded) ? decoded : {};
  if (typeof body.message === 'string') return body.message;
  const field = (key: string) => text(body[key]).trim();
  const reason = field('reason');
  if (reason === 'invalid_credential') return 'The environment credential is invalid.';
  if (reason === 'scope_not_granted' || reason === 'invalid_scope') return 'The environment rejected the authentication request.';
  if (field('_tag') === 'PullRequestUnavailableError') {
    const sentence = pullRequestUnavailable(reason, field('requirement'));
    if (sentence) return sentence;
  }
  if (field('_tag') === 'PullRequestOperationError' && field('detail')) return `Pull request operation ${field('operation')} failed: ${field('detail')}`;
  return `The server returned HTTP ${status}.`;
}

// MARK: Compatibility (connection/compatibility.ts)

export function protocolOf(descriptor: Record<string, unknown>): number {
  return typeof descriptor.orchestrationProtocolVersion === 'number' ? descriptor.orchestrationProtocolVersion : 1;
}

/** Whether this client can drive the host's update remotely. */
export function canSelfUpdate(descriptor: Record<string, unknown>): boolean {
  const capabilities = isObject(descriptor.capabilities) ? descriptor.capabilities : {};
  const method = text(capabilities.serverSelfUpdate);
  if (!method) return false;
  return method !== 'desktop-managed' || capabilities.desktopAppUpdate === true;
}

/** orchestrationProtocolCompatibilityError: null when compatible; the message names the direction. */
export function compatibility(descriptor: Record<string, unknown>, label = text(descriptor.label) || 'this server'): { message: string; serverUpdateRequired: boolean } | null {
  const version = protocolOf(descriptor);
  if (version === PROTOCOL_VERSION) return null;
  if (version > PROTOCOL_VERSION) {
    return { message: `This client is not supported by this server. Update your app or use a compatible release to connect to ${label}.`, serverUpdateRequired: false };
  }
  return { message: `This client requires a newer server. Update T3 Code on ${label} to connect.`, serverUpdateRequired: canSelfUpdate(descriptor) };
}

// MARK: Retry policy (connection/supervisor.ts, rpc/client.ts)

export const RETRY_BASE_DELAY_MS = 1_000;
export const RETRY_MAX_DELAY_MS = 300_000;
/** The ladder restarts only after a connection stays up this long (BACKOFF_RESET_AFTER_MS). */
export const BACKOFF_RESET_AFTER_MS = 30_000;
/** An explicit retry and an offline report want a fast answer (QUICK_CONNECTION_PROBE_TIMEOUT). */
export const QUICK_PROBE_MS = 3_000;
/** Returning to the app (CONNECTION_PROBE_TIMEOUT). */
export const FOREGROUND_PROBE_MS = 15_000;
/** CONNECTION_ESTABLISHMENT_TIMEOUT, plus the second the native transport waited past it. */
export const SOCKET_OPEN_TIMEOUT_MS = 16_000;
/**
 * The Ping cadence (makeProtocolSocket's 5 s default) and the silence after which the server counts as gone
 * (PING_TIMEOUT, rpc/protocol.ts). Any frame counts as the server's voice; the timeout is checked when the next
 * Ping is due, and only once a Ping went unanswered, so a late tick sends a Ping before it gives up.
 */
export const PING_INTERVAL_MS = 5_000;
export const PONG_TIMEOUT_MS = 15_000;
/** UNRESPONSIVE_HOST_HINT (errors/network.ts): a host that stopped answering on an open connection, so the network path worked. */
export const UNRESPONSIVE_HOST_HINT = 'It may be overloaded, asleep, or off the network.';
/** MAX_EXPECTED_FAILURE_RETRY_DELAY_MS and the first retry (`retryExpectedFailureAfter`). */
export const STREAM_RETRY_FIRST_MS = 250;
export const STREAM_RETRY_MAX_MS = 30_000;

/** retryDelayMs: the ceiling min(300 s, 1 s·2^(n+1)) and a uniform point in its upper half. `random` ∈ [0, 1). */
export function retryDelayMs(failureCount: number, random: number): number {
  const ceiling = Math.min(RETRY_MAX_DELAY_MS, RETRY_BASE_DELAY_MS * 2 ** (Math.max(0, failureCount) + 1));
  return Math.round(ceiling / 2 + (ceiling / 2) * Math.min(Math.max(random, 0), 1));
}

/** A same-session resubscribe after an expected stream failure: 250 ms doubling, at most 30 s. */
export function streamRetryMs(retries: number): number {
  return Math.min(STREAM_RETRY_FIRST_MS * 2 ** Math.min(Math.max(0, retries), 16), STREAM_RETRY_MAX_MS);
}

/** Errors a resubscribe cannot fix: they wait for the next session. */
export const retriesStream = (kind: string) => kind !== 'EnvironmentAuthorizationError';

// Random bytes drawn ahead. exact2 lets only an answer still in flight draw from `crypto`, and code that
// awaited a server reply resumes in the delivering answer after that answer has replied (transport.ts), so
// the transport's answers top this pool up as they start and every draw takes from it first.
let pool = new Uint8Array(0), used = 0;
const POOL_LOW = 512, POOL_DRAW = 2048;

/** Tops the pool up; a no-op where no answer is in flight to draw. */
export function refillRandom(): void {
  if (pool.length - used >= POOL_LOW) return;
  let fresh: Uint8Array;
  try { fresh = crypto.getRandomValues(new Uint8Array(POOL_DRAW)); } catch { return; }
  const rest = pool.subarray(used), next = new Uint8Array(rest.length + fresh.length);
  next.set(rest); next.set(fresh, rest.length);
  pool = next; used = 0;
}

/** `n` random bytes: from the pool, else from `crypto` (which refuses outside an answer). */
export function randomBytes(n: number): Uint8Array {
  if (pool.length - used < n) refillRandom();
  if (pool.length - used < n) return crypto.getRandomValues(new Uint8Array(n));
  const out = pool.slice(used, used + n);
  used += n;
  return out;
}

/** A uniform number in [0, 1). */
export function random(): number {
  const [a, b, c, d] = randomBytes(4);
  return ((a! << 24 >>> 0) + (b! << 16) + (c! << 8) + d!) / 2 ** 32;
}

/** A lowercase v4 UUID, for ids the server keys requests by. */
export function uuid(): string {
  const bytes = randomBytes(16);
  bytes[6] = (bytes[6]! & 0x0f) | 0x40; bytes[8] = (bytes[8]! & 0x3f) | 0x80;
  return Array.from(bytes, (byte, index) => `${[4, 6, 8, 10].includes(index) ? '-' : ''}${byte.toString(16).padStart(2, '0')}`).join('');
}
