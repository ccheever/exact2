// Incremental structural facts on the server DOM use the same executor as
// the browser. These holders intentionally have no data-exact-box marker.
import { test, expect } from 'bun:test';
import { createDocument } from '../../web-js/dom.js';
import { paintList, paintFacts, paintFlush, paintWait } from '../../web-js/paint.js';

function tree() {
  const doc = createDocument();
  const add = (p, kind = 'box', facts = {}) => {
    const e = doc.createElement('div');
    e.setAttribute('data-exact-kind', kind);
    for (const [k, v] of Object.entries(facts)) e.setAttribute('data-exact-' + k, v);
    p.append(e); paintList(e);
    return e;
  };
  const root = add(doc.root, 'box', { root: '' });
  return { doc, add, root };
}

test('a nested z insertion and removal propagates through unmarked lists to the root', () => {
  const { doc, root, add } = tree();
  const outer = add(root), holder = add(outer), follower = add(root);
  paintFlush();
  expect(holder.style.isolation).toBe('');
  const z = add(holder, 'box', { position: 'absolute', zi: '-2' });
  paintFlush();
  expect(holder.style.isolation).toBe('isolate');
  expect(outer.style.isolation).toBe('');
  expect(follower.style.isolation).toBe('isolate');
  const html = doc.rootHTML(); paintFlush(); expect(doc.rootHTML()).toBe(html);
  z.remove(); paintList(holder); paintFlush();
  expect(holder.style.isolation).toBe('');
  expect(follower.style.isolation).toBe('');
});

test('layout-transition and text-flow policy come from the actual children and siblings', () => {
  const { root, add } = tree();
  const holder = add(root), paragraph = add(holder, 'text');
  const child = add(holder, 'box', { layout: '', position: 'absolute', wrap: '' });
  paintFlush();
  expect(holder.hasAttribute('data-exact-policy')).toBe(true);
  expect(paragraph.style.isolation).toBe('isolate');
  child.remove(); paintList(holder); paintFlush();
  expect(holder.style.isolation).toBe('');
  expect(paragraph.style.isolation).toBe('');
});

test('applicability keeps authored z and responds to parent display and position', () => {
  const { root, add } = tree();
  const holder = add(root), item = add(holder, 'box', { zi: '-4' });
  paintFlush(); expect(holder.style.isolation).toBe('');
  holder.setAttribute('data-exact-flex', ''); paintFacts(holder); paintFlush();
  expect(holder.style.isolation).toBe('isolate');
  holder.removeAttribute('data-exact-flex'); paintFacts(holder); paintFlush();
  expect(holder.style.isolation).toBe('');
  expect(item.getAttribute('data-exact-zi')).toBe('-4');
  item.setAttribute('data-exact-position', 'relative'); paintFacts(item); paintFlush();
  expect(holder.style.isolation).toBe('isolate');
});

test('host policy is separate from authored isolation and outside boxes contribute nothing', () => {
  const { root, add } = tree();
  const button = add(root, 'control', { type: 'button', 'button-style': 'plain', disabled: 'true' });
  const canvas = add(root, 'canvas'), image = add(root, 'image', { source: 'symbol:star' });
  const holder = add(root), dialog = add(holder, 'box', { semantic: 'dialog', position: 'relative', zi: '5' });
  paintFlush();
  for (const e of [button, canvas, image]) {
    expect(e.style.isolation).toBe('isolate');
    expect(e.hasAttribute('data-exact-own-isolation')).toBe(false);
  }
  expect(holder.style.isolation).toBe('');
  button.setAttribute('data-exact-button-style', 'gray'); paintFacts(button);
  image.removeAttribute('data-exact-source'); paintFacts(image);
  dialog.removeAttribute('data-exact-semantic'); paintFacts(dialog); paintFlush();
  expect(button.style.isolation).toBe('');
  expect(image.style.isolation).toBe('');
  expect(holder.style.isolation).toBe('isolate');
});

test('waiting server rows retain layout and exclusion facts for their adopted relatives', () => {
  const { root, add } = tree();
  const holder = add(root), paragraph = add(holder, 'text');
  const waiting = add(holder, 'box', { layout: '', position: 'absolute', wrap: '' });
  globalThis.__exactRender = true;
  try { paintFlush(); } finally { delete globalThis.__exactRender; }
  paintWait(waiting);
  // The Rust document has the paint summary before template attributes arrive.
  for (const fact of ['layout', 'position', 'wrap', 'kind']) waiting.removeAttribute('data-exact-' + fact);
  paintFacts(holder); paintFlush();
  expect(holder.style.isolation).toBe('isolate');
  expect(paragraph.style.isolation).toBe('isolate');
});
