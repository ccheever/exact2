// Text around shapes on the JS target (LLP 1043.000 §3): the web host's own
// exclusions executor, `textflow-glue.js` over `textflow.wasm`, unchanged,
// fetched after first paint by a plan with a `wrap-flow` row. This file is
// the runner's half the wasm host keeps in Rust: which nodes are exclusions
// and the contexts they wrap (host/web/src/flow_host.rs `emit_textflow`),
// and the kernel's structural admission rule for an auto-height paragraph
// (kernel/src/flow.rs `structural_refusal`), read from the page's styles.
import { createTextFlow, createFlowRequest } from './textflow-glue.js';

// kernel FlowRefusal::message, for the journal line the executor writes.
const REFUSAL = {
  context: 'its wrapping context (the exclusion\'s parent) is flex or grid, or sets align-content; make that parent display: block, or give the text a height',
  placement: 'an exclusion\'s top or height depends on its context\'s height; give every exclusion there a top and a height in points (not %, not bottom), or give the text a height',
  chain: 'the text, or a box between it and the wrapping context, is absolutely positioned, flex or grid, sets align-content, or has a percentage top or bottom; keep ordinary in-flow blocks between them, or give the text a height',
};

export async function flow({ views, viewId, wraps, clock, wall, say }) {
  const controller = createTextFlow({ views, request: await createFlowRequest(), agentMode: clock.agent, log: say,
    now: () => clock.agent ? clock.now : Math.max(clock.now, wall()), advance: () => {} });
  const cs = el => getComputedStyle(el);
  // An authored length that resolves without the containing block's height
  // (kernel `own_length`): points, not a percentage, not `auto`. The
  // computed value (`computedStyleMap`) keeps a percentage as authored.
  const value = (el, p) => el.computedStyleMap?.().get(p);
  const own = (el, p) => { const v = value(el, p); return v?.unit === 'px' || v?.unit === 'number'; };
  const auto = (el, p) => { const v = value(el, p); return v == null || v.value === 'auto' || v.value === 'none'; };
  const orders = el => { const s = cs(el); return s.display === 'block' && s.alignContent === 'normal'; };
  const inFlow = el => ['relative', 'static'].includes(cs(el).position) && ['top', 'bottom'].every(p => auto(el, p) || own(el, p));
  const placed = el => own(el, 'top') && (own(el, 'height') || auto(el, 'height') && auto(el, 'bottom'))
    && ['min-height', 'max-height'].every(p => auto(el, p) || own(el, p));
  // A text's authored height (the kernel's `height != auto`): its inline
  // style (a dynamic row) or its class's rule.
  const rules = new Map();
  const classHeight = el => {
    const c = el.getAttribute('class');
    if (!c) return '';
    if (!rules.has(c)) {
      let h = '';
      for (const sheet of document.styleSheets) for (const r of sheet.cssRules ?? []) if (r.selectorText === '.' + c) h = r.style.height || h;
      rules.set(c, h);
    }
    return rules.get(c);
  };
  const definite = el => !!(el.style.height || classHeight(el)) && !/^auto$/.test(el.style.height || classHeight(el));
  const runs = el => el.parentElement?.hasAttribute('data-exact-text');
  let last = '';
  // The contexts after a commit, as the wasm host publishes them.
  function contexts() {
    const exclusions = [...wraps].filter(e => e.isConnected && e.$wrap === 'both' && cs(e).position === 'absolute' && e.getClientRects().length);
    const by = new Map();
    for (const e of exclusions) if (e.parentElement) (by.get(e.parentElement) ?? by.set(e.parentElement, []).get(e.parentElement)).push(e);
    const refusals = new Map([...by].map(([c, list]) => [c, !orders(c) ? 'context' : list.every(placed) ? null : 'placement']));
    const refusal = leaf => {
      let chain = inFlow(leaf);
      for (let at = leaf.parentElement; at; at = at.parentElement) {
        if (refusals.has(at)) { if (refusals.get(at)) return REFUSAL[refusals.get(at)]; if (!chain) return REFUSAL.chain; }
        chain &&= inFlow(at) && orders(at);
      }
      return null;
    };
    const out = [];
    for (const [c, list] of by) {
      const paragraphs = [], stack = [...c.children].reverse(), skip = new Set(list);
      while (stack.length) {
        const el = stack.pop();
        if (skip.has(el) || el.hasAttribute('data-surface') || cs(el).display === 'none') continue;
        if (el.hasAttribute('data-exact-text') && !runs(el)) paragraphs.push({ id: viewId(el), definite: definite(el), refusal: definite(el) ? null : refusal(el) });
        else stack.push(...[...el.children].reverse());
      }
      out.push({ id: viewId(c), exclusions: list.map(viewId), paragraphs });
    }
    return out;
  }
  return {
    // Before a commit touches the tree: flowed paragraphs back to their text
    // (the executor's `beforeBatch` on a topology change).
    before() { controller.beforeBatch({ ops: [{ op: 'children' }] }); },
    // After it: the contexts, and a new layout of every paragraph.
    after() {
      const c = contexts(), text = JSON.stringify(c), ops = [{ op: 'children' }];
      // Each paragraph's own text and children, before fragments replace
      // them: the tree the agent reads (the runner's, not the fragments).
      for (const p of c.flatMap(x => x.paragraphs)) {
        const el = views.get(p.id);
        if (el && !el.querySelector(':scope > [data-flow-fragment]')) el.$flow = { text: el.textContent, kids: [...el.children] };
      }
      if (text !== last) { last = text; ops.push({ op: 'textflow', contexts: c }); }
      controller.afterBatch({ ops });
    },
    settle: () => controller.settle(),
  };
}
