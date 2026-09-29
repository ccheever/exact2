// The agent's door into a JS-target page (LLP 1012's operations, as far as
// the JS target carries them): `exact.agentSettled(request)` answers what
// `scripts/agent.mjs web` asks. Input and screenshots stay the carrier's own
// (CDP). Loaded only under `?agent`; never part of an app's boot bytes.
import names, { types } from './names.js';
// A runtime value as the runner's typed JSON: records by field name.
const typed = (v, t) => v == null || typeof t === 'string' ? v : Array.isArray(t) ? (t[0] === '?' ? typed(v, t[1]) : v.map(x => typed(x, t[1]))) : Object.fromEntries(Object.keys(t).map((k, i) => [k, typed(v[i], t[k])]));
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
  const type = el => el.namespaceURI === 'http://www.w3.org/2000/svg' ? svg(el) : el.exactMarkup ? 'TextInput' : el.hasAttribute('data-exact-text') || run(el) ? 'Text' : el.querySelector(':scope > canvas[data-surface]') ? 'Canvas' : el.dataset.scroll ? (el.getAttribute('role') === 'list' ? 'List' : 'ScrollView') : TYPES[el.tagName] ?? 'View';
  const record = (el, depth) => {
    const props = {};
    if (el.dataset.testid) props.testId = el.dataset.testid;
    if (el.hasAttribute('aria-label')) props.accessibilityLabel = el.getAttribute('aria-label');
    else if (el.tagName === 'IMG' && el.getAttribute('alt')) props.accessibilityLabel = el.getAttribute('alt');
    // A paragraph of runs has no text of its own: its runs carry it.
    if (/^(Text|SvgText|SvgTSpan)$/.test(type(el)) && (el.$source != null || !kids(el).length)) props.text = el.$source ?? (flowed(el) ? el.$flow.text : el.textContent);
    if ('value' in el && el.tagName !== 'BUTTON') props.value = el.value;
    const n = { id: id(el), type: type(el), depth, props };
    if (el.dataset.exactOn) n.handlers = el.dataset.exactOn.split(' ');
    if (document.activeElement === el) n.focused = true;
    return n;
  };
  const all = () => {
    const out = [];
    const walk = (el, d) => { const n = record(el, d); out.push(n); n.children = kids(el).map(c => walk(c, d + 1).id); return n; };
    kids(document.getElementById('exact-root')).forEach(el => walk(el, 0));
    return out;
  };
  const tags = () => ({ clock: exact.clock.now, epoch: 1 });
  // presence-glue.js `observation`, by this runtime's view ids (its elements carry no `data-view`).
  const presence = () => [...document.querySelectorAll('#exact-root [style*="--exact-layout-transition"], #exact-root [style*="--exact-exit-animation"], #exact-root [data-exiting]')].map(el => {
    const r = el.getBoundingClientRect();
    return { id: id(el), x: r.x, y: r.y, w: r.width, h: r.height, opacity: Number(getComputedStyle(el).opacity), exiting: el.hasAttribute('data-exiting') };
  });
  // The browser runs CSS animations; under the agent they follow its clock,
  // each from the clock time it began, author-paused ones keeping their own
  // (the web host's own `animationClock`), and `clock settle` runs the clock
  // to where the last one ends, as the wasm host's does.
  // (navigation.js's `animationClock`, restated: importing it would pull it
  // into every app's module, since rt.js imports navigation.js.)
  const starts = new WeakMap(), held = new WeakSet();
  const anim = {
    register(t) { for (const a of document.getAnimations()) if (!starts.has(a)) { starts.set(a, t); if (a.playState === 'paused') held.add(a); } },
    seek(to) {
      for (const a of document.getAnimations()) {
        const timing = a.effect?.getComputedTiming();
        if (!timing || held.has(a)) continue;
        const t = to - (starts.get(a) ?? exact.clock.now);
        if (t >= timing.endTime && timing.endTime !== Infinity) a.finish(); else { a.pause(); a.currentTime = t; }
      }
      exact.synced?.();
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
  const seek = () => { anim.register(exact.clock.now); anim.seek(exact.clock.now); };
  exact.After.push(seek);
  // Held device requests (LLP 1069.007 D3): `openAuthSession`'s (auth.js).
  const holds = () => exact.auth?.holds() ?? [];
  exact.agentSettled = async (req) => {
    seek();
    // `tap @t <choice>` / `type @t <value>` answer a held request (D4).
    if ((req.op === 'tap' || req.op === 'type') && req.ticket != null) return exact.auth ? exact.auth.answer(req) : { error: `not pending: @${req.ticket}` };
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
        const nodes = all().map(n => { const b = views.get(n.id).getBoundingClientRect(); return { id: n.id, x: b.x, y: b.y, w: b.width, h: b.height }; }).filter(n => n.w || n.h);
        return { nodes, ...tags() };
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
        return {};
      case 'type': return {};
      case 'logs': { const from = req.since ?? 0; return { lines: exact.journal.slice(from), from, next: exact.journal.length }; }
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
            // Animations (and springs, `settleAt`) that end later move the clock there.
            const to = anim.settle();
            if (!(to > exact.clock.now)) break;
            exact.advance(to); seek();
            await new Promise(r => requestAnimationFrame(() => r()));
          }
          const waiting = holds();
          if (waiting.length) return { clock: exact.clock.now, settled: false, reason: 'device', tickets: waiting.map(h => h.ticket) };
          return { clock: exact.clock.now, settled: !exact.inflight.n };
        }
        // A jump that crosses timers stops after each timer whose commit
        // sends, and its reply lands before the next fires, as the wasm
        // host's does (Runner::advance_until_request): the runner keeps one
        // request per target, so the next tick's send would drop it.
        for (const end = performance.now() + 20000; ;) {
          const next = Math.min(...exact.clock.timers.map(t => t.due));
          if (!(next <= req.to)) break;
          exact.advance(next);
          while (exact.inflight.n > holds().length && performance.now() < end) await new Promise(r => setTimeout(r, 15));
        }
        exact.advance(req.to); seek();
        await new Promise(r => requestAnimationFrame(() => r()));
        return { clock: exact.clock.now };
      }
      case 'tags': return tags();
      case 'state': {
        const [slots, derives, resources] = names.map((list, k) => Object.fromEntries(list.map((n, i) => [n, typed(exact.state[k][i](), types[k][i])])));
        // What is in flight: the network's by resource, then held device requests.
        const pending = [...exact.resources.filter(r => r.ticket).map(r => ({ name: r.name, ticket: r.ticket.id })), ...holds()];
        // The painted surface of views with presence rows, exit ghosts included (glue.js `st.presence`).
        return { slots, derives, resources, pending, ...(exact.lists ? { scrollIntoView: exact.lists.intoView() } : {}), ...(exact.presenceLive ? { presence: presence() } : {}), ...tags() };
      }
      // The page group (LLP 1069.000 D6), where the plan reads `exactPage` (facts.js).
      case 'prefer': try { return { page: exact.page ? exact.page.prefer(req.page ?? {}) : {} }; } catch (e) { return { error: e.message }; }
      default: return { error: `${req.op} is not carried by the JS target` };
    }
  };
  return true;
}
