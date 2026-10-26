// The design read again after a peer's write (the third pass's #5) against a real `tamber serve`: a write that names
// its tokens (an agent's set-token) is read again as describe, a resolve for each token it moved and each whose
// entry moved (a package's token derived from the one the app's line set, which the event, of the app's file, does
// not name), and hello, its rows shown with every other row as the last whole read had it; a hand edit, which names
// no tokens, and a write that adds a token each read whole, each in one `resolve` (tamber/1.17's `tokens`; one per
// token took the clone's 267 3.6 s, at the one server behind each other).
import { expect, onTestFinished, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { DRIVEN, ROOT, drive, lines, proxied, signals, toolTurns } from './design-tool-harness.mjs';
import { spawnOwned } from './test-children.mjs';

toolTurns();

const FILLERS = ['a', 'b', 'c', 'd', 'e', 'f'].map(f => `filler.${f}`);

test('a peer\'s write is read again as only the tokens it moved, a package\'s derived token among them; a hand edit and an added token each read whole', async () => {
  const app = mkdtempSync(join(tmpdir(), 'exact-design-tool-reread-'));
  cpSync(resolve(ROOT, 'apps/theme-fixture/app.json'), join(app, 'app.json'));
  // brand, a token of the app's derived from it, and the package's reading.card set over to it, whose
  // reading.edge, the package's own line, derives from reading.card.
  const design = `tamber t\n\ntheme\n  brand = #336699\n  accent = brand\n  reading.card = brand\n${FILLERS.map((f, i) => `  ${f} = #10101${i}\n`).join('')}`;
  writeFileSync(join(app, 'app.tamber'), design);
  const pkg = join(app, 'node_modules/@acme/reading');
  mkdirSync(pkg, { recursive: true });
  writeFileSync(join(pkg, 'package.json'), '{"name":"@acme/reading","version":"1.0.0","exports":{".":"./index.contract"}}');
  writeFileSync(join(pkg, 'index.contract'), 'component ReadingCard\n  view\n    column background-color="reading.card" border-color="reading.edge" border-width=1\n      text "x"\n');
  writeFileSync(join(pkg, 'reading.tamber'), 'tamber reading\n\ntheme\n  reading.card = #112233\n  reading.edge = reading.card\n');
  writeFileSync(join(app, 'app.contract'), `use ReadingCard from "@acme/reading"\ncomponent App\n  view\n    column background-color="accent" color="brand"\n${FILLERS.map(f => `      text "${f}" color="${f}"\n`).join('')}      ReadingCard()\n`);
  const git = (...args) => spawnSync('git', ['-c', 'user.email=t@example.test', '-c', 'user.name=t', ...args], { cwd: app });
  git('init', '-q'); git('add', 'app.json', 'app.tamber', 'app.contract'); git('commit', '-q', '-m', 'design');
  // Each request the tool sends the server, in order (the long poll's GET has no method).
  const asked = [];
  const tool = await proxied(app, request => { if (request.method) asked.push({ method: request.method, tokens: request.tokens ?? [request.token] }); });
  const said = signals('reread');
  onTestFinished(said.drop);
  try {
    const token = (await (await fetch(`${tool.origin}/__tamber/token`)).json()).token;
    const api = async body => (await (await fetch(`${tool.origin}/__tamber`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: JSON.stringify(body) })).json());
    const all = (await api({ method: 'describe' })).ok.ir.tokens.map(t => t.name);
    expect(all).toEqual(expect.arrayContaining(['brand', 'accent', 'reading.card', 'reading.edge', ...FILLERS]));
    const settle = ['clock held changes 30s', 'clock data', 'clock +1', 'clock data', 'clock +1', 'clock data', 'tree'];
    const ops = ['type name Ada', 'tap save-name', 'clock data', 'clock +1', 'clock data', 'clock +1', 'clock data', 'tree',
      said.pause(120_000, 1), ...settle, said.pause(120_000, 2), ...settle, said.pause(120_000, 3), ...settle];
    const drive = spawnOwned(process.execPath, [resolve(ROOT, 'scripts/agent.mjs'), 'web', ...DRIVEN.args, '--url', tool.url, '--storage', `design-tool-reread-${process.pid}`, '--json', ...ops], { cwd: ROOT, env: DRIVEN.env, stdio: ['ignore', 'pipe', 'pipe'] });
    let driveSaid = '';
    drive.stderr.on('data', d => { driveSaid = (driveSaid + d).slice(-4000); });
    const out = lines(drive), replies = () => out.seen.filter(l => l.startsWith('{'));
    // Before each pause's reply: the requests so far marked, then the write, then the signal.
    const marks = [];
    const write = async (n, act) => {
      await out.until(() => replies().length >= 8 + (n - 1) * (settle.length + 1), 600_000).catch(e => { throw new Error(`${e.message}\n${driveSaid}`); });
      marks.push(asked.length);
      await act();
      said.fire(n);
    };
    const peer = async op => {
      const files = (await api({ method: 'hello' })).ok.files;
      const r = await api({ method: 'transaction', client: 'agent:peer', files, ops: [op] });
      expect(r.error).toBeUndefined();
    };
    // 1: the peer sets brand; 2: a hand edit of a filler; 3: the peer adds a token.
    await write(1, () => peer({ op: 'set-token', name: 'brand', when: [], value: '#cc3300' }));
    await write(2, () => writeFileSync(join(app, 'app.tamber'), readFileSync(join(app, 'app.tamber'), 'utf8').replace('filler.a = #101010', 'filler.a = #202020')));
    await write(3, () => peer({ op: 'add-token', name: 'filler.new', value: '#303030' }));
    await new Promise(ok => drive.once('exit', ok));
    expect(drive.exitCode, driveSaid).toBe(0);
    marks.push(asked.length);
    const trees = replies().map(l => JSON.parse(l)).filter(r => r.nodes);
    const text = (t, id) => t.nodes.find(n => n.props.testId === id)?.props.text;
    const cell = (t, name) => text(t, `cell-text-${name}-0-light`);
    const [first, peered, hand, added] = trees;
    const between = n => asked.slice(marks[n - 1], marks[n]);
    const resolved = n => between(n).filter(r => r.method === 'resolve').flatMap(r => r.tokens).sort();
    const count = (n, method) => between(n).filter(r => r.method === method).length;
    // Read whole first: brand's colour through the app's line into the package's derived token.
    expect(['brand', 'accent', 'reading.card', 'reading.edge'].map(t => cell(first, t))).toEqual(Array(4).fill('#336699ff'));
    // The peer's write: describe, the four tokens it moved (the package's derived one by its entry), hello; every row shown.
    expect(resolved(1)).toEqual(['accent', 'brand', 'reading.card', 'reading.edge']);
    // One describe, one resolve of them all (tamber/1.17's `tokens`), and its hello.
    expect(count(1, 'describe')).toBe(1);
    expect(count(1, 'resolve')).toBe(1);
    expect(count(1, 'hello')).toBeGreaterThanOrEqual(1);
    expect(['brand', 'accent', 'reading.card', 'reading.edge'].map(t => cell(peered, t))).toEqual(Array(4).fill('#cc3300ff'));
    for (const f of FILLERS) expect(cell(peered, f)).toBe(cell(first, f));
    // The hand edit names no tokens: read whole, the filler's new value shown.
    expect(resolved(2)).toEqual([...all].sort());
    expect(count(2, 'resolve')).toBe(1);
    expect(cell(hand, 'filler.a')).toBe('#202020ff');
    expect(cell(hand, 'brand')).toBe('#cc3300ff');
    // A token added: read whole, the new one among them.
    expect(resolved(3)).toEqual([...all, 'filler.new'].sort());
    expect(cell(added, 'filler.new')).toBe('#303030ff');
  } finally {
    await tool.close();
    rmSync(app, { recursive: true, force: true });
  }
}, 900_000);

test('a batch whose reply passes 1 MiB (a stream\'s cap, not a fetch\'s) is read whole: each read one resolve, every row shown', async () => {
  const app = mkdtempSync(join(tmpdir(), 'exact-design-tool-batch-big-'));
  cpSync(resolve(ROOT, 'apps/theme-fixture/app.json'), join(app, 'app.json'));
  // 180 families near the value bound (about 3.9 KB each, twice that in an answer): describe under 1 MiB, the batch's answers past it.
  const stack = n => Array.from({ length: 550 }, (_, i) => `f${n}x${i}`).join(', ').slice(0, 3900).replace(/, [^,]*$/, '');
  const faces = Array.from({ length: 180 }, (_, n) => `face${n}`);
  writeFileSync(join(app, 'app.tamber'), `tamber t\n\ntheme\n  ink = #111111\n${faces.map((f, n) => `  ${f}: family = ${stack(n)}\n`).join('')}`);
  writeFileSync(join(app, 'app.contract'), `component App\n  view\n    column background-color="ink"\n${faces.map(f => `      text "x" font-family="${f}"\n`).join('')}`);
  const git = (...args) => spawnSync('git', ['-c', 'user.email=t@example.test', '-c', 'user.name=t', ...args], { cwd: app });
  git('init', '-q'); git('add', '.'); git('commit', '-q', '-m', 'design');
  const asked = [], sizes = [];
  const tool = await proxied(app, request => { if (request.method) asked.push({ method: request.method, tokens: request.tokens ?? [request.token] }); });
  try {
    const token = (await (await fetch(`${tool.origin}/__tamber/token`)).json()).token;
    const reply = await (await fetch(`${tool.origin}/__tamber`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: JSON.stringify({ method: 'resolve', tokens: ['ink', ...faces] }) })).text();
    sizes.push(reply.length);
    expect(reply.length).toBeGreaterThan(1 << 20);
    // (Two cells' subtrees: the whole tree of 180 rows of 4 KB stacks is past what a reply prints.)
    const replies = await drive(tool.url, `design-tool-batch-big-${process.pid}`, ['type name Ada', 'tap save-name', 'clock data', 'clock +1', 'clock data', 'clock +1', 'clock data', 'tree cell-face179-0-light', 'tree cell-ink-0-light']);
    const [face, ink] = replies.filter(r => r.nodes).map(t => t.nodes.map(n => n.props.text).filter(Boolean).join(' '));
    expect(face).toContain('f179x0');
    expect(ink).toContain('#111111ff');
    // Each read one resolve of them all, never by name (a reply the fetch refused would fail the read).
    const resolves = asked.filter(r => r.method === 'resolve');
    expect(resolves.length).toBeGreaterThan(0);
    expect(resolves.every(r => r.tokens.length === 181)).toBe(true);
  } finally {
    await tool.close();
    rmSync(app, { recursive: true, force: true });
  }
}, 900_000);

test('what a read again resolves: the tokens each event since the kept read named and each whose entry moved; an event that named none, a gap, another run, another outline or params, an unknown token or a dropped event is a full read', async () => {
  // (A module of this test's own: its event log starts empty.)
  const { again, heard, keep } = await import(`../apps/design-tool/reread.ts?unit=${Date.now()}`);
  const tok = (name, value, more = {}) => ({ name, kind: 'color', values: [value], entries: null, ...more });
  // Each token's resolve as the server gives it at describe's revision: its one tuple's value the entry's.
  const answersOf = tokens => tokens.map(t => ({ tuples: [{ value: t.values[0] }] }));
  const described = (tokens, components = []) => ({ files: { 'app.tamber': 'r' }, packages: [], ir: { axes: [], tuples: 1, tokens, components } });
  const base = [tok('brand', '#111'), tok('accent', '#111'), tok('edge', '#111', { file: 'p.tamber', package: 'p' }), tok('other', '#222')];
  const kept = keep('e1', 5, described(base), answersOf(base));
  const moved = [tok('brand', '#333'), tok('accent', '#333'), tok('edge', '#333', { file: 'p.tamber', package: 'p' }), tok('other', '#222')];
  const event = (seq, changed, epoch = 'e1') => heard({ event: 'changed', epoch, seq, changed: changed && changed.map(token => ({ file: 'app.tamber', token })) });
  // Before and at the kept read's seq, and another run's: not this read's. After it: brand, accent; edge by its entry.
  event(4, null); event(5, ['other']); event(9, null, 'e0'); event(6, ['brand']); event(7, ['accent']); heard({ event: 'committed', seq: 8 });
  expect([...again(kept, 'e1', described(moved), [])].sort()).toEqual(['accent', 'brand', 'edge']);
  // An answer not of describe's revision (another value, or a tuple short) keeps no entry: resolved again next.
  const torn = keep('e1', 7, described(base), [{ tuples: [{ value: '#999' }] }, { tuples: [] }, ...answersOf(base.slice(2))]);
  expect([...again(torn, 'e1', described(base), [])].sort()).toEqual(['accent', 'brand']);
  // A token named as a param is (the param is read first, its default a package's source moves with no event): every time.
  const card = [{ identity: '@acme/x/card.contract#Card', params: [{ name: '@acme/x/card.contract#Card.fill', short: 'other', token: 'param.y.Card.fill' }] }];
  expect([...again(keep('e1', 7, described(base, card), answersOf(base)), 'e1', described(base, card), [])]).toEqual(['other']);
  // Nothing since and nothing moved: nothing.
  expect([...again(keep('e1', 7, described(base), answersOf(base)), 'e1', described(base), [])]).toEqual([]);
  // A full read for: a gap since, another run, no kept read, another outline (a token added, another kind, a param added).
  expect(again(kept, 'e1', described(moved), [{ seq: 7, gap: true }], )).toBeNull();
  expect([...again(kept, 'e1', described(moved), [{ seq: 5, gap: true }])].sort()).toEqual(['accent', 'brand', 'edge']);
  expect(again(kept, 'e2', described(moved), [])).toBeNull();
  expect(again(undefined, 'e1', described(moved), [])).toBeNull();
  expect(again(kept, 'e1', described([...moved, tok('new', '#444')]), [])).toBeNull();
  expect(again(kept, 'e1', described(moved.map(t => t.name === 'other' ? { ...t, kind: 'length' } : t)), [])).toBeNull();
  expect(again(kept, 'e1', described(moved, [{ identity: 'app.contract#Card', params: [{ name: 'app.contract#Card.fill', short: 'Card.fill', token: 'param.x.Card.fill' }] }]), [])).toBeNull();
  // An event since that named a token the kept read has not, then one that named none (a hand edit).
  event(10, ['gone']);
  expect(again(kept, 'e1', described(moved), [])).toBeNull();
  expect([...again(keep('e1', 10, described(base), answersOf(base)), 'e1', described(moved), [])].sort()).toEqual(['accent', 'brand', 'edge']);
  event(11, null);
  expect(again(keep('e1', 10, described(base), answersOf(base)), 'e1', described(moved), [])).toBeNull();
  // Past the newest 1,000 events, a read kept before the oldest is read whole.
  for (let seq = 12; seq < 1013; seq++) event(seq, ['brand']);
  expect(again(keep('e1', 11, described(base), answersOf(base)), 'e1', described(moved), [])).toBeNull();
  expect([...again(keep('e1', 12, described(base), answersOf(base)), 'e1', described(moved), [])].sort()).toEqual(['accent', 'brand', 'edge']);
});

test('resolveAll: one resolve with tokens; asked again by name when a server answers without answers; never after one that reached no server, nor once overtaken; a timeout by name stops the rest', async () => {
  const { resolveAll } = await import(`../apps/design-tool/reread.ts?all=${Date.now()}`);
  const ok = token => ({ error: '', reply: { ok: { token, tuples: [], files: { a: '1' } } } });
  const sent = [];
  // A server with `tokens`: one request; a refused name in place.
  const batch = async (_, body) => { sent.push(body); return { error: '', reply: { ok: { files: { a: '1' }, answers: body.tokens.map(t => t === 'no' ? { error: { message: 'no token `no`' } } : { token: t, tuples: [] }) } } }; };
  const got = await resolveAll(batch, ['x', 'no', 'y'], {}, 1000, () => true);
  expect(sent).toEqual([{ method: 'resolve', tokens: ['x', 'no', 'y'] }]);
  expect(got.map(r => [r.error, r.reply.ok?.token ?? null, r.reply.ok?.files ?? null])).toEqual([['', 'x', { a: '1' }], ['no token `no`', null, null], ['', 'y', { a: '1' }]]);
  // A server before 1.17 refuses `tokens` (no `token`): each by name, with the file asked.
  sent.length = 0;
  const old = async (_, body) => { sent.push(body); return body.tokens ? { error: 'refused', reply: { error: { message: '`token` is a string, and needed' } } } : ok(body.token); };
  expect((await resolveAll(old, ['x', 'y'], { file: 'p.tamber' }, 1000, () => true)).map(r => r.reply.ok.token)).toEqual(['x', 'y']);
  expect(sent.map(b => b.token ?? 'batch')).toEqual(['batch', 'x', 'y']);
  expect(sent.every(b => b.file === 'p.tamber')).toBe(true);
  // A batch that reached no server: each name's answer, nothing asked again.
  sent.length = 0;
  const gone = async (_, body) => { sent.push(body); return { error: 'no server', reply: {} }; };
  expect((await resolveAll(gone, ['x', 'y'], {}, 1000, () => true)).map(r => r.error)).toEqual(['no server', 'no server']);
  expect(sent.length).toBe(1);
  // Past 1,024 names, an ask each 1,024; a batch whose answers pass the server's bytes, asked again in halves.
  sent.length = 0;
  const many = Array.from({ length: 2500 }, (_, i) => `t${i}`);
  const r2500 = await resolveAll(batch, many, {}, 1000, () => true);
  expect(sent.map(b => b.tokens.length)).toEqual([1024, 1024, 452]);
  expect(r2500.map(r => r.reply.ok.token)).toEqual(many);
  sent.length = 0;
  const heavy = async (_, body) => { sent.push(body); return body.tokens.length > 2 ? { error: 'refused', reply: { error: { message: 'tamber-resolve-batch: one resolve answers at most 16 MiB', id: 'tamber-resolve-batch' } } } : batch(_, body).then(r => (sent.pop(), r)); };
  const r5 = await resolveAll(heavy, ['a', 'b', 'c', 'd', 'e'], {}, 1000, () => true);
  expect(r5.map(r => r.reply.ok.token)).toEqual(['a', 'b', 'c', 'd', 'e']);
  expect(sent.map(b => b.tokens.length)).toEqual([5, 3, 2, 1, 2]);
  // The gateway's 504 while the batch still runs (or any other refusal): each name's answer, nothing asked again.
  sent.length = 0;
  const late = async (_, body) => { sent.push(body); return { error: 'the design server did not answer', reply: { error: { message: 'the design server did not answer' } } }; };
  expect((await resolveAll(late, ['x', 'y'], {}, 1000, () => true)).map(r => r.error)).toEqual(['the design server did not answer', 'the design server did not answer']);
  expect(sent.length).toBe(1);
  // Overtaken before it starts: nothing sent.
  sent.length = 0;
  expect(await resolveAll(gone, ['x'], {}, 1000, () => false)).toEqual([]);
  expect(sent.length).toBe(0);
  // By name, a timeout stops the rest: each name not asked answers with it.
  sent.length = 0;
  const slow = async (_, body) => { sent.push(body); return body.tokens ? { error: 'refused', reply: { error: { message: '`token` is a string, and needed' } } } : body.token === 'b' ? { error: 'timed out', reply: {} } : ok(body.token); };
  const names = ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j'];
  const r = await resolveAll(slow, names, {}, 1000, () => true);
  expect(r.length).toBe(names.length);
  expect(r[1].error).toBe('timed out');
  expect(sent.length).toBeLessThan(names.length + 1);
  expect(r.slice(sent.length - 1).every(x => x.error === 'timed out')).toBe(true);
});
