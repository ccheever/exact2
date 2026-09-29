// Canvas 2D surfaces on the JS runtime (LLP 1056; LLP 1071 §7): the
// runner's half of the protocol (runner/src/runner/canvas2d.rs) over the
// web host's own replayer. Fetched two frames after the first 2D canvas
// mounts (rt.js `c2`), as the wasm host fetches its glue; an app without one
// never loads it (LLP 1047).
//
// `canvas2d-glue.js` is the wasm host's file, unchanged: it watches each
// canvas's content box with a ResizeObserver and replays stamped lists into
// the element's CanvasRenderingContext2D. It talks to "the wasm" by name
// (`exact.wasm.exact_canvas_geometry`, `exact_advance`, `exact_canvas_fonts`,
// `memory`), so this module answers those names: geometry keys a canvas's
// size generation, and every due draw is one synchronous call into the Rust
// data module (`exact_logic_draw`, logic/abi/src/draw.rs), whose lists are
// handed to the glue as `[address, length]` pairs in a buffer standing in
// for its memory.
import './canvas2d-glue.js';

// The module's draws, over what rust-data.js keeps of it (`data.logic`): a
// request (draw.rs `draw_request`) and its reply, both in the module's ABI.
function client({ exports: e, session, writer, reader, encode, ABI }) {
  if (!e.exact_logic_draw) return null;
  // A computed argument has no declared type: encoded by its shape.
  const value = (w, v, t) => t ? encode(w, v, t) : Array.isArray(v) ? (w.u8(6), w.u32(v.length), v.forEach(x => value(w, x))) : v == null ? w.u8(4) : encode(w, v, typeof v === 'number' ? 'n' : typeof v === 'boolean' ? 'b' : 's');
  const u64 = (w, v) => { w.u32(v >>> 0); w.u32(Math.floor(v / 4294967296)); };
  const call = fill => {
    const w = writer(); w.u32(ABI); w.u8(5); fill(w);
    const bytes = w.done(), p = e.exact_logic_alloc(bytes.length);
    new Uint8Array(e.memory.buffer, p, bytes.length).set(bytes);
    const rc = e.exact_logic_draw(session, p, bytes.length);
    e.exact_logic_dealloc(p, bytes.length);
    if (rc !== 0) throw new Error('the Rust module rejected the draw');
    const r = reader(new Uint8Array(e.memory.buffer, e.exact_logic_output(session), e.exact_logic_output_len(session)).slice());
    if (r.u32() !== ABI || r.u8() !== 5) throw new Error('expected a draw reply');
    const lists = []; for (let n = r.u32(); n--;) lists.push(r.bytes(r.u32()));
    return { lists, frame: r.u8() === 1, error: r.u8() ? r.str() : null };
  };
  return {
    draw: q => call(w => {
      w.u8(0); u64(w, q.canvas); w.u32(q.generation); u64(w, q.seq); w.str(q.surface);
      w.u8(6); w.u32(q.args.length); q.args.forEach((a, i) => value(w, a, q.types[i]));
      w.u32(q.names.length); for (const n of q.names) w.str(n);
      const [time, mounted, cause, width, height, pw, ph, scale] = q.frame;
      w.f64(time); w.f64(mounted); w.u8(cause); w.f64(width); w.f64(height); w.u32(pw); w.u32(ph); w.f64(scale);
      if (q.color) { w.u8(1); for (const c of q.color.slice(0, 3)) w.u8(c); w.f64(q.color[3]); } else w.u8(0);
      w.u8(q.rtl ? 1 : 0);
    }),
    retire: canvas => call(w => { w.u8(1); u64(w, canvas); }),
  };
}

const MOUNT = 1, ARGS = 2, SIZE = 4, FRAME = 8, FONT = 32;
let lifetimes = 1;

export function engine(rt) {
  const { data, clock, inflight, journal } = rt;
  const x = globalThis.exact;
  const records = new Map(); // view id -> record
  let module = null; // the module's draws, once it is ready
  const say = line => journal.push(`t=${clock.now} ${line}`);
  const memory = { buffer: new ArrayBuffer(8) };
  const colour = el => {
    const m = /rgba?\(([\d.]+),\s*([\d.]+),\s*([\d.]+)(?:,\s*([\d.]+))?/.exec(getComputedStyle(el).color);
    return m ? [+m[1], +m[2], +m[3], m[4] == null ? 1 : +m[4]] : null;
  };
  // One pass over the canvases with causes, as `draw_canvases` runs.
  function drawAll() {
    if (!module) return;
    for (const [id, r] of records) {
      if (!r.backing || !r.backing.w || !r.backing.h || !r.causes) continue;
      let causes = r.causes;
      r.causes = 0;
      if (causes & FRAME) r.framedAt = clock.now;
      const b = r.backing, el = r.c.e, seq = r.seq++;
      let reply;
      try {
        reply = module.draw({
          canvas: r.lifetime, generation: r.generation, seq, surface: r.c.name, args: r.c.args, types: r.c.types, names: r.c.names,
          frame: [clock.now, r.c.mounted, causes, b.width, b.height, b.w, b.h, b.scale], color: colour(el), rtl: getComputedStyle(el).direction === 'rtl',
        });
      } catch (e) { reply = { lists: [], frame: false, error: String(e?.message ?? e) }; }
      // The lists, 8-aligned in one buffer the glue reads in place.
      let size = 0;
      for (const l of reply.lists) size += (l.length + 7) & ~7;
      const bytes = new Uint8Array(size), at = [];
      let o = 0;
      for (const l of reply.lists) { bytes.set(l, o); at.push([o, l.length]); o += (l.length + 7) & ~7; }
      memory.buffer = bytes.buffer;
      glue.op({ id, lifetime: r.lifetime, generation: r.generation, seq, lists: at });
      r.text = reply.lists.some(hasText);
      r.frames = reply.frame && !reply.error;
      if (reply.error) say(`canvas ${id} (${r.c.name}) draw threw: ${reply.error}`);
    }
    const frames = [...records.values()].some(r => r.frames);
    if (frames !== wanting) { wanting = frames; glue.op({ frames }); }
  }
  let wanting = false;
  // What the glue calls the wasm for.
  const wasm = x.wasm ??= {};
  Object.defineProperty(wasm, 'memory', { get: () => memory, configurable: true });
  wasm.exact_canvas_geometry = (id, width, height, scale) => {
    const r = records.get(id);
    if (!r) return 0;
    const b = { w: Math.max(0, Math.round(width * scale)), h: Math.max(0, Math.round(height * scale)), width, height, scale };
    if (r.backing && r.backing.w === b.w && r.backing.h === b.h && r.backing.scale === b.scale) { r.backing = b; return 0; }
    // A new size generation: a fresh bitmap and recorder (D4).
    if (r.backing) { r.generation++; r.causes |= SIZE; }
    r.backing = b;
    glue.op({ id, lifetime: r.lifetime, generation: r.generation, seq: 0, fresh: true, w: b.w, h: b.h, scale, stretch: false, lists: [] });
    if (r.waiting) { r.waiting = false; inflight.n--; }
    drawAll();
    return 0;
  };
  // A presented frame: the clock moves to the page's time (its timers fire),
  // and canvases that asked draw once at it (D5).
  wasm.exact_advance = () => { rt.advance(rt.wall()); frame(); return 0; };
  wasm.exact_canvas_fonts = () => { for (const r of records.values()) if (r.text) r.causes |= FONT; drawAll(); return 0; };
  x.send ??= () => {};
  x.writeIn ??= () => 0;
  function frame() {
    for (const r of records.values()) if (r.frames && r.framedAt !== clock.now) r.causes |= FRAME;
    drawAll();
  }
  // Under the agent the clock is the driver's: its moves are the frames.
  if (clock.agent && x.advance) { const advance = x.advance; x.advance = to => { advance(to); frame(); }; }
  const glue = x.canvas2dGlue({ views: rt.views, now: () => clock.now });
  // The module arrives after first pixel (rust-data.js); until then causes wait.
  const ready = () => {
    module = client(data.logic);
    if (!module) say('canvas2d: the Rust module draws no Canvas 2D surface (exact_logic_abi::export_draw!)');
    drawAll();
  };
  if (data.logic) ready(); else data.ready(ready);
  return {
    args(c) {
      if (!c.e.isConnected) return;
      const id = rt.viewId(c.e);
      let r = records.get(id);
      if (r && r.c === c) r.causes |= ARGS;
      else {
        if (r) module?.retire(r.lifetime);
        r = { c, lifetime: lifetimes++, generation: 0, seq: 1, causes: MOUNT, backing: null, waiting: true };
        records.set(id, r);
        inflight.n++;
        // The glue's ResizeObserver names a canvas by its `data-view`, as the wasm host's elements carry it.
        c.e.dataset.view = id;
        glue.op({ id, watch: true });
      }
      drawAll();
    },
    gone(c) {
      for (const [id, r] of records) if (r.c === c) {
        records.delete(id);
        module?.retire(r.lifetime);
        if (r.waiting) inflight.n--;
      }
    },
  };
}

// Whether a list draws text: a font that loads redraws it (D8).
function hasText(l) {
  const v = new DataView(l.buffer, l.byteOffset, l.byteLength);
  for (let at = 8; at + 8 <= l.byteLength;) {
    const code = v.getUint32(at, true), n = v.getUint32(at + 4, true);
    if (code === 61 || code === 62) return true;
    at += 8 + n * 8;
  }
  return false;
}
