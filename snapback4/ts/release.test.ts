import {expect, test} from 'bun:test';
import {readFileSync} from 'node:fs';
import {Snapback as Online} from '../online/snapback.ts';
import {Snapback as Device} from './snapback.ts';

test('both isolated mounts keep the served-release decoder identical', () => {
  expect(readFileSync(new URL('../online/release.ts', import.meta.url), 'utf8')).toBe(readFileSync(new URL('./release.ts', import.meta.url), 'utf8'));
});
for (const device of [false, true]) test(`${device ? 'device' : 'online'} catalog is authenticated and follows rollback generations without stale replies`, async () => {
  let generation = 2, denied = false, malformed = false;
  const paths: string[] = [], generations: number[] = [];
  const server = Bun.serve({hostname: '127.0.0.1', port: 0, fetch(request) {
    expect(request.headers.get('x-snapback-persona')).toBe('alice');
    paths.push(new URL(request.url).pathname);
    if (denied) return Response.json({denied: {code: 'E_AUTH', message: 'Sign in'}}, {status: 401});
    return Response.json(malformed ? {generation} : {generation, ops: generation === 2 ? [{name: 'newFeature', kind: 'mutation', args: {}}] : [], tables: []}, {headers: {'x-snapback-generation': String(generation)}});
  }});
  const options = {origin: `http://127.0.0.1:${server.port}/`, headers: () => ({'x-snapback-persona': 'alice'})};
  const db = device ? await Device.open({...options, app: 'release-test', name: 'release', viewer: 'dev:alice', native: {call: () => ({ok: {opened: true}})}}) : new Online(options);
  try {
    const unsubscribe = db.subscribeGeneration(g => generations.push(g));
    const current = await db.release();
    expect('generation' in current && !!current.operations.newFeature).toBe(true);
    generation = 3; // Rollback has a newer generation but removes the operation.
    const rollback = await db.release();
    expect('generation' in rollback && !!rollback.operations.newFeature).toBe(false);
    expect(generations).toEqual([3]);
    generation = 2;
    expect(await db.release()).toEqual({loading: true});
    denied = true;
    expect(await db.release()).toMatchObject({denied: {code: 'E_AUTH'}});
    denied = false; malformed = true; generation = 4;
    expect(await db.release()).toEqual({loading: true});
    unsubscribe();
    generation = 5; await db.release();
    expect(generations).toEqual([3, 4]);
    expect(paths.every(path => path === '/')).toBe(true);
  } finally { if (db instanceof Device) await db.close(); server.stop(true); }
  if (!device) expect(await db.release()).toEqual({loading: true});
});
