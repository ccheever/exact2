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

// The store is Exact's secret.keep interface, not localStorage owned by the adapter.
function secretStore() {
  const values = new Map<string, string>();
  return {get: (key: string) => values.get(key) ?? null, set: (key: string, value: string) => {values.set(key, value);}, forget: (key: string) => {values.delete(key);}};
}

test('online sessions persist across clients, refresh, refuse invalid credentials and forget offline', async () => {
  let revision = 0;
  const tokens = new Set<string>(), requests: {path: string; auth: string | null; persona: string | null}[] = [];
  const server = Bun.serve({hostname: '127.0.0.1', port: 0, async fetch(request) {
    const path = new URL(request.url).pathname, token = request.headers.get('authorization')?.replace('Bearer ', '');
    requests.push({path, auth: request.headers.get('authorization'), persona: request.headers.get('x-snapback-persona')});
    if (path === '/auth/signup' || path === '/auth/login') {
      const args = await request.json() as {email: string; password: string};
      if (args.password !== 'correct password') return Response.json({denied: {code: 'E_AUTH', message: 'Invalid credentials'}}, {status: 401});
    } else if (!token || !tokens.has(token)) return Response.json({denied: {code: 'E_AUTH', message: 'Expired'}}, {status: 401});
    if (path === '/auth/logout') {tokens.delete(token!); return Response.json({ok: true});}
    if (path.startsWith('/auth/')) {
      if (token) tokens.delete(token);
      const session = {principal: 'user:alice', kind: 'password', token: `token-${++revision}`, expiresAt: 100_000 + revision * 50_000};
      tokens.add(session.token); return Response.json({session});
    }
    return Response.json({data: ['private'], complete: true});
  }});
  const store = secretStore(), origin = `http://127.0.0.1:${server.port}/`;
  const options = () => ({origin, auth: {store, key: 'session', now: 1000}, headers: () => ({Authorization: 'Bearer wrong', 'X-Snapback-Persona': 'alice'})});
  const db = new Snapback(options());
  try {
    await expect(db.read('private', {})).rejects.toMatchObject({code: 'E_AUTH'});
    expect(await db.signup('alice@example.com', 'wrong')).toMatchObject({ok: false, why: {code: 'E_AUTH'}});
    expect(store.get('session')).toBeNull();
    expect(await db.signup('alice@example.com', 'correct password')).toMatchObject({ok: true, session: {principal: 'user:alice'}});
    expect(store.get('session')).not.toBeNull();
    const reopened = new Snapback(options());
    expect(await reopened.restore(1000)).toMatchObject({token: 'token-1'});
    expect(await reopened.read('private', {})).toMatchObject({data: ['private']});
    expect(await reopened.restore(100_000)).toMatchObject({token: 'token-2'});
    expect(db.session?.token).toBe('token-2');
    await expect(db.login('other@example.com', 'correct password')).rejects.toMatchObject({code: 'E_AUTH'});
    expect(await reopened.logout()).toEqual({ok: true});
    expect(db.session).toBeNull();
    expect(await db.login('alice@example.com', 'correct password')).toMatchObject({ok: true});
    tokens.clear();
    expect(await db.read('private', {})).toMatchObject({denied: {code: 'E_AUTH'}});
    expect(db.session).toBeNull();
    expect(await db.login('alice@example.com', 'correct password')).toMatchObject({ok: true});
    expect(requests.every(r => r.persona === null)).toBe(true);
    expect(requests.find(r => r.path === '/q/private')?.auth).toBe('Bearer token-1');
  } finally {server.stop(true);}
  const valid = db.session;
  expect(valid).not.toBeNull();
  expect(await db.refreshSession(1000)).toMatchObject({ok: false, offline: true});
  expect(db.session?.token).toBe(valid!.token);
  expect(await db.logout()).toMatchObject({ok: false, offline: true});
  expect(store.get('session')).toBeNull();
  store.set('session', 'broken json');
  expect(await db.restore(1000)).toBeNull();
  store.set('session', JSON.stringify({...valid, expiresAt: 1000}));
  expect(await db.restore(1000)).toBeNull();
  expect(store.get('session')).toBeNull();
});

test('online requests from an old session cannot publish into a new account', async () => {
  const store = secretStore();
  const first = {principal: 'user:alice', kind: 'password', token: 'alice', expiresAt: 100_000};
  store.set('session', JSON.stringify(first));
  let entered!: () => void, release!: () => void;
  const started = new Promise<void>(ok => {entered = ok;}), held = new Promise<void>(ok => {release = ok;});
  const server = Bun.serve({hostname: '127.0.0.1', port: 0, async fetch(request) {await request.arrayBuffer(); entered(); await held; return Response.json({data: ['alice private'], complete: true});}});
  try {
    const db = new Snapback({origin: `http://127.0.0.1:${server.port}`, auth: {store, key: 'session', now: 1}});
    const reading = db.read('private', {});
    await started;
    store.set('session', JSON.stringify({...first, principal: 'user:bob', token: 'bob'}));
    release();
    await expect(reading).rejects.toMatchObject({code: 'E_STALE'});
    expect(db.session?.principal).toBe('user:bob');
  } finally {release(); server.stop(true);}
});

test('online readAll preserves cuts, refuses later denials, limits and looping cursors', async () => {
  const requests: unknown[] = [];
  const server = Bun.serve({hostname: '127.0.0.1', port: 0, async fetch(request) {
    const {args} = await request.json() as {args: Record<string, unknown>}; requests.push(args);
    if (args.case === 'denial' && args.page) return Response.json({denied: {code: 'HIDDEN', message: 'No longer visible', family: 'application'}});
    if (args.case === 'scalar') return Response.json({data: {}, complete: true});
    if (args.case === 'missing') return Response.json({data: []});
    return Response.json({data: [args.page ? 2 : 1], complete: args.case !== 'cut', cut: args.case === 'cut' ? 'meter' : null,
      next: args.case === 'loop' || !args.page ? 'opaque cursor' : null});
  }});
  try {
    const db = new Snapback({origin: `http://127.0.0.1:${server.port}`});
    const op = {kind: 'query', name: 'pages'} as {kind: 'query'; name: string; __args?: {case: string; page?: Cursor}; __result?: number[]};
    const rows = await db.readAll(op, {case: 'complete'}, 'page');
    expect([...rows]).toEqual([1, 2]); expect(rows.complete).toBe(true); expect(rows.cut).toBeNull();
    expect(requests.slice(0, 2)).toEqual([{case: 'complete'}, {case: 'complete', page: 'opaque cursor'}]);
    const partial = await db.readAll(op, {case: 'cut'}, 'page');
    expect([...partial]).toEqual([1, 2]); expect(partial.complete).toBe(false); expect(partial.cut).toBe('meter');
    const missing = await db.readAll(op, {case: 'missing'}, 'page'); expect(missing.complete).toBe(false);
    await expect(db.readAll(op, {case: 'denial'}, 'page')).rejects.toMatchObject({code: 'HIDDEN', refusal: {family: 'application'}});
    await expect(db.readAll(op, {case: 'loop'}, 'page')).rejects.toMatchObject({code: 'E_SERVER'});
    await expect(db.readAll(op, {case: 'scalar'}, 'page')).rejects.toMatchObject({code: 'E_SERVER'});
    await expect(db.readAll(op, {case: 'complete'}, 'page', 1)).rejects.toMatchObject({code: 'E_CLIENT'});
  } finally {server.stop(true);}
});
