// The Effect RPC wire T3's server speaks over its socket, and the HTTP pieces every
// connection shares (T3 Code 9c161462c9, MIT, see LICENSE-T3: packages/client-runtime/src/rpc/
// protocol.ts and session.ts frame JSON with effect's RpcSerialization.layerJson;
// connection/compatibility.ts, supervisor.ts retryDelayMs, rpc/client.ts's expected-failure
// retry; authorization/remote.ts bootstrapRemoteBearerSession; connection/errors.ts
// mapRemoteEnvironmentError). Effect itself is not imported: a data module has no timers.

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
  constructor(readonly kind: string, message: string, readonly uncertain = false, readonly reason = '', readonly detail = '', readonly context: Record<string, string> = {}) {
    super(message);
  }
  json() { return { kind: this.kind, message: this.message, uncertain: this.uncertain, reason: this.reason, detail: this.detail, ...(Object.keys(this.context).length ? { context: this.context } : {}) }; }
  with(fields: { message?: string; uncertain?: boolean }) {
    return new TransportFailure(this.kind, fields.message ?? this.message, fields.uncertain ?? this.uncertain, this.reason, this.detail, this.context);
  }
}

/** A typed error's context fields a client reads (ProjectReadFileError's failure, operation and the paths it tried). */
const CONTEXT_FIELDS = ['failure', 'operation', 'cwd', 'relativePath', 'resolvedPath', 'operationPath'];

const isObject = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value);
const text = (value: unknown) => typeof value === 'string' ? value : '';

/** The largest frame either side sends (the clone's own bound, as T3Wire had it). */
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

/** A request carrying the orchestration protocol header. */
export function request(id: string, method: string, payload: unknown): Frame {
  return { _tag: 'Request', id, tag: method, payload, headers: [['x-t3-orchestration-protocol', String(PROTOCOL_VERSION)]] };
}

/** PullRequestUnavailableError's message is a getter, not a wire field: a stable sentence from its reason and host. */
const requirement: Record<string, { missing: string; unauthenticated: string }> = {
  github: { missing: 'GitHub CLI (`gh`) is required to browse change requests on this host. Install it from https://cli.github.com/ and reload.',
    unauthenticated: 'GitHub CLI is not authenticated. Run `gh auth login` and retry.' },
  forgejo: { missing: 'Install Forgejo CLI (`fj` 0.6 or later) from https://codeberg.org/forgejo-contrib/forgejo-cli or Gitea CLI (`tea` 0.16 or later) from https://gitea.com/gitea/tea to browse Forgejo pull requests.',
    unauthenticated: 'Authenticate your Forgejo or Gitea server with `fj --host <server-url> auth add-token` on the T3 Code server. If fj is missing or unconfigured for that server, use `tea login add`. A configured fj account must be repaired with fj.' },
  gitlab: { missing: 'GitLab CLI (`glab`) is required to browse change requests on this host. Install it from https://gitlab.com/gitlab-org/cli and reload.',
    unauthenticated: 'GitLab CLI is not authenticated. Run `glab auth login` and retry.' },
  'azure-devops': { missing: 'Azure CLI (`az`) with the Azure DevOps extension is required. Install `az`, then run `az extension add --name azure-devops`.',
    unauthenticated: 'Azure CLI is not signed in. Run `az login` and retry.' },
  bitbucket: { missing: 'Bitbucket needs API credentials on the server. Add them in Settings → Source Control.',
    unauthenticated: 'Bitbucket rejected the configured credentials. Check them in Settings → Source Control.' },
};
export function pullRequestUnavailable(reason: string, provider: string): string {
  const known = requirement[provider];
  if (reason === 'cli-missing') return known?.missing ?? 'The tool this host is read through is not installed or set up.';
  if (reason === 'cli-unauthenticated') return known?.unauthenticated ?? 'This host has no working credentials.';
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

function typed(value: unknown): TransportFailure {
  if (!isObject(value)) return failure(value);
  const tag = text(value._tag) || text(value.name) || 'RPC';
  const field = (key: string) => text(value[key]).trim();
  let message = field('message');
  // PullRequestOperationError's and PullRequestUnavailableError's messages are getters, not wire fields.
  if (!message && tag === 'PullRequestOperationError' && field('detail')) message = `Pull request operation ${field('operation')} failed: ${field('detail')}`;
  if (!message && tag === 'PullRequestUnavailableError') message = pullRequestUnavailable(field('reason'), field('provider'));
  if (!message && field('detail')) message = field('detail');
  if (!message && 'cause' in value) message = failure(value.cause).message;
  if (!message || message === 'The server request failed.') message = `The server refused the request (${tag}).`;
  const context = tag === 'ProjectReadFileError' ? Object.fromEntries(CONTEXT_FIELDS.filter(key => text(value[key])).map(key => [key, text(value[key])])) : {};
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
    const sentence = pullRequestUnavailable(reason, field('provider'));
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
/** The Ping cadence and the silence after which the server counts as gone. */
export const PING_INTERVAL_MS = 10_000;
export const PONG_TIMEOUT_MS = 35_000;
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
