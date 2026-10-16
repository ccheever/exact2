// The iPhone Duo suite (LLP 1078 D9): `bun scripts/smoke.mjs duo`. Drives
// `apps/duo-lab` and the `insets` and `keyboard-bar` corpus fixtures on the
// Duo simulator through open 180°, book 130°, half 90° and closed 0°, the
// hinge set from inside the simulator by `scripts/duo/hinge_helper.c`
// (compiled per run with the selected Xcode's simulator SDK). At every pose:
// the cover-root and inset invariants `smoke ios` checks; the Fold screen's
// two panes exactly on the division's segments (one pane flat or closed);
// `devicePosture` folded at 130° and 90° only; keyboard, sheet, push and
// scroll state across a fold; the two-session host's panes. A failed check
// leaves one screenshot of the app in the run's directory.
//
// Run by hand, like `smoke deploy`: it needs Xcode 27.1 beta and a Duo
// simulator (`DEVELOPER_DIR`, `EXACT_SIM`), and the bundles built first:
// `bun host/apple/build.mjs --ios duo-lab` (and `--ios --host` for the host
// fixture). Without the Duo it prints `duo: unsupported — …` and the smoke
// exits 0, the convention for a missing carrier. Rotation is not driven: this
// beta's simulator ignores CoreDevice orientation on the Duo (LLP 1008 §9).
// `--only insets,kbar,lab,host` runs the named legs; the keyboard checks run
// only when a probe shows a software keyboard on this simulator.
import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { appleArtifacts } from '../host/apple/build.mjs';
import { simulator, simulators } from '../host/apple/devices.mjs';
import { HOST_DEV, resolveApp } from './app.mjs';

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
export async function duoSmoke(options) {
  const reset = {};
  try {
    return await suite(options, reset);
  } finally {
    reset.hinge?.();
  }
}

/** The suite's body; once the hinge helper is ready it sets `reset.hinge`, which the caller runs whatever happens. */
async function suite({ open: openSession, check: record }, reset) {
  // The phase a check runs in, and the first failure's since the last screenshot, which names it
  let where = 'start', failedAt = null, shots = 0;
  const check = (ok, what) => { if (!ok) { console.log('  FAIL ' + what); failedAt ??= where; } return record(ok, what); };
  const only = process.argv.includes('--only') ? process.argv[process.argv.indexOf('--only') + 1].split(',') : null; // insets, kbar, lab, host
  const leg = (name) => !only || only.includes(name);
  const pick = process.env.EXACT_SIM;
  const version = (d) => Number(/iOS-(\d+)-(\d+)/.exec(d.runtime)?.slice(1).join('.') ?? 0);
  const dev = pick ? simulators().find((d) => d.udid === pick || d.name === pick) : null;
  if (!dev || dev.type !== 'iPhone-Duo' || version(dev) < 27.1) {
    console.log('duo: unsupported — needs an iPhone Duo simulator on iOS ≥ 27.1 under Xcode 27.1 (EXACT_SIM, DEVELOPER_DIR)');
    return 'unsupported';
  }
  const udid = simulator(pick).udid;
  const out = mkdtempSync(resolve(tmpdir(), 'exact-duo-'));
  console.log(`duo: ${dev.name} ${udid} on ${dev.runtime.split('.').pop()}; screenshots of failures in ${out}`);
  let recorded = null; // whether the open pose reports the recorded inner panel
  const panels = {}; // what each panel reported first, when the record does not apply

  // The hinge helper, compiled for this run with the selected Xcode's simulator SDK and ad-hoc signed.
  const helper = resolve(out, 'hinge_helper');
  const arch = process.arch === 'arm64' ? 'arm64' : 'x86_64';
  const cc = spawnSync('xcrun', ['-sdk', 'iphonesimulator', 'clang', '-arch', arch, '-mios-simulator-version-min=17.0', '-O2', '-framework', 'IOKit', '-framework', 'CoreFoundation', '-o', helper, resolve(ROOT, 'scripts/duo/hinge_helper.c')], { encoding: 'utf8' });
  if (!check(cc.status === 0, `the hinge helper did not compile: ${cc.stderr || cc.stdout}`)) return;
  const sign = spawnSync('codesign', ['-f', '-s', '-', helper], { encoding: 'utf8' });
  if (!check(sign.status === 0, `the hinge helper was not signed: ${sign.stderr}`)) return;
  /** Set the hinge. Xcode 27.1 RC's devicectl has no hinge readback (`device motion` is gone), so a pose is judged by what the session reports. */
  const hinge = (deg) => {
    const r = spawnSync('xcrun', ['simctl', 'spawn', udid, helper, 'set', String(deg)], { encoding: 'utf8' });
    check(r.status === 0, `hinge ${deg}: ${r.stderr || r.stdout}`);
  };
  /** Settle the app's clock; a settle that gives up is a failure naming why. */
  const settle = async (s) => {
    await sleep(300);
    const r = await s.clock('settle');
    check(r?.settled !== false, `${where}: clock settle did not settle: ${r?.diagnostic ?? JSON.stringify(r)}`);
    await sleep(200);
  };
  /** One app screenshot when a check failed since the last call, named for the phase it failed in. A session's
   * `tap`, `type` and `close` and a pose call it first, so the picture is of the failing state. An error taking
   * it is printed, never a failure. */
  const shot = async (s) => {
    if (failedAt === null) return;
    const at = failedAt;
    failedAt = null;
    const p = resolve(out, `${String(++shots).padStart(2, '0')}-${at.replace(/[^A-Za-z0-9-]+/g, '_')}.png`);
    try { await s.screenshot(p); console.log(`  screenshot ${p}`); } catch (e) { console.log(`  no screenshot at ${at}: ${e.message}`); }
  };
  /** A session whose state-changing operations take a failure's screenshot first (`shot`). */
  const open = async (options) => {
    const s = await openSession(options);
    for (const op of ['tap', 'type', 'close']) {
      const run = s[op].bind(s);
      s[op] = async (...args) => { await shot(s); return run(...args); };
    }
    return s;
  };
  /** Whether `layout.env` reports the pose's segments: two at 130° and 90°, one (`1×1`, none listed) flat or closed. Which two is each leg's check (a session laid out in part of the panel has its own). */
  const segmented = (l, deg) => {
    const segs = l.env['viewport-segments'], h = l.env['horizontal-viewport-segments'], v = l.env['vertical-viewport-segments'];
    return folded(deg) ? Array.isArray(segs) && segs.length === 2 && h * v === 2 : h === 1 && v === 1 && Array.isArray(segs) && segs.length === 0;
  };
  /** Whether the session is on the cover panel: its screen is the cover's, either way round. */
  const onCover = (l) => [l.screen?.w, l.screen?.h].sort((a, b) => a - b).join() === [RECORDED.cover.w, RECORDED.cover.h].sort((a, b) => a - b).join();
  /** Set the hinge and wait, over 50 reads, for the session to report the pose: the cover panel at 0° and only then, the posture and the segments (the division regions can trail the posture). A pose the session never reports fails here. A failure before the fold is photographed first. */
  const pose = async (s, deg) => {
    await shot(s);
    hinge(deg);
    let panel = false, postured = false, split = false;
    for (let i = 0; i < 50 && !(panel && postured && split); i++) {
      if (i > 0) await sleep(100);
      const l = await s.layout();
      panel = onCover(l) === (deg === 0);
      postured = (l.env['device-posture'] === 'folded') === folded(deg);
      split = segmented(l, deg);
    }
    const missed = [panel ? null : `the session is ${deg === 0 ? 'not ' : ''}on the cover panel`, postured ? null : `the posture is not ${folded(deg) ? 'folded' : 'continuous'}`, split ? null : `the segments are not ${folded(deg) ? 'two' : 'one'}`].filter(Boolean);
    check(!missed.length, `the hinge moved to ${deg}° and the session missed it: ${missed.join(', ')}`);
    await settle(s);
  };
  /** The facts at a pose: layout and state, one line printed. */
  const facts = async (s, tag) => {
    const l = await s.layout();
    const st = await s.state();
    // Judged by a session that fills its panel (the host's sessions each take a part of it)
    if (recorded === null && l.screen && l.viewport.w === l.screen.w && l.viewport.h === l.screen.h) {
      recorded = l.viewport.w === RECORDED.inner.w && l.viewport.h === RECORDED.inner.h;
      if (!recorded) console.log(`duo: the inner panel reports ${l.viewport.w}×${l.viewport.h}, not the recorded ${RECORDED.inner.w}×${RECORDED.inner.h} (screen orientation ${l.screen?.orientation ?? 'unknown'}; CoreDevice cannot rotate the Duo, LLP 1008 §9): panel sizes and segments are checked against what the device reports`);
    }
    console.log(`  ${tag}: viewport ${l.viewport.w}×${l.viewport.h} screen ${l.screen?.w}×${l.screen?.h}@${l.screen?.scale} at ${l.screen?.x},${l.screen?.y} insets t${l.env['safe-area-inset-top']} r${l.env['safe-area-inset-right']} b${l.env['safe-area-inset-bottom']} l${l.env['safe-area-inset-left']} kb${l.env['keyboard-inset-height']} posture ${l.env['device-posture']} segments ${l.env['horizontal-viewport-segments']}×${l.env['vertical-viewport-segments']} ${JSON.stringify(l.env['viewport-segments'])}`);
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
      const rec = RECORDED[which], same = (w, h) => l.viewport.w === w && l.viewport.h === h; // the simulator turns either panel on its own
      check(same(rec.w, rec.h) || same(rec.h, rec.w), `${tag}: the ${which} panel is ${rec.w}×${rec.h} either way up, not ${l.viewport.w}×${l.viewport.h}`);
    } else {
      check(l.screen && l.viewport.w <= l.screen.w + 0.01 && l.viewport.h <= l.screen.h + 0.01, `${tag}: the cover viewport fits its panel ${JSON.stringify(l.viewport)} on ${JSON.stringify(l.screen)}`);
      panels[which] ??= { w: l.viewport.w, h: l.viewport.h };
      check(l.viewport.w === panels[which].w && l.viewport.h === panels[which].h, `${tag}: the ${which} panel is ${panels[which].w}×${panels[which].h} as first reported, not ${l.viewport.w}×${l.viewport.h}`);
      if (panels.cover && panels.inner) check(panels.cover.w !== panels.inner.w || panels.cover.h !== panels.inner.h, `${tag}: the cover and inner panels differ (${JSON.stringify(panels)})`);
    }
  };
  /** Poll the keyboard inset until `want` (up or down), then settle and read the layout again for the checks; `reached` says whether it got there. */
  const keyboard = async (s, want) => {
    const there = (l) => want ? l.env['keyboard-inset-height'] > 0 : l.env['keyboard-inset-height'] === 0;
    let l, reached = false;
    for (let i = 0; i < 40 && !reached; i++) { if (i > 0) await sleep(50); l = await s.layout(); reached = there(l); }
    if (reached) { await settle(s); l = await s.layout(); reached = there(l); }
    return { l, reached };
  };
  /** Wait for the keyboard to go, a failure naming `tag` when it stays. */
  const down = async (s, tag) => {
    const k = await keyboard(s, false);
    check(k.reached, `${tag}: the keyboard went down (keyboard-inset-height ${k.l.env['keyboard-inset-height']})`);
    return k.l;
  };
  const eachPose = async (s, name, body, dismiss) => {
    for (const [tag, deg] of POSES) {
      where = `${name}-${tag}`;
      await pose(s, deg);
      if (dismiss && (await s.layout()).env['keyboard-inset-height'] > 0) { await s.tap(dismiss); await down(s, where); } // a keyboard the panel switch kept
      const f = await facts(s, where);
      try { await body(f, where, deg); } catch (e) { check(false, `${where}: ${e.message}`); }
    }
    where = `${name}-reopened`;
    await pose(s, 180); // closed to open in a live session, checked
  };
  // The insets fixture serves its leg and the keyboard probe, which the legs that type need
  const probe = leg('insets') || leg('kbar') || leg('lab');
  const plans = {};
  for (const name of [probe ? 'insets' : null, leg('kbar') ? 'keyboard-bar' : null].filter(Boolean)) {
    const plan = resolve(out, `${name}.plan`);
    const c = spawnSync('cargo', ['run', '-q', '--profile', HOST_DEV, '-p', 'contract', '--', 'build', resolve(ROOT, `contract/corpus/${name}.contract`), '-o', plan], { cwd: ROOT, encoding: 'utf8' });
    if (check(c.status === 0, `the ${name} fixture did not compile: ${c.stderr}`)) plans[name] = plan;
  }
  const lab = resolveApp('duo-lab');
  const labBundle = appleArtifacts(lab, { destination: 'ios-simulator' }).bundle;
  if (!check(existsSync(labBundle), 'duo: build the simulator bundle first: bun host/apple/build.mjs --ios duo-lab')) return;
  // A bundle linked against the 27.0 SDK finds neither UIKit 27.1 class and reports flat (LLP 1078 §As built, Apple): the fold needs the beta's SDK.
  const sdk = JSON.parse(readFileSync(resolve(labBundle, 'receipt.json'), 'utf8')).sdk ?? '';
  const sdkVersion = Number(/iPhoneSimulator(\d+\.\d+)\.sdk/.exec(sdk)?.[1] ?? 0);
  if (!check(sdkVersion >= 27.1, `duo: the duo-lab bundle was built with ${sdk || 'an unknown SDK'}; the fold needs the iOS 27.1 SDK — DEVELOPER_DIR=<Xcode 27.1> bun host/apple/build.mjs --ios duo-lab`)) return;
  reset.hinge = () => hinge(180); // leave the simulator open, whatever happens
  // Whether this simulator shows a software keyboard at all. The simulator's state
  // is what counts, and it moves under a run: a Device Hub window sets the device's
  // own keyboard preference — `com.apple.keyboard.preferences`
  // `AutomaticMinimizationEnabled = 1`, the software keyboard minimized behind the
  // hardware keyboard a simulator always has, what "Connect Hardware Keyboard" set —
  // and the key stays after the window closes, per device, so a session that raised
  // the keyboard at 14:24 found none at 14:37 on 2026-10-02 (LLP 1078 §As built) —
  // the "pushed route raises no keyboard" the queue carried. The key is causal
  // (written, the keyboard goes; deleted, it is back) and read in well under a
  // second with `simctl spawn <udid> defaults read`; backboardd's "Hardware keyboard
  // attached" is not the state (every booted simulator gets it at once, and a fresh
  // device raises its keyboard through it). At the start a fixture probe in its own
  // session (no leg is open then; the iOS carrier terminates a running copy of the
  // bundle) is read beside the key; absent with the key set, the keyboard checks are
  // reported unsupported once, and absent with the key clear they run and fail, a
  // host failure. Mid-run, the first keyboard that fails to rise reads the key
  // again, never opens a session: set since the start, the keyboard checks are
  // unsupported from there, named; clear, they fail. Every other check runs.
  const minimized = () => {
    const r = spawnSync('xcrun', ['simctl', 'spawn', udid, 'defaults', 'read', 'com.apple.keyboard.preferences', 'AutomaticMinimizationEnabled'], { encoding: 'utf8', timeout: 60000 });
    const set = r.status === 0 && /^\s*1\s*$/.test(r.stdout ?? '');
    const says = set ? 'the device has com.apple.keyboard.preferences AutomaticMinimizationEnabled = 1 (its software keyboard minimized behind the hardware keyboard a simulator always has — what "Connect Hardware Keyboard" set; a Device Hub window writes it and it stays)'
      : r.status === 0 ? `the device has AutomaticMinimizationEnabled = ${(r.stdout ?? '').trim()}` : 'the device has no AutomaticMinimizationEnabled key';
    return { set, says };
  };
  let softKeyboard = false;
  let kept = null; // the probe's session, which the insets leg goes on with
  if (plans.insets) {
    const s = await open({ host: 'ios', app: 'duo-lab', plan: plans.insets });
    let up = false, seen = '';
    try {
      where = 'probe';
      await pose(s, 180);
      await s.type('note', 'hi');
      const { l } = await keyboard(s, true), st = await s.state();
      up = l.env['keyboard-inset-height'] > 0;
      seen = `keyboard-inset-height ${l.env['keyboard-inset-height']}, state.keyboard ${JSON.stringify(st.keyboard)}`;
      // Leave no editing behind: the insets leg's first baseline is taken without a keyboard
      await s.tap('dismiss');
      await down(s, where);
      check((await s.state()).slots.focused === false, `${where}: dismiss ended editing`);
      if (leg('insets')) kept = s;
    } catch (e) { seen = `the probe failed — ${e.message}`; } finally { if (kept !== s) await s.close(); }
    const m = minimized();
    console.log(`duo keyboard: software keyboard ${up ? 'rises' : 'absent'} at the start (${seen}); ${m.says}`);
    if (up) softKeyboard = true;
    else if (m.set) console.log(`duo keyboard: unsupported — the software keyboard is minimized on this simulator; clear it (xcrun simctl spawn ${udid} defaults delete com.apple.keyboard.preferences AutomaticMinimizationEnabled) or boot a fresh device; the keyboard checks are not run`);
    else { softKeyboard = true; console.log('duo keyboard: no software keyboard rose and the device\'s keyboard preference is not minimized — treated as a host failure; the keyboard checks run'); }
  }
  /** A keyboard that should have risen: true to check it (and fail), false when the device's keyboard preference turned minimized since the run began. */
  const rose = (kb, tag) => {
    if (!softKeyboard) return false;
    if (kb > 0) return true;
    const m = minimized();
    if (m.set) {
      softKeyboard = false;
      console.log(`duo keyboard: ${m.says} — during the run; from ${tag} the keyboard checks are unsupported`);
      return false;
    }
    console.log(`duo keyboard: no software keyboard rose at ${tag} and the device's keyboard preference is not minimized (${m.says}) — a host failure`);
    return true;
  };

  // 1. The insets fixture: a cover root, the Duo's asymmetric insets, the
  // keyboard (resizes-visual) at each pose, and a fold while editing.
  if (plans.insets && leg('insets')) {
    const s = kept ?? await open({ host: 'ios', app: 'duo-lab', plan: plans.insets });
    try {
      await eachPose(s, 'insets', async ({ l }, tag, deg) => {
        coverChecks(l, tag, deg);
        const noteBefore = box(l, 'note'), typed = `hi ${tag}`;
        await s.type('note', typed);
        let { l: l2 } = await keyboard(s, true);
        const st = await s.state(), kb = l2.env['keyboard-inset-height'];
        check(st.slots.note === typed && st.slots.focused === true, `${tag}: typing focused the field ${JSON.stringify(st.slots)}`);
        check(l2.viewport.h === l.viewport.h, `${tag}: the layout viewport does not change for a keyboard (${l2.viewport.h} vs ${l.viewport.h})`);
        const note = box(l2, 'note');
        if (rose(kb, tag)) {
          check(kb > 100, `${tag}: the software keyboard rose: keyboard-inset-height ${kb}`);
          check(kb > 0 && note.y + note.h <= l2.viewport.h - kb + 0.01 && note.y < noteBefore.y, `${tag}: the field is revealed above the keyboard ${JSON.stringify(note)} kb ${kb} (was ${JSON.stringify(noteBefore)})`);
        }
        // Fold while editing: focus and text survive the panel switch, the new panel is covered.
        const next = after(deg);
        await pose(s, next);
        ({ l: l2 } = await keyboard(s, true));
        const st2 = await s.state(), kb2 = l2.env['keyboard-inset-height'];
        check(st2.slots.note === typed && st2.slots.focused === true, `${tag}: focus and text survive a fold to ${next}° ${JSON.stringify(st2.slots)}`);
        if (rose(kb2, `${tag}→${next}°`)) check(kb2 > 0, `${tag}→${next}°: the keyboard is up on the new panel (keyboard-inset-height ${kb2})`);
        coverChecks(l2, `${tag}→${next}° while editing`, next);
        await pose(s, deg);
        await s.tap('dismiss');
        const l3 = await down(s, tag);
        const st3 = await s.state();
        check(st3.slots.focused === false && l3.env['keyboard-inset-height'] === 0 && box(l3, 'note').y === noteBefore.y, `${tag}: dismiss took the keyboard away: focused ${st3.slots.focused}, kb ${l3.env['keyboard-inset-height']}, the field at ${box(l3, 'note')?.y} (was ${noteBefore.y})`);
      });
    } catch (e) { check(false, `insets: ${e.message}`); } finally { await s.close(); }
  }

  // 2. The keyboard-bar fixture (resizes-content): the bar rides the keyboard at each pose and across a fold.
  if (plans['keyboard-bar'] && leg('kbar')) {
    const s = await open({ host: 'ios', app: 'duo-lab', plan: plans['keyboard-bar'] });
    try {
      await eachPose(s, 'kbar', async ({ l, st }, tag, deg) => {
        const fact0 = st.resources?.viewport;
        check(fact0 && fact0.width === l.viewport.w && fact0.height === l.viewport.h, `${tag}: the viewport fact ${JSON.stringify(fact0)} is the layout's ${JSON.stringify(l.viewport)}`);
        const bar0 = box(l, 'bar'), bottom0 = l.env['safe-area-inset-bottom'];
        check(bar0 && near(bar0.y + bar0.h, l.viewport.h - bottom0), `${tag}: the bar sits on the bottom inset ${JSON.stringify(bar0)} in ${l.viewport.h} inset ${bottom0}`);
        await s.type('note', 'hi');
        let { l: l2 } = await keyboard(s, true);
        const kb = l2.env['keyboard-inset-height'], bar = box(l2, 'bar');
        if (rose(kb, tag)) {
          check(kb > 100, `${tag}: the software keyboard rose: keyboard-inset-height ${kb}`);
          check(near(l2.viewport.h, l.viewport.h - kb) && box(l2, 'root').h === l2.viewport.h, `${tag}: the layout viewport ends at the keyboard ${JSON.stringify(l2.viewport)} (was ${JSON.stringify(l.viewport)}, kb ${kb})`);
          check(l2.env['safe-area-inset-bottom'] === 0, `${tag}: the bottom inset is the keyboard's while it is up`);
          check(bar && near(bar.y + bar.h, l2.viewport.h) && bar.y < bar0.y, `${tag}: the bar rides on the keyboard ${JSON.stringify(bar)}`);
        }
        // Fold with the keyboard up: the bar still ends where the viewport does on the new panel.
        const next = after(deg);
        await pose(s, next);
        ({ l: l2 } = await keyboard(s, true));
        const barF = box(l2, 'bar'), kbF = l2.env['keyboard-inset-height'];
        if (rose(kbF, `${tag}→${next}°`)) check(kbF > 0 && barF && near(barF.y + barF.h, l2.viewport.h) && box(l2, 'root').h === l2.viewport.h, `${tag}→${next}°: the bar ends at the viewport's bottom with the keyboard ${kbF} ${JSON.stringify(barF)} in ${JSON.stringify(l2.viewport)}`);
        await pose(s, deg);
        await s.tap('dismiss');
        const l3 = await down(s, tag);
        check(l3.viewport.h === l.viewport.h && l3.env['safe-area-inset-bottom'] === bottom0 && box(l3, 'bar').y === bar0.y, `${tag}: after the keyboard went everything is back ${JSON.stringify(l3.viewport)} bar ${box(l3, 'bar')?.y} (was ${bar0.y})`);
      }, 'dismiss');
    } catch (e) { check(false, `keyboard-bar: ${e.message}`); } finally { await s.close(); }
  }

  // 3. Duo Lab's four screens.
  if (leg('lab')) {
    const s = await open({ host: 'ios', app: 'duo-lab' });
    try {
      where = 'lab';
      await settle(s);
      // Fold: the facts, the panes on the segments, a selection kept across every pose.
      check(byTestId(await s.tree(), 'screen-fold') != null, 'duo-lab opens on the Fold screen');
      await s.tap('item-2'); await settle(s);
      const chosen = byTestId(await s.tree(), 'detail-title')?.props.text;
      check(typeof chosen === 'string' && chosen.length > 0, `item-2 selected a detail: ${chosen}`);
      await eachPose(s, 'fold', async ({ l }, tag, deg) => {
        const t = await s.tree();
        const two = folded(deg);
        const text = (id) => byTestId(t, id)?.props.text;
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
        // The panes sit exactly on the two rects, whichever axis the device splits
        // (across: each pane the viewport's height; down: each the viewport's width).
        const list = box(l, 'pane-list'), detail = box(l, 'pane-detail');
        const on = (pane, r, full) => pane && near(pane.x, r[0]) && near(pane.y, r[1]) && near(pane.w, full ? l.viewport.w : r[2]) && near(pane.h, full ? r[3] : l.viewport.h);
        if (segs?.length === 2 && (h === 2 || v === 2)) check(on(list, segs[0], v === 2) && on(detail, segs[1], v === 2), `${tag}: the panes sit on the segments ${JSON.stringify(segs)} (${h}×${v}): list ${JSON.stringify(list)} detail ${JSON.stringify(detail)}`);
        else check(list && detail && near(list.w, l.viewport.w) && near(detail.w, l.viewport.w) && detail.y >= list.y + list.h - 0.01, `${tag}: one pane, the list above the detail: list ${JSON.stringify(list)} detail ${JSON.stringify(detail)}`);
        check(text('detail-title') === chosen, `${tag}: the selection survived the fold: ${text('detail-title')} (was ${chosen})`);
      });
      // Images: a virtualized list of image rows, folded mid-scroll, twice.
      await s.tap('tab-images'); await settle(s);
      check(byTestId(await s.tree(), 'screen-images') != null, 'the Images tab opened');
      // The row under the list's top edge and how far up it sits: a fold keeps both.
      const anchor = (l) => {
        const list = box(l, 'image-list');
        const row = list && l.nodes.find((n) => n.testId?.startsWith('image-row-') && n.y <= list.y && n.y + n.h > list.y);
        return row ? { id: row.testId, by: list.y - row.y, sy: list.sy } : null;
      };
      await eachPose(s, 'images', async ({ l }, tag, deg) => {
        check(box(l, 'image-list') != null, `${tag}: the image list is up`);
        // Twice: out and back, then out again, the next pose going on from there
        for (let round = 0; round < 2; round++) {
          await s.tap('image-list', { wheel: [0, 900] }); await settle(s);
          const before = anchor(await s.layout());
          check(before && before.sy > 0, `${tag}: the list scrolled ${JSON.stringify(before)}`);
          const next = after(deg);
          await pose(s, next);
          const l2 = await s.layout(), now = anchor(l2), row = before && box(l2, before.id);
          check(before && row && near(box(l2, 'image-list').y - row.y, before.by, 1), `${tag}→${next}° round ${round + 1}: the row under the top edge stayed there across the fold: ${JSON.stringify(before)} → ${JSON.stringify(now)}`);
          if (round === 0) await pose(s, deg);
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
        const l2 = await s.layout(), port = box(l2, 'reflow-scroll');
        const afterFold = port?.sy ?? 0;
        // The document reflows, so the offset moves; it stays inside the content (`oy`: how far past an end it sits)
        check(before > 0 && afterFold > 0 && port.oy == null && box(l2, 'document') != null, `${tag}→${next}°: the document is up and its offset survived inside the content (${before} → ${afterFold}${port?.oy != null ? `, ${port.oy} past an end` : ''})`);
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
        // Each pose writes its own, so a value left from the last cannot pass.
        const title = `fold ${tag}`, draft = `fold me ${tag}`;
        await s.type('note-title-input', title);
        let { l } = await keyboard(s, true);
        const kb = l.env['keyboard-inset-height'];
        if (rose(kb, tag)) check(kb > 100, `${tag}: the keyboard rose under the sheet (${kb})`);
        await s.type('note-textarea', draft);
        await keyboard(s, true);
        // The sheet's height springs to the shortened viewport: read it settled, in the same layout as the viewport
        await settle(s);
        l = await s.layout();
        const sheet = box(l, 'note-sheet');
        check(sheet && near(sheet.y + sheet.h, l.viewport.h) && (!softKeyboard || l.env['keyboard-inset-height'] > 0), `${tag}: the sheet sits on the keyboard-shortened viewport ${JSON.stringify(sheet)} in ${JSON.stringify(l.viewport)} kb ${l.env['keyboard-inset-height']}`);
        const next = after(deg);
        await pose(s, next);
        ({ l } = await keyboard(s, true));
        t = await s.tree();
        const st = await s.state(), kbF = l.env['keyboard-inset-height'];
        check(byTestId(t, 'screen-note') != null && byTestId(t, 'note-sheet') != null, `${tag}→${next}°: the pushed screen and its sheet survived the fold`);
        check(st.slots.title === title && st.slots.draft === draft && st.slots.noteFocused === true, `${tag}→${next}°: the title, the draft and the focus survived ${JSON.stringify({ title: st.slots.title, draft: st.slots.draft, noteFocused: st.slots.noteFocused })}`);
        if (rose(kbF, `${tag}→${next}°`)) check(kbF > 0, `${tag}→${next}°: the keyboard is up under the sheet (${kbF})`);
        check(near((box(l, 'note-sheet')?.y ?? 0) + (box(l, 'note-sheet')?.h ?? 0), l.viewport.h), `${tag}→${next}°: the sheet still ends at the viewport ${JSON.stringify(box(l, 'note-sheet'))} in ${JSON.stringify(l.viewport)}`);
        // Back across the reverse fold with the pushed screen and its sheet up
        await pose(s, deg);
        await s.tap('note-dismiss'); await down(s, tag);
        check((await s.state()).slots.noteFocused === false, `${tag}: dismiss ended editing`);
        await s.tap('note-back'); await settle(s);
        t = await s.tree();
        check(byTestId(t, 'screen-combos') != null && byTestId(t, 'screen-note') == null, `${tag}: back popped the note screen`);
        const echo = byTestId(t, 'draft-echo')?.props.text;
        check(echo === `Draft so far: ${title} / ${draft}`, `${tag}: the draft is echoed on the Combinations screen: ${echo}`);
      });
    } catch (e) { check(false, `duo-lab: ${e.message}`); } finally { await s.close(); }
  }

  // 4. The two-session sample host: both panes follow the pose.
  const hostBundle = appleArtifacts(resolveApp('caltrain'), { destination: 'ios-simulator', host: true }).bundle;
  if (!leg('host')) { /* not asked for */ }
  else if (!existsSync(hostBundle)) console.log('duo host: unsupported — run bun host/apple/build.mjs --ios --host first');
  else {
    let s = null;
    try {
      s = await open({ host: 'host-ios', session: 'a' }); // a stale bundle is refused by name: reported, not a crash
      await settle(s);
      await eachPose(s, 'host', async ({ l }, tag) => {
        let lb;
        try { s.session = 'b'; lb = await s.layout(); } finally { s.session = 'a'; }
        const fold = (x) => `${x.viewport.w}×${x.viewport.h} ${x.env['device-posture']} ${x.env['horizontal-viewport-segments']}×${x.env['vertical-viewport-segments']}`;
        check(l.viewport.w > 0 && l.viewport.h > 0 && fold(l) === fold(lb), `${tag}: both sessions follow the pose (a ${fold(l)}, b ${fold(lb)})`);
      });
    } catch (e) { check(false, `host: ${e.message}`); } finally { await s?.close(); }
  }
}
