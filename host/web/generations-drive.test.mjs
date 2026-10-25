// A drive is served from the build it booted (generations.mjs; theme2's ruling on the build-swap pair): a drive on
// build N loads N's lazy chunk after N+1 is put in place, its reload stays on N, and a drive opened after boots N+1.
// The app: one source whose answer imports a module of the app's own (a lazy chunk, `lazy-<hash>.js`) only once a
// press asks for it, so the chunk is first fetched after the next build lands.
import { expect, test } from 'bun:test';
import { spawn, spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

const ROOT = resolve(new URL('../..', import.meta.url).pathname);

function app(dir, said) {
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, 'app.json'), JSON.stringify({ name: 'Lazy', short_name: 'Lazy', id: 'com.exact.lazy-generation', start_url: '/', display: 'standalone', theme_color: '#ffffff', background_color: '#ffffff', app: { id: 'com.exact.lazy-generation', name: 'Lazy' } }));
  writeFileSync(join(dir, 'app.contract'), `shape Said
  text: string

component App
  state go = 0
  resource said = said(go) as shape Said
  action more
    go = go + 1
  view
    column
      text said.text testId="said"
      button press=more testId="more"
        text "More"
`);
  writeFileSync(join(dir, 'app.ts'), `import type { Answer } from './app.contract.d.ts';
export const appId = 'com.exact.lazy-generation';
export const grants = '';
export const answer: Answer = ((source: string, args: unknown[]) => {
  if (source !== 'said') throw new Error(\`unknown source \${source}\`);
  return args[0] === 0 ? { text: 'not asked' } : import('./lazy.ts').then((m) => ({ text: m.text }));
}) as unknown as Answer;
`);
  writeFileSync(join(dir, 'lazy.ts'), `export const text = ${JSON.stringify(said)};\n`);
}
const build = (dir, dist) => {
  const r = spawnSync(process.execPath, ['host/web/build.mjs', 'app-web'], { cwd: ROOT, encoding: 'utf8', env: { ...process.env, EXACT_APP_DIR: dir, EXACT_WEB_DIST: dist } });
  expect(r.status, r.stderr).toBe(0);
  return readdirSync(dist).find((f) => /^lazy-.*\.js$/.test(f));
};
/** A drive of the app's build: each reply parsed as it comes, the process's end awaited by `done`. */
function drive(dir, dist, storage, ops) {
  const child = spawn(process.execPath, ['scripts/agent.mjs', 'web', '--storage', storage, '--json', ...ops], { cwd: ROOT, env: { ...process.env, EXACT_APP_DIR: dir, EXACT_WEB_DIST: dist }, stdio: ['ignore', 'pipe', 'pipe'] });
  const replies = [], waiters = [];
  let buf = '', said = '';
  child.stdout.on('data', (d) => { buf += d; const lines = buf.split('\n'); buf = lines.pop(); for (const l of lines) if (l.startsWith('{')) replies.push(JSON.parse(l)); for (const w of [...waiters]) w(); });
  child.stderr.on('data', (d) => { said = (said + d).slice(-4000); });
  const done = new Promise((ok) => child.once('close', (code) => ok(code)));
  const until = (n) => new Promise((ok, fail) => { const check = () => { if (replies.length >= n) ok(); }; waiters.push(check); check(); done.then(() => replies.length < n && fail(new Error(`the drive ended after ${replies.length} replies: ${said}`))); });
  return { replies, until, done, said: () => said };
}
const saidOf = (state) => state.resources?.said?.text ?? state.derives?.said?.text;

test('a drive on build N loads N\'s lazy chunk after N+1 lands; its reload stays on N; a new drive boots N+1', async () => {
  const root = mkdtempSync(join(resolve(ROOT, 'target'), 'generation-drive-')), dir = join(root, 'app'), dist = join(root, 'dist'), signal = join(root, 'signal');
  try {
    app(dir, 'lazy one');
    const first = build(dir, dist);
    // Booted on N; it waits for N+1 to land (the signal), then asks for the chunk; then reloads and asks again.
    const d = drive(dir, dist, `generation-drive-${process.pid}`, ['clock data', 'state', `clock +120000 real until ${signal}`, 'tap more', 'clock data', 'state',
      'reload', 'clock data', 'tap more', 'clock data', 'state']);
    await d.until(2);
    expect(saidOf(d.replies[1])).toBe('not asked');
    app(dir, 'lazy two');
    const second = build(dir, dist);
    expect(second).not.toBe(first); // N+1's chunk has another name: N's is in N's kept tree only
    writeFileSync(signal, '');
    expect(await d.done, d.said()).toBe(0);
    const states = d.replies.filter((r) => r.resources || r.slots);
    expect(states.map(saidOf)).toEqual(['not asked', 'lazy one', 'lazy one']);
    // A drive opened now boots N+1.
    const fresh = drive(dir, dist, `generation-drive-new-${process.pid}`, ['clock data', 'tap more', 'clock data', 'state']);
    expect(await fresh.done, fresh.said()).toBe(0);
    expect(saidOf(fresh.replies.at(-1))).toBe('lazy two');
  } finally { rmSync(root, { recursive: true, force: true }); }
}, 900_000);

test('under its generation\'s base, a routes app\'s client navigation, a reload at a deep path and a deep link resolve routes as before', async () => {
  const root = mkdtempSync(join(resolve(ROOT, 'target'), 'generation-routes-')), dir = join(root, 'app'), dist = join(root, 'dist');
  try {
    app(dir, 'lazy');
    writeFileSync(join(dir, 'app.contract'), `routes nav
  home "/"
  item "/item/:id"

shape Said
  text: string

component App
  action open
    nav = push(nav, path("item", "7"))
  view
    column
      text top(nav).url testId="url"
      button press=open testId="open"
        text "Open"
`);
    writeFileSync(join(dir, 'app.ts'), `import type { Answer } from './app.contract.d.ts';
export const appId = 'com.exact.lazy-generation';
export const grants = '';
export const answer: Answer = ((source: string) => { throw new Error(\`unknown source \${source}\`); }) as unknown as Answer;
`);
    build(dir, dist);
    const d = drive(dir, dist, `generation-routes-${process.pid}`, ['clock data', 'tap open', 'clock data', 'state', 'reload', 'clock data', 'state', 'tree url']);
    expect(await d.done, d.said()).toBe(0);
    const states = d.replies.filter((r) => r.navigation);
    // The location is the route's own, never under the generation, before and after a reload there.
    expect(states.map((s) => s.navigation.url)).toEqual(['/item/7', '/item/7']);
    // The reload at that deep path booted the build's document there: its route's own text.
    expect(JSON.stringify(d.replies.at(-1))).toContain('"/item/7"');
  } finally { rmSync(root, { recursive: true, force: true }); }
}, 900_000);
