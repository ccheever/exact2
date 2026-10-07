// The smoke's inspection steps (`smoke.mjs` imports them as it does
// `smoke-duo.mjs`): 2b, `layout <node>` (LLP 1035.002 D1); 2c, `state`'s
// host sections (D2, D3); 7a, `layout agree` (LLP 1080.001 D5) at the drive's
// settled points; and the pooling drive on Carousel's virtualized feed.
import { render } from './agent-inspect.mjs';
import { spawnSync } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { HOST_DEV } from './app.mjs';

const byTestId = (t, id) => t.nodes.find((n) => n.props.testId === id);
const box = (l, id) => l.nodes.find((n) => n.testId === id);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// An HTTP guest must not become mixed content merely because the native
// host supplies a wrapper (#106). Loopback IP needs no ATS exception, so
// the existing smoke app can exercise the WebKit path without changing
// its manifest. The named-host ATS opt-in is verified on a built bundle.
// A bundled page (`local.html`) loads a loopback `http:` stylesheet, as
// Chrome does from a secure page, and the host logs one that fails (#135).
// A bundled PDF (`local.pdf`) is shown by its type, WebKit's PDF view, not
// as its bytes in an HTML document (#115).
export async function httpFrameSmoke({ host, open, check }) {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-http-frames-'));
  const source = resolve(dir, 'app.contract'), plan = resolve(dir, 'app.plan');
  const server = createServer((req, res) => {
    if (req.url === '/style.css') { res.writeHead(200, { 'Content-Type': 'text/css' }); return res.end('body { background: rgb(26, 127, 55) }'); }
    if (req.url === '/missing.css') { res.writeHead(404); return res.end(); }
    res.writeHead(200, { 'Content-Type': 'text/html', 'Cache-Control': 'no-store' });
    // No viewport meta: a top-level iOS load would report 980, not 300.
    res.end('<!doctype html><p id="result">script blocked</p><script>document.getElementById("result").textContent = "script ran at " + innerWidth + " origin " + self.origin; parent.postMessage("http-ready", "*")</script>');
  });
  let s;
  try {
    await new Promise((done, reject) => { server.once('error', reject); server.listen(0, '127.0.0.1', done); });
    const url = `http://127.0.0.1:${server.address().port}/`;
    writeFileSync(resolve(dir, 'local.html'), `<!doctype html><link rel="stylesheet" href="${url}style.css"><link rel="stylesheet" href="${url}missing.css"><p id="result">unstyled</p><script>addEventListener("load", () => { document.getElementById("result").textContent = getComputedStyle(document.body).backgroundColor + " secure " + isSecureContext })</script>`);
    // One page, "Hello PDF": the smallest file WebKit's PDF view opens.
    writeFileSync(resolve(dir, 'local.pdf'), Buffer.from('JVBERi0xLjQKMSAwIG9iago8PCAvVHlwZSAvQ2F0YWxvZyAvUGFnZXMgMiAwIFIgPj4KZW5kb2JqCjIgMCBvYmoKPDwgL1R5cGUgL1BhZ2VzIC9LaWRzIFszIDAgUl0gL0NvdW50IDEgPj4KZW5kb2JqCjMgMCBvYmoKPDwgL1R5cGUgL1BhZ2UgL1BhcmVudCAyIDAgUiAvTWVkaWFCb3ggWzAgMCAzMDAgMjAwXSAvQ29udGVudHMgNCAwIFIgL1Jlc291cmNlcyA8PCAvRm9udCA8PCAvRjEgNSAwIFIgPj4gPj4gPj4KZW5kb2JqCjQgMCBvYmoKPDwgL0xlbmd0aCA0MCA+PgpzdHJlYW0KQlQgL0YxIDI0IFRmIDQwIDEwMCBUZCAoSGVsbG8gUERGKSBUaiBFVAplbmRzdHJlYW0KZW5kb2JqCjUgMCBvYmoKPDwgL1R5cGUgL0ZvbnQgL1N1YnR5cGUgL1R5cGUxIC9CYXNlRm9udCAvSGVsdmV0aWNhID4+CmVuZG9iagp4cmVmCjAgNgowMDAwMDAwMDAwIDY1NTM1IGYgCjAwMDAwMDAwMDkgMDAwMDAgbiAKMDAwMDAwMDA1OCAwMDAwMCBuIAowMDAwMDAwMTE1IDAwMDAwIG4gCjAwMDAwMDAyNDEgMDAwMDAgbiAKMDAwMDAwMDMzMSAwMDAwMCBuIAp0cmFpbGVyCjw8IC9TaXplIDYgL1Jvb3QgMSAwIFIgPj4Kc3RhcnR4cmVmCjQwMQolJUVPRgo=', 'base64'));
    writeFileSync(source, `component Frames
  state received = ""
  action message(payload)
    received = payload
  view
    column
      iframe src="${url}" width=300 height=70 testId="http-open"
      iframe src="${url}" sandbox="" width=300 height=70 testId="http-blocked"
      iframe src="${url}" sandbox="allow-scripts" message=message width=300 height=70 testId="http-scripts"
      iframe src="local.html" width=300 height=70 testId="local-http"
      iframe src="local.pdf" width=300 height=200 testId="local-pdf"
`);
    const built = spawnSync('cargo', ['run', '-q', '--profile', HOST_DEV, '-p', 'contract', '--', 'build', source, '-o', plan], { cwd: resolve(import.meta.dir, '..'), encoding: 'utf8' });
    if (!check(built.status === 0, `${host} HTTP iframe fixture compiles: ${built.stderr}`)) return;
    s = await open({ host, plan, env: { EXACT_ASSETS: dir } });
    const expected = { 'http-open': `script ran at 300 origin ${new URL(url).origin}`, 'http-blocked': 'script blocked', 'http-scripts': 'script ran at 300 origin null', 'local-http': 'rgb(26, 127, 55) secure true' };
    const failed = `${url}missing.css did not load`;
    const guest = (tree, id) => byTestId(tree, id)?.guest?.find(n => n.id === 'result')?.text;
    // Before #115 the PDF's bytes were the text of an HTML body, which has
    // no element to outline; WebKit's PDF document has its annotation layer
    // (a refused load's error page outlines too, so the layer is named).
    const pdf = (tree) => byTestId(tree, 'local-pdf');
    const pdfShown = (tree) => pdf(tree)?.loading === false && !!pdf(tree).guest?.some(n => n.id === 'annotationContainer');
    let tree, state;
    for (let i = 0; i < 100; i++) {
      tree = await s.tree(); state = await s.state();
      if (Object.entries(expected).every(([id, text]) => byTestId(tree, id)?.loading === false && guest(tree, id) === text) && pdfShown(tree) && state.slots.received === 'http-ready' && s.carrier.hostLines.some(l => l.includes(failed))) break;
      await sleep(50);
    }
    for (const [id, text] of Object.entries(expected)) {
      check(byTestId(tree, id)?.loading === false, `${host} ${id}: HTTP guest did not finish loading`);
      check(guest(tree, id) === text, `${host} ${id}: expected ${text}, got ${JSON.stringify(guest(tree, id))}`);
    }
    check(pdfShown(tree), `${host} local-pdf: a bundled PDF was not shown as a PDF: ${JSON.stringify(pdf(tree))?.slice(0, 200)}`);
    check(state.slots.received === 'http-ready', `${host} HTTP sandboxed guest did not deliver its message`);
    check(s.carrier.hostLines.some(l => l.includes(failed)), `${host} a bundled page's failed sub-resource was not logged: ${s.carrier.hostLines.slice(-5).join(' | ')}`);
  } catch (error) { check(false, `${host} HTTP iframe fixture: ${error.message}`); }
  finally { try { await s?.close(); } finally { await new Promise(done => server.close(done)); rmSync(dir, { recursive: true, force: true }); } }
}

// 2b. `layout <node>` (LLP 1035.002 D1): the runner's half names where
// each value came from, the host's half the spaces it has; the explained
// box is the listing's box; a stale id is refused by name.
export async function explainNode(s, { tree, layout, check }) {
  const explained = await s.layout('station-name');
  const n = explained.node;
  const listed = box(layout, 'station-name');
  check(n && n.id === byTestId(tree, 'station-name')?.id && n.style && n.space?.viewport, `layout station-name carries no node detail: ${JSON.stringify(explained.node)}`);
  check(n && ['authored', 'inherited', 'initial'].includes(n.style.text_color?.source), `text_color has no source: ${JSON.stringify(n?.style?.text_color)}`);
  check(n && listed && Math.abs(n.space.viewport.x - listed.x) < 0.01 && Math.abs(n.space.viewport.w - listed.w) < 0.01, `the explained box ${JSON.stringify(n?.space?.viewport)} disagrees with the listing ${JSON.stringify(listed)}`);
  check(await s.op({ op: 'layout', id: 999999 }).then(() => false, (e) => /stale node/.test(e.message)), 'a stale node id was not refused by name');
  // The wire refuses what it would answer by doing nothing (habits F8): a method's `target`, an unknown op.
  check(await s.op({ op: 'tap', target: 'station-name' }).then(() => false, (e) => /s\.tap\("station-name"/.test(e.message)), 'a raw tap with a target was not refused');
  check(await s.op({ op: 'tapp', id: n?.id }).then(() => false, () => true), 'an unknown op was not refused');
}

// 2c. `state`'s host sections (LLP 1035.002 D2) are present on every host
// (Linux says `unavailable`, never nothing); where the host has a focus,
// the field just typed into is the editor; on iOS the keyboard comes up
// (its notification lands asynchronously, so poll). Every reply is
// tagged with the runner's epoch and incarnation (D3). Answers the state
// and tree it read last.
export async function hostSections(s, { host, tree, state, check }) {
  check(state.focus && state.keyboard && state.navigation, `state lacks a host section: ${Object.keys(state).join(', ')}`);
  check(Number.isInteger(state.epoch) && Number.isInteger(state.incarnation), `state is untagged: epoch ${state.epoch}, incarnation ${state.incarnation}`);
  check(Number.isInteger((await s.layout()).epoch), 'the layout reply is untagged');
  if (host !== 'linux') {
    const field = byTestId(tree, 'station-search')?.id;
    check(state.focus.editor === field && state.focus.logical === field, `the typed field is not the focus: ${JSON.stringify(state.focus)}`);
    if (host === 'ios') {
      for (let i = 0; i < 40 && !state.keyboard.visible; i++) { await sleep(50); state = await s.state(); }
      check(state.keyboard.visible === true && state.keyboard.overlap > 0, `the keyboard is not up: ${JSON.stringify(state.keyboard)}`);
    }
    check(state.slots.searchFocused === true, `typing did not focus the field (searchFocused ${state.slots.searchFocused})`);
    await s.type('station-search', { key: 'Enter' });
    state = await s.state();
    tree = await s.tree();
    check(state.slots.lastKey === 'Enter' && byTestId(tree, 'search-hint')?.props.text === 'searching · last key Enter', `Enter at the field: lastKey ${JSON.stringify(state.slots.lastKey)}, hint ${JSON.stringify(byTestId(tree, 'search-hint')?.props.text)}`);
    check(byTestId(tree, 'station-search')?.props.value === 'Palo', `Enter changed the field's text to ${JSON.stringify(byTestId(tree, 'station-search')?.props.value)}`);
  }
  return { state, tree };
}

// 7a. `layout agree` (LLP 1080.001 D5) after `clock settle`: a failure for a
// drive that does not settle, a missing reply, an incomplete walk (each
// reason named), a required kind that covered nothing, and each listed
// disagreement as the transcript renders it. The web and Linux answer
// `unavailable`, which is asserted instead. Answers the report.
export async function agree(s, label, { host, check }) {
  const settled = await s.clock('settle');
  if (settled?.settled === false) { check(false, `${label}: clock settle did not settle (${settled.reason ?? 'bound'}); agreement not asserted`); return null; }
  const reply = await s.layout(undefined, undefined, { agree: true });
  const a = reply?.agreement;
  if (!check(a, `${label}: layout agree answered no agreement`)) return null;
  if (host === 'web' || host === 'linux') { check(typeof a.unavailable === 'string', `${label}: ${host} must answer agreement unavailable, not ${JSON.stringify(a).slice(0, 200)}`); return a; }
  check(a.complete === true, `${label}: the agreement walk is incomplete (${(a.incomplete ?? []).join(', ')})`);
  const c = a.coverage ?? {};
  check(c.stray?.judged > 0 && c.hidden?.compared > 0 && c.frame?.compared > 0, `${label}: a required kind covered nothing: ${JSON.stringify({ stray: c.stray, hidden: c.hidden?.compared, frame: c.frame?.compared })}`);
  for (const line of render('layout', reply).split('\n').slice(2)) check(false, `${label}: ${line.trim()}`);
  console.log(`${host} agree, ${label}: ${a.complete ? 'complete' : 'INCOMPLETE'}, ${Object.values(a.counts ?? {}).reduce((x, y) => x + y, 0)} disagreements; ${c.stray?.judged} judged, ${c.frame?.compared} frames compared, ${c.hidden?.compared} hidden compared, ${c.kept?.parkedRoots} parked roots`);
  return a;
}

// The pooling drive (LLP 1080.001 D5), when the app is Carousel: on the
// feed page, `Down` moves the virtualized feed far, so rows retire and park and the rows built
// there take them. Both counters must grow — retirement and reuse happened —
// then the walk is clean and counts every parked root `state` reports.
export async function poolingDrive(s, { host, check }) {
  // Carousel opens on its strips; `feed-page` shows the virtualized feed.
  if (!byTestId(await s.tree(), 'feed')) { await s.tap('feed-page'); await s.clock('settle'); }
  const before = (await s.state()).pool;
  if (!check(before && Number.isInteger(before.parks), `carousel: state has no pool section (${JSON.stringify(before)})`)) return;
  await s.tap('feed-far');
  const a = await agree(s, 'carousel: after the feed moved far', { host, check });
  const after = (await s.state()).pool;
  check(after.parks > before.parks && after.takes > before.takes, `carousel: the feed's far move parked ${after.parks - before.parks} and took ${after.takes - before.takes} rows; both must grow`);
  if (a) check(a.coverage?.kept?.parkedRoots === after.parked, `carousel: layout agree counted ${a.coverage?.kept?.parkedRoots} parked roots, state.pool ${after.parked}`);
  console.log(`${host} carousel pool: parks ${before.parks}→${after.parks}, takes ${before.takes}→${after.takes}, parked ${after.parked}`);
}
