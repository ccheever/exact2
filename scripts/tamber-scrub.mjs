// Scrub sessions (LLP 1121 D13, LLP 1122 D6; stage 4, commit 6): a token or
// dial dragged on the tool previews on every running build as a live batch,
// written to no file, and is written once, on release.
//
// - Opening or updating one needs `write` scope; one is open at a time, its
//   client's.
// - Each update is the whole gesture's edit, folded on the files as the
//   scrub opened on them (`scrub-fold`: the design server's resident fold,
//   no file read and no Contract source walked) and patched into the design
//   the builds run (`patch`: the dev loop sets the changed rows and sends
//   the batch, compiling nothing). Where the loop cannot patch a frame it is
//   planned (`propose` with `texts`) and the texts compiled (`preview`), as
//   before. Updates that come while one previews wait, and only the latest
//   is previewed next; the others are answered `superseded`. Nothing else
//   paces them: a build shows the latest batch it has at its next frame.
// - A patched frame is verified once the gesture rests (VERIFY_MS with no
//   update): the fold again with the write's own checks, and the compile
//   of its texts (`verify`), must give the design it patched; where either
//   refuses it or they differ the scrub ends (`mismatch`) and the builds get
//   the committed design back.
// - A rebuild-class edit has no scrub: it applies on release, as a write.
// - Release writes once, as one transaction checked against the digests
//   the scrub opened with: a file moved since is a conflict, nothing is
//   written, and the builds get the committed design back.
// - A cancel, a competing write, its client gone, the design server lost,
//   or 30 s with no update end it without a write, and every build gets
//   the latest committed design (`restore`), not the gesture's first.

import { randomBytes } from 'node:crypto';

export const SCRUB_IDLE_MS = 30_000;
/** At most one preview starts per this many ms; the latest value waits, the
 * ones before it are superseded. None: a 16 ms pace held a frame that came
 * early for the next (+13 ms p95 on the clone, a frame in 30 superseded),
 * and the one preview at a time already keeps the latest. */
export const PACE_MS = 0;
/** A patched frame is verified once no update came for this long. */
export const VERIFY_MS = 150;
// EXACT_SCRUB_TRACE=1: every 120 patched frames, the p50/p95 of each step's
// ms, from the update's arrival: waiting for the pace or the frame before,
// the fold, the patch (to the batch sent to the builds).
const trace = process.env.EXACT_SCRUB_TRACE ? traced() : null;
function traced() {
  const steps = [[], [], []];
  const pct = (a, p) => [...a].sort((x, y) => x - y)[Math.min(a.length - 1, Math.floor(a.length * p))].toFixed(1);
  return (_request, t) => {
    if (t.length < 4) return;
    for (let i = 0; i < 3; i++) steps[i].push(t[i + 1] - t[i]);
    if (steps[0].length < 120) return;
    console.log(`scrub trace (${steps[0].length} frames, ms p50/p95): wait ${pct(steps[0], .5)}/${pct(steps[0], .95)} · fold ${pct(steps[1], .5)}/${pct(steps[1], .95)} · patch ${pct(steps[2], .5)}/${pct(steps[2], .95)}`);
    for (const s of steps) s.length = 0;
  };
}

/**
 * `ask` sends the design server a request; `write` makes a write as the
 * wire's own do (announced, attributed); `patch(values, frame, files)`,
 * when the dev loop has one, sets a frame's folded rows (`scrub-fold`'s
 * `values`, folded over `files`, the digests it read) on the design it
 * committed and sends the builds the batch (resolves `{fallback}` when it
 * cannot patch this frame, its design not built from those files among
 * the reasons, `{error}` when it cannot show it); `verify(texts)` compiles
 * the frame shown in full and resolves `{mismatch}` when that is not the
 * design it patched (or `{error}` when it does not compile);
 * `preview(texts, frame)` shows a scrub's
 * texts on the builds (resolves `{error}` when it cannot); `restore(frame)`
 * sends them the committed design; each `frame` (`{id, rev, run}`) marks the
 * batch and snapshot it makes; `released(frame)`, before a release's write, has
 * the builds keep the preview until the build of what it writes, which
 * carries `frame` (a refused write restores); `opened(e)` is told a scrub's first preview,
 * `ended(e)` each end of one that previewed.
 */
export function scrubSessions({ ask, write, patch = null, verify = null, preview, restore, released = () => {}, opened = () => {}, ended = () => {}, idleMs = SCRUB_IDLE_MS, paceMs = PACE_MS, verifyMs = VERIFY_MS }) {
  // The scrub open (from its first update: one opening is open, and ends
  // as one does) and `ends`, a count of ends, by which a preview that
  // lands after one is taken back.
  let open = null, count = 0, ends = 0, releasing = false;
  // The update previewing now, the latest waiting behind it, when the last
  // started (the pace), and the timer that starts the waiting one.
  let busy = null, waiting = null, started = 0, pacer = null;
  // Every frame a scrub sends the builds is marked `{id, rev, run}`, `rev`
  // rising over every frame of every scrub (*1.11*): a build drops one of
  // the run it last applied that is not newer, and applies them with no
  // transition.
  let frames = 0;
  // This run's name, which a build orders its frames by across scrubs
  // (*1.11*): a restarted server's revisions start again.
  const run = randomBytes(6).toString('hex');
  const mark = (id) => ({ id, rev: ++frames, run });
  const refused = (status, message) => ({ status, error: { message } });
  const REBUILD = 'a rebuild-class edit has no scrub: it applies on release; send it as a `transaction`';

  const end = (reason) => {
    if (!open) return false;
    const was = open;
    clearTimeout(was.timer);
    clearTimeout(was.verifier);
    open = null;
    ends++;
    if (waiting) { waiting.done({ ok: { scrub: was.id, ended: reason } }); waiting = null; }
    // One that never previewed changed no build.
    if (was.shown) { void restore(mark(was.id)); ended({ scrub: was.id, client: was.client, reason }); }
    return true;
  };
  const idle = () => {
    clearTimeout(open.timer);
    open.timer = setTimeout(() => end('idle'), idleMs);
    open.timer.unref?.();
  };
  /** An opening scrub whose first update failed: gone, with nothing to restore. */
  const failed = (id) => {
    if (open?.id === id && !open.shown) { clearTimeout(open.timer); clearTimeout(open.verifier); open = null; }
  };

  /** Fold and patch one update of scrub `id`, or plan and preview it. */
  async function show(request, id, arrived = null) {
    if (open?.id !== id) return refused(409, 'the scrub ended');
    const at = ends;
    const gone = () => ends !== at || open?.id !== id;
    let planned = null, shown = null;
    const t = trace && [arrived ?? performance.now(), performance.now()];
    if (patch) {
      planned = await ask({ method: 'scrub-fold', ops: request.ops, files: open.files, open: !open.folded });
      t?.push(performance.now());
      if (gone()) return refused(409, 'the scrub ended');
      if (!planned.ok) { failed(id); return planned; }
      if (planned.ok.class !== 'live') { failed(id); return refused(409, REBUILD); }
      open.folded = true;
      shown = await patch(planned.ok.values ?? [], mark(id), planned.ok.files);
      t?.push(performance.now());
      if (gone()) { void restore(mark(id)); return refused(409, 'the scrub ended'); }
      if (shown?.fallback) shown = null;
    }
    const patched = shown != null;
    if (!shown) {
      planned = await ask({ method: 'propose', ops: request.ops, files: open.files, texts: true });
      if (gone()) return refused(409, 'the scrub ended');
      if (!planned.ok) { failed(id); return planned; }
      if (planned.ok.class !== 'live') { failed(id); return refused(409, REBUILD); }
      // Texts that change nothing preview the committed design.
      shown = await preview(planned.ok.texts ?? {}, mark(id));
    }
    // Ended while it previewed: the committed design again, after it.
    if (gone()) { void restore(mark(id)); return refused(409, 'the scrub ended'); }
    if (shown?.error) { failed(id); return refused(502, `the builds could not preview it: ${shown.error}`); }
    if (!open.shown) opened({ scrub: id, client: open.client });
    open.shown = true;
    open.patched = patched;
    open.ops = request.ops;
    open.rev++;
    if (open.patched && verify) verifyLater();
    if (t) trace(request, t);
    return { ok: { scrub: id, class: 'live', changed: planned.ok.changed, synthetic: planned.ok.synthetic, files: planned.ok.files } };
  }
  /** Verify the patched frame shown once the gesture rests. */
  function verifyLater() {
    clearTimeout(open.verifier);
    const id = open.id, rev = open.rev;
    open.verifier = setTimeout(async () => {
      const now = () => open?.id === id && open.rev === rev && !busy && !waiting;
      if (!now()) return;
      // The fold again with the write's own checks, and its texts.
      const planned = await ask({ method: 'scrub-fold', ops: open.ops, files: open.files, verify: true });
      if (!now()) return;
      const checked = planned.ok ? await verify(planned.ok.texts ?? {}) : { mismatch: planned.error?.message ?? 'refused' };
      if (now() && (checked?.mismatch || checked?.error)) end('mismatch');
    }, verifyMs);
    open.verifier.unref?.();
  }
  /** Start the waiting update, at the pace. */
  function pump() {
    if (busy || !waiting) return;
    const wait = started + paceMs - Date.now();
    if (wait > 0) { pacer ??= setTimeout(() => { pacer = null; pump(); }, wait); return; }
    const j = busy = waiting;
    waiting = null;
    started = Date.now();
    (async () => {
      try { j.done(await show(j.request, j.id, j.at)); } catch (e) { failed(j.id); j.done(refused(500, e.message)); }
      busy = null;
      pump();
    })();
  }

  return {
    /** The scrub open now, if any: `{scrub, client}`. */
    current: () => (open ? { scrub: open.id, client: open.client } : null),
    /** `scrub` `{client, ops, files, scrub?}`: open one, or update the
     * client's own. */
    update(request) {
      if (!Array.isArray(request.ops)) return Promise.resolve(refused(400, 'a scrub carries its `ops`'));
      // A release being written: the next scrub opens on what it wrote.
      if (releasing) return Promise.resolve(refused(409, 'a scrub\'s release is being written; open one again once it lands'));
      if (open && open.client !== request.client) return Promise.resolve(refused(409, `a scrub is open, \`${open.client}\`'s: one at a time`));
      if (request.scrub !== undefined && request.scrub !== open?.id) return Promise.resolve(refused(409, `\`${request.scrub}\` is not the scrub open now`));
      if (!open) {
        const files = request.files;
        if (typeof files !== 'object' || files === null || Array.isArray(files)) return Promise.resolve(refused(400, 'a scrub opens on the `files` digests its client read'));
        open = { id: `scrub-${++count}`, client: request.client, files, ops: request.ops, shown: false, timer: null, verifier: null, folded: false, patched: false, rev: 0 };
      }
      // Idle counts from each update's arrival.
      idle();
      const id = open.id;
      clearTimeout(open.verifier);
      return new Promise((done) => {
        if (waiting) waiting.done({ ok: { scrub: id, superseded: true } });
        waiting = { request, id, done, at: trace ? performance.now() : null };
        pump();
      });
    },
    /** `scrub-release` `{client, scrub}`: the one write. */
    async release(request) {
      const mine = () => open && open.client === request.client && request.scrub === open.id;
      if (!mine()) return refused(409, 'no such scrub open for this client');
      while (busy || waiting) await new Promise(ok => setTimeout(ok, 5));
      // It may have ended, and another opened, while a preview finished.
      if (!mine()) return refused(409, 'the scrub ended');
      const { files, ops, id } = open;
      clearTimeout(open.timer);
      clearTimeout(open.verifier);
      open = null;
      ends++;
      releasing = true;
      // The builds keep the preview until the build of what it writes.
      await released(mark(id));
      let result;
      try { result = await write({ method: 'transaction', client: request.client, files, ops }); } finally { releasing = false; }
      // Refused (a file moved since it opened; `by` names who moved it):
      // nothing written, the builds back on the committed design.
      if (!result.ok) void restore(mark(id));
      ended({ scrub: id, client: request.client, reason: result.ok ? 'released' : 'refused' });
      return result.ok ? { ok: { ...result.ok, scrub: id } } : result;
    },
    /** `scrub-cancel` `{client, scrub}`. */
    cancel(request) {
      if (!open || open.client !== request.client || request.scrub !== open.id) return refused(409, 'no such scrub open for this client');
      end('cancelled');
      return { ok: { scrub: request.scrub, ended: 'cancelled' } };
    },
    /** A write that is not this scrub's release, or a hand edit. */
    compete: () => end('competing write'),
    /** Its client no longer present. */
    absent: present => { if (open && !present(open.client)) end('disconnected'); },
    /** The design server gone. */
    lost: () => end('server lost'),
  };
}
