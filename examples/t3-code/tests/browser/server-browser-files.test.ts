import { expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import { adoptAppFiles, saveSnapshot } from '../../src/media/save-file';
import { downloadStreamFile, streamMediaUrl, uploadStreamFiles } from '../../src/browser/server-browser-files';
const native = { available: true } as Native;
function fixture() {
  let ticket = 0;
  const client = { async call() { return { httpBase: 'http://127.0.0.1/api/preview-stream', ticket: `media-${++ticket}` }; } } as unknown as T3Client;
  return client;
}
test('each media request gets its own ticket and safely encodes the chooser identity', async () => {
  const client = fixture();
  const download = new URL(await streamMediaUrl(client, native, 'thread&other', 'tab', 'download', '../a?b'));
  const upload = new URL(await streamMediaUrl(client, native, 'thread&other', 'tab', 'upload', '../a?b'));
  expect(download.pathname).toBe('/api/preview-stream/download'); expect(download.searchParams.get('id')).toBe('../a?b');
  expect(upload.searchParams.get('chooser')).toBe('../a?b'); expect(upload.searchParams.get('threadId')).toBe('thread&other');
  expect([download.searchParams.get('wsTicket'), upload.searchParams.get('wsTicket')]).toEqual(['media-1', 'media-2']);
});
test('a downloaded file preserves its bytes and opens Save only while its tab still owns the request', async () => {
  const client = fixture(), writes: { path: string; bytes: number[] }[] = [], original = globalThis.fetch;
  adoptAppFiles({ fs: { mkdir: async () => {}, readFile: async () => new ArrayBuffer(0), writeFile: async (path: string, bytes: Uint8Array) => { writes.push({ path, bytes: [...bytes] }); } } });
  try {
    globalThis.fetch = (async () => new Response(new Uint8Array([0, 255, 10]))) as typeof fetch;
    const download = { id: 'file', fileName: '../report.pdf', sizeBytes: 3 };
    await downloadStreamFile(client, native, 'thread', 'tab', download, () => false); expect(writes).toEqual([]);
    await downloadStreamFile(client, native, 'thread', 'tab', download, () => true); expect(writes[0]?.bytes).toEqual([0, 255, 10]);
    expect(saveSnapshot(client).saveRequest.name).toBe('..-report.pdf');
  } finally { globalThis.fetch = original; }
});
test('a chooser uploads picked bytes as multipart and rejects an arbitrary local path', async () => {
  const client = fixture(), original = globalThis.fetch, requests: RequestInit[] = [], bytes = new Uint8Array([0, 255, 10]);
  adoptAppFiles({ fs: { mkdir: async () => {}, writeFile: async () => {}, readFile: async () => bytes.buffer } });
  try {
    globalThis.fetch = (async (_url: unknown, init?: RequestInit) => { requests.push(init!); return new Response(''); }) as typeof fetch;
    await expect(uploadStreamFiles(client, native, 'thread', 'tab', 'chooser', [{ path: '/Users/person/secret', name: 'secret', type: 'text/plain', size: 3 }], () => true)).rejects.toThrow('browser file picker');
    await uploadStreamFiles(client, native, 'thread', 'tab', 'chooser', [{ path: 'app:/tmp/picked/file', name: 'a"\r\nb.bin', type: 'application/octet-stream', size: 3 }], () => true);
    expect(requests).toHaveLength(1); expect(requests[0]?.method).toBe('POST');
    const body = requests[0]!.body as Uint8Array, prefix = new TextDecoder().decode(body);
    expect(prefix).toContain('filename="a___b.bin"'); expect(prefix).toContain('name="file"');
    const offset = prefix.indexOf('\r\n\r\n') + 4; expect([...body.slice(offset, offset + 3)]).toEqual([...bytes]);
    await uploadStreamFiles(client, native, 'thread', 'tab', 'chooser', [], () => false); expect(requests).toHaveLength(1);
  } finally { globalThis.fetch = original; }
});
