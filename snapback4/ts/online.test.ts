import { expect, test } from 'bun:test';
import { Snapback } from '../online/snapback.ts';

test('online operations use the generated args and result envelopes without a device', async () => {
  const requests: { path: string; body: unknown; persona: string | null; device: string | null }[] = [];
  const server = Bun.serve({ hostname: '127.0.0.1', port: 0, async fetch(request) {
    const path = new URL(request.url).pathname;
    requests.push({path, body: await request.json(), persona: request.headers.get('x-snapback-persona'), device: request.headers.get('x-snapback-device')});
    if (path === '/m/refuse') return Response.json({state: 'failed', why: {code: 'FULL', message: 'No room'}});
    if (path === '/m/broken') return Response.json({data: null});
    return Response.json(path.startsWith('/q/') ? {data: [{body: 'hello'}]} : {state: 'sent', result: {id: 'note-1'}});
  }});
  try {
    const db = new Snapback({origin: `http://127.0.0.1:${server.port}/`, headers: () => ({'x-snapback-persona': 'alice'})});
    const api = {post: {kind: 'mutation', name: 'post'} as {kind: 'mutation'; name: string; __args?: {body: string}; __result?: {id: string}}};
    expect(await db.send(api.post, {body: 'hello'})).toEqual({state: 'sent', result: {id: 'note-1'}});
    expect(await db.read('recent', {})).toEqual({data: [{body: 'hello'}], server: true});
    expect(await db.send('refuse', {})).toEqual({state: 'failed', why: {code: 'FULL', message: 'No room'}});
    await expect(db.send('broken', {})).rejects.toMatchObject({code: 'E_SERVER'});
    expect(requests[0]).toEqual({path: '/m/post', body: {args: {body: 'hello'}}, persona: 'alice', device: null});
    expect(requests).toHaveLength(4);
  } finally { server.stop(true); }
});
