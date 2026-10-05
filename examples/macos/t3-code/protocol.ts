// T3 protocol 2, inspected at 4f7760e6. The backend remains the wire authority.
import { obj, str, num, arr, type Obj } from './domain';

export interface BridgeReply {
  ok: boolean;
  generation: number;
  value?: unknown;
  error?: { kind: string; message: string; uncertain: boolean };
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
  constructor(message: string, readonly kind = 'client', readonly uncertain = false) {
    super(message);
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
export function parsePairing(input: string, credential: string): { origin: string; credential: string } {
  if (/^(https?|wss?):\/\//i.test(credential.trim())) return parsePairing(credential.trim(), '');
  const trimmed = input.trim();
  if (!trimmed) throw new ClientError('Enter the T3 server address or pairing link.');
  const url = new URL(/^[a-z]+:\/\//i.test(trimmed) ? trimmed : `https://${trimmed}`);
  if (!['https:', 'http:', 'wss:', 'ws:'].includes(url.protocol)) {
    throw new ClientError('Use an HTTP or HTTPS server address.');
  }
  const fragment = new URLSearchParams(url.hash.replace(/^#/, ''));
  const token = fragment.get('token') || url.searchParams.get('token') || credential.trim();
  const host = url.searchParams.get('host');
  const base = host ? new URL(/^[a-z]+:\/\//i.test(host) ? host : `https://${host}`) : url;
  if (!['https:', 'http:', 'wss:', 'ws:'].includes(base.protocol) || base.username || base.password) {
    throw new ClientError('The server address is invalid.');
  }
  base.protocol = base.protocol === 'ws:' ? 'http:' : base.protocol === 'wss:' ? 'https:' : base.protocol;
  return { origin: base.origin, credential: token };
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
export function sendPayload(commandId: string, threadId: string, messageId: string, text: string): Obj {
  return {
    type: 'message.dispatch', commandId, threadId, messageId, text, attachments: [],
    createdBy: 'user', creationSource: 'web', deliveryIntent: 'auto', dispatchMode: { type: 'start_immediately' },
  };
}
export function launchPayload(commandId: string, threadId: string, messageId: string, projectId: string,
  text: string, selection: Obj, runtimeMode: string, interactionMode: string): Obj {
  return {
    commandId, threadId, projectId, title: text.trim().split('\n')[0].slice(0, 100) || 'New thread',
    generateTitle: true, creationSource: 'web', modelSelection: selection, runtimeMode, interactionMode,
    workspaceStrategy: { type: 'root' }, initialMessage: { messageId, text, attachments: [] },
  };
}

export function applyConfig(current: Obj, event: Obj): Obj {
  if (event.type === 'snapshot') {
    const config = obj(event.config);
    if (!Array.isArray(config.providers) || !str(obj(config.environment).environmentId)) {
      throw new ClientError('T3 returned an invalid provider configuration.', 'protocol');
    }
    return config;
  }
  const payload = obj(event.payload);
  if (event.type === 'providerStatuses') return { ...current, providers: arr(payload.providers) };
  if (event.type === 'settingsUpdated') return { ...current, settings: obj(payload.settings) };
  return current;
}
