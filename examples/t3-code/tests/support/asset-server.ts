// A stand-in for the T3 server's signed asset routes, which an attachment's preview and Save fetch
// (attachment-files.ts, media-actions.ts readMediaBytes), and for the app's own files, where Save writes the bytes and
// a draft's staged copy is read (save-file.ts). `serveAssets` replaces `fetch` until `restore`.
import { adoptAppFiles, saveSnapshot } from '../../src/media/save-file';

export type AssetAnswer = { status?: number; body?: string | Uint8Array; headers?: Record<string, string> } | Error;

export function serveAssets(answer: (url: string, range: string | null) => AssetAnswer) {
  const calls: { url: string; range: string | null }[] = [];
  const original = globalThis.fetch;
  globalThis.fetch = (async (input: string | URL | Request, init?: RequestInit) => {
    const url = String(input), range = new Headers(init?.headers).get('Range');
    calls.push({ url, range });
    const reply = answer(url, range);
    if (reply instanceof Error) throw reply;
    return new Response(reply.body ?? '', { status: reply.status ?? 200, headers: reply.headers ?? {} });
  }) as typeof fetch;
  return { calls, restore: () => { globalThis.fetch = original; } };
}

/** The app's files in memory: `files` by `app:/` path. */
export function memoryAppFiles(files: Map<string, Uint8Array> = new Map()) {
  adoptAppFiles({ fs: {
    async readFile(path: string) { const bytes = files.get(path); if (!bytes) throw Object.assign(new Error('ENOENT'), { code: 'ENOENT' }); return bytes.slice().buffer; },
    async writeFile(path: string, bytes: Uint8Array) { files.set(path, new Uint8Array(bytes)); },
    async mkdir() {},
  } });
  return files;
}

/** The save the window was asked for last, with the bytes written for it. */
export function lastSave(client: object, files: Map<string, Uint8Array>) {
  const request = saveSnapshot(client).saveRequest;
  const bytes = files.get(request.from);
  return { ...request, text: bytes ? new TextDecoder().decode(bytes) : null };
}
