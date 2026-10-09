// T3 protocol 2, inspected at 4f7760e6. The backend remains the wire authority.
import { obj, str, num, arr, type Obj } from './domain';

export interface BridgeReply {
  ok: boolean;
  generation: number;
  value?: unknown;
  error?: { kind: string; message: string; uncertain: boolean; reason?: string; detail?: string };
}
export interface Native {
  available: boolean;
  watch(topic: string): void;
  later(request: unknown): Promise<unknown>;
}
export interface Files {
  fs: {
    mkdir(path: string): Promise<unknown>;
    readFile(path: string): Promise<ArrayBuffer>;
    atomicWriteFile(path: string, bytes: Uint8Array): Promise<unknown>;
  };
}
export class ClientError extends Error {
  /** A typed server error's own fields (PullRequestOperationError.reason "not-found", .detail); '' otherwise. */
  readonly reason: string;
  readonly detail: string;
  constructor(message: string, readonly kind = 'client', readonly uncertain = false, typed: { reason?: string; detail?: string } = {}) {
    super(message);
    this.reason = typed.reason ?? ''; this.detail = typed.detail ?? '';
  }
}
export function reply(value: unknown): BridgeReply {
  const data = obj(value);
  if (typeof data.ok !== 'boolean' || typeof data.generation !== 'number') {
    throw new ClientError('The native connection returned an invalid reply.', 'protocol');
  }
  const error = obj(data.error);
  return {
    ok: data.ok, generation: data.generation, value: data.value,
    ...(data.ok ? {} : { error: {
      kind: str(error.kind, 'transport'), message: str(error.message, 'The connection failed.'),
      uncertain: error.uncertain === true,
      ...(str(error.reason) ? { reason: str(error.reason) } : {}), ...(str(error.detail) ? { detail: str(error.detail) } : {}),
    } }),
  };
}

export async function bridgeReply(native: Native, request: unknown): Promise<BridgeReply> {
  const response = reply(await native.later(request));
  const transfer = obj(obj(response.value)._nativeTransfer);
  if (!response.ok || !str(transfer.id)) return response;
  const id = str(transfer.id), parts = num(transfer.parts), generation = response.generation;
  if (!Number.isInteger(parts) || parts < 1 || parts > 4096) throw new ClientError('T3 returned an invalid transfer.', 'protocol');
  const fragments: string[] = [];
  try {
    for (let index = 0; index < parts; index++) {
      const chunk = reply(await native.later({ op: 'readChunk', id, index, generation }));
      if (!chunk.ok || chunk.generation !== generation || typeof obj(chunk.value).text !== 'string') {
        throw new ClientError('The connection changed while reading a T3 response.', 'transport');
      }
      fragments.push(str(obj(chunk.value).text));
    }
    return { ...response, value: JSON.parse(fragments.join('')) as unknown };
  } finally {
    await native.later({ op: 'releaseChunk', id, generation }).catch(() => {});
  }
}

// Keep the versioned preference file under Exact's app-scoped native data root.
// Each operation uses this answer's native ticket; it never opens an Exact
// storage turn or chains work behind another answer's pending promise.
export function nativeFiles(native: Native): Files {
  const invoke = async (request: Obj) => {
    const result = await bridgeReply(native, request);
    if (!result.ok) throw new ClientError(result.error!.message, result.error!.kind);
    return obj(result.value);
  };
  const checkPath = (path: string) => {
    if (path !== 'app:/data/t3-code.json') throw new ClientError('The preference path is invalid.', 'Persistence');
  };
  return { fs: {
    async mkdir() {},
    async readFile(path) {
      checkPath(path);
      const result = await invoke({ op: 'readPreferences' });
      if (typeof result.text !== 'string') throw new ClientError('The saved preferences could not be read.', 'Persistence');
      return new TextEncoder().encode(result.text).buffer;
    },
    async atomicWriteFile(path, bytes) {
      checkPath(path);
      await invoke({ op: 'writePreferences', text: new TextDecoder().decode(bytes) });
    },
  } };
}
/**
 * The address and code a pairing names. A host with a space, which URL refuses ("invalid international domain name"),
 * is one the reference's Chromium renderer escapes instead (`not a url` is `https://not%20a%20url/`) and then fails to
 * reach: such a target carries that origin and `unreachable`, and the caller reports the reference's transport failure
 * (environmentFetchFailure) once its own checks (a pairing code) pass. Any other address URL refuses is "Backend URL is
 * invalid." (RemoteBackendUrlInvalidError).
 */
export function parsePairing(input: string, credential: string): { origin: string; credential: string; unreachable?: true } {
  if (/^(https?|wss?):\/\//i.test(credential.trim())) return parsePairing(credential.trim(), '');
  const trimmed = input.trim();
  if (!trimmed) throw new ClientError('Enter the T3 server address or pairing link.');
  const address = /^[a-z]+:\/\//i.test(trimmed) ? trimmed : `https://${trimmed}`;
  let url: URL;
  try { url = new URL(address); } catch { return escapedTarget(address, credential.trim()); }
  if (!['https:', 'http:', 'wss:', 'ws:'].includes(url.protocol)) {
    throw new ClientError('Use an HTTP or HTTPS server address.');
  }
  const fragment = new URLSearchParams(url.hash.replace(/^#/, ''));
  const token = fragment.get('token') || url.searchParams.get('token') || credential.trim();
  const host = url.searchParams.get('host');
  const hostAddress = host ? (/^[a-z]+:\/\//i.test(host) ? host : `https://${host}`) : '';
  let base: URL;
  try { base = host ? new URL(hostAddress) : url; } catch { return escapedTarget(hostAddress, token); }
  if (!['https:', 'http:', 'wss:', 'ws:'].includes(base.protocol) || base.username || base.password) {
    throw new ClientError('The server address is invalid.');
  }
  base.protocol = base.protocol === 'ws:' ? 'http:' : base.protocol === 'wss:' ? 'https:' : base.protocol;
  return { origin: base.origin, credential: token };
}
/** Chromium's origin for an address whose host has spaces (escaped as %20), or "Backend URL is invalid." */
function escapedTarget(address: string, credential: string): { origin: string; credential: string; unreachable: true } {
  const match = /^(https?|wss?):\/\/([^/?#@]*)(?:[/?#]|$)/i.exec(address);
  const scheme = match ? (match[1]!.toLowerCase() === 'ws' ? 'http' : match[1]!.toLowerCase() === 'wss' ? 'https' : match[1]!.toLowerCase()) : '';
  const authority = match ? match[2]!.toLowerCase().replace(/ /g, '%20') : '';
  if (!match || !authority.includes('%20') || !/^[a-z0-9.%_~-]+(:\d{1,5})?$/.test(authority)) throw new ClientError('Backend URL is invalid.');
  return { origin: `${scheme}://${authority}`, credential, unreachable: true };
}
/** failRemoteRequest's message for a request that never reached the host (packages/client-runtime/src/rpc/http.ts). */
export function environmentFetchFailure(origin: string): string {
  const url = `${origin}/.well-known/t3/environment`;
  return `Failed to fetch remote environment endpoint ${url} (HttpClientError: Transport error (GET ${url})).`;
}
export function activeRun(projection: Obj): Obj | undefined {
  return arr(projection.runs).slice().reverse().find(run => ['preparing', 'starting', 'running', 'waiting'].includes(str(run.status)));
}
export function providerAvailable(provider: Obj): boolean {
  return provider.enabled === true && provider.installed === true && provider.availability !== 'unavailable'
    && obj(provider.auth).status !== 'unauthenticated' && provider.status !== 'error';
}
export function modelSelection(instanceId: string, model: string, options?: unknown): Obj {
  if (!instanceId || !model) throw new ClientError('Choose an available provider and model.');
  return { instanceId, model, ...(Array.isArray(options) && options.length ? { options: arr(options) } : {}) };
}
export function sendPayload(commandId: string, threadId: string, messageId: string, text: string, attachments: Obj[] = []): Obj {
  return {
    type: 'message.dispatch', commandId, threadId, messageId, text, attachments,
    createdBy: 'user', creationSource: 'web', deliveryIntent: 'auto', dispatchMode: { type: 'start_immediately' },
  };
}
export function launchPayload(commandId: string, threadId: string, messageId: string, projectId: string,
  text: string, selection: Obj, runtimeMode: string, interactionMode: string, attachments: Obj[] = []): Obj {
  return {
    commandId, threadId, projectId, title: text.trim().split('\n')[0].slice(0, 100) || 'New thread',
    generateTitle: true, creationSource: 'web', modelSelection: selection, runtimeMode, interactionMode,
    workspaceStrategy: { type: 'root' }, initialMessage: { messageId, text, attachments },
  };
}

/**
 * applyServerConfigProjection (client-runtime state/serverConfigProjection.ts). A
 * HEAD server may send a second snapshot mid-stream (late editor discovery,
 * 0080e80); snapshots never carry published themes or usage-limit sources, so a
 * capable server's current sets are carried across it.
 */
export function applyConfig(current: Obj, event: Obj): Obj {
  if (event.type === 'snapshot') {
    const config = obj(event.config);
    if (!Array.isArray(config.providers) || !str(obj(config.environment).environmentId)) {
      throw new ClientError('T3 returned an invalid provider configuration.', 'protocol');
    }
    const capabilities = obj(obj(config.environment).capabilities);
    const carried: Obj = { ...config };
    delete carried.environmentThemes; delete carried.usageLimitSources;
    if (capabilities.environmentThemes === true && Array.isArray(current.environmentThemes)) carried.environmentThemes = current.environmentThemes;
    if (capabilities.usageLimitSources === true && Array.isArray(current.usageLimitSources)) carried.usageLimitSources = current.usageLimitSources;
    return carried;
  }
  if (!Object.keys(current).length) return current;
  const payload = obj(event.payload);
  const without = (key: string): Obj => { const next = { ...current }; delete next[key]; return next; };
  if (event.type === 'keybindingsUpdated') return { ...current, keybindings: arr(payload.keybindings), issues: arr(payload.issues) };
  if (event.type === 'providerStatuses') return { ...current, providers: arr(payload.providers) };
  if (event.type === 'settingsUpdated') return { ...current, settings: obj(payload.settings) };
  // The full published set; an empty set clears it (never in a snapshot).
  if (event.type === 'environmentThemesUpdated') return arr(payload.themes).length ? { ...current, environmentThemes: arr(payload.themes) } : without('environmentThemes');
  // Quota from the server's usage-limit sources (never in a snapshot; /usage-limits reads it).
  if (event.type === 'usageLimitSourcesUpdated') return arr(payload.sources).length ? { ...current, usageLimitSources: arr(payload.sources) } : without('usageLimitSources');
  return current;
}
