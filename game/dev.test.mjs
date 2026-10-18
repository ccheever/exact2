import {test, expect} from 'bun:test';
import {mkdtempSync, mkdirSync} from 'node:fs';
import {resolve} from 'node:path';
import {buildingServer, devPort} from './dev.mjs';
import {readStatus, startStatus, waitReady} from './status.mjs';

test('HTTP answers during a wedged build; readiness belongs to the current invocation and generation', async () => {
  mkdirSync(resolve(import.meta.dir, '../target'), {recursive:true});
  const app = mkdtempSync(resolve(import.meta.dir, '../target/dev-test-'));
  const {port} = await devPort([], {start:29301});
  const early = await buildingServer({app, host:'127.0.0.1', port, origins:[{origin:`http://127.0.0.1:${port}`}], gate:{check:() => ({allowed:true})}});
  try {
    early.status.set({phase:'build'});
    startStatus(app, 'prove');
    expect(readStatus(app).map(r => r.verb).sort()).toEqual(['prove','web']);
    expect(readStatus(app)[0].state).toBe('running');
    expect((await fetch(`http://127.0.0.1:${port}/`)).status).toBe(503);
    await expect(waitReady(app, {timeout:0.1})).rejects.toThrow('timed out');
    await expect(devPort(['--port', String(port)])).rejects.toThrow('in use');
    early.serve((req, res) => res.end('game'));
    expect((await waitReady(app, {timeout:1})).invocation).toBe(early.status.get().invocation);
    early.status.set({phase:'failed'});
    await expect(waitReady(app, {timeout:1})).rejects.toThrow('build failed');
    early.status.set({state:'exited', exitCode:0});
    await expect(waitReady(app, {timeout:1})).rejects.toThrow('exited');
  } finally { early.server.closeAllConnections(); early.server.close(); /* exit handlers still write status */ }
});
