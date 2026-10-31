// The JS target's decision to scroll to a fragment (host/web-js/router.js; LLP 1153 D5): a navigation counts only when a
// commit that lands assigns its very value to the router, and the scroll waits for every request in flight, a mutation's
// too, as the runner's `fragment_command` waits on `has_pending`. rt.js and router.js run beside stand-ins for the
// modules they import, and a fragment.js that records what it is asked.
import { test, expect } from 'bun:test';
import { copyFileSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { standIns } from './stand-ins.mjs';

const dir = mkdtempSync(resolve(tmpdir(), 'exact-js-fragment-'));
const REAL = ['rt.js', 'held.js', 'font-family.js', 'observers.js', 'grid.js', 'overlay.js', 'roster.js', 'router.js', 'route-pattern.js', 'schedule.js', 'budget.js', 'elements.js'];
for (const f of REAL) copyFileSync(resolve(new URL(`../../web-js/${f}`, import.meta.url).pathname), resolve(dir, f));
for (const [file, source] of standIns(REAL)) writeFileSync(resolve(dir, file), source);
writeFileSync(resolve(dir, 'fragment.js'), `const asked = globalThis.fragmentAsked;
export const arm = (root, op) => asked.push(['arm', op.fragment, op.activation]);
export const scroll = (root, visit, fragment, log, activation) => asked.push(['scroll', fragment, activation]);
export const cancel = () => asked.push(['cancel']);`);

const turn = () => new Promise(r => setTimeout(r, 0));
const until = async ok => { for (let i = 0; i < 500 && !ok(); i++) await turn(); };

test('a fragment scrolls for a navigation a landed commit assigns the router, once its requests are answered', async () => {
  const page = globalThis.document;
  globalThis.document = { addEventListener() {}, getElementById: () => null, querySelector: () => null };
  const asked = globalThis.fragmentAsked = [], applied = [];
  try {
    const rt = await import(resolve(dir, 'rt.js'));
    await import(resolve(dir, 'fragment.js')); // loaded, so router.js's own import of it answers in a turn
    rt.routes([['home', '/', null, true, false], ['item', '/item/:id', 0, false, false]]);
    const nav = rt.sig(rt.launch('/item/7#moves'), null, 'nav');
    rt.router(nav, { apply: op => applied.push(op), project() {}, personScrolls: 0 });
    await until(() => asked.length >= 2);
    expect(asked).toEqual([['arm', 'moves', 1], ['scroll', 'moves', 1]]); // the launch's
    const moves = () => rt.x_go(nav(), '/item/7#moves');

    // A navigation to the fragment shown, in a commit that is then refused, or computed and not assigned: nothing.
    expect(rt.commit(() => { rt.W(nav, moves()); throw new rt.Refusal('the save failed'); }, 'save')).toBe(false);
    rt.commit(() => { moves(); }, 'peek');
    await turn();
    expect([asked.length, applied.length]).toEqual([2, 1]);

    // Assigned, it is a change of its own to the history, and scrolls once the mutation it sent is answered.
    let answer;
    rt.data.answer = source => source === 'save' ? { promise: new Promise(r => { answer = r; }) } : null;
    const save = rt.mut('save', rt.sig(null, null, 'saved'), [], null);
    rt.commit(() => { rt.W(nav, moves()); rt.M(save, 'save', []); }, 'save');
    await turn();
    expect(applied.at(-1)).toMatchObject({ fragment: 'moves', scroll: true });
    expect(asked.slice(2)).toEqual([['arm', 'moves', 2]]);
    answer('ok');
    await until(() => asked.length >= 4);
    expect(asked.slice(2)).toEqual([['arm', 'moves', 2], ['scroll', 'moves', 2]]);

    // The app's own scrollIntoView ends a fragment's scroll held or yet to come.
    (globalThis.exact ??= {}).fragment = { cancel: () => asked.push(['cancel']) };
    rt.Hosts.scrollIntoView('row-1');
    expect(asked.at(-1)).toEqual(['cancel']);
  } finally { if (page === undefined) delete globalThis.document; else globalThis.document = page; }
});
