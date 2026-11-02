// A standalone live `time` on the wasm page (docs/reference.md, Live time):
// its box reserved at its font for every string the runner names in
// `data-time-reservation` (one per line), as the JS target's live-time.js
// and the native hosts' kernel do, and its formatted text kept as the node
// before its children, which stay as the fallback (index.html hides them
// while it formats). Loaded after first paint, the first time a page has one.

// As host/web-js/live-time.js copies them onto its measure.
const fontKeys = ['fontFamily', 'fontSize', 'fontWeight', 'fontStyle', 'fontStretch', 'lineHeight', 'fontSizeAdjust', 'fontVariantCaps', 'fontVariantLigatures', 'fontVariantPosition', 'fontVariantNumeric', 'fontVariantEastAsian', 'fontVariantAlternates', 'fontVariantEmoji', 'fontFeatureSettings', 'fontVariationSettings', 'letterSpacing', 'wordSpacing', 'fontKerning', 'fontOpticalSizing', 'textTransform', 'writingMode', 'textOrientation'];

/** The widest of `strings` at `el`'s font, along its inline axis. */
function widest(el, strings, font) {
  const measure = document.createElement('span');
  Object.assign(measure.style, { position: 'fixed', visibility: 'hidden', pointerEvents: 'none', whiteSpace: 'pre', inlineSize: 'max-content', blockSize: 'max-content' });
  for (const key of fontKeys) if (font[key]) measure.style[key] = font[key];
  for (const text of strings) { const line = document.createElement('span'); line.style.display = 'block'; line.textContent = text; measure.append(line); }
  document.body.append(measure);
  const rect = measure.getBoundingClientRect();
  measure.remove();
  return /^(vertical|sideways)/.test(font.writingMode) ? rect.height : rect.width;
}

function install(el) {
  if (el.exactLiveTime) { el.exactLiveTime.sync(); return; }
  let reserved = '', width = '';
  // Inside a paragraph, at any depth of runs, it is an inline run: no reservation (as the JS target compiles it).
  const inline = () => !!el.parentElement?.closest('[data-exact-text]');
  function sync() {
    const values = el.getAttribute('data-time-reservation');
    // An authored width or maximum is the box (as the JS target compiles no reservation under one).
    const sized = el.style.width || el.style.maxWidth || el.style.inlineSize || el.style.maxInlineSize;
    if (values == null || inline() || sized || !el.isConnected) {
      if (width && el.style.minInlineSize === width) el.style.minInlineSize = '';
      reserved = width = ''; return;
    }
    const font = getComputedStyle(el), stamp = `${values}\n${fontKeys.map(k => font[k]).join('/')}`;
    if (stamp !== reserved) { width = `${Math.ceil(widest(el, values.split('\n'), font))}px`; reserved = stamp; }
    // A style write (a batch, a motion frame) replaces the inline style.
    if (el.style.minInlineSize !== width) el.style.minInlineSize = width;
  }
  function text(value) {
    let node = el.firstChild;
    if (value == null) { if (node?.nodeType === Node.TEXT_NODE) node.remove(); return; }
    if (node?.nodeType !== Node.TEXT_NODE) el.prepend(node = document.createTextNode(''));
    if (node.data !== value) node.data = value;
  }
  const watch = new MutationObserver(sync);
  watch.observe(el, { attributes: true, attributeFilter: ['style', 'data-time-reservation'] });
  const resized = typeof ResizeObserver === 'function' ? new ResizeObserver(sync) : null;
  resized?.observe(el);
  el.exactLiveTime = { sync, text, destroy() { watch.disconnect(); resized?.disconnect(); } };
  if (el.exactTimeText !== undefined) text(el.exactTimeText);
  sync();
}

if (globalThis.exact) globalThis.exact.installLiveTime = install;
