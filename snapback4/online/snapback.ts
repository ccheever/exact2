// @ref LLP 1086 D12 — server-only operations: no device, partition, or wasm import.
export interface Refusal { code: string; message: string; family?: string; retryable?: boolean }
export interface Op<A = Record<string, unknown>, R = unknown> {
  readonly kind: string; readonly name: string; readonly __args?: A; readonly __result?: R;
}
export type ArgsOf<O> = O extends Op<infer A, unknown> ? A : Record<string, unknown>;
export type ResultOf<O> = O extends Op<unknown, infer R> ? R : unknown;
export interface Options {
  origin: string;
  headers?: () => Record<string, string | undefined>;
  /** Download writes under the dispatcher's fs.write grant; no partition is opened. */
  storage?: { fs: { writeFile(path: string, bytes: Uint8Array): Promise<void> } } | null;
  name?: string;
}
export type Cursor = string & { readonly __cursor: 'Cursor' };
export type ReadCut = 'meter' | 'words' | null;
/** The server's /q envelope. Missing metadata on a denial is never filled in. */
export interface Read<T> {
  data?: T; complete?: boolean; cut?: ReadCut; next?: Cursor | null;
  server: true; denied?: Refusal;
}
export type Sent<T> = { state: 'sent'; result: T } | { state: 'failed'; why: Refusal };
export interface Asset { id: string; type: string; bytes: number; name?: string; width?: number | null; height?: number | null; duration?: number | null }
export interface UploadOptions { file?: { type: string; name: string } }
/** `upload`'s answer: the asset, or why not (`E_ASSET_TYPE`,
 * `E_ASSET_TOO_LARGE`, `E_ASSET_BOUNDS`, `E_RATE_LIMIT`, `E_OFFLINE`, …). */
export type Uploaded = { ok: true; asset: Asset } | { ok: false; why: Refusal };
/** `download`'s answer: the app file holding the bytes (an `image` or
 * `video` source), or why not (`E_ASSET_UNKNOWN`: missing, or hidden by its
 * row's read rule; `E_ASSET_VIEWED`; `E_OFFLINE`). */
export type Downloaded = { ok: true; path: string; type: string; bytes: number; name?: string } | { ok: false; why: Refusal };

const EXTENSIONS: Record<string, string> = { 'image/jpeg': 'jpg', 'image/png': 'png', 'image/webp': 'webp', 'video/mp4': 'mp4', 'text/csv': 'csv', 'text/tab-separated-values': 'tsv', 'text/plain': 'txt', 'text/markdown': 'md', 'text/html': 'html', 'application/json': 'json', 'application/xml': 'xml', 'application/yaml': 'yaml' };

/** A fetch that rejected: unreached is `E_OFFLINE`; Exact's own refusal
 * before sending (`exactBodyFrom` naming a missing or ungranted file) is
 * `E_CLIENT`. */
function unreached(error: unknown): Refusal {
  const kind = (error as { kind?: string } | null)?.kind, message = error instanceof Error ? error.message : String(error);
  return kind === 'Refused' || kind === 'Unsupported'
    ? { code: 'E_CLIENT', family: 'client', message }
    : { code: 'E_OFFLINE', family: 'transport', retryable: true, message: `the server was not reached: ${message}` };
}


const nameOf = (op: string | Op): string => typeof op === 'string' ? op : op.name;

/** Use when the schema syncs nothing. Calls go straight to /q and /m; writes
 * have no prediction or outbox, and an interrupted write has an unknown fate.
 * Never retry it automatically: the server may already have committed it. */
export class Snapback {
  constructor(private readonly options: Options) {}
  private headers(): Record<string, string> {
    const headers: Record<string, string> = {};
    for (const [key, value] of Object.entries(this.options.headers?.() ?? {})) if (value !== undefined) headers[key] = value;
    return headers;
  }
  private async exchange(path: string, args: unknown): Promise<Record<string, unknown>> {
    const headers = { ...this.headers(), 'content-type': 'application/json' };
    let response: Response;
    try { response = await fetch(this.options.origin.replace(/\/+$/, '') + path, { method: 'POST', headers, body: JSON.stringify({ args }) }); }
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
  async upload(file: string | ArrayBuffer | ArrayBufferView, options: UploadOptions = {}): Promise<Uploaded> {
    const headers = { ...this.headers(), 'content-type': options.file?.type ?? 'application/octet-stream' };
    const query = options.file ? `?as=file&name=${encodeURIComponent(options.file.name)}` : '';
    const init: RequestInit & { exactBodyFrom?: string } = typeof file === 'string'
      ? { method: 'POST', headers, exactBodyFrom: file }
      // A copy: the bytes sent are these, whatever the caller does with its buffer meanwhile.
      : { method: 'POST', headers, body: Uint8Array.from(ArrayBuffer.isView(file) ? new Uint8Array(file.buffer, file.byteOffset, file.byteLength) : new Uint8Array(file)) };
    let response: Response;
    try { response = await fetch(`${this.options.origin.replace(/\/+$/, '')}/assets${query}`, init); } catch (error) { return { ok: false, why: unreached(error) }; }
    const body = await response.json().catch(() => null) as { asset?: Asset; denied?: Refusal } | null;
    if (response.ok && body?.asset?.id) return { ok: true, asset: body.asset };
    return { ok: false, why: body?.denied ?? { code: 'E_SERVER', family: 'transport', retryable: response.status >= 500, message: `the server answered the upload with HTTP ${response.status}` } };
  }

  /** Download an asset (`GET /assets/<id>`, under this client's `headers`,
   * so its row's read rule decides) into an app file, and answer its path
   * for an `image` or `video`: by default
   * `app:/cache/snapback4-<name>-<id>.<jpg|png|webp|mp4>` (grant `fs.write
   * app:/cache`). Online only, and nothing is purged here: the file stays
   * until the app removes it, so give a `once` asset (one view per
   * principal, which this download spends) a path under `app:/tmp`. */
  async download(id: string, path?: string): Promise<Downloaded> {
    const fs = this.options.storage?.fs;
    if (!fs) return { ok: false, why: { code: 'E_CLIENT', family: 'client', message: 'download writes an app file: pass the dispatcher\'s `storage`, and grant fs.write for the path' } };
    let response: Response;
    try { response = await fetch(`${this.options.origin.replace(/\/+$/, '')}/assets/${encodeURIComponent(id)}`, { headers: this.headers() }); } catch (error) { return { ok: false, why: unreached(error) }; }
    if (!response.ok) {
      const body = await response.json().catch(() => null) as { denied?: Refusal } | null;
      return { ok: false, why: body?.denied ?? { code: 'E_SERVER', family: 'transport', retryable: response.status >= 500, message: `the server answered asset ${id} with HTTP ${response.status}` } };
    }
    const type = (response.headers.get('content-type') ?? '').split(';')[0].trim();
    const disposition = response.headers.get('content-disposition') ?? '';
    const encoded = /filename\*=UTF-8''([^;]+)/i.exec(disposition)?.[1];
    let name = /filename="([^"]*)"/i.exec(disposition)?.[1];
    if (encoded) try { name = decodeURIComponent(encoded); } catch { /* Keep the plain filename. */ }
    // An attachment name is metadata, never an app path supplied by the server.
    const filename = name?.replace(/[/\\\x00-\x1f\x7f]/g, '_');
    const bytes = new Uint8Array(await response.arrayBuffer());
    const file = path ?? `app:/cache/snapback4-${encodeURIComponent(this.options.name ?? 'online')}-${encodeURIComponent(id)}${filename ? `-${filename}` : `.${EXTENSIONS[type] ?? 'bin'}`}`;
    try { await fs.writeFile(file, bytes); } catch (error) { return { ok: false, why: { code: 'E_CLIENT', family: 'client', message: `could not write ${file}: ${error instanceof Error ? error.message : String(error)}` } }; }
    return { ok: true, path: file, type, bytes: bytes.byteLength, ...(name === undefined ? {} : { name }) };
  }

}

export function refused(why: Refusal): Error {
  return Object.assign(new Error(`${why.code}: ${why.message}`), { code: why.code, reason: why.code, refusal: why });
}
