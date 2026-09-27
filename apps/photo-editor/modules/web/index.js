// `<photo-editor>` on the web (LLP 1024 D7): the module table's names as
// exports. The host defines the custom element and hands this module the
// element; the editor renders into its shadow root with Pointer Events as the
// native canvases' gestures: two pointers pinch about their centroid and
// rotate about it, one pointer drags against rubber-band bounds and flings
// with its release velocity, the crop rectangle's corner and edge handles
// drag, a double click resets, and a trackpad pinch (a ctrl-wheel) zooms at
// the pointer. Props: photo, turns, reset; events: load, change (the edit
// state when a gesture ends or a prop moves it), message "edited".
export const abi = 1;
export const roster = { 'photo-editor': { snapshot: false } };

const MIN_SCALE = 0.5, MAX_SCALE = 8, MIN_CROP = 0.12;
const STYLE = `
:host { display: block; position: relative; overflow: hidden; background: #121318; }
.canvas { position: absolute; inset: 0; touch-action: none; cursor: grab; user-select: none; -webkit-user-select: none; }
.canvas.dragging { cursor: grabbing; }
img { position: absolute; left: 0; top: 0; transform-origin: 0 0; pointer-events: none; -webkit-user-drag: none; }
.crop { position: absolute; box-shadow: 0 0 0 100vmax rgba(0,0,0,.55); outline: 1.5px solid rgba(255,255,255,.95); outline-offset: -1.5px; pointer-events: none;
  background-image: linear-gradient(to right, transparent calc(33.333% - .5px), rgba(255,255,255,.35) calc(33.333% - .5px), rgba(255,255,255,.35) calc(33.333% + .5px), transparent calc(33.333% + .5px), transparent calc(66.667% - .5px), rgba(255,255,255,.35) calc(66.667% - .5px), rgba(255,255,255,.35) calc(66.667% + .5px), transparent calc(66.667% + .5px)),
    linear-gradient(to bottom, transparent calc(33.333% - .5px), rgba(255,255,255,.35) calc(33.333% - .5px), rgba(255,255,255,.35) calc(33.333% + .5px), transparent calc(33.333% + .5px), transparent calc(66.667% - .5px), rgba(255,255,255,.35) calc(66.667% - .5px), rgba(255,255,255,.35) calc(66.667% + .5px), transparent calc(66.667% + .5px)); }
.handle { position: absolute; width: 44px; height: 44px; margin: -22px 0 0 -22px; touch-action: none; }
.handle::after { content: ''; position: absolute; left: 50%; top: 50%; background: #fff; border-radius: 2px; }
.handle.corner::after { width: 16px; height: 16px; margin: -8px 0 0 -8px; background: none; border: 0 solid #fff; }
.handle.l.t::after { border-width: 4px 0 0 4px; margin: -2px 0 0 -2px; } .handle.r.t::after { border-width: 4px 4px 0 0; margin: -2px 0 0 -14px; }
.handle.l.b::after { border-width: 0 0 4px 4px; margin: -14px 0 0 -2px; } .handle.r.b::after { border-width: 0 4px 4px 0; margin: -14px 0 0 -14px; }
.handle.edge.t::after, .handle.edge.b::after { width: 22px; height: 4px; margin: -2px 0 0 -11px; }
.handle.edge.l::after, .handle.edge.r::after { width: 4px; height: 22px; margin: -11px 0 0 -2px; }
.handle.l.t, .handle.r.b { cursor: nwse-resize; } .handle.r.t, .handle.l.b { cursor: nesw-resize; }
.handle.edge.l, .handle.edge.r { cursor: ew-resize; } .handle.edge.t, .handle.edge.b { cursor: ns-resize; }
`;
const GRIPS = [['l', 't'], ['r', 't'], ['l', 'b'], ['r', 'b'], ['l'], ['r'], ['t'], ['b']];

// `turning` is a quarter turn in flight: an animation's share of the angle.
const fresh = (turns = 0) => ({ scale: 1, rotation: 0, turns, turning: 0, x: 0, y: 0, crop: { l: 0, t: 0, r: 1, b: 1 } });
const angle = (e) => e.rotation + e.turns * Math.PI / 2 + (e.turning ?? 0);
// What the Contract shows: readable, and regular enough to parse. The pan is
// the photo's centre off the stage's, as a fraction of the stage.
function summary(e, stage) {
  let d = ((angle(e) * 180 / Math.PI) % 360 + 360) % 360;
  d = Math.round(d * 10) / 10 % 360;
  const f = (v) => (Math.abs(v) < 0.005 ? 0 : v).toFixed(2);
  return `scale ${f(e.scale)}× · rotation ${d.toFixed(1)}° · pan x ${f(stage.w ? e.x / stage.w : 0)} y ${f(stage.h ? e.y / stage.h : 0)} · crop x ${f(e.crop.l)} y ${f(e.crop.t)} w ${f(e.crop.r - e.crop.l)} h ${f(e.crop.b - e.crop.t)}`;
}

class Editor {
  constructor(element, props, event) {
    this.event = event;
    this.edit = fresh(Number(props.turns) || 0);
    this.reset = props.reset ?? '0';
    this.pointers = new Map();
    this.root = element.shadowRoot ?? element.attachShadow({ mode: 'open' });
    this.root.innerHTML = `<style>${STYLE}</style><div class="canvas" role="img" aria-label="Photo"><img alt=""><div class="crop"></div>${GRIPS.map((g) => `<div class="handle ${g.length === 2 ? 'corner' : 'edge'} ${g.join(' ')}" data-grip="${g.join('')}"></div>`).join('')}</div>`;
    this.canvas = this.root.querySelector('.canvas');
    this.img = this.root.querySelector('img');
    this.cropBox = this.root.querySelector('.crop');
    this.handles = [...this.root.querySelectorAll('.handle')];
    this.size = { w: 0, h: 0 };
    // The stage the photo fits and the crop spans, inset so the handles at
    // the crop's edges stay whole: `size` is the stage, `margin` its inset.
    this.resize = new ResizeObserver(() => { const r = this.canvas.getBoundingClientRect(); this.margin = { x: Math.min(20, r.width / 4), y: Math.min(20, r.height / 4) }; this.size = { w: r.width - 2 * this.margin.x, h: r.height - 2 * this.margin.y }; this.apply(); });
    this.margin = { x: 0, y: 0 };
    this.resize.observe(this.canvas);
    this.listen();
    this.img.onload = () => { this.apply(); this.event(7); this.event(1, summary(this.edit, this.size)); const e = this.edit; this.reported = JSON.stringify([e.scale, e.rotation, e.turns, e.x, e.y, e.crop]); };
    this.img.onerror = () => this.event(8, `no photo at ${this.photo}`);
    this.setPhoto(props.photo);
  }

  setPhoto(photo) {
    if (!photo || photo === this.photo) return;
    this.photo = photo;
    this.img.src = new URL(photo, document.baseURI).href;
  }

  setProps(props) {
    this.setPhoto(props.photo);
    const turns = Number(props.turns) || 0, reset = props.reset ?? '0';
    if (reset !== this.reset) { this.reset = reset; this.rest(fresh(turns), 350); return; }
    if (turns !== this.edit.turns) this.rest({ ...this.edit, crop: { ...this.edit.crop }, turns }, 350);
  }

  destroy() {
    this.resize.disconnect();
    cancelAnimationFrame(this.frame);
    this.dead = true;
  }

  // A gesture that moved nothing (a click, a double click's first half) is no edit.
  report(e = this.edit) {
    const key = JSON.stringify([e.scale, e.rotation, e.turns, e.x, e.y, e.crop]);
    if (this.dead || key === this.reported) return;
    this.reported = key;
    this.event(1, summary(e, this.size)); this.event(8, 'edited');
  }

  /// Where an edit comes to rest is known when the gesture ends: reported
  /// then, and eased to on screen.
  rest(target, ms) { this.report(target); this.animate(target, ms); }

  // The photo fitted into the stage at scale 1, turned by its quarter turns:
  // a portrait turn of a landscape photo fills the stage as a portrait.
  get fit() {
    const w = this.img.naturalWidth, h = this.img.naturalHeight, q = this.edit.turns * Math.PI / 2 + (this.edit.turning ?? 0);
    if (!w || !h || !this.size.w || !this.size.h) return { w: this.size.w, h: this.size.h };
    const c = Math.abs(Math.cos(q)), s = Math.abs(Math.sin(q));
    const k = Math.min(this.size.w / (w * c + h * s), this.size.h / (w * s + h * c));
    return { w: w * k, h: h * k };
  }

  apply() {
    const e = this.edit, f = this.fit, m = this.margin, cx = m.x + this.size.w / 2 + e.x, cy = m.y + this.size.h / 2 + e.y;
    this.img.style.width = `${f.w}px`; this.img.style.height = `${f.h}px`;
    this.img.style.transform = `translate(${cx}px, ${cy}px) rotate(${angle(e)}rad) scale(${e.scale}) translate(${-f.w / 2}px, ${-f.h / 2}px)`;
    const c = e.crop, W = this.size.w, H = this.size.h;
    Object.assign(this.cropBox.style, { left: `${m.x + c.l * W}px`, top: `${m.y + c.t * H}px`, width: `${(c.r - c.l) * W}px`, height: `${(c.b - c.t) * H}px` });
    for (const h of this.handles) {
      const g = h.dataset.grip;
      const x = g.includes('l') ? c.l : g.includes('r') ? c.r : (c.l + c.r) / 2, y = g.includes('t') ? c.t : g.includes('b') ? c.b : (c.t + c.b) / 2;
      h.style.left = `${m.x + x * W}px`; h.style.top = `${m.y + y * H}px`;
    }
    this.canvas.setAttribute('aria-label', `Photo, ${summary(e, this.size)}`);
  }

  // The same edit arithmetic as the native canvases (PhotoEditor.swift).
  zoom(factor, px, py) {
    const e = this.edit, next = Math.min(Math.max(e.scale * factor, MIN_SCALE), MAX_SCALE), k = next / e.scale;
    e.x = px - (px - e.x) * k; e.y = py - (py - e.y) * k; e.scale = next;
  }
  turn(delta, px, py) {
    const e = this.edit, dx = e.x - px, dy = e.y - py, c = Math.cos(delta), s = Math.sin(delta);
    e.x = px + dx * c - dy * s; e.y = py + dx * s + dy * c; e.rotation += delta;
  }
  bounds(e = this.edit) { const f = this.fit; return { w: f.w * e.scale / 2, h: f.h * e.scale / 2 }; }
  band(x, y) {
    const b = this.bounds(), one = (v, lim) => { const over = Math.abs(v) - lim; return over <= 0 ? v : Math.sign(v) * (lim + over * 0.35); };
    return [one(x, b.w), one(y, b.h)];
  }

  settle(vx = 0, vy = 0) {
    if (this.pointers.size) return;
    const e = this.edit, t = { ...e, crop: { ...e.crop } };
    t.scale = Math.max(e.scale, 1);
    if (t.scale !== e.scale) { t.x = e.x * t.scale / e.scale; t.y = e.y * t.scale / e.scale; }
    const b = this.bounds(t), clamp = (v, lim) => Math.min(Math.max(v, -lim), lim);
    t.x = clamp(t.x + vx * 0.32, b.w); t.y = clamp(t.y + vy * 0.32, b.h);
    this.rest(t, Math.hypot(vx, vy) > 50 ? 600 : 300);
  }

  /// A new gesture lands the one in flight where it was going.
  finish() {
    cancelAnimationFrame(this.frame);
    if (this.target) { this.edit = this.target; this.target = null; this.apply(); }
  }

  animate(target, ms, done) {
    cancelAnimationFrame(this.frame);
    this.target = target;
    const from = { ...this.edit, crop: { ...this.edit.crop } }, start = performance.now();
    const mix = (a, b, t) => a + (b - a) * t;
    const step = (now) => {
      const u = Math.min(1, (now - start) / ms), t = 1 - (1 - u) ** 3;
      if (u >= 1) { this.edit = target; this.target = null; this.apply(); done?.(); return; }
      this.edit = { ...target, scale: mix(from.scale, target.scale, t), x: mix(from.x, target.x, t), y: mix(from.y, target.y, t),
        turns: from.turns, rotation: mix(from.rotation, target.rotation, t), turning: mix(from.turning ?? 0, (target.turns - from.turns) * Math.PI / 2, t),
        crop: { l: mix(from.crop.l, target.crop.l, t), t: mix(from.crop.t, target.crop.t, t), r: mix(from.crop.r, target.crop.r, t), b: mix(from.crop.b, target.crop.b, t) } };
      this.apply();
      this.frame = requestAnimationFrame(step);
    };
    this.frame = requestAnimationFrame(step);
  }

  local(ev) { const r = this.canvas.getBoundingClientRect(); return [ev.clientX - r.left - r.width / 2, ev.clientY - r.top - r.height / 2]; }

  listen() {
    const c = this.canvas;
    c.addEventListener('pointerdown', (ev) => {
      this.finish();
      const grip = ev.target.dataset?.grip;
      try { c.setPointerCapture(ev.pointerId); } catch {} // a pointer the browser no longer tracks
      if (grip && !this.pointers.size) { this.grip = { grip, id: ev.pointerId, x: ev.clientX, y: ev.clientY, crop: { ...this.edit.crop } }; return; }
      this.pointers.set(ev.pointerId, { x: ev.clientX, y: ev.clientY, samples: [[ev.timeStamp, ev.clientX, ev.clientY]] });
      this.raw = [this.edit.x, this.edit.y];
      this.pair = this.pairState();
      c.classList.add('dragging');
    });
    c.addEventListener('pointermove', (ev) => {
      if (this.grip?.id === ev.pointerId) {
        const g = this.grip, dx = (ev.clientX - g.x) / this.size.w, dy = (ev.clientY - g.y) / this.size.h, k = g.crop;
        let { l, t, r, b } = k;
        if (g.grip.includes('l')) l = Math.min(Math.max(0, k.l + dx), k.r - MIN_CROP);
        if (g.grip.includes('r')) r = Math.max(Math.min(1, k.r + dx), k.l + MIN_CROP);
        if (g.grip.includes('t')) t = Math.min(Math.max(0, k.t + dy), k.b - MIN_CROP);
        if (g.grip.includes('b')) b = Math.max(Math.min(1, k.b + dy), k.t + MIN_CROP);
        this.edit.crop = { l, t, r, b };
        this.apply();
        return;
      }
      const p = this.pointers.get(ev.pointerId);
      if (!p) return;
      const dx = ev.clientX - p.x, dy = ev.clientY - p.y;
      p.x = ev.clientX; p.y = ev.clientY;
      p.samples.push([ev.timeStamp, ev.clientX, ev.clientY]);
      while (p.samples.length > 2 && ev.timeStamp - p.samples[0][0] > 100) p.samples.shift();
      if (this.pointers.size === 1) {
        this.raw = [this.raw[0] + dx, this.raw[1] + dy];
        [this.edit.x, this.edit.y] = this.band(...this.raw);
      } else {
        // Two pointers: the centroid carries the photo, the spread zooms and
        // the turn rotates, both about the centroid.
        const next = this.pairState(), prev = this.pair;
        if (prev && next) {
          this.edit.x += next.cx - prev.cx; this.edit.y += next.cy - prev.cy;
          this.zoom(next.d / prev.d, next.cx, next.cy);
          this.turn(next.a - prev.a, next.cx, next.cy);
          this.raw = [this.edit.x, this.edit.y];
        }
        this.pair = next;
      }
      this.apply();
    });
    const up = (ev) => {
      if (this.grip?.id === ev.pointerId) { this.grip = null; this.report(); return; }
      const p = this.pointers.get(ev.pointerId);
      if (!p) return;
      const wasSingle = this.pointers.size === 1;
      this.pointers.delete(ev.pointerId);
      this.pair = this.pairState();
      this.raw = [this.edit.x, this.edit.y];
      if (this.pointers.size) return;
      c.classList.remove('dragging');
      const [t0, x0, y0] = p.samples[0], [t1, x1, y1] = p.samples[p.samples.length - 1], dt = (t1 - t0) / 1000;
      const recent = ev.timeStamp - t1 < 60 && dt > 0 && wasSingle && ev.type === 'pointerup';
      this.settle(recent ? (x1 - x0) / dt : 0, recent ? (y1 - y0) / dt : 0);
    };
    c.addEventListener('pointerup', up);
    c.addEventListener('pointercancel', up);
    c.addEventListener('dblclick', () => this.rest(fresh(this.edit.turns), 350));
    c.addEventListener('wheel', (ev) => {
      ev.preventDefault();
      this.finish();
      const [px, py] = this.local(ev);
      if (ev.ctrlKey) this.zoom(Math.exp(-ev.deltaY * 0.01), px, py);
      else [this.edit.x, this.edit.y] = this.band(this.edit.x - ev.deltaX, this.edit.y - ev.deltaY);
      this.apply();
      clearTimeout(this.wheelTimer);
      this.wheelTimer = setTimeout(() => this.settle(), 250);
    }, { passive: false });
  }

  pairState() {
    if (this.pointers.size < 2) return null;
    const [a, b] = [...this.pointers.values()], r = this.canvas.getBoundingClientRect();
    return { cx: (a.x + b.x) / 2 - r.left - r.width / 2, cy: (a.y + b.y) / 2 - r.top - r.height / 2,
      d: Math.max(1, Math.hypot(b.x - a.x, b.y - a.y)), a: Math.atan2(b.y - a.y, b.x - a.x) };
  }
}

export function create(tag, element, json, event) { return new Editor(element, JSON.parse(json), event); }
export function setProps(editor, json) { editor.setProps(JSON.parse(json)); }
export function destroy(editor) { editor.destroy(); }
