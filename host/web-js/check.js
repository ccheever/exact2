// `check` on the web (LLP 1116.003 D8): what the presented page shows, as
// observations the driver judges (scripts/agent-check.mjs): each view's box in
// the viewport, a text's own text, ink and type, whether it fits its box or an
// ancestor's clip cuts it, a control and who sized it, what lies over its
// middle at rest, a filled box's fill and edge, and how far a view runs past
// the viewport's sides. Read from the page, never a second model; the
// background a text stands on is the nearest fill painted under it (LLP
// 1116.006 D5), else the driver's, from the pixels.
import { inertAncestor } from './navigation.js';
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
  // What the author wrote on an element's classes: its platform colour role, and whether it set a size. The last class
  // first: a lowered node's arm class (lower.js) stands over its static class, as its rule does.
  const rules = new Map();
  const gather = (list) => { for (const r of list) { const m = r.selectorText && /\.([\w-]+)$/.exec(r.selectorText.trim()); if (m) { if (!rules.has(m[1])) rules.set(m[1], []); rules.get(m[1]).push(r.style); } if (r.cssRules) gather(r.cssRules); } };
  for (const sheet of document.styleSheets) { try { gather(sheet.cssRules); } catch { /* another origin's sheet */ } }
  const authored = (el, prop) => { for (const c of [...(el.classList ?? [])].reverse()) for (const st of rules.get(c) ?? []) { const v = st.getPropertyValue(prop); if (v) return v; } return el.style?.getPropertyValue(prop) ?? ''; };
  const role = (el, prop) => /^var\(--exact-([\w-]+)/.exec(authored(el, prop))?.[1];
  const W = innerWidth, H = innerHeight, r2 = (x) => Math.round(x * 100) / 100;
  const modal = document.querySelector('dialog:modal');
  const shown = (el, b) => b.width > 0 && b.height > 0
    && (!el.checkVisibility || el.checkVisibility({ opacityProperty: true, visibilityProperty: true }))
    && !inertAncestor(el) && !el.closest('[data-exiting]') && (!modal || modal.contains(el)) && !inactive(el);
  const opacity = (el) => { let o = 1; for (let e = el; e && e !== document.documentElement; e = e.parentElement) o *= Number(getComputedStyle(e).opacity); return o; };
  // What paints that a computed colour does not say: a picture, a native widget, a gradient.
  const opaque = (e, s) => /^(img|svg|canvas|video|iframe|picture|object|embed)$/.test(e.localName) || s.backgroundImage !== 'none'
    || (/^(button|input|select|textarea|progress|meter)$/.test(e.localName) && (s.appearance ?? s.webkitAppearance ?? 'none') !== 'none');
  const paints = (e) => { const s = getComputedStyle(e); return opaque(e, s) || (s.backgroundClip !== 'text' && !!rgba(s.backgroundColor)); };
  // LLP 1116.006 D5: what a text stands on, at its ink's middle: the nearest painted fill — its own box's, a bubble's, a
  // card's, the page's — composited over what lies below where it is translucent, and whose it is (the page's: null).
  // Unknown where something the styles do not say paints there (a picture, a native widget, a box laid between that is not
  // its ancestor): the capture's pixels answer then.
  let canvasColor;
  const backing = (el, at) => {
    const cx = at.left + at.width / 2, cy = at.top + at.height / 2, chain = [];
    for (let e = el; e; e = e.parentElement) chain.push(e);
    const alpha = []; // each box's opacity times its ancestors'
    for (let i = chain.length - 1, o = 1; i >= 0; i--) alpha[i] = o *= Number(getComputedStyle(chain[i]).opacity);
    const layers = [];
    let by, accent = false, top = null;
    for (let i = 0; i < chain.length; i++) {
      const e = chain[i], s = getComputedStyle(e), canvas = e === document.documentElement || e === document.body;
      if (!canvas) { const r = e.getBoundingClientRect(); if (cx < r.left || cx > r.right || cy < r.top || cy > r.bottom) continue; }
      if (opaque(e, s)) return null;
      const c = s.backgroundClip === 'text' ? null : rgba(s.backgroundColor);
      if (!c) continue;
      c[3] *= alpha[i];
      if (!layers.length) {
        by = canvas ? null : viewOf(e);
        // The accent (`AccentColor`, which Contract writes as the accent's variable over it).
        accent = !canvas && /^var\(--exact-accent\b|^AccentColor$/.test(authored(e, 'background-color'));
      }
      layers.push(c);
      if (c[3] >= 0.999) { top = e; break; }
    }
    // Below every box, the canvas: the root's used `Canvas` colour where neither the root nor the body paints one.
    if (!top) {
      if (canvasColor === undefined) {
        const p = document.createElement('span'); p.style.backgroundColor = 'Canvas'; document.documentElement.append(p);
        canvasColor = rgba(getComputedStyle(p).backgroundColor); p.remove();
      }
      if (!canvasColor || canvasColor[3] < 0.999) return null;
      if (!layers.length) by = null;
      layers.push(canvasColor); top = document.documentElement;
    }
    // A box painted between the text and that fill, not one of its ancestors: an image under a title, a stacked sibling.
    const stack = document.elementsFromPoint(Math.min(Math.max(cx, 0), W - 1), Math.min(Math.max(cy, 0), H - 1)), from = stack.indexOf(el);
    if (from >= 0) for (const e of stack.slice(from + 1)) {
      if (e === top) break;
      if (!e.contains(el) && !el.contains(e) && paints(e)) return null;
    }
    // `solid`: the nearest fill under it is a view's own and opaque (it hides what lies beneath that view).
    const solid = by != null && layers[0][3] >= 0.999;
    let bg = layers.pop().slice(0, 3);
    while (layers.length) bg = mix(layers.pop(), bg);
    return { bg: bg.map((v) => Math.round(v)), by, accent, solid };
  };
  const mix = (fg, bg) => [0, 1, 2].map((i) => fg[i] * fg[3] + bg[i] * (1 - fg[3]));
  // The part of box `b` (el's) that is presented: each ancestor that clips or scrolls its overflow cuts it at its padding
  // box, up to a fixed box, which escapes them (LLP 1116.006 D5: a row its list cuts overlaps nothing past the list).
  const visible = (el, b) => {
    let l = b.left, t = b.top, r = b.right, btm = b.bottom;
    for (let e = el; e && e !== document.documentElement; e = e.parentElement) {
      const s = getComputedStyle(e);
      if (e !== el) {
        const a = e.getBoundingClientRect(), x0 = a.left + e.clientLeft, y0 = a.top + e.clientTop;
        if (s.overflowX !== 'visible') { l = Math.max(l, x0); r = Math.min(r, x0 + e.clientWidth); }
        if (s.overflowY !== 'visible') { t = Math.max(t, y0); btm = Math.min(btm, y0 + e.clientHeight); }
      }
      if (s.position === 'fixed') break;
    }
    return { x: r2(l), y: r2(t), w: r2(Math.max(0, r - l)), h: r2(Math.max(0, btm - t)) };
  };
  // Whether a box holds what a person reads or uses: a control, or a text whose words show. A bare text is
  // `display: contents`, with no box of its own: its words' box is its.
  const holds = (el) => [...el.querySelectorAll(`[data-exact-text], ${CONTROL}`)].some((e) => {
    if (getComputedStyle(e).display !== 'contents') return shown(e, e.getBoundingClientRect());
    const words = document.createRange(); words.selectNodeContents(e);
    return getComputedStyle(e).visibility === 'visible' && shown(e.parentElement, words.getBoundingClientRect());
  });
  const fixed = (el, upTo) => { for (let e = el; e && e !== upTo; e = e.parentElement) if (/^(fixed|sticky)$/.test(getComputedStyle(e).position)) return true; return false; };
  const scrolls = (e) => /^(auto|scroll)$/.test(getComputedStyle(e).overflowY) && e.scrollHeight > e.clientHeight + 1;
  // What keeps a box's sideways overflow its own: a box that clips on x, or one that scrolls sideways on purpose (an authored
  // `overflow-x`, or the shorthand's first value). A vertical scroller's x is `auto` only because CSS computes `visible`
  // beside `auto` so (CSS Overflow 3 §3.1), and a native host neither scrolls nor clips there as a person would scroll to it:
  // UIKit's vertical scroll view cuts it at the screen's edge (LLP 1116.020 D4; round 18's focus-timer and trivia-quiz).
  const holdsX = (e, s = getComputedStyle(e)) => /^(hidden|clip)$/.test(s.overflowX)
    || (/^(auto|scroll)$/.test(s.overflowX) && (/^(auto|scroll)$/.test(authored(e, 'overflow-x').trim()) || /^(auto|scroll)(\s|$)/.test(authored(e, 'overflow').trim())));
  // A box whose written percentage width fits its parent, and whose own padding and border (content-box sizing) push it past
  // the parent's content edge (LLP 1116.009 D5; focus-timer and recipe-finder, round 11: `width="100%"` with padding clipped
  // the phone's right side while `check` said nothing, the scroller around it holding the overflow). The cause is judged, not
  // where it lands: only a written percentage, only content-box, only a parent that takes the percentage of its own content
  // (a block, or a column: a row's items shrink to fit, a grid's resolve against a cell) and does not scroll sideways on purpose
  // (an authored `overflow-x`). A wider percentage, a fixed width, a bar's or sheet's own chrome (border-box) are none of it.
  const widerThanParent = (el, cs) => {
    const parent = el.parentElement;
    if (!parent || parent.id === 'exact-root' || cs.boxSizing !== 'content-box' || /^(absolute|fixed)$/.test(cs.position) || cs.transform !== 'none') return null;
    const written = authored(el, 'width').trim();
    if (!/^\d+(\.\d+)?%$/.test(written)) return null;
    const ps = getComputedStyle(parent);
    if (!(/^(block|flow-root|list-item)$/.test(ps.display) || (/^(inline-)?flex$/.test(ps.display) && ps.flexDirection.startsWith('column')))) return null;
    if (/^(auto|scroll)$/.test(authored(parent, 'overflow-x').trim()) || /^(auto|scroll)(\s|$)/.test(authored(parent, 'overflow').trim())) return null;
    const content = parseFloat(cs.width), pad = ['Left', 'Right'].reduce((n, side) => n + parseFloat(cs[`padding${side}`]) + parseFloat(cs[`border${side}Width`]), 0);
    const room = parent.clientWidth - parseFloat(ps.paddingLeft) - parseFloat(ps.paddingRight), past = content + pad - room;
    return pad > 0.5 && past > 1.5 && content <= room + 1.5 ? { by: viewOf(parent), dx: r2(past), pad: r2(pad), width: written } : null;
  };
  const page = document.scrollingElement;
  // Whether a scroller holding `el` could move it out from under `hit` (or, with none, into view): one `hit` does not
  // move with. The cover is the subtree beside `el` that `hit` is in (the bar, the header, the sheet), and the middle `my`
  // clears it by landing above it (the content moves up: room toward the end of at least `my - top`) or below it (the
  // content moves down: room back of at least `bottom - my`), within what the scroller shows of the screen: a bar along
  // its bottom edge is cleared only upward, a header along its top only downward, and a last row that scrolls only a few
  // points does not clear a bar taller than that (LLP 1116.012 D3; contacts, round 14: a fold row under an in-flow tab bar,
  // with the list's whole range to scroll it up by). With no `hit` (`my` unused), `above` says the element lies above
  // the screen: scrolling back reveals.
  const reach = (el, hit, above, my) => {
    let cover = null;
    if (hit) { let c = hit; while (c.parentElement && !c.parentElement.contains(el)) c = c.parentElement; cover = c.getBoundingClientRect(); }
    for (let s = el.parentElement; s; s = s.parentElement) {
      const scroller = s === page || s === document.body ? page : scrolls(s) ? s : null;
      if (!scroller || (hit && ((scroller !== page && scroller.contains(hit) && !fixed(hit, scroller)) || (scroller === page && !fixed(hit))))) continue;
      const back = scroller.scrollTop, end = scroller.scrollHeight - scroller.clientHeight - scroller.scrollTop;
      if (!cover) { if (above ? back > 0.5 : end > 0.5) return true; continue; }
      const shown = scroller === page ? { top: 0, bottom: H } : scroller.getBoundingClientRect(), top = Math.max(0, shown.top), bottom = Math.min(H, shown.bottom);
      if ((cover.top > top + 1 && end > my - cover.top + 0.5) || (cover.bottom < bottom - 1 && back > cover.bottom - my + 0.5)) return true;
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
    // A padded percentage width wider than its parent is a finding wherever the box lies, below the fold too.
    const padded = widerThanParent(el, cs);
    if (padded) item.padded = padded;
    // Off the screen: only a control out of reach (past its top or bottom, and nothing scrolls to it) is anything.
    if (!(b.right > 0 && b.bottom > 0 && b.left < W && b.top < H)) {
      if (b.right > 0 && b.left < W && el.matches(CONTROL) && !el.parentElement?.closest('button, a[href]') && !reach(el, null, b.bottom <= 0)) {
        items.push({ ...item, control: el.localName, sized: 'platform', cover: { kind: 'offscreen', name: b.bottom <= 0 ? 'the top of the screen' : 'the bottom of the screen' } });
      } else if (padded) items.push(item);
      continue;
    }
    const o = opacity(el);
    // Past the viewport's sides, nothing scrolling it back sideways nor clipping it: the outermost such view only. Where it
    // stands with a vertical scroller back at its sideways start, as a native host never moves it (a reveal moves it in Chrome).
    let held = false, sx = 0;
    for (let e = el.parentElement; e && e.id !== 'exact-root'; e = e.parentElement) {
      if (out.has(e) || holdsX(e)) { held = true; break; }
      sx += e.scrollLeft;
    }
    const past = Math.max(b.right + sx - W, -(b.left + sx));
    if (!held && past > 1 && !fixed(el)) { item.outX = r2(past); out.add(el); }
    // A flex or grid item past its container's sides, within the viewport (LLP 1116.007 D3: tip-splitter's fifth radio at 320 px,
    // an option wider than its row, poking out of its card): the container neither scrolling nor clipping it, nor any ancestor
    // it lies beyond, nor transformed or placed out of flow. Past the viewport it is the finding above.
    const parent = el.parentElement, ps = parent && parent.id !== 'exact-root' ? getComputedStyle(parent) : null;
    if (!item.outX && ps && /^(inline-)?(flex|grid)$/.test(ps.display) && !holdsX(parent, ps) && !/^(absolute|fixed)$/.test(cs.position) && cs.transform === 'none') {
      const pb = parent.getBoundingClientRect(), left = pb.left + parent.clientLeft, spill = Math.max(b.right - (left + parent.clientWidth), left - b.left);
      let beyond = false;
      for (let e = parent.parentElement; e && e.id !== 'exact-root' && !beyond; e = e.parentElement) {
        if (out.has(e)) beyond = true;
        else if (holdsX(e)) { const a = e.getBoundingClientRect(), l = a.left + e.clientLeft; beyond = b.right > l + e.clientWidth + 0.5 || b.left < l - 0.5; }
      }
      if (spill > 1.5 && !beyond && !fixed(el)) { item.spill = { by: viewOf(parent), dx: r2(spill) }; out.add(el); }
    }
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
      const ellipsis = cs.textOverflow === 'ellipsis' && cs.overflowX !== 'visible';
      if (ellipsis || cs.webkitLineClamp && cs.webkitLineClamp !== 'none') item.clamp = true;
      if (ellipsis) item.ellipsis = true;
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
          // Where its lines are drawn, where that is not its box: the driver judges two texts running into each other on it.
          if (Math.abs(rr.left - b.left) > 0.5 || Math.abs(rr.right - b.right) > 0.5 || Math.abs(rr.top - b.top) > 0.5 || Math.abs(rr.bottom - b.bottom) > 0.5) {
            item.ink = { x: r2(rr.left), y: r2(rr.top), w: r2(rr.width), h: r2(rr.height) };
          }
        }
      }
      const under = backing(el, ink);
      if (under) { item.bg = under.bg; item.on = under.by === item.id ? 'own' : under.by ?? 'page'; if (under.accent) item.onAccentFill = true; if (under.solid) item.solid = true; }
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
          // What is drawn over its middle at rest (the subtree beside it the hit is in), scrolled clear or not.
          let c = hit; while (c.parentElement && !c.parentElement.contains(el)) c = c.parentElement;
          const cb = c.getBoundingClientRect();
          item.beneath = { by: viewOf(c), x: r2(cb.left), y: r2(cb.top), w: r2(cb.width), h: r2(cb.height) };
          if (!reach(el, hit, false, my)) item.cover = { by: viewOf(hit), name: hit.localName, kind: modal && !modal.contains(hit) ? 'sheet' : 'node' };
        }
      }
      if (text && !item.cut && b.bottom - H >= 2 && b.top < H && !reach(el, null, false)) item.cut = { edge: 'bottom', dx: 0, dy: r2(b.bottom - H) };
    }
    // A card: a filled box, not itself text or a control, that holds a text or a control, its fill what sets that group
    // apart from the page. None is a box with square corners that reaches a side of the viewport (the page itself, or a
    // band across it); a list, whose ground is the page under its rows (a grouped list's grey, its sections the cards);
    // or a shape that holds no text or control (an illustration's, a map's), whether it shows or not
    // (AgentCheckIOS.swift's is iOS's). A symbol's fill is its ink through a mask, and a picture is not a fill.
    // `faded`: an ancestor's opacity composites it with what lies behind that ancestor, which the capture cannot separate.
    const picture = /^(img|svg|canvas|video|iframe)$/.test(el.localName) || el.hasAttribute('data-symbol-source') || cs.backgroundImage !== 'none' || (cs.maskImage ?? cs.webkitMaskImage ?? 'none') !== 'none' || (cs.webkitMaskImage ?? 'none') !== 'none';
    const fill = !text && !item.control && !picture && !el.hasAttribute('data-exact-list') ? rgba(cs.backgroundColor) : null;
    if (fill && b.width >= 16 && b.height >= 16) {
      const radius = parseFloat(cs.borderTopLeftRadius) || 0, inset = b.left >= 2 && b.right <= W - 2;
      if ((radius > 0 || inset) && holds(el)) {
        fill[3] = r2(fill[3] * o);
        item.fill = fill; item.radius = radius;
        if (o < Number(cs.opacity) * 0.999) item.faded = true;
        const fillRole = role(el, 'background-color');
        if (fillRole) item.fillRole = fillRole;
        const border = ['Top', 'Right', 'Bottom', 'Left'].some((side) => parseFloat(cs[`border${side}Width`]) > 0 && rgba(cs[`border${side}Color`]));
        if (border || cs.boxShadow !== 'none' || (cs.outlineStyle !== 'none' && parseFloat(cs.outlineWidth) > 0)) item.edge = true;
      }
    }
    if (item.text || item.control || item.fill) {
      const v = visible(el, b);
      if (Math.abs(v.x - item.x) > 0.5 || Math.abs(v.y - item.y) > 0.5 || Math.abs(v.w - item.w) > 0.5 || Math.abs(v.h - item.h) > 0.5) item.vis = v;
    }
    if (item.text || item.control || item.fill || item.outX || item.spill || item.padded) items.push(item);
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
  const used = getComputedStyle(probe).color.includes('2)') ? 'dark' : 'light';
  // What the page draws each of Exact's colour roles in, light and dark (host/web/look.mjs names them): the driver knows an ink
  // or a ground by it.
  const names = getComputedStyle(document.documentElement).getPropertyValue('--exact-apple-roles').trim(), palette = {};
  if (names) for (const scheme of ['light', 'dark']) {
    probe.style.colorScheme = scheme; palette[scheme] = {};
    for (const name of JSON.parse(names).split(' ')) { probe.style.color = `var(--exact-${name})`; palette[scheme][name] = rgba(getComputedStyle(probe).color); }
  }
  probe.remove();
  const coarse = matchMedia('(any-pointer: coarse)').matches, overlap = Math.max(0, H - (globalThis.visualViewport?.height ?? H));
  return { viewport: { w: W, h: H, scrollW: page.scrollWidth }, scheme: used, ...(names ? { palette } : {}), target: coarse ? 44 : 24, ...(overlap ? { keyboard: r2(H - overlap) } : {}), items, ...(advice.length ? { advice } : {}) };
}
