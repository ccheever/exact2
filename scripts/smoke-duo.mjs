// The iPhone Duo suite (LLP 1076 D9): `bun scripts/smoke.mjs duo`. Drives
// `apps/duo-lab` and the `insets` and `keyboard-bar` corpus fixtures on the
// Duo simulator through open 180°, book 130°, half 90° and closed 0°, the
// hinge set from inside the simulator by `scripts/duo/hinge_helper.c`
// (compiled per run with the selected Xcode's simulator SDK), the angle read
// back and each panel captured by `xcrun devicectl`. At every pose: the
// cover-root and inset invariants `smoke ios` checks; the Fold screen's two
// panes exactly on the division's segments (one pane flat or closed);
// `devicePosture` folded at 130° and 90° only; keyboard, sheet, push and
// scroll state across a fold; the two-session host's panes.
//
// Run by hand, like `smoke deploy`: it needs Xcode 27.1 beta and a Duo
// simulator (`DEVELOPER_DIR`, `EXACT_SIM`), and the bundles built first:
// `bun host/apple/build.mjs --ios duo-lab` (and `--ios --host` for the host
// fixture). Without the Duo it prints `duo: unsupported — …` and the smoke
// exits 0, the convention for a missing carrier. Rotation is not driven: this
// beta's simulator ignores CoreDevice orientation on the Duo (LLP 1008 §9).
import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { appleArtifacts, simulator, simulators } from '../host/apple/build.mjs';
import { resolveApp } from './app.mjs';

const ROOT = resolve(new URL('..', import.meta.url).pathname);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const byTestId = (t, id) => t.nodes.find((n) => n.props.testId === id);
const box = (l, id) => l.nodes.find((n) => n.testId === id);
const near = (a, b, eps = 0.01) => typeof a === 'number' && Math.abs(a - b) < eps;

// The matrix, and the Duo's geometry under the 27.1 beta (LLP 1008 §9): the
// cover panel 466×678, the inner panel 951×669, the division (455.5, 0, 40, 669)
// active at book and half, so the segments are (0, 0, 455.5, 669) and
// (495.5, 0, 455.5, 669).
// The simulator does not always sit in that state (2026-10-02: the same
// simulator reported `landscapeLeft`, its inner panel 669×871 under an 80-pt
// strip, and CoreDevice could not rotate it back): when the open pose does
// not report the recorded inner panel, the panel sizes are checked against
// what the device reports and the segments against `layout.env`'s own rects.
const POSES = [['open', 180], ['book', 130], ['half', 90], ['closed', 0]];
const folded = (deg) => deg === 130 || deg === 90;
const RECORDED = { inner: { w: 951, h: 669 }, cover: { w: 466, h: 678 } };
const SEGMENTS = [[0, 0, 455.5, 669], [495.5, 0, 455.5, 669]];
const after = (deg) => POSES[(POSES.findIndex((p) => p[1] === deg) + 1) % POSES.length][1];

/** The suite. `open` is the smoke's session opener; `check` records a failure. Returns 'unsupported' when there is no Duo to drive. */
export async function duoSmoke({ open, check }) {
  const pick = process.env.EXACT_SIM;
  const version = (d) => Number(/iOS-(\d+)-(\d+)/.exec(d.runtime)?.slice(1).join('.') ?? 0);
  const dev = pick ? simulators().find((d) => d.udid === pick || d.name === pick) : null;
  if (!dev || dev.name !== 'iPhone Duo' || version(dev) < 27.1) {
    console.log('duo: unsupported — needs an iPhone Duo simulator on iOS ≥ 27.1 under Xcode 27.1 (EXACT_SIM, DEVELOPER_DIR)');
    return 'unsupported';
  }
  const udid = simulator(pick).udid;
  const out = mkdtempSync(resolve(tmpdir(), 'exact-duo-'));
  const orientation = (spawnSync('xcrun', ['devicectl', 'device', 'orientation', 'get', '-d', udid], { encoding: 'utf8' }).stdout.match(/Current Device Orientation:\s*(\w+)/) ?? [])[1] ?? 'unknown';
  console.log(`duo: ${dev.name} ${udid} on ${dev.runtime.split('.').pop()}, orientation ${orientation}; captures in ${out}`);
  let recorded = null; // whether the open pose reports the recorded inner panel
  const panels = {}; // what each panel reported first, when the record does not apply

  // The hinge helper, compiled for this run with the selected Xcode's simulator SDK and ad-hoc signed.
  const helper = resolve(out, 'hinge_helper');
  const arch = process.arch === 'arm64' ? 'arm64' : 'x86_64';
  const cc = spawnSync('xcrun', ['-sdk', 'iphonesimulator', 'clang', '-arch', arch, '-mios-simulator-version-min=17.0', '-O2', '-framework', 'IOKit', '-framework', 'CoreFoundation', '-o', helper, resolve(ROOT, 'scripts/duo/hinge_helper.c')], { encoding: 'utf8' });
  if (!check(cc.status === 0, `the hinge helper did not compile: ${cc.stderr || cc.stdout}`)) return;
  const sign = spawnSync('codesign', ['-f', '-s', '-', helper], { encoding: 'utf8' });
  if (!check(sign.status === 0, `the hinge helper was not signed: ${sign.stderr}`)) return;
  const devicectl = (...a) => spawnSync('xcrun', ['devicectl', ...a, '-d', udid], { encoding: 'utf8' });
  const hingeAngle = () => {
    const r = spawnSync('xcrun', ['devicectl', 'device', 'motion', 'hinge-angle', '-d', udid, '--session-timeout', '2', '-t', '8'], { encoding: 'utf8' });
    const m = (r.stdout + r.stderr).match(/Angle:\s*([0-9.]+)/);
    return m ? Number(m[1]) : null;
  };
  /** Set the hinge and wait for the simulator to report it, then for the panels to switch. */
  const hinge = async (deg) => {
    const r = spawnSync('xcrun', ['simctl', 'spawn', udid, helper, 'set', String(deg)], { encoding: 'utf8' });
    check(r.status === 0, `hinge ${deg}: ${r.stderr || r.stdout}`);
    let angle = null;
    for (let i = 0; i < 12 && !(angle != null && Math.abs(angle - deg) < 1); i++) { await sleep(250); angle = hingeAngle(); }
    check(angle != null && Math.abs(angle - deg) < 1, `the simulator reports the hinge at ${angle}, not ${deg}`);
    await sleep(1500);
  };
  const settle = async (s) => { await sleep(300); await s.clock('settle'); await sleep(200); };
  const pose = async (s, deg) => { await hinge(deg); await settle(s); };
  const captures = (tag) => {
    const r = devicectl('device', 'info', 'displays', '-j', '-');
    let list = [];
    try { const j = JSON.parse(r.stdout); list = j.result?.displays ?? j.result ?? []; } catch { /* no displays listed: nothing captured */ }
    const seen = [];
    for (const disp of Array.isArray(list) ? list : []) {
      const id = disp.uniqueID ?? disp.uniqueId ?? disp.id;
      const p = resolve(out, `${tag}-display-${String(disp.name ?? id).replace(/[^A-Za-z0-9]+/g, '_')}.png`);
      const c = devicectl('device', 'capture', 'screenshot', '--destination', p, ...(id ? ['--display-unique-id', String(id)] : []));
      seen.push(`${disp.name ?? id}:${c.status === 0 ? 'ok' : 'no'}`);
    }
    return seen;
  };
  /** The facts at a pose: layout, state, the app's own screenshot and each display's capture, one line printed. */
  const facts = async (s, tag) => {
    const l = await s.layout();
    const st = await s.state();
    await s.screenshot(resolve(out, `${tag}-app.png`));
    const seen = captures(tag);
    if (recorded === null) {
      recorded = l.viewport.w === RECORDED.inner.w && l.viewport.h === RECORDED.inner.h;
      if (!recorded) console.log(`duo: the inner panel reports ${l.viewport.w}×${l.viewport.h}, not the recorded ${RECORDED.inner.w}×${RECORDED.inner.h} (orientation ${orientation}; CoreDevice cannot rotate the Duo, LLP 1008 §9): panel sizes and segments are checked against what the device reports`);
    }
    console.log(`  ${tag}: viewport ${l.viewport.w}×${l.viewport.h} screen ${l.screen?.w}×${l.screen?.h}@${l.screen?.scale} at ${l.screen?.x},${l.screen?.y} insets t${l.env['safe-area-inset-top']} r${l.env['safe-area-inset-right']} b${l.env['safe-area-inset-bottom']} l${l.env['safe-area-inset-left']} kb${l.env['keyboard-inset-height']} posture ${l.env['device-posture']} segments ${l.env['horizontal-viewport-segments']}×${l.env['vertical-viewport-segments']} ${JSON.stringify(l.env['viewport-segments'])} hinge ${hingeAngle()} displays ${seen.join(' ')}`);
    return { l, st };
  };
  /** The cover-root and inset invariants `smoke ios` checks, on this panel. */
  const coverChecks = (l, tag, deg) => {
    const root = box(l, 'root'), content = box(l, 'content');
    const env = l.env, top = env['safe-area-inset-top'], right = env['safe-area-inset-right'], bottom = env['safe-area-inset-bottom'], left = env['safe-area-inset-left'];
    check(root && root.w === l.viewport.w && root.h === l.viewport.h, `${tag}: a cover root fills the viewport ${JSON.stringify(root)} in ${JSON.stringify(l.viewport)}`);
    check(content && root && near(content.x - root.x, left) && near(content.y - root.y, top) && near(content.w, l.viewport.w - left - right) && near(content.h, l.viewport.h - top - bottom), `${tag}: the content keeps out of the insets ${JSON.stringify(content)} in root ${JSON.stringify(root)} for ${JSON.stringify(env)}`);
    check([top, right, bottom, left].every((v) => Number.isFinite(v) && v >= 0), `${tag}: insets are finite and non-negative ${JSON.stringify(env)}`);
    const which = deg === 0 ? 'cover' : 'inner';
    if (recorded) {
      check(l.screen && l.screen.x === 0 && l.screen.y === 0 && l.viewport.w === l.screen.w && l.viewport.h === l.screen.h, `${tag}: the cover viewport is the whole panel ${JSON.stringify(l.viewport)} on ${JSON.stringify(l.screen)}`);
      check(l.viewport.w === RECORDED[which].w && l.viewport.h === RECORDED[which].h, `${tag}: the ${which} panel is ${RECORDED[which].w}×${RECORDED[which].h}, not ${l.viewport.w}×${l.viewport.h}`);
    } else {
      check(l.screen && l.viewport.w <= l.screen.w + 0.01 && l.viewport.h <= l.screen.h + 0.01, `${tag}: the cover viewport fits its panel ${JSON.stringify(l.viewport)} on ${JSON.stringify(l.screen)}`);
      panels[which] ??= { w: l.viewport.w, h: l.viewport.h };
      check(l.viewport.w === panels[which].w && l.viewport.h === panels[which].h, `${tag}: the ${which} panel is ${panels[which].w}×${panels[which].h} as first reported, not ${l.viewport.w}×${l.viewport.h}`);
      if (panels.cover && panels.inner) check(panels.cover.w !== panels.inner.w || panels.cover.h !== panels.inner.h, `${tag}: the cover and inner panels differ (${JSON.stringify(panels)})`);
    }
  };
  /** Poll the keyboard inset until `want` (up or down). */
  const keyboard = async (s, want) => {
    let l;
    for (let i = 0; i < 40; i++) { l = await s.layout(); const kb = l.env['keyboard-inset-height']; if (want ? kb > 0 : kb === 0) break; await sleep(50); }
    return l;
  };
  const eachPose = async (s, name, body) => {
    for (const [tag, deg] of POSES) {
      await pose(s, deg);
      const f = await facts(s, `${name}-${tag}`);
      try { await body(f, `${name}-${tag}`, deg); } catch (e) { check(false, `${name}-${tag}: ${e.message}`); }
    }
    await pose(s, 180);
  };
  const plans = {};
  for (const name of ['insets', 'keyboard-bar']) {
    const plan = resolve(out, `${name}.plan`);
    const c = spawnSync('cargo', ['run', '-q', '--release', '-p', 'contract', '--', 'build', resolve(ROOT, `contract/corpus/${name}.contract`), '-o', plan], { cwd: ROOT, encoding: 'utf8' });
    if (check(c.status === 0, `the ${name} fixture did not compile: ${c.stderr}`)) plans[name] = plan;
  }
  const lab = resolveApp('duo-lab');
  if (!check(existsSync(appleArtifacts(lab, { destination: 'ios-simulator' }).bundle), 'duo: build the simulator bundle first: bun host/apple/build.mjs --ios duo-lab')) return;
  await hinge(180);

  // 1. The insets fixture: a cover root, the Duo's asymmetric insets, the
  // keyboard (resizes-visual) at each pose, and a fold while editing.
  if (plans.insets) {
    const s = await open({ host: 'ios', app: 'duo-lab', plan: plans.insets });
    try {
      await eachPose(s, 'insets', async ({ l }, tag, deg) => {
        coverChecks(l, tag, deg);
        const noteBefore = box(l, 'note');
        await s.type('note', 'hi');
        let l2 = await keyboard(s, true);
        const st = await s.state(), kb = l2.env['keyboard-inset-height'];
        check(st.slots.note === 'hi' && st.slots.focused === true, `${tag}: typing focused the field ${JSON.stringify(st.slots)}`);
        check(l2.viewport.h === l.viewport.h, `${tag}: the layout viewport does not change for a keyboard (${l2.viewport.h} vs ${l.viewport.h})`);
        check(kb > 100, `${tag}: the software keyboard rose: keyboard-inset-height ${kb} (Simulator › I/O › Keyboard › Connect Hardware Keyboard hides it)`);
        const note = box(l2, 'note');
        check(kb > 0 && note.y + note.h <= l2.viewport.h - kb + 0.01 && note.y < noteBefore.y, `${tag}: the field is revealed above the keyboard ${JSON.stringify(note)} kb ${kb} (was ${JSON.stringify(noteBefore)})`);
        await s.screenshot(resolve(out, `${tag}-keyboard.png`));
        // Fold while editing: focus and text survive the panel switch, the new panel is covered.
        const next = after(deg);
        await pose(s, next);
        l2 = await keyboard(s, true);
        const st2 = await s.state();
        check(st2.slots.note === 'hi' && st2.slots.focused === true, `${tag}: focus and text survive a fold to ${next}° ${JSON.stringify(st2.slots)}`);
        coverChecks(l2, `${tag}→${next}° while editing`, next);
        await pose(s, deg);
        await s.tap('dismiss');
        const l3 = await keyboard(s, false);
        const st3 = await s.state();
        check(st3.slots.focused === false && l3.env['keyboard-inset-height'] === 0 && box(l3, 'note').y === noteBefore.y, `${tag}: dismiss took the keyboard away: focused ${st3.slots.focused}, kb ${l3.env['keyboard-inset-height']}, the field at ${box(l3, 'note')?.y} (was ${noteBefore.y})`);
        await s.tap('note'); await s.type('note', { key: 'Backspace' }); await s.type('note', { key: 'Backspace' }); await s.tap('dismiss'); await keyboard(s, false);
      });
    } catch (e) { check(false, `insets: ${e.message}`); } finally { await s.close(); }
  }

  // 2. The keyboard-bar fixture (resizes-content): the bar rides the keyboard at each pose and across a fold.
  if (plans['keyboard-bar']) {
    const s = await open({ host: 'ios', app: 'duo-lab', plan: plans['keyboard-bar'] });
    try {
      await eachPose(s, 'kbar', async ({ l, st }, tag, deg) => {
        const fact0 = st.resources?.viewport;
        check(fact0 && fact0.width === l.viewport.w && fact0.height === l.viewport.h, `${tag}: the viewport fact ${JSON.stringify(fact0)} is the layout's ${JSON.stringify(l.viewport)}`);
        const bar0 = box(l, 'bar'), bottom0 = l.env['safe-area-inset-bottom'];
        check(bar0 && near(bar0.y + bar0.h, l.viewport.h - bottom0), `${tag}: the bar sits on the bottom inset ${JSON.stringify(bar0)} in ${l.viewport.h} inset ${bottom0}`);
        await s.type('note', 'hi');
        let l2 = await keyboard(s, true);
        const kb = l2.env['keyboard-inset-height'], bar = box(l2, 'bar');
        check(kb > 100, `${tag}: the software keyboard rose: keyboard-inset-height ${kb}`);
        check(near(l2.viewport.h, l.viewport.h - kb) && box(l2, 'root').h === l2.viewport.h, `${tag}: the layout viewport ends at the keyboard ${JSON.stringify(l2.viewport)} (was ${JSON.stringify(l.viewport)}, kb ${kb})`);
        check(l2.env['safe-area-inset-bottom'] === 0, `${tag}: the bottom inset is the keyboard's while it is up`);
        check(bar && near(bar.y + bar.h, l2.viewport.h) && bar.y < bar0.y, `${tag}: the bar rides on the keyboard ${JSON.stringify(bar)}`);
        await s.screenshot(resolve(out, `${tag}-keyboard.png`));
        // Fold with the keyboard up: the bar still ends where the viewport does on the new panel.
        const next = after(deg);
        await pose(s, next);
        l2 = await keyboard(s, true);
        const barF = box(l2, 'bar'), kbF = l2.env['keyboard-inset-height'];
        check(barF && near(barF.y + barF.h, l2.viewport.h) && box(l2, 'root').h === l2.viewport.h, `${tag}→${next}°: the bar ends at the viewport's bottom with the keyboard ${kbF} ${JSON.stringify(barF)} in ${JSON.stringify(l2.viewport)}`);
        await pose(s, deg);
        await s.tap('dismiss');
        const l3 = await keyboard(s, false);
        check(l3.viewport.h === l.viewport.h && l3.env['safe-area-inset-bottom'] === bottom0 && box(l3, 'bar').y === bar0.y, `${tag}: after the keyboard went everything is back ${JSON.stringify(l3.viewport)} bar ${box(l3, 'bar')?.y} (was ${bar0.y})`);
        await s.tap('note'); await s.type('note', { key: 'Backspace' }); await s.type('note', { key: 'Backspace' }); await s.tap('dismiss'); await keyboard(s, false);
      });
    } catch (e) { check(false, `keyboard-bar: ${e.message}`); } finally { await s.close(); }
  }

  // 3. Duo Lab's four screens.
  {
    const s = await open({ host: 'ios', app: 'duo-lab' });
    try {
      await settle(s);
      // Fold: the facts, the panes on the segments, a selection kept across every pose.
      check(byTestId(await s.tree(), 'screen-fold') != null, 'duo-lab opens on the Fold screen');
      await s.tap('item-2'); await settle(s);
      const chosen = byTestId(await s.tree(), 'detail-title')?.props.text;
      check(typeof chosen === 'string' && chosen.length > 0, `item-2 selected a detail: ${chosen}`);
      await eachPose(s, 'fold', async ({ l }, tag, deg) => {
        const t = await s.tree(), text = (id) => byTestId(t, id)?.props.text;
        const two = folded(deg);
        check(text('fact-posture') === (two ? 'folded' : 'continuous'), `${tag}: fact-posture is ${text('fact-posture')}, expected ${two ? 'folded' : 'continuous'}`);
        // The segments: the record's two side by side, or what the device reports (two
        // rects inside the viewport, one row or one column, the band between them empty).
        const segs = Array.isArray(l.env['viewport-segments']) ? l.env['viewport-segments'] : null;
        const h = l.env['horizontal-viewport-segments'], v = l.env['vertical-viewport-segments'];
        if (recorded) {
          check(h === (two ? 2 : 1) && v === 1 && JSON.stringify(segs) === JSON.stringify(two ? SEGMENTS : []), `${tag}: layout.env reports segments ${h}×${v} ${JSON.stringify(segs)}, expected ${two ? '2×1 ' + JSON.stringify(SEGMENTS) : '1×1 []'}`);
        } else if (two) {
          const [a, b] = segs ?? [];
          const sideBySide = a && b && near(a[1], b[1]) && a[0] + a[2] < b[0] + 0.01, stacked = a && b && near(a[0], b[0]) && a[1] + a[3] < b[1] + 0.01;
          check(segs?.length === 2 && (sideBySide || stacked) && h * v === 2 && (sideBySide ? h === 2 : v === 2) && [a, b].every((r) => r[0] >= 0 && r[1] >= 0 && r[0] + r[2] <= l.viewport.w + 0.01 && r[1] + r[3] <= l.viewport.h + 0.01), `${tag}: layout.env reports two segments inside ${JSON.stringify(l.viewport)} with a band between: ${h}×${v} ${JSON.stringify(segs)}`);
        } else check(h === 1 && v === 1 && segs?.length === 0, `${tag}: layout.env reports one segment: ${h}×${v} ${JSON.stringify(segs)}`);
        check(text('fact-h') === String(h) && text('fact-v') === String(v), `${tag}: the screen prints ${text('fact-h')}×${text('fact-v')}, layout.env ${h}×${v}`);
        check(text('fact-size') === `${l.viewport.w}×${l.viewport.h}`, `${tag}: fact-size ${text('fact-size')} vs layout ${l.viewport.w}×${l.viewport.h}`);
        check(l.env['device-posture'] === (two ? 'folded' : 'continuous'), `${tag}: layout.env reports posture ${l.env['device-posture']}`);
        const list = box(l, 'pane-list'), detail = box(l, 'pane-detail');
        if (h === 2 && segs?.length === 2) check(list && detail && near(list.x, segs[0][0]) && near(list.w, segs[0][2]) && near(list.h, l.viewport.h) && near(detail.x, segs[1][0]) && near(detail.w, segs[1][2]) && near(detail.h, l.viewport.h), `${tag}: the panes sit on the segments ${JSON.stringify(segs)}: list ${JSON.stringify(list)} detail ${JSON.stringify(detail)}`);
        else check(list && detail && near(list.w, l.viewport.w) && near(detail.w, l.viewport.w) && detail.y >= list.y + list.h - 0.01, `${tag}: one pane, the list above the detail: list ${JSON.stringify(list)} detail ${JSON.stringify(detail)}`);
        check(text('detail-title') === chosen, `${tag}: the selection survived the fold: ${text('detail-title')} (was ${chosen})`);
      });
      // Images: a virtualized list of image rows, folded mid-scroll, twice.
      await s.tap('tab-images'); await settle(s);
      check(byTestId(await s.tree(), 'screen-images') != null, 'the Images tab opened');
      await eachPose(s, 'images', async ({ l }, tag, deg) => {
        check(box(l, 'image-list') != null, `${tag}: the image list is up`);
        for (let round = 0; round < 2; round++) {
          await s.tap('image-list', { wheel: [0, 900] }); await settle(s);
          const before = box(await s.layout(), 'image-list')?.sy ?? 0;
          check(before > 0, `${tag}: the list scrolled (${before})`);
          const next = after(deg);
          await pose(s, next);
          const afterFold = box(await s.layout(), 'image-list')?.sy ?? 0;
          check(afterFold > 0, `${tag}→${next}° round ${round + 1}: the scroll offset survived the fold (${before} → ${afterFold})`);
          await pose(s, deg);
        }
        await s.tap('image-list', { wheel: [0, -100000] }); await settle(s);
      });
      // Reflow: the document reflows to each panel's width; the offset survives.
      await s.tap('tab-reflow'); await settle(s);
      check(byTestId(await s.tree(), 'screen-reflow') != null, 'the Reflow tab opened');
      await eachPose(s, 'reflow', async ({ l }, tag, deg) => {
        const doc = box(l, 'document');
        check(doc && doc.w > 0 && doc.w <= l.viewport.w - l.env['safe-area-inset-right'] && doc.h > l.viewport.h, `${tag}: the document fills its panel's width ${JSON.stringify(doc)} in ${JSON.stringify(l.viewport)}`);
        await s.tap('reflow-scroll', { wheel: [0, 1200] }); await settle(s);
        const before = box(await s.layout(), 'reflow-scroll')?.sy ?? 0;
        const next = after(deg);
        await pose(s, next);
        const l2 = await s.layout();
        const afterFold = box(l2, 'reflow-scroll')?.sy ?? 0;
        check(before > 0 && afterFold > 0 && box(l2, 'document') != null, `${tag}→${next}°: the document is up and its offset survived (${before} → ${afterFold})`);
        await pose(s, deg);
        await s.tap('reflow-scroll', { wheel: [0, -100000] }); await settle(s);
      });
      // Combinations: a pushed route, its sheet, the keyboard, then a fold.
      await s.tap('tab-combos'); await settle(s);
      check(byTestId(await s.tree(), 'screen-combos') != null, 'the Combinations tab opened');
      await eachPose(s, 'combos', async (_f, tag, deg) => {
        await s.tap('open-note'); await settle(s);
        let t = await s.tree();
        check(byTestId(t, 'screen-note') != null && byTestId(t, 'note-sheet') != null, `${tag}: the note screen pushed with its sheet`);
        // The title field raises the keyboard (a textarea under the driver's `type` takes
        // text and focus but no keyboard, 2026-10-02); the textarea takes the draft.
        await s.type('note-title-input', 'fold');
        let l = await keyboard(s, true);
        const kb = l.env['keyboard-inset-height'], sheet = box(l, 'note-sheet');
        check(kb > 100, `${tag}: the keyboard rose under the sheet (${kb})`);
        await s.type('note-textarea', 'fold me');
        l = await keyboard(s, true);
        check(sheet && near(sheet.y + sheet.h, l.viewport.h), `${tag}: the sheet sits on the keyboard-shortened viewport ${JSON.stringify(sheet)} in ${JSON.stringify(l.viewport)}`);
        await s.screenshot(resolve(out, `${tag}-sheet-keyboard.png`));
        const next = after(deg);
        await pose(s, next);
        l = await keyboard(s, true);
        t = await s.tree();
        const st = await s.state();
        check(byTestId(t, 'screen-note') != null && byTestId(t, 'note-sheet') != null, `${tag}→${next}°: the pushed screen and its sheet survived the fold`);
        check(typeof st.slots.draft === 'string' && st.slots.draft.includes('fold me') && typeof st.slots.title === 'string' && st.slots.title.includes('fold') && st.slots.noteFocused === true, `${tag}→${next}°: the title, the draft and the focus survived ${JSON.stringify({ title: st.slots.title, draft: st.slots.draft, noteFocused: st.slots.noteFocused })}`);
        check(near((box(l, 'note-sheet')?.y ?? 0) + (box(l, 'note-sheet')?.h ?? 0), l.viewport.h), `${tag}→${next}°: the sheet still ends at the viewport ${JSON.stringify(box(l, 'note-sheet'))} in ${JSON.stringify(l.viewport)}`);
        await s.screenshot(resolve(out, `${tag}-sheet-folded.png`));
        await pose(s, deg);
        await s.tap('note-dismiss'); await keyboard(s, false);
        check((await s.state()).slots.noteFocused === false, `${tag}: dismiss ended editing`);
        await s.tap('note-back'); await settle(s);
        t = await s.tree();
        check(byTestId(t, 'screen-combos') != null && byTestId(t, 'screen-note') == null, `${tag}: back popped the note screen`);
        check((byTestId(t, 'draft-echo')?.props.text ?? '').includes('fold me'), `${tag}: the draft is echoed on the Combinations screen: ${byTestId(t, 'draft-echo')?.props.text}`);
      });
    } catch (e) { check(false, `duo-lab: ${e.message}`); } finally { await s.close(); }
  }

  // 4. The two-session sample host: both panes follow the pose.
  const hostBundle = appleArtifacts(resolveApp('caltrain'), { destination: 'ios-simulator', host: true }).bundle;
  if (!existsSync(hostBundle)) console.log('duo host: unsupported — run bun host/apple/build.mjs --ios --host first');
  else {
    const s = await open({ host: 'host-ios', session: 'a' });
    try {
      await settle(s);
      await eachPose(s, 'host', async ({ l }, tag) => {
        s.session = 'b'; const lb = await s.layout(); s.session = 'a';
        check(l.viewport.w > 0 && lb.viewport.w > 0 && l.viewport.w === lb.viewport.w, `${tag}: both sessions follow the pose (a ${l.viewport.w}×${l.viewport.h}, b ${lb.viewport.w}×${lb.viewport.h})`);
      });
    } catch (e) { check(false, `host: ${e.message}`); } finally { await s.close(); }
  }
  await hinge(180);
}
