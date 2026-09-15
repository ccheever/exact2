// @ref LLP 1038 D7/D11 — real Chrome driver invoked by the web smoke.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { cpSync, readFileSync, writeFileSync, mkdirSync, rmSync } from 'node:fs';
import { resolve } from 'node:path';
import { Cdp, open } from '../../../scripts/agent.mjs';
import { serveStatic, readWebRequest } from '../serve.mjs';
import { publishRoot } from '../../../scripts/deploy.mjs';
import { DirectoryOrigin, appDocumentPath } from '../../../scripts/origin.mjs';
const dir = process.env.EXACT_ROUTER_TEST, dist = resolve(dir, 'dist');
cpSync(process.env.EXACT_ROUTER_DIST, dist, { recursive: true });
for (const name of ['glue.js', 'navigation.js']) cpSync('host/web/' + name, dist + '/' + name);
const bytes = [...readFileSync(dir + '/app.plan')];
const noNavigate = [...readFileSync(dir + '/no-navigate.plan')];
// Only substitute the baked plan at the actual wasm's boot ABI. All
// commits, keyed rows, control presses and journal lines are production.
function fixture(plan) {
  globalThis.fixtureBoot = crypto.randomUUID();
  globalThis.fixturePlan = plan;
  const agentURL = new URL(location.href); agentURL.searchParams.set('agent', '1');
  history.replaceState(null, '', agentURL);
  const instantiate = WebAssembly.instantiateStreaming;
  WebAssembly.instantiateStreaming = async (...args) => {
    const result = await instantiate(...args), w = result.instance.exports;
    history.replaceState(null, '', location.origin + location.pathname);
    globalThis.historyCalls = [];
    for (const name of ['pushState', 'replaceState', 'go']) {
      const original = history[name].bind(history);
      history[name] = (...args) => {
        if (name !== 'go' && args[2] !== location.origin + args[0].url) throw new Error('History API needs an origin-prefixed URL');
        historyCalls.push({ name, args }); return original(...args);
      };
    }
    globalThis.popEvents = [];
    addEventListener('popstate', e => popEvents.push({ state: e.state, url: location.pathname + location.search }));
    let input;
    return { ...result, instance: { exports: { ...w,
      exact_in(n) { input = w.exact_in(n); return input; }, exact_boot(width, height, n) {
      const launch = new Uint8Array(w.memory.buffer, input, n).slice();
      const ptr = w.exact_in(plan.length + n);
      new Uint8Array(w.memory.buffer, ptr, plan.length).set(plan);
      new Uint8Array(w.memory.buffer, ptr + plan.length, n).set(launch);
      return w.exact_boot_plan(plan.length, width, height, n);
    } } } };
  };
}
let html = readFileSync(dist + '/index.html', 'utf8');
html = html.replace('<script type="module" src="./glue.js"></script>', `<script>(${fixture})(${JSON.stringify(bytes)})</script><script type="module" src="./glue.js"></script>`);
writeFileSync(dist + '/index.html', html);
const published = resolve(dir, 'origin');
await publishRoot({ origin: new DirectoryOrigin(published), row: {}, web: dist });
let served = dist;
const server = createServer((req, res) => {
  if (req.url === '/__test-mirror') { res.setHeader('content-type', 'text/html'); res.end('<main id="host"></main>'); return; }
  serveStatic(served, req, res);
});
await new Promise(r => server.listen(0, '127.0.0.1', r));
const url = `http://127.0.0.1:${server.address().port}`;
const child = spawn(process.env.CHROME, ['--headless=new', '--no-sandbox', '--remote-debugging-pipe', '--no-first-run', '--disable-background-networking', `--user-data-dir=${dir}/chrome`, 'about:blank'], { detached: true, stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
const cdp = new Cdp(child.stdio[3], child.stdio[4]);
const exited = new Promise(r => child.on('exit', () => { cdp.fail('Chrome closed'); r(); }));
const rows = [], failures = [], consoleLines = [];
try {
  const { targetInfos } = await cdp.send('Target.getTargets');
  const { sessionId } = await cdp.send('Target.attachToTarget', { targetId: targetInfos.find(t => t.type === 'page').targetId, flatten: true });
  const call = (method, params) => cdp.send(method, params, sessionId);
  cdp.listeners.push(msg => {
    if (msg.sessionId === sessionId && msg.method === 'Runtime.exceptionThrown') consoleLines.push(msg.params.exceptionDetails.exception?.description ?? msg.params.exceptionDetails.text);
  });
  await call('Page.enable'); await call('Runtime.enable');
  const evaluate = async expression => {
    const r = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
    if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? r.exceptionDetails.text);
    return r.result.value;
  };
  const until = async expression => {
    for (let n = 0; n < 300; n++) {
      if (await evaluate(expression).catch(() => false)) return;
      await new Promise(r => setTimeout(r, 10));
    }
    throw new Error('timed out: ' + expression + '\n' + consoleLines.join('\n'));
  };
  const fresh = async (path = '/') => {
    const before = await evaluate('globalThis.fixtureBoot ?? null');
    await call('Page.navigate', { url: url + path + '?agent=1' });
    await until(`globalThis.fixtureBoot !== ${JSON.stringify(before)} && globalThis.exact?.root?.dataset.bootMs && globalThis.exact?.agent`);
    await evaluate('exact.ready');
    return evaluate('history.length');
  };
  const state = () => evaluate(`exact.agent({op:'state'})`);
  const tap = async prefix => {
    const key = (await state()).navigation.route;
    const at = await evaluate(`(()=>{const el=document.querySelector('[data-testid="${prefix}-${key}"]'); el.scrollIntoView({block:'center'}); const r=el.getBoundingClientRect(); return {x:r.x+r.width/2,y:r.y+r.height/2};})()`);
    await call('Input.dispatchMouseEvent', { type: 'mousePressed', button: 'left', clickCount: 1, ...at });
    await call('Input.dispatchMouseEvent', { type: 'mouseReleased', button: 'left', clickCount: 1, ...at });
  };
  let since = 0;
  const record = async (name, path, length, depth, presses, journal = null) => {
    const row = await evaluate(`(()=>{const state=exact.agent({op:'state'});return {origin:location.origin,location:location.pathname+location.search,length:history.length,stamp:history.state,navigation:state.navigation,backPresses:state.slots.backPresses,navigatePresses:state.slots.navigatePresses,logs:exact.agent({op:'logs',since:${since}}),calls:historyCalls.splice(0),pops:popEvents.splice(0)};})()`);
    since = row.logs.next; rows.push({ name, ...row }); console.log(JSON.stringify({ name, ...row }));
    assert.equal(row.location, path, name); assert.equal(row.length, length, name);
    assert.equal(row.navigation.url, path, name); assert.equal(row.navigation.stack.length, depth, name);
    assert.equal(row.backPresses, presses, name);
    if (journal !== null) assert.equal(row.logs.lines.filter(l => l.includes(journal)).length, 1, name);
    return row;
  };
  const historyTap = async delta => {
    const before = await evaluate('popEvents.length');
    const r = await evaluate(`exact.agent({op:'tap',id:exact.agent({op:'tree'}).roots[0],history:${delta}})`);
    assert.equal(r.delivery, 'platform');
    await until(`popEvents.length > ${before}`);
  };
  const run = async (name, fn) => { try { since = 0; await fn(); } catch (error) { failures.push(name + ': ' + error.stack); } };
  const followed = row => {
    assert.equal(row.navigatePresses, 1, 'followLink ran once');
    assert.equal(row.logs.lines.filter(l => l.includes('navigate view') && l.includes('(followLink)')).length, 1);
    assert.equal(row.stamp.url, row.navigation.url);
    assert.equal(String(row.stamp.id), row.navigation.route);
  };
  await run('Back, Forward, replace and programmatic pop', async () => {
    const n = await fresh();
    await record('boot', '/', n, 1, 0);
    await tap('push-post'); await record('push post', '/post/42', n + 1, 2, 0);
    await historyTap(-1); await until(`location.pathname==='/'`);
    const back = await record('browser Back', '/', n + 1, 1, 1);
    assert.equal(back.logs.lines.filter(l => l.includes('(back)')).length, 1);
    assert.equal(back.calls.filter(c => c.name === 'go').length, 1, 'completed pop emits no second go');
    await historyTap(1); await until(`location.pathname==='/post/42'`);
    const forward = await record('Forward follows link', '/post/42', n + 1, 2, 1);
    followed(forward);
    assert.deepEqual(forward.calls.filter(c => c.name === 'go').map(c => c.args[0]), [1]);
    await historyTap(-1); await record('Back after Forward', '/', n + 1, 1, 2);
    await tap('push-post'); await record('push truncates Forward', '/post/42', n + 1, 2, 2);
    await tap('replace'); const replaced = await record('replace top URL', '/post/43', n + 1, 2, 2);
    assert.equal(replaced.calls.filter(c => c.name === 'replaceState').length, 1);
    await tap('back'); await until(`location.pathname==='/'`);
    const program = await record('programmatic Back echo', '/', n + 1, 1, 3);
    assert.deepEqual(program.calls.filter(c => c.name === 'go').map(c => c.args[0]), [-1]);
  });
  await run('in-document reboot retains the carried history mirror', async () => {
    const n = await fresh(); await tap('push-post');
    const pushed = await record('reload prelude', '/post/42', n + 1, 2, 0);
    // The replacement plan has no navigate handler: only the Back control
    // can accept the traversal and perform the app's Back-side effects.
    await evaluate(`exact.reload(new Uint8Array(${JSON.stringify(noNavigate)}))`);
    const reloaded = await record('carried router after reload', '/post/42', n + 1, 2, 0);
    assert.deepEqual(reloaded.stamp, pushed.stamp);
    assert.equal(reloaded.calls.length, 0, 'a carried boot writes no History entry');
    await historyTap(-1);
    const back = await record('Back after reload presses the new control', '/', n + 1, 1, 1);
    assert.equal(back.navigatePresses, 0);
    assert.equal(back.logs.lines.filter(l => l.includes('(back)')).length, 1);
    assert.deepEqual(back.calls.filter(c => c.name === 'go').map(c => c.args[0]), [-1]);
    const m = await fresh('/post/42');
    const freshBoot = await record('fresh document starts at index zero', '/post/42', m, 2, 0);
    assert.equal(freshBoot.stamp.exact, 0);
    assert.equal(freshBoot.calls.filter(c => c.name === 'pushState').length, 0);
  });
  await run('Forward whose handler pushes /other', async () => {
    const n = await fresh(); await tap('push-post'); await tap('push-person');
    await historyTap(-1); await until(`location.pathname==='/post/42'`);
    await tap('redirect-link'); await record('redirect prelude', '/post/42', n + 2, 2, 1);
    await historyTap(1);
    // Restoring then pushing truncates the Forward tail: one pushState,
    // but the total history.length stays unchanged.
    const redirected = await record('Forward redirects to /other', '/other', n + 2, 3, 1);
    followed(redirected);
    assert.deepEqual(redirected.calls.filter(c => c.name === 'go').map(c => c.args[0]), [1, -1]);
    assert.equal(redirected.calls.filter(c => c.name === 'pushState').length, 1);
    await tap('back'); await until(`location.pathname==='/post/42'`);
    const back = await record('in-app Back after redirect', '/post/42', n + 2, 2, 2);
    assert.deepEqual(back.calls.filter(c => c.name === 'go').map(c => c.args[0]), [-1]);
    assert.equal(back.stamp.url, back.navigation.url);
    assert.equal(String(back.stamp.id), back.navigation.route);
  });
  await run('Back can replace the revealed key in the same commit', async () => {
    const n = await fresh(); await tap('push-post'); await tap('replace-back');
    await record('Back replacement prelude', '/post/42', n + 1, 2, 0);
    await historyTap(-1);
    const back = await record('Back selects key with new URL', '/?from=back', n + 1, 1, 1);
    assert.equal(back.stamp.url, back.location);
  });
  await run('two-step Back and go(-2) echo', async () => {
    const n = await fresh(); await tap('push-post'); await tap('push-person');
    await record('two pushes', '/people/7', n + 2, 3, 0);
    await historyTap(-2); await until(`location.pathname==='/'`);
    const traversed = await record('two-step Back follows link', '/', n + 2, 1, 0);
    followed(traversed);
    assert.deepEqual(traversed.calls.filter(c => c.name === 'go').map(c => c.args[0]), [-2]);
    await tap('push-post'); await tap('push-person');
    await record('two pushes after browser Back', '/people/7', n + 2, 3, 0);
    await tap('go-home'); await until(`location.pathname==='/'`);
    const go = await record('commit pops two written ids', '/', n + 2, 1, 0);
    assert.deepEqual(go.calls.filter(c => c.name === 'go').map(c => c.args[0]), [-2]);
  });
  await run('tabs retain ids and push history', async () => {
    const n = await fresh(); await tap('push-post'); await record('tab prelude', '/post/42', n + 1, 2, 0);
    await tap('select-prompts'); await record('tab switch', '/prompts', n + 2, 1, 0);
    await historyTap(-1); await until(`location.pathname==='/post/42'`);
    const undo = await record('tab undo follows link', '/post/42', n + 2, 2, 0);
    followed(undo);
    assert.deepEqual(undo.calls.filter(c => c.name === 'go').map(c => c.args[0]), [-1]);
    await tap('select-prompts'); await record('tab switch truncates Forward', '/prompts', n + 2, 1, 0);
    await tap('select-home'); await record('select retained top pushes', '/post/42', n + 3, 2, 0);
    await tap('back'); const back = await record('Back after tab switch pushes', '/', n + 4, 1, 1);
    assert.equal(back.calls.filter(c => c.name === 'pushState').length, 1);
  });
  await run('disabled Back restores once', async () => {
    const n = await fresh(); await tap('push-post'); await tap('refuse'); await record('disabled fixture', '/post/42', n + 1, 2, 0);
    await historyTap(-1); await until(`location.pathname==='/post/42'`);
    const back = await record('refused Back', '/post/42', n + 1, 2, 0, 'history: Back refused');
    assert.deepEqual(back.calls.filter(c => c.name === 'go').map(c => c.args[0]), [-1, 1]);
    assert.equal(back.logs.lines.length, 1);
  });
  await run('navigate handler without a matching commit restores once', async () => {
    const n = await fresh(); await tap('push-post'); await historyTap(-1); await tap('refuse-link');
    await record('refused navigate prelude', '/', n + 1, 1, 1);
    await historyTap(1); await until(`location.pathname==='/'`);
    const refused = await record('uncommitted Forward restored', '/', n + 1, 1, 1, 'history: navigate');
    followed(refused);
    assert.deepEqual(refused.calls.filter(c => c.name === 'go').map(c => c.args[0]), [1, -1]);
  });
  await run('reload opens the declared chain', async () => {
    let n = await fresh('/prompt/5/write');
    const launched = await record('deep launch', '/prompt/5/write', n, 3, 0);
    assert.deepEqual(launched.navigation.stack, ['1','5','6']);
    await tap('push-post'); await record('nondeclared stack before reload', '/post/42', n + 1, 4, 0);
    await tap('open-write'); n = await evaluate('history.length');
    const before = await evaluate('fixtureBoot');
    await call('Page.reload'); await until(`globalThis.fixtureBoot !== ${JSON.stringify(before)} && globalThis.exact?.root?.dataset.bootMs`); await evaluate('exact.ready');
    since = 0;
    const reloaded = await record('reload /prompt/5/write', '/prompt/5/write', n, 3, 0);
    assert.deepEqual(reloaded.navigation.stack, ['1','5','6']);
    assert.equal(reloaded.stamp.exact, 0);
  });
  await run('notfound double slash stays a path on this origin', async () => {
    const n = await fresh(); await record('notfound prelude', '/', n, 1, 0);
    await tap('open-unknown');
    const opened = await record('open //evil.invalid/x', '//evil.invalid/x', n + 1, 2, 0);
    assert.equal(await evaluate('location.origin'), url);
    assert.equal(opened.calls.find(c => c.name === 'pushState').args[2], url + '//evil.invalid/x');
    await tap('replace-unknown');
    const replaced = await record('replace //evil.invalid/y', '//evil.invalid/y', n + 1, 2, 0);
    assert.equal(replaced.stamp.id, opened.stamp.id);
    assert.equal(replaced.calls.find(c => c.name === 'replaceState').args[2], url + '//evil.invalid/y');
    await tap('push-post'); await record('push above notfound', '/post/42', n + 2, 3, 0);
    await historyTap(-1);
    await record('Back selects notfound', '//evil.invalid/y', n + 2, 2, 1);
    const launchedLength = await fresh('//evil.invalid/x'); since = 0;
    const launched = await record('launch //evil.invalid/x', '//evil.invalid/x', launchedLength, 2, 0);
    assert.equal(await evaluate('location.origin'), url);
    assert.equal(launched.calls.find(c => c.name === 'replaceState').args[2], url + '//evil.invalid/x');
    assert.deepEqual(consoleLines, [], 'no browser exception for a double-slash notfound location');
  });
  await run('accepted entries before boot and queued commits', async () => {
    await call('Page.navigate', {url: url + '/__test-mirror'});
    await until(`document.querySelector('#host') !== null`);
    const result = await evaluate(`(async()=>{
      const {navigation:m}=await import('/navigation.js');
      const host=document.querySelector('#host'), nav=document.createElement('main');host.append(nav);
      nav.setAttribute('navigationBack','back');
      const journals=[], calls=[]; let supersede=false, refuseNavigate=false;
      addEventListener('popstate',e=>{if(supersede&&e.state?.exact===2){supersede=false;history.go(-1);}});
      const original=history.go.bind(history);
      history.go=n=>{calls.push(n);original(n);};
      const root={id:0,url:'/'}, post={id:5,url:'/post/42'}, person={id:6,url:'/people/7'};
      history.replaceState(null,'','/');history.pushState(null,'',post.url);history.pushState(null,'',person.url);
      let stack=[];
      const emit=next=>{
        const removed=stack.filter(e=>!next.some(n=>n.id===e.id)).map(e=>e.id);stack=next;
        nav.replaceChildren(...next.map(e=>{const r=document.createElement('section');r.setAttribute('navigationKey',e.id);const b=document.createElement('button');b.id='back';b.textContent='Back';b.onclick=()=>emit(stack.slice(0,-1));r.append(b);return r;}));
        nav.setAttribute('navigationKey',next.at(-1).id);
        m.apply({top:next.at(-1).id,url:next.at(-1).url,removed});m.project(host,s=>journals.push(s));
      };
      m.connect(host,url=>{if(refuseNavigate){journals.push('navigate refused');return {};}emit([root,post,person].slice(0,[root,post,person].findIndex(e=>e.url===url)+1));},s=>journals.push(s));
      emit([root,post,person]);await m.travel(nav,-1);await m.travel(nav,-1);
      const before=history.length;emit([root,{id:7,url:'/post/43'}]);
      calls.length=0;nav.lastElementChild.firstElementChild.click();
      await new Promise(resolve=>addEventListener('popstate',resolve,{once:true}));
      const negative={location:location.pathname,stamp:history.state,url:m.observation(host).url,length:history.length,calls:[...calls]};
      emit([root,post]);calls.length=0;
      nav.lastElementChild.firstElementChild.click();emit([root,{id:8,url:'/people/8'}]);
      await new Promise(resolve=>addEventListener('popstate',()=>queueMicrotask(resolve),{once:true}));
      const queued={location:location.pathname,stamp:history.state,url:m.observation(host).url,calls:[...calls]};
      m.reset();stack=[];emit([root]);emit([root,post]);emit([root,post,person]);emit([root,post,person,{id:8,url:'/people/8'}]);
      nav.lastElementChild.firstElementChild.disabled=true;
      calls.length=0;supersede=true;refuseNavigate=true;
      await m.travel(nav,-1);
      return {negative,before,queued,superseded:{location:location.pathname,stamp:history.state,url:m.observation(host).url,calls},journals};
    })()`);
    rows.push({name:'pre-boot entries and queued commits (host seam fixture)',...result});
    assert.equal(result.negative.location,'/');assert.equal(result.negative.url,'/');
    assert.equal(result.negative.stamp.exact,-2);assert.deepEqual(result.negative.calls,[-1]);
    assert.equal(result.queued.location,'/people/8');assert.equal(result.queued.url,'/people/8');
    assert.equal(result.queued.stamp.exact,-1);
    assert.equal(result.superseded.location,'/people/8');assert.equal(result.superseded.url,'/people/8');
    assert.equal(result.superseded.stamp.exact,3);assert.equal(result.journals.length,3);
  });
  await run('raw request targets cannot normalize into public files or app fallback', async () => {
    const targets = ['/.exact/%2e%2e/post/42', '/assets/%2e%2e/app.wasm',
      '/assets/%2E%2E/app.wasm', '/assets/.%2e/app.wasm', '/assets/%2e./app.wasm',
      '/assets/../app.wasm', '/.exact/../post/42', '/%2e/post/42', '/%2E/post/42',
      '/assets\\..\\app.wasm', '/assets/%5c../app.wasm', '/post/%00/42',
      '/assets/%2f%2e%2e/app.wasm'];
    for (const [name, tree] of [['static', dist], ['origin', published]]) {
      served = tree;
      for (const target of targets) {
        // fetch/new URL normalize dot segments client-side. curl carries
        // the exact raw target to the same HTTP handler Chrome uses.
        const headers = await new Promise((resolve, reject) => {
          const curl = spawn('curl', ['--silent', '--show-error', '--path-as-is', '--request-target', target,
            '--dump-header', '-', '--output', '/dev/null', url + '/']);
          let out = '', err = '';
          curl.stdout.on('data', b => out += b); curl.stderr.on('data', b => err += b);
          curl.on('error', reject); curl.on('exit', code => code === 0 ? resolve(out) : reject(new Error(err)));
        });
        const row = {name:'raw HTTP ' + name, target, status:Number(/^HTTP\/\S+ (\d+)/.exec(headers)?.[1]),
          cache:/^cache-control: (.*)$/im.exec(headers)?.[1].trim()};
        rows.push(row); console.log(JSON.stringify(row));
        assert.equal(row.status, 404, target); assert.equal(row.cache, 'no-store', target);
        assert.equal(appDocumentPath(target), false, target);
        assert.equal((await readWebRequest(tree, target)).found, null, target);
      }
    }
  });
  await run('published app deep launch and HTTP policy', async () => {
    served = published;
    const n = await fresh('/post/42'); await record('published /post/42', '/post/42', n, 2, 0);
    await fresh('/prompt/5/write'); const s = await state(); assert.deepEqual(s.navigation.stack, ['1','5','6']);
    for (const path of ['/post/42', '/prompt/5/write', '/.exact/install/', '/missing.png', '/__dev/missing', '/.exact/missing', '/.git/config']) {
      const response = await fetch(url + path), body = await response.text();
      const entry = { name: 'published HTTP', path, status: response.status, type: response.headers.get('content-type'), vary: response.headers.get('vary') }; rows.push(entry); console.log(JSON.stringify(entry));
      assert.equal(response.status, path === '/post/42' || path === '/prompt/5/write' || path === '/.exact/install/' ? 200 : 404);
      if (response.status === 200) assert.equal(entry.type, 'text/html');
      if (path.startsWith('/.exact/install')) assert.ok(body.includes('Install') && !body.includes('exact-root'));
    }
    for (const path of ['/__dev/missing/index.html', '/.exact/missing/index.html', '/absent/index.html']) {
      const absent = await readWebRequest(dist, path, 'application/vnd.exact.envelope+json');
      assert.equal(absent.index, false); assert.equal(absent.found, null);
    }
    const envelope = await fetch(url + '/post/42', { headers: { accept: 'application/vnd.exact.envelope+json' } });
    assert.equal(envelope.headers.get('vary'), 'Accept'); assert.equal(envelope.headers.get('content-type'), 'application/vnd.exact.envelope+json');
    const payload = await envelope.json();
    assert.ok(payload.plan.url.startsWith('/.exact/root/web/releases/'));
    const release = payload.plan.url.slice(0, -'app.plan'.length);
    for (const path of [release, release + 'index%2ehtml', '/index%2ehtml']) {
      const native = await readWebRequest(published, path, 'application/vnd.exact.envelope+json');
      assert.equal(native.found?.route, '/exact.json');
    }
    for (const path of ['/post/42', '/prompt/5/write']) {
      const { found, index } = await readWebRequest(dist, path); assert.equal(index, true); assert.equal(found.route, '/index.html');
      const native = await readWebRequest(dist, path, 'application/vnd.exact.envelope+json');
      assert.equal(new URL(JSON.parse(native.found.body).plan.url, url + path).pathname, '/app.plan');
      assert.ok(found.body.toString().includes('href="/exact.json"'));
    }
    // The public session API's form runs the same page path.
    served = dist;
    const session = await open({ host: 'web', url: url + '/' });
    try {
      let key = (await session.state()).navigation.route;
      await session.tap('push-post-' + key);
      assert.equal((await session.tap('navigation', { history: -1 })).delivery, 'platform');
      assert.equal((await session.state()).navigation.url, '/');
      assert.equal((await session.state()).slots.backPresses, 1);
    } finally { await session.close(); }
  });
} finally {
  if (process.env.EXACT_ROUTER_EVIDENCE) { mkdirSync(process.env.EXACT_ROUTER_EVIDENCE, { recursive: true }); writeFileSync(process.env.EXACT_ROUTER_EVIDENCE + '/browser.json', JSON.stringify({ rows, failures, consoleLines }, null, 2)); }
  process.kill(-child.pid, 'SIGKILL'); await exited;
  server.close(); server.closeAllConnections();
}
assert.deepEqual(failures, []);
console.log('router browser: all session-history and serving cases passed');
process.exit(0);
