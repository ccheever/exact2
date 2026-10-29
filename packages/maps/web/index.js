// @exact/maps on the web (LLP 1133.021 D1-D3, §12): `exact-map`, raster
// tiles in Web Mercator from the source the app names in `app.json`
// (`packages["@exact/maps"].web.tiles`, an XYZ template), with the source's
// attribution, the package's pins and zoom buttons. map-demo's `index.js`,
// made a package.
//
// - Props are strings (`markers` its JSON). A camera prop moves the map
//   only when its value changes, and only that prop: a new `zoom` zooms
//   about the current centre, wherever the person panned (D1).
// - `moveend` is emitted when the camera stops, whoever moved it: a drag's
//   end, a wheel or key gesture once it rests, a button, a prop's move.
// - `change` is the selected marker's id, or `""`, when the person selects
//   or deselects; a `selected` prop selects without one.
// - The person drags, wheels, pinches, and with the map focused uses the
//   arrow keys and `+`/`-` (D2). Markers are buttons in `markers` order,
//   each labelled by its title.
// - Under the agent the tiles are plain, a flat colour with each tile's
//   edges, so a screenshot is stable and offline (§12); each marker is a
//   part named by its id (`tap map/ferry`).

const TILE = 256;
const MIN_ZOOM = 1;
const REST_MS = 250;
const STYLE = `
.exact-map { position: absolute; inset: 0; overflow: hidden; background: #dfe6ea; touch-action: none; user-select: none; cursor: grab; outline-offset: -2px; }
.exact-map.dragging { cursor: grabbing; }
.exact-map .tile { position: absolute; width: ${TILE}px; height: ${TILE}px; transform-origin: 0 0; pointer-events: none; }
.exact-map .plain { box-sizing: border-box; background: #e8ecef; border-right: 1px solid #c9d1d8; border-bottom: 1px solid #c9d1d8; }
.exact-map .marker { position: absolute; transform: translate(-50%, -100%); border: 0; padding: 0; margin: 0; background: none; cursor: pointer; }
.exact-map .marker svg { display: block; filter: drop-shadow(0 1px 2px rgba(0,0,0,.35)); }
.exact-map .marker .label { position: absolute; left: 50%; top: 100%; transform: translateX(-50%); white-space: nowrap; font: 600 12px system-ui, sans-serif; color: #15171c; text-shadow: 0 0 2px #fff, 0 0 2px #fff; pointer-events: none; }
.exact-map .zoom { position: absolute; left: 10px; bottom: 10px; display: flex; flex-direction: column; border-radius: 8px; overflow: hidden; box-shadow: 0 1px 4px rgba(0,0,0,.3); }
.exact-map .zoom button { width: 32px; height: 32px; border: 0; background: #fff; font: 600 18px system-ui, sans-serif; color: #15171c; cursor: pointer; }
.exact-map .zoom button + button { border-top: 1px solid #d5d9de; }
.exact-map .credit { position: absolute; right: 0; bottom: 0; font: 11px system-ui, sans-serif; background: rgba(255,255,255,.8); padding: 2px 6px; color: #333; }
`;
// The platform-red pin (D2's marker colour), selected darker and larger.
const pin = () => `<svg viewBox="0 0 28 38" aria-hidden="true"><path d="M14 37C14 37 27 22 27 13.5A13 13 0 0 0 1 13.5C1 22 14 37 14 37Z" stroke="#fff" stroke-width="2"/><circle cx="14" cy="13.5" r="4.5" fill="#fff"/></svg>`;
// Selection restyles the pin's own elements, so the element a press landed on stays in the marker.
const restyle = (svg, selected) => { svg.setAttribute('width', selected ? 32 : 26); svg.setAttribute('height', selected ? 42 : 34); svg.firstChild.setAttribute('fill', selected ? '#c22a1f' : '#ea4335'); };

// Web Mercator in world pixels at zoom z: the world is 256 × 2^z wide.
const project = (lat, lon, z) => {
  const s = TILE * 2 ** z, r = Math.sin(Math.max(-85.05, Math.min(85.05, lat)) * Math.PI / 180);
  return [(lon + 180) / 360 * s, (0.5 - Math.log((1 + r) / (1 - r)) / (4 * Math.PI)) * s];
};
const unproject = (x, y, z) => {
  const s = TILE * 2 ** z, n = Math.PI - 2 * Math.PI * y / s;
  return [180 / Math.PI * Math.atan(Math.sinh(n)), ((x / s * 360 + 540) % 360) - 180];
};
const round = (n, places) => Math.round(n * 10 ** places) / 10 ** places;
const markersOf = text => {
  try {
    const list = JSON.parse(text || '[]');
    return Array.isArray(list) ? list.filter(m => typeof m?.id === 'string' && Number.isFinite(m.latitude) && Number.isFinite(m.longitude)) : [];
  } catch { return []; }
};
let styled = null;

class ExactMap {
  constructor({ implementation, element, props, events, view }) {
    const doc = element.ownerDocument;
    if (styled !== doc) { const s = doc.createElement('style'); s.textContent = STYLE; doc.head.append(s); styled = doc; }
    this.events = events; this.view = view; this.implementation = implementation;
    this.config = implementation?.config ?? {};
    this.plain = !!implementation?.plain();
    this.maxZoom = Number(this.config.maxZoom) || 19;
    element.style.position ||= 'relative';
    this.map = doc.createElement('div');
    this.map.className = 'exact-map';
    this.map.tabIndex = 0;
    this.map.setAttribute('role', 'application');
    this.map.setAttribute('aria-roledescription', 'map');
    this.tiles = doc.createElement('div');
    this.layer = doc.createElement('div');
    this.zoomBox = doc.createElement('div');
    this.zoomBox.className = 'zoom';
    this.zoomBox.innerHTML = '<button type="button" aria-label="Zoom in">+</button><button type="button" aria-label="Zoom out">−</button>';
    this.credit = doc.createElement('div');
    this.credit.className = 'credit';
    this.credit.textContent = this.plain ? 'Plain tiles' : (this.config.attribution ?? '');
    this.map.append(this.tiles, this.layer, this.zoomBox, this.credit);
    element.append(this.map);
    this.element = element;
    this.size = { w: 0, h: 0 };
    this.centre = [0, 0]; this.zoom = 12; this.markers = []; this.selected = '';
    this.images = new Map();
    this.last = {};
    this.resize = new ResizeObserver(() => { const r = this.map.getBoundingClientRect(); this.size = { w: r.width, h: r.height }; this.draw(); });
    this.resize.observe(this.map);
    this.listen();
    this.setProps(props, true);
    implementation?.made(view, this);
    events.load();
  }

  setProps(props, first = false) {
    const changed = k => first || props[k] !== this.last[k];
    let moved = false;
    if (changed('latitude') || changed('longitude')) {
      const at = [Number(props.latitude), Number(props.longitude)];
      if (at.every(Number.isFinite) && (at[0] !== this.centre[0] || at[1] !== this.centre[1])) { this.centre = at; moved = true; }
    }
    if (changed('zoomLevel')) {
      const z = props.zoomLevel == null ? 12 : Number(props.zoomLevel);
      if (Number.isFinite(z) && z !== this.zoom) { this.zoom = this.clamp(z); moved = true; }
    }
    if (changed('markers')) this.markers = markersOf(props.markers);
    if (changed('selected')) this.selected = props.selected ?? '';
    this.last = { ...props };
    this.draw();
    this.implementation?.propsOf(this.view, this.camera(), this.markers, this.selected);
    if (moved && !first) this.rest();
  }

  clamp(z) { return Math.min(this.maxZoom, Math.max(MIN_ZOOM, z)); }
  camera() { return { latitude: round(this.centre[0], 6), longitude: round(this.centre[1], 6), zoom: round(this.zoom, 3) }; }

  // The camera has stopped: `moveend`, once, after a gesture rests.
  rest() {
    clearTimeout(this.resting);
    this.resting = setTimeout(() => {
      const camera = this.camera();
      this.implementation?.propsOf(this.view, camera, this.markers, this.selected);
      this.events.emit('moveend', camera);
    }, REST_MS);
  }

  zoomBy(dz, px = 0, py = 0) {
    const z = this.zoom, [x, y] = project(this.centre[0], this.centre[1], z), under = unproject(x + px, y + py, z);
    this.zoom = this.clamp(z + dz);
    const [ux, uy] = project(under[0], under[1], this.zoom);
    this.centre = unproject(ux - px, uy - py, this.zoom);
    this.draw();
    this.rest();
  }

  panBy(dx, dy) {
    const [x, y] = project(this.centre[0], this.centre[1], this.zoom);
    this.centre = unproject(x + dx, y + dy, this.zoom);
    this.draw();
  }

  select(id) {
    if (id === this.selected) return;
    this.selected = id;
    this.draw();
    this.implementation?.propsOf(this.view, this.camera(), this.markers, this.selected);
    this.events.change(id);
  }

  tileURL(z, x, y) {
    return String(this.config.tiles ?? '').replace('{z}', z).replace('{x}', x).replace('{y}', y);
  }

  draw() {
    const { w, h } = this.size;
    if (!w || !h) return;
    const z = Math.min(this.maxZoom, Math.max(0, Math.round(this.zoom))), k = 2 ** (this.zoom - z);
    const [cx, cy] = project(this.centre[0], this.centre[1], z);
    const left = cx - w / 2 / k, top = cy - h / 2 / k, n = 2 ** z, seen = new Set();
    const doc = this.map.ownerDocument;
    for (let ty = Math.floor(top / TILE); ty <= Math.floor((top + h / k) / TILE); ty++) {
      if (ty < 0 || ty >= n) continue;
      for (let tx = Math.floor(left / TILE); tx <= Math.floor((left + w / k) / TILE); tx++) {
        const key = `${z}/${((tx % n) + n) % n}/${ty}@${tx}`;
        seen.add(key);
        let tile = this.images.get(key);
        if (!tile) {
          if (this.plain) { tile = doc.createElement('div'); tile.className = 'tile plain'; }
          else { tile = doc.createElement('img'); tile.className = 'tile'; tile.alt = ''; tile.decoding = 'async'; tile.src = this.tileURL(z, ((tx % n) + n) % n, ty); }
          this.images.set(key, tile);
          this.tiles.append(tile);
        }
        tile.style.transform = `translate(${(tx * TILE - left) * k}px, ${(ty * TILE - top) * k}px) scale(${k})`;
      }
    }
    for (const [key, tile] of this.images) if (!seen.has(key)) { tile.remove(); this.images.delete(key); }
    // Each marker keeps its element while its id stays, so a part a drive aimed at is the one its input lands on.
    const was = this.parts ?? new Map();
    this.parts = new Map();
    this.layer.replaceChildren(...this.markers.map(m => {
      const [x, y] = project(m.latitude, m.longitude, z), b = was.get(m.id) ?? doc.createElement('button'), on = m.id === this.selected;
      b.type = 'button';
      b.className = 'marker';
      b.dataset.marker = m.id;
      b.setAttribute('aria-label', m.title ?? m.id);
      b.setAttribute('aria-pressed', String(on));
      b.style.left = `${(x - left) * k}px`; b.style.top = `${(y - top) * k}px`;
      if (b.dataset.drawn !== (m.title ?? '')) {
        b.dataset.drawn = m.title ?? '';
        b.innerHTML = pin();
        if (m.title) { const label = doc.createElement('span'); label.className = 'label'; label.textContent = m.title; b.append(label); }
      }
      restyle(b.firstChild, on);
      this.parts.set(m.id, b);
      return b;
    }));
  }

  /** The element of marker `id`, a part a drive taps (§12). */
  part(id) { const el = this.parts?.get(id); return el?.isConnected ? el : null; }
  partIds() { return [...(this.parts?.keys() ?? [])]; }

  listen() {
    const m = this.map;
    const [zin, zout] = this.zoomBox.querySelectorAll('button');
    zin.addEventListener('click', ev => { ev.stopPropagation(); this.zoomBy(1); });
    zout.addEventListener('click', ev => { ev.stopPropagation(); this.zoomBy(-1); });
    const pointers = new Map();
    m.addEventListener('pointerdown', ev => {
      if (ev.target.closest?.('.zoom')) return;
      const marker = ev.target.closest?.('.marker');
      if (marker) { this.pressed = marker.dataset.marker; return; }
      pointers.set(ev.pointerId, { x: ev.clientX, y: ev.clientY });
      this.dragged = false;
      try { m.setPointerCapture(ev.pointerId); } catch { /* a synthetic pointer */ }
      m.classList.add('dragging');
    });
    m.addEventListener('pointermove', ev => {
      const p = pointers.get(ev.pointerId);
      if (!p) return;
      if (pointers.size === 2) {
        const [a, b] = [...pointers.values()], before = Math.hypot(a.x - b.x, a.y - b.y);
        p.x = ev.clientX; p.y = ev.clientY;
        const [c, d] = [...pointers.values()], after = Math.hypot(c.x - d.x, c.y - d.y);
        if (before > 0 && after > 0) { this.zoom = this.clamp(this.zoom + Math.log2(after / before)); this.draw(); }
        this.dragged = true;
        return;
      }
      const dx = ev.clientX - p.x, dy = ev.clientY - p.y;
      p.x = ev.clientX; p.y = ev.clientY;
      if (dx || dy) { this.dragged = true; this.panBy(-dx, -dy); }
    });
    const up = ev => {
      if (this.pressed) {
        const marker = ev.target.closest?.('.marker');
        if (marker?.dataset.marker === this.pressed) this.select(this.pressed);
        this.pressed = null;
        return;
      }
      if (!pointers.delete(ev.pointerId)) return;
      if (!pointers.size) {
        m.classList.remove('dragging');
        if (this.dragged) this.rest();
        else if (ev.type === 'pointerup' && this.selected) this.select('');
      }
    };
    m.addEventListener('pointerup', up);
    m.addEventListener('pointercancel', up);
    m.addEventListener('wheel', ev => {
      ev.preventDefault();
      const r = m.getBoundingClientRect();
      this.zoomBy(-ev.deltaY * (ev.ctrlKey ? 0.01 : 0.002), ev.clientX - r.left - r.width / 2, ev.clientY - r.top - r.height / 2);
    }, { passive: false });
    m.addEventListener('keydown', ev => {
      if (ev.target !== m) return;
      const step = 80, keys = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] };
      if (keys[ev.key]) { ev.preventDefault(); this.panBy(...keys[ev.key]); this.rest(); }
      else if (ev.key === '+' || ev.key === '=') { ev.preventDefault(); this.zoomBy(1); }
      else if (ev.key === '-' || ev.key === '_') { ev.preventDefault(); this.zoomBy(-1); }
    });
  }

  destroy() {
    clearTimeout(this.resting);
    this.resize.disconnect();
    this.implementation?.gone(this.view);
    this.map.remove();
  }
}

export const roster = { 'exact-map': { view: ExactMap } };

/** The session's maps: their cameras, markers and selection for
 * `state.extensions`, and whether tiles are plain (under the agent unless
 * `prefer extension maps.tiles=live`). */
export function implementation({ agent, config } = {}) {
  const maps = new Map();
  let live = false;
  return {
    config: config ?? {},
    plain: () => !!agent && !live,
    preferred(values) { live = values?.['maps.tiles'] === 'live' || values?.tiles === 'live'; },
    made(view, map) { maps.set(view, { map }); },
    gone(view) { maps.delete(view); },
    propsOf(view, camera, markers, selected) {
      const m = maps.get(view);
      if (m) Object.assign(m, { camera, markers: markers.map(x => x.id), selected });
    },
    state() {
      return { maps: [...maps].map(([view, m]) => ({ view: Number(view), camera: m.camera, markers: m.markers ?? [], selected: m.selected ?? '', tiles: m.map.plain ? 'plain' : 'live' })) };
    },
  };
}
