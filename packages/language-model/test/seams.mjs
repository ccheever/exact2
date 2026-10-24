// The seam is public (LLP 1133.001 D8, LLP 1133.016 §13 stage 3): two tests
// that need nothing an outside author lacks.
//
// - Out of the tree: the package, packed as a registry carries it (`bun pm
//   pack`), installed into an app `exact new` made outside the repo, builds
//   unchanged; in WebKit, which has no `LanguageModel`, it reads
//   `unavailable`, `not-supported`, and a create fails `NotSupportedError`.
// - Replaced: a fixture implementation of `@exact/language-model` outside
//   the tree, a direct dependency of the app, replaces the endorsed web
//   implementation (LLP 1133.001 D4's second rule). It answers a
//   constrained prompt with the shape's value and calls the app's
//   `findNotes` handler through `invoke` (D15 item 1), whose answer the
//   response shows.
//
// usage: bun packages/language-model/test/seams.mjs
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, extname, join, resolve } from 'node:path';
import { checker, makeFixture, pkg, root, run } from './fixture.mjs';

const { webkit } = await import(resolve(root, 'node_modules/playwright-core/index.mjs'));
const { check, failed } = checker();
const scratch = mkdtempSync(join(tmpdir(), 'exact-language-model-seams-'));

/** A built app's page in WebKit, given to `drive(page, text, until)`. */
async function inPage(app, drive) {
  run('bun', ['exact.mjs', 'web-build', '--render', 'none'], app);
  const dist = join(app, 'target/web-dist');
  const types = { '.js': 'text/javascript', '.html': 'text/html', '.wasm': 'application/wasm', '.json': 'application/json', '.css': 'text/css' };
  const server = Bun.serve({ port: 0, async fetch(req) {
    const asked = new URL(req.url).pathname, path = asked === '/' ? '/index.html' : asked, file = Bun.file(join(dist, path));
    return (await file.exists()) ? new Response(file, { headers: { 'content-type': types[extname(path)] ?? 'application/octet-stream' } }) : new Response('not found', { status: 404 });
  } });
  const browser = await webkit.launch();
  const text = (page, id) => page.getByTestId(id).first().innerText({ timeout: 10000 }).catch(() => '');
  const until = async (page, id, want) => {
    const end = Date.now() + 10000;
    let got = '';
    while (Date.now() < end && !want(got = await text(page, id))) await page.waitForTimeout(100);
    return got;
  };
  try {
    const page = await (await browser.newContext()).newPage(), errors = [];
    page.on('pageerror', e => errors.push(e.message));
    await page.goto(`http://127.0.0.1:${server.port}`);
    return { ...await drive(page, until), errors };
  } finally {
    await browser.close();
    server.stop();
  }
}

// Out of the tree: the tarball, installed as any registry package is.
run('bun', ['pm', 'pack', '--destination', scratch], pkg);
const tarball = join(scratch, readdirSync(scratch).find(f => f.startsWith('exact-language-model') && f.endsWith('.tgz')));
const packed = makeFixture({ model: `file:${tarball}` });
const installed = JSON.parse(readFileSync(join(packed.app, 'node_modules/@exact/language-model/package.json'), 'utf8')).name;
check('packed', 'the tarball is installed, not a link into the checkout', installed === '@exact/language-model' && !readFileSync(join(packed.app, 'package.json'), 'utf8').includes(`file:${pkg}`), installed);
const plain = await inPage(packed.app, async (page, until) => {
  const availability = await until(page, 'availability', t => t.startsWith('unavailable'));
  await page.getByTestId('create').click();
  return { availability, created: await until(page, 'created-error', t => t !== '') };
});
check('packed', 'it builds unchanged and reads WebKit as it is', plain.availability === 'unavailable not-supported -1' && plain.created === 'NotSupportedError' && !plain.errors.length, JSON.stringify(plain));
rmSync(dirname(packed.app), { recursive: true, force: true });

// Replaced: an implementation outside the tree that answers from a fixed script.
const dir = join(scratch, 'fixture-language-model-web');
mkdirSync(dir, { recursive: true });
writeFileSync(join(dir, 'package.json'), JSON.stringify({ name: '@fixture/language-model-web', version: '1.0.0', exports: { '.': './exact.json' } }));
writeFileSync(join(dir, 'exact.json'), JSON.stringify({ kind: 'implementation', implements: '@exact/language-model@^1.0', platform: 'web', targets: ['web'], sources: { javascript: ['index.js'] } }));
writeFileSync(join(dir, 'index.js'), `export function implementation({ invoke }) {
  const sessions = new Map(), responses = new Map();
  let next = 0;
  return {
    async languageModelAvailability() { return { availability: 'available', contextWindow: 1024, leavesDevice: false, structuredOutput: 'validated', tools: true }; },
    async createLanguageModel({ tools = [] } = {}) {
      const id = 'fixture-' + ++next;
      sessions.set(id, tools.map(t => t.name));
      return { id, contextWindow: 1024, tools: sessions.get(id), overflowed: false };
    },
    async prompt({ session, responseConstraint }) {
      if (!sessions.has(session)) return { error: 'InvalidStateError' };
      // A constraint from Contract is the named shape's schema as JSON text (D8), and its answer the shape's value.
      const schema = typeof responseConstraint === 'string' ? JSON.parse(responseConstraint) : responseConstraint;
      return schema?.properties?.title ? { title: 'Fixture title' } : { text: 'Fixture text', overflowed: false };
    },
    async promptStreaming({ session }) {
      if (!sessions.get(session)?.includes('findNotes')) return { error: 'NotSupportedError' };
      const r = { id: 'fixture-response-' + ++next, session, state: 'calling-tool', text: '', overflowed: false,
        toolCalls: [{ id: 'c1', name: 'findNotes', arguments: '{"query":"lifetimes"}', state: 'running', truncated: false }] };
      responses.set(r.id, r);
      // The handler runs once this answer has reached the runtime, which registers the handle it serves.
      setTimeout(async () => {
        const answer = await invoke(r.id, 'findNotes', { query: 'lifetimes' });
        Object.assign(r.toolCalls[0], answer?.error ? { state: 'failed', result: answer.error } : { state: 'done', result: answer.text });
        Object.assign(r, { state: 'complete', text: 'Fixture found it.' });
      }, 0);
      return { id: r.id, session };
    },
    async languageModelResponse({ response }) {
      if (!response) return { id: '', session: '', state: 'complete', text: '', toolCalls: [], overflowed: false };
      return responses.get(response) ?? { error: 'InvalidStateError' };
    },
    async abortLanguageModelResponse({ response }) { return { id: response }; },
    async destroyLanguageModel({ session }) { sessions.delete(session); return { id: session }; },
  };
}
`);
const replaced = makeFixture({ dependencies: { '@fixture/language-model-web': `file:${dir}` } });
const outcome = await inPage(replaced.app, async (page, until) => {
  const availability = await until(page, 'availability', t => t.startsWith('available'));
  await page.getByTestId('create').click();
  const created = await until(page, 'created', t => t.startsWith('fixture-'));
  await page.getByTestId('make-title').click();
  const title = await until(page, 'title', t => t !== '');
  await page.getByTestId('stream').click();
  const state = await until(page, 'reply-state', t => t.startsWith('complete 1'));
  return { availability, created, title, state, tool: await until(page, 'tool', t => t.includes('done')), reply: await until(page, 'reply', t => t !== '') };
});
check('replaced', 'an implementation outside the tree answers in place of the endorsed one', outcome.availability === 'available - -1' && outcome.created === 'fixture-1 1024 findNotes false', JSON.stringify(outcome));
check('replaced', 'its constrained prompt answers the shape the app named', outcome.title === 'Fixture title', JSON.stringify(outcome));
check('replaced', 'its invoke runs the app\'s handler, whose answer the response shows', outcome.state === 'complete 1' && outcome.tool.startsWith('findNotes done [{"title":"Borrowing"') && outcome.reply === 'Fixture found it.' && !outcome.errors.length, JSON.stringify(outcome));
rmSync(dirname(replaced.app), { recursive: true, force: true });
rmSync(scratch, { recursive: true, force: true });
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
