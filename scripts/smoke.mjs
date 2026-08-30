#!/usr/bin/env node
// The smoke: the Caltrain app driven through the agent API (LLP 1012) — the
// same script on the web (headless Chrome), on macOS, on iOS (a simulator),
// and on the Linux host (headless, wherever it was built). Asserts the
// landmarks, the layout (root width, the image's box from its ratio), the
// clock, one whole interaction through the host's real input path, scrolling
// in the app and in the nested fixture (LLP 1010), the GPU module, and a
// clean journal; prints the numbers. Not a blocking check (it needs Chrome or
// a window server): `node scripts/smoke.mjs <web|macos|ios|linux> [--shot <png>]`
// after `node host/web/build.mjs` / `node host/apple/build.mjs [--ios]` /
// `cargo build --release -p caltrain-linux`.
import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { open, render, runTests } from './agent.mjs';
import { crop, decodePng, diff, encodePng } from './png.mjs';

const argv = process.argv.slice(2);
const ROOT = resolve(new URL('..', import.meta.url).pathname);

// 0. The transcript form (LLP 1012 §7): the one text rendering of the
// replies, pinned by a fixture — `scripts/fixtures/transcript.json` rendered
// must equal `transcript.txt`. `--record` rewrites the text from the code
// after a deliberate change to the form.
const transcript = () => {
  const sample = JSON.parse(readFileSync(resolve(ROOT, 'scripts/fixtures/transcript.json'), 'utf8'));
  // A sample named `empty` or `dropped` is a `logs` reply; the rest are named by their op.
  return Object.entries(sample).map(([name, reply]) => `--- ${name}\n${render(name === 'empty' || name === 'dropped' ? 'logs' : name, reply)}`).join('\n\n') + '\n';
};
const pinned = resolve(ROOT, 'scripts/fixtures/transcript.txt');
if (argv.includes('--record')) { writeFileSync(pinned, transcript()); console.log(`recorded ${pinned.replace(ROOT + '/', '')}`); process.exit(0); }

const host = argv[0] === 'macos' || argv[0] === 'mac' ? 'macos' : argv[0] === 'web' ? 'web' : argv[0] === 'ios' ? 'ios' : argv[0] === 'linux' ? 'linux' : null;
if (!host) { console.error('usage: node scripts/smoke.mjs <web|macos|ios|linux> [--shot <png>] | --record'); process.exit(2); }
// The two Apple presenters share one Canvases: children captured through the
// surface, placements (LLP 1014 D2, D5) — what the canvas steps below assert.
const apple = host === 'macos' || host === 'ios';
const shot = argv.includes('--shot') ? argv[argv.indexOf('--shot') + 1] : process.env.EXACT_SHOT;
// --record-canvas rewrites this host's reference picture of the canvas
// fixture (step 10) after a deliberate change to what it shows.
const recordCanvas = argv.includes('--record-canvas');
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const failures = [];
const check = (ok, what) => { if (!ok) failures.push(what); return ok; };
const t0 = Date.now();
const byTestId = (t, id) => t.nodes.find((n) => n.props.testId === id);
const box = (l, id) => l.nodes.find((n) => n.testId === id);
// The app's viewport (step 2): the safe area on a phone, which step 12's
// full-bleed fixture grows by the insets.
let appViewport;

check(transcript() === readFileSync(pinned, 'utf8'), 'the transcript form drifted from scripts/fixtures/transcript.txt (a deliberate change: node scripts/smoke.mjs --record)');

const s = await open({ host });
try {
  // 1. Landmarks and text, from the kernel.
  let tree = await s.tree();
  for (const id of ['caltrain-main', 'station-name', 'board-north', 'board-south', 'change-station']) check(byTestId(tree, id), `missing testId ${id}`);
  check(byTestId(tree, 'station-name')?.props.text === 'Mountain View', 'station name not presented');
  const countdowns = tree.nodes.filter((n) => n.props.testId?.startsWith('countdown-'));
  check(countdowns.length > 0, 'no countdowns');

  // 2. Layout, as the host renders it: the root fills the viewport's width
  // (the block rule for a root), and the image is laid out from its natural
  // size once it has loaded — 320×120 at width 96 is 96×36.
  let layout = await s.layout();
  appViewport = layout.viewport;
  const root = box(layout, 'caltrain-main');
  check(root?.w === layout.viewport.w, `the root is ${root?.w} wide in a ${layout.viewport.w} viewport`);
  let logo = box(layout, 'logo');
  for (let i = 0; i < 40 && !(logo && Math.round(logo.h) === 36); i++) { await sleep(50); logo = box(await s.layout(), 'logo'); }
  check(logo && Math.round(logo.w) === 96 && Math.round(logo.h) === 36, `the logo is ${logo?.w}×${logo?.h}, not 96×36 from its 320×120 ratio`);

  // 3. The clock: a minute later every countdown still shown is one less —
  // sixty timer fires from one seek, and nothing waited.
  const before = new Map(countdowns.map((n) => [n.props.testId, Number(n.props.text)]));
  await s.clock('+60000');
  tree = await s.tree();
  const after = tree.nodes.filter((n) => n.props.testId?.startsWith('countdown-') && before.has(n.props.testId));
  const wrong = after.filter((n) => Number(n.props.text) !== before.get(n.props.testId) - 1).map((n) => `${n.props.testId} ${before.get(n.props.testId)}→${n.props.text}`);
  check(after.length > 0 && wrong.length === 0, `after +60 s every countdown is one less; not: ${wrong.join(', ') || 'none left'}`);
  let state = await s.state();
  check(state.clock === 60000 && state.slots.nowMs === 1787915400000 + 60000, `state after +60 s: clock ${state.clock}, nowMs ${state.slots.nowMs}`);
  const journal = await s.logs();
  check(journal.lines.some((l) => /advance → 60 timers fired/.test(l)), 'the journal does not show sixty timers firing from one seek');

  // 4. One interaction through the host's real input path: change station,
  // search, pick, home.
  await s.tap('change-station');
  tree = await s.tree();
  check(byTestId(tree, 'stations-screen'), 'tapping Change station did not open the stations screen');
  // 4a. The events beyond press and change (LLP 1005 §3), through the
  // host's own paths: a hover highlights the row under the pointer and
  // leaves with it; typing focuses the field; a key reaches it by name.
  if (host !== 'linux') {
    await s.tap('station-sf', { hover: true });
    state = await s.state();
    tree = await s.tree();
    check(state.slots.hoverOn === true && state.slots.hoverId === 'sf' && byTestId(tree, 'station-hot-sf'), `a hover over station-sf: hoverOn ${state.slots.hoverOn}, hoverId ${JSON.stringify(state.slots.hoverId)}, highlighted ${!!byTestId(tree, 'station-hot-sf')}`);
    await s.tap('station-search', { hover: true });
    state = await s.state();
    check(state.slots.hoverOn === false, `the pointer left station-sf: hoverOn ${state.slots.hoverOn}`);
  }
  await s.type('station-search', 'Palo');
  tree = await s.tree();
  check(byTestId(tree, 'station-search')?.props.value === 'Palo', `typing left the field at ${JSON.stringify(byTestId(tree, 'station-search')?.props.value)}`);
  state = await s.state();
  check(state.slots.query === 'Palo', `the query slot did not hear the change (${JSON.stringify(state.slots.query)})`);
  if (host !== 'linux') {
    check(state.slots.searchFocused === true, `typing did not focus the field (searchFocused ${state.slots.searchFocused})`);
    await s.type('station-search', { key: 'Enter' });
    state = await s.state();
    tree = await s.tree();
    check(state.slots.lastKey === 'Enter' && byTestId(tree, 'search-hint')?.props.text === 'searching · last key Enter', `Enter at the field: lastKey ${JSON.stringify(state.slots.lastKey)}, hint ${JSON.stringify(byTestId(tree, 'search-hint')?.props.text)}`);
    check(byTestId(tree, 'station-search')?.props.value === 'Palo', `Enter changed the field's text to ${JSON.stringify(byTestId(tree, 'station-search')?.props.value)}`);
  }
  const matches = tree.nodes.filter((n) => n.type === 'Pressable' && n.props.testId?.startsWith('station-'));
  check(matches.length === 1 && matches[0].props.testId === 'station-paloalto', `the search shows ${matches.map((m) => m.props.testId).join(', ') || 'nothing'}, not station-paloalto alone`);
  await s.tap('station-paloalto');
  tree = await s.tree();
  check(byTestId(tree, 'station-name')?.props.text === 'Palo Alto', `after picking Palo Alto the station is ${byTestId(tree, 'station-name')?.props.text}`);
  check(byTestId(tree, 'home-screen'), 'picking a station did not return home');

  // 5. Scrolling (LLP 1010): a wheel over the content moves it, and exactly
  // one scroll container takes it — the node when it can, else the page.
  layout = await s.layout();
  const nameBefore = box(layout, 'station-name').y;
  const inner0 = layout.nodes.find((n) => n.type === 'ScrollView');
  await s.tap('station-name', { wheel: [0, 300] });
  layout = await s.layout();
  const moved = nameBefore - box(layout, 'station-name').y;
  const inner = layout.nodes.find((n) => n.id === inner0.id);
  const innerMoved = inner.sy > inner0.sy, pageMoved = box(layout, 'caltrain-main').y < 0;
  check(moved === 300, `a wheel of 300 over the content scrolled it by ${moved}`);
  check(innerMoved !== pageMoved, `one scroll container takes a wheel: inner moved ${innerMoved}, page moved ${pageMoved}`);

  // 5a. A command (LLP 1005 §3): `setScheme` reaches the host as an op and
  // sets its colour scheme; the journal records it and the host reports no
  // error (step 7 reads both). Back to light for the pictures below.
  await s.tap('scheme-dark');
  await s.tap('scheme-light');

  // 6. The pixels when asked, and the GPU module where the host renders it
  // (a canvas is on the page; headless Chrome has WebGPU).
  if (shot) console.log(JSON.stringify(await s.screenshot(shot)));
  if (host === 'web') {
    let g = s.gpuMs();
    for (let i = 0; i < 60 && g == null; i++) { await sleep(50); g = s.gpuMs(); }
    check(g != null, 'a canvas is on the page but the GPU module did not load (no beacon; WebGPU unavailable in this Chrome?)');
    if (g != null) console.log(`gpu: module loaded ${g} ms after injection (after the first paint)`);
  }

  // 6a. A canvas's children (LLP 1014 D1): laid out by the kernel in the
  // canvas's box and presented over its surface on every host — the aurora's
  // title is the station's name, and its box lies inside the aurora's.
  tree = await s.tree();
  check(byTestId(tree, 'aurora-title')?.props.text === byTestId(tree, 'station-name')?.props.text, `the aurora's title is ${JSON.stringify(byTestId(tree, 'aurora-title')?.props.text)}, not the station's name`);
  layout = await s.layout();
  const aurora = box(layout, 'aurora'), title = box(layout, 'aurora-title');
  const inside = aurora && title && title.x >= aurora.x - 0.5 && title.y >= aurora.y - 0.5 && title.x + title.w <= aurora.x + aurora.w + 0.5 && title.y + title.h <= aurora.y + aurora.h + 0.5;
  check(inside, `the aurora's title ${JSON.stringify(title)} is not inside the aurora ${JSON.stringify(aurora)}`);

  // 7. The journal: a boot, the presses, the change, the timers — no refusal,
  // no host error.
  const logs = await s.logs();
  const lines = [...journal.lines, ...logs.lines];
  check(lines[0]?.includes('boot: '), 'no boot line in the journal');
  check(journal.dropped === 0 && logs.dropped === 0, 'the journal ring dropped lines in a short run');
  const bad = lines.filter((l) => /refused|poisoned/.test(l));
  check(bad.length === 0, 'refusals in the journal:\n    ' + bad.join('\n    '));
  const hostBad = [...journal.host, ...logs.host].filter((l) => /exact:|error|exception/i.test(l));
  check(hostBad.length === 0, 'the host reported errors:\n    ' + hostBad.join('\n    '));
  console.log(`${host}: boot ${s.boot.toFixed(1)} ms; ${tree.nodes.length} nodes; ${lines.length} journal lines`);
} finally {
  await s.close();
}

// 8. The nested case (LLP 1010, the scroll fixture): a scroll node that
// overflows on a page that overflows. A wheel over the node scrolls it; at
// its edge the wheel chains to the page.
const tmp = mkdtempSync(resolve(tmpdir(), 'exact-smoke-'));
const plan = resolve(tmp, 'scroll.plan');
const c = spawnSync('cargo', ['run', '-q', '--release', '-p', 'contract', '--', 'build', resolve(ROOT, 'contract/corpus/scroll.contract'), '-o', plan], { cwd: ROOT, encoding: 'utf8' });
if (c.status !== 0) failures.push('the scroll fixture did not compile: ' + c.stderr);
else {
  const f = await open({ host, plan });
  try {
    let l = await f.layout();
    check(box(l, 'rows') && box(l, 'root'), 'the fixture did not boot');
    await f.tap('row-1', { wheel: [0, 100] });
    l = await f.layout();
    check(box(l, 'rows').sy === 100, `the scroll node took a wheel of 100: scrolled ${box(l, 'rows').sy}`);
    check(box(l, 'root').y === 0, 'the page moved for a wheel the scroll node consumed');
    for (let i = 0; i < 12; i++) await f.tap('rows', { wheel: [0, 400] });
    l = await f.layout();
    const limit = box(l, 'rows').sy;
    // 652 is a parity number: the fixture's rows under each host's text
    // metrics. If it moves on one host, the hosts have parted.
    check(limit === 652, `the scroll node stops at ${limit}, not 652 (the same limit on both hosts)`);
    check(box(l, 'root').y < 0, `a scroll node at its edge (${limit}) did not chain to the page (root at ${box(l, 'root').y})`);
    check(box(l, 'rows').sy === limit, 'the scroll node moved past its edge');
    console.log(`${host} fixture: the scroll node stops at ${limit}, then the page scrolls (root at ${box(l, 'root').y})`);
  } finally {
    await f.close();
  }
}
rmSync(tmp, { recursive: true, force: true });

// 9. Children through the surface (LLP 1014, the canvas fixture): a button
// and an input inside a canvas whose surface samples its children. Both are
// laid out in the canvas's box; a tap on the button reaches it — on macOS
// through the alpha-0 overlay (D5) — and typing reaches the field; on macOS
// the presenter captures the canvas for the batch and while the field is
// edited (D4 a, d), which agent mode reports in the logs.
{
  const tmp = mkdtempSync(resolve(tmpdir(), 'exact-smoke-'));
  const plan = resolve(tmp, 'canvas.plan');
  const c = spawnSync('cargo', ['run', '-q', '--release', '-p', 'contract', '--', 'build', resolve(ROOT, 'contract/corpus/canvas.contract'), '-o', plan], { cwd: ROOT, encoding: 'utf8' });
  if (c.status !== 0) failures.push('the canvas fixture did not compile: ' + c.stderr);
  else {
    // On Linux the picture is the oracle painter's (tiny-skia; LLP 1015 §2)
    // over the pinned font the driver sets (§5): the same bytes on every
    // machine, so the reference below is exact, not a band — the GPU painter
    // is held to it by `tests/paint.rs`'s band instead.
    const f = await open({ host, plan, env: host === 'linux' ? { EXACT_PAINTER: 'cpu' } : undefined });
    try {
      let t = await f.tree();
      check(byTestId(t, 'sky-zoom') && byTestId(t, 'sky-label'), 'the canvas fixture did not boot');
      const l = await f.layout();
      const sky = box(l, 'sky'), zoom = box(l, 'sky-zoom');
      check(sky && zoom && zoom.y >= sky.y - 0.5 && zoom.y + zoom.h <= sky.y + sky.h + 0.5, `the button ${JSON.stringify(zoom)} is not inside the canvas ${JSON.stringify(sky)}`);
      let logs = '';
      if (apple) {
        // The module loads after the first paint; the first capture puts the
        // overlay at alpha 0 — the tap below must go through it. The first
        // dlopen of a freshly built dylib can take seconds (macOS checks a
        // new binary on its first load), so the poll is long; it stops at
        // the first capture, which is ~200 ms in the usual case.
        for (let i = 0; i < 200 && !/captured/.test(logs); i++) { await sleep(50); logs += JSON.stringify(await f.logs()); }
        check(/captured/.test(logs), 'the canvas fixture was never captured within 10 s (did the surface want its children?)');
      }

      // 10. The readback (LLP 1014 §2 step 3, LLP 1009 D1): the canvas as
      // this host composes it — the sky at clock 0 with its children through
      // it on macOS, over it on the web — cropped from the screenshot by the
      // canvas's box and held against this host's recorded reference,
      // scripts/fixtures/canvas-sky.<host>.png (`--record-canvas` rewrites
      // it). The clock is the agent's, so the sky is the same picture every
      // run; the band is for the GPU's arithmetic (on Linux the picture is
      // the CPU oracle's with the pinned font, and matches to the pixel on
      // any machine). Taken before the tap and the edit: a caret blinks on
      // the wall clock.
      if (host === 'web') { let g = f.gpuMs(); for (let i = 0; i < 60 && g == null; i++) { await sleep(50); g = f.gpuMs(); } }
      await sleep(150); // one frame of the surface after its first capture
      const shotPath = resolve(tmp, 'canvas.png');
      await f.screenshot(shotPath, true);
      const image = decodePng(readFileSync(shotPath));
      const scale = image.width / l.viewport.w;
      const top = image.height - Math.round(l.viewport.h * scale); // a window shot carries the title bar above the content
      const region = crop(image, Math.round(sky.x * scale), top + Math.round(sky.y * scale), Math.round(sky.w * scale), Math.round(sky.h * scale));
      const reference = resolve(ROOT, `scripts/fixtures/canvas-sky.${host}.png`);
      if (recordCanvas) { writeFileSync(reference, encodePng(region)); console.log(`recorded ${reference.replace(ROOT + '/', '')} (${region.width}×${region.height})`); }
      else if (!existsSync(reference)) failures.push(`no reference picture ${reference.replace(ROOT + '/', '')}: node scripts/smoke.mjs ${host} --record-canvas`);
      else {
        const d = diff(region, decodePng(readFileSync(reference)));
        check(d.differing <= 0.01, `the canvas differs from its reference: ${(d.differing * 100).toFixed(2)}% of pixels beyond the band, mean ${d.mean.toFixed(2)} (${d.size})`);
        console.log(`${host} readback: the canvas matches its reference — ${(d.differing * 100).toFixed(2)}% beyond the band, mean ${d.mean.toFixed(2)} (${d.size})`);
      }

      await f.tap('sky-zoom');
      const st = await f.state();
      check(st.slots.zoom === 2, `a tap on a button inside the canvas did not reach it (zoom ${JSON.stringify(st.slots.zoom)})`);
      await f.type('sky-label', 'aurora');
      t = await f.tree();
      check(byTestId(t, 'sky-label')?.props.value === 'aurora', `typing into an input inside the canvas left it at ${JSON.stringify(byTestId(t, 'sky-label')?.props.value)}`);
      if (apple) {
        await sleep(100);
        logs += JSON.stringify(await f.logs());
        const captures = (logs.match(/canvas \d+: captured/g) ?? []).length;
        check(captures >= 3, `the presenter captured the canvas ${captures} time(s); expected the first, the tap's batch, and the edit`);
        console.log(`${host} fixture: the canvas captured ${captures} times`);
      }
    } finally {
      await f.close();
    }
  }
  rmSync(tmp, { recursive: true, force: true });
}

// 11. The deck and the materials (LLP 1014 §1a, §1b): `Deck` opens a canvas
// whose cards are the kernel's buttons — placed by the surface on macOS, a
// column over the surface on the web and on Linux (D2) — and a tap on a
// card focuses it. On macOS the placements are settled before every reply
// (§1c): at clock 0 the deck is closed, its cards placed near the top of the
// canvas, and the canvas's middle is nothing but kernel frames — a tap there
// reaches no card. Two seconds of the agent's clock move the springs. A
// material button switches the sky.
{
  const d = await open({ host });
  try {
    await d.tap('deck-toggle');
    let st = await d.state();
    check(st.slots.deck === true, `the deck did not open (${JSON.stringify(st.slots.deck)})`);
    let l = await d.layout();
    const cards = l.nodes.filter((n) => n.testId?.startsWith('card-'));
    check(cards.length >= 2, `the deck has ${cards.length} card(s)`);
    if (apple) {
      const deck = box(l, 'deck');
      const onCanvas = cards.filter((c) => c.x < deck.x + deck.w && c.x + c.w > deck.x);
      check(onCanvas.length > 0 && onCanvas.length < cards.length, `${onCanvas.length} of ${cards.length} cards placed on the canvas (a closed deck shows a few; the rest are off it)`);
      const low = onCanvas.filter((c) => c.y + c.h > deck.y + deck.h / 2);
      check(low.length === 0, `${low.length} card(s) placed in the lower half of a closed deck: ${low.slice(0, 2).map((c) => `${c.testId} y=${c.y} h=${c.h}`).join(', ')}`);
      await d.tap('deck');
      st = await d.state();
      check(st.slots.focus === null, `a tap on the canvas's middle, outside every placed card, focused ${JSON.stringify(st.slots.focus)} — a kernel frame was hit`);
    }
    // The front card: a tap lands on the middle of a card's box as seen, and
    // on a closed deck every card but the first is mostly behind the one in
    // front of it, which is what the tap then reaches (the browser's rule).
    await d.tap(cards[0].testId);
    st = await d.state();
    check(st.slots.focus === cards[0].testId.slice(5), `a tap on ${cards[0].testId} focused ${JSON.stringify(st.slots.focus)}`);
    // One frame of the clock moves the springs, and the reply carries the
    // new placements (LLP 1014.000 §1c: `clock` settles the canvases — the
    // nested deck is read back into the sky's capture before the reply, not
    // when a later redraw happens to ask).
    await d.clock('+100');
    l = await d.layout();
    if (apple) {
      const later = l.nodes.filter((n) => n.testId?.startsWith('card-'));
      const moved = later.filter((c, i) => cards[i] && Math.abs(c.y - cards[i].y) > 1).length;
      check(moved > 0, 'a tenth of a second on, no card moved: the deck was not read back for the clock (placements refresh late)');
    }
    await d.tap('material-crt');
    st = await d.state();
    check(st.slots.material === 'crt', `the material is ${JSON.stringify(st.slots.material)}`);
    console.log(`${host} deck: ${cards.length} cards, ${cards[0].testId} focused by a tap; material crt`);
  } finally {
    await d.close();
  }
}

// 11. Motion under the clock (LLP 1012 §2, the motion fixture): three boxes
// scale 1 → 2 — a 250 ms linear transition and a spring on a press, a 500 ms
// linear transition when a timer fires at t = 1000. Nothing plays between
// operations; a seek lands on the curve; `settle` is a fixed point that
// crosses the timer; and one seek across the timer gives what stepping
// across it gives (LLP 1002 D3) — on both hosts, the same numbers.
{
  const tmp = mkdtempSync(resolve(tmpdir(), 'exact-smoke-'));
  const plan = resolve(tmp, 'motion.plan');
  const c = spawnSync('cargo', ['run', '-q', '--release', '-p', 'contract', '--', 'build', resolve(ROOT, 'contract/corpus/motion.contract'), '-o', plan], { cwd: ROOT, encoding: 'utf8' });
  if (c.status !== 0) failures.push('the motion fixture did not compile: ' + c.stderr);
  else {
    const w = (l, id) => box(l, id)?.w;
    const near = (a, b, tol = 0.05) => Math.abs(a - b) <= tol;
    const m = await open({ host, plan });
    try {
      await m.tap('toggle');
      // Frozen: the press started two transitions; a wall-clock pause between
      // two reads changes nothing (this sleep tests that nothing moves — it
      // is not a wait for anything).
      const a = await m.layout();
      await sleep(300);
      const b = await m.layout();
      check(w(a, 'linear') === 50 && w(a, 'spring') === 50 && w(b, 'linear') === 50 && w(b, 'spring') === 50, `after a press the boxes sit at local time 0: ${w(a, 'linear')}, ${w(a, 'spring')} then ${w(b, 'linear')}, ${w(b, 'spring')}`);
      await m.clock('+125');
      let l = await m.layout();
      check(w(l, 'linear') === 75, `at 125 ms of a 250 ms linear scale 1→2 the box is ${w(l, 'linear')} wide, not 75`);
      check(near(w(l, 'spring'), 86.55, 0.5), `at 125 ms the spring(180, 12, 1) box is ${w(l, 'spring')} wide (both hosts: 86.55)`);
      check(w(l, 'timed') === 50, `the timer has not fired yet: ${w(l, 'timed')}`);
      await m.clock('+125');
      l = await m.layout();
      check(w(l, 'linear') === 100, `at 250 ms the linear box is ${w(l, 'linear')} wide, not 100`);
      const settled = await m.clock('settle');
      l = await m.layout();
      // Two rounds: the spring settles at 1295.8 ms (the same on both hosts), the
      // seek there crosses the timer at 1000, whose transition ends at 1500.
      check(settled.settled === true && settled.clock === 1500, `settle: ${JSON.stringify(settled)} (expected a fixed point at 1500: the spring's 1295.8, then the timer's transition)`);
      check(w(l, 'spring') === 100 && w(l, 'timed') === 100, `after settle the spring box is ${w(l, 'spring')} and the timer's ${w(l, 'timed')}; both should be 100`);
    } finally {
      await m.close();
    }
    // One seek across the timer versus stepping across it: the transition is
    // born at the timer's due time either way.
    const once = await open({ host, plan });
    let oneShot;
    try { await once.clock(1250); oneShot = [w(await once.layout(), 'timed')]; await once.clock(1500); oneShot.push(w(await once.layout(), 'timed')); } finally { await once.close(); }
    const steps = await open({ host, plan });
    let stepwise;
    try { await steps.clock(1000); stepwise = [w(await steps.layout(), 'timed')]; await steps.clock(1250); stepwise.push(w(await steps.layout(), 'timed')); await steps.clock(1500); stepwise.push(w(await steps.layout(), 'timed')); } finally { await steps.close(); }
    check(oneShot[0] === 75 && oneShot[1] === 100, `one seek to 1250 then 1500 across the timer: ${oneShot.join(', ')} (expected 75, 100)`);
    check(stepwise[0] === 50 && stepwise[1] === 75 && stepwise[2] === 100, `stepping 1000, 1250, 1500: ${stepwise.join(', ')} (expected 50, 75, 100)`);
    console.log(`${host} motion: linear 75 at 125 ms, spring in flight, settle a fixed point; across the timer one seek = steps (${oneShot.join('/')} vs ${stepwise.slice(1).join('/')})`);
  }
  rmSync(tmp, { recursive: true, force: true });
}

// 12. The page's environment (LLP 1008 §9, the insets fixture): a root that
// says `viewport-fit="cover"` is laid out to the whole screen, its content
// kept out of the safe areas by `env(safe-area-inset-*)` lengths — on a
// phone the viewport is the app's (step 2, the safe area) plus the insets;
// everywhere else the insets are zero and nothing moves. Focusing the input
// at the bottom: on iOS the software keyboard rises, the viewport insets
// itself by the keyboard's height and reveals the field above it, the layout
// viewport untouched — a browser's visual viewport; a tap on the dismiss
// button takes the focus, and the keyboard goes. `layout.env` reports both,
// by the web's `env()` names, on every host.
{
  const tmp = mkdtempSync(resolve(tmpdir(), 'exact-smoke-'));
  const plan = resolve(tmp, 'insets.plan');
  const c = spawnSync('cargo', ['run', '-q', '--release', '-p', 'contract', '--', 'build', resolve(ROOT, 'contract/corpus/insets.contract'), '-o', plan], { cwd: ROOT, encoding: 'utf8' });
  if (c.status !== 0) failures.push('the insets fixture did not compile: ' + c.stderr);
  else {
    const f = await open({ host, plan });
    try {
      let l = await f.layout();
      const env = l.env ?? {};
      const names = ['safe-area-inset-top', 'safe-area-inset-right', 'safe-area-inset-bottom', 'safe-area-inset-left', 'keyboard-inset-height'];
      check(names.every((k) => typeof env[k] === 'number'), `layout.env is ${JSON.stringify(l.env)}`);
      const [top, right, bottom, left] = names.map((k) => env[k] ?? 0);
      const viewport0 = l.viewport;
      const rootBox = box(l, 'root'), content = box(l, 'content');
      check(rootBox && rootBox.w === l.viewport.w && rootBox.h === l.viewport.h, `a cover root fills the viewport: ${JSON.stringify(rootBox)} in ${JSON.stringify(l.viewport)}`);
      check(content && content.x === left && content.y === top && Math.abs(content.w - (l.viewport.w - left - right)) < 0.01 && Math.abs(content.h - (l.viewport.h - top - bottom)) < 0.01, `the content keeps out of the insets: ${JSON.stringify(content)} for env ${JSON.stringify(env)} in ${JSON.stringify(l.viewport)}`);
      check(appViewport && Math.abs(l.viewport.h - (appViewport.h + top + bottom)) < 0.01 && Math.abs(l.viewport.w - (appViewport.w + left + right)) < 0.01, `a cover root's viewport is the app's plus the insets: ${JSON.stringify(l.viewport)} vs ${JSON.stringify(appViewport)} + ${top}/${right}/${bottom}/${left}`);
      if (host === 'ios') check(top > 0 && bottom > 0, `a phone reports its status bar and home indicator: ${top}, ${bottom}`);
      else check(top === 0 && right === 0 && bottom === 0 && left === 0, `no safe area here: ${JSON.stringify(env)}`);
      // The keyboard: typing focuses the field at the bottom.
      const noteBefore = box(l, 'note');
      await f.type('note', 'hi');
      let st = await f.state();
      // (The Linux host's `type` sets the field without focusing it, as step 4a notes.)
      check(st.slots.note === 'hi' && (host === 'linux' || st.slots.focused === true), `typing focused the field and set it: ${JSON.stringify(st.slots)}`);
      let kb = 0;
      for (let i = 0; i < 40; i++) { l = await f.layout(); kb = l.env['keyboard-inset-height']; if (host !== 'ios' || kb > 0) break; await sleep(50); }
      const note = box(l, 'note');
      check(l.viewport.h === viewport0.h && box(l, 'root').h === rootBox.h, `the layout viewport does not change for a keyboard: ${JSON.stringify(l.viewport)}, root ${JSON.stringify(box(l, 'root'))}`);
      if (host === 'ios') {
        check(kb > 100, `the software keyboard rose on the simulator: keyboard-inset-height ${kb} (Simulator › I/O › Keyboard › Connect Hardware Keyboard hides it)`);
        check(note.y + note.h <= l.viewport.h - kb + 0.01 && note.y < noteBefore.y, `the field is revealed above the keyboard: ${JSON.stringify(note)} under a keyboard of ${kb} in ${l.viewport.h}; before ${JSON.stringify(noteBefore)}`);
        console.log(`${host} insets: safe area ${top}/${right}/${bottom}/${left}, the viewport ${l.viewport.w}×${l.viewport.h}; the keyboard ${kb} revealed the field at y ${note.y} (was ${noteBefore.y})`);
      } else {
        check(kb === 0 && note.y === noteBefore.y, `no software keyboard here: keyboard-inset-height ${kb}, the field at ${note.y} (was ${noteBefore.y})`);
        console.log(`${host} insets: env ${JSON.stringify(env)}; the viewport ${l.viewport.w}×${l.viewport.h}; no keyboard`);
      }
      // Dismiss: the button takes the focus (it has a focus handler), the
      // field blurs, the keyboard goes, and the field is where it was.
      await f.tap('dismiss');
      st = await f.state();
      check(st.slots.focused === false, `the dismiss button took the focus: ${JSON.stringify(st.slots)}`);
      for (let i = 0; i < 40; i++) { l = await f.layout(); if (l.env['keyboard-inset-height'] === 0) break; await sleep(50); }
      check(l.env['keyboard-inset-height'] === 0 && box(l, 'note').y === noteBefore.y, `after the keyboard went the field is back: keyboard ${l.env['keyboard-inset-height']}, the field at ${box(l, 'note').y} (was ${noteBefore.y})`);
    } finally {
      await f.close();
    }
  }
  rmSync(tmp, { recursive: true, force: true });
}

// 13. `interactive-widget="resizes-content"` (LLP 1008 §9, the keyboard-bar
// fixture): the layout viewport ends at the keyboard's top, so a bar pinned
// to the bottom of the root rises with it and the bottom safe-area inset is
// the keyboard's — zero — while it is up; the dismiss brings everything back.
// Where no keyboard exists nothing moves.
{
  const tmp = mkdtempSync(resolve(tmpdir(), 'exact-smoke-'));
  const plan = resolve(tmp, 'keyboard-bar.plan');
  const c = spawnSync('cargo', ['run', '-q', '--release', '-p', 'contract', '--', 'build', resolve(ROOT, 'contract/corpus/keyboard-bar.contract'), '-o', plan], { cwd: ROOT, encoding: 'utf8' });
  if (c.status !== 0) failures.push('the keyboard-bar fixture did not compile: ' + c.stderr);
  else {
    const f = await open({ host, plan });
    try {
      let l = await f.layout();
      const viewport0 = l.viewport, bottom0 = l.env['safe-area-inset-bottom'];
      const bar0 = box(l, 'bar');
      check(bar0 && Math.abs(bar0.y + bar0.h - (l.viewport.h - bottom0)) < 0.01, `the bar sits on the bottom inset: ${JSON.stringify(bar0)} in ${JSON.stringify(l.viewport)}, inset ${bottom0}`);
      await f.type('note', 'hi');
      let kb = 0;
      for (let i = 0; i < 40; i++) { l = await f.layout(); kb = l.env['keyboard-inset-height']; if (host !== 'ios' || kb > 0) break; await sleep(50); }
      const bar = box(l, 'bar');
      if (host === 'ios') {
        check(kb > 100, `the software keyboard rose: keyboard-inset-height ${kb}`);
        check(Math.abs(l.viewport.h - (viewport0.h - kb)) < 0.01 && box(l, 'root').h === l.viewport.h, `the layout viewport ends at the keyboard: ${JSON.stringify(l.viewport)} (was ${JSON.stringify(viewport0)}, keyboard ${kb}), root ${JSON.stringify(box(l, 'root'))}`);
        check(l.env['safe-area-inset-bottom'] === 0, `the bottom inset is the keyboard's while it is up: ${l.env['safe-area-inset-bottom']}`);
        check(bar && Math.abs(bar.y + bar.h - l.viewport.h) < 0.01 && bar.y < bar0.y, `the bar rides on the keyboard: ${JSON.stringify(bar)} in ${l.viewport.h} (was ${JSON.stringify(bar0)})`);
        console.log(`${host} keyboard bar: the viewport ${viewport0.h} → ${l.viewport.h} under a keyboard of ${kb}; the bar's bottom ${bar0.y + bar0.h} → ${bar.y + bar.h}`);
      } else {
        check(kb === 0 && l.viewport.h === viewport0.h && bar.y === bar0.y, `no software keyboard here: ${JSON.stringify(l.viewport)}, the bar at ${bar.y} (was ${bar0.y})`);
      }
      await f.tap('dismiss');
      for (let i = 0; i < 40; i++) { l = await f.layout(); if (l.env['keyboard-inset-height'] === 0) break; await sleep(50); }
      check(l.viewport.h === viewport0.h && l.env['safe-area-inset-bottom'] === bottom0 && box(l, 'bar').y === bar0.y, `after the keyboard went everything is back: ${JSON.stringify(l.viewport)}, inset ${l.env['safe-area-inset-bottom']}, the bar at ${box(l, 'bar').y} (was ${bar0.y})`);
      // A tap on plain text — nothing focusable, nothing pressable — blurs
      // the field, as a tap on a page's ground does, and the keyboard goes
      // (the web and iOS; the Linux host's `type` never focused).
      if (host !== 'linux') {
        await f.type('note', 'again');
        for (let i = 0; i < 40; i++) { l = await f.layout(); if (host !== 'ios' || l.env['keyboard-inset-height'] > 0) break; await sleep(50); }
        await f.tap('title');
        let st = await f.state();
        for (let i = 0; i < 40; i++) { l = await f.layout(); if (l.env['keyboard-inset-height'] === 0) break; await sleep(50); }
        check(st.slots.focused === false && l.env['keyboard-inset-height'] === 0 && l.viewport.h === viewport0.h, `a tap on the title blurred the field and sent the keyboard away: ${JSON.stringify(st.slots)}, keyboard ${l.env['keyboard-inset-height']}, viewport ${JSON.stringify(l.viewport)}`);
      }
    } finally {
      await f.close();
    }
  }
  rmSync(tmp, { recursive: true, force: true });
// 12. The app's own tests (LLP 1017 P7): `apps/caltrain/app.test.contract`,
// its `test` blocks driven through a fresh session by the same operations.
{
  const t = await runTests({ host, file: resolve(ROOT, 'apps/caltrain/app.test.contract') });
  for (const r of t.results) for (const f of r.failures) check(false, `test "${r.name}": ${f}`);
  console.log(`${host} tests: ${t.passed} passed, ${t.failed} failed (app.test.contract)`);
}

console.log(`${host} smoke: ${failures.length ? `${failures.length} failure(s)` : 'ok'} in ${((Date.now() - t0) / 1000).toFixed(1)} s`);
if (failures.length) { for (const f of failures) console.error('  ' + f); process.exit(1); }
