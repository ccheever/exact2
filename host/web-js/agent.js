// The agent's door into a JS-target page (LLP 1012's operations, as far as
// the JS target carries them): `exact.agentSettled(request)` answers what
// `scripts/agent.mjs web` asks. Input and screenshots stay the carrier's own
// (CDP). Loaded only under `?agent`; never part of an app's boot bytes.
import names, { types } from './names.js';
import { R, eq, pieces, pageHistory, Head } from './rt.js';
import * as perf from './perf.js';
import { environment, navigation, guestOutline, guestTap, guestType, viewBox, foldEnv, preferFold } from './navigation.js';
// A runtime value as the runner's typed JSON: records by field name.
const typed = (v, t) => v == null || typeof t === 'string' ? v : Array.isArray(t) ? (t[0] === '?' ? typed(v, t[1]) : v.map(x => typed(x, t[1]))) : Object.fromEntries(Object.keys(t).map((k, i) => [k, typed(v[i], t[k])]));
const PROPS = [['aria-live', 'accessibilityLive'], ['role', 'accessibilityRole'], ['aria-description', 'accessibilityHint'], ['aria-keyshortcuts', 'accessibilityKeyShortcuts'], ['aria-orientation', 'accessibilityOrientation'], ['aria-pressed', 'accessibilityPressed'], ['aria-level', 'accessibilityHeadingLevel', 1], ['aria-posinset', 'accessibilityPosInSet', 1], ['aria-setsize', 'accessibilitySetSize', 1], ['placeholder', 'placeholder'], ['viewportFit', 'viewportFit'], ['interactiveWidget', 'interactiveWidget'], ['data-hook', 'hook'], ['data-nativeviewmodulename', 'nativeViewModuleName'], ['data-nativeviewprops', 'nativeViewProps']];
const TYPES = { TEMPLATE: 'Head', BUTTON: 'Pressable', INPUT: 'TextInput', TEXTAREA: 'TextInput', VIDEO: 'Video', IMG: 'Image', IFRAME: 'WebView', A: 'Pressable' };
export function install(exact) {
  const views = exact.views, id = exact.viewId;
  // A Markdown text's pieces are its content, not views.
  // A view leaving with its exit animation (presence-glue.js) is no view: the runner destroyed it.
  // A paragraph text flows around shapes is its fragments on the page; the
  // tree is its own text and runs (flow.js keeps them).
  const flowed = el => el.$flow && el.querySelector(':scope > [data-flow-fragment]');
  const kids = el => el.getAttribute('markup') === 'markdown' ? [] : flowed(el) ? el.$flow.kids : [...el.children].filter(c => !c.hasAttribute('data-surface') && !c.hasAttribute('data-exiting'));
  // A text's inline runs are text nodes too (the runner's tree; element.rs
  // marks only the paragraph `data-exact-text`).
  // (A flowed paragraph's runs are held off the page: runs too.)
  const run = el => !el.isConnected || el.parentElement?.hasAttribute('data-exact-text') && el.parentElement.getAttribute('markup') !== 'markdown';
  // An SVG element's node type, by element.rs's tags (a nested `svg` is a viewport).
  const SVG = { svg: 'Svg', g: 'SvgGroup', path: 'SvgPath', polyline: 'SvgPolyline', polygon: 'SvgPolygon', circle: 'SvgCircle', line: 'SvgLine', rect: 'SvgRect', ellipse: 'SvgEllipse', defs: 'SvgDefs', linearGradient: 'SvgLinearGradient', radialGradient: 'SvgRadialGradient', stop: 'SvgStop', use: 'SvgUse', symbol: 'SvgSymbol', clipPath: 'SvgClipPath', text: 'SvgText', tspan: 'SvgTSpan', marker: 'SvgMarker', mask: 'SvgMask', pattern: 'SvgPattern', foreignObject: 'SvgForeignObject', filter: 'SvgFilter' };
  const svg = el => el.localName === 'svg' && el.parentElement?.namespaceURI === el.namespaceURI ? 'SvgViewport' : SVG[el.localName] ?? (el.localName.startsWith('fe') ? 'SvgFe' : 'View');
  const type = el => el.exactNative ? 'NativeView' : el.namespaceURI === 'http://www.w3.org/2000/svg' ? svg(el) : el.exactMarkup ? 'TextInput' : el.localName === 'select' || el.localName === 'button' && el.hasAttribute('data-button-style') || el.localName === 'input' && /^(file|checkbox|range|date|time|datetime-local)$/.test(el.type) ? 'Control' : el.localName === 'option' || el.hasAttribute('data-exact-text') || run(el) ? 'Text' : el.querySelector(':scope > canvas[data-surface]') ? 'Canvas' : el.dataset.scroll ? (el.getAttribute('role') === 'list' ? 'List' : 'ScrollView') : TYPES[el.tagName] ?? 'View';
  const record = (el, depth) => {
    const props = {};
    if (el.dataset.testid) props.testId = el.dataset.testid;
    if (el.tagName === 'IMG') props.imageSource = el.dataset.symbolSource ?? el.getAttribute('src') ?? '';
    if (el.hasAttribute('aria-label')) props.accessibilityLabel = el.getAttribute('aria-label');
    else if (el.tagName === 'IMG' && el.getAttribute('alt')) props.accessibilityLabel = el.getAttribute('alt');
    // A paragraph of runs has no text of its own: its runs carry it.
    if (/^(Text|SvgText|SvgTSpan)$/.test(type(el)) && (el.$source != null || !kids(el).length)) props.text = el.$source ?? (flowed(el) ? el.$flow.text : el.textContent);
    if ('value' in el && el.tagName !== 'BUTTON' && el.type !== 'checkbox') props.value = el.value;
    // The runner's props that element.rs writes as attributes, by its names.
    for (const [attr, prop, num] of PROPS) if (el.hasAttribute(attr)) props[prop] = num ? Number(el.getAttribute(attr)) : el.getAttribute(attr);
    // The intent `tree --ax` reads (LLP 1080.002 D7), as the runner names it.
    if (el.hasAttribute('inert')) props.inert = true;
    if (el.getAttribute('aria-hidden') === 'true') props.accessibilityElementsHidden = true;
    if (el.getAttribute('aria-modal') === 'true') props.accessibilityModal = true;
    if (el.hasAttribute('autofocus')) props.autofocus = true; else if (el.dataset.autofocus === 'false') props.autofocus = false;
    const n = { id: id(el), type: type(el), depth, props };
    if (el.dataset.exactOn) n.handlers = el.dataset.exactOn.split(' ');
    if (document.activeElement === el) n.focused = true;
    // As glue.js's `tree` adds them: an iframe's url, load state and
    // same-origin guest outline (LLP 1020 D4).
    // A module view's status (LLP 1024 D8.3): what native-glue.js keeps on
    // the element, as glue.js's `tree` reports it.
    if (el.exactNative) n.module = el.exactNative.status();
    if (el instanceof HTMLIFrameElement) {
      n.url = el.getAttribute('src') ?? '';
      n.loading = loaded.get(el) !== el.getAttribute('src');
      const guest = guestOutline(el);
      if (guest !== null) n.guest = guest;
    }
    return n;
  };
  const all = () => {
    const out = [];
    const walk = (el, d) => { const n = record(el, d); out.push(n); n.children = kids(el).map(c => walk(c, d + 1).id); return n; };
    kids(document.getElementById('exact-root')).forEach(el => walk(el, 0));
    return out;
  };
  // The runner's tags (LLP 1035.002 D3): a commit is an epoch; a JS page
  // has one incarnation (a plan swap is a new page).
  const tags = () => ({ clock: exact.clock.now, epoch: exact.clock.epoch, incarnation: 1 });
  // An iframe's latest src load (glue.js `iframeLoading`): loading until the
  // load event of the src it has now.
  const loaded = new WeakMap();
  // (A load event does not reach the window: the document hears it.)
  document.addEventListener('load', e => { if (e.target instanceof HTMLIFrameElement) loaded.set(e.target, e.target.getAttribute('src')); }, true);
  for (const f of document.querySelectorAll('#exact-root iframe')) { try { if (f.contentDocument?.readyState === 'complete' && f.contentWindow.location.href !== 'about:blank') loaded.set(f, f.getAttribute('src')); } catch {} }
  // `layout <node>` (LLP 1035.002 D1), as glue.js `nodeDetail` gives the
  // host's half. The JS target keeps no kernel, so the rows are the page's:
  // an own row is a declaration of the element's class or inline style
  // (`dynamic` when a binding wrote it), an inherited one comes from the
  // nearest view that declares it, else `initial`; values are the browser's
  // computed ones, under the runner's row names.
  const INHERITED = { text_color: 'color', font_family: 'font-family', font_size: 'font-size', font_weight: 'font-weight', font_style: 'font-style', line_height: 'line-height', letter_spacing: 'letter-spacing', font_variant_numeric: 'font-variant-numeric', direction: 'direction', white_space: 'white-space', overflow_wrap: 'overflow-wrap', text_align: 'text-align' };
  const rowOf = prop => Object.keys(INHERITED).find(k => INHERITED[k] === prop) ?? prop.replace(/^-+/, '').replace(/-/g, '_');
  // A class's rule may be nested: the build wraps them all in
  // `#exact-root#exact-root { & .cN { … } }` for specificity (emit.rs).
  const declared = el => {
    const out = new Set(el.style), classes = new Set([...el.classList].map(c => '.' + c));
    const walk = rules => { for (const r of rules) {
      if (r.style && classes.has(r.selectorText?.replace(/^&\s+/, ''))) for (const p of r.style) out.add(p);
      if (r.cssRules) walk(r.cssRules);
    } };
    for (const sheet of document.styleSheets) { let rules; try { rules = sheet.cssRules; } catch { continue; } walk(rules); }
    return out;
  };
  const r2 = x => Math.round(x * 100) / 100;
  const nodeDetail = nid => {
    const el = views.get(nid);
    if (!el || !el.isConnected) return { error: `stale node #${nid} (incarnation 1)` };
    const own = declared(el), cs = getComputedStyle(el), style = {};
    for (const p of own) if (!p.startsWith('--')) style[rowOf(p)] = { value: cs.getPropertyValue(p), source: el.$css?.[p] !== undefined ? 'dynamic' : 'authored' };
    for (const [row, prop] of Object.entries(INHERITED)) {
      if (own.has(prop)) continue;
      let from = null;
      for (let a = el.parentElement; a && a.id !== 'exact-root'; a = a.parentElement) if (a.style.getPropertyValue(prop) || declared(a).has(prop)) { from = id(a); break; }
      style[row] = { value: cs.getPropertyValue(prop), ...(from != null ? { source: 'inherited', from } : { source: 'initial' }) };
    }
    const r = viewBox(el), rect = b => ({ x: r2(b.x), y: r2(b.y), w: r2(b.width), h: r2(b.height) });
    const scroll = [], clip = [];
    let clipped = r.width === 0 || r.height === 0, parent = null;
    for (let a = el.parentElement; a && a.id !== 'exact-root'; a = a.parentElement) {
      const aid = id(a);
      if (aid == null) continue;
      parent ??= aid;
      const ac = getComputedStyle(a);
      if (a.dataset.scroll === 'true') scroll.unshift({ id: aid, sx: r2(a.scrollLeft), sy: r2(a.scrollTop) });
      for (const kind of (ac.overflowX !== 'visible' || ac.overflowY !== 'visible' ? ['overflow'] : []).concat(ac.clipPath !== 'none' ? ['clip-path'] : [])) {
        clip.unshift({ id: aid, kind });
        const c = a.getBoundingClientRect();
        if (r.right <= c.left || r.left >= c.right || r.bottom <= c.top || r.top >= c.bottom) clipped = true;
      }
    }
    if (scrollX || scrollY) scroll.unshift({ viewport: true, sx: r2(scrollX), sy: r2(scrollY) });
    const n = record(el, 0);
    return {
      ...tags(), id: nid, type: n.type, parent, props: n.props, style,
      // A development build names the element's plan node (emit.rs `data-site`).
      ...(el.dataset?.site != null ? { site: Number(el.dataset.site) } : {}),
      space: { viewport: rect(r), local: { w: r2(el.clientWidth), h: r2(el.clientHeight) }, capture: { scale: devicePixelRatio } },
      scroll, clip,
      visible: { hidden: el.checkVisibility ? !el.checkVisibility({ visibilityProperty: true }) : false, inert: !!el.closest('[inert]'), inViewport: r.right > 0 && r.bottom > 0 && r.left < innerWidth && r.top < innerHeight, clipped },
      native: { element: el.localName, ...(el.hasAttribute('data-symbol-source') ? { symbol: {
        source: el.dataset.symbolSource, name: el.dataset.symbolSource.slice(el.dataset.symbolSource.startsWith('symbol:sf/') ? 10 : 7), found: !!el.dataset.symbolPath,
        ...(!el.dataset.symbolPath ? { reason: el.dataset.symbolSource === 'symbol:sf/' ? 'empty' : el.dataset.symbolSource.startsWith('symbol:sf/') ? 'platform' : 'role' } : {}),
      } } : {}) },
      browser: Object.fromEntries(Object.entries(INHERITED).map(([row, prop]) => [row, cs.getPropertyValue(prop)])),
      observed: { clock: exact.clock.now, wall: Date.now() },
    };
  };
  // presence-glue.js `observation`, by this runtime's view ids (its elements carry no `data-view`).
  const presence = () => [...document.querySelectorAll('#exact-root [style*="--exact-layout-transition"], #exact-root [style*="--exact-exit-animation"], #exact-root [data-exiting]')].map(el => {
    const r = el.getBoundingClientRect();
    return { id: id(el), x: r.x, y: r.y, w: r.width, h: r.height, opacity: Number(getComputedStyle(el).opacity), exiting: el.hasAttribute('data-exiting') };
  });
  // The browser runs CSS animations; under the agent they follow its clock,
  // each from the clock time it began, author-paused ones keeping their own
  // (the web host's own `animationClock`), and `clock settle` runs the clock
  // to where the last one ends, as the wasm host's does.
  // (navigation.js's `animationClock`, restated; the build gives this module
  // its own copy of navigation.js, so it could now be imported.)
  const starts = new WeakMap(), held = new WeakSet();
  const anim = {
    register(t) { for (const a of document.getAnimations()) if (!starts.has(a)) { starts.set(a, t); if (a.playState === 'paused') held.add(a); } },
    seek(to, sync = true) {
      for (const a of document.getAnimations()) {
        const timing = a.effect?.getComputedTiming();
        if (!timing || held.has(a)) continue;
        const t = to - (starts.get(a) ?? exact.clock.now);
        if (t >= timing.endTime && timing.endTime !== Infinity) a.finish(); else { a.pause(); a.currentTime = t; }
      }
      if (sync) exact.synced?.();
    },
    settle() {
      let to = Math.max(exact.clock.now, exact.settleAt?.() ?? 0);
      for (const a of document.getAnimations()) {
        const timing = a.effect?.getComputedTiming();
        if (timing && timing.endTime !== Infinity && !held.has(a)) to = Math.max(to, (starts.get(a) ?? exact.clock.now) + timing.endTime);
      }
      return to;
    },
  };
  // Animated images held to the agent's clock (LLP 1011.000): the web host's
  // own `image-glue.js`, fetched at the first seek of a page with a GIF or
  // WebP (the only images it holds; a page without one does no work).
  let images = null;
  const holdImages = () => {
    if (!images && document.querySelector('#exact-root img[src*=".gif" i], #exact-root img[src*=".webp" i]')) images = import('./image-glue.js').then(() => exact.holdImages({ root: document.getElementById('exact-root'), now: () => exact.clock.now }));
    images?.then(h => h.seek());
  };
  const seek = (sync = true) => { anim.register(exact.clock.now); anim.seek(exact.clock.now, sync); holdImages(); };
  // motion.js owns post-commit reconciliation. The agent registers a
  // commit's new animations here, and reconciles only when its clock moves.
  exact.After.push(() => seek(false));
  // Held device requests (LLP 1069.007 D3): `openAuthSession`'s (auth.js).
  const holds = () => [...exact.auth?.holds() ?? [], ...exact.files?.holds() ?? []];
  // After a clock move, `exactTime` is answered again where its answer
  // changed: the offset at the new virtual instant, which a DST change
  // moves (LLP 1069.007 D2), as the wasm host's `exact_set_time` after `clock`.
  const retime = () => {
    const time = exact.data.reserved?.exactTime;
    const stale = time ? exact.resources.filter(r => r.source === 'exactTime' && !eq(r.value, time('exactTime', [], r.name))) : [];
    if (stale.length) exact.commit(() => { for (const r of stale) R(r); }, 'time');
  };
  const settleGpu = async () => await exact.gpu?.settled?.() ?? [];
  const gpuPendingReply = (req, pending) => {
    const names = pending.map(item => item.name ?? 'GPU work');
    return req.op === 'clock' ? { ...tags(), settled: false, reason: 'gpu', pending: names }
      : { error: `GPU is not settled: ${names.join(', ')}`, pending: names };
  };
  exact.agentSettled = async (req) => {
    await pieces();
    const beforeGpu = await settleGpu();
    if (beforeGpu.length) return gpuPendingReply(req, beforeGpu);
    // A read presents this clock without standing in for a commit hook.
    seek(false);
    // `tap @t <choice>` / `type @t <value>` answer a held request (D4).
    if ((req.op === 'tap' || req.op === 'type') && req.ticket != null) return (await exact.files?.answer(req)) ?? (exact.auth ? exact.auth.answer(req) : { error: `not pending: @${req.ticket}` });
    switch (req.op) {
      case 'tree': {
        let nodes = all(), roots = nodes.filter(n => n.depth === 0).map(n => n.id);
        if (req.target != null) {
          const hit = nodes.find(n => n.id === req.target || n.props.testId === req.target);
          if (!hit) return { error: `no view matches ${req.target}` };
          nodes = req.shallow ? [hit] : nodes.filter(n => n === hit || views.get(hit.id).contains(views.get(n.id)));
          roots = [hit.id];
        }
        return { nodes, roots, ...tags() };
      }
      case 'layout': {
        // Every view, a zero box too (an empty text, a closed popover), as
        // the wasm host's layout reports them.
        // The viewport, its safe-area environment and a port's scroll offsets, as glue.js's reply.
        const r2 = x => Math.round(x * 100) / 100;
        const nodes = all().map(n => {
          const el = views.get(n.id), b = viewBox(el);
          // An iframe says whether its centre hits it (glue.js).
          const hit = el instanceof HTMLIFrameElement ? { hit: document.elementFromPoint(b.left + b.width / 2, b.top + b.height / 2) === el } : {};
          return { id: n.id, x: b.x, y: b.y, w: b.width, h: b.height, ...hit, ...(el.dataset.scroll === 'true' ? { sx: r2(el.scrollLeft), sy: r2(el.scrollTop) } : {}) };
        });
        const reply = { viewport: { w: innerWidth, h: innerHeight }, env: environment(), nodes, ...tags() };
        if (req.id != null) { const node = nodeDetail(req.id); if (node.error) return node; reply.node = node; }
        if (req.agree) return { viewport: reply.viewport, agreement: { unavailable: 'no independent model: the page is the tree' }, ...tags() }; else if (req.native && reply.node) { delete reply.nodes; reply.node.native = { ...reply.node.native, subviews: { unavailable: 'the DOM is the tree; layout <target> names the element' } }; } // @ref LLP 1080.001 D1, D2
        return reply;
      }
      case 'focus': { const el = views.get(req.id); if (!el) return { error: `no view ${req.id}` }; el.focus(); if (req.select !== false) el.select?.(); return {}; }
      case 'tap':
        // A virtualized list's row brought into view by key (LLP 1070.000 §5; list.js).
        if (req.into) {
          if (!exact.lists) return { error: `view ${req.id} is not a mounted virtualized list` };
          try { exact.lists.into(req.id, String(req.into.key ?? ''), req.into.block, req.into.inline); } catch (e) { return { error: e.message }; }
          return { tapped: req.id, into: req.into };
        }
        // The browser's own traversal (LLP 1038 D11); popstate reaches the app.
        if (req.history) { history.go(req.history); await new Promise(r => setTimeout(r, 300)); return { history: req.history, delivery: 'platform' }; }
        {
          const el = views.get(req.id);
          if (el && (el.closest('[inert]') || ['hidden', 'collapse'].includes(getComputedStyle(el).visibility))) return { handled: true, error: `view ${req.id} is hidden or inert` };
          // A tap addressed to an iframe enters its guest (glue.js, LLP 1020 D4).
          return el instanceof HTMLIFrameElement ? guestTap(el, req) : {};
        }
      case 'type': { const el = views.get(req.id); return el instanceof HTMLIFrameElement ? guestType(el, req) : {}; }
      case 'logs': { const j = exact.journal, from = Math.max(req.since ?? 0, j.start); return { lines: j.slice(from - j.start), from, next: j.start + j.length }; }
      // `perf <target>` (LLP 1079 D2): the plan sites under a view, with their work (perf.js).
      case 'perf': {
        if (req.frames) return { virtual: true }; // the agent's clock presents no frame (LLP 1079 D4)
        let el = document.getElementById('exact-root');
        if (req.target != null) { const hit = all().find(n => n.id === req.target || n.props.testId === req.target); if (!hit) return { error: `no view matches ${req.target}` }; el = views.get(hit.id); }
        return perf.reply(el, tags());
      }
      case 'clock': {
        if (req.settle) {
          // Settled: no request in flight and no commit pending, within 20 s.
          // Virtualized lists report until a round sends nothing, reading
          // layout now (collection-glue.js `settle`, as glue.js's clock does).
          const end = performance.now() + 20000;
          for (let round = 0; round < 16; round++) {
            // A held request is in flight until the agent answers it: never waited on.
            do await new Promise(r => setTimeout(r, 30)); while (exact.inflight.n > holds().length && performance.now() < end);
            // Declared faces loading (the stylesheet's, LLP 1019) are the page's too.
            await document.fonts?.ready;
            // Text around shapes lays out in the frames after a commit (flow.js).
            await exact.flowSettle?.();
            if (exact.lists) exact.lists.settle();
            if (exact.inflight.n > holds().length) continue;
            // Animations (and springs, `settleAt`) that end later move the clock there; an
            // armed `then` runs now, and what it starts is settled in the next round.
            // What is in flight lands before the next timer or `then` fires, as in a
            // jump (below; glue.js's clock, the Linux agent's): the advance stops
            // after each commit that sends and waits for its reply, within this
            // round, so a run of sends never spends the rounds. Past the deadline,
            // or 4096 stops, the rest is one advance.
            const at = exact.clock.now, epoch = exact.clock.epoch, to = anim.settle();
            for (let stops = 0; ; stops++) {
              const before = exact.inflight.n, held = stops < 4096 && performance.now() < end;
              const stopped = exact.advance(to, false, held ? () => exact.inflight.n > before : undefined);
              if (typeof stopped === 'string') { retime(); seek(); return { error: `clock: ${stopped}`, clock: exact.clock.now }; }
              if (!stopped) break;
              while (exact.inflight.n > holds().length && performance.now() < end) await new Promise(r => setTimeout(r, 1));
            }
            if (!(to > at) && exact.clock.epoch === epoch) break;
            seek();
            await new Promise(r => requestAnimationFrame(() => r()));
          }
          retime();
          const gpuPending = await settleGpu();
          if (gpuPending.length) return gpuPendingReply(req, gpuPending);
          const waiting = holds();
          if (waiting.length) return { clock: exact.clock.now, settled: false, reason: 'device', tickets: waiting.map(h => h.ticket) };
          return { clock: exact.clock.now, settled: !exact.inflight.n };
        }
        // A jump that crosses timers stops after each timer whose commit
        // sends, and its reply lands before the next fires, as the wasm
        // host's does (Runner::advance_until_request): the runner keeps one
        // request per target, so the next tick's send would drop it.
        // A jump that fires timers which send nothing is one advance (one
        // journal line), as the runner's is.
        for (const end = performance.now() + 20000; ;) {
          const before = exact.inflight.n, stopped = exact.advance(req.to, false, () => exact.inflight.n > before);
          // A refusal stops the jump at its time: the runner's error (a timer's, a `then`'s).
          if (typeof stopped === 'string') { retime(); seek(); return { error: `clock: ${stopped}`, clock: exact.clock.now }; }
          if (!stopped) break;
          // A reply is usually a task or two away: poll at the browser's
          // shortest timer, not a frame's worth (a 300 ms timer's minute
          // is 200 of these).
          while (exact.inflight.n > holds().length && performance.now() < end) await new Promise(r => setTimeout(r, 1));
        }
        retime(); seek();
        await new Promise(r => requestAnimationFrame(() => r()));
        const gpuPending = await settleGpu();
        if (gpuPending.length) return gpuPendingReply(req, gpuPending);
        return { clock: exact.clock.now };
      }
      case 'tags': return tags();
      // @ref LLP 1080.002 D4 — the ids `tree` gives, where CDP's DOM snapshot reads them, and the document's nonce.
      case 'axStamp': { all(); for (const [i, el] of views) if (el.isConnected && el.getAttribute('data-agent-view') !== String(i)) el.setAttribute('data-agent-view', i); return { ...tags(), nonce: performance.timeOrigin }; }
      case 'state': {
        const [slots, derives, resources] = names.map((list, k) => Object.fromEntries(list.map((n, i) => [n, typed(exact.state[k][i](), types[k][i])])));
        // What is in flight: the network's by resource, then held device requests.
        const pending = [...exact.resources.filter(r => r.ticket).map(r => ({ name: r.name, ticket: r.ticket.id })), ...holds()];
        // The painted surface of views with presence rows, exit ghosts included (glue.js `st.presence`).
        // Focus, the keyboard's overlap and the document's language, as glue.js's `state` adds them.
        const active = document.activeElement && document.activeElement !== document.body ? document.activeElement : null, activeId = active ? id(active) : null;
        const overlap = Math.max(0, innerHeight - (globalThis.visualViewport?.height ?? innerHeight));
        const focus = { logical: activeId, editor: active && (active.localName === 'input' || active.localName === 'textarea' || active.exactMarkup) ? activeId : null, responder: active?.localName ?? null, pending: null };
        const language = { lang: document.documentElement.lang || 'en', dir: document.documentElement.dir || 'ltr' };
        const keyboard = { visible: overlap > 0, overlap: Math.round(overlap * 100) / 100, policy: document.querySelector('[interactiveWidget]')?.getAttribute('interactiveWidget') ?? 'resizes-visual', interactive: false };
        const media = [...document.querySelectorAll('#exact-root video')].map(el => ({ id: id(el), state: { currentTime: el.currentTime, duration: Number.isFinite(el.duration) ? el.duration : null, paused: el.paused, muted: el.muted, volume: el.volume, playbackRate: el.playbackRate, readyState: el.readyState, videoWidth: el.videoWidth, videoHeight: el.videoHeight, src: el.currentSrc, error: el.error ? { code: el.error.code, message: el.error.message } : null, renderer: 'HTMLVideoElement' } }));
        // The active head's fields, `null` where none is set, as the runner's `state.head` (agent.rs).
        const head = Object.fromEntries(['title', 'description', 'image', 'canonical', 'robots', 'status'].map(k => [k, Head['head' + k[0].toUpperCase() + k.slice(1)] ?? null]));
        return { slots, derives, resources, pending, head, focus, language, keyboard, navigation: (pageHistory() ?? navigation).observation(document.getElementById('exact-root')), media, window: { title: document.title }, ...(exact.canvas2dState ? { canvas: exact.canvas2dState() } : {}), ...(exact.surfaceRefusals ? { surfaceRefusals: exact.surfaceRefusals() } : {}), ...(exact.lists ? { scrollIntoView: exact.lists.intoView() } : {}), ...(exact.presenceLive ? { presence: presence() } : {}), ...(exact.hookStats ? { hooks: exact.hookStats } : {}), ...tags() };
      }
      // The page group (LLP 1069.000 D6), where the plan reads `exactPage` (facts.js).
      // The fold group (LLP 1078 D7) likewise: through facts.js where the plan reads the fold's fields (it re-answers them), else the
      // substitute lands here for `layout.env`; without a fold group the fold stays as it is.
      case 'prefer': try { return { page: exact.page ? exact.page.prefer(req.page ?? {}) : {}, fold: !req.fold ? foldEnv() : exact.fold ? exact.fold.prefer(req.fold) : preferFold(Object.keys(req.fold).length ? req.fold : null) }; } catch (e) { return { error: e.message }; }
      default: return { error: `${req.op} is not carried by the JS target` };
    }
  };
  return true;
}
