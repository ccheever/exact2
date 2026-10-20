// The driver's `post` on the web (LLP 1139.004 D1): the page's agent answers the wire's `type`
// with a `message` through glue.js `postSurface`, the path the page's postMessage takes, into
// the real gpu-glue.js over a stand-in GPU module, in Chrome.
import {test, expect} from 'bun:test';
import {readFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {cdpBrowser} from '../../../scripts/agent-launch.mjs';
import {glueDeclarations} from './glue-fixture.mjs';

test('a post is held for its surface, the 65th refused by name, an iframe id refused, and the held ones delivered in order once the canvas exists', async () => {
  const web = resolve(import.meta.dir, '..');
  const adapter = glueDeclarations(readFileSync(resolve(web, 'glue.js'), 'utf8'), ['postSurface', 'agentReply']);
  const moduleSource = `
    export default function() {}
    export function gpu_load() {} export function gpu_unload() {}
    export function gpu_child_view() {} export function gpu_seekable() {} export function gpu_period() {}
    export function gpu_shader_names(){return '[]';} export function gpu_shaders_clear() {}
    export function gpu_create(){return 1;} export function gpu_bind_at(){return true;}
    export function gpu_wants_input(){return false;} export function gpu_dirty(){return false;}
    export function gpu_render(){return 0;} export function gpu_flush(){return true;}
    export function gpu_published() {} export function gpu_messages() {} export function gpu_lifecycle(){}
    export function gpu_assets(){return '{"requests":[],"retired":[]}';}
    export function gpu_agent(){return '{"world":{"tick":0}}';}
    export function gpu_input(id, json){globalThis.inputs.push(JSON.parse(json)); return true;}
    export function gpu_error(){return '';}
  `;
  const html = `<div id="host"><canvas data-surface></canvas></div><iframe id="frame"></iframe><script type="module">
    globalThis.inputs = [];
    const host = document.getElementById('host');
    globalThis.exact = {views:new Map([[1, host], [2, document.getElementById('frame')]]), root:document.body, now:()=>0, devAssets:[], generation:0, postBound:64};
    await import('/gpu-glue.js');
    const views = exact.views, retiredViews = new WeakSet(), POST_BOUND = 64, incarnation = 0, now = () => 0, wasm = {}, tagged = x => x, ask = () => ({}), log = line => console.log(line);
    ${adapter}
    globalThis.askAgent = agentReply; globalThis.ready = true;
  </script>`;
  const server = Bun.serve({port:0, hostname:'127.0.0.1', fetch(req) {
    const path = new URL(req.url).pathname;
    if (path === '/') return new Response(html, {headers:{'content-type':'text/html'}});
    if (path === '/gpu.js') return new Response(moduleSource, {headers:{'content-type':'text/javascript'}});
    if (path === '/favicon.ico') return new Response('', {status:404});
    return new Response(Bun.file(resolve(web, path.slice(1))), {headers:{'content-type':'text/javascript'}});
  }});
  const suite = await cdpBrowser(), {cdp} = suite, lines = [];
  let listener;
  try {
    const {sessionId} = await suite.page();
    const call = (method, params = {}) => cdp.send(method, params, sessionId);
    cdp.listeners.push(listener = msg => { if (msg.sessionId === sessionId && msg.method === 'Runtime.consoleAPICalled') lines.push(msg.params.args.map(a => a.value ?? a.description).join(' ')); if (msg.sessionId === sessionId && msg.method === 'Runtime.exceptionThrown') lines.push(JSON.stringify(msg.params.exceptionDetails)); });
    await call('Runtime.enable');
    const evaluate = async expression => { const r = await call('Runtime.evaluate', {expression, returnByValue:true, awaitPromise:true}); if (r.exceptionDetails) throw new Error(JSON.stringify(r.exceptionDetails)); return r.result.value; };
    await call('Page.navigate', {url:`http://127.0.0.1:${server.port}/`});
    const deadline = Date.now() + 30000;
    while (!await evaluate('globalThis.ready === true')) { if (Date.now() > deadline) throw new Error('fixture did not boot: ' + lines.join('\n')); await Bun.sleep(50); }
    const post = request => evaluate(`askAgent(${JSON.stringify({op:'type', ...request})})`);
    expect((await post({message:'spawn'})).error).toMatch(/^post: no world is live; name its surface/);
    for (let i = 0; i < 64; i++) expect(await post({message:`m${i}`, surface:'world'})).toEqual({posted:'world', delivery:'posted', held:true});
    expect(await post({message:'m64', surface:'world'})).toEqual({error:'post refused: postMessage: dropped: 64 posts already wait for surface "world"'});
    expect(await post({message:'x', surface:'frame'})).toEqual({error:'post refused: postMessage: canvas name collides with an iframe id "frame"'});
    expect(await evaluate('inputs.length')).toBe(0);
    await evaluate('exact.gpu.surface(1, "world", []); exact.gpu.settled()');
    expect(await evaluate('inputs.map(e => e.t === "message" ? e.text : e.t)')).toEqual(Array.from({length:64}, (_, i) => `m${i}`));
    // With the canvas live: no surface is its one world, and its view id names it.
    expect(await post({message:'unnamed'})).toEqual({posted:'world', delivery:'posted'});
    expect(await post({message:'by-id', id:1})).toEqual({posted:'world', delivery:'posted'});
    expect(await evaluate('inputs.slice(64).map(e => e.text)')).toEqual(['unnamed', 'by-id']);
  } finally {
    if (listener) cdp.listeners.splice(cdp.listeners.indexOf(listener), 1);
    await suite.close(); server.stop(true);
  }
}, 120000);
