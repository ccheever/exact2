// Copied from exact2's apps/map-demo/modules/web (the web half of the module the xheavy app's Apple copy
// came from), plus `interactive="false"` (SPEC 7: all interaction disabled): no listeners, no pointer events.
//
// `<native-map>` on the web (LLP 1024 D7): OpenStreetMap raster tiles (no API
// key; attribution shown, as the tile policy asks) under the pins, in the
// element's shadow root. A drag pans, the wheel zooms at the pointer, a pin
// press selects it. Props: latitude, longitude, span (degrees of latitude in
// view), pins (`id|lat|lon|title;…`), selected (an id); events: change with
// the id of a pin selected on the map. A `selected` prop selects and centres
// without an event.
export const abi = 1;
export const roster = { 'native-map': { snapshot: false } };

const TILE = 256;
const STYLE = `
:host { display: block; position: relative; overflow: hidden; background: #dfe6ea; }
.map { position: absolute; inset: 0; touch-action: none; cursor: grab; user-select: none; }
.map.dragging { cursor: grabbing; }
img.tile { position: absolute; width: ${TILE}px; height: ${TILE}px; pointer-events: none; }
.pin { position: absolute; transform: translate(-50%, -100%); border: 0; padding: 0; background: none; cursor: pointer; }
.pin svg { display: block; filter: drop-shadow(0 1px 2px rgba(0,0,0,.35)); }
.pin .label { position: absolute; left: 50%; bottom: 100%; transform: translateX(-50%); white-space: nowrap; font: 600 12px system-ui; color: #15171c; background: #fff; padding: 3px 7px; border-radius: 6px; box-shadow: 0 1px 4px rgba(0,0,0,.25); display: none; }
.pin.selected .label { display: block; }
.credit { position: absolute; right: 0; bottom: 0; font: 11px system-ui; background: rgba(255,255,255,.8); padding: 2px 6px; color: #333; }
.credit a { color: inherit; }
`;
const pinSVG = (fill) => `<svg width="28" height="38" viewBox="0 0 28 38" aria-hidden="true"><path d="M14 37C14 37 27 22 27 13.5A13 13 0 0 0 1 13.5C1 22 14 37 14 37Z" fill="${fill}" stroke="#fff" stroke-width="2"/><circle cx="14" cy="13.5" r="4.5" fill="#fff"/></svg>`;

// Web Mercator in world pixels at zoom z.
const project = (lat, lon, z) => {
  const s = TILE * 2 ** z, r = Math.sin(lat * Math.PI / 180);
  return [(lon + 180) / 360 * s, (0.5 - Math.log((1 + r) / (1 - r)) / (4 * Math.PI)) * s];
};
const unproject = (x, y, z) => {
  const s = TILE * 2 ** z, n = Math.PI - 2 * Math.PI * y / s;
  return [180 / Math.PI * Math.atan(Math.sinh(n)), x / s * 360 - 180];
};
const parsePins = (text = '') => text.split(';').map((p) => p.split('|')).filter((p) => p.length >= 3)
  .map(([id, lat, lon, title]) => ({ id, lat: Number(lat), lon: Number(lon), title: title ?? id }));

class NativeMap {
  constructor(element, props, event) {
    this.event = event;
    this.root = element.shadowRoot ?? element.attachShadow({ mode: 'open' });
    this.root.innerHTML = `<style>${STYLE}</style><div class="map" role="application" aria-label="Map"><div class="tiles"></div><div class="pins"></div></div><div class="credit">© <a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noopener">OpenStreetMap</a> contributors</div>`;
    this.map = this.root.querySelector('.map');
    this.tiles = this.root.querySelector('.tiles');
    this.pinLayer = this.root.querySelector('.pins');
    this.size = { w: 0, h: 0 };
    this.zoom = 14;
    this.centre = [37.7946, -122.404];
    this.images = new Map();
    this.resize = new ResizeObserver(() => { const r = this.map.getBoundingClientRect(); this.size = { w: r.width, h: r.height }; this.fitSpan(); this.draw(); });
    this.resize.observe(this.map);
    if (props.interactive === 'false') this.map.style.pointerEvents = 'none'; else this.listen();
    this.setProps(props, true);
    event(7);
  }

  setProps(props, first = false) {
    if (first || props.latitude !== this.props?.latitude || props.longitude !== this.props?.longitude) this.centre = [Number(props.latitude) || 0, Number(props.longitude) || 0];
    if (first || props.span !== this.props?.span) { this.span = Number(props.span) || 0.05; this.fitSpan(); }
    if (first || props.pins !== this.props?.pins) this.pins = parsePins(props.pins);
    this.props = props;
    this.select(props.selected ?? '', false);
    this.draw();
  }

  // The zoom (fractional) that shows `span` degrees of latitude.
  fitSpan() {
    if (!this.size.h || !this.span) return;
    const [, y0] = project(this.centre[0] - this.span / 2, 0, 0), [, y1] = project(this.centre[0] + this.span / 2, 0, 0);
    this.zoom = Math.min(19, Math.max(2, Math.log2(this.size.h / Math.abs(y0 - y1))));
  }

  select(id, fromMap) {
    if (id === this.selected) return;
    this.selected = id;
    const pin = this.pins.find((p) => p.id === id);
    if (pin && !fromMap) this.centre = [pin.lat, pin.lon];
    if (fromMap && pin) this.event(1, id);
    this.draw();
  }

  draw() {
    const { w, h } = this.size;
    if (!w || !h) return;
    const z = Math.round(this.zoom), k = 2 ** (this.zoom - z);
    const [cx, cy] = project(this.centre[0], this.centre[1], z);
    const left = cx - w / 2 / k, top = cy - h / 2 / k, n = 2 ** z;
    const seen = new Set();
    for (let ty = Math.floor(top / TILE); ty <= Math.floor((top + h / k) / TILE); ty++) {
      if (ty < 0 || ty >= n) continue;
      for (let tx = Math.floor(left / TILE); tx <= Math.floor((left + w / k) / TILE); tx++) {
        const key = `${z}/${((tx % n) + n) % n}/${ty}`;
        seen.add(key + '@' + tx);
        let img = this.images.get(key + '@' + tx);
        if (!img) {
          img = document.createElement('img');
          img.className = 'tile'; img.alt = ''; img.decoding = 'async';
          img.src = `https://tile.openstreetmap.org/${key}.png`;
          this.images.set(key + '@' + tx, img);
          this.tiles.append(img);
        }
        img.style.transform = `translate(${(tx * TILE - left) * k}px, ${(ty * TILE - top) * k}px) scale(${k})`;
        img.style.transformOrigin = '0 0';
      }
    }
    for (const [key, img] of this.images) if (!seen.has(key)) { img.remove(); this.images.delete(key); }
    this.pinLayer.replaceChildren(...this.pins.map((p) => {
      const [x, y] = project(p.lat, p.lon, z), b = document.createElement('button');
      b.className = 'pin' + (p.id === this.selected ? ' selected' : '');
      b.dataset.pin = p.id;
      b.setAttribute('aria-label', p.title);
      b.style.left = `${(x - left) * k}px`; b.style.top = `${(y - top) * k}px`;
      b.innerHTML = pinSVG(p.id === this.selected ? '#d93a2b' : '#2d6cdf') + `<span class="label">${p.title.replace(/[&<>"]/g, (c) => `&#${c.charCodeAt(0)};`)}</span>`;
      return b;
    }));
  }

  listen() {
    const m = this.map;
    m.addEventListener('pointerdown', (ev) => {
      const pin = ev.target.closest?.('.pin');
      if (pin) { this.pressed = pin.dataset.pin; return; }
      this.drag = { id: ev.pointerId, x: ev.clientX, y: ev.clientY };
      try { m.setPointerCapture(ev.pointerId); } catch {}
      m.classList.add('dragging');
    });
    m.addEventListener('pointermove', (ev) => {
      if (this.drag?.id !== ev.pointerId) return;
      const z = this.zoom, [x, y] = project(this.centre[0], this.centre[1], z);
      this.centre = unproject(x - (ev.clientX - this.drag.x), y - (ev.clientY - this.drag.y), z);
      this.drag.x = ev.clientX; this.drag.y = ev.clientY;
      this.draw();
    });
    const up = (ev) => {
      if (this.pressed) { const pin = ev.target.closest?.('.pin'); if (pin?.dataset.pin === this.pressed) this.select(this.pressed, true); this.pressed = null; return; }
      if (this.drag?.id === ev.pointerId) { this.drag = null; m.classList.remove('dragging'); }
    };
    m.addEventListener('pointerup', up);
    m.addEventListener('pointercancel', up);
    m.addEventListener('wheel', (ev) => {
      ev.preventDefault();
      const r = m.getBoundingClientRect(), px = ev.clientX - r.left - r.width / 2, py = ev.clientY - r.top - r.height / 2;
      const z = this.zoom, [x, y] = project(this.centre[0], this.centre[1], z), under = unproject(x + px, y + py, z);
      this.zoom = Math.min(19, Math.max(2, z - ev.deltaY * (ev.ctrlKey ? 0.01 : 0.002)));
      const [ux, uy] = project(under[0], under[1], this.zoom);
      this.centre = unproject(ux - px, uy - py, this.zoom);
      this.draw();
    }, { passive: false });
  }

  destroy() { this.resize.disconnect(); }
}

export function create(tag, element, json, event) { return new NativeMap(element, JSON.parse(json), event); }
export function setProps(map, json) { map.setProps(JSON.parse(json)); }
export function destroy(map) { map.destroy(); }
