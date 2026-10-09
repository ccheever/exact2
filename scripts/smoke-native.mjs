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
      // 1075.003.000.001 §3.1–3.3, §8 stage 1): its counters and snapshot in
      // `state`, its line in `logs`, its calls in `perf hatches`. Reads are
      // cumulative: a second read is the same.
      {
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
        // By value: a native host's JSON orders an object's keys as it likes.
        const canon = (v) => JSON.stringify(v, (_, x) => (x && typeof x === 'object' && !Array.isArray(x) ? Object.fromEntries(Object.entries(x).sort(([a], [b]) => (a < b ? -1 : 1))) : x));
        check(canon(again.calls) === canon(perf.calls) && canon(again.counters) === canon(perf.counters), `${host} native: a perf hatches read changes nothing: ${canon(perf.calls)} then ${canon(again.calls)}`);
        // A node's calls are timed by its plan site, and `perf <target>`'s row for that site names them (§3.1).
        const site = (await s.perf('hatched-badge')).sites?.find((r) => r.site === built?.site);
        check(Number.isInteger(built?.site) && site?.hatch?.calls === 2 && site.hatch.ms >= 0, `${host} native: a hatched node's calls are timed by its plan site, which perf <target> names: site ${built?.site}, ${JSON.stringify(site?.hatch)}`);
      }
      // What a hatch asks of an authored node (LLP 1075.003.000.001 §2.5, §8
      // stage 2): `input` replaces a field's value and Contract hears it, the
      // journal holding its length and never its text; `clock settle` returns
      // only once a queued click has committed; and a hatch whose acts keep
      // causing acts is named by settle after 16 drains, without hanging.
      {
        await s.tap('feed'); await settle(s); await s.clock('settle');
        const all = (await s.op({ op: 'logs', since: 0 })).lines.filter((l) => / hatch /.test(l));
        check((await s.state()).slots.fed === 'Palo Alto', `${host} native: a hatch's input replaces the field's value in Contract: ${JSON.stringify((await s.state()).slots.fed)}`);
        check(all.some((l) => /hatch element feed #\d+: input \(9 chars, delivery: hatch\)/.test(l)) && !all.some((l) => /Palo/.test(l)), `${host} native: the journal holds the input's length, never its text: ${all.filter((l) => /feed/.test(l)).join(' | ')}`);
        await s.tap('arm'); await s.clock('settle');
        check((await s.state()).slots.hatchPresses === 1, `${host} native: clock settle returns only after a hatch's queued click has committed: ${(await s.state()).slots.hatchPresses}`);
        await s.tap('loop');
        const stuck = await s.clock('settle');
        check(stuck.settled === false && stuck.reason === 'hatches', `${host} native: a hatch whose acts keep causing acts is named by settle: ${JSON.stringify(stuck)}`);
        await s.tap('stop-loop'); await settle(s);
        const done = await s.clock('settle'), st = await s.state();
        check(done.settled === true && st.slots.hatchPresses > 16 && st.hatches?.inFlight === 0, `${host} native: stopped, it settles: ${JSON.stringify(done)} after ${st.slots.hatchPresses} presses`);
      }
      // Each platform handles the words app.json gives it (LLP
      // 1075.003.000.001 §4.3, §5): the fixture's `detail-list` is iOS's
      // alone, so elsewhere its node is shown, never called, and listed.
      {
        const st = (await s.state()).hatches, all = (await s.op({ op: 'logs', since: 0 })).lines.join('\n');
        const here = host === 'ios' ? ['badge', 'clock', 'detail-list', 'dot', 'feed', 'presser'] : ['badge', 'clock', 'dot', 'feed', 'presser'];
        check(JSON.stringify(st?.platform) === JSON.stringify(here), `${host} native: state.hatches names the words this platform handles: ${JSON.stringify(st?.platform)}`);
        if (host === 'ios') check(st?.unhandled?.length === 0 && st.words?.['detail-list']?.calls?.built === 1, `${host} native: iOS handles detail-list: ${JSON.stringify(st?.unhandled)}`);
        else check(st?.unhandled?.length === 1 && st.unhandled[0].word === 'detail-list' && !st.words?.['detail-list'] && /hatch element detail-list: (not handled|the plan does not give it to this platform)/.test(all) && byTestId(await s.tree(), 'list-detail')?.props.hatch === 'detail-list',
          `${host} native: a word this platform does not handle is shown, never called, and listed: ${JSON.stringify(st?.unhandled)}`);
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
    // A sheet over a sheet (the Bluesky clone's prompt over its muted words
    // sheet), tapped back to back as a drive sends its taps: an input replies
    // once the sheet it opened or closed is up (LLP 1035.003 D5), so the next
    // tap finds its target. Close returns to the sheet under it; Close both
    // closes the two.
    const onScreen = async (id) => {
      const error = await s.tap(id).then((r) => r?.error, (e) => String(e?.message ?? e));
      check(!error, `${host} native: sheet over sheet: ${id} taps${error ? `: ${error}` : ''}`);
    };
    for (const id of ['sheet', 'sheet-menu', 'menu-more', 'menu-close', 'sheet-menu', 'menu-home']) await onScreen(id);
    await s.clock('settle');
    check(!(await sheetShown()), `${host} native: Close both dismisses the two sheets`);
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
  // Two drives of the same steps agree on what the hatches did (LLP
  // 1075.003.000.001 §4.6, §8 stage 1): every call's count, every counter,
  // each span's count and time on the session clock, and the journal's
  // hatch lines. Measured milliseconds are the wall's and are left out.
  {
    const drive = async () => {
      const d = await open({ host });
      try {
        await d.clock('settle');
        await d.tap('rows'); await settle(d); await d.clock('settle');
        await d.tap('hatched-list', { wheel: [0, 4000] }); await settle(d); await d.clock('settle');
        await d.clock('+1000'); await d.clock('settle');
        const perf = await d.op({ op: 'perf', hatches: true });
        const calls = perf.calls.map((c) => `${c.hatch} ${c.site ?? ''} ${c.moment} ${c.calls}`).sort();
        const spans = Object.entries(perf.timings).flatMap(([scope, names]) => Object.entries(names).filter(([, t]) => !t.measured).map(([name, t]) => `${scope} ${name} ${t.count} ${t.sum}`)).sort();
        const said = (await d.op({ op: 'logs', since: 0 })).lines.filter((l) => / hatch /.test(l));
        return JSON.stringify({ calls, counters: Object.entries(perf.counters).map(([scope, names]) => [scope, Object.entries(names).sort()]).sort(), spans, said });
      } finally { await d.close(); }
    };
    const first = await drive(), second = await drive();
    check(first === second && JSON.parse(first).calls.length > 0, `${host} native: two drives agree on every hatch call, counter, span and line: ${first === second ? first.slice(0, 200) : `${first}\n  then ${second}`}`);
  }

  // The app and window scopes (LLP 1075.003.000.001 §2.1, §8 stage 2), in a
  // session of their own: both are built as the hatches connect; a fact that
  // changes is told to `app` once; a new size is told to `window`; the
  // embedder's grants decide what each is handed.
  {
    const d = await open({ host });
    try {
      const scopes = async () => (await d.state()).hatches?.scopes ?? {};
      const waited = async (what, test) => { for (let i = 0; i < 60; i++) { const sc = await scopes(); if (test(sc)) return sc; await d.clock('+50'); await sleep(50); } check(false, `${host} native: ${what}: ${JSON.stringify(await scopes())}`); return scopes(); };
      let sc = await waited('the app and window hatches are built', (sc) => sc.app?.calls?.built === 1 && sc.window?.calls?.built === 1);
      const lines = (await d.op({ op: 'logs', since: 0 })).lines;
      check(lines.some((l) => /hatch app: built/.test(l)) && lines.some((l) => /hatch window: built/.test(l)), `${host} native: the app and window hatches are journaled: ${lines.filter((l) => /hatch (app|window)/.test(l)).join(' | ')}`);
      const told = sc.module?.published ?? {};
      // ExactMac's sessions own their windows and none the process; iOS's one session, and a page, own both.
      check(told.scopes?.exclusive === true && told.scopes.hasWindow === true && told.app?.processOwner === (host !== 'macos') && told.app.hasApplication === (host !== 'macos'),
        `${host} native: the embedder's grants decide what the hatches are handed: ${JSON.stringify(told)}`);
      check(told.scopes?.frame?.[0] > 0 && told.app?.visibilityState === 'visible' && told.scopes.scheme === 'light', `${host} native: the window's frame and the app's facts reach the hatches: ${JSON.stringify(told)}`);
      await d.prefer({ 'prefers-color-scheme': 'dark' });
      sc = await waited('a changed fact is told to the app hatch', (sc) => sc.app?.calls?.changed >= 1 && sc.module?.published?.scopes?.scheme === 'dark');
      check(sc.app?.calls?.changed === 1, `${host} native: one change is one call: ${JSON.stringify(sc.app)}`);
      await d.prefer({ 'prefers-color-scheme': 'light' });
      await waited('the fact changing back is told too', (sc) => sc.module?.published?.scopes?.scheme === 'light');
      // Regions and parts (LLP 1075.003.000.001 §3.4, §3.5, §8 stage 3): the
      // badge's hatch draws a seal and says so. `tree` shows the declaration
      // beside what the host observes, and the part under its node; when the
      // seal goes its region stays as a tombstone that every read sees alike.
      {
        const badge = () => d.tree().then((t) => byTestId(t, 'hatched-badge'));
        let b = await badge();
        const own = b?.owns?.[0], part = b?.parts?.[0];
        check(b?.owns?.length === (host === 'web' ? 1 : 2) && own.by === 'element badge' && own.kind === 'view' && /^seal: /.test(own.what) && own.observed?.live === true && own.observed.frame?.w === 12 && own.observed.frame.h === 12 && !!own.observed.class,
          `${host} native: tree shows a region's declaration and what the host observes of it: ${JSON.stringify(b?.owns)}`);
        check(b?.parts?.length === 1 && part.id === 'seal' && part.role === 'button' && part.label === 'Verified' && part.live === true && part.frame?.w === 12,
          `${host} native: tree lists a part under its node, by id, with its observed frame: ${JSON.stringify(b?.parts)}`);
        // `tree --ax` joins the part by ownership and says whether the platform's tree exposes it (§3.5).
        const ax = await d.tree(undefined, { ax: true }).catch((error) => ({ error: error.message }));
        if (!ax.error && ax.ax && !ax.ax.unavailable) {
          const listed = ax.ax.coverage?.parts?.find((p) => p.id === 'seal' && p.node === b.id), joined = (ax.ax.elements ?? []).filter((e) => e.part === `${b.id}/seal`);
          check(listed && (listed.ax === 'exposed') === (joined.length > 0) && (host !== 'web' || listed.ax === 'exposed'),
            `${host} native: tree --ax joins the seal by ownership and lists it in coverage: ${JSON.stringify(listed)} ${JSON.stringify(joined.map((e) => e.role))} of ${JSON.stringify(ax.ax.coverage?.parts)}`);
        } else check(host !== 'web', `${host} native: tree --ax answers: ${JSON.stringify(ax.error ?? ax.ax)}`);
        const hs = (await d.state()).hatches;
        check(hs?.owns?.some((o) => o.by === 'element badge' && o.kind === 'view'), `${host} native: state.hatches lists the regions: ${JSON.stringify(hs?.owns)}`);
        // `tap <node>/<part>` reaches the seal as a real pointer event at its place (§3.5): it lands
        // on the part, and the hatch's own code counts the press. A part that is not there is refused
        // by name. The simulator's agent taps are not real touches, so there it is `unsupported`.
        const tapped = await d.tap('hatched-badge/seal').catch((error) => ({ error: error.message }));
        await settle(d);
        const presses = (await d.state()).hatches?.scopes?.module?.counters?.['seal.presses'];
        if (host === 'ios') check(tapped.delivery === 'unsupported' && presses === undefined, `${host} native: a part takes a real touch, so an agent tap is unsupported: ${JSON.stringify(tapped)}`);
        else check(tapped.landed === 'part' && tapped.delivery === 'platform' && presses === 1, `${host} native: tap <node>/<part> lands a real pointer event on the part: ${JSON.stringify(tapped)}, ${presses} presses`);
        const absent = await d.tap('hatched-badge/nope').catch((error) => ({ error: error.message }));
        check(host === 'ios' ? absent.delivery === 'unsupported' : /part nope: view \d+ has no live part of that id/.test(absent.error ?? ''), `${host} native: a part that is not there is refused by name: ${JSON.stringify(absent)}`);
        const moved = own.observed.frame;
        // The badge's tone turns busy: its hatch takes the seal away.
        await d.tap('compose-home'); await settle(d); await d.clock('settle');
        // The same press changes the root's `data-mood`: the root's words are the app hatch's (§2.5).
        sc = await waited('a root data word reaches the app hatch, and its change is a changed moment', (sc) => sc.module?.published?.app?.mood === 'busy');
        check(told.app?.mood === 'calm', `${host} native: the app hatch reads the root's data words: ${JSON.stringify(told.app)}`);
        b = await badge();
        const tomb = b?.owns?.[0], again = (await badge())?.owns?.[0];
        check(b?.owns?.length === (host === 'web' ? 1 : 2) && b.owns.every((o) => o.observed?.live === false) && tomb.observed?.live === false && typeof tomb.observed.ended === 'number' && !b.parts && again?.observed?.live === false && again.observed.ended === tomb.observed.ended && again.what === tomb.what && moved.w === 12,
          `${host} native: a region whose view is gone stays as one tombstone, the same at each read: ${JSON.stringify(b?.owns)} parts ${JSON.stringify(b?.parts)}`);
      }
      if (host !== 'ios') {
        const [w, h] = told.scopes.frame;
        await d.resize(w - 40, h - 40);
        sc = await waited('a new size is told to the window hatch', (sc) => sc.window?.calls?.changed >= 1 && sc.module?.published?.scopes?.frame?.[0] === w - 40);
        await d.resize(w, h);
      }
    } catch (error) {
      check(false, `${host} native: the scopes drive stopped: ${error.stack ?? error.message}`);
    } finally { await d.close(); }
  }

  // The frame clock (LLP 1075.003.000.001 §2.4, §8 stage 2), a session a
  // drive: a ticket ticks 60 times for `clock +1000`, tick k seeing the count
  // a frame task made at the same instant; an `after` chained in a tick fires
  // inside the seek and a stopped one never; 65 acts a tick asks, and the one
  // their `changed` asks, all land at that tick's instant; and one seek of a
  // second is ten of a tenth, line for line.
  {
    const clockDrive = async (steps) => {
      const d = await open({ host });
      try {
        await d.clock('settle');
        await d.tap('detail'); await settle(d);
        await until(d, 'the detail route is pushed', (t) => !!byTestId(t, 'route-detail'));
        await d.clock('settle');
        const from = (await d.op({ op: 'logs', since: 0 })).next;
        await d.tap('clock-start');
        for (const step of steps) await d.clock(`+${step}`);
        const st = await d.state(), counters = st.hatches?.scopes?.module?.counters ?? {}, told = st.hatches?.scopes?.module?.published?.clock ?? {};
        const lines = (await d.op({ op: 'logs', since: from })).lines.filter((l) => / hatch /.test(l));
        return { counters, told, frameCount: st.slots.frameCount, presses: st.slots.clockPresses, lines, d: null };
      } finally { await d.close(); }
    };
    try {
      const whole = await clockDrive([1000]), stepped = await clockDrive([100, 100, 100, 100, 100, 100, 100, 100, 100, 100]);
      check(whole.counters['clock.ticks'] === 60 && whole.frameCount === 60 && whole.told.ticks === 60, `${host} native: a frame ticket ticks 60 times for clock +1000: ${JSON.stringify({ counters: whole.counters, told: whole.told, frames: whole.frameCount })}`);
      check(whole.told.agreed === 60, `${host} native: tick k sees the count the frame task made at the same instant: ${JSON.stringify(whole.told)}`);
      check(whole.counters['clock.after0'] === 1 && whole.counters['clock.after20'] === 1 && whole.counters['clock.stopped'] === undefined, `${host} native: a chained after fires inside the seek, and a stopped one never: ${JSON.stringify(whole.counters)}`);
      const acts = whole.lines.filter((l) => /hatch element clock #\d+: click \(delivery: hatch\)/.test(l));
      check(whole.presses === 66 && acts.length === 66 && new Set(acts.map((l) => l.split(' ')[0])).size === 1, `${host} native: 65 acts a tick asks, and the one their changed asks, land at that tick's instant: ${whole.presses} presses, ${acts.length} lines at ${[...new Set(acts.map((l) => l.split(' ')[0]))].join(', ')}`);
      // By value: a native host's JSON orders an object's keys as it likes.
      const sorted = (v) => JSON.stringify(v, (_, x) => (x && typeof x === 'object' && !Array.isArray(x) ? Object.fromEntries(Object.entries(x).sort(([a], [b]) => (a < b ? -1 : 1))) : x));
      const a = sorted([whole.counters, whole.told, whole.frameCount, whole.presses]), b = sorted([stepped.counters, stepped.told, stepped.frameCount, stepped.presses]);
      const same = a === b && JSON.stringify(whole.lines) === JSON.stringify(stepped.lines);
      check(same, `${host} native: clock +1000 is ten clock +100, line for line${same ? '' : `: ${a} then ${b}; ${whole.lines.length} lines, then ${stepped.lines.length}; first difference ${JSON.stringify(whole.lines.find((l, i) => l !== stepped.lines[i]))} / ${JSON.stringify(stepped.lines.find((l, i) => l !== whole.lines[i]))}`}`);
    } catch (error) {
      check(false, `${host} native: the clock drive stopped: ${error.stack ?? error.message}`);
    }
  }

  // The crash breadcrumb and the switch its line names (LLP 1075.003.000.001
  // §4.4, §2.6; Apple): a run that dies inside the badge's hatch is named by
  // the next launch, once; with `EXACT_HATCHES=off` the module's views and
  // calls work and no hatch is connected.
  // With hatches off (§2.6) the web's page module still serves its views; no hatch is called.
  if (host === 'web') {
    const off = await open({ host, env: { EXACT_HATCHES: 'off' } });
    try {
      await until(off, 'the module loads with hatches off', (t) => module(t, 'box')?.state === 'ready');
      await off.clock('settle');
      const lines = (await off.op({ op: 'logs', since: 0 })).lines;
      check(lines.some((l) => /hatches: off/.test(l)) && !lines.some((l) => /hatch (element|app|window|navigation|route|tabs)/.test(l)) && Object.keys((await off.state()).hatches?.words ?? {}).length === 0,
        `${host} native: ?hatches=off connects no hatch: ${lines.filter((l) => / hatch/.test(l)).join(' | ')}`);
      await off.tap('bump'); await off.clock('+50');
      check((await off.state()).slots.count === 1, `${host} native: with hatches off the app works`);
    } catch (error) {
      check(false, `${host} native: the hatches-off drive stopped: ${error.stack ?? error.message}`);
    } finally { await off.close(); }
  }
  if (host === 'macos') {
    // The journal, once a line matching `pattern` is in it (or as it stands after 5 s).
    const journal = async (d, pattern) => {
      for (let i = 0; ; i++) {
        const lines = (await d.op({ op: 'logs', since: 0 })).lines;
        if (i === 50 || lines.some((l) => pattern.test(l))) return lines;
        await d.clock('+50'); await sleep(50);
      }
    };
    try {
      const dying = await open({ host, env: { EXACT_FIXTURE_DIE: 'badge' } }).catch(() => null);
      // It dies as its hatches connect, after first pixel: wait for the driver to lose it.
      if (dying) { for (let i = 0; i < 50; i++) { try { await dying.clock('+50'); await sleep(100); } catch { break; } } await dying.close().catch(() => {}); }
      const next = await open({ host });
      try {
        const lines = await journal(next, /hatch: connected/), said = lines.filter((l) => /the last run ended while inside hatch/.test(l));
        check(said.length === 1 && /ended while inside hatch element badge \(built, call \d+\).*EXACT_HATCHES=off/.test(said[0]), `${host} native: the launch after a death inside a hatch names it: ${said.join(' | ') || lines.filter((l) => /hatch/.test(l)).slice(0, 4).join(' | ')}`);
        check((await next.state()).hatches?.lastEnd?.length === 1, `${host} native: state.hatches carries the last run's end`);
      } finally { await next.close(); }
      const third = await open({ host });
      try {
        check(!(await journal(third, /hatch: connected/)).some((l) => /the last run ended/.test(l)), `${host} native: a breadcrumb is read once`);
      } finally { await third.close(); }
      const off = await open({ host, env: { EXACT_HATCHES: 'off' } });
      try {
        await until(off, 'the module loads with hatches off', (t) => module(t, 'box')?.state === 'ready');
        const lines = await journal(off, /hatches: off/), st = await off.state();
        check(lines.some((l) => /hatches: off \(EXACT_HATCHES=off\)/.test(l)) && !lines.some((l) => /hatch (element|toolbar|navigation|route|tabs)/.test(l)) && Object.values(st.hatches?.words ?? {}).every((w) => Object.keys(w.calls ?? {}).length === 0),
          `${host} native: EXACT_HATCHES=off connects no hatch and calls none: ${lines.filter((l) => / hatch/.test(l)).join(' | ')} ${JSON.stringify(st.hatches?.words)}`);
        await off.tap('bump'); await off.clock('+50');
        check((await off.state()).slots.count === 1 && module(await off.tree(), 'box')?.state === 'ready', `${host} native: with hatches off the app and its module's views work`);
      } finally { await off.close(); }
    } catch (error) {
      check(false, `${host} native: the breadcrumb drive stopped: ${error.stack ?? error.message}`);
    }
  }

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

// The fixture's hatches on a painting host (LLP 1075.003.000.001 §8 stage 4):
// `bun scripts/smoke.mjs linux --app native-fixture`. Linux loads no native
// module (LLP 1024), so the seam above is not driven here; the hatches are.
// A screenshot shows each row's dot under its overlay, and the detail list's
// redrawn at a new size after a resize; an observed press says whether
// Exact handled it; the module's counters, its line and `perf hatches` read
// back; with the Contract app idle a hatch's `draw` and then its `clear`
// each show in the next screenshot; `clock settle` returns only after a
// queued act has committed; the frame clock ticks at the virtual display's
// instants; a run that aborts inside a hatch is named by the next launch;
// and `EXACT_HATCHES=off` connects none.
export async function paintingHatchSmoke({ host, open, check: record, shots }) {
  let checks = 0, failed = 0;
  const check = (ok, what) => { checks += 1; if (!ok) failed += 1; return record(ok, what); };
  const t0 = Date.now();
  const tmp = mkdtempSync(resolve(tmpdir(), 'exact-hatches-'));
  const byTestId = (t, id) => t.nodes.find((n) => n.props.testId === id);
  const hatches = async (s) => (await s.state()).hatches ?? {};
  const journal = async (s) => (await s.op({ op: 'logs', since: 0 })).lines;
  const near = (rgb, hex) => rgb && [1, 3, 5].every((o, k) => Math.abs(rgb[k] - parseInt(hex.slice(o, o + 2), 16)) <= 12);
  // A screenshot, as a function from a viewport point to its pixel.
  const picture = async (s, name) => {
    const path = resolve(shots ?? tmp, `native-fixture-${host}-${name}.png`);
    const taken = await s.screenshot(path), png = decodePng(readFileSync(path)), scale = png.width / taken.w;
    return (x, y) => { const i = (Math.round(y * scale) * png.width + Math.round(x * scale)) * 4; return [png.data[i], png.data[i + 1], png.data[i + 2]]; };
  };
  const boxOf = async (s, testId) => (await s.layout()).nodes.find((n) => n.testId === testId);
  const centre = (b) => [b.x + b.w / 2, b.y + b.h / 2];
  const until = async (s, what, test) => {
    for (let i = 0; i < 100; i++) { const t = await s.tree(); if (test(t)) return t; await sleep(20); }
    check(false, `${host} hatches: ${what}`);
    return s.tree();
  };
  const WORDS = ['badge', 'clock', 'detail-list', 'dot', 'feed', 'presser'];

  const s = await open({ host });
  try {
    // Connected after first pixel: the app, its window, and the nodes there.
    let h = await hatches(s), lines = await journal(s);
    const at = (re) => lines.findIndex((l) => re.test(l));
    check(at(/hatch: connected/) >= 0 && at(/hatch: connected/) < at(/hatch app: built/) && at(/hatch app: built/) < at(/hatch window: built/) && at(/hatch window: built/) < at(/hatch element badge #\d+: built/),
      `${host} hatches: the hatches connect, then the app, its window and the nodes are built: ${lines.filter((l) => / hatch/.test(l)).join(' | ')}`);
    check(JSON.stringify(h.platform) === JSON.stringify(WORDS) && h.unhandled?.length === 0 && h.measuring === true, `${host} hatches: state.hatches names the words this platform handles: ${JSON.stringify(h.platform)} ${JSON.stringify(h.unhandled)}`);
    const told = h.scopes?.module?.published ?? {}, size = (await s.layout()).viewport ?? {};
    check(h.scopes?.app?.calls?.built === 1 && h.scopes?.window?.calls?.built === 1 && told.app?.processOwner === true && told.app?.visibilityState === 'visible' && told.scopes?.exclusive === true && told.scopes?.scheme === 'light',
      `${host} hatches: the entry's one session owns its window and the process, and the facts reach the app hatch: ${JSON.stringify(told)}`);
    check(told.scopes?.frame?.[0] > 0 && (size.w === undefined || told.scopes.frame[0] === size.w), `${host} hatches: the window hatch has the surface's frame: ${JSON.stringify(told.scopes?.frame)}`);

    // An observed press on a box no handler hears: heard after Exact's dispatch, which did nothing.
    await s.tap('hatched-badge');
    let badge = (await hatches(s)).words?.badge;
    check(badge?.published?.observed?.phase === 'up' && badge.published.observed.handled === false && badge.published.observed.inside === true && badge.counters?.['observed.down'] === 1 && badge.counters?.['observed.up'] === 1,
      `${host} hatches: a press on the badge is observed, unhandled: ${JSON.stringify(badge?.published)} ${JSON.stringify(badge?.counters)}`);

    // What a hatch says of itself, and what Exact timed (§3.1–§3.3).
    await s.tap('compose-home'); await s.clock('settle');
    // The same press changes the root's `data-mood`: the root's words are the app hatch's (§2.5).
    const mooded = await hatches(s);
    check(told.app?.mood === 'calm' && mooded.scopes?.module?.published?.app?.mood === 'busy' && mooded.scopes?.app?.calls?.changed === 1,
      `${host} hatches: the app hatch reads the root's data words, and a word's change is one changed moment: ${JSON.stringify(told.app)} → ${JSON.stringify(mooded.scopes?.module?.published?.app)} ${JSON.stringify(mooded.scopes?.app?.calls)}`);
    badge = mooded.words?.badge;
    check(badge?.calls?.built === 1 && badge.calls.changed === 1 && badge.live === 1 && badge.counters?.built === 1 && badge.counters.changed === 1 && badge.published?.tone?.tone === 'busy',
      `${host} hatches: state.hatches counts the badge's calls and carries its counters and snapshot: ${JSON.stringify(badge)}`);
    lines = (await journal(s)).filter((l) => /hatch element badge: /.test(l));
    check(lines.some((l) => /hatch element badge: built, tone info/.test(l)) && lines.some((l) => /hatch element badge: changed, tone busy/.test(l)), `${host} hatches: a hatch's log lines reach the journal under its scope: ${lines.join(' | ')}`);
    const perf = await s.op({ op: 'perf', hatches: true }), timed = perf.hatches?.['element badge'], built = perf.calls?.find((c) => c.hatch === 'element badge' && c.moment === 'built');
    check(perf.measuring && timed?.calls >= 2 && timed.ms >= 0 && timed.worst <= timed.ms && built?.calls === 1 && Number.isInteger(built.site) && perf.counters?.['element badge']?.built === 1 && perf.plan && Number.isInteger(perf.seq),
      `${host} hatches: perf hatches times each call by site and moment and carries the counters: ${JSON.stringify(perf).slice(0, 600)}`);
    const canon = (v) => JSON.stringify(v, (_, x) => (x && typeof x === 'object' && !Array.isArray(x) ? Object.fromEntries(Object.entries(x).sort(([a], [b]) => (a < b ? -1 : 1))) : x));
    const again = await s.op({ op: 'perf', hatches: true });
    check(canon(again.calls) === canon(perf.calls) && canon(again.counters) === canon(perf.counters), `${host} hatches: a perf hatches read changes nothing`);
    const site = (await s.perf('hatched-badge')).sites?.find((r) => r.site === built?.site);
    check(site?.hatch?.calls >= 2 && site.hatch.ms >= 0, `${host} hatches: perf <target> names the hatched site's calls: ${JSON.stringify(site?.hatch)}`);

    // With the Contract app idle, a hatch's draw and then its clear each show
    // in the next screenshot (§2.2.1): the badge's change asked for both, on
    // the session clock alone.
    {
      const b = await boxOf(s, 'hatched-badge'), [x, y] = centre(b);
      const before = (await s.state()).epoch, idle = (await picture(s, 'badge-idle'))(x, y);
      await s.clock('+100');
      const drawn = (await picture(s, 'badge-drawn'))(x, y), shown = (await hatches(s)).words?.badge?.overlay;
      await s.clock('+100');
      const cleared = (await picture(s, 'badge-cleared'))(x, y), after = await s.state();
      check(near(idle, '#f59e0b') && near(drawn, '#00c853') && near(cleared, '#f59e0b'), `${host} hatches: a draw and then a clear each show in the next screenshot: ${idle} → ${drawn} → ${cleared}`);
      check(shown?.shown === 1 && after.hatches?.words?.badge?.overlay?.shown === 0 && after.hatches.words.badge.overlay.published === 2 && after.epoch === before,
        `${host} hatches: both were published with the Contract app idle (epoch ${before} → ${after.epoch}): ${JSON.stringify(after.hatches?.words?.badge?.overlay)}`);
    }

    // The detail route: what a hatch asks of an authored node (§2.5).
    await s.tap('detail');
    await until(s, 'the detail route is pushed', (t) => !!byTestId(t, 'route-detail'));
    await s.clock('settle');
    await s.tap('feed'); await s.clock('settle');
    lines = (await journal(s)).filter((l) => / hatch /.test(l));
    check((await s.state()).slots.fed === 'Palo Alto', `${host} hatches: a hatch's input replaces the field's value in Contract: ${JSON.stringify((await s.state()).slots.fed)}`);
    check(lines.some((l) => /hatch element feed #\d+: input \(9 chars, delivery: hatch\)/.test(l)) && !lines.some((l) => /Palo/.test(l)), `${host} hatches: the journal holds the input's length, never its text: ${lines.filter((l) => /feed/.test(l)).join(' | ')}`);
    await s.tap('arm'); await s.clock('settle');
    let st = await s.state();
    check(st.slots.hatchPresses === 1 && st.hatches?.inFlight === 0, `${host} hatches: clock settle returns only after a hatch's queued click has committed: ${st.slots.hatchPresses} presses, ${st.hatches?.inFlight} in flight`);
    await s.tap('loop');
    const stuck = await s.clock('settle');
    check(stuck.settled === false && stuck.reason === 'hatches', `${host} hatches: a hatch whose acts keep causing acts is named by settle: ${JSON.stringify(stuck)}`);
    await s.tap('stop-loop');
    const done = await s.clock('settle');
    st = await s.state();
    check(done.settled === true && st.slots.hatchPresses > 16 && st.hatches?.inFlight === 0, `${host} hatches: stopped, it settles: ${JSON.stringify(done)} after ${st.slots.hatchPresses} presses`);

    // An observed press on the button: its handler ran first, and the hatch is told so.
    const presses = st.slots.hatchPresses;
    await s.tap('hatch-press'); await s.clock('settle');
    st = await s.state();
    const heard = st.hatches?.words?.presser?.published?.observed;
    check(st.slots.hatchPresses === presses + 1 && heard?.phase === 'up' && heard.handled === true, `${host} hatches: a press on the button is observed, handled: ${JSON.stringify(heard)}; ${presses} → ${st.slots.hatchPresses} presses (the observer pressed nothing)`);

    // The detail list's overlay, and again at a new size after a resize
    // (§2.2.1): the old recording is dropped, never stretched, and the
    // hatch hears `changed` with the new frame.
    {
      const edge = async (name) => { const b = await boxOf(s, 'list-detail'); const at = await picture(s, name); return { b, right: at(b.x + b.w - 2, b.y + b.h / 2), inside: at(b.x + b.w - 12, b.y + b.h / 2), outside: at(b.x + b.w / 2, b.y - 3) }; };
      const first = await edge('detail'), list = (await hatches(s)).words?.['detail-list'];
      check(near(first.right, '#ff00ff') && !near(first.inside, '#ff00ff') && !near(first.outside, '#ff00ff') && list?.overlay?.shown === 1,
        `${host} hatches: a screenshot shows the detail list's overlay along its edge, and nowhere outside its box: ${JSON.stringify(first)} ${JSON.stringify(list?.overlay)}`);
      const [w, hgt] = [first.b.w, first.b.h];
      await s.resize(380, 800);
      const second = await edge('detail-resized'), resized = await hatches(s), now = resized.words?.['detail-list'];
      check(second.b.w === 380 && second.b.h !== hgt && near(second.right, '#ff00ff') && !near(second.inside, '#ff00ff') && now?.overlay?.dropped === 1 && now.overlay.published === 2 && now.published?.frame?.[0] === 380 && now.published.frame[1] === second.b.h,
        `${host} hatches: after a resize the overlay is recorded again at the new size (${w}×${hgt} → ${second.b.w}×${second.b.h}): ${JSON.stringify(second)} ${JSON.stringify(now?.overlay)} ${JSON.stringify(now?.published)}`);
      check(resized.scopes?.window?.calls?.changed === 1 && resized.scopes.module?.published?.scopes?.frame?.[0] === 380 && resized.scopes.module.published.scopes.frame[1] === 800, `${host} hatches: a new size is told to the window hatch: ${JSON.stringify(resized.scopes?.module?.published?.scopes)}`);
      await s.resize(420, 900);
    }
    await s.tap('back');
    await until(s, 'Back pops the detail route', (t) => !byTestId(t, 'route-detail'));

    // A fact that changes is told to the app hatch, once (the root's word was the call before it).
    const toldBefore = (await hatches(s)).scopes?.app?.calls?.changed;
    await s.prefer({ 'prefers-color-scheme': 'dark' });
    h = await hatches(s);
    check(h.scopes?.app?.calls?.changed === toldBefore + 1 && h.scopes.module?.published?.scopes?.scheme === 'dark', `${host} hatches: a changed fact is one call to the app hatch: ${JSON.stringify(h.scopes?.app)} after ${toldBefore}`);
    await s.prefer({ 'prefers-color-scheme': 'light' });

    // A list whose rows each hold a hatched node: each dot is under its
    // overlay, clipped to its round box; a retired row's hatch hears `ended`.
    await s.tap('rows');
    let t = await until(s, 'the hatched list shows rows', (t) => !!byTestId(t, 'row-1'));
    await s.clock('settle');
    {
      const dot = t.nodes.find((n) => n.props.hatch === 'dot'), b = (await s.layout()).nodes.find((n) => n.id === dot?.id);
      const at = await picture(s, 'rows');
      // The corner of its box is outside the round dot: the row's white, whatever the edge's antialiasing leaves.
      check(b && near(at(...centre(b)), '#ff3b30') && at(b.x + 0.5, b.y + 0.5).every((c) => c > 224) && near(at(b.x - 2, b.y + b.h / 2), '#ffffff'),
        `${host} hatches: a screenshot shows the dot's overlay, clipped to its round box: ${b && at(...centre(b))} at its centre, ${b && at(b.x + 0.5, b.y + 0.5)} at its corner, ${b && at(b.x - 2, b.y + b.h / 2)} beside it`);
      const dots = (await hatches(s)).words?.dot;
      check(dots?.calls?.built > 0 && dots.live === dots.calls.built && dots.overlay?.shown === dots.live, `${host} hatches: each mounted row's dot is hatched and shown: ${JSON.stringify(dots)}`);
      await s.tap('hatched-list', { wheel: [0, 4000] }); await s.clock('settle');
      const scrolled = (await hatches(s)).words?.dot;
      check(scrolled?.calls?.ended > 0 && scrolled.live === scrolled.calls.built - scrolled.calls.ended, `${host} hatches: a retired row's hatch hears ended: ${JSON.stringify(scrolled?.calls)}`);
      lines = (await journal(s)).filter((l) => /hatch element dot/.test(l));
      check(lines.some((l) => /hatch element dot: nothing beyond the call/.test(l)) && lines.some((l) => /hatch element dot is in a row of list hatched-list/.test(l)) && lines.filter((l) => /#\d+: built/.test(l)).length === 1,
        `${host} hatches: the journal says what a hatched node costs, warns for a row's, and names a row's first call only: ${lines.slice(0, 4).join(' | ')}`);
    }
  } catch (error) {
    check(false, `${host} hatches: the fixture drive stopped: ${error.stack ?? error.message}`);
  } finally { await s.close(); }

  // Two drives of the same steps agree on what the hatches did (§4.6).
  try {
    const drive = async () => {
      const d = await open({ host });
      try {
        await d.clock('settle');
        await d.tap('rows'); await d.clock('settle');
        await d.tap('hatched-list', { wheel: [0, 4000] }); await d.clock('settle');
        await d.clock('+1000'); await d.clock('settle');
        const perf = await d.op({ op: 'perf', hatches: true });
        const calls = perf.calls.map((c) => `${c.hatch} ${c.site ?? ''} ${c.moment} ${c.calls}`).sort();
        const spans = Object.entries(perf.timings).flatMap(([scope, names]) => Object.entries(names).filter(([, t]) => !t.measured).map(([name, t]) => `${scope} ${name} ${t.count} ${t.sum}`)).sort();
        return JSON.stringify({ calls, counters: perf.counters, spans, said: (await journal(d)).filter((l) => / hatch /.test(l)) });
      } finally { await d.close(); }
    };
    const first = await drive(), second = await drive();
    check(first === second && JSON.parse(first).calls.length > 0, `${host} hatches: two drives agree on every hatch call, counter, span and line: ${first === second ? first.slice(0, 200) : `${first}\n  then ${second}`}`);
  } catch (error) {
    check(false, `${host} hatches: the two drives stopped: ${error.stack ?? error.message}`);
  }

  // The frame clock (§2.4), a session a drive: sixty ticks for `clock +1000`,
  // tick k seeing the count a frame task made at the same instant; a chained
  // `after` fires inside the seek and a stopped one never; 65 acts a tick
  // asks, and the one their `changed` asks, land at that tick's instant; and
  // one seek of a second is ten of a tenth, line for line.
  try {
    const clockDrive = async (steps) => {
      const d = await open({ host });
      try {
        await d.clock('settle');
        await d.tap('detail');
        await until(d, 'the detail route is pushed', (t) => !!byTestId(t, 'route-detail'));
        await d.clock('settle');
        const from = (await d.op({ op: 'logs', since: 0 })).next;
        await d.tap('clock-start');
        for (const step of steps) await d.clock(`+${step}`);
        const st = await d.state(), module = st.hatches?.scopes?.module ?? {};
        const lines = (await d.op({ op: 'logs', since: from })).lines.filter((l) => / hatch /.test(l));
        return { counters: module.counters ?? {}, told: module.published?.clock ?? {}, frameCount: st.slots.frameCount, presses: st.slots.clockPresses, lines };
      } finally { await d.close(); }
    };
    const whole = await clockDrive([1000]), stepped = await clockDrive([100, 100, 100, 100, 100, 100, 100, 100, 100, 100]);
    check(whole.counters['clock.ticks'] === 60 && whole.frameCount === 60 && whole.told.ticks === 60, `${host} hatches: a frame ticket ticks 60 times for clock +1000: ${JSON.stringify({ counters: whole.counters, told: whole.told, frames: whole.frameCount })}`);
    check(whole.told.agreed === 60, `${host} hatches: tick k sees the count the frame task made at the same instant: ${JSON.stringify(whole.told)}`);
    check(whole.counters['clock.after0'] === 1 && whole.counters['clock.after20'] === 1 && whole.counters['clock.stopped'] === undefined, `${host} hatches: a chained after fires inside the seek, and a stopped one never: ${JSON.stringify(whole.counters)}`);
    const acts = whole.lines.filter((l) => /hatch element clock #\d+: click \(delivery: hatch\)/.test(l));
    check(whole.presses === 66 && acts.length === 66 && new Set(acts.map((l) => l.split(' ')[0])).size === 1, `${host} hatches: 65 acts a tick asks, and the one their changed asks, land at that tick's instant: ${whole.presses} presses, ${acts.length} lines at ${[...new Set(acts.map((l) => l.split(' ')[0]))].join(', ')}`);
    const same = JSON.stringify([whole.counters, whole.told, whole.frameCount, whole.presses, whole.lines]) === JSON.stringify([stepped.counters, stepped.told, stepped.frameCount, stepped.presses, stepped.lines]);
    check(same, `${host} hatches: clock +1000 is ten clock +100, line for line${same ? '' : `: ${whole.lines.length} lines, then ${stepped.lines.length}; first difference ${JSON.stringify(whole.lines.find((l, i) => l !== stepped.lines[i]))} / ${JSON.stringify(stepped.lines.find((l, i) => l !== whole.lines[i]))}`}`);
  } catch (error) {
    check(false, `${host} hatches: the clock drive stopped: ${error.stack ?? error.message}`);
  }

  // The crash breadcrumb and the switch its line names (§4.4, §2.6): a run
  // that aborts inside the badge's hatch is named by the next launch, once;
  // with `EXACT_HATCHES=off` no hatch is connected and the app works.
  try {
    // It dies as its hatches connect, after first pixel, before it is ready.
    const dying = await open({ host, env: { EXACT_FIXTURE_DIE: 'badge' } }).catch(() => null);
    check(dying === null, `${host} hatches: the fixture's stand-in for a crash ends the run inside its hatch`);
    await dying?.close().catch(() => {});
    const next = await open({ host });
    try {
      const said = (await journal(next)).filter((l) => /the last run ended while inside hatch/.test(l));
      check(said.length === 1 && /ended while inside hatch element badge \(built, call \d+\).*EXACT_HATCHES=off/.test(said[0]), `${host} hatches: the launch after a death inside a hatch names it: ${said.join(' | ')}`);
      check((await hatches(next)).lastEnd?.length === 1, `${host} hatches: state.hatches carries the last run's end`);
    } finally { await next.close(); }
    const third = await open({ host });
    try {
      check(!(await journal(third)).some((l) => /the last run ended/.test(l)) && (await hatches(third)).lastEnd === undefined, `${host} hatches: a breadcrumb is read once`);
    } finally { await third.close(); }
    const off = await open({ host, env: { EXACT_HATCHES: 'off' } });
    try {
      const lines = await journal(off), h = await hatches(off);
      check(lines.some((l) => /hatches: off \(EXACT_HATCHES=off\)/.test(l)) && !lines.some((l) => /hatch (element|app|window)/.test(l)) && Object.keys(h.words ?? {}).length === 0 && Object.keys(h.scopes ?? {}).length === 0,
        `${host} hatches: EXACT_HATCHES=off connects no hatch and calls none: ${lines.filter((l) => / hatch/.test(l)).join(' | ')} ${JSON.stringify(h.words)}`);
      // Every Contract state the hatched run reaches is reached by authored input (§2.6).
      await off.tap('bump'); await off.tap('detail');
      await until(off, 'the detail route is pushed with hatches off', (t) => !!byTestId(t, 'route-detail'));
      await off.type('fed', 'Palo Alto'); await off.tap('hatch-press'); await off.clock('settle');
      const st = await off.state(), b = await boxOf(off, 'list-detail'), right = (await picture(off, 'off'))(b.x + b.w - 2, b.y + b.h / 2);
      check(st.slots.count === 1 && st.slots.fed === 'Palo Alto' && st.slots.hatchPresses === 1 && !near(right, '#ff00ff'), `${host} hatches: with hatches off the app works by authored input, and nothing is drawn over it: ${JSON.stringify({ count: st.slots.count, fed: st.slots.fed, presses: st.slots.hatchPresses, right })}`);
    } finally { await off.close(); }
  } catch (error) {
    check(false, `${host} hatches: the breadcrumb drive stopped: ${error.stack ?? error.message}`);
  }
  rmSync(tmp, { recursive: true, force: true });
  console.log(`${host} hatches: ${checks - failed} of ${checks} checks passed in ${((Date.now() - t0) / 1000).toFixed(1)} s (LLP 1075.003.000.001 stage 4)`);
}
