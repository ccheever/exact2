// Logical text selection over a virtualized list's rows, mounted or not;
// navigation.js reports the list's geometry. Loaded after paint only for
// lists; ordinary selection stays native.
globalThis.exact.installListSelection = function ({ root, lists, index, text }) {
  let frame = null;
  function schedule() {
    if (frame === null && lists.size) frame = requestAnimationFrame(flush);
  }
  function flush() {
    frame = null;
    readSelection();
  }
  function sync() { schedule(); }
  let selected = null, lastList = null, applying = false, nativeStamp = [], selectionPending = false;
  const rowSelector = '[data-listitemkey]', paragraphSelector = '[data-exact-text]';
  const style = document.createElement('style');
  style.textContent = '::highlight(exact-list-selection){background:Highlight;color:HighlightText}';
  document.head.append(style);
  const element = node => node?.nodeType === Node.ELEMENT_NODE ? node : node?.parentElement;
  const editor = target => element(target)?.closest('input,textarea,select,[contenteditable]:not([contenteditable="false"])');
  function owner(node) {
    for (let el = element(node); el && el !== root; el = el.parentElement) if (lists.has(el)) return el;
    return null;
  }
  function paragraphs(row) {
    return [...row.querySelectorAll(paragraphSelector)].filter(el => {
      for (let p = el; p && p !== row; p = p.parentElement) if (getComputedStyle(p).display === 'none') return false;
      return true;
    });
  }
  function position(node, offset) {
    const p = element(node)?.closest(paragraphSelector), row = p?.closest(rowSelector), list = owner(row);
    if (!p || !row || !list) return null;
    const range = document.createRange();
    range.selectNodeContents(p);
    try { range.setEnd(node, offset); } catch { return null; }
    return { list, key: row.dataset.listitemkey, row: Number(row.getAttribute('aria-posinset')) - 1,
      paragraph: paragraphs(row).indexOf(p), offset: range.toString().length };
  }
  // Selection.toString() asks for rendered text and forces layout even for a
  // collapsed selection. Endpoint identity/offsets detect native changes without
  // reading geometry, including distinct text nodes inside the same inline run.
  const stamp = s => s ? [s.anchorNode, s.anchorOffset, s.focusNode, s.focusOffset] : [];
  function capture() {
    const s = getSelection();
    if (!s || s.isCollapsed) return;
    const a = position(s.anchorNode, s.anchorOffset), b = position(s.focusNode, s.focusOffset);
    if (a && b && a.list === b.list) selected = { list: a.list, a, b, all: false };
  }
  function clear() { selected = null; CSS.highlights?.delete('exact-list-selection'); }
  function compare(a, b) { return a.row - b.row || a.paragraph - b.paragraph || a.offset - b.offset; }
  function endpoint(p, offset) {
    const walk = document.createTreeWalker(p, NodeFilter.SHOW_TEXT);
    let n;
    while ((n = walk.nextNode())) { if (offset <= n.length) return [n, offset]; offset -= n.length; }
    return [p, p.childNodes.length];
  }
  function paint() {
    if (!selected) return;
    const { list, a, b, all } = selected;
    if (!list.isConnected || !lists.has(list)) { clear(); return; }
    a.row = index(list, a.key); b.row = index(list, b.key);
    if (a.row === 0xffffffff || b.row === 0xffffffff) { clear(); return; }
    const [lo, hi] = compare(a, b) <= 0 ? [a, b] : [b, a], ranges = [];
    for (const row of list.querySelectorAll(rowSelector)) {
      const ordinal = Number(row.getAttribute('aria-posinset')) - 1;
      for (const [i, p] of paragraphs(row).entries()) {
        const at = { row: ordinal, paragraph: i, offset: 0 };
        const end = { ...at, offset: p.textContent.length };
        if (!all && (compare(end, lo) < 0 || compare(at, hi) > 0)) continue;
        const startOffset = !all && ordinal === lo.row && i === lo.paragraph ? lo.offset : 0;
        const endOffset = !all && ordinal === hi.row && i === hi.paragraph ? hi.offset : end.offset;
        if (endOffset <= startOffset) continue;
        const r = document.createRange();
        r.setStart(...endpoint(p, startOffset)); r.setEnd(...endpoint(p, endOffset)); ranges.push(r);
      }
    }
    if (globalThis.Highlight && CSS.highlights) CSS.highlights.set('exact-list-selection', new Highlight(...ranges));
  }
  function readSelection() {
    if (!selectionPending) return;
    // Even reading Selection.anchorNode can flush browser layout. Reconcile
    // only after layout, never inside the batch's write phase.
    nativeStamp = stamp(getSelection()); applying = false;
    selectionPending = false; paint();
  }
  document.addEventListener('selectionchange', () => {
    if (applying || stamp(getSelection()).every((value, i) => value === nativeStamp[i])) return;
    capture(); paint();
  });
  root.addEventListener('pointerdown', event => {
    applying = false; selectionPending = false;
    lastList = owner(event.target);
    if (!event.shiftKey) clear();
  }, true);
  document.addEventListener('keydown', event => {
    applying = false; selectionPending = false;
    if (editor(event.target)) return;
    if (event.key === 'Escape') { clear(); return; }
    if (event.key.toLowerCase() !== 'a' || !(event.metaKey || event.ctrlKey) || event.altKey) return;
    const list = owner(getSelection()?.anchorNode) ?? (lastList?.isConnected ? lastList : null)
      ?? (lists.size === 1 ? lists.keys().next().value : null);
    if (!list) return;
    const rows = [...list.querySelectorAll(rowSelector)];
    if (!rows.length) return;
    const point = row => ({ list, key: row.dataset.listitemkey, row: 0, paragraph: 0, offset: 0 });
    selected = { list, a: point(rows[0]), b: point(rows.at(-1)), all: true };
    event.preventDefault(); getSelection().removeAllRanges(); nativeStamp = stamp(getSelection()); paint();
  });
  document.addEventListener('copy', event => {
    if (editor(event.target) || !selected || !event.clipboardData) return;
    paint();
    if (!selected) return;
    const value = text(selected.list, selected.all ? null : selected.a, selected.all ? null : selected.b);
    event.clipboardData.setData('text/plain', value); event.preventDefault();
  });
  return {
    sync,
    forget(el) { lists.get(el)?.observer?.disconnect(); },
    before() { if (!applying) capture(); applying = true; },
    after() {
      if (!lists.size) { clear(); applying = false; selectionPending = false; nativeStamp = []; return; }
      // Read selection/visibility at observer delivery or the next frame's
      // read phase, before the next batch can dirty layout again.
      selectionPending = true; schedule();
    },
  };
};
