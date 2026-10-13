// @ref LLP 1086 D12 — server-only operations: no device, partition, or wasm import.
export interface Refusal { code: string; message: string; retryable?: boolean }
export interface Op<A = Record<string, unknown>, R = unknown> {
  readonly kind: string; readonly name: string; readonly __args?: A; readonly __result?: R;
}
export type ArgsOf<O> = O extends Op<infer A, unknown> ? A : Record<string, unknown>;
export type ResultOf<O> = O extends Op<unknown, infer R> ? R : unknown;
export interface Options { origin: string; headers?: () => Record<string, string | undefined> }
export type Read<T> = { data?: T; next?: string | null; server: true; denied?: Refusal; offline?: boolean };
export type Sent<T> = { state: 'sent'; result: T } | { state: 'failed'; why: Refusal };
const nameOf = (op: string | Op): string => typeof op === 'string' ? op : op.name;

/** Use when the schema syncs nothing. Calls go straight to /q and /m; writes
 * have no prediction or outbox, and an interrupted write has an unknown fate.
 * Never retry it automatically: the server may already have committed it. */
export class Snapback {
  constructor(private readonly options: Options) {}
  private async exchange(path: string, args: unknown): Promise<Record<string, unknown>> {
    const headers: Record<string, string> = { 'content-type': 'application/json' };
    for (const [key, value] of Object.entries(this.options.headers?.() ?? {})) if (value !== undefined) headers[key] = value;
    let response: Response;
    try { response = await fetch(this.options.origin.replace(/\/$/, '') + path, { method: 'POST', headers, body: JSON.stringify({ args }) }); }
    catch { throw refused({ code: 'E_OFFLINE', message: 'The server was not reached; a write may already have committed. Read its state before retrying.' }); }
    const body = await response.json().catch(() => null) as Record<string, unknown> | null;
    if (!body || typeof body !== 'object' || Array.isArray(body)) throw refused({ code: 'E_SERVER', message: `Invalid reply (HTTP ${response.status}); read state before retrying a write.` });
    if (!response.ok && !body.denied && !(body.state === 'failed' && body.why)) throw refused({ code: 'E_SERVER', message: `HTTP ${response.status}; read state before retrying a write.` });
    return body;
  }
  async read<O extends string | Op>(op: O, args: NoInfer<ArgsOf<O>>): Promise<Read<ResultOf<O>>> {
    const body = await this.exchange('/q/' + encodeURIComponent(nameOf(op)), args);
    if (!Object.hasOwn(body, 'data') && !body.denied) throw refused({code: 'E_SERVER', message: 'Invalid query envelope'});
    return { ...body, server: true } as Read<ResultOf<O>>;
  }
  async send<O extends string | Op>(op: O, args: NoInfer<ArgsOf<O>>): Promise<Sent<ResultOf<O>>> {
    const body = await this.exchange('/m/' + encodeURIComponent(nameOf(op)), args);
    const why = body.state === 'failed' ? body.why : body.denied;
    if (why && typeof why === 'object' && typeof (why as Refusal).code === 'string' && typeof (why as Refusal).message === 'string') return { state: 'failed', why: why as Refusal };
    if (body.state !== 'sent' || !Object.hasOwn(body, 'result')) throw refused({ code: 'E_SERVER', message: 'Invalid mutation envelope; read state before retrying.' });
    return { state: 'sent', result: body.result as ResultOf<O> };
  }
}

export function refused(why: Refusal): Error {
  return Object.assign(new Error(`${why.code}: ${why.message}`), { code: why.code, reason: why.code, refusal: why });
}
