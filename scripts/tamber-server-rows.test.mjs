// LLP 1121 stage 9 D2 on the wire: held proposals of `set-row`s on styles
// with no `file` (scripts/tamber-server.mjs's base and guards).
import { expect, test } from 'bun:test';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { developmentGate } from './install-page.mjs';
import { tamberServer } from './tamber-server.mjs';

// Review round 3: two `set-row`s, one already as asked, are accepted: the
// source left as it was is guarded by the plan's digest, not dropped from
// the base; edited by hand meanwhile, the proposal is planned again on the
// fresh read (an inferred guard is never carried as the caller's).
test('a held set-row that leaves one source as it was is accepted, and planned again after a hand edit', async () => {
  const repo = mkdtempSync(join(tmpdir(), 'exact-tamber-setrow-'));
  const POST = 'style Post\n  background-color="#0a7aff"\n\ncomponent PostButton\n  view\n    column class=Post\n      text "Post"\n';
  const APP = 'use PostButton from "./post.contract"\n\nstyle Card\n  background-color="#ffffff"\n\ncomponent App\n  view\n    column class=Card\n      PostButton()\n';
  writeFileSync(join(repo, 'post.contract'), POST);
  writeFileSync(join(repo, 'app.contract'), APP);
  writeFileSync(join(repo, 'app.tamber'), 'tamber t\n\ntheme\n  ink = #000000\n');
  const listener = createServer();
  await new Promise(ok => listener.listen(0, '127.0.0.1', ok));
  const port = listener.address().port;
  const gate = developmentGate([{ origin: `http://127.0.0.1:${port}` }], port);
  const wire = tamberServer({ appDir: repo, log: () => {}, settle: 250 });
  listener.on('request', async (req, res) => {
    if (!(await wire.handle(req, res, new URL(req.url, 'http://x'), gate.check(req)))) { res.writeHead(404); res.end(); }
  });
  const at = `http://127.0.0.1:${port}`;
  let token;
  const call = async request => {
    const res = await fetch(`${at}/__tamber`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: JSON.stringify(request) });
    return { status: res.status, ...(await res.json()) };
  };
  const ops = color => [
    { op: 'set-row', style: 'Post', row: 'background-color', value: color },
    { op: 'set-row', style: 'Card', row: 'background-color', value: '#ffffff' },
  ];
  try {
    await wire.ready;
    ({ token } = await (await fetch(`${at}/__tamber/token`)).json());
    const held = await call({ method: 'propose', hold: true, client: 'agent', ops: ops('#0b7bff') });
    expect(held.status, JSON.stringify(held)).toBe(200);
    const taken = await call({ method: 'accept', client: 'person', proposal: held.ok.proposal });
    expect(taken.status, JSON.stringify(taken)).toBe(200);
    expect(readFileSync(join(repo, 'post.contract'), 'utf8')).toContain('#0b7bff');
    expect(readFileSync(join(repo, 'app.contract'), 'utf8')).toBe(APP);
    // Held again; the source it leaves as it was edited by hand meanwhile.
    const again = await call({ method: 'propose', hold: true, client: 'agent', ops: ops('#0c7cff') });
    expect(again.status, JSON.stringify(again)).toBe(200);
    writeFileSync(join(repo, 'app.contract'), `${APP}// moved\n`);
    await Bun.sleep(400);
    const replanned = await call({ method: 'accept', client: 'person', proposal: again.ok.proposal });
    expect(replanned.status, JSON.stringify(replanned)).toBe(200);
    expect(replanned.ok.recomputed, JSON.stringify(replanned)).toBe(true);
    const done = await call({ method: 'accept', client: 'person', proposal: replanned.ok.proposal });
    expect(done.status, JSON.stringify(done)).toBe(200);
    expect(readFileSync(join(repo, 'post.contract'), 'utf8')).toContain('#0c7cff');
  } finally {
    await wire.close();
    listener.close();
    rmSync(repo, { recursive: true, force: true });
  }
}, 120_000);
