// `check` on the web (LLP 1116.003 D8): what the presented page shows, as
// observations the driver judges (scripts/agent-check.mjs): each view's box in
// the viewport, a text's own text, ink and type, whether it fits its box or an
// ancestor's clip cuts it, a control and who sized it, what lies over its
// middle at rest, a filled box's fill and edge, and how far a view runs past
// the viewport's sides. Read from the page, never a second model; the
// background a text stands on is the driver's, from the pixels.
const CONTROL = 'button, a[href], input:not([type="hidden"]), select, textarea, [role="button"], [role="link"], [role="tab"], [role="switch"], [role="checkbox"], [role="radio"], [role="menuitem"], [data-exact-on~="press"]';
// A user agent's own control at its own size (WCAG 2.5.8's exception), unless the author sized it.
const AGENT_SIZED = 'input[type="checkbox"], input[type="radio"], input[type="range"], input[type="color"], input[type="date"], input[type="time"], input[type="datetime-local"], select, progress';
const SIZE = ['width', 'height', 'min-width', 'min-height', 'max-width', 'max-height', 'padding-top', 'padding-bottom', 'padding-left', 'padding-right', 'flex-basis'];

/** `views`: the runtime's id → element map; `inactive(el)`: under a route or tab its navigation root has not selected. */
export function observe(views, inactive = () => false) {
  const ids = new Map();
  for (const [k, v] of views) ids.set(v, k);
  const viewOf = (el) => { for (let e = el; e; e = e.parentElement) if (ids.has(e)) return ids.get(e); return null; };
  // Every colour as sRGB bytes and an alpha, however the page wrote it (a canvas converts).
  const ctx = Object.assign(document.createElement('canvas'), { width: 1, height: 1 }).getContext('2d', { willReadFrequently: true });
  const rgba = (css) => {
    if (!css || css === 'transparent' || css === 'none') return null;
    ctx.clearRect(0, 0, 1, 1); ctx.fillStyle = '#0000'; ctx.fillStyle = css; ctx.fillRect(0, 0, 1, 1);
    const d = ctx.getImageData(0, 0, 1, 1).data;
    return d[3] ? [d[0], d[1], d[2], Math.round(d[3] / 255 * 1000) / 1000] : null;
  };
  // What the author wrote on an element's classes: its platform colour role, and whether it set a size.
  const rules = new Map();
  const gather = (list) => { for (const r of list) { const m = r.selectorText && /\.([\w-]+)$/.exec(r.selectorText.trim()); if (m) { if (!rules.has(m[1])) rules.set(m[1], []); rules.get(m[1]).push(r.style); } if (r.cssRules) gather(r.cssRules); } };
  for (const sheet of document.styleSheets) { try { gather(sheet.cssRules); } catch { /* another origin's sheet */ } }
  const authored = (el, prop) => { for (const c of el.classList ?? []) for (const st of rules.get(c) ?? []) { const v = st.getPropertyValue(prop); if (v) return v; } return el.style?.getPropertyValue(prop) ?? ''; };
  const role = (el, prop) => /^var\(--exact-([\w-]+)/.exec(authored(el, prop))?.[1];
  const W = innerWidth, H = innerHeight, r2 = (x) => Math.round(x * 100) / 100;
  const modal = document.querySelector('dialog:modal');
  const shown = (el, b) => b.width > 0 && b.height > 0
    && (!el.checkVisibility || el.checkVisibility({ opacityProperty: true, visibilityProperty: true }))
    && !el.closest('[inert], [data-exiting]') && (!modal || modal.contains(el)) && !inactive(el);
  const opacity = (el) => { let o = 1; for (let e = el; e && e !== document.documentElement; e = e.parentElement) o *= Number(getComputedStyle(e).opacity); return o; };
  const fixed = (el, upTo) => { for (let e = el; e && e !== upTo; e = e.parentElement) if (/^(fixed|sticky)$/.test(getComputedStyle(e).position)) return true; return false; };
  const scrolls = (e) => /^(auto|scroll)$/.test(getComputedStyle(e).overflowY) && e.scrollHeight > e.clientHeight + 1;
  const page = document.scrollingElement;
  // Whether a scroller holding `el` could move it out from under `hit` (or, with none, into view): one `hit` does not
  // move with, with room toward the side the cover is not on (`above`: the cover is above, so scrolling back reveals).
  const reach = (el, hit, above) => {
    for (let s = el.parentElement; s; s = s.parentElement) {
      const scroller = s === page || s === document.body ? page : scrolls(s) ? s : null;
      if (!scroller || (hit && ((scroller !== page && scroller.contains(hit) && !fixed(hit, scroller)) || (scroller === page && !fixed(hit))))) continue;
      if (above ? scroller.scrollTop > 0.5 : scroller.scrollTop < scroller.scrollHeight - scroller.clientHeight - 0.5) return true;
    }
    return false;
  };
  const items = [];
  const out = new Set();
  // Outermost first, so a view past the viewport's side is named and not each of its children.
  const all = [...document.querySelectorAll('#exact-root *')];
  for (const el of all) {
    if (!ids.has(el) || el.namespaceURI !== 'http://www.w3.org/1999/xhtml') continue;
    const b = el.getBoundingClientRect();
    if (!shown(el, b)) continue;
    const cs = getComputedStyle(el), item = { id: ids.get(el), x: r2(b.x), y: r2(b.y), w: r2(b.width), h: r2(b.height) };
    // Off the screen: only a control out of reach (past its top or bottom, and nothing scrolls to it) is anything.
    if (!(b.right > 0 && b.bottom > 0 && b.left < W && b.top < H)) {
      if (b.right > 0 && b.left < W && el.matches(CONTROL) && !el.parentElement?.closest('button, a[href]') && !reach(el, null, b.bottom <= 0)) {
        items.push({ ...item, control: el.localName, sized: 'platform', cover: { kind: 'offscreen', name: b.bottom <= 0 ? 'the top of the screen' : 'the bottom of the screen' } });
      }
      continue;
    }
    const o = opacity(el);
    // Past the viewport's sides, nothing scrolling it back nor clipping it: the outermost such view only.
    let held = false;
    for (let e = el.parentElement; e && e.id !== 'exact-root'; e = e.parentElement) {
      if (out.has(e)) { held = true; break; }
      const s = getComputedStyle(e);
      if (s.overflowX !== 'visible') { held = true; break; }
    }
    const past = Math.max(b.right - W, -b.left);
    if (!held && past > 1 && !fixed(el)) { item.outX = r2(past); out.add(el); }
    const text = el.hasAttribute('data-exact-text') ? (el.textContent ?? '').replace(/\s+/g, ' ').trim() : '';
    if (text) {
      item.text = text.length > 40 ? text.slice(0, 39) + '…' : text;
      const fg = rgba(cs.webkitTextFillColor && cs.webkitTextFillColor !== cs.color ? cs.webkitTextFillColor : cs.color);
      if (fg) { fg[3] = r2(fg[3] * o); item.fg = fg; }
      item.size = parseFloat(cs.fontSize); item.weight = parseInt(cs.fontWeight, 10) || 400;
      const fgRole = role(el, 'color');
      if (fgRole) item.fgRole = fgRole;
      // A control's own label on the control's own fill (D3's ink rule): the nearest control within three levels. Ink no
      // one authored on a host-drawn control (a tab bar's item) is the platform's, as UIKit's own labels are.
      for (let e = el.parentElement, k = 0; e && k < 3; e = e.parentElement, k++) if (e.matches(CONTROL)) {
        item.of = viewOf(e);
        // A disabled control's label is an inactive component's text, which WCAG 1.4.3 does not judge.
        if (e.matches(':disabled') || e.getAttribute('aria-disabled') === 'true') item.inactive = true;
        if (rgba(getComputedStyle(e).backgroundColor)) item.label = true;
        let inked = false;
        for (let a = el; a && !inked; a = a === e ? null : a.parentElement) inked = !!authored(a, 'color');
        if (!inked && e.hasAttribute('data-native')) item.platformInk = true;
        break;
      }
      const clamp = cs.webkitLineClamp && cs.webkitLineClamp !== 'none' || cs.textOverflow === 'ellipsis' && cs.overflowX !== 'visible';
      if (clamp) item.clamp = true;
      let ink = b;
      if (cs.overflowX !== 'visible' || cs.overflowY !== 'visible') {
        const dx = el.scrollWidth - el.clientWidth, dy = el.scrollHeight - el.clientHeight;
        if (dx > 1 || dy > 1) { item.over = [Math.max(0, dx), Math.max(0, dy)]; item.clips = true; }
      } else {
        const range = document.createRange(); range.selectNodeContents(el);
        const rr = range.getBoundingClientRect();
        if (rr.width && rr.height) {
          ink = rr;
          const dx = Math.max(rr.right - b.right, b.left - rr.left), dy = Math.max(rr.bottom - b.bottom, b.top - rr.top);
          if (dx > 1 || dy > 1) item.over = [r2(Math.max(0, dx)), r2(Math.max(0, dy))];
        }
      }
      // An ancestor that clips (never one that scrolls: a person scrolls to it) cutting part of the text away.
      // One that hides it all is a collapsed disclosure, on purpose.
      for (let e = el.parentElement; e && e.id !== 'exact-root'; e = e.parentElement) {
        const s = getComputedStyle(e), hx = /^(hidden|clip)$/.test(s.overflowX), hy = /^(hidden|clip)$/.test(s.overflowY);
        if (!hx && !hy) continue;
        const a = e.getBoundingClientRect(), left = a.left + e.clientLeft, top = a.top + e.clientTop, right = left + e.clientWidth, bottom = top + e.clientHeight;
        if (ink.right <= left || ink.left >= right || ink.bottom <= top || ink.top >= bottom) break;
        const dx = hx ? Math.max(0, left - ink.left) + Math.max(0, ink.right - right) : 0, dy = hy ? Math.max(0, top - ink.top) + Math.max(0, ink.bottom - bottom) : 0;
        if (dx >= 2 || dy >= 2) { item.cut = { by: viewOf(e), dx: r2(dx), dy: r2(dy) }; break; }
      }
    }
    if (el.matches(CONTROL) && !el.parentElement?.closest('button, a[href]')) {
      item.control = el.localName === 'a' ? 'link' : el.localName === 'input' ? (el.type || 'text') : el.getAttribute('role') ?? (el.localName === 'button' || el.localName === 'select' || el.localName === 'textarea' ? el.localName : 'press');
      const own = SIZE.some((p) => authored(el, p));
      item.sized = !own && (el.hasAttribute('data-native') || el.matches(AGENT_SIZED)) ? 'platform' : 'author';
      if (el.localName === 'a' && cs.display === 'inline') item.inline = true;
      if (el.matches(':disabled') || el.getAttribute('aria-disabled') === 'true') item.disabled = true;
    }
    // What a finger at its middle reaches; covered only where no scroller could move it clear. Past the screen's top
    // or bottom where nothing scrolls it in (a column taller than the screen) is out of reach: a control's finding.
    if (item.control || text) {
      const my = b.top + b.height / 2;
      if (my >= H || my < 0) { if (item.control && !reach(el, null, my < 0)) item.cover = { kind: 'offscreen', name: my < 0 ? 'the top of the screen' : 'the bottom of the screen' }; }
      else {
        const cx = Math.min(Math.max(b.left + b.width / 2, 1), W - 1), hit = document.elementFromPoint(cx, my);
        if (hit && hit !== el && !el.contains(hit) && !hit.contains(el)) {
          const hb = hit.getBoundingClientRect();
          if (!reach(el, hit, hb.top + hb.height / 2 < my)) item.cover = { by: viewOf(hit), name: hit.localName, kind: modal && !modal.contains(hit) ? 'sheet' : 'node' };
        }
      }
      if (text && !item.cut && b.bottom - H >= 2 && b.top < H && !reach(el, null, false)) item.cut = { edge: 'bottom', dx: 0, dy: r2(b.bottom - H) };
    }
    // A filled box that is not text or a control: a card, unless it is the page itself. A symbol's fill is its
    // ink through a mask, and a picture is not a fill.
    const picture = /^(img|svg|canvas|video|iframe)$/.test(el.localName) || el.hasAttribute('data-symbol-source') || cs.backgroundImage !== 'none' || (cs.maskImage ?? cs.webkitMaskImage ?? 'none') !== 'none' || (cs.webkitMaskImage ?? 'none') !== 'none';
    const fill = !text && !item.control && !picture ? rgba(cs.backgroundColor) : null;
    if (fill && b.width >= 16 && b.height >= 16) {
      const radius = parseFloat(cs.borderTopLeftRadius) || 0, inset = b.left >= 2 && b.right <= W - 2;
      if (radius > 0 || inset) {
        fill[3] = r2(fill[3] * o);
        item.fill = fill; item.radius = radius;
        const fillRole = role(el, 'background-color');
        if (fillRole) item.fillRole = fillRole;
        const border = ['Top', 'Right', 'Bottom', 'Left'].some((side) => parseFloat(cs[`border${side}Width`]) > 0 && rgba(cs[`border${side}Color`]));
        if (border || cs.boxShadow !== 'none' || (cs.outlineStyle !== 'none' && parseFloat(cs.outlineWidth) > 0)) item.edge = true;
      }
    }
    if (item.text || item.control || item.fill || item.outX) items.push(item);
  }
  // LLP 1116.004 D1.3: a phone's layout at a desktop width, as advice (never a finding): a root tab bar stretched along
  // the window's bottom, and a stack the projection did not split whose root route's list pushed the detail shown. A
  // list at the root may push or not (a settings list's steppers): nothing says, so nothing is advised.
  const advice = [];
  if (W >= 900) for (const nav of document.querySelectorAll('#exact-root [navigationBack]')) {
    const bar = [...nav.querySelectorAll('[role="tablist"]')].find((l) => l.parentElement?.closest('[navigationKey]') === nav && l.querySelector(':scope > [role="tab"][aria-controls]'));
    const bb = bar?.getBoundingClientRect();
    if (bb && !nav.hasAttribute('data-exact-sidebar') && bb.width >= W * 0.75 && bb.bottom >= H - 2 && bb.height * 4 < bb.width) advice.push({ kind: 'tabbar', id: viewOf(bar), w: r2(bb.width) });
    const key = nav.getAttribute('navigationKey'), top = [...nav.querySelectorAll('[navigationKey]')].find((r) => r !== nav && r.getAttribute('navigationKey') === key && r.parentElement.closest('[navigationKey]') === nav);
    const first = top && [...top.parentElement.children].find((r) => r.hasAttribute('navigationKey'));
    if (!first || first === top || first.hasAttribute('data-exact-split') || first.getAttribute('navigationSplit') === 'none' || /^(modal|fullscreen)$/.test(top.getAttribute('navigationPresentation') ?? '')) continue;
    if (first.querySelector('[role="list"] :is(button, a[href])')) advice.push({ kind: 'push', id: viewOf(top) });
  }
  // The scheme the page draws in: a light-dark() probe in the root, whatever the system says.
  const root = document.getElementById('exact-root'), probe = document.createElement('span');
  probe.style.color = 'light-dark(rgb(0, 0, 1), rgb(0, 0, 2))'; root.append(probe);
  const used = getComputedStyle(probe).color.includes('2)') ? 'dark' : 'light'; probe.remove();
  const coarse = matchMedia('(pointer: coarse)').matches, overlap = Math.max(0, H - (globalThis.visualViewport?.height ?? H));
  return { viewport: { w: W, h: H, scrollW: page.scrollWidth }, scheme: used, target: coarse ? 44 : 24, ...(overlap ? { keyboard: r2(H - overlap) } : {}), items, ...(advice.length ? { advice } : {}) };
}
