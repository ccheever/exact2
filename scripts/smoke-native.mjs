// The native-module seam, driven (LLP 1024 D8.3) — `smoke.mjs` runs this for
// the fixture app: `bun scripts/smoke.mjs <web|macos|ios> --app native-fixture`.
// Every assertion is through the eight operations: `tree` shows the status
// object through loading → ready; props replace (several reactive keys,
// clearing, canonical order, escaping); all nine events arrive with their
// payloads; a callback after `destroy` is dropped; loading starts only after
// first pixel; a plan reload does not re-define the element (web); capture
// shows the boxes and the tokened snapshot answers (Apple); and the failure
// family — missing artifact, missing factory, wrong ABI, refused props —
// each yields its named status, an empty box, a log line and a running app.
// Then the hatches (LLP 1075.003): data-* words, the authored header under the
// agent, the hatches' journal on iOS, and a push and its Back.
import { spawnSync } from 'node:child_process';
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { jsTargetBuild, serveBuildTree, serveStatic } from '../host/web/serve.mjs';
import { decodePng } from './png.mjs';

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const EXPECTED_EVENTS = 'press;change:changed;hover:true;focus;blur;key:Enter;submit;message:hello;';
const NOTE = 'quote " slash \\ tab\t<&>';

export async function nativeSmoke({ host, open, check: record, webDist, shots }) {
  let checks = 0, failed = 0;
  const check = (ok, what) => { checks += 1; if (!ok) failed += 1; return record(ok, what); };
  const t0 = Date.now();
  const byTestId = (t, id) => t.nodes.find((n) => n.props.testId === id);
  const module = (t, id) => byTestId(t, id)?.module;
  const tmp = mkdtempSync(resolve(tmpdir(), 'exact-native-'));
  const settle = async (s) => { await s.clock('+50'); await sleep(host === 'web' ? 50 : 120); await s.clock('+50'); };
  const until = async (s, what, test, ms = 5000) => {
    const end = Date.now() + ms;
    let last;
    while (Date.now() < end) { last = await s.tree(); if (test(last)) return last; await sleep(40); }
    check(false, `${host} native: ${what}: ${JSON.stringify(last?.nodes.filter((n) => n.type === 'NativeView').map((n) => n.module))}`);
    return last;
  };
  const received = (st) => { const r = st.slots.received; return r.startsWith('props:') ? JSON.parse(r.slice(6)) : null; };
  const pixel = async (s, path, testId) => {
    const shot = await s.screenshot(path);
    const png = decodePng(readFileSync(path));
    const box = (await s.layout()).nodes.find((n) => n.testId === testId);
    if (!box) return null;
    const scale = png.width / shot.w, x = Math.round((box.x + box.w / 2) * scale), y = Math.round((box.y + box.h / 2) * scale);
    const i = (y * png.width + x) * 4;
    return [png.data[i], png.data[i + 1], png.data[i + 2]];
  };
  const near = (rgb, hex) => rgb && [1, 3, 5].every((o, k) => Math.abs(rgb[k] - parseInt(hex.slice(o, o + 2), 16)) <= 12);

  // The standard artifact.
  const s = await open({ host });
  try {
    let t = await until(s, 'the fixture and the plain box reach ready', (t) => module(t, 'box')?.state === 'ready' && module(t, 'plain')?.state === 'ready');
    let logs = await s.logs();
    const lines = logs.lines.join('\n');
    check(/exact-fixture #\d+: loading[\s\S]*exact-fixture #\d+: ready/.test(lines), `${host} native: the log shows loading then ready: ${logs.lines.filter((l) => /native/.test(l)).join(' | ')}`);
    // Missing factory: in the roster, not in the artifact.
    check(module(t, 'absent')?.state === 'error' && /no factory for exact-absent/.test(module(t, 'absent')?.error ?? ''), `${host} native: exact-absent reports its missing factory: ${JSON.stringify(module(t, 'absent'))}`);
    check(/exact-absent #\d+: error: the module artifact has no factory/.test(lines), `${host} native: the missing factory is logged`);
    // After first pixel, never before: the browser's paint entry, or, when
    // headless Chrome records none, the glue's first-frame stamp (a lower
    // bound). On the JS target the adapter is the native.js chunk that holds
    // native-glue.js, and where Chrome records no entry the runtime's gate is
    // two frames and 250 ms after its first commit (`bootMs`, rt.js `painted`).
    if (host === 'web') {
      const order = await s.carrier.evaluate(jsTargetBuild(webDist)
        ? `(() => { const at = (re) => performance.getEntriesByType('resource').find((e) => re.test(e.name))?.startTime; const entry = performance.getEntriesByType('paint')[0]?.startTime; return { paint: entry ?? Number(document.getElementById('exact-root').dataset.bootMs) + 250, entry: entry != null, glue: at(/\\/native-[\\w-]+\\.js$/), table: at(/\\/modules\\/index\\.js$/) }; })()`
        : `(() => { const glueEnd = performance.getEntriesByType('resource').find((e) => e.name.endsWith('/glue.js'))?.responseEnd ?? 0; const paint = performance.getEntriesByType('paint')[0]?.startTime ?? (glueEnd + Number(document.getElementById('exact-root').dataset.frameCallbackMs ?? NaN)); const glue = performance.getEntriesByType('resource').find((e) => e.name.endsWith('/native-glue.js'))?.startTime; const table = performance.getEntriesByType('resource').find((e) => e.name.endsWith('/modules/index.js'))?.startTime; return { paint, glue, table }; })()`);
      check(Number.isFinite(order.paint) && order.glue > order.paint && order.table > order.glue, `${host} native: the adapter and the module load after first paint: ${JSON.stringify(order)}`);
    } else {
      const m = /native loading .*libexact_modules\.dylib (-?[\d.]+) ms after first pixel/.exec(lines);
      check(m && Number(m[1]) >= 0, `${host} native: the artifact loads after first pixel: ${m?.[0] ?? 'no load line'}`);
    }
    // A module's natural content size uses ordinary CSS layout: decoration
    // adds to the content box, growth moves a sibling, and clearing forgets it.
    const dimensions = async () => {
      const nodes = (await s.layout()).nodes;
      return Object.fromEntries(['plain', 'absent', 'box'].map(id => [id, nodes.find(n => n.testId === id)]));
    };
    await settle(s);
    const natural = await dimensions();
    check(natural.plain?.w === 132 && natural.plain?.h === 44, `${host} native: preferred 120×32 plus padding/border: ${JSON.stringify(natural.plain)}`);
    check(natural.box?.w === 120 && natural.box?.h === 80, `${host} native: authored dimensions override the reported preference`);
    await s.tap('grow'); await settle(s);
    const grown = await dimensions();
    check(grown.plain?.h === 76 && grown.absent?.y - natural.absent?.y === 32, `${host} native: content growth moves the next sibling by 32`);
    await s.tap('sizing'); await settle(s);
    const cleared = await dimensions();
    check(cleared.plain?.h === 12 && cleared.plain?.w === 12, `${host} native: clearing the preference leaves only padding/border`);
    await s.tap('sizing'); await s.tap('grow'); await settle(s);
    // Props: the canonical aggregate the plan carries, and what the module got.
    const canonical = `{"count":"0","emit":"0","note":${JSON.stringify(NOTE)},"reject":"false","tint":"#2266ee"}`;
    check(byTestId(t, 'box')?.props.nativeViewProps === canonical, `${host} native: the plan's aggregate is sorted and escaped: ${byTestId(t, 'box')?.props.nativeViewProps}`);
    let st = await s.state();
    check(JSON.stringify(received(st)) === canonical, `${host} native: the module received the aggregate: ${st.slots.received}`);
    check(st.slots.loads === 1, `${host} native: load fired once at create: ${st.slots.loads}`);
    await s.tap('bump'); await s.tap('mode'); await settle(s);
    st = await s.state();
    check(received(st)?.count === '1' && received(st)?.mode === 'wide', `${host} native: two reactive keys replaced the aggregate: ${st.slots.received}`);
    await s.tap('mode'); await settle(s);
    st = await s.state();
    check(received(st) && !('mode' in received(st)) && received(st).count === '1', `${host} native: a none clears its key: ${st.slots.received}`);
    // All nine events, from a background source, in order.
    await s.tap('fire'); await settle(s);
    st = await s.state();
    check(st.slots.events === EXPECTED_EVENTS, `${host} native: nine events with payloads: ${JSON.stringify(st.slots.events)}`);
    check(st.slots.loads === 2, `${host} native: load arrived as an event: ${st.slots.loads}`);
    // The module view takes no hits: the agent's tap is the node's press.
    await s.tap('box'); await settle(s);
    st = await s.state();
    // (A browser's real pointer also hovers and focuses the element first.)
    check(st.slots.events.startsWith(EXPECTED_EVENTS) && st.slots.events.endsWith('press;'), `${host} native: a tap on the box is the node's press: ${JSON.stringify(st.slots.events)}`);
    // The hatches (LLP 1075.003): the route's data-* words, the authored header
    // under the agent (one presentation, LLP 1021 D4), the hatches' moments in
    // the journal (iOS; macOS projects no routes; the web's are checked
    // below), and a push and its Back.
    t = await s.tree();
    const words = host === 'web'
      ? await s.carrier.evaluate(`JSON.stringify((({ menu, screen, transition, violate }) => ({ menu, screen, transition, violate }))(document.querySelector('[data-testid="route-home"]').dataset))`)
      : byTestId(t, 'route-home')?.props.dataset;
    check(words === '{"menu":"compose","screen":"home","transition":"false","violate":"false"}', `${host} native: the route carries its data-* words: ${words}`);
    const header = (await s.layout()).nodes.find((n) => n.testId === 'header-home');
    check(header?.h > 0, `${host} native: the agent sees the authored header: ${JSON.stringify(header)}`);
    // `logs` reads on from where it last stopped: the journal is both reads.
    logs = await s.logs();
    const journal = `${lines}\n${logs.lines.join('\n')}`;
    if (host === 'ios') {
      check(/hatch: connected/.test(journal) && /hatch navigation #1: built[\s\S]*hatch route \d+: built/.test(journal), `${host} native: a stack's hatch runs before its routes': ${journal.split('\n').filter((l) => /hatch/.test(l)).join(' | ')}`);
    } else if (host === 'macos') {
      // macOS projects no routes; the web's page module has its own (below).
      check(!/hatch (navigation|route)/.test(journal), `${host} native: no route hatch runs on macOS: ${logs.lines.filter((l) => /hatch/.test(l)).join(' | ')}`);
    }
    await s.tap('compose-home'); await settle(s);
    check((await s.state()).slots.composed === 1, `${host} native: the authored Compose runs the handler a bar item presses`);
    // The window toolbar's hatch (macOS, LLP 1075.003.000 §3.7): its display
    // mode and an item of the app's after Exact's, whose own items still press.
    if (host === 'macos') {
      await s.clock('settle');
      const bar = (await s.state()).window?.toolbar;
      check(bar?.installed && bar.displayMode === 1 && bar.appItems?.includes('fixture.hatched') && bar.items?.at(-1) === 'fixture.hatched',
        `${host} native: the toolbar hatch sets the display mode and adds an item after Exact's: ${JSON.stringify(bar)}`);
      check(bar?.appEnabled?.every(Boolean), `${host} native: the app's own toolbar item is enabled (its target validates it): ${JSON.stringify(bar?.appEnabled)}`);
      await s.tap('toolbar-compose'); await settle(s);
      check((await s.state()).slots.composed === 2, `${host} native: Exact's toolbar item still presses its command`);
    }
    await s.tap('detail'); await settle(s);
    t = await until(s, 'the detail route is pushed', (t) => !!byTestId(t, 'route-detail'));
    // Hatched nodes (LLP 1075.003.000): the tree shows each word, the hatch
    // hears a node's mount and its data-* change (`state` counts them; on the
    // web the fixture's page module does), and a development build journals a
    // write to what Exact owns of one (iOS).
    const webCalls = async () => JSON.parse(await s.carrier.evaluate('JSON.stringify(globalThis.exactFixtureHatches ?? {})'));
    const hatchCalls = async (word) => host === 'web'
      ? Object.fromEntries(Object.entries(await webCalls()).filter(([k]) => k.startsWith(word + ':')).map(([k, v]) => [k.slice(word.length + 1), v]))
      : (await s.state()).hatches?.words?.[word]?.calls ?? {};
    {
      await s.clock('settle');
      check(byTestId(await s.tree(), 'hatched-badge')?.props.hatch === 'badge', `${host} native: the tree shows a node's hatch word`);
      const badge = await hatchCalls('badge');
      check(badge.built === 1 && badge.changed === 1, `${host} native: a hatched node is built, and its data-* change reaches its hatch: ${JSON.stringify(badge)}`);
      // The host's own count of the calls (`state.hatches`), every host alike.
      const counted = (await s.state()).hatches?.words?.badge;
      check(counted?.calls?.built === 1 && counted.calls.changed === 1 && counted.live === 1, `${host} native: state.hatches counts the hatch's calls: ${JSON.stringify(counted)}`);
      // What the hatch said of itself, and what Exact timed (LLP
      // 1075.003.000.001 §3.1–3.3; the web first, §8 stage 1): its counters
      // and snapshot in `state`, its line in `logs`, its calls in `perf
      // hatches`. Reads are cumulative: a second read is the same.
      if (host === 'web') {
        check(counted?.counters?.built === 1 && counted.counters.changed === 1 && counted.published?.tone?.tone === 'busy',
          `${host} native: state.hatches carries the hatch's counters and its snapshot: ${JSON.stringify(counted)}`);
        const said = (await s.op({ op: 'logs', since: 0 })).lines.filter((l) => /hatch element badge: /.test(l)); // the whole journal: an earlier read took `built`
        check(said.some((l) => /hatch element badge: built, tone info/.test(l)) && said.some((l) => /hatch element badge: changed, tone busy/.test(l)),
          `${host} native: a hatch's log lines reach the journal under its scope: ${said.join(' | ')}`);
        const perf = await s.op({ op: 'perf', hatches: true });
        const timed = perf.hatches?.['element badge'], built = perf.calls?.find((c) => c.hatch === 'element badge' && c.moment === 'built');
        check(perf.measuring && timed?.calls === 2 && timed.ms >= 0 && timed.worst <= timed.ms && built?.calls === 1 && perf.counters?.['element badge']?.built === 1 && perf.plan,
          `${host} native: perf hatches times each call and carries the counters: ${JSON.stringify(perf)}`);
        const again = await s.op({ op: 'perf', hatches: true });
        check(JSON.stringify(again.calls) === JSON.stringify(perf.calls) && JSON.stringify(again.counters) === JSON.stringify(perf.counters), `${host} native: a perf hatches read changes nothing`);
      }
      if (host === 'ios') {
        await s.tap('violate'); await settle(s);
        await s.tap('violate'); await settle(s);
        const owned = (await s.logs()).lines.join('\n');
        check(/element detail-list #\d+: contentInset changed outside Exact, which owns it/.test(owned), `${host} native: the development check covers hatched nodes`);
      }
    }
    // An authored scrollTop lands as the browser's (LLP 1075.003 §3.7).
    await s.tap('scroll-80'); await settle(s);
    const scrolled = (await s.layout()).nodes.find((n) => n.testId === 'list-detail');
    check(scrolled?.sy === 80, `${host} native: scrollTop 80 is an offset of 80: ${JSON.stringify(scrolled)}`);
    await s.tap('scroll-0'); await settle(s);
    await s.tap('back'); await settle(s);
    t = await until(s, 'Back pops the detail route', (t) => !byTestId(t, 'route-detail'));
    if (host === 'ios') {
      logs = await s.logs();
      check(logs.lines.some((l) => /hatch route \d+: ended/.test(l)), `${host} native: a popped route's hatch hears routeEnded`);
    }
    // A list whose rows each hold a hatched node: the journal names what the
    // word gives up and warns for it in a row; a retired row's hatch hears
    // `ended`; on iOS no row holding one is reused (LLP 1075.003.000 §3.3).
    {
      const takes = (await s.state()).pool?.takes;
      await s.tap('rows'); await settle(s);
      t = await until(s, 'the hatched list shows rows', (t) => !!byTestId(t, 'row-1'));
      await s.clock('settle');
      check((await hatchCalls('dot')).built > 0, `${host} native: each shown row's node is hatched`);
      await s.tap('hatched-list', { wheel: [0, 4000] }); await settle(s); await s.clock('settle');
      const dot = await hatchCalls('dot');
      check(dot.ended > 0, `${host} native: a retired row's hatch hears ended: ${JSON.stringify(dot)}`);
      if (host === 'ios') check((await s.state()).pool?.takes === takes, `${host} native: a row holding a hatched node is never reused: ${takes} → ${(await s.state()).pool?.takes}`);
      const said = (await s.logs()).lines.join('\n');
      const gave = host === 'ios' ? /hatch element dot: a view, not a flat leaf; its row is not reused/ : /hatch element dot: nothing beyond the call/;
      check(gave.test(said) && /hatch element dot is in a row of list hatched-list/.test(said), `${host} native: the journal says what a hatched node gives up: ${said.split('\n').filter((l) => /hatch element dot/.test(l)).slice(0, 3).join(' | ')}`);
      // A hatch that undoes what it adds says so (`reusable`, LLP
      // 1075.003.000.000 §8): its rows are reused again. The live rows read
      // the component's state the button flips, so their hatches hear it now.
      if (host === 'ios') {
        await s.tap('reuse'); await settle(s); await s.clock('settle');
        const live = (await s.tree()).nodes.filter((n) => n.props.hatch === 'dot');
        check(live.length > 0 && live.every((n) => n.props.dataset === '{"reuse":"true"}'), `${host} native: the live rows re-read the state the button flips: ${live.length} rows, ${[...new Set(live.map((n) => n.props.dataset))]}`);
        const from = (await s.state()).pool?.takes;
        await s.tap('hatched-list', { wheel: [0, -4000] }); await settle(s); await s.clock('settle');
        const st = await s.state(), freed = (await s.logs()).lines.join('\n');
        check(st.pool?.takes > from && st.hatches?.words?.dot?.reusable > 0 && /hatch element dot: its hatch undoes what it adds; its row is reused/.test(freed),
          `${host} native: a reusable hatch's rows are reused: takes ${from} → ${st.pool?.takes}, ${JSON.stringify(st.hatches?.words?.dot)}`);
      }
      await s.tap('back'); await settle(s);
      t = await until(s, 'Back pops the rows route', (t) => !byTestId(t, 'hatched-list'));
    }
    // The page module's container hatches (LLP 1075.003.000 §3.7): the root,
    // its tablist, each route as it mounts and as it leaves.
    if (host === 'web') {
      await s.clock('settle');
      const c = await webCalls();
      check(c['navigation:built'] === 1 && c['tabs:built'] === 1 && c['route:built'] >= 3 && c['route:ended'] >= 2,
        `${host} native: the page module's container hatches run as routes mount and leave: ${JSON.stringify(c)}`);
    }
    // A sheet over the tabs, and its Close (LLP 1075.003 §3.7, from James's review).
    // (macOS projects no routes: there the sheet is its route, shown.)
    const sheetShown = async () => host === 'macos' ? !!byTestId(await s.tree(), 'route-sheet') : (await s.state()).navigation?.presentation === 'modal';
    await s.tap('sheet'); await settle(s);
    check(await sheetShown(), `${host} native: the sheet is presented over the tabs`);
    await s.tap('close-sheet'); await settle(s);
    check(!(await sheetShown()), `${host} native: Close dismisses the sheet`);
    // UIKit's dismissal runs in platform time, its snapshot over the tabs until it ends (LLP 1035.003 D5).
    await s.clock('settle');
    // Retained tabs (LLP 1075.003 §3.7): a tab's scroll survives a switch away and back.
    await s.tap('tab-second'); await settle(s); await s.clock('settle');
    await s.tap('list-second', { wheel: [0, 300] }); await settle(s); await s.clock('settle');
    const offset = async () => (await s.layout()).nodes.find((n) => n.testId === 'list-second')?.sy;
    const away = await offset();
    await s.tap('tab-home'); await settle(s); await s.clock('settle');
    await s.tap('tab-second'); await settle(s); await s.clock('settle');
    const kept = await offset();
    check(away > 0 && kept === away, `${host} native: a tab's scroll survives a switch: ${away} → ${kept}`);
    await s.tap('tab-home'); await settle(s);
    // Refused props: named, the last accepted kept, the app still running.
    const before = st.slots.received;
    await s.tap('reject'); await settle(s);
    t = await s.tree(); st = await s.state();
    check(module(t, 'box')?.state === 'error' && /props refused: reject=true/.test(module(t, 'box')?.error ?? ''), `${host} native: refused props are named: ${JSON.stringify(module(t, 'box'))}`);
    check(st.slots.received === before, `${host} native: the last accepted props stay active`);
    logs = await s.logs();
    check(logs.lines.some((l) => /exact-fixture #\d+: error: props refused/.test(l)), `${host} native: the refusal is logged`);
    await s.tap('bump'); await settle(s);
    check((await s.state()).slots.count === 2, `${host} native: the app runs after a refusal`);
    await s.tap('reject'); await settle(s);
    t = await s.tree();
    check(module(t, 'box')?.state === 'ready', `${host} native: an accepted replacement clears the refusal: ${JSON.stringify(module(t, 'box'))}`);
    // Capture: the boxes, and on Apple the fixture's tokened snapshot.
    const shot = resolve(shots ?? tmp, `native-fixture-${host}.png`);
    const box = await pixel(s, shot, 'box'), plain = await pixel(s, shot, 'plain'), absent = await pixel(s, shot, 'absent');
    check(near(box, '#2266ee'), `${host} native: the capture shows the fixture's colour: ${box}`);
    check(near(plain, '#11aa44'), `${host} native: the ordinary capture shows the plain box: ${plain}`);
    const plainBounds = (await s.layout()).nodes.find(n => n.testId === 'plain');
    const capture = decodePng(readFileSync(shot));
    const scale = capture.width / (await s.screenshot(shot)).w;
    const paddingPixel = (Math.round((plainBounds.y + plainBounds.h / 2) * scale) * capture.width + Math.round((plainBounds.x + 3) * scale)) * 4;
    check(near(Array.from(capture.data.slice(paddingPixel, paddingPixel + 3)), '#ffffff'), `${host} native: module content leaves the authored padding visible`);
    check(near(absent, '#f3f4f6'), `${host} native: the missing factory leaves its empty box: ${absent}`);
    if (host !== 'web') {
      logs = await s.logs();
      check(logs.lines.some((l) => /exact-fixture #\d+: snapshot token \d+, \d+ bytes/.test(l)), `${host} native: the capture asked the fixture's tokened snapshot: ${logs.lines.filter((l) => /snapshot/.test(l)).join(' | ')}`);
    }
    // Destroy: the nonce dies first; the late background callback is dropped.
    await s.tap('toggle'); await settle(s); await sleep(300); await settle(s);
    st = await s.state(); logs = await s.logs();
    check(!st.slots.events.includes('late'), `${host} native: a callback after destroy never reached the app: ${st.slots.events}`);
    check(logs.lines.some((l) => /dropped message from nonce \d+ after destroy/.test(l)), `${host} native: the dropped callback is logged: ${logs.lines.filter((l) => /native/.test(l)).join(' | ')}`);
    await s.tap('toggle');
    t = await until(s, 'a remount attaches a new instance', (t) => module(t, 'box')?.state === 'ready');
    await settle(s);
    check((await s.state()).slots.loads === 3, `${host} native: the new instance loaded`);
    // Under the agent a closed popover paints nothing and covers nothing
    // (LLP 1021 D4, as on the web and macOS): its opener's tap shows it in
    // place, a tap outside dismisses it and still presses, its hide-only
    // button closes it.
    if (host === 'ios') {
      const pop = byTestId(await s.tree(), 'native-popover');
      check(pop?.open === false, 'ios popover: closed, it does not paint in agent mode');
      await s.tap('open-native-popover'); await settle(s);
      const opened = await s.state();
      check(byTestId(await s.tree(), 'native-popover')?.open === true && opened.navigation.popover?.popover === pop?.id, `ios popover: its opener's tap shows it: ${JSON.stringify(opened.navigation.popover)}`);
      check(opened.focus.logical === byTestId(await s.tree(), 'popover-search')?.id, `ios popover: it focuses its autofocus field: ${JSON.stringify(opened.focus)}`);
      const count = opened.slots.count;
      await s.tap('bump'); await settle(s);
      const dismissed = await s.state();
      check(byTestId(await s.tree(), 'native-popover')?.open === false && dismissed.navigation.popover == null && dismissed.slots.count === count + 1, 'ios popover: a tap outside dismisses it and still presses');
      await s.tap('open-native-popover'); await settle(s);
      await s.tap('popover-close'); await settle(s);
      check(byTestId(await s.tree(), 'native-popover')?.open === false, 'ios popover: its hide-only button closes it');
    }
    if (host === 'macos') {
      const input = byTestId(await s.tree(), 'native-input').id;
      const plainId = byTestId(await s.tree(), 'plain').id;
      const focus = async () => (await s.state()).focus.logical;
      await s.tap('focus-input'); await settle(s);
      check(await focus() === input, 'macos native: focus action reaches the editable descendant');
      await s.type('native-input', 'Hello 한글'); await settle(s);
      check((await s.state()).slots.inputValue === 'Hello 한글', 'macos native: standard type replaces text through AppKit');
      await s.type('native-input', 'abc');
      await s.type('native-input', { key: 'Backspace' }); await settle(s);
      check((await s.state()).slots.inputValue === 'ab' && (await s.state()).slots.inputEvents.includes('key:Backspace;'), 'macos native: standard key reaches the editor and deletes a character');
      await s.type('native-input', { key: 'Backspace', for: 10 }); await settle(s);
      check((await s.state()).slots.inputValue === 'a', 'macos native: held key releases through the same instance');
      check((await s.state()).slots.inputEvents.split('focus;').length === 2, 'macos native: typing and keys preserve an existing editing session');
      const unsupported = await s.carrier.ask({ op: 'type', id: input, key: 'Meta+Q' });
      check(/does not support key/.test(unsupported.error ?? ''), 'macos native: unsupported key is an honest refusal');
      await s.type('before-input', { key: 'Tab' }); await settle(s);
      const tabbed = (await s.state()).focus;
      check(tabbed.logical === input && tabbed.responder === 'FixtureEditor', `macos native: Tab reaches the module's editing descendant: ${JSON.stringify(tabbed)}`);
      await s.type('native-input', { key: 'Meta+Shift+Enter' }); await settle(s);
      check((await s.state()).slots.inputCommands === 1, 'macos native: declared commands run before the module input hook');
      await s.type('native-input', { key: 'Meta+Shift+Enter', for: 10 }); await settle(s);
      check((await s.state()).slots.inputCommands === 2, 'macos native: a held host command runs once and owns its release');
      await s.type('native-input', { key: 'Meta++' }); await settle(s);
      check((await s.state()).slots.inputCommands === 3, 'macos native: literal Plus reaches the same host shortcut router');
      const typing = await s.carrier.ask({ op: 'type', id: input, key: 'c' });
      check(/does not support key/.test(typing.error ?? '') && (await s.state()).slots.inputCommands === 3, 'macos native: bare character shortcuts stay with the editor');
      const passive = await s.carrier.ask({ op: 'type', id: plainId, text: 'no' });
      check(/refused focus/.test(passive.error ?? ''), 'macos native: a widget without a focus hook refuses input');
      await s.tap('blur-plain'); await settle(s);
      check(await focus() === input, 'macos native: targeted blur of another widget preserves ownership');
      const blurredHold = await s.carrier.input(input, 'key', { key: 'ArrowLeft', phase: 'down', ownedRelease: true });
      await s.tap('blur-input'); await settle(s);
      check(await focus() !== input && (await s.state()).slots.inputEvents.endsWith('blur;'), 'macos native: targeted blur resigns the descendant');
      const blurredRelease = await blurredHold.release().catch(error => ({ error: error.message }));
      check(/no longer owns focus/.test(blurredRelease.error ?? '') && await focus() !== input, 'macos native: a held release after blur does not reclaim focus');
      for (const [button, reason] of [['block-input', 'disabled'], ['hide-input', 'hidden'], ['inert-input', 'inert']]) {
        await s.tap(button); await s.tap('focus-input'); await settle(s);
        const refused = await s.carrier.ask({ op: 'type', id: input, text: 'forbidden' });
        check(Boolean(refused.error) && await focus() !== input && (await s.state()).slots.inputValue === 'a', `macos native: ${reason} refuses focus and agent input`);
        await s.tap(button); await settle(s);
      }
      for (const [source, reset, slot] of [['disable-on-blur', 'block-input', 'blocked'], ['inert-on-blur', 'inert-input', 'inputInert']]) {
        await s.tap(source); await settle(s);
        const refused = await s.carrier.ask({ op: 'type', id: input, text: 'forbidden' });
        await settle(s);
        const state = await s.state();
        check(Boolean(refused.error) && state.slots[slot] === true && state.slots.inputValue === 'a', `macos native: ${slot} applied by the previous responder's blur refuses input`);
        await s.tap(reset); await settle(s);
      }
      const wrapper = byTestId(await s.tree(), 'box').id;
      await s.tap('box'); await settle(s);
      check(await focus() === wrapper, 'macos native: clicking a passive module focuses its wrapper');
      const beforeWrapper = (await s.state()).slots.events.split('blur;').length;
      await s.tap('blur-wrapper'); await settle(s);
      const afterWrapper = await s.state();
      check(afterWrapper.focus.logical !== wrapper && afterWrapper.slots.events.split('blur;').length === beforeWrapper + 1, `macos native: targeted blur resigns the wrapper: ${JSON.stringify(afterWrapper.focus)}`);
      await s.tap('focus-input'); await settle(s);
      const held = await s.carrier.input(input, 'key', { key: 'ArrowLeft', phase: 'down', ownedRelease: true });
      await s.tap('toggle'); await settle(s);
      const retired = await held.release().catch(error => ({ error: error.message }));
      check(Boolean(retired.error) && await focus() !== input, 'macos native: unmount retires focus and refuses a held release');
      await s.tap('toggle'); await settle(s);
      const dialogInput = byTestId(await s.tree(), 'dialog-input').id;
      const dialogCommand = byTestId(await s.tree(), 'dialog-command').id;
      const dialogClose = byTestId(await s.tree(), 'dialog-close').id;
      const dialogOpener = byTestId(await s.tree(), 'open-native-dialog').id;
      await s.tap('open-native-dialog'); await settle(s);
      let dialogState = await s.state();
      check(dialogState.dialog?.phase === 'open' && dialogState.focus.logical === dialogInput && dialogState.focus.responder === 'FixtureEditor', 'macos native: a dialog initially focuses the native editor without focus/key handlers');
      await s.type('dialog-input', 'modal draft'); await settle(s);
      check((await s.state()).slots.dialogValue === 'modal draft', 'macos native: the dialog editor receives ordinary text');
      await s.type('dialog-input', { key: 'Tab', for: 10 }); await settle(s);
      check(await focus() === dialogCommand, 'macos native: Tab advances from the actual editor and releases after focus moves');
      await s.type('dialog-command', { key: 'Shift+Tab' }); await settle(s);
      dialogState = await s.state();
      check(dialogState.focus.logical === dialogInput && dialogState.focus.responder === 'FixtureEditor', 'macos native: Shift-Tab returns to the editing descendant');
      await s.type('dialog-input', { key: 'Shift+Tab' }); await settle(s);
      check(await focus() === dialogClose, 'macos native: Shift-Tab wraps from the editor to the final dialog button');
      await s.type('dialog-close', { key: 'Tab' }); await settle(s);
      dialogState = await s.state();
      check(dialogState.focus.logical === dialogInput && dialogState.focus.responder === 'FixtureEditor', 'macos native: Tab wraps back into the editor');
      await s.type('dialog-input', { key: 'Meta+Shift+Enter', for: 10 }); await settle(s);
      dialogState = await s.state();
      check(dialogState.slots.dialogCommands === 1 && dialogState.slots.inputCommands === 3, 'macos native: the modal shortcut runs once and leaves the background command inert');
      await s.type('dialog-input', { key: 'Escape', for: 10 }); await settle(s);
      dialogState = await s.state();
      check(dialogState.dialog == null && dialogState.focus.logical === dialogOpener && dialogState.slots.dialogValue === 'modal draft', 'macos native: Escape closes and releases safely, restores focus and preserves the draft');
      const popoverTree = await s.tree();
      const popover = byTestId(popoverTree, 'native-popover').id;
      const search = byTestId(popoverTree, 'popover-search').id;
      const popoverInput = byTestId(popoverTree, 'popover-input').id;
      const bump = byTestId(popoverTree, 'bump').id;
      check(byTestId(popoverTree, 'native-popover').open === false, 'macos popover: the closed form does not paint in agent mode');
      const hiddenType = await s.carrier.ask({ op: 'type', id: search, text: 'forbidden' });
      check(Boolean(hiddenType.error) && (await s.state()).slots.popoverSearch === '', 'macos popover: a closed field refuses input');
      await s.tap('open-native-popover'); await settle(s);
      let popoverState = await s.state();
      check(popoverState.navigation.popover?.popover === popover && popoverState.focus.logical === search, 'macos popover: a target-only button opens the real form and autofocuses its search field');
      await s.type('popover-search', 'find models'); await settle(s);
      await s.type('popover-input', 'custom draft'); await settle(s);
      popoverState = await s.state();
      check(popoverState.slots.popoverSearch === 'find models' && popoverState.slots.popoverValue === 'custom draft' && popoverState.focus.logical === popoverInput && popoverState.focus.responder === 'FixtureEditor', 'macos popover: both the ordinary field and native editor receive text');
      await s.screenshot(resolve(shots ?? tmp, 'native-popover.png'));
      await s.type('popover-search', { key: 'Tab' }); await settle(s);
      check(await focus() === popoverInput, 'macos popover: Tab from the search field reaches the native editing descendant');
      await s.type('popover-close', { key: 'Shift+Tab' }); await settle(s);
      check(await focus() === popoverInput, 'macos popover: reverse Tab also reaches the native editing descendant');
      await s.type('popover-close', { key: 'Tab' }); await settle(s);
      check(await focus() === bump, 'macos popover: Tab returns to the page after the invoker');
      await s.type('bump', { key: 'Escape', for: 10 }); await settle(s);
      popoverState = await s.state();
      check(popoverState.navigation.popover == null && popoverState.focus.logical === bump, 'macos popover: Escape closes without stealing focus back from the page');
      await s.tap('open-native-popover'); await settle(s);
      check((await s.state()).slots.popoverValue === 'custom draft' && (await s.state()).slots.popoverSearch === 'find models', 'macos popover: reopening preserves both drafts');
      const countBeforeDismiss = (await s.state()).slots.count;
      await s.tap('bump'); await settle(s);
      popoverState = await s.state();
      check(popoverState.navigation.popover == null && popoverState.slots.count === countBeforeDismiss + 1, 'macos popover: outside click dismisses and still activates its button');
      await s.tap('open-native-popover'); await settle(s);
      await s.tap('popover-close'); await settle(s);
      check((await s.state()).navigation.popover == null && byTestId(await s.tree(), 'native-popover').open === false, 'macos popover: the hide-only button closes the form');
    }
    // Each roster tag is defined once a page (web): by now instances have
    // come and gone (the remount above). On the wasm target a plan reload
    // swaps the plan into the page and reuses the definitions; on the JS
    // target the plan is the page (an edit rebuilds and reloads it, LLP 1071),
    // so its reload is the page's, whose modules attach again.
    if (host === 'web') {
      const roster = await s.carrier.evaluate('exact.nativeArtifact.then((m) => Object.keys(m.roster).length)');
      const defined = await s.carrier.evaluate('exact.nativeDefines?.count');
      if (jsTargetBuild(webDist)) {
        // The carrier waits until the old document has gone, then for the
        // new page's agent. Polling ready after location.reload can see the
        // old page and pass just before navigation destroys its context.
        await s.carrier.reset({ keep: true });
        s.now = 0; s.logCursor = 0;
      } else await s.carrier.evaluate(`fetch('./app.plan').then((r) => r.arrayBuffer()).then((b) => exact.reload(new Uint8Array(b)))`);
      t = await until(s, 'the reloaded plan attaches again', (t) => module(t, 'box')?.state === 'ready');
      const after = await s.carrier.evaluate('exact.nativeDefines?.count');
      check(roster > 0 && defined === roster && after === roster, `${host} native: a plan reload does not re-define the elements: ${defined} → ${after} of ${roster} roster tags`);
    }
  } catch (error) {
    check(false, `${host} native: the fixture drive stopped: ${error.stack ?? error.message}`);
  } finally { await s.close(); }

  // The failure family's load failures: a session each.
  const failing = async (name, options, pattern, cleanup = () => {}) => {
    const f = await open({ host, ...options });
    try {
      const t = await until(f, `${name}: every module node is unavailable`, (t) => ['box', 'plain', 'absent'].every((id) => module(t, id)?.state === 'unavailable'));
      check(['box', 'plain'].every((id) => pattern.test(module(t, id)?.error ?? '')), `${host} native: ${name} is named: ${JSON.stringify(module(t, 'box'))}`);
      const logs = await f.logs();
      check(logs.lines.some((l) => /native .*unavailable/.test(l) && pattern.test(l)), `${host} native: ${name} is logged: ${logs.lines.filter((l) => /native/.test(l)).join(' | ')}`);
      await f.tap('bump'); await f.clock('+50');
      check((await f.state()).slots.count === 1, `${host} native: the app runs with ${name}`);
      const rgb = await pixel(f, resolve(tmp, `${name.replace(/\W+/g, '-')}.png`), 'box');
      check(near(rgb, '#ffffff'), `${host} native: ${name} leaves an empty box: ${rgb}`);
    } catch (error) {
      check(false, `${host} native: ${name} stopped: ${error.message}`);
    } finally { await f.close(); cleanup(); }
  };
  if (host === 'web') {
    // A copy of the build without, and with a skewed, module artifact, served as-is.
    const variant = (name, change) => {
      const dir = resolve(tmp, name);
      cpSync(webDist, dir, { recursive: true });
      change(dir);
      // A JS-target build's agent pieces are served as `serve.mjs` serves them.
      const server = createServer((req, res) => jsTargetBuild(webDist) ? serveBuildTree(dir, req, res) : serveStatic(dir, req, res));
      return new Promise((ok) => server.listen(0, '127.0.0.1', () => ok({ url: `http://127.0.0.1:${server.address().port}/`, close: () => server.close() })));
    };
    const missing = await variant('missing', (dir) => rmSync(resolve(dir, 'modules'), { recursive: true, force: true }));
    await failing('a missing artifact', { url: missing.url }, /did not load/, missing.close);
    const skewed = await variant('skewed', (dir) => {
      const file = resolve(dir, 'modules/index.js');
      writeFileSync(file, readFileSync(file, 'utf8').replace('export const abi = 1;', 'export const abi = 2;'));
    });
    await failing('a wrong ABI', { url: skewed.url }, /module ABI 2, host ABI 1/, skewed.close);
  } else {
    await failing('a missing artifact', { env: { EXACT_MODULES: resolve(tmp, 'absent/libexact_modules.dylib') } }, /no module artifact/);
    // An artifact built against another ABI: a views-only table of major 1
    // (LLP 1024), from before the module entries (LLP 1067.000).
    const source = resolve(tmp, 'skew.c'), dylib = resolve(tmp, 'libexact_modules.dylib');
    writeFileSync(source, 'static const struct { unsigned major, size; const char *roster; void *f[7]; } t = { 1, 72, "{}", { 0 } };\nconst void *exact_native_abi(void) { return &t; }\n');
    const cc = spawnSync('xcrun', host === 'ios' ? ['--sdk', 'iphonesimulator', 'clang', '-target', 'arm64-apple-ios17.0-simulator', '-dynamiclib', '-o', dylib, source] : ['clang', '-dynamiclib', '-o', dylib, source], { encoding: 'utf8' });
    check(cc.status === 0, `${host} native: the skewed artifact compiles: ${cc.stderr}`);
    await failing('a wrong ABI', { env: { EXACT_MODULES: dylib } }, /module ABI 1, host ABI 3/);
  }
  rmSync(tmp, { recursive: true, force: true });
  console.log(`${host} native: ${checks - failed} of ${checks} checks passed in ${((Date.now() - t0) / 1000).toFixed(1)} s (the LLP 1024 D8 fixture)`);
}
