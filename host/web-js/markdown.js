// Built sources carry display pieces. New text and inline atoms load the
// parser; an unseen source reserves space until formatted content can be painted.
let Markdown = null, Ready = null, Atoms = null, Built = null;
const Pending = new Set();
function prepareMarkdown() {
  if (!Markdown) { Markdown = markdown().then(pieces => Ready = pieces, e => { Markdown = null; throw e; }); }
  return Markdown;
}
const key = (source, atoms, faces) => JSON.stringify([source, atoms, faces]);
function built() {
  if (Built) { return Built; }
  const data = globalThis.__exactMarkdownBuilt ?? JSON.parse(document.querySelector('script[type="application/vnd.exact.markdown"]')?.textContent ?? '[]');
  return Built = new Map(data.map(([source, atoms, faces, pieces]) => [key(source, atoms, faces), pieces]));
}
export async function compileReaders(paths, resources) {
  const parse = await prepareMarkdown();
  await Promise.allSettled([...Pending]);
  const out = new Map(globalThis.__exactMarkdown ?? []);
  for (const [index, ...path] of paths) {
    const r = resources[index];
    if (!r || r.store || r.failed || r.ticket || r.settled === undefined) { continue; }
    let value = r.value;
    for (const field of path) { value = value?.[field]; }
    if (typeof value === 'string') { out.set(key(value, '', ''), parse(value, '', '')); }
  }
  return [...out].map(([k, pieces]) => [...JSON.parse(k), pieces]);
}
export function markdownText(e, f, prepared, { effect, inflight, adopting, renderMarkup }) {
  let first = adopting() && e.childElementCount > 0 && !e.querySelector('[data-exact-atom]');
  let rendered = first, generation = 0;
  const draw = (v, pieces, k) => {
    if (e.$source !== v || key(v, e.getAttribute('data-atoms') ?? '', e.getAttribute('data-atom-faces') ?? '') !== k) { return; }
    if (!e.getAttribute('data-atoms')) { globalThis.__exactMarkdown?.set(k, pieces); }
    renderMarkup(e, pieces);
    rendered = true;
    e.removeAttribute('data-exact-markdown-pending');
  };
  const render = v => {
    const current = ++generation;
    const atoms = e.getAttribute('data-atoms') ?? '', faces = e.getAttribute('data-atom-faces') ?? '';
    const dynamic = prepared === false;
    const k = key(v, atoms, faces), pieces = dynamic || atoms ? undefined : prepared && v === prepared[0] ? prepared[1] : built().get(k);
    if (pieces !== undefined) { draw(v, pieces, k); return; }
    if (Ready && !atoms && !dynamic) { draw(v, Ready(v, atoms, faces), k); return; }
    if (!rendered) {
      e.setAttribute('data-exact-markdown-pending', '');
      e.textContent = v;
    }
    inflight.n++;
    const job = Promise.all([prepareMarkdown(), (dynamic || atoms) && (Atoms ??= import('./atoms.js'))])
      .then(([parse]) => {
        if (current !== generation || e.$source !== v) { return; }
        const atoms = e.getAttribute('data-atoms') ?? '', faces = e.getAttribute('data-atom-faces') ?? '';
        draw(v, parse(v, atoms, faces), key(v, atoms, faces));
      })
      .catch(() => {
        if (current !== generation || e.$source !== v) { return; }
        e.textContent = v;
        e.removeAttribute('data-exact-markdown-pending');
        rendered = false;
      })
      .finally(() => { inflight.n--; Pending.delete(job); });
    Pending.add(job);
    return job;
  };
  if (typeof MutationObserver === 'function') {
    new MutationObserver(() => render(e.$source))
      .observe(e, { attributes: true, attributeFilter: ['data-atoms', 'data-atom-faces', 'data-atom-styles'] });
  }
  effect(() => {
    const v = f() ?? '';
    e.$source = v;
    if (first) { first = false; return; }
    render(v);
  });
}
async function markdown() {
  const bytes = globalThis.__files ? globalThis.__files('markdown.wasm') : await fetch('./markdown.wasm').then(r => {
    if (!r.ok) { throw new Error(`Markdown parser download failed (HTTP ${r.status}); changing the reader text retries the request.`); }
    return r.arrayBuffer();
  });
  const made = await WebAssembly.instantiate(bytes, {}), x = (made.instance ?? made).exports;
  return (source, atoms, faces) => {
    const enc = new TextEncoder(), s = enc.encode(source), a = enc.encode(atoms), f = enc.encode(faces), b = new Uint8Array(s.length + a.length + f.length), p = x.alloc(b.length);
    b.set(s); b.set(a, s.length); b.set(f, s.length + a.length);
    new Uint8Array(x.memory.buffer, p, b.length).set(b);
    const n = x.pieces(p, b.length, a.length, f.length);
    return new TextDecoder().decode(new Uint8Array(x.memory.buffer, x.output(), n));
  };
}
