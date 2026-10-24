// @exact/language-model's conformance suite on the web (LLP 1133.016 §10,
// §13 stages 0 and 1): the fixture app (`test/fixture`) made with
// `exact new` outside the repo, depending on this package by `file:`, built
// for the web and driven in each engine with the web implementation bound.
//
// No engine here has a model: Playwright's Chromium ships no Prompt API, and
// Firefox and WebKit have none. So each engine is driven twice. As it is,
// the module reads `unavailable`, `not-supported`, and a create fails
// `NotSupportedError`. Then with a stand-in `LanguageModel` (below), written
// to the explainer's interface (https://github.com/webmachinelearning/prompt-api),
// which holds the implementation to that interface: availability by
// options, a create inside the press with its download's progress, both
// prompt forms, a constraint passed as a schema, the open tool loop and
// abort by signal. It proves nothing of a browser's own model: that is
// drive 9, by a person in Chrome (`test/checklist.md`).
//
// `--agent` drives the fixture under the agent instead (§10's drives 7 and
// 8 on the web runtime): a constrained prompt's typed answer, a tool's round
// trip, a handler that prompts its own session, a handler that would start a
// download, and a superseded prompt cancelled at the implementation.
//
// usage: bun packages/language-model/test/web.mjs [--agent] [chromium] [firefox] [webkit]
//   (every engine by default; `bunx playwright@1.64.1 install firefox webkit` first)
import { rmSync } from 'node:fs';
import { dirname, extname, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { agentDrives, checker, makeFixture, root, run } from './fixture.mjs';

const args = process.argv.slice(2), agent = args.includes('--agent');
const engines = args.filter(a => a !== '--agent').length ? args.filter(a => a !== '--agent') : ['chromium', 'firefox', 'webkit'];
const { check, failed } = checker();
const { app } = makeFixture();

if (agent) {
  for (const [name, ops, want] of agentDrives()) {
    const r = spawnSync('bun', ['exact.mjs', 'agent', 'web', ...ops], { cwd: app, encoding: 'utf8', maxBuffer: 64 << 20 });
    const out = `${r.stdout}\n${r.stderr}`, missing = want.filter(w => !out.includes(w));
    check('agent', name, r.status === 0 && !missing.length, `missing ${JSON.stringify(missing)}\n${out.split('\n').slice(-30).join('\n')}`);
  }
} else {
  run('bun', ['exact.mjs', 'web-build', '--render', 'none'], app);
  const dist = join(app, 'target/web-dist');
  const types = { '.js': 'text/javascript', '.html': 'text/html', '.wasm': 'application/wasm', '.json': 'application/json', '.css': 'text/css' };
  const server = Bun.serve({ port: 0, async fetch(req) {
    let path = new URL(req.url).pathname;
    if (path === '/') path = '/index.html';
    const file = Bun.file(join(dist, path));
    if (!(await file.exists())) return new Response('not found', { status: 404 });
    return new Response(file, { headers: { 'content-type': types[extname(path)] ?? 'application/octet-stream' } });
  } });
  const origin = `http://127.0.0.1:${server.port}`;
  const { chromium, firefox, webkit } = await import(resolve(root, 'node_modules/playwright-core/index.mjs'));
  for (const engine of engines) {
    const browser = await { chromium, firefox, webkit }[engine].launch();
    const text = (page, id) => page.getByTestId(id).first().innerText({ timeout: 10000 }).catch(e => `(${e.message.split('\n')[0]})`);
    const until = async (page, id, want) => {
      const end = Date.now() + 10000;
      let got;
      while (Date.now() < end) { if (want(got = await text(page, id))) return got; await page.waitForTimeout(50); }
      return got;
    };
    try {
      // As the engine is: no model.
      {
        const context = await browser.newContext(), page = await context.newPage(), errors = [];
        page.on('pageerror', e => errors.push(e.message));
        await page.goto(origin);
        const native = await page.evaluate(() => typeof globalThis.LanguageModel);
        const a = await until(page, 'availability', t => /^(unavailable|downloadable|downloading|available) /.test(t));
        // An engine with the API answers its own state, which the line records; no create is pressed there, since
        // one would start the model's download.
        check(engine, native === 'undefined' ? 'with no Prompt API: unavailable, not-supported' : `with the engine's own Prompt API: ${a}`, native === 'undefined' ? a === 'unavailable not-supported -1' : /^(unavailable \S+|downloadable -|downloading -|available -)/.test(a), a);
        if (native === 'undefined') {
          await page.getByTestId('create').click();
          const e = await until(page, 'created-error', t => t !== '');
          check(engine, 'a create there fails NotSupportedError', e === 'NotSupportedError', e);
        }
        check(engine, 'no page errors', !errors.length, errors.join('; '));
        await context.close();
      }
      // The stand-in.
      const context = await browser.newContext();
      await context.addInitScript(standIn);
      const page = await context.newPage(), errors = [];
      page.on('pageerror', e => errors.push(e.message));
      await page.goto(origin);
      const a = await until(page, 'availability', t => t.startsWith('downloadable'));
      check(engine, 'availability is the browser\'s', a === 'downloadable - -1', a);
      const tooled = await until(page, 'tooled', t => t.startsWith('downloadable'));
      check(engine, 'tool use is asked by expectedOutputs', tooled === 'downloadable true', tooled);
      await page.getByTestId('create').click();
      const made = await until(page, 'created', t => t.startsWith('session-'));
      check(engine, 'a create downloads, then answers the session', /^session-\d+ 4096 findNotes false$/.test(made), made);
      const asked = await page.evaluate(() => globalThis.__standIn.created);
      check(engine, 'the create was called inside the press, its download watched', asked?.event === 'click' && asked.monitor === true && asked.tools === true, JSON.stringify(asked));
      const after = await until(page, 'availability', t => t.startsWith('available'));
      check(engine, 'availability reads available after the download', after.startsWith('available'), after);

      await page.getByTestId('make-title').click();
      const title = await until(page, 'title', t => t !== '');
      check(engine, 'a constrained prompt sends the schema and answers its value', title === 'Stand-in title', `${title} ${await text(page, 'title-error')}`);

      await page.getByTestId('stream').click();
      const streamed = await until(page, 'reply', t => t === 'Hello there, friend.');
      check(engine, 'a streamed reply\'s pieces land in the response', streamed === 'Hello there, friend.', streamed);
      const done = await until(page, 'reply-state', t => t.startsWith('complete'));
      check(engine, 'and it completes', done === 'complete 0', done);

      await page.evaluate(() => { globalThis.__standIn.next = 'tool'; });
      await page.getByTestId('stream').click();
      const tool = await until(page, 'tool', t => t.includes('done'));
      check(engine, 'the open loop runs the app\'s handler', tool.startsWith('findNotes done [{"title":"Borrowing"'), tool);
      const answered = await until(page, 'reply', t => t === 'You noted it.');
      check(engine, 'and prompts again with its answer', answered === 'You noted it.' && (await page.evaluate(() => globalThis.__standIn.toolResponse)) === true, answered);

      await page.evaluate(() => { globalThis.__standIn.next = 'slow'; });
      await page.getByTestId('stream').click();
      await until(page, 'reply', t => t.startsWith('Slowly'));
      await page.getByTestId('stop').click();
      const stopped = await until(page, 'reply-state', t => t.startsWith('aborted'));
      check(engine, 'abort by signal ends the response aborted, its text kept', stopped === 'aborted 0' && (await text(page, 'reply')).startsWith('Slowly'), `${stopped} ${await text(page, 'reply')}`);
      await page.waitForFunction(() => globalThis.__standIn.aborted, null, { timeout: 2000 }).catch(() => {});
      check(engine, 'the stand-in saw the signal', await page.evaluate(() => globalThis.__standIn.aborted), 'not aborted');
      check(engine, 'no page errors', !errors.length, errors.join('; '));
      await context.close();
    } finally {
      await browser.close();
    }
  }
  server.stop();
}
rmSync(dirname(app), { recursive: true, force: true });
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);

/** A stand-in `LanguageModel` with the explainer's interface: `downloadable` until a create downloads it. */
function standIn() {
  const state = globalThis.__standIn = { model: 'downloadable', next: 'text', created: null, aborted: false, toolResponse: false };
  const stream = (pieces, signal, wait = 10) => new ReadableStream({
    async pull(c) {
      if (!pieces.length) return c.close();
      await new Promise(r => setTimeout(r, wait));
      if (signal?.aborted) { state.aborted = true; return c.error(new DOMException('aborted', 'AbortError')); }
      c.enqueue(pieces.shift());
    },
  });
  class Session extends EventTarget {
    constructor(o) { super(); this.contextWindow = 4096; this.contextUsage = 0; this.tools = (o.expectedOutputs ?? []).some(x => x.type === 'tool-call'); }
    async measureContextUsage(input) { return 4 + Math.ceil(JSON.stringify(input).length / 4); }
    promptStreaming(input, o = {}) {
      if (Array.isArray(input) && input[0]?.content?.[0]?.type === 'tool-response') { state.toolResponse = true; return stream(['You ', 'noted ', 'it.'], o.signal); }
      if (o.responseConstraint) return stream([JSON.stringify({ title: 'Stand-in title' })], o.signal);
      const next = state.next;
      state.next = 'text';
      if (next === 'tool' && this.tools) return stream([[{ type: 'tool-call', value: { callID: 'c1', name: 'findNotes', arguments: { query: 'lifetimes' } } }]], o.signal);
      if (next === 'slow') return stream(['Slowly ', 'and ', 'surely ', ...Array(200).fill('on ')], o.signal, 100);
      return stream(['Hello ', 'there, ', 'friend.'], o.signal);
    }
    async prompt(input, o) { let t = ''; for await (const p of this.promptStreaming(input, o)) t += p; return t; }
    async append() {}
    async clone() { return new Session({}); }
    destroy() {}
  }
  globalThis.LanguageModel = class LanguageModel {
    static async availability(o = {}) { return state.model; }
    static async create(o = {}) {
      state.created = { event: globalThis.event?.type ?? null, monitor: typeof o.monitor === 'function', tools: (o.expectedOutputs ?? []).some(x => x.type === 'tool-call') };
      if (state.model !== 'available') {
        const m = new EventTarget();
        o.monitor?.(m);
        for (const loaded of [0, 0.5, 1]) { m.dispatchEvent(Object.assign(new Event('downloadprogress'), { loaded })); await new Promise(r => setTimeout(r, 20)); }
        state.model = 'available';
      }
      return new Session(o);
    }
  };
}
