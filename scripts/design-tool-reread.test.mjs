// The design read again after a peer's write (the third pass's #5) against a real `tamber serve`: a write that names
// its tokens (an agent's set-token) is read again as describe, a resolve for each token it moved and each whose
// entry moved (a package's token derived from the one the app's line set, which the event, of the app's file, does
// not name), and hello, its rows shown with every other row as the last whole read had it; a hand edit, which names
// no tokens, and a write that adds a token each read whole. A full read is a resolve per token, at the one server
// behind each other (the clone's 267 took 3.6 s).
import { expect, onTestFinished, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { DRIVEN, ROOT, lines, proxied, signals, toolTurns } from './design-tool-harness.mjs';
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
  const tool = await proxied(app, request => { if (request.method) asked.push({ method: request.method, token: request.token }); });
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
    const resolved = n => between(n).filter(r => r.method === 'resolve').map(r => r.token).sort();
    const count = (n, method) => between(n).filter(r => r.method === method).length;
    // Read whole first: brand's colour through the app's line into the package's derived token.
    expect(['brand', 'accent', 'reading.card', 'reading.edge'].map(t => cell(first, t))).toEqual(Array(4).fill('#336699ff'));
    // The peer's write: describe, the four tokens it moved (the package's derived one by its entry), hello; every row shown.
    expect(resolved(1)).toEqual(['accent', 'brand', 'reading.card', 'reading.edge']);
    expect(count(1, 'describe')).toBe(1);
    expect(count(1, 'hello')).toBeGreaterThanOrEqual(1);
    expect(['brand', 'accent', 'reading.card', 'reading.edge'].map(t => cell(peered, t))).toEqual(Array(4).fill('#cc3300ff'));
    for (const f of FILLERS) expect(cell(peered, f)).toBe(cell(first, f));
    // The hand edit names no tokens: read whole, the filler's new value shown.
    expect(resolved(2)).toEqual([...all].sort());
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
