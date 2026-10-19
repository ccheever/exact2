// The design tool's reading of the wire's scopes (apps/design-tool/wire.ts
// NEEDS) is the server's table (scripts/tamber-pairing.mjs METHOD_SCOPE),
// so a paired tool greys and holds back exactly what the server refuses
// (LLP 1122 D12, T6).
import { expect, test } from 'bun:test';
import { METHOD_SCOPE } from './tamber-pairing.mjs';
import { above, doorOf, GONE, NEEDS, needOf, wire } from '../apps/design-tool/wire.ts';
import { canvasFrame, pairingSources } from '../apps/design-tool/pairing.ts';

test('the tool\'s scope table is the server\'s, method for method', () => {
  expect(NEEDS).toEqual({ ...METHOD_SCOPE });
});

test('a paired client holds back a request above its scope, and the local tool\'s methods always', () => {
  expect(above('propose', 'write', 'transaction')).toContain('needs write');
  expect(above('propose', 'propose', 'propose')).toBe('');
  expect(above('write', 'local', 'pair-code')).toContain('local tool\'s alone');
});

test('a pairing goes only to an https door or a tunnel on this machine\'s loopback', () => {
  expect(doorOf('https://192.168.1.20:7743/__tamber/tool/').origin).toBe('https://192.168.1.20:7743');
  expect(doorOf('http://127.0.0.1:7744').origin).toBe('http://127.0.0.1:7744');
  expect(doorOf('http://[::1]:7744').origin).toBe('http://[::1]:7744');
  for (const bad of ['http://192.168.1.20:7743', 'http://example.test', 'ftp://127.0.0.1', 'https://ada:pw@192.168.1.20:7743', 'not a url', 'http://127.0.0.1.example.test']) expect(doorOf(bad).error).not.toBe('');
});

test('what a request needs, as the server reads it: an inspect toggle and a drop of another\'s proposal are write', () => {
  const owner = id => ({ mine: 'remote:ada#1', theirs: 'design-tool:Grace' })[id] ?? null;
  expect(needOf('/__tamber', { method: 'inspect', refresh: true })).toBe('read');
  expect(needOf('/__tamber', { method: 'inspect', on: true })).toBe('write');
  expect(needOf('/__tamber', { method: 'drop', proposal: 'mine', client: 'remote:ada#1' }, owner)).toBe('propose');
  expect(needOf('/__tamber', { method: 'drop', proposal: 'theirs', client: 'remote:ada#1' }, owner)).toBe('write');
  expect(needOf('/__tamber', { method: 'set-secret' })).toBe('local');
  expect(needOf('/__tamber/events', {})).toBe('read');
});

test('the paired client: a token goes only to the server it came from, nothing above its scope is sent, and after a revoke nothing at all', async () => {
  const sent = [], real = globalThis.fetch;
  let revoked = false, slowToken = null;
  globalThis.fetch = async (url, init = {}) => {
    sent.push({ url: String(url), token: init.headers?.['x-exact-tamber-token'] ?? null, body: init.body ?? null });
    if (String(url).endsWith('/__tamber/token')) { if (slowToken) await slowToken; return Response.json({ token: 'local-token' }); }
    if (String(url).endsWith('/__tamber/pair')) return Response.json({ ok: { token: 'paired-token', client: 'remote:ada#1', scope: 'propose', expires: '' } });
    if (revoked) return Response.json({ error: { message: 'gone' } }, { status: 401 });
    return Response.json({ ok: {} });
  };
  try {
    const w = wire('http://127.0.0.1:7740');
    // Local first: this machine's token, to this machine's server; a review bound to it.
    const local = (await w.call('/__tamber', { method: 'hello' })).used;
    expect(sent.at(-1)).toMatchObject({ url: 'http://127.0.0.1:7740/__tamber', token: 'local-token' });
    // (What the tool keeps of it is a handle, never the token.)
    expect(local).not.toContain('local-token');
    // A door that is not https, nor loopback: refused, nothing sent.
    const before = sent.length;
    expect((await w.pairWith('http://192.168.1.20:7743', '7Q2KD-M4X01', 'ada')).ok).toBe(false);
    expect(sent.length).toBe(before);
    // A token asked for before pairing that answers after it is never the one used.
    let let_ = () => {}; slowToken = new Promise(ok => { let_ = ok; });
    const w2 = wire('http://127.0.0.1:7740');
    const asked = w2.call('/__tamber', { method: 'hello' });
    await w2.pairWith('https://door.example.test:7743', '7Q2KD-M4X01', 'ada');
    let_(); slowToken = null;
    await asked;
    expect(sent.at(-1)).toMatchObject({ url: 'https://door.example.test:7743/__tamber', token: 'paired-token' });
    expect(await w.pairWith('https://door.example.test:7743', '7Q2KD-M4X01', 'ada')).toMatchObject({ ok: true, scope: 'propose' });
    const n = sent.length;
    // A write bound to this machine's token (made before pairing) is not sent to the door.
    expect((await w.call('/__tamber', { method: 'propose', ops: [] }, 15000, local)).error).toContain('before the tool paired');
    // Above its scope: held back.
    expect((await w.call('/__tamber', { method: 'transaction', ops: [] })).error).toContain('needs write');
    expect((await w.call('/__tamber', { method: 'pair-code', scope: 'read' })).error).toContain('local tool\'s alone');
    expect(sent.length).toBe(n);
    // Within it: to the door, with the paired token.
    const mine = (await w.call('/__tamber', { method: 'propose', ops: [] })).used;
    expect(sent.at(-1)).toMatchObject({ url: 'https://door.example.test:7743/__tamber', token: 'paired-token' });
    // Revoked: told so, and nothing more is sent, bound or not.
    revoked = true;
    expect((await w.call('/__tamber', { method: 'hello' })).error).toBe(GONE);
    const m = sent.length;
    expect((await w.call('/__tamber', { method: 'hello' })).error).toBe(GONE);
    expect((await w.call('/__tamber', { method: 'propose', ops: [] }, 15000, mine)).error).toBe(GONE);
    expect(sent.length).toBe(m);
    // The code went once, in a body, never in a URL.
    for (const s of sent) expect(s.url).not.toContain('7Q2KD');
  } finally { globalThis.fetch = real; }
});

test('an answer from before a pairing, or from an older one, changes nothing: this machine\'s token read late, an old pairing\'s 401', async () => {
  const real = globalThis.fetch, sent = [];
  let readLate = null, lateRevoke = null, pairs = 0;
  globalThis.fetch = async (url, init = {}) => {
    const at = String(url), token = init.headers?.['x-exact-tamber-token'] ?? null;
    sent.push({ at, token });
    if (at.endsWith('/__tamber/token')) return { ok: true, status: 200, json: async () => { await readLate; return { token: 'local-token' }; } };
    if (at.endsWith('/__tamber/pair')) { pairs += 1; return Response.json({ ok: { token: `paired-${pairs}`, client: 'remote:ada#1', scope: 'propose', expires: '' } }); }
    if (token === 'paired-1' && lateRevoke) { await lateRevoke; return Response.json({ error: { message: 'gone' } }, { status: 401 }); }
    return Response.json({ ok: {} });
  };
  try {
    // This machine's token, its body still being read when the tool pairs: the paired token stays.
    let release = () => {}; readLate = new Promise(ok => { release = ok; });
    const w = wire('http://127.0.0.1:7740');
    const asked = w.call('/__tamber', { method: 'hello' });
    await Bun.sleep(10);
    await w.pairWith('https://door.example.test:7743', 'X', 'ada');
    release(); readLate = null;
    await asked;
    expect(sent.at(-1)).toMatchObject({ at: 'https://door.example.test:7743/__tamber', token: 'paired-1' });
    // A 401 to the first pairing's token, answered after the tool paired again: the new session goes on.
    let revoke = () => {}; lateRevoke = new Promise(ok => { revoke = ok; });
    const old = w.call('/__tamber', { method: 'hello' });
    await Bun.sleep(10);
    await w.pairWith('https://door.example.test:7743', 'Y', 'ada');
    revoke();
    expect((await old).error).toBe(GONE);
    expect((await w.call('/__tamber', { method: 'hello' })).error).toBe('');
    expect(sent.at(-1)).toMatchObject({ token: 'paired-2' });
  } finally { globalThis.fetch = real; }
});

test('a canvas frame only on the paired door\'s host, over https, another port, at /canvas/<client>/; the grant only in its fragment', () => {
  const grant = 'A'.repeat(43), door = 'https://192.168.1.20:7743', client = 'remote:Ada#1', at = '/canvas/remote%3AAda%231/';
  expect(canvasFrame(`https://192.168.1.20:7745${at}`, grant, door, client)).toEqual({ frame: `https://192.168.1.20:7745${at}#g=${grant}`, root: 'https://192.168.1.20:7745/', error: '' });
  for (const url of [
    `http://127.0.0.1:5173${at}`, // the viewer's own dev loop
    `https://127.0.0.1:7745${at}`, // this machine, not the door's host
    `https://evil.example:7745${at}`, // another host
    `https://192.168.1.20:7743${at}`, // the door's own origin: the frame could reach the tool
    `http://192.168.1.20:7745${at}`, // not https
    'https://192.168.1.20:7745/canvas/remote%3AGrace%231/', // another client's
    `https://192.168.1.20:7745${at}?x=1`, `https://192.168.1.20:7745${at}#g=other`, `https://u:p@192.168.1.20:7745${at}`, 'not a url',
  ]) expect([url, canvasFrame(url, grant, door, client).frame]).toEqual([url, '']);
  expect(canvasFrame(`https://192.168.1.20:7745${at}`, 'short', door, client).frame).toBe('');
  expect(canvasFrame(`https://192.168.1.20:7745${at}`, 'A'.repeat(44), door, client).frame).toBe('');
  expect(canvasFrame(`https://192.168.1.20:7745${at}`, crypto.randomUUID(), door, client).frame).toBe('');
  // Through a tunnel (plain http to this machine's loopback) there is no canvas: its host is this machine.
  expect(canvasFrame(`https://127.0.0.1:8443${at}`, grant, 'http://127.0.0.1:7744', client).frame).toBe('');
  // Nor through a TLS door on loopback (a tunnel's far end, say): any other port there may be another service.
  for (const host of ['127.0.0.1', 'localhost', '[::1]']) expect(canvasFrame(`https://${host}:8443${at}`, grant, `https://${host}:7743`, client).frame).toBe('');
  expect(canvasFrame(`https://192.168.1.20:7745${at}`, 'x#y'.repeat(10), door, client).frame).toBe('');
});

test('the canvas entry page\'s word (C2) from the minted canvas\'s origin alone drops the grant from the frame, with no new grant', async () => {
  let mints = 0;
  const grant = 'B'.repeat(43);
  const call = async () => { mints += 1; return { error: '', reply: { ok: { url: 'https://192.168.1.20:7745/canvas/remote%3AAda%231/', grant: mints === 1 ? grant : 'C'.repeat(43), expires: 0 } }, used: 'run-1' }; };
  const src = pairingSources(call, async () => ({ ok: false, error: '', client: '', scope: '' }), () => 'https://192.168.1.20:7743');
  const first = await src.loadCanvas([false, 'remote:Ada#1', 'epoch-1', 0, false]);
  expect(first.frame).toBe(`https://192.168.1.20:7745/canvas/remote%3AAda%231/#g=${grant}`);
  expect((await src.canvasEntered([0, 0])).entered).toBe(false);
  // Another origin, or another message, is not the entry page's word.
  globalThis.dispatchEvent(new MessageEvent('message', { data: { type: 'exact-canvas-entered' }, origin: 'https://evil.example:7745' }));
  globalThis.dispatchEvent(new MessageEvent('message', { data: { type: 'something-else' }, origin: 'https://192.168.1.20:7745' }));
  expect((await src.canvasEntered([0, 1])).entered).toBe(false);
  globalThis.dispatchEvent(new MessageEvent('message', { data: { type: 'exact-canvas-entered' }, origin: 'https://192.168.1.20:7745' }));
  expect((await src.canvasEntered([0, 2])).entered).toBe(true);
  // The same canvas back as its gateway page, the grant gone, no second grant asked.
  const after = await src.loadCanvas([false, 'remote:Ada#1', 'epoch-1', 0, true]);
  expect(after.frame).toBe('https://192.168.1.20:7745/');
  expect(JSON.stringify(after)).not.toContain(grant);
  expect(mints).toBe(1);
  // Opened again (another tick): a fresh grant, not yet entered.
  const again = await src.loadCanvas([false, 'remote:Ada#1', 'epoch-1', 1, true]);
  expect(again.frame).toContain('#g=');
  expect(mints).toBe(2);
  expect((await src.canvasEntered([1, 3])).entered).toBe(false);
});
