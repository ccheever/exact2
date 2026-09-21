import { createTimerScheduler } from "./timer-glue.js";
// @ref LLP 1043.000 §3 D6–D8 — optional browser executor of CSS Exclusions.
// Loaded by the existing script-module loader only after a textflow batch.
// The Rust walker owns segmentation, shape resolution and stopping bounds.
// DOM originals are retained offscreen (detached, never a second accessible copy).
// Fragments carry the original slices in logical DOM order. The browser owns
// bidi within each fragment, links, find-in-page, selection and accessibility.
// This is the exclusions executor, not a replacement for ordinary CSS text.
// Per-fragment bidi/shaping is not paragraph-wide browser parity; detached
// source alone cannot supply browser find or accessibility for unmounted lines.
// @ref LLP 1044.001 §7.4 — giant-paragraph admission requires that parity first.

// Per-frame glue: O(P + E*ancestor-depth + R + F log R + changed source units),
// plus wasm's documented bound; preparations are O(source + ranges*log runs).
// Pool size never exceeds a paragraph's maximum returned fragment count.
const MAX_CACHE = 8192, MAX_CACHE_UNITS = 1024 * 1024;
const encoder = new TextEncoder();
const px = value => Number.parseFloat(value) || 0;
const lower = (items, value) => {
  let lo = 0, hi = items.length;
  while (lo < hi) { const mid = (lo + hi) >>> 1; if (items[mid] < value) lo = mid + 1; else hi = mid; }
  return lo;
};

// Pure diff; pool slots keep their DOM identity across all frame geometries.
// Paint-only revision invalidates content explicitly, never on a shape move.
export function fragmentDiff(previous, next, repaint = false) {
  const updates = [];
  for (let i = 0; i < next.length; i++) {
    const a = previous[i], b = next[i];
    const content = repaint || !a || a.start !== b.start || a.end !== b.end || a.paint_start !== b.paint_start || a.paint_end !== b.paint_end || a.hyphenated !== b.hyphenated || a.ellipsis !== b.ellipsis;
    const position = !a || a.x !== b.x || a.y !== b.y;
    if (content || position) updates.push({ index: i, content, position });
  }
  return { updates, remove: Math.max(0, previous.length - next.length) };
}

// All geometry/computed-font reads finish before any fragment/probe write.
// This is also the native-free unit-test seam: a write cannot leak into read().
export function framePhases(items, read, compute, write) {
  const snapshots = items.map(read);
  const results = snapshots.map(compute);
  results.forEach(write);
  return results;
}

export function fontStyle(style, mac = false, lang = '') {
  // Pretext's platform ledger: Canvas and DOM resolve different SF optical
  // faces on macOS. Pin both measurement AND painted runs to a named face.
  const family = mac ? style.fontFamily.replace(/(^|,\s*)(?:system-ui|-apple-system|ui-sans-serif)(?=\s*(?:,|$))/g, '$1"Helvetica Neue"') : style.fontFamily;
  const font = `${style.fontStyle || 'normal'} ${style.fontWeight || '400'} ${style.fontSize} ${family}`;
  const extra = { fontKerning: style.fontKerning || 'auto', fontStretch: style.fontStretch || 'normal',
    fontVariantCaps: style.fontVariantCaps || 'normal', textRendering: style.textRendering || 'auto',
    direction: style.direction || 'ltr', lang };
  const spacing = px(style.letterSpacing);
  return { font, family, spacing, extra, key: JSON.stringify([font, spacing, extra]), size: px(style.fontSize),
    lineHeight: px(style.lineHeight),
    fontFeatureSettings: style.fontFeatureSettings, fontVariationSettings: style.fontVariationSettings };
}

// Work/storage: cache <=8192 (font,string) pairs / 1M UTF-16 units, LRU eviction.
// A single paragraph's Prepared stays in wasm after eviction of measurement data.
export function measurementCache(context) {
  const cache = new Map(); let units = 0, calls = 0;
  return {
    clear() { cache.clear(); units = 0; },
    get calls() { return calls; },
    get size() { return cache.size; },
    measure(font, text, clusters, emojis, correction = 0) {
      const key = `${font.key}\0${text}`;
      const hit = cache.get(key);
      if (hit !== undefined) { cache.delete(key); cache.set(key, hit); return hit; }
      context.font = font.font;
      for (const [key, value] of Object.entries(font.extra)) if (key in context) context[key] = value;
      let nativeSpacing = false;
      if ('letterSpacing' in context) {
        context.letterSpacing = `${font.spacing}px`;
        nativeSpacing = px(context.letterSpacing) === font.spacing;
      }
      const width = Math.max(0, context.measureText(text).width + (nativeSpacing ? 0 : font.spacing * clusters) - correction * emojis);
      calls++;
      if (key.length <= MAX_CACHE_UNITS) {
        while (cache.size >= MAX_CACHE || units + key.length > MAX_CACHE_UNITS) {
          const first = cache.keys().next().value; units -= first.length; cache.delete(first);
        }
        cache.set(key, width); units += key.length;
      }
      return width;
    },
  };
}

function floats(values) {
  const bytes = new Uint8Array(values.length * 4), view = new DataView(bytes.buffer);
  values.forEach((value, i) => view.setFloat32(i * 4, value, true));
  return bytes;
}
function sourceInput(text, overflow, whiteSpace) {
  const source = encoder.encode(text), bytes = new Uint8Array(source.length + 8);
  new DataView(bytes.buffer).setUint32(0, overflow === 'anywhere' ? 2 : overflow === 'break-word' ? 1 : 0, true);
  new DataView(bytes.buffer).setUint32(4, whiteSpace === 'pre-wrap' ? 1 : 0, true);
  bytes.set(source, 8); return bytes;
}
function flowInput(snapshot, exclusions) {
  // @ref LLP 1043.000 §3 D5 — wasm owns the fragment floor; send the strut font size.
  const bytes = floats([snapshot.width, snapshot.lineHeight, snapshot.font.size, 0, snapshot.height, 0,
    ...exclusions.flatMap(e => [0, e.width, e.height, e.x - snapshot.x, e.y - snapshot.y])]);
  const view = new DataView(bytes.buffer);
  view.setUint32(12, snapshot.maxLines, true);
  view.setUint32(20, snapshot.direction === 'rtl' ? 1 : 0, true);
  exclusions.forEach((e, i) => view.setUint32(24 + i * 20, e.id, true));
  return bytes;
}

// getBoundingClientRect is the batched, fractional fast path. CSS transforms
// never alter exclusions (D8); transformed trees use untransformed offset boxes.
// No animation value is sampled or sent to the walker.
function layoutBox(el, styles) {
  const rect = el.getBoundingClientRect();
  let transformed = false;
  for (let a = el; a; a = a.parentElement) {
    if (!styles.has(a)) styles.set(a, getComputedStyle(a));
    const s = styles.get(a);
    if (s.transform !== 'none' || (s.translate && s.translate !== 'none') || (s.scale && s.scale !== 'none') || (s.rotate && s.rotate !== 'none')) transformed = true;
  }
  if (!transformed) return { x: rect.x, y: rect.y, width: rect.width, height: rect.height, transformed: false };
  let x = 0, y = 0;
  for (let a = el; a; a = a.offsetParent) {
    x += a.offsetLeft; y += a.offsetTop;
    if (a.offsetParent) { x += a.offsetParent.clientLeft; y += a.offsetParent.clientTop; }
  }
  for (let a = el.parentElement; a; a = a.parentElement) { x -= a.scrollLeft; y -= a.scrollTop; }
  return { x, y, width: el.offsetWidth, height: el.offsetHeight, transformed: true };
}

// A percentage against an auto-height containing block is not definite. CSSOM
// height reports a used px value even then; checking that value would collapse
// the paragraph as soon as its children become absolute. Only explicit chains
// are admitted here; stretched/implicit percentage definiteness waits for M8.
function definiteHeight(el) {
  for (let node = el; node; node = node.parentElement) {
    const height = node.style.height || '';
    if (!height.endsWith('%')) return node === el || /px$|^(?:calc|env)\(/.test(height);
  }
  return false;
}

function sourceRuns(el, mac) {
  const runs = [], walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
  let at = 0;
  while (walker.nextNode()) {
    const node = walker.currentNode, text = node.data, parent = node.parentElement;
    if (!text) continue;
    const style = getComputedStyle(parent), lang = parent.closest('[lang]')?.lang || document.documentElement.lang;
    const chain = [];
    for (let a = parent; a && a !== el; a = a.parentElement) chain.unshift(a);
    runs.push({ start: at, end: at + text.length, text, font: fontStyle(style, mac, lang), chain });
    at += text.length;
  }
  return runs;
}

function replaceNodes(element, nodes) {
  element.replaceChildren();
  for (const node of nodes) element.append(node); // no argument-stack bound on rich text
}

function paintContent(span, fragment, state, lineHeight) {
  const children = [];
  // Binary search avoids scanning all inline runs for every fragment.
  let i = lower(state.runEnds, fragment.paint_start + 1);
  for (; i < state.runs.length && state.runs[i].start < fragment.paint_end; i++) {
    const run = state.runs[i];
    let outer = null, inner = null;
    for (const original of run.chain) {
      const clone = original.cloneNode(false);
      clone.removeAttribute('id'); clone.removeAttribute('data-testid');
      // Original listeners remain valid by kernel view identity while detached.
      // Forward authored events only; a plain <a> keeps its native default.
      for (const event of original.exactFlowEvents ?? []) clone.addEventListener(event, e => {
        e.stopPropagation();
        const forwarded = new e.constructor(e.type, e);
        if (!original.dispatchEvent(forwarded)) e.preventDefault();
      });
      if (inner) inner.append(clone); else outer = clone;
      inner = clone;
    }
    const piece = document.createElement('span');
    // Normal collapses source whitespace, including consumed hard-break controls;
    // max-content on the absolutely positioned outer prevents independent wrapping.
    piece.style.cssText = `white-space:${state.whiteSpace === 'pre-wrap' ? 'pre' : 'normal'};font:${run.font.font};letter-spacing:${run.font.spacing}px;line-height:${lineHeight}px;`;
    // Keep color inherited through cloned authored runs: light-dark() must
    // respond to setScheme without a measurement, batch style op or DOM rebuild.
    piece.style.fontKerning = run.font.extra.fontKerning;
    piece.style.fontStretch = run.font.extra.fontStretch;
    piece.style.fontVariantCaps = run.font.extra.fontVariantCaps;
    piece.style.fontFeatureSettings = run.font.fontFeatureSettings;
    piece.style.fontVariationSettings = run.font.fontVariationSettings;
    piece.exactFlowSourceStart = Math.max(run.start, fragment.paint_start);
    piece.textContent = run.text.slice(Math.max(0, fragment.paint_start - run.start), fragment.paint_end - run.start);
    if (inner) inner.append(piece); else outer = piece;
    children.push(outer);
  }
  if (fragment.ellipsis) {
    const piece = document.createElement('span');
    const font = state.runs[fragment.ellipsisRun]?.font;
    if (font) piece.style.cssText = `font:${font.font};letter-spacing:${font.spacing}px;`;
    piece.textContent = '…'; piece.exactFlowSourceStart = fragment.paint_end;
    children.push(piece);
  }
  replaceNodes(span, children);
  span.dataset.hyphen = String(fragment.hyphenated);
}

export function createTextFlow({ views, request, advance, agentMode, log = console.error,
  now = () => globalThis.exact?.now?.() ?? performance.now(),
  raf = requestAnimationFrame, cancel = cancelAnimationFrame, delay = setTimeout, clearDelay = clearTimeout }) {
  const states = new Map(), calibration = new Map(), autoHeightWarned = new Set();
  const canvas = document.createElement('canvas');
  // A connected canvas inherits the document language in WebKit too.
  canvas.hidden = true; document.body.append(canvas);
  const context = canvas.getContext('2d'), metrics = measurementCache(context);
  const sheet = document.createElement('style');
  sheet.textContent = '[data-flow-fragment][data-hyphen="true"]::after{content:"-"}';
  document.head.append(sheet);
  const mac = /Mac|iPhone|iPad|iPod/.test(navigator.platform);
  let contexts = [], dirty = false, fontsReady = false, disposed = false, pendingSelection = null;
  const clock = createTimerScheduler({ now, advance, agentMode, paint: onFrame, raf, cancel, delay, clearDelay });
  const observer = new ResizeObserver(() => invalidate());
  const observed = new Set();
  const wake = () => { if (!disposed) clock.requestFrame(); };
  function invalidate(allBoxes = true) {
    if (allBoxes) for (const s of states.values()) s.box = null;
    dirty = true; wake();
  }
  function release(s) {
    if (s.prepared || s.boundaries) request(3, s.id, new Uint8Array());
    s.prepared = false; s.boundaries = null;
  }
  function restore(s) {
    if (!s.rendered) return;
    replaceNodes(s.el, s.original);
    s.el.style.position = s.position;
    s.el.style.contain = s.contain;
    s.rendered = false;
    s.last = [];
  }
  function refreshFonts() {
    pendingSelection ??= selectionSnapshot();
    metrics.clear();
    for (const p of calibration.values()) p.element?.remove();
    calibration.clear();
    for (const s of states.values()) { restore(s); s.needsSource = true; s.prepared = false; }
    invalidate();
  }
  document.fonts.ready.then(() => { if (!disposed) { fontsReady = true; refreshFonts(); } });
  document.fonts.addEventListener('loadingdone', refreshFonts);
  document.addEventListener('scroll', invalidate, true);

  function reconcile() {
    const wanted = new Map(), targets = new Set();
    for (const c of contexts) {
      for (const id of c.exclusions) if (views.has(id)) targets.add(views.get(id));
      if (views.has(c.id)) targets.add(views.get(c.id));
      for (const p of c.paragraphs) {
        let item = wanted.get(p.id);
        if (!item) wanted.set(p.id, item = { ...p, exclusions: new Set() });
        c.exclusions.forEach(id => item.exclusions.add(id));
      }
    }
    for (const [id, s] of states) if (!wanted.has(id) || views.get(id) !== s.el) {
      restore(s); request(3, id, new Uint8Array()); states.delete(id);
    }
    for (const [id, info] of wanted) {
      const el = views.get(id); if (!el) continue;
      targets.add(el);
      let s = states.get(id);
      if (!s) states.set(id, s = { id, el, original: [], runs: [], runEnds: [], pool: [], last: [], needsSource: true,
        prepared: false, rendered: false, prepareCount: 0, facts: null });
      s.info = info;
    }
    for (const el of observed) if (!targets.has(el)) { observer.unobserve(el); observed.delete(el); }
    for (const el of targets) if (!observed.has(el)) { observer.observe(el); observed.add(el); }
  }

  function readFrame() {
    const styles = new Map(), boxes = new Map();
    const readBox = id => {
      if (boxes.has(id)) return boxes.get(id);
      const el = views.get(id);
      const box = el?.isConnected ? layoutBox(el, styles) : null;
      if (box) { box.id = id; box.hidden = !el.getClientRects().length; }
      boxes.set(id, box); return box;
    };
    // Emoji calibration elements were installed in the previous write phase.
    for (const p of calibration.values()) if (p.element && p.correction == null) {
      context.font = p.font.font;
      if ('letterSpacing' in context) context.letterSpacing = '0px';
      const width = context.measureText('😀').width;
      const delta = width - p.element.getBoundingClientRect().width;
      p.correction = width > p.font.size + 0.5 && delta > 0.5 ? delta : 0;
    }
    return [...states.values()].map(s => {
      const box = s.box ?? readBox(s.id);
      if (!s.box && box) { s.box = box; s.boxStyle = styles.get(s.el); }
      if (s.boxStyle) styles.set(s.el, s.boxStyle);
      if (!box || box.hidden) return { s, skipped: 'paragraph is hidden' };
      const style = styles.get(s.el), font = fontStyle(style, mac, s.el.closest('[lang]')?.lang || document.documentElement.lang);
      // @ref LLP 1043.000 §3 D4/D7 — box intersection precedes source
      // extraction/preparation. Moving exclusions reuse stable paragraph boxes.
      const exclusions = [...s.info.exclusions].map(readBox).filter(b => {
        if (!b || b.hidden) return false;
        const margin = Math.max(0, px(styles.get(views.get(b.id))?.shapeMargin));
        return b.x - margin < box.x + box.width && b.x + b.width + margin > box.x
          && b.y - margin < box.y + box.height && b.y + b.height + margin > box.y;
      });
      s.meetingIds = new Set(exclusions.map(e => e.id));
      if (!s.info.definite || !definiteHeight(s.el)) {
        const meeting = exclusions.length > 0;
        return meeting ? { s, autoHeight: true, skipped: 'height is not proven definite; auto-height flow requires M8' } : { s, inactive: true };
      }
      if (s.needsSource) {
        s.original = [...s.el.childNodes]; s.runs = sourceRuns(s.el, mac); s.runEnds = s.runs.map(r => r.end);
        s.text = s.runs.map(r => r.text).join(''); s.needsSource = false; s.recheck = true;
        s.position = s.el.style.position; s.contain = s.el.style.contain;
      }
      const fonts = [font, ...s.runs.map(r => r.font)];
      const lineHeight = fonts.reduce((height, f) => Math.max(height, f.lineHeight || f.size * 1.2), 0);
      const left = px(style.borderLeftWidth) + px(style.paddingLeft), top = px(style.borderTopWidth) + px(style.paddingTop);
      return { s, font, distance: Math.max(0, box.y - (globalThis.innerHeight || 900), -box.y - box.height), width: Math.max(0, box.width - left - px(style.borderRightWidth) - px(style.paddingRight)),
        height: Math.max(0, box.height - top - px(style.borderBottomWidth) - px(style.paddingBottom)),
        x: box.x + left, y: box.y + top, contentOrigin: { x: left, y: top }, padX: px(style.paddingLeft), padY: px(style.paddingTop),
        lineHeight, maxLines: px(style.webkitLineClamp), overflow: style.overflowWrap, whiteSpace: style.whiteSpace,
        align: style.textAlign, direction: style.direction, position: style.position, exclusions, transformed: box.transformed };
    });
  }

  function prepare(snapshot) {
    const { s } = snapshot;
    if (s.prepared && !s.recheck) return true;
    s.recheck = false;
    const stamp = JSON.stringify([s.text, snapshot.overflow, snapshot.whiteSpace, snapshot.font.key, s.runs.map(r => [r.start, r.end, r.font.key])]);
    if (stamp !== s.prepareKey) { s.prepareKey = stamp; s.prepared = false; s.boundaries = null; }
    if (s.prepared) return true;
    if (!s.boundaries) {
      const data = request(0, s.id, sourceInput(s.text, snapshot.overflow, snapshot.whiteSpace));
      if (data.error) throw Error(data.error);
      s.boundaries = data;
      s.clusterStarts = data.graphemes.map(g => g[2]);
      s.emojiPrefix = [0];
      for (const g of data.graphemes) s.emojiPrefix.push(s.emojiPrefix.at(-1) + Number(/\p{Emoji_Presentation}|\p{Emoji}\uFE0F/u.test(s.text.slice(g[2], g[3]))));
    }
    if (s.emojiPrefix.at(-1)) {
      for (const r of s.runs) if (!calibration.has(r.font.key)) calibration.set(r.font.key, { font: r.font, correction: null, element: null });
      if (s.runs.some(r => calibration.get(r.font.key)?.correction == null)) return false;
    }
    const widths = s.boundaries.ranges.map(([, , start, end]) => {
      let total = 0;
      for (let i = lower(s.runEnds, start + 1); i < s.runs.length && s.runs[i].start < end; i++) {
        const r = s.runs[i], a = Math.max(start, r.start), b = Math.min(end, r.end);
        const from = lower(s.clusterStarts, a), to = lower(s.clusterStarts, b);
        const raw = s.text.slice(a, b);
        const whitespace = snapshot.whiteSpace === 'pre-wrap' ? raw.replace(/\t/g, '        ') : raw.replace(/[ \t\n\r\f\u0085\u2028\u2029]+/g, ' ');
        const text = whitespace.replace(/[\u00ad\u200b\u200e\u200f\u202a-\u202e\u2066-\u2069]/g, '');
        total += metrics.measure(r.font, text, to - from, s.emojiPrefix[to] - s.emojiPrefix[from], calibration.get(r.font.key)?.correction || 0);
      }
      return total;
    });
    const result = request(1, s.id, floats([metrics.measure(snapshot.font, '-', 1, 0), ...widths]));
    if (result.error) throw Error(result.error);
    s.whiteSpace = snapshot.whiteSpace; s.prepared = true; s.prepareCount++; return true;
  }

  // The final clamped fragment reserves real run-font ellipsis width. Binary
  // search caps canvas work at O(log G) prefix probes (source is <=64 KiB).
  function clampFragment(f, snapshot) {
    const { s } = snapshot;
    const ink = end => {
      const run = Math.max(0, lower(s.runEnds, end));
      return { run, width: metrics.measure(s.runs[run]?.font ?? snapshot.font, '…', 1, 0) };
    };
    const boundaries = s.clusterStarts.filter(p => p >= f.paint_start && p <= f.paint_end);
    if (boundaries.at(-1) !== f.paint_end) boundaries.push(f.paint_end);
    const width = end => {
      let total = 0;
      for (const r of s.runs) {
        const a = Math.max(f.paint_start, r.start), b = Math.min(end, r.end);
        if (a >= b) continue;
        let text = s.text.slice(a, b).replace(/[\u00ad\u200b]/g, '');
        if (snapshot.whiteSpace !== 'pre-wrap') text = text.replace(/[ \t\r\n\f\u0085\u2028\u2029]+/g, ' ');
        if (b === end) text = text.trimEnd();
        total += metrics.measure(r.font, text, lower(s.clusterStarts, b) - lower(s.clusterStarts, a), 0);
      }
      return total;
    };
    let lo = 0, hi = boundaries.length;
    while (lo < hi) { const mid = (lo + hi) >>> 1; if (width(boundaries[mid]) + ink(boundaries[mid]).width <= f.available) lo = mid + 1; else hi = mid; }
    const end = boundaries[Math.max(0, lo - 1)] ?? f.paint_start;
    const ellipsis = ink(end), fits = ellipsis.width <= f.available;
    return { ...f, paint_end: end, width: width(end) + (fits ? ellipsis.width : 0),
      ellipsis: fits, ellipsisRun: ellipsis.run, hyphenated: false };
  }

  function compute(snapshot) {
    const { s } = snapshot;
    if (snapshot.skipped || snapshot.inactive || !fontsReady) return snapshot;
    try {
      const key = JSON.stringify([snapshot.width, snapshot.height, snapshot.lineHeight, snapshot.maxLines, snapshot.align,
        snapshot.direction, snapshot.x, snapshot.y, snapshot.exclusions]);
      if (s.geometry === key && s.rendered) return { s, unchanged: true };
      if (!prepare(snapshot)) return { ...snapshot, pending: true };
      const facts = request(2, s.id, flowInput(snapshot, snapshot.exclusions));
      if (facts.error) throw Error(facts.error);
      if (!facts.complete && !facts.clamped) return { s, skipped: 'flow band limit reached; ordinary text retained', partial: facts };
      const fragments = facts.fragments.map((f, index) => {
        if (facts.clamped && index + 1 === facts.fragments.length) f = clampFragment(f, snapshot);
        const spare = Math.max(0, f.available - f.width);
        const align = snapshot.align === 'start' || snapshot.align === 'justify' ? (snapshot.direction === 'rtl' ? 'right' : 'left')
          : snapshot.align === 'end' ? (snapshot.direction === 'rtl' ? 'left' : 'right') : snapshot.align;
        return { ...f, x: f.x + (align === 'center' ? spare / 2 : align === 'right' ? spare : 0), y: f.y };
      });
      return { ...snapshot, facts, fragments, key };
    } catch (error) { return { s, skipped: String(error) }; }
    finally { if (!snapshot.retain) release(s); }
  }

  function write(result) {
    const { s } = result;
    if (result.unchanged || result.pending) return;
    if (result.inactive) { release(s); restore(s); s.facts = { fragments: [], shapes: [] }; return; }
    if (!result.facts) {
      if (result.skipped) {
        release(s); restore(s); s.facts = { fragments: [], shapes: [], ...result.partial, skipped: result.skipped };
        if (result.autoHeight) {
          if (!autoHeightWarned.has(s.id)) { autoHeightWarned.add(s.id); log(`wrap-flow: text #${s.id} has auto height and is not flowed (LLP 1043.000 stage 2)`); }
        } else if (s.error !== result.skipped) { log(`textflow #${s.id}: ${result.skipped}`); s.error = result.skipped; }
      }
      return;
    }
    if (result.facts.limited && !s.shapeLimitWarned) {
      log(`wrap-flow: text #${s.id} exceeds 64 exclusions; overflow uses a conservative bounding rectangle`); s.shapeLimitWarned = true;
    }
    const { fragments } = result;
    const diff = fragmentDiff(s.last, fragments, !s.rendered);
    if (!s.rendered) {
      s.el.replaceChildren();
      if (result.position === 'static') s.el.style.position = 'relative';
      s.el.style.contain = 'layout';
      s.rendered = true;
    }
    for (const change of diff.updates) {
      const i = change.index, f = fragments[i];
      let span = s.pool[i];
      if (!span) {
        span = document.createElement('span'); span.dataset.flowFragment = '';
        span.style.cssText = 'position:absolute;left:0;top:0;white-space:pre;width:max-content;';
        s.pool[i] = span;
      }
      if (change.content) {
        span.style.font = result.font.font; span.style.letterSpacing = `${result.font.spacing}px`;
        span.style.lineHeight = `${result.lineHeight}px`; span.style.direction = result.direction;
        paintContent(span, f, s, result.lineHeight);
      }
      if (change.position) span.style.transform = `translate(${f.x + result.padX}px,${f.y + result.padY}px)`;
      if (span.parentNode !== s.el) s.el.append(span);
    }
    for (let i = fragments.length; i < s.last.length; i++) s.pool[i]?.remove();
    s.last = fragments; s.geometry = result.key;
    s.facts = { ...result.facts, fragments, coordinate_space: 'content', content_origin: result.contentOrigin, prepare_count: s.prepareCount, touched_spans: diff.updates.length,
      pooled_spans: s.pool.length, measurement_calls: metrics.calls, transformed_box: result.transformed };
    // Definite height is untouched. M8 can publish result.facts.height here,
    // in this write phase, once auto-height admission comes from the host.
  }

  function selectionSnapshot() {
    const selection = document.getSelection();
    if (!selection?.anchorNode || !selection.focusNode) return null;
    const point = (node, offset) => {
      for (const s of states.values()) if (s.el.contains(node)) {
        const sourceStart = node.parentElement?.exactFlowSourceStart;
        if (s.rendered && sourceStart != null) return { id: s.id, offset: sourceStart + offset };
        const range = document.createRange(); range.selectNodeContents(s.el); range.setEnd(node, offset);
        return { id: s.id, offset: range.toString().length };
      }
      return { node, offset };
    };
    const anchor = point(selection.anchorNode, selection.anchorOffset), focus = point(selection.focusNode, selection.focusOffset);
    return anchor.id != null || focus.id != null ? { anchor, focus } : null;
  }
  function restoreSelection(saved) {
    if (!saved) return;
    const point = p => {
      if (p.id == null) return p;
      const s = states.get(p.id); if (!s) return null;
      const walker = document.createTreeWalker(s.el, NodeFilter.SHOW_TEXT);
      let offset = p.offset, last = null;
      while (walker.nextNode()) {
        const node = walker.currentNode; last = node;
        const sourceStart = node.parentElement?.exactFlowSourceStart;
        if (s.rendered && sourceStart != null) {
          if (p.offset <= sourceStart + node.data.length) return { node, offset: Math.max(0, p.offset - sourceStart) };
        } else {
          if (offset <= node.data.length) return { node, offset };
          offset -= node.data.length;
        }
      }
      return last ? { node: last, offset: last.data.length } : { node: s.el, offset: 0 };
    };
    const a = point(saved.anchor), b = point(saved.focus);
    if (a?.node.isConnected && b?.node.isConnected) document.getSelection()?.setBaseAndExtent(a.node, a.offset, b.node, b.offset);
  }

  function flush() {
    if (!dirty || !fontsReady || disposed) return false;
    dirty = false;
    const selected = pendingSelection ?? selectionSnapshot(); pendingSelection = null;
    const snapshots = readFrame(); // reads first; stable paragraph boxes are cached
    const meeting = snapshots.filter(s => s.exclusions?.length).sort((a, b) => a.distance - b.distance);
    meeting.forEach((s, i) => {
      if (i >= 64) s.skipped = '64 paragraph budget reached; nearest-to-viewport paragraphs retain flow';
      // Reserve one temporary slot for clear paragraphs and the 64th walker.
      s.retain = i < 63;
    });
    for (const s of snapshots) if (!s.retain) release(s.s);
    snapshots.sort((a, b) => Number(!!a.retain) - Number(!!b.retain));
    let moved = false;
    for (const snapshot of snapshots) if (snapshot.exclusions) {
      const boxes = JSON.stringify([snapshot.x, snapshot.y, snapshot.width, snapshot.height, snapshot.exclusions]);
      moved ||= snapshot.s.boxes != null && snapshot.s.boxes !== boxes; snapshot.s.boxes = boxes;
    }
    framePhases(snapshots, value => value, compute, write);
    restoreSelection(selected);
    let pending = false;
    for (const p of calibration.values()) {
      if (p.correction != null) { p.element?.remove(); p.element = null; continue; }
      if (!p.element) {
        const span = document.createElement('span'); span.textContent = '😀';
        span.style.cssText = `position:absolute;visibility:hidden;white-space:pre;display:inline-block;font:${p.font.font};letter-spacing:0;`;
        span.lang = p.font.extra.lang; document.body.append(span); p.element = span;
      }
      pending = true;
    }
    if (pending || moved) { dirty = true; wake(); }
    return pending;
  }
  function onFrame() {
    // The scheduler advances first, then paints in this same frame.
    flush();
    if (dirty) wake();
  }

  return {
    beforeBatch(batch) {
      const ops = batch.ops ?? [];
      const topology = ops.some(op => ['children', 'roots', 'destroy', 'textflow'].includes(op.op));
      for (const s of states.values()) {
        const relevant = topology || ops.some(op => ['style', 'props'].includes(op.op) &&
          (op.id === s.id || views.get(op.id)?.contains(s.el) || s.runs.some(r => r.chain.includes(views.get(op.id)))));
        if (relevant) { pendingSelection ??= selectionSnapshot(); restore(s); s.needsSource = true; s.geometry = null; }
      }
    },
    afterBatch(batch) {
      const op = batch.ops?.find(op => op.op === 'textflow');
      if (op) { contexts = op.contexts; reconcile(); }
      for (const s of states.values()) if (batch.ops?.some(op => op.op === 'style' && s.meetingIds?.has(op.id))) s.geometry = null;
      clock.update(batch.timer_due_ms);
      if (batch.ops?.some(op => ['style', 'props', 'create', 'destroy', 'children', 'textflow'].includes(op.op))) {
        const onlyExclusions = batch.ops.every(op => op.op === 'style' && contexts.some(c => c.exclusions.includes(op.id)));
        invalidate(!onlyExclusions);
      }
    },
    async settle() {
      await document.fonts.ready; fontsReady = true;
      // Calibration needs a separate write/read cycle, never alternating nodes.
      for (let i = 0; i < 3 && dirty; i++) { flush(); if (dirty) await new Promise(resolve => raf(resolve)); }
    },
    facts(id) { return states.get(id)?.facts ?? null; },
    reset() {
      clock.reset();
      for (const s of states.values()) restore(s);
      states.clear(); autoHeightWarned.clear(); pendingSelection = null; contexts = []; observed.clear(); observer.disconnect(); dirty = false;
      refreshFonts();
    },
    dispose() {
      this.reset(); disposed = true;
      clock.dispose();
      document.fonts.removeEventListener('loadingdone', refreshFonts); document.removeEventListener('scroll', invalidate, true); canvas.remove(); sheet.remove();
    },
  };
}

// Module scripts register with the same on-demand loader as Rust/GPU glue.
if (globalThis.exact) globalThis.exact.createTextFlow = createTextFlow;
