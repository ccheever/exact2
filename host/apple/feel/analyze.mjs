#!/usr/bin/env bun
// Turns a probe's trace (feel.m's lines) and gestures (probe.mjs) into feel
// metrics with a verdict each: ok, tell (someone who knows iOS would notice)
// or bad. The thresholds are UIKit's behaviour as measured on the simulator
// (README.md, "Calibration").
//   bun host/apple/feel/analyze.mjs DIR            re-analyze one probe's output, rewriting feel.json and feel.txt
//   bun host/apple/feel/analyze.mjs DIR DIR …      and a table comparing them
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

/** The list to probe in a scan: a shown, reachable, enabled scroll view whose content is meaningfully taller than its bounds; the largest. */
export function pickList(scan) {
  const views = (scan?.views ?? []).filter((v) => v.shown && v.enabled && v.reach && v.frame[3] >= 200 && v.cs[1] > v.frame[3] * 1.5 + 100);
  return views.sort((a, b) => b.frame[2] * b.frame[3] - a.frame[2] * a.frame[3])[0] ?? null;
}

/** The trace's lines by kind. */
export function readTrace(text) {
  const t = { hello: null, frames: [], scans: [], touches: [], blanks: [], bchecks: [], checked: [], wheres: [] };
  for (const line of text.split('\n')) {
    if (!line) continue;
    let o; try { o = JSON.parse(line); } catch { continue; } // a line cut by a flush in progress
    if (o.k === 'hello') t.hello = o;
    else if (o.k === 'f') t.frames.push(o);
    else if (o.k === 'scan') t.scans.push(o);
    else if (o.k === 'touch') t.touches.push(o);
    else if (o.k === 'blank') t.blanks.push(o);
    else if (o.k === 'bcheck') t.bchecks.push(o);
    else if (o.k === 'checked') t.checked.push(o);
    else if (o.k === 'where') t.wheres.push(o);
  }
  return t;
}

const mean = (a) => a.length ? a.reduce((s, x) => s + x, 0) / a.length : null;
const pct = (a, p) => { if (!a.length) return null; const s = [...a].sort((x, y) => x - y); return s[Math.min(s.length - 1, Math.floor(p * s.length))]; };
const round = (x, d = 1) => x == null || !Number.isFinite(x) ? null : Math.round(x * 10 ** d) / 10 ** d;
/** Least squares y = a + b x, with R². */
function fit(xs, ys) {
  const n = xs.length, mx = mean(xs), my = mean(ys);
  let sxy = 0, sxx = 0, syy = 0;
  for (let i = 0; i < n; i++) { sxy += (xs[i] - mx) * (ys[i] - my); sxx += (xs[i] - mx) ** 2; syy += (ys[i] - my) ** 2; }
  const b = sxx ? sxy / sxx : 0;
  return { a: my - b * mx, b, r2: syy ? (sxy * sxy) / (sxx * syy) : 1 };
}
/** ok when `x` is within `ok`, tell within `tell`, else bad; each [lo, hi]. */
const band = (x, ok, tell) => x == null ? 'n/a' : x >= ok[0] && x <= ok[1] ? 'ok' : x >= tell[0] && x <= tell[1] ? 'tell' : 'bad';

// The thresholds. Hitch time ratio is Apple's (ms of hitch per s of motion: under 5 good, over 10 bad); the
// rest is UIKit's behaviour on the simulator with margin (README.md, "Calibration").
const T = {
  hitch: [[0, 5], [0, 10]], stall: [[0, 34], [0, 67]], busyShare: [[0, 0.5], [0, 0.8]],
  ratio: [[0.97, 1.03], [0.9, 1.1]], lag: [[0, 0], [0, 1]], glide: [[0, 5], [0, 30]],
  tau: [[440, 560], [380, 650]], handoff: [[0.8, 1.25], [0.65, 1.5]], jumps: [[0, 0], [0, 2]],
  band: [[0.3, 0.65], [0.15, 0.85]], settle: [[0.25, 1.0], [0.15, 1.5]], toTop: [[0, 1.0], [0, 2.0]],
  blank: [[0, 0], [0, 0.02]],
};

/** A late frame's cause: `tick` when a timer's callout took at least a quarter of its sampled busy time (4 ms or
 * more), else `scroll`. */
function causeOfFrame(w) {
  const busy = Object.values(w.ms ?? {}).reduce((a, b) => a + b, 0), timer = w.via?.timers ?? 0;
  return timer >= 4 && timer >= busy / 4 ? 'tick' : 'scroll';
}

/** Late frames' sampled main-thread time summed: by who (the label before ` · `), each with its what and ms, and by
 * the run loop's callout it ran in (`via`). */
export function whereOf(hitches) {
  const who = new Map(), via = {};
  for (const h of hitches) {
    if (!h.where) continue;
    for (const [k, ms] of Object.entries(h.via ?? {})) via[k] = (via[k] ?? 0) + ms;
    for (const [label, ms] of Object.entries(h.where)) {
      const [w, what] = label.split(' · ');
      const e = who.get(w) ?? { who: w, ms: 0, what: {} };
      e.ms += ms;
      if (what) e.what[what] = (e.what[what] ?? 0) + ms;
      who.set(w, e);
    }
  }
  const by = [...who.values()].sort((a, b) => b.ms - a.ms).map((e) => ({ who: e.who, ms: round(e.ms),
    what: Object.entries(e.what).sort((a, b) => b[1] - a[1]).map(([k, v]) => [k, round(v)]) }));
  return { by, via: Object.fromEntries(Object.entries(via).map(([k, ms]) => [k, round(ms)])) };
}

/** The late frames by gesture, each with where their time went. */
function whereClusters(hitches) {
  const gestures = [...new Set(hitches.filter((h) => h.where).map((h) => h.gesture))];
  return gestures.map((g) => {
    const hs = hitches.filter((h) => h.gesture === g);
    return { gesture: g, frames: hs.length, lateMs: round(hs.reduce((sum, h) => sum + (h.late ?? 0), 0)),
      ticks: hs.filter((h) => h.cause === 'tick').length, ...whereOf(hs) };
  });
}

/** One cluster as a line: `who ms (what ms)` for the three that took most, and what woke the main thread. */
export function whereLine(c) {
  const parts = c.by.filter((b) => b.ms >= 2).slice(0, 3).map((b) => `${b.who} ${Math.round(b.ms)} ms${b.what[0]?.[1] >= 2 ? ` (in ${b.what[0][0]} ${Math.round(b.what[0][1])})` : ''}`);
  const via = Object.entries(c.via).filter(([, ms]) => ms >= 2).sort((a, b) => b[1] - a[1]).map(([k, ms]) => `${k} ${Math.round(ms)} ms`).join(', ');
  return `${c.gesture}: ${c.frames} late frame${c.frames === 1 ? '' : 's'} (${c.ticks} on a timer's tick), ${Math.round(c.lateMs)} ms late; ${parts.join(' · ') || 'the main thread was idle (the delay was elsewhere)'}${via ? `; in ${via}` : ''}`;
}

export function analyze(t, meta) {
  const P = 1 / (t.hello?.maxFps || 60);
  const media = (wall) => t.hello.media + (wall - t.hello.wall) / 1000;
  const frames = t.frames;
  const sampled = new Map(t.blanks.map((b) => [b.t.toFixed(4), b]));
  const gestures = meta.gestures.slice(meta.steps ?? 0).map((g) => ({ ...g, m0: media(g.start) - 0.03, m1: media(g.end) }));
  gestures.forEach((g, i) => { g.until = gestures[i + 1]?.m0 ?? Infinity; });
  const gestureAt = (time) => gestures.findLast((g) => g.m0 <= time)?.name ?? 'launch';
  const listId = meta.listId ?? frames.findLast((f) => f.v != null)?.v;
  // A scrub crosses the list in a second: timed apart (`out.scrub`), so the hitch ratio stays the scrolling a finger does.
  const scrubs = gestures.filter((g) => g.kind === 'scrub');
  const inScrub = (time) => scrubs.some((g) => time >= g.m0 && time < g.until);
  const scanView = (s) => s?.views.find((v) => v.id === listId);
  const config = scanView(t.scans.findLast((s) => scanView(s))) ?? null;

  // Where the content shows: `vy`, the offset less the content's shift under it so far. A list that measures rows
  // above the viewport moves its content under the offset (`ag`, feel.m) and its offset by as much in the same
  // frame, which nobody sees; motion is measured on `vy`.
  let shifted = 0;
  for (const f of frames) if (f.v === listId) { shifted += f.ag ?? 0; f.vy = f.oy - shifted; }
  // Consecutive frames of the list: velocity (pt/s, + is down the list) and the app's main-thread time.
  const steps = [];
  for (let i = 1; i < frames.length; i++) {
    const a = frames[i - 1], b = frames[i];
    // Consecutive ticks only: `dt` is the recorder's time since its previous tick, recorded or not.
    if (b.v !== listId || a.v !== listId || b.t <= a.t || Math.abs(b.t - a.t - b.dt / 1000) > 0.002) continue;
    const prior = sampled.get(a.t.toFixed(4));
    steps.push({ f: b, prev: a, dt: b.t - a.t, v: (b.vy - a.vy) / (b.t - a.t), shift: b.ag ?? 0,
      app: b.busy - a.own - (prior?.cost ?? 0) });
  }
  const minOff = (f) => -f.it, maxOff = (f) => Math.max(-f.it, f.ch - f.bh + f.ib);
  const inside = (f) => f.oy >= minOff(f) - 0.5 && f.oy <= maxOff(f) + 0.5;

  // Smoothness over every moving frame while blanks were not sampled (a sample costs the app's main thread).
  const from = gestures[0]?.m0 ?? 0; // not the navigation steps
  const moving = steps.filter((s) => s.f.mv && !s.f.s && !s.prev.s && s.f.t >= from && !inScrub(s.f.t));
  const wheres = new Map(t.wheres.map((w) => [w.t.toFixed(4), w]));
  const hitchMs = moving.reduce((sum, s) => sum + (s.dt > 1.5 * P ? (s.dt - P) * 1000 : 0), 0);
  const motionS = moving.reduce((sum, s) => sum + s.dt, 0);
  /** Hitches and main-thread time over `moving` (consecutive frames of the list in motion, unsampled). */
  const smoothOf = (moving) => {
    const hitchMs = moving.reduce((sum, s) => sum + (s.dt > 1.5 * P ? (s.dt - P) * 1000 : 0), 0);
    const motionS = moving.reduce((sum, s) => sum + s.dt, 0);
    return {
      frames: moving.length, motionS: round(motionS, 2), fps: round(t.hello?.maxFps, 0),
      hitchRatio: round(motionS ? hitchMs / motionS : null), longestFrameMs: round(Math.max(0, ...moving.map((s) => s.dt * 1000))),
      hitches: moving.filter((s) => s.dt > 1.5 * P).length,
      hitchAt: moving.filter((s) => s.dt > 1.5 * P).map((s) => {
        // Where the main thread's time went (feel.m's sampler) and what woke it: a hitch on a timer's tick is the
        // app updating itself; any other is the scrolling (rows coming on screen, their layout and commit).
        const w = wheres.get(s.f.t.toFixed(4));
        return { gesture: gestureAt(s.f.t), ms: round(s.dt * 1000), mainMs: round(s.app, 1), late: (s.dt - P) * 1000,
          ...(w ? { where: w.ms, via: w.via ?? {}, cause: causeOfFrame(w) } : {}) };
      }),
      mainMs: round(mean(moving.map((s) => s.app)), 2), mainP95Ms: round(pct(moving.map((s) => s.app), 0.95), 2),
    };
  };
  const smooth = smoothOf(moving);
  // The hitch time by cause, per s of motion: timer ticks against scrolling (hitches the sampler saw).
  const lateBy = (cause) => smooth.hitchAt.filter((h) => h.cause === cause).reduce((sum, h) => sum + h.late, 0);
  if (t.wheres.length || smooth.hitchAt.some((h) => h.where)) Object.assign(smooth, { tickHitchRatio: round(motionS ? lateBy('tick') / motionS : null), scrollHitchRatio: round(motionS ? lateBy('scroll') / motionS : null) });
  for (const h of smooth.hitchAt) h.late = round(h.late);
  smooth.where = whereClusters(smooth.hitchAt);
  smooth.verdict = { hitch: band(smooth.hitchRatio, ...T.hitch), stall: band(smooth.longestFrameMs, ...T.stall),
    main: band(smooth.mainP95Ms == null ? null : smooth.mainP95Ms / (P * 1000), ...T.busyShare) };

  const touchesIn = (g) => t.touches.filter((x) => x.t >= g.m0 && x.t < g.until);
  const framesIn = (g, from = g.m0, to = g.until) => frames.filter((f) => f.t >= from && f.t < to && f.v === listId);
  const stepsIn = (from, to) => steps.filter((s) => s.f.t >= from && s.f.t < to);
  const byName = (re) => gestures.filter((g) => re.test(g.name));
  const out = { smooth };

  // Finger tracking: the slow drag, finger position at each frame's callback against where the content shows (`vy`).
  const slow = byName(/^slow-drag$/)[0];
  if (slow) {
    const tt = touchesIn(slow), began = tt.find((x) => x.ph === 'began'), ended = tt.findLast((x) => x.ph === 'ended');
    const fs = framesIn(slow).filter((f) => f.down > 0 && began && f.c >= began.t);
    const fingerAt = (c) => tt.findLast((x) => x.t <= c && (x.ph === 'began' || x.ph === 'moved'))?.y;
    const o0 = fs[0]?.vy;
    const firstMove = fs.findIndex((f) => Math.abs(f.vy - o0) > 0.5);
    let best = null;
    if (firstMove > 0) {
      // Lag k: content at frame i against the finger k frames earlier; the k that fits best.
      for (let k = 0; k <= 3; k++) {
        const xs = [], ys = [];
        for (let i = Math.max(firstMove + 1, k); i < fs.length; i++) {
          const y = fingerAt(fs[i - k].c);
          if (y == null || !inside(fs[i])) continue;
          xs.push(y); ys.push(fs[i].vy);
        }
        if (xs.length < 5) continue;
        const r = fit(xs, ys);
        const err = Math.max(...xs.map((x, j) => Math.abs(ys[j] - (r.a + r.b * x))));
        if (!best || err < best.err) best = { k, ratio: -r.b, err };
      }
    }
    const lift = ended?.t ?? slow.m1;
    const after = framesIn(slow, lift);
    const atLift = fs.findLast((f) => f.c <= lift + P) ?? fs.at(-1);
    out.tracking = {
      ratio: round(best?.ratio, 3), lagFrames: best?.k ?? null, maxErrorPt: round(best?.err),
      slopPt: firstMove > 0 && began ? round(Math.abs(began.y - fingerAt(fs[firstMove].c)) - Math.abs(fs[firstMove].vy - o0)) : null,
      glidePt: atLift && after.length ? round(Math.max(...after.map((f) => Math.abs(f.vy - atLift.vy)))) : null,
    };
    out.tracking.verdict = { ratio: band(out.tracking.ratio, ...T.ratio), lag: band(out.tracking.lagFrames, ...T.lag), glide: band(out.tracking.glidePt, ...T.glide) };
  }

  // Momentum: each fling's deceleration fitted as v(t) = v0 · rate^t (UIKit's .normal: 0.998 a ms, τ 500 ms).
  out.flings = [];
  for (const g of byName(/^fling/)) {
    const tt = touchesIn(g), ended = tt.findLast((x) => x.ph === 'ended');
    if (!ended) continue;
    const next = t.touches.find((x) => x.t > ended.t && x.ph === 'began')?.t ?? Infinity;
    const moves = tt.filter((x) => x.ph === 'moved' && x.t <= ended.t);
    const last = moves.at(-1), base = moves.findLast((x) => last && last.t - x.t >= 0.03);
    const finger = last && base ? -(last.y - base.y) / (last.t - base.t) : null;
    // A fling that reaches the list's end decelerates until then: the bounce back, which `edgeBounce` measures, is
    // no part of its curve. Its last frames settle within the half point `inside` allows (notes-exact's 6,000 pt/s
    // fling, rounds r25 to r29: two still frames 0.47 pt past an end off the pixel grid), and read as jumps.
    const glide = stepsIn(ended.t + P, Math.min(next, g.until)), edgeAt = glide.find((s) => !inside(s.f))?.f.t ?? Infinity;
    const dec = glide.filter((s) => s.f.t < edgeAt && s.f.down === 0 && s.f.de && inside(s.f) && inside(s.prev));
    const dir = Math.sign(dec.reduce((sum, s) => sum + s.v, 0));
    const usable = dec.filter((s) => Math.abs(s.v) > 60 && Math.sign(s.v) === dir);
    const r = { name: g.name, asked: g.velocity, fingerV: round(finger, 0) };
    if (usable.length >= 6) {
      const ms = usable.map((s) => (s.f.t - ended.t) * 1000), f = fit(ms, usable.map((s) => Math.log(Math.abs(s.v))));
      const predicted = (s) => dir * Math.exp(f.a + f.b * (s.f.t - ended.t) * 1000);
      // A frame whose step is far from the fitted curve: a stall, a frozen frame or a jump in the offset, the
      // content moving back included (fitted on the frames that move the fling's way). A still frame in the two
      // after the lift is the hand-off's (`liftFrameMs`); UIKit's lists have one too.
      const handing = (s) => s.f.t - ended.t < 2 * P + 0.002 && Math.abs(s.v) < 60;
      const jumpers = dec.filter((s) => !s.f.s && !s.prev.s && !handing(s) && Math.abs(s.v - predicted(s)) > Math.max(0.15 * Math.abs(predicted(s)), 150)), jumps = jumpers.length;
      Object.assign(r, {
        tauMs: round(-1 / f.b, 0), ratePerMs: round(Math.exp(f.b), 5), r2: round(f.r2, 4), contentV: round(Math.exp(f.a) * Math.sign(usable[0].v), 0),
        // Against the asked speed: the synthesizer's moves are too few and uneven to give the finger's speed at the lift.
        handoff: round(Math.exp(f.a) / g.velocity, 2), frames: usable.length, jumps,
        jumpAt: jumpers.map((s) => ({ ms: round((s.f.t - ended.t) * 1000, 0), v: round(s.v, 0), fit: round(predicted(s), 0), dtMs: round(s.dt * 1000, 1) })),
        travelPt: round(Math.abs(dec.at(-1).f.vy - dec[0].prev.vy), 0),
      });
    }
    // The hand-off itself: the longest of the three frames after the lift, when blanks were not being sampled.
    const atLift = steps.filter((s) => s.f.t > ended.t - P && s.f.t <= ended.t + 3 * P + 1e-3);
    if (atLift.length && !atLift.some((s) => s.f.s || s.prev.s)) Object.assign(r, { liftFrameMs: round(Math.max(...atLift.map((s) => s.dt)) * 1000), liftMainMs: round(Math.max(...atLift.map((s) => s.app))) });
    const all = framesIn(g);
    r.contentSizeChanges = all.filter((f, i) => i && f.ch !== all[i - 1].ch).length;
    r.contentShifts = dec.filter((s) => Math.abs(s.shift) > 0.5).length;
    out.flings.push(r);
  }
  const fitted = out.flings.filter((f) => f.tauMs != null && !/^fling-up|for-tap/.test(f.name));
  out.momentum = {
    tauMs: round(pct(fitted.map((f) => f.tauMs), 0.5), 0), handoff: round(pct(fitted.map((f) => f.handoff).filter((x) => x != null), 0.5), 2),
    jumps: out.flings.reduce((s, f) => s + (f.jumps ?? 0), 0), contentSizeChanges: out.flings.reduce((s, f) => s + f.contentSizeChanges, 0),
    contentShifts: out.flings.reduce((s, f) => s + f.contentShifts, 0),
  };
  const lifts = out.flings.filter((f) => f.liftFrameMs != null);
  out.momentum.liftDrops = lifts.filter((f) => f.liftFrameMs > 1.5 * P * 1000).length;
  out.momentum.lifts = lifts.length;
  out.momentum.liftMainMs = round(Math.max(0, ...lifts.map((f) => f.liftMainMs)));
  out.momentum.verdict = { lift: lifts.length ? band(out.momentum.liftDrops / lifts.length, [0, 0], [0, 0.34]) : 'n/a', tau: band(out.momentum.tauMs, ...T.tau), handoff: band(out.momentum.handoff, ...T.handoff), jumps: band(out.momentum.jumps, ...T.jumps) };

  // Edges: momentum carried past the top or bottom, and how long it takes to come back.
  const edge = [];
  for (const g of gestures) {
    const ended = touchesIn(g).findLast((x) => x.ph === 'ended');
    if (!ended || g.name === 'pull-top' || /^(h-|scrub)/.test(g.name)) continue;
    const fs = framesIn(g, ended.t);
    const out1 = fs.findIndex((f) => !inside(f));
    if (out1 < 0) continue;
    const back = fs.findIndex((f, i) => i > out1 && fs.slice(i).every((x) => inside(x) || Math.min(Math.abs(x.oy - minOff(x)), Math.abs(x.oy - maxOff(x))) < 0.5));
    const over = Math.max(...fs.map((f) => Math.max(minOff(f) - f.oy, f.oy - maxOff(f))));
    const vIn = stepsIn(ended.t, fs[out1].t + 1e-6).at(-1)?.v;
    edge.push({ name: g.name, overshootPt: round(over), entryV: round(vIn, 0), settleS: back > 0 ? round(fs[back].t - fs[out1].t, 2) : null });
  }
  out.edgeBounce = edge.length ? { ...edge.at(-1), verdict: { settle: band(edge.at(-1).settleS, ...T.settle) } } : { verdict: { settle: 'n/a' }, note: 'no fling reached an edge' };

  // Rubber band: pulled past the top and held, then released.
  const pull = byName(/^pull-top$/)[0];
  if (pull) {
    const tt = touchesIn(pull), ended = tt.findLast((x) => x.ph === 'ended');
    const fs = framesIn(pull);
    const fingerAt = (c) => tt.findLast((x) => x.t <= c && x.ph !== 'ended')?.y;
    const cross = fs.findIndex((f) => f.oy < minOff(f) - 0.5);
    const held = fs.filter((f) => f.down > 0);
    const deepest = held.reduce((m, f) => (m && m.oy - minOff(m) <= f.oy - minOff(f) ? m : f), null);
    const over = deepest ? minOff(deepest) - deepest.oy : 0;
    const travel = cross > 0 && deepest ? fingerAt(deepest.c) - fingerAt(fs[cross - 1].c) : null;
    const after = ended ? fs.filter((f) => f.t >= ended.t) : [];
    const back = after.findIndex((f, i) => after.slice(i).every((x) => Math.abs(x.oy - minOff(x)) < 0.5));
    out.rubberBand = { overscrollPt: round(over), fingerPt: round(travel), ratio: travel ? round(over / travel, 2) : null,
      settleS: ended && back >= 0 ? round(after[back].t - ended.t, 2) : null, bounces: config?.bounces ?? null };
    // A pull that began away from the top measures no rubber band.
    const before = frames.findLast((f) => f.t < pull.m0 && f.v === listId), fromTop = !before || before.oy <= minOff(before) + 1;
    out.rubberBand.verdict = !fromTop ? { ratio: 'n/a', settle: 'n/a' } : { ratio: over < 1 ? 'bad' : band(out.rubberBand.ratio, ...T.band), settle: band(out.rubberBand.settleS, ...T.settle) };
  }

  // Scroll to top from a status-bar tap.
  const bar = byName(/^status-bar$/)[0];
  if (bar) {
    const fs = framesIn(bar, bar.m0 - 0.5);
    const before = fs.findLast((f) => f.t < bar.m0) ?? frames.findLast((f) => f.t < bar.m0 && f.v === listId);
    const moved = fs.find((f) => f.t >= bar.m0 && before && Math.abs(f.oy - before.oy) > 0.5);
    const top = fs.find((f) => f.t >= bar.m0 && f.oy <= minOff(f) + 1);
    const far = before && before.oy - minOff(before) >= 50;
    out.scrollToTop = { fromPt: round(before ? before.oy - minOff(before) : null, 0), reached: !!top,
      startS: moved ? round(moved.t - bar.m0, 2) : null, durationS: top && moved ? round(top.t - moved.t, 2) : null, scrollsToTop: config?.scrollsToTop ?? null };
    out.scrollToTop.verdict = { reached: !far ? 'n/a' : top ? band(top.t - bar.m0, ...T.toTop) : 'bad' };
  }

  // A tap during deceleration: the list stops at the touch, and nothing is activated.
  const tap = byName(/^tap-decelerating$/)[0];
  if (tap) {
    const tt = touchesIn(tap), began = tt.find((x) => x.ph === 'began');
    const pre = stepsIn(tap.m0 - 0.1, began?.t ?? tap.m1).filter((s) => s.f.c < (began?.t ?? Infinity));
    const fs = began ? framesIn(tap, began.t) : [];
    const stopI = fs.findIndex((f, i) => i && fs.slice(i).every((x) => Math.abs(x.vy - f.vy) < 0.5));
    // The list as the scan after the tap had it, before any later gesture.
    const next = gestures[gestures.indexOf(tap) + 1]?.m0 ?? Infinity;
    const view = scanView(t.scans.findLast((s) => s.t < next) ?? t.scans.at(-1));
    const activated = !view || !view.reach || !view.shown;
    const v = out.tapToStop = { beforeV: round(pre.at(-1)?.v, 0), stopFrames: stopI >= 0 ? stopI : null,
      travelPt: fs.length ? round(Math.abs(fs.at(-1).vy - fs[0].vy)) : null, activated };
    v.verdict = { stop: !began || Math.abs(v.beforeV ?? 0) < 100 ? 'n/a' : activated ? 'bad' : v.travelPt < 5 ? 'ok' : v.travelPt < 50 ? 'tell' : 'bad' };
  }

  // Blank bands: rows not yet drawn where the list shows content.
  const movingAt = new Set(frames.filter((f) => f.mv).map((f) => f.t.toFixed(4)));
  // A uniform band is a blank only where the same frame drawn again at rest has ink in it (feel.m `check`): the
  // checked bands give the share of the rest. A trace without the check counts every uniform band, unsure.
  const checks = new Map(t.bchecks.filter((c) => !c.gone && !c.unjudged).map((c) => [c.t.toFixed(4), c]));
  const verified = t.checked.length > 0;
  const blankOf = (samples) => {
    const uniform = samples.filter((b) => b.blank >= 60);
    const checkedOnes = uniform.filter((b) => checks.has(b.t.toFixed(4)));
    const late = checkedOnes.filter((b) => checks.get(b.t.toFixed(4)).late >= 60);
    // Every uniform band left unjudged (a list whose content moved under the offset): unknown, not none.
    const unjudged = verified && uniform.length > 0 && !checkedOnes.length;
    const blankSamples = !verified ? uniform.length : unjudged ? null : uniform.length && late.length * uniform.length / checkedOnes.length;
    const r = { samples: samples.length, uniformSamples: uniform.length, checked: checkedOnes.length, lateChecked: late.length, verified,
      blankSamples: round(blankSamples, 1), share: samples.length && blankSamples != null ? round(blankSamples / samples.length, 3) : null,
      maxPt: round(Math.max(0, ...(verified ? late.map((b) => checks.get(b.t.toFixed(4)).late) : uniform.map((b) => b.blank)))),
      sampleMs: round(mean(samples.map((b) => b.cost)), 2) };
    r.verdict = { blank: band(r.share, ...T.blank) };
    return r;
  };
  out.blank = blankOf(t.blanks.filter((b) => movingAt.has(b.t.toFixed(4)) && !inScrub(b.t)));

  // Scrubbing: the scroll indicator dragged end to end (probe.mjs `scrub`), a screen or more of rows a frame. Timed
  // in `scrub`, blanks sampled in `scrub-sampled`.
  if (scrubs.length) {
    const timed = scrubs.find((g) => g.name === 'scrub'), sampledScrub = scrubs.find((g) => g.name === 'scrub-sampled');
    const fs = timed ? framesIn(timed) : [];
    const range = fs.length ? Math.max(1, fs.at(-1).ch - fs.at(-1).bh + fs.at(-1).ib + fs.at(-1).it) : 1;
    const crossed = fs.length ? (Math.max(...fs.map((f) => f.vy)) - Math.min(...fs.map((f) => f.vy))) / range : 0;
    const st = timed ? stepsIn(timed.m0, timed.until).filter((s) => s.f.mv && !s.f.s && !s.prev.s) : [];
    const sc = out.scrub = { took: crossed >= 0.3, crossed: round(crossed, 2), maxStepPt: round(Math.max(0, ...st.map((s) => Math.abs(s.v * s.dt))), 0) };
    if (timed && sc.took) {
      sc.smooth = smoothOf(st);
      for (const h of sc.smooth.hitchAt) h.late = round(h.late);
      sc.smooth.where = whereClusters(sc.smooth.hitchAt);
    }
    if (sampledScrub) {
      const own = t.blanks.filter((b) => movingAt.has(b.t.toFixed(4)) && b.t >= sampledScrub.m0 && b.t < sampledScrub.until);
      sc.blank = blankOf(own);
      // Blanks that linger: those sampled once the finger has held the indicator still for 100 ms (each frame since
      // moved the list under a twentieth of its viewport).
      const moved = steps.filter((s) => s.f.t >= sampledScrub.m0 && s.f.t < sampledScrub.until && Math.abs(s.v * s.dt) >= 0.05 * (s.f.bh - s.f.it - s.f.ib));
      sc.heldBlank = blankOf(own.filter((b) => { const last = moved.findLast((s) => s.f.t <= b.t + 1e-4); return last && b.t - last.f.t >= 0.1; }));
    }
    sc.verdict = { hitch: sc.smooth ? band(sc.smooth.hitchRatio, ...T.hitch) : 'n/a', stall: sc.smooth ? band(sc.smooth.longestFrameMs, ...T.stall) : 'n/a',
      blank: sc.blank && sc.took ? sc.blank.verdict.blank : 'n/a', held: sc.heldBlank?.samples && sc.took ? sc.heldBlank.verdict.blank : 'n/a' };
  }

  // Horizontal lists in the list (probe.mjs `horizontal`): the same metrics along x, by the vertical analysis on the
  // carousel's frames with x for y, and the two diagonal drags' arbitration.
  const mark = gestures.find((g) => g.name === 'h-start');
  if (mark && meta.axis !== 'x') {
    const C = mark.carousel, end = gestures.find((g) => g.name === 'h-end');
    const hs = meta.gestures.filter((g) => g.start >= mark.start && (!end || g.start <= end.start) && g.kind !== 'mark');
    const rename = (n) => n.replace(/^h-fling-back-/, 'fling-up-').replace(/^h-/, '');
    const tx = { ...t, blanks: [], bchecks: [], checked: [],
      frames: t.frames.filter((f) => f.v === C).map((f) => ({ ...f, oy: f.ox, ch: f.cw, bh: f.bw, it: f.il ?? 0, ib: f.ir ?? 0, ag: 0 })),
      touches: t.touches.map((x) => ({ ...x, y: x.x })) };
    const hx = analyze(tx, { app: meta.app, steps: 0, gestures: [...hs.map((g) => ({ ...g, name: rename(g.name) })), ...(end ? [end] : [])], listId: C, axis: 'x' });
    /** `id`'s offset along `axis` over time, from the frames it was the moving view in and the others it moved in. */
    const track = (id, axis) => {
      const k = axis === 'x' ? 'ox' : 'oy', i = axis === 'x' ? 1 : 2, pts = [];
      for (const f of frames) {
        if (f.v === id) pts.push([f.t, f[k]]);
        else for (const o of f.o ?? []) if (o[0] === id) pts.push([f.t, o[i]]);
      }
      return pts;
    };
    const travelIn = (pts, from, to) => {
      const before = pts.findLast((p) => p[1] != null && p[0] < from), during = pts.filter((p) => p[0] >= from && p[0] <= to);
      const base = before?.[1] ?? during[0]?.[1];
      return base == null ? 0 : Math.max(0, ...during.map((p) => Math.abs(p[1] - base)));
    };
    const diagonal = (name) => {
      const g = gestures.find((x) => x.name === name);
      if (!g) return null;
      const tt = touchesIn(g), began = tt.find((x) => x.ph === 'began'), ended = tt.findLast((x) => x.ph === 'ended');
      if (!began || !ended) return null;
      const lastMove = tt.findLast((x) => x.ph === 'moved' && x.t <= ended.t) ?? ended;
      const to = ended.t + 0.3;
      return { fingerX: round(Math.abs(lastMove.x - began.x), 0), fingerY: round(Math.abs(lastMove.y - began.y), 0),
        carouselPt: round(travelIn(track(C, 'x'), began.t, to), 0), listPt: round(travelIn(track(listId, 'y'), began.t, to), 0) };
    };
    const h = diagonal('h-diagonal'), v = diagonal('v-diagonal');
    if (h) { h.ratio = h.fingerX ? round(h.carouselPt / h.fingerX, 2) : null; h.verdict = h.listPt < 5 && h.ratio >= 0.8 ? 'ok' : h.listPt < 20 && h.ratio >= 0.5 ? 'tell' : 'bad'; }
    if (v) { v.ratio = v.fingerY ? round(v.listPt / v.fingerY, 2) : null; v.verdict = v.carouselPt < 5 && v.ratio >= 0.8 ? 'ok' : v.carouselPt < 20 && v.ratio >= 0.5 ? 'tell' : 'bad'; }
    out.horizontal = { carousel: C, list: hx.list, smooth: hx.smooth, tracking: hx.tracking ?? null, flings: hx.flings, momentum: hx.momentum, edgeBounce: hx.edgeBounce,
      across: h, along: v, verdict: { across: h?.verdict ?? 'n/a', along: v?.verdict ?? 'n/a' } };
  }
  out.list = config ? { cls: config.cls, decelerationRate: round(config.decel, 4), bounces: config.bounces, scrollsToTop: config.scrollsToTop, delaysContentTouches: config.delays, contentHeight: config.cs[1], ...(meta.axis === 'x' ? { contentWidth: config.cs[0], paging: config.paging } : {}) } : null;
  return { app: meta.app, verdict: 'probed', ...out };
}

// Each metric: its name, value and verdict; UIKit's value on the simulator (README.md, "Calibration"); and, for an app
// author, what in an app causes a tell or a bad there, or that it is Exact's own (`OWN`, the evidence named).
const OWN = (evidence) => `Exact's own: report it${evidence ? ` with ${evidence}` : ''}`;
const at = (r) => { const g = [...new Set(r.smooth.hitchAt.map((h) => h.gesture))]; return g.length ? ` (during ${g.join(', ')})` : ''; };
const ROWS = 'make the rows lighter (fewer nodes, images at the size they show, no shadow, blur or filter on each row)';
/** What took the late frames' main-thread time, for an app author, when the sampler saw it: the two that took most,
 * how many frames came on a timer's tick, and whose each is (each a quarter of the time or more). Null without samples. */
function causeOf(hitches, e) {
  const top = whereOf(hitches).by.filter((b) => b.ms >= 2).slice(0, 2);
  if (!top.length) return null;
  const ticks = hitches.filter((h) => h.cause === 'tick').length, total = top.reduce((sum, b) => sum + b.ms, 0);
  const head = `of the late frames' main thread, ${top.map((b) => `${b.who} took ${Math.round(b.ms)} ms`).join(' and ')}${ticks ? ` (${ticks} of ${hitches.length} on a timer's tick)` : ''}`;
  const whose = [...new Set(top.filter((b) => b.ms >= total / 4).map((b) =>
    /^app JS/.test(b.who) ? `the app's data module runs on the frame's critical path: do less on each ${ticks ? 'tick' : 'update'} while the list moves (fewer or smaller state changes)`
      : /^Exact/.test(b.who) ? OWN(e)
        : /^app code/.test(b.who) ? `the app's own code${b.what[0]?.[1] >= b.ms / 2 ? ` (in ${b.what[0][0]})` : ''} does this; ${ticks ? 'make the timer\'s work cheaper' : ROWS}`
          : `the platform's ${b.who} work for the rows; ${ROWS}`))];
  return `${head}: ${whose.join('; and ')}`;
}
const VERDICTS = [
  ['hitch ratio (ms/s)', (r) => r.smooth.hitchRatio, (r) => r.smooth.verdict.hitch, '0',
    (r, e) => causeOf(r.smooth.hitchAt, e) ?? `frames came late while the list moved${at(r)}: the main thread is busy as rows come on screen; ${ROWS}. Where each row is a few texts and an image, it is ${OWN(e)}`],
  ['longest frame (ms)', (r) => r.smooth.longestFrameMs, (r) => r.smooth.verdict.stall, '16.7',
    (r, e) => { const worst = [...r.smooth.hitchAt].sort((a, b) => b.ms - a.ms)[0]; const c = worst && causeOf([worst], e); return `a frame of ${r.smooth.longestFrameMs} ms${worst ? ` during ${worst.gesture} (main thread ${worst.mainMs} ms)` : ''}: ${c ?? `a row costly the first time it shows; ${ROWS}. Where they are light, it is ${OWN(e)}`}`; }],
  ['main thread p95 (ms/frame)', (r) => r.smooth.mainP95Ms, (r) => r.smooth.verdict.main, '1.5–2.1',
    (r, e) => `the rows' layout and drawing take most of each frame while the list moves; ${ROWS}. Where they are light, it is ${OWN(e)}`],
  ['tracking ratio', (r) => r.tracking?.ratio, (r) => r.tracking?.verdict.ratio, '1.000',
    (r, e) => `the content ${r.tracking.ratio < 1 ? 'trails' : 'outruns'} the finger: a scroll= handler that moves content as the list scrolls does this; without one it is ${OWN(e)}`],
  ['touch-to-content lag (frames)', (r) => r.tracking?.lagFrames, (r) => r.tracking?.verdict.lag, '0',
    (r, e) => `the content follows the finger ${r.tracking.lagFrames} frames late: a scroll= handler that sets state each frame, or rows heavy enough to hold the main thread, does this; without either it is ${OWN(e)}`],
  ['glide after a still lift (pt)', (r) => r.tracking?.glidePt, (r) => r.tracking?.verdict.glide, '0',
    (r, e) => `the list moved ${r.tracking.glidePt} pt after a finger lifted still: a scroll= handler that scrolls the list, or scroll snapping, does this; without either it is ${OWN(e)}`],
  ['deceleration τ (ms; .normal 500)', (r) => r.momentum.tauMs, (r) => r.momentum.verdict.tau, '502',
    (r, e) => `a fling slows down unlike UIKit's (.normal): ${OWN(e)}`],
  ['momentum hand-off (content/asked)', (r) => r.momentum.handoff, (r) => r.momentum.verdict.handoff, '0.95–1.05',
    (r, e) => `a fling's glide starts at ${r.momentum.handoff}× the finger's speed: ${OWN(e)}`],
  ['flings that drop a frame at the lift', (r) => r.momentum.lifts ? `${r.momentum.liftDrops}/${r.momentum.lifts}` : null, (r) => r.momentum.verdict.lift, '0',
    (r, e) => `the list stalls a frame as the finger lifts (main thread ${r.momentum.liftMainMs} ms then), the host's work at a fling's release: ${OWN(e)}`],
  ['offset jumps while decelerating', (r) => r.momentum.jumps, (r) => r.momentum.verdict.jumps, '0',
    (r, e) => `the gliding list stalled or jumped ${r.momentum.jumps} times off UIKit's curve: a frame the main thread had no time for (heavy rows), or a row that resizes once it shows (an image without width and height or aspect-ratio); otherwise ${OWN(e)}`],
  ['edge bounce settle (s)', (r) => r.edgeBounce.settleS, (r) => r.edgeBounce.verdict.settle, '0.63–0.85',
    (r, e) => `momentum carried past the list's end comes back in ${r.edgeBounce.settleS} s: an overscroll-behavior other than auto on the list or above it changes the bounce; without one it is ${OWN(e)}`],
  ['rubber band (overscroll/finger)', (r) => r.rubberBand?.ratio, (r) => r.rubberBand?.verdict.ratio, '0.47',
    (r, e) => r.rubberBand.bounces === false || !r.rubberBand.overscrollPt
      ? `the list does not stretch past its top as iOS lists do: overscroll-behavior none (or contain) on the list or above it turns the bounce off; drop it. Without one it is ${OWN(e)}`
      : `pulled past its top, the list stretches ${r.rubberBand.ratio} of the finger's travel: ${OWN(e)}`],
  ['rubber band settle (s)', (r) => r.rubberBand?.settleS, (r) => r.rubberBand?.verdict.settle, '0.54–0.55',
    (r, e) => `released past its top, the list takes ${r.rubberBand.settleS ?? 'too long'} s to come back: ${OWN(e)}`],
  ['status-bar scroll to top (s)', (r) => r.scrollToTop?.reached ? round((r.scrollToTop.startS ?? 0) + (r.scrollToTop.durationS ?? 0), 2) : 'no', (r) => r.scrollToTop?.verdict.reached, '0.93–0.94',
    (r, e) => r.scrollToTop.reached
      ? `a tap on the status bar takes the list to its top slower than UIKit: ${OWN(e)}`
      : `a tap on the status bar did not take the list to its top: iOS scrolls one scroller a screen, the route's content (navigationScroll names it), so name this list there when the screen has more than one; where it is named, it is ${OWN(e)}`],
  ['tap while decelerating: travel (pt)', (r) => r.tapToStop?.activated ? 'activated' : r.tapToStop?.travelPt, (r) => r.tapToStop?.verdict.stop, '0',
    (r, e) => r.tapToStop.activated
      ? `a tap that stopped the gliding list also opened what it landed on, where UIKit only stops the list: a row that acts on pointerdown or a touch's start does this (act on press=); without one it is ${OWN(e)}`
      : `the list kept moving ${r.tapToStop.travelPt} pt after a finger landed on it: ${OWN(e)}`],
  ['blank samples (share)', (r) => r.blank.share, (r) => r.blank.verdict.blank, '0',
    (r, e) => r.blank.verified
      ? `${r.blank.blankSamples} of ${r.blank.samples} frames sampled while the list moved showed a band (up to ${r.blank.maxPt} pt) blank where the same frame at rest has rows drawn: rows drawn late. A row whose content waits on something (an image loading, data fetched as it shows) does this; otherwise it is ${OWN(e)}`
      : `${r.blank.blankSamples} of ${r.blank.samples} frames sampled while the list moved showed a blank band (up to ${r.blank.maxPt} pt) where rows belong: rows drawn late. A row that is itself 60 pt or more of one plain colour, or drawn on a canvas, reads as blank too; otherwise it is ${OWN(e)}`],
  // The scrub and the carousel: shown in feel.txt only when the probe played them (the sixth entry).
  ['scrub: hitch ratio (ms/s)', (r) => r.scrub?.smooth?.hitchRatio, (r) => r.scrub?.verdict.hitch, '0',
    (r, e) => causeOf(r.scrub.smooth.hitchAt, e) ?? `frames came late while the scroll indicator was dragged down the list, a screen or more of rows a frame: ${ROWS}. Where they are light, it is ${OWN(e)}`, (r) => !!r.scrub],
  ['scrub: longest frame (ms)', (r) => r.scrub?.smooth?.longestFrameMs, (r) => r.scrub?.verdict.stall, '16.7',
    (r, e) => `a frame of ${r.scrub.smooth.longestFrameMs} ms while the indicator was dragged: ${causeOf([...r.scrub.smooth.hitchAt].sort((a, b) => b.ms - a.ms).slice(0, 1), e) ?? `${ROWS}. Where they are light, it is ${OWN(e)}`}`, (r) => !!r.scrub],
  ['scrub: blank samples (share)', (r) => r.scrub?.blank?.share, (r) => r.scrub?.verdict.blank, '0',
    (r, e) => `${r.scrub.blank.blankSamples} of ${r.scrub.blank.samples} frames sampled while the indicator was dragged showed a band (up to ${r.scrub.blank.maxPt} pt) blank where the same frame at rest has rows: rows drawn late. A row whose content waits on something does this; otherwise it is ${OWN(e)}`, (r) => !!r.scrub?.blank],
  ['scrub: blanks while held (share)', (r) => r.scrub?.heldBlank?.share, (r) => r.scrub?.verdict.held, '0',
    (r, e) => `${r.scrub.heldBlank.blankSamples} of ${r.scrub.heldBlank.samples} frames sampled 100 ms or more into a hold of the dragged indicator still showed a blank band (up to ${r.scrub.heldBlank.maxPt} pt): rows that stay unbuilt after the list stops. A row whose content waits on something does this; otherwise it is ${OWN(e)}`, (r) => !!r.scrub?.heldBlank?.samples],
  ['carousel: hitch ratio (ms/s)', (r) => r.horizontal?.smooth.hitchRatio, (r) => r.horizontal?.smooth.verdict.hitch, '0',
    (r, e) => causeOf(r.horizontal.smooth.hitchAt, e) ?? `frames came late while a horizontal list in the list moved: ${ROWS}. Where its items are light, it is ${OWN(e)}`, (r) => !!r.horizontal],
  ['carousel: longest frame (ms)', (r) => r.horizontal?.smooth.longestFrameMs, (r) => r.horizontal?.smooth.verdict.stall, '16.7',
    (r, e) => `a frame of ${r.horizontal.smooth.longestFrameMs} ms while a horizontal list moved: ${ROWS}. Where its items are light, it is ${OWN(e)}`, (r) => !!r.horizontal],
  ['carousel: tracking ratio', (r) => r.horizontal?.tracking?.ratio, (r) => r.horizontal?.tracking?.verdict.ratio, '1.000',
    (r, e) => `a horizontal list's content ${r.horizontal.tracking.ratio < 1 ? 'trails' : 'outruns'} the finger: ${OWN(e)}`, (r) => !!r.horizontal?.tracking],
  ['carousel: deceleration τ (ms; .normal 500)', (r) => r.horizontal?.momentum.tauMs, (r) => r.horizontal?.momentum.verdict.tau, '502',
    (r, e) => `a horizontal list's fling slows down unlike UIKit's (.normal): scroll snapping does this; without it it is ${OWN(e)}`, (r) => !!r.horizontal],
  ['carousel: momentum hand-off (content/asked)', (r) => r.horizontal?.momentum.handoff, (r) => r.horizontal?.momentum.verdict.handoff, '1.0',
    (r, e) => `a horizontal list's glide starts at ${r.horizontal.momentum.handoff}× the finger's speed: ${OWN(e)}`, (r) => !!r.horizontal],
  ['carousel: edge bounce settle (s)', (r) => r.horizontal?.edgeBounce.settleS, (r) => r.horizontal?.edgeBounce.verdict.settle, '0.6–0.85',
    (r, e) => `momentum carried past a horizontal list's end comes back in ${r.horizontal.edgeBounce.settleS} s: ${OWN(e)}`, (r) => !!r.horizontal],
  ['carousel: a mostly-horizontal drag (carousel/finger, list pt)', (r) => r.horizontal?.across && `${r.horizontal.across.ratio}, ${r.horizontal.across.listPt}`, (r) => r.horizontal?.verdict.across, '0.95, 0',
    (r, e) => `a drag mostly along a horizontal list moved it ${r.horizontal.across.ratio} of the finger's travel and the list it is in ${r.horizontal.across.listPt} pt, where UIKit gives it to the horizontal list alone: ${OWN(e)}`, (r) => !!r.horizontal],
  ['carousel: a mostly-vertical drag (list/finger, carousel pt)', (r) => r.horizontal?.along && `${r.horizontal.along.ratio}, ${r.horizontal.along.carouselPt}`, (r) => r.horizontal?.verdict.along, '0.95, 0',
    (r, e) => `a drag mostly down the list that began on a horizontal list moved the list ${r.horizontal.along.ratio} of the finger's travel and the horizontal list ${r.horizontal.along.carouselPt} pt, where UIKit gives it to the list alone: ${OWN(e)}`, (r) => !!r.horizontal],
];
/** The metrics `r` measured: every one of the main script's, and the scrub's and the carousel's when played. */
const shownIn = (r) => VERDICTS.filter((v) => !v[5] || v[5](r));

/** feel.txt: one line a metric. */
export function report(r) {
  if (r.verdict !== 'probed') return `${r.app}: ${r.verdict}\n`;
  const lines = [`${r.app}: ${r.list?.cls} (decelerationRate ${r.list?.decelerationRate}, bounces ${r.list?.bounces}, scrollsToTop ${r.list?.scrollsToTop}), ${r.smooth.fps} Hz, ${r.smooth.motionS} s of motion`];
  for (const [name, value, verdict] of shownIn(r)) lines.push(`  ${(verdict(r) ?? 'n/a').padEnd(5)} ${name.padEnd(38)} ${value(r) ?? '–'}`);
  const counts = shownIn(r).map(([, , v]) => v(r)).reduce((m, v) => ({ ...m, [v]: (m[v] ?? 0) + 1 }), {});
  lines.push(`  ${counts.ok ?? 0} ok, ${counts.tell ?? 0} tell, ${counts.bad ?? 0} bad`);
  if (r.smooth.where?.length) {
    const split = r.smooth.tickHitchRatio != null ? ` (hitch ms/s from timer ticks ${r.smooth.tickHitchRatio}, from scrolling ${r.smooth.scrollHitchRatio})` : '';
    lines.push(`  where the late frames' main-thread time went${split}:`, ...r.smooth.where.map((c) => `    ${whereLine(c)}`));
  }
  for (const [what, s] of [['scrub', r.scrub?.smooth], ['carousel', r.horizontal?.smooth]]) {
    if (s?.where?.length) lines.push(`  where the ${what}'s late frames' main-thread time went:`, ...s.where.map((c) => `    ${whereLine(c)}`));
  }
  return lines.join('\n') + '\n';
}

const UNITS = new Set(['ms/s', 'ms', 'ms/frame', 'frames', 'pt', 's']);
// Metrics too unsure to put to an app author as a finding: blank bands measured without the check at rest read a
// plain region (a grouped list's background, a uniform card) as blank on native lists too (round r21: SwiftUI's
// Settings 0.36, UIKit's inbox 0.22). With the check they are a finding.
const unsure = (r, metric) => metric === 'blank samples' && !r.blank?.verified;

/** The metrics that time the main thread: a machine busy with other work makes them, not the app. */
export const TIMING = new Set(['hitch ratio', 'longest frame', 'main thread p95', 'flings that drop a frame at the lift',
  'offset jumps while decelerating', 'touch-to-content lag', 'scrub: hitch ratio', 'scrub: longest frame', 'carousel: hitch ratio', 'carousel: longest frame']);
/** Whose the late frames' time was by the sampler: the label that took most, as an owner (`exact`: Exact's runtime
 * or host; `app`: the app's JS or code), or null when it saw none or the platform took most. */
function ownerOfTime(hitches) {
  const top = whereOf(hitches).by[0];
  return !top || top.ms < 2 ? null : /^Exact/.test(top.who) ? 'exact' : /^app (JS|code)/.test(top.who) ? 'app' : null;
}
/** Who can fix a finding: `exact` when only Exact can (UIKit's physics, which an app cannot change, or time the
 * sampler puts in Exact's runtime or host), `app` when the sampler puts the time in the app's JS or code or the
 * authored cause the remedy names is present, else `either`. */
function ownerOf(r, metric) {
  switch (metric) {
    case 'deceleration τ': case 'momentum hand-off': case 'flings that drop a frame at the lift': case 'rubber band settle':
      return 'exact';
    case 'rubber band':
      // Not bouncing: overscroll-behavior turns it off (the app's); bouncing by the wrong amount is UIKit's own.
      return r.rubberBand.bounces === false ? 'app' : r.rubberBand.overscrollPt ? 'exact' : 'either';
    case 'status-bar scroll to top': return r.scrollToTop.reached ? 'exact' : 'either';
    case 'tap while decelerating: travel': return r.tapToStop.activated ? 'either' : 'exact';
    case 'hitch ratio': case 'offset jumps while decelerating': case 'touch-to-content lag':
      return ownerOfTime(r.smooth.hitchAt) ?? 'either';
    case 'longest frame': {
      const worst = [...r.smooth.hitchAt].sort((a, b) => b.ms - a.ms)[0];
      return (worst && ownerOfTime([worst])) ?? 'either';
    }
    case 'scrub: hitch ratio': case 'scrub: longest frame': return ownerOfTime(r.scrub.smooth.hitchAt) ?? 'either';
    case 'carousel: hitch ratio': case 'carousel: longest frame': return ownerOfTime(r.horizontal.smooth.hitchAt) ?? 'either';
    case 'carousel: tracking ratio': case 'carousel: momentum hand-off': case 'carousel: edge bounce settle':
    case 'carousel: a mostly-horizontal drag': case 'carousel: a mostly-vertical drag':
      return 'exact';
    default: return 'either';
  }
}
/** The metrics a tell or a bad, each `{ metric, unit, verdict, value, uikit, remedy, owner }` (`metric` without its
 * unit; `unsure`: a note, not a finding; `owner` from `ownerOf`); `evidence` names the file to report Exact's own
 * with. `timing: false` leaves out the metrics that time the main thread (`TIMING`). */
export function findings(r, evidence = null, { timing = true } = {}) {
  if (r.verdict !== 'probed') return [];
  return VERDICTS.filter(([, , verdict]) => ['tell', 'bad'].includes(verdict(r))).map(([label, value, verdict, uikit, remedy]) => {
    const unit = label.match(/\(([^;)]+)/)?.[1], metric = label.replace(/ \(.*\)$/, '');
    return { metric, unit: UNITS.has(unit) ? unit : '', verdict: verdict(r), value: value(r), uikit, remedy: remedy(r, evidence), owner: ownerOf(r, metric), ...(unsure(r, metric) ? { unsure: true } : {}) };
  }).filter((f) => timing || !TIMING.has(f.metric));
}

/** The key numbers, for a list that scrolls like UIKit's: what was measured, in a few words each. */
export function summary(r) {
  if (r.verdict !== 'probed') return '';
  return [r.smooth.hitchRatio != null && `hitches ${r.smooth.hitchRatio} ms/s`, r.smooth.longestFrameMs != null && `longest frame ${r.smooth.longestFrameMs} ms`,
    r.smooth.mainP95Ms != null && `main thread p95 ${r.smooth.mainP95Ms} ms`, r.momentum.tauMs != null && `τ ${r.momentum.tauMs} ms`,
    r.momentum.lifts && `${r.momentum.liftDrops}/${r.momentum.lifts} lifts dropped a frame`,
    r.scrub?.smooth && `scrubbing ${r.scrub.smooth.hitchRatio} ms/s`, r.horizontal && `a carousel ${r.horizontal.smooth.hitchRatio} ms/s`].filter(Boolean).join(', ');
}

if (import.meta.main) {
  const dirs = process.argv.slice(2);
  if (!dirs.length) { console.error('usage: bun host/apple/feel/analyze.mjs DIR [DIR …]'); process.exit(2); }
  const results = dirs.map((d) => {
    const r = analyze(readTrace(readFileSync(resolve(d, 'trace.jsonl'), 'utf8')), JSON.parse(readFileSync(resolve(d, 'gestures.json'), 'utf8')));
    writeFileSync(resolve(d, 'feel.json'), JSON.stringify(r, null, 1));
    writeFileSync(resolve(d, 'feel.txt'), report(r));
    return r;
  });
  if (results.length === 1) process.stdout.write(report(results[0]));
  else {
    const cell = (s) => String(s ?? '–').padStart(11);
    console.log(''.padEnd(38) + dirs.map((d) => cell(d.split('/').filter(Boolean).at(-1).slice(-10))).join(''));
    for (const [name, value, verdict] of VERDICTS) {
      const mark = (r) => ({ tell: '*', bad: '!' })[verdict(r)] ?? '';
      console.log(name.padEnd(38) + results.map((r) => cell(`${value(r) ?? '–'}${mark(r)}`)).join(''));
    }
    console.log('(* tell, ! bad)');
  }
}
