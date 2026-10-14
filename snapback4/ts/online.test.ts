import { expect, test } from 'bun:test';
import { Snapback, type Cursor } from '../online/snapback.ts';

test('online operations use the generated args and result envelopes without a device', async () => {
  const requests: { path: string; body: unknown; persona: string | null; device: string | null }[] = [];
  const server = Bun.serve({ hostname: '127.0.0.1', port: 0, async fetch(request) {
    const path = new URL(request.url).pathname;
    requests.push({path, body: await request.json(), persona: request.headers.get('x-snapback-persona'), device: request.headers.get('x-snapback-device')});
    if (path === '/q/denied') return Response.json({denied: {code: 'E_AUTH', message: 'Sign in'}}, {status: 401});
    if (path === '/m/refuse') return Response.json({state: 'failed', why: {code: 'FULL', message: 'No room'}});
    if (path === '/m/broken') return Response.json({data: null});
    return Response.json(path.startsWith('/q/') ? {data: [{body: 'hello'}], complete: false, cut: 'meter', next: 'opaque'} : {state: 'sent', result: {id: 'note-1'}});
  }});
  try {
    const db = new Snapback({origin: `http://127.0.0.1:${server.port}/`, headers: () => ({'x-snapback-persona': 'alice'})});
    const api = {post: {kind: 'mutation', name: 'post'} as {kind: 'mutation'; name: string; __args?: {body: string}; __result?: {id: string}}};
    expect(await db.send(api.post, {body: 'hello'})).toEqual({state: 'sent', result: {id: 'note-1'}});
    expect(await db.read('recent', {})).toEqual({data: [{body: 'hello'}], complete: false, cut: 'meter', next: 'opaque', server: true});
    expect(await db.send('refuse', {})).toEqual({state: 'failed', why: {code: 'FULL', message: 'No room'}});
    await expect(db.send('broken', {})).rejects.toMatchObject({code: 'E_SERVER'});
    expect(requests[0]).toEqual({path: '/m/post', body: {args: {body: 'hello'}}, persona: 'alice', device: null});
    const page = await db.read('recent', {});
    const cursor: Cursor | null | undefined = page.next;
    expect(cursor).toBe('opaque');
    expect(await db.read('denied', {})).toEqual({denied: {code: 'E_AUTH', message: 'Sign in'}, server: true});
    expect(requests).toHaveLength(6);
  } finally { server.stop(true); }
});


test('online assets use credentialed server routes, app files and explicit refusals', async () => {
  const bytes = new Uint8Array([1, 2, 3]);
  const requests: { path: string; auth: string | null; type: string | null; bytes: number[] }[] = [];
  const files = new Map<string, Uint8Array>();
  const server = Bun.serve({hostname: '127.0.0.1', port: 0, async fetch(request) {
    const url = new URL(request.url);
    requests.push({path: url.pathname + url.search, auth: request.headers.get('authorization'), type: request.headers.get('content-type'), bytes: [...new Uint8Array(await request.arrayBuffer())]});
    if (url.pathname === '/assets/hidden') return Response.json({denied: {code: 'E_ASSET_UNKNOWN', message: 'Hidden'}}, {status: 404});
    if (request.method === 'POST') return Response.json({asset: {id: 'asset-1', type: 'application/pdf', bytes: 3, name: 'review notes.pdf'}});
    return new Response(bytes, {headers: {'content-type': 'application/pdf', 'content-disposition': "attachment; filename*=UTF-8''review%20notes.pdf"}});
  }});
  const origin = `http://127.0.0.1:${server.port}/`;
  const db = new Snapback({origin, headers: () => ({authorization: 'Bearer session', unused: undefined}), storage: {fs: {async writeFile(path, bytes) { files.set(path, bytes); }}}});
  try {
    const uploaded = await db.upload(bytes.subarray(1), {file: {type: 'application/pdf', name: 'review notes.pdf'}});
    expect(uploaded).toEqual({ok: true, asset: {id: 'asset-1', type: 'application/pdf', bytes: 3, name: 'review notes.pdf'}});
    expect(requests[0]).toEqual({path: '/assets?as=file&name=review%20notes.pdf', auth: 'Bearer session', type: 'application/pdf', bytes: [2, 3]});
    const downloaded = await db.download('asset-1');
    expect(downloaded).toEqual({ok: true, path: 'app:/cache/snapback4-online-asset-1-review notes.pdf', type: 'application/pdf', bytes: 3, name: 'review notes.pdf'});
    expect([...files.values()][0]).toEqual(bytes);
    expect(await db.download('asset-1', 'app:/tmp/once.pdf')).toMatchObject({ok: true, path: 'app:/tmp/once.pdf'});
    expect(await db.download('hidden')).toEqual({ok: false, why: {code: 'E_ASSET_UNKNOWN', message: 'Hidden'}});
    expect(requests.every(r => r.auth === 'Bearer session')).toBe(true);
    const missingStorage = new Snapback({origin});
    expect(await missingStorage.download('asset-1')).toMatchObject({ok: false, why: {code: 'E_CLIENT'}});
  } finally { server.stop(true); }
  expect(await db.upload(bytes)).toMatchObject({ok: false, why: {code: 'E_OFFLINE'}});
  expect(await db.download('asset-1')).toMatchObject({ok: false, why: {code: 'E_OFFLINE'}});
});
