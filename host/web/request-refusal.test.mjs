import { test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';
const source = readFileSync(new URL('./glue.js', import.meta.url), 'utf8');
const defer = source.match(/function deferFulfill\(\.\.\.args\) \{[\s\S]*?\n\}/)[0];
const branch = source.match(/case "refuse": \{[^\n]*\}/)[0];
test('admission refusal delivers Refused only after the enclosing batch, with its incarnation', async () => {
  const delivered = [];
  const apply = Function('safelyFulfill', 'encoder', `${defer}; return (op,incarnation)=>{switch(op.op){${branch}}};`)(
    (...args) => delivered.push(args), new TextEncoder());
  apply({ op: 'refuse', ticket: 17, message: 'only HTTP may opt into independent transport' }, 4);
  expect(delivered.length).toBe(0);
  await Promise.resolve();
  expect(delivered.length).toBe(1);
  expect(delivered[0].slice(0, 5)).toEqual([4, 17, 2, 0, '']);
  expect(new TextDecoder().decode(delivered[0][5])).toBe('only HTTP may opt into independent transport');
});

test('ordinary and early fetch refuse redirects before an ungranted POST destination is reached', async () => {
  const { boundedHttpBody } = await import('./http-body.js');
  const moduleSource = readFileSync(new URL('./module-glue.js', import.meta.url), 'utf8');
  const earlySource = moduleSource.slice(moduleSource.indexOf('const early ='), moduleSource.indexOf('export async function baked'));
  let hits = 0;
  const destination = Bun.serve({ port: 0, fetch() { hits++; return new Response('secret'); } });
  const origin = Bun.serve({ port: 0, fetch(req) {
    if (new URL(req.url).pathname === '/direct') return new Response('ok');
    return new Response(null, { status: 307, headers: { Location: `${destination.url}private` } });
  } });
  const inflight = new Set(), controllers = new Set(), results = [];
  const granted = url => new URL(url).origin === origin.url.origin;
  const requestBranch = source.slice(source.indexOf('case "request": {'), source.indexOf('case "command": {'));
  const apply = Function('grants', 'granted', 'deferFulfill', 'safelyFulfill', 'encoder', 'inflight', 'controllers', 'moduleLoader', 'boundedHttpBody',
    `return (op, incarnation) => { switch(op.op) { ${requestBranch} } };`)(
      [`net.fetch ${origin.url.origin}`], granted, (...args) => results.push(args), (...args) => results.push(args),
      new TextEncoder(), inflight, controllers, null, boundedHttpBody);
  try {
    apply({ op: 'request', ticket: 1, method: 'POST', url: `${origin.url}redirect`, headers: {}, body: btoa('private body') }, 1);
    await Promise.all([...inflight]);
    expect(results[0][2]).toBe(1);
    expect(hits).toBe(0);
    apply({ op: 'request', ticket: 2, method: 'GET', url: `${origin.url}direct`, headers: {} }, 1);
    await Promise.all([...inflight]);
    expect(results[1][2]).toBe(0);
    expect(new TextDecoder().decode(results[1][5])).toBe('ok');
    const early = Function(`${earlySource.replace('export function claim', 'function claim')}; return { fetchEarly, claim };`)();
    const url = `${origin.url}redirect`;
    early.fetchEarly({ method: 'GET', url, headers: {} }, `net.fetch ${origin.url.origin}`);
    const promise = early.claim(url, { method: 'GET', headers: {}, redirect: 'error', cache: 'default' });
    expect(promise).not.toBeNull();
    await expect(promise).rejects.toThrow();
    expect(hits).toBe(0);
  } finally { origin.stop(true); destination.stop(true); }
});
