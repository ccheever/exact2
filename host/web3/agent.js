// The agent's door into an exact3 page (LLP 1012's operations, as far as
// the spike carries them): `exact.agentSettled(request)` answers what
// `scripts/agent.mjs web` asks. Input and screenshots stay the carrier's own
// (CDP). Loaded only under `?agent`; never part of an app's boot bytes.
const TYPES = { BUTTON: 'Pressable', INPUT: 'TextInput', TEXTAREA: 'TextInput', VIDEO: 'Video', IMG: 'Image', IFRAME: 'WebView', A: 'Pressable' };
export function install(exact) {
  const ids = new WeakMap(), views = new Map();
  let next = 1;
  const id = el => { let i = ids.get(el); if (!i) { i = next++; ids.set(el, i); } views.set(i, el); return i; };
  const kids = el => [...el.children].filter(c => !c.hasAttribute('data-surface'));
  const type = el => el.hasAttribute('data-exact-text') ? 'Text' : el.querySelector(':scope > canvas[data-surface]') ? 'Canvas' : el.dataset.scroll ? 'ScrollView' : TYPES[el.tagName] ?? 'View';
  const record = el => {
    const props = {};
    if (el.dataset.testid) props.testId = el.dataset.testid;
    if (el.hasAttribute('aria-label')) props.accessibilityLabel = el.getAttribute('aria-label');
    if (type(el) === 'Text') props.text = el.textContent;
    if ('value' in el && el.tagName !== 'BUTTON') props.value = el.value;
    return { id: id(el), type: type(el), props, children: kids(el).map(id) };
  };
  const all = () => { const out = []; const walk = el => { out.push(record(el)); kids(el).forEach(walk); }; kids(document.getElementById('exact-root')).forEach(walk); return out; };
  const tags = () => ({ clock: exact.clock.now, epoch: 1 });
  exact.views = views;
  exact.agentSettled = async (req) => {
    switch (req.op) {
      case 'tree': {
        let nodes = all();
        if (req.target != null) {
          const hit = nodes.find(n => n.id === req.target || n.props.testId === req.target);
          if (!hit) return { error: `no view matches ${req.target}` };
          nodes = req.shallow ? [hit] : nodes.filter(n => n === hit || views.get(hit.id).contains(views.get(n.id)));
        }
        return { nodes, ...tags() };
      }
      case 'layout': {
        const nodes = all().map(n => { const b = views.get(n.id).getBoundingClientRect(); return { id: n.id, x: b.x, y: b.y, w: b.width, h: b.height }; }).filter(n => n.w || n.h);
        return { nodes, ...tags() };
      }
      case 'focus': { const el = views.get(req.id); if (!el) return { error: `no view ${req.id}` }; el.focus(); if (req.select !== false) el.select?.(); return {}; }
      case 'tap': case 'type': return {};
      case 'logs': { const from = req.since ?? 0; return { lines: exact.journal.slice(from), from, next: exact.journal.length }; }
      case 'clock': {
        if (req.settle) { await new Promise(r => setTimeout(r, 50)); return { clock: exact.clock.now, settled: true }; }
        exact.advance(req.to);
        await new Promise(r => requestAnimationFrame(() => r()));
        return { clock: exact.clock.now };
      }
      case 'tags': return tags();
      case 'state': return { note: 'the spike does not name state for the agent', ...tags() };
      case 'prefer': return { page: {} };
      default: return { error: `${req.op} is not carried by the exact3 spike` };
    }
  };
  return true;
}
