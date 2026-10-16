// Scrub sessions (LLP 1121 D13, LLP 1122 D6; stage 4, commit 6): a token or
// dial dragged on the tool previews on every running build as a live batch,
// written to no file, and is written once, on release.
//
// - Opening or updating one needs `write` scope; one is open at a time, its
//   client's.
// - Each update is the whole gesture's edit, planned (`propose` with
//   `texts`) on the files as the scrub opened on them, and previewed: the
//   dev loop compiles the texts it would write and sends the builds the
//   batch (`preview`). Updates that come while one previews wait, and only
//   the latest is previewed; the others are answered `superseded`.
// - A rebuild-class edit has no scrub: it applies on release, as a write.
// - Release writes once, as one transaction checked against the digests
//   the scrub opened with: a file moved since is a conflict, nothing is
//   written, and the builds get the committed design back.
// - A cancel, a competing write, its client gone, the design server lost,
//   or 30 s with no update end it without a write, and every build gets
//   the latest committed design (`restore`), not the gesture's first.

export const SCRUB_IDLE_MS = 30_000;

/**
 * `ask` sends the design server a request; `write` makes a write as the
 * wire's own do (announced, attributed); `preview(texts)` shows a scrub's
 * texts on the builds (resolves `{error}` when it cannot); `restore()` sends
 * them the committed design; `released()`, before a release's write, has the
 * builds keep the preview until the build of what it writes (a refused
 * write restores); `opened(e)` is told a scrub's first preview,
 * `ended(e)` each end of one that previewed.
 */
export function scrubSessions({ ask, write, preview, restore, released = () => {}, opened = () => {}, ended = () => {}, idleMs = SCRUB_IDLE_MS }) {
  // The scrub open (from its first update: one opening is open, and ends
  // as one does) and `ends`, a count of ends, by which a preview that
  // lands after one is taken back.
  let open = null, count = 0, ends = 0, releasing = false;
  // The update previewing now, and the latest waiting behind it.
  let busy = null, waiting = null;
  const refused = (status, message) => ({ status, error: { message } });

  const end = (reason) => {
    if (!open) return false;
    const was = open;
    clearTimeout(was.timer);
    open = null;
    ends++;
    if (waiting) { waiting.done({ ok: { scrub: was.id, ended: reason } }); waiting = null; }
    // One that never previewed changed no build.
    if (was.shown) { void restore(); ended({ scrub: was.id, client: was.client, reason }); }
    return true;
  };
  const idle = () => {
    clearTimeout(open.timer);
    open.timer = setTimeout(() => end('idle'), idleMs);
    open.timer.unref?.();
  };
  /** An opening scrub whose first update failed: gone, with nothing to restore. */
  const failed = (id) => {
    if (open?.id === id && !open.shown) { clearTimeout(open.timer); open = null; }
  };

  /** Plan and preview one update of scrub `id`. */
  async function show(request, id) {
    if (open?.id !== id) return refused(409, 'the scrub ended');
    const at = ends;
    const planned = await ask({ method: 'propose', ops: request.ops, files: open.files, texts: true });
    if (ends !== at || open?.id !== id) return refused(409, 'the scrub ended');
    if (!planned.ok) { failed(id); return planned; }
    if (planned.ok.class !== 'live') { failed(id); return refused(409, 'a rebuild-class edit has no scrub: it applies on release; send it as a `transaction`'); }
    // Texts that change nothing preview the committed design.
    const shown = await preview(planned.ok.texts ?? {});
    // Ended while it previewed: the committed design again, after it.
    if (ends !== at || open?.id !== id) { void restore(); return refused(409, 'the scrub ended'); }
    if (shown?.error) { failed(id); return refused(502, `the builds could not preview it: ${shown.error}`); }
    if (!open.shown) opened({ scrub: id, client: open.client });
    open.shown = true;
    open.ops = request.ops;
    return { ok: { scrub: id, class: 'live', changed: planned.ok.changed, synthetic: planned.ok.synthetic, files: planned.ok.files } };
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
      if (open && request.scrub !== undefined && request.scrub !== open.id) return Promise.resolve(refused(409, `\`${request.scrub}\` is not the scrub open now`));
      if (!open) {
        const files = request.files;
        if (typeof files !== 'object' || files === null || Array.isArray(files)) return Promise.resolve(refused(400, 'a scrub opens on the `files` digests its client read'));
        open = { id: `scrub-${++count}`, client: request.client, files, ops: request.ops, shown: false, timer: null };
      }
      // Idle counts from each update's arrival.
      idle();
      const id = open.id;
      return new Promise((done) => {
        const job = { request, id, done };
        if (busy) {
          if (waiting) waiting.done({ ok: { scrub: id, superseded: true } });
          waiting = job;
          return;
        }
        const run = async (j) => {
          busy = j;
          try { j.done(await show(j.request, j.id)); } catch (e) { failed(j.id); j.done(refused(500, e.message)); }
          busy = null;
          const after = waiting;
          waiting = null;
          if (after) void run(after);
        };
        void run(job);
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
      open = null;
      ends++;
      releasing = true;
      // The builds keep the preview until the build of what it writes.
      await released();
      let result;
      try { result = await write({ method: 'transaction', client: request.client, files, ops }); } finally { releasing = false; }
      // Refused (a file moved since it opened; `by` names who moved it):
      // nothing written, the builds back on the committed design.
      if (!result.ok) void restore();
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
