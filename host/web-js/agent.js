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
  const kids = el => el.getAttribute('markup') === 'markdown' ? [] : [...el.children].filter(c => !c.hasAttribute('data-surface'));
  const type = el => el.hasAttribute('data-exact-text') ? 'Text' : el.querySelector(':scope > canvas[data-surface]') ? 'Canvas' : el.dataset.scroll ? 'ScrollView' : TYPES[el.tagName] ?? 'View';
  const record = (el, depth) => {
    const props = {};
    if (el.dataset.testid) props.testId = el.dataset.testid;
    if (el.hasAttribute('aria-label')) props.accessibilityLabel = el.getAttribute('aria-label');
    else if (el.tagName === 'IMG' && el.getAttribute('alt')) props.accessibilityLabel = el.getAttribute('alt');
    if (type(el) === 'Text') props.text = el.$source ?? el.textContent;
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
  // The browser runs CSS animations; under the agent they follow its clock:
  // each held at the driver's time, seeked on every read.
  const seek = () => { for (const a of document.getAnimations()) { a.pause(); a.currentTime = exact.clock.now; } };
  exact.agentSettled = async (req) => {
    seek();
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
        // The browser's own traversal (LLP 1038 D11); popstate reaches the app.
        if (req.history) { history.go(req.history); await new Promise(r => setTimeout(r, 300)); return { history: req.history, delivery: 'platform' }; }
        return {};
      case 'type': return {};
      case 'logs': { const from = req.since ?? 0; return { lines: exact.journal.slice(from), from, next: exact.journal.length }; }
      case 'clock': {
        if (req.settle) {
          // Settled: no request in flight and no commit pending, within 20 s.
          const end = performance.now() + 20000;
          do await new Promise(r => setTimeout(r, 30)); while (exact.inflight.n && performance.now() < end);
          // Declared faces loading (the stylesheet's, LLP 1019) are the page's too.
          await document.fonts?.ready;
          return { clock: exact.clock.now, settled: !exact.inflight.n };
        }
        exact.advance(req.to);
        await new Promise(r => requestAnimationFrame(() => r()));
        return { clock: exact.clock.now };
      }
      case 'tags': return tags();
      case 'state': {
        const [slots, derives, resources] = names.map((list, k) => Object.fromEntries(list.map((n, i) => [n, typed(exact.state[k][i](), types[k][i])])));
        return { slots, derives, resources, ...tags() };
      }
      case 'prefer': return { page: {} };
      default: return { error: `${req.op} is not carried by the JS target` };
    }
  };
  return true;
}
