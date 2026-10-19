// LLP 1122 T3's param rung on the server (tamber/1.11): the app's component
// params read by describe, again when its files move, for a new plan and
// after a failed read; a selection names a param row on the param rung, and
// neither the `selection` nor the `selected` event carries an IR token.
import { expect, test } from 'bun:test';
import { PassThrough } from 'node:stream';
import { EventEmitter } from 'node:events';
import { developmentGate } from './install-page.mjs';
import { tamberServer } from './tamber-server.mjs';

/** A stand-in for `contract tamber serve` that answers `hello`. */
function fakeServe() {
  const child = Object.assign(new EventEmitter(), { stdin: new PassThrough(), stdout: new PassThrough(), stderr: new PassThrough(), exitCode: null, signalCode: null });
  child.kill = () => { child.exitCode = 0; child.emit('exit', 0); };
  let buf = '';
  child.stdin.on('data', chunk => {
    buf += chunk; const lines = buf.split('\n'); buf = lines.pop();
    for (const line of lines) child.stdout.write(`${JSON.stringify({ id: JSON.parse(line).id, ok: { files: {} } })}\n`);
  });
  return child;
}

/** A request as the gate and the handler see it, and what it was answered. */
async function ask(tamber, gate, { path, method = 'GET', headers = {}, peer = '127.0.0.1', body = '' }) {
  const req = Object.assign(new PassThrough(), { method, headers, socket: { remoteAddress: peer } });
  let status = 0, text = '';
  const res = { writeHead(s) { status = s; }, write(t) { text += t; }, end(t) { text += t ?? ''; } };
  queueMicrotask(() => req.end(body));
  const handled = await tamber.handle(req, res, new URL(path, 'http://x'), gate.check(req));
  return { handled, status, body: text ? JSON.parse(text) : null };
}

test('the server reads the app\'s component params, again for a new plan or after a failed read, and a selection names a param row on the param rung; neither event carries an IR token (LLP 1122 T3)', async () => {
  const IR = 'param.f1a2b3c4d.Card.fill';
  const components = [{ identity: 'app.contract#Card', name: 'Card', params: [{ name: 'app.contract#Card.fill', short: 'Card.fill', token: IR, default: 'ink', set: null }] }];
  let describes = 0;
  const serve = () => {
    const child = fakeServe();
    child.stdin.removeAllListeners('data');
    let buf = '';
    child.stdin.on('data', chunk => {
      buf += chunk; const lines = buf.split('\n'); buf = lines.pop();
      for (const line of lines) {
        const r = JSON.parse(line);
        // The first describe fails: the next selection's plan reads again.
        const reply = r.method !== 'describe' ? { ok: { files: { 'app.tamber': 'd1' } } }
          : ++describes === 1 ? { error: { message: 'not now' } }
          : { ok: { files: { 'app.tamber': 'd1' }, ir: { tokens: [{ name: 'ink' }], components } } };
        child.stdout.write(`${JSON.stringify({ id: r.id, ...reply })}\n`);
      }
    });
    return child;
  };
  const map = { digest: 'p1', sources: [{ file: 'exact:app', sha256: 'x' }], nodes: [{ file: 'exact:app', line: 3, col: 5, end_col: 9, component: 'Card', chain: [], bindings: [{ row: 'background_color', origin: 'own' }] }] };
  const port = 8803;
  const gate = developmentGate([{ origin: `http://127.0.0.1:${port}` }], port);
  const tamber = tamberServer({ appDir: '/nowhere', spawnServe: serve, log: () => {}, sourceMap: digest => Buffer.from(JSON.stringify({ ...map, digest: digest ?? 'p1' })) });
  await tamber.ready;
  const settled = async () => { for (let i = 0; i < 100 && describes < 1; i++) await Bun.sleep(5); await Bun.sleep(5); };
  await settled();
  const headers = { host: `127.0.0.1:${port}`, 'x-exact-tamber-token': tamber.token };
  await ask(tamber, gate, { path: '/__tamber/events', method: 'POST', headers, body: JSON.stringify({ wait: false, client: 'tool' }) });
  const { nonce } = tamber.channel.open(() => {});
  const report = body => tamber.channel.report(JSON.stringify({ nonce, ...body }));
  report({ kind: 'register', id: 'web-1', host: 'web', capabilities: ['pin', 'inspect'], plan: 'g1', design: 'd1' });
  report({ kind: 'status', scheme: 'light', revision: 0 });
  tamber.channel.inspect('tool', null, { on: true });
  const selected = { node: 2, site: 0, instance: [], planDigest: 'p1', tuple: {}, rows: [{ name: 'background_color', value: '#fff', source: 'authored', token: { names: [IR], text: IR, modes: '', implied: false } }] };
  const events = async () => (await ask(tamber, gate, { path: '/__tamber/events', method: 'POST', headers, body: JSON.stringify({ after: 0, wait: false, client: 'tool' }) })).body.events;
  // The read failed: read-only, said so; this plan reads them again.
  expect(report({ kind: 'selected', selected, plan: 'g1', design: 'd1', revision: 0 }).status).toBe(204);
  expect((await events()).filter(e => e.event === 'selection').at(-1).rows[0].editable).toMatchObject({ rung: null, reason: expect.stringContaining('has not read yet') });
  for (let i = 0; i < 100 && describes < 2; i++) await Bun.sleep(5);
  await Bun.sleep(5);
  expect(report({ kind: 'selected', selected, plan: 'g1', design: 'd1', revision: 0 }).status).toBe(204);
  const all = await events();
  expect(all.filter(e => e.event === 'selection').at(-1).rows[0].editable).toEqual({ rung: 'param', param: 'app.contract#Card.fill', short: 'Card.fill', token: 'ink', modes: '' });
  // The report as published too (`selected`): every one named by the param, or left out.
  expect(all.filter(e => e.event === 'selected').at(-1).selected.rows[0].token.names).toEqual(['app.contract#Card.fill']);
  expect(JSON.stringify(all)).not.toContain(IR);
  // A new plan: read again, its names kept meanwhile but no alias offered.
  const p2 = { ...selected, planDigest: 'p2' };
  expect(report({ kind: 'selected', selected: p2, plan: 'g1', design: 'd1', revision: 0 }).status).toBe(204);
  expect((await events()).filter(e => e.event === 'selection').at(-1).rows[0].editable).toEqual({ rung: 'param', param: 'app.contract#Card.fill', short: 'Card.fill', modes: '' });
  for (let i = 0; i < 100 && describes < 3; i++) await Bun.sleep(5);
  await Bun.sleep(5);
  expect(report({ kind: 'selected', selected: p2, plan: 'g1', design: 'd1', revision: 0 }).status).toBe(204);
  expect((await events()).filter(e => e.event === 'selection').at(-1).rows[0].editable.token).toBe('ink');
  await tamber.close();
});

test('a describe of other files than the read was for is not kept: the param map stays empty and the row read-only (LLP 1122 T3)', async () => {
  const IR = 'param.f1a2b3c4d.Card.fill';
  const components = [{ identity: 'app.contract#Card', name: 'Card', params: [{ name: 'app.contract#Card.fill', short: 'Card.fill', token: IR, default: 'ink', set: null }] }];
  let describes = 0;
  const serve = () => {
    const child = fakeServe();
    child.stdin.removeAllListeners('data');
    let buf = '';
    child.stdin.on('data', chunk => {
      buf += chunk; const lines = buf.split('\n'); buf = lines.pop();
      for (const line of lines) {
        const r = JSON.parse(line);
        // The files moved between hello and describe.
        const reply = r.method !== 'describe' ? { ok: { files: { 'app.tamber': 'd1' } } } : (describes++, { ok: { files: { 'app.tamber': 'd2' }, ir: { tokens: [{ name: 'ink' }], components } } });
        child.stdout.write(`${JSON.stringify({ id: r.id, ...reply })}\n`);
      }
    });
    return child;
  };
  const map = { digest: 'p1', sources: [{ file: 'exact:app', sha256: 'x' }], nodes: [{ file: 'exact:app', line: 3, col: 5, end_col: 9, component: 'Card', chain: [], bindings: [{ row: 'background_color', origin: 'own' }] }] };
  const port = 8804;
  const gate = developmentGate([{ origin: `http://127.0.0.1:${port}` }], port);
  const tamber = tamberServer({ appDir: '/nowhere', spawnServe: serve, log: () => {}, sourceMap: () => Buffer.from(JSON.stringify(map)) });
  await tamber.ready;
  for (let i = 0; i < 100 && describes < 1; i++) await Bun.sleep(5);
  await Bun.sleep(5);
  const headers = { host: `127.0.0.1:${port}`, 'x-exact-tamber-token': tamber.token };
  await ask(tamber, gate, { path: '/__tamber/events', method: 'POST', headers, body: JSON.stringify({ wait: false, client: 'tool' }) });
  const { nonce } = tamber.channel.open(() => {});
  const report = body => tamber.channel.report(JSON.stringify({ nonce, ...body }));
  report({ kind: 'register', id: 'web-1', host: 'web', capabilities: ['pin', 'inspect'], plan: 'g1', design: 'd1' });
  report({ kind: 'status', scheme: 'light', revision: 0 });
  tamber.channel.inspect('tool', null, { on: true });
  const selected = { node: 2, site: 0, instance: [], planDigest: 'p1', tuple: {}, rows: [{ name: 'background_color', value: '#fff', source: 'authored', token: { names: [IR], text: IR, modes: '', implied: false } }] };
  expect(report({ kind: 'selected', selected, plan: 'g1', design: 'd1', revision: 0 }).status).toBe(204);
  const events = (await ask(tamber, gate, { path: '/__tamber/events', method: 'POST', headers, body: JSON.stringify({ after: 0, wait: false, client: 'tool' }) })).body.events;
  expect(events.filter(e => e.event === 'selection').at(-1).rows[0].editable).toMatchObject({ rung: null, reason: expect.stringContaining('has not read yet') });
  await tamber.close();
});
