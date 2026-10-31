// A stand-in for the T3 server's signed asset routes, which an attachment's preview and Save fetch
// (attachment-files.ts, media-actions.ts readMediaBytes), and for the app's own files, where Save writes the bytes and
// a draft's staged copy is read (save-file.ts). `serveAssets` replaces `fetch` until `restore`; as the host does for
// `exactBodyTo`, it writes a 2xx body into the app's files and answers with an empty one.
import { adoptAppFiles, adoptedFiles, saveSnapshot } from '../../src/media/save-file';

export type AssetAnswer = { status?: number; body?: string | Uint8Array; headers?: Record<string, string> } | Error;

export function serveAssets(answer: (url: string, range: string | null) => AssetAnswer) {
  const calls: { url: string; range: string | null }[] = [];
  const original = globalThis.fetch;
  globalThis.fetch = (async (input: string | URL | Request, init?: RequestInit) => {
    const url = String(input), range = new Headers(init?.headers).get('Range');
    calls.push({ url, range });
    const reply = answer(url, range);
    if (reply instanceof Error) throw reply;
    const status = reply.status ?? 200, to = (init as { exactBodyTo?: string } | undefined)?.exactBodyTo;
    if (to && status >= 200 && status < 300) {
      const body = reply.body ?? '';
      await adoptedFiles()!.fs.writeFile(to, typeof body === 'string' ? new TextEncoder().encode(body) : body);
      return new Response('', { status, headers: reply.headers ?? {} });
    }
    return new Response(reply.body ?? '', { status, headers: reply.headers ?? {} });
  }) as typeof fetch;
  return { calls, restore: () => { globalThis.fetch = original; } };
}

/** The app's files in memory: `files` by `app:/` path. */
export function memoryAppFiles(files: Map<string, Uint8Array> = new Map()) {
  adoptAppFiles({ fs: {
    async readFile(path: string) { const bytes = files.get(path); if (!bytes) throw Object.assign(new Error('ENOENT'), { code: 'ENOENT' }); return bytes.slice().buffer; },
    // As the native host: a write takes bytes and refuses anything else.
    async writeFile(path: string, bytes: Uint8Array | ArrayBuffer) {
      if (!(bytes instanceof Uint8Array) && !(bytes instanceof ArrayBuffer)) throw new TypeError('invalid argument: fs writes require an ArrayBuffer or typed array');
      files.set(path, new Uint8Array(bytes));
    },
    async copyFile(from: string, to: string) { const bytes = files.get(from); if (!bytes) throw Object.assign(new Error('ENOENT'), { code: 'ENOENT' }); files.set(to, bytes.slice()); },
    async mkdir() {},
    async rm(path: string) { files.delete(path); },
    // The names directly in `path`, as a directory listing has them.
    async readdir(path: string) { return [...new Set([...files.keys()].filter(file => file.startsWith(`${path}/`)).map(file => file.slice(path.length + 1).split('/')[0]!))]; },
  } });
  return files;
}

/** The save the window was asked for last, with the bytes written for it. */
export function lastSave(client: object, files: Map<string, Uint8Array>) {
  const request = saveSnapshot(client).saveRequest;
  const bytes = files.get(request.from);
  return { ...request, text: bytes ? new TextDecoder().decode(bytes) : null };
}
