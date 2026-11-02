// The AppDrop origin adapter: the URL app.json's "appdrop" resolves to, and
// the signed requests a publish sends. A local stand-in records them; the
// real server is expo/appdrop's web/worker/exactUpdates.ts.
import { afterAll, expect, test } from 'bun:test';
import { createPublicKey, generateKeyPairSync, sign, verify } from 'node:crypto';
import { AppDropOrigin, appdropOrigin, channelOrigin, openOrigin, sha256 } from './origin.mjs';

const SHELF = '/dF36/m50wjVH0iDogmQB4FTDF6bx+qwAyD0seHMV70=';
const manifest = (channels) => ({ deploy: { channels, signing: { key: 'shelf', keys: { shelf: SHELF } } } });

function signer() {
  const { privateKey, publicKey } = generateKeyPairSync('ed25519');
  return {
    keyId: 'test',
    publicKey: publicKey.export({ type: 'spki', format: 'der' }).subarray(-32).toString('base64'),
    signBytes: (bytes) => sign(null, bytes, privateKey),
  };
}

const requests = [];
let head = null;
const server = Bun.serve({
  port: 0,
  async fetch(request) {
    const body = Buffer.from(await request.arrayBuffer());
    requests.push({ method: request.method, path: new URL(request.url).pathname, headers: request.headers, body });
    if (request.method === 'GET' && request.url.endsWith('/exact.json')) return head ? new Response(head, { headers: { etag: '"h1"' } }) : new Response('', { status: 404 });
    if (request.method === 'GET') return Response.json({ names: ['a', 'b'] });
    if (request.headers.get('if-match') && request.headers.get('if-match') !== '"h1"') return Response.json({ error: 'The head changed since it was read' }, { status: 412 });
    return Response.json({ written: true });
  },
});
afterAll(() => server.stop());

test('"appdrop" is AppDrop\'s origin for the signing key, as the build bakes it', () => {
  // The same key id contract/cli's appdrop_is_named_after_the_signing_key expects.
  expect(appdropOrigin(manifest({}))).toBe('https://appdrop.app/u/832cad2da443ed7c9e8948f16e5b137d');
  expect(channelOrigin(manifest({ dogfood: 'appdrop' }), 'dogfood')).toBe('https://appdrop.app/u/832cad2da443ed7c9e8948f16e5b137d');
  expect(channelOrigin(manifest({ dogfood: '/srv/origin' }), 'dogfood')).toBe('/srv/origin');
  expect(() => appdropOrigin({ deploy: {} })).toThrow('deploy.signing.keys');
  expect(openOrigin('https://appdrop.app/u/832cad2da443ed7c9e8948f16e5b137d')).toBeInstanceOf(AppDropOrigin);
  expect(openOrigin('https://example.com/updates')).not.toBeInstanceOf(AppDropOrigin);
});

test('an AppDrop origin refuses a signer whose key it is not named after', () => {
  const origin = new AppDropOrigin('https://appdrop.app/u/832cad2da443ed7c9e8948f16e5b137d');
  expect(() => origin.useSigner(signer())).toThrow('belongs to another update key');
});

test('every write is signed over its path and body, and the head is put If-Match what was read', async () => {
  const key = signer();
  const keyId = sha256(Buffer.from(key.publicKey, 'base64')).slice(0, 32);
  const origin = new AppDropOrigin(`http://127.0.0.1:${server.port}/u/${keyId}`).useSigner(key);
  const blob = Buffer.from('plan');
  expect(await origin.put(`.exact/blobs/${sha256(blob)}`, blob)).toBe('written');
  const put = requests.at(-1);
  const message = Buffer.from(`appdrop-exact-put\n${keyId}\n.exact/blobs/${sha256(blob)}\n${sha256(blob)}`);
  const publicKey = createPublicKey({ key: Buffer.concat([Buffer.from('302a300506032b6570032100', 'hex'), Buffer.from(key.publicKey, 'base64')]), format: 'der', type: 'spki' });
  expect(verify(null, message, publicKey, Buffer.from(put.headers.get('x-exact-signature'), 'base64'))).toBe(true);

  expect(await origin.list('.exact/dogfood')).toEqual(['a', 'b']);
  expect(requests.at(-1).path).toBe(`/u/${keyId}/.exact/dogfood/`);

  const stream = { channel: 'dogfood', compatibilityId: 'c' };
  await origin.putHead(stream, Buffer.from('{"seq":1}'), { previousDigest: null });
  expect(requests.at(-1).headers.get('if-none-match')).toBe('*');
  head = '{"seq":1}';
  await origin.putHead(stream, Buffer.from('{"seq":2}'), { previousDigest: sha256(Buffer.from(head)) });
  expect(requests.at(-1).headers.get('if-match')).toBe('"h1"');
  await expect(origin.putHead(stream, Buffer.from('{"seq":3}'), { previousDigest: sha256(Buffer.from('other')) })).rejects.toThrow('changed since it was read');
});
