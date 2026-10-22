// One display formula, typeset as T3 Code 74541bab17 typesets it (apps/web/src/components/chat/MathTypeset.tsx; MIT,
// see LICENSE-T3): KaTeX's HTML and MathML, with the reference's options. The app frames this page per formula
// (markdown.contract MathBlock) with `theme`, `size` (the chat's text size in px) and `tex` in the fragment, and
// the page tells it, as a JSON string posted to its parent, what came of it: `rendered` with the formula's size,
// again when its fonts load, or `error` when KaTeX cannot parse it, which leaves the TeX source shown
// (markdown-math.ts).

const params = new URLSearchParams(location.hash.slice(1));
const theme = params.get('theme') === 'dark' ? 'dark' : 'light';
const size = Number(params.get('size')) || 14;
const tex = params.get('tex') || '';
const round = value => Math.round(value * 1000) / 1000;

function tell(result) {
  parent.postMessage(JSON.stringify({ ...result, theme, size, tex }), '*');
}

async function render() {
  document.documentElement.dataset.theme = theme;
  document.body.style.fontSize = `${size}px`;
  const host = document.getElementById('host');
  try {
    const katex = (await import('./vendor/katex.mjs')).default;
    katex.render(tex.trim(), host, {
      displayMode: true,
      output: 'htmlAndMathml',
      throwOnError: true,
      // No \href, \url, \htmlClass, or other commands that emit active HTML.
      trust: false,
      strict: 'ignore',
      maxSize: 20,
      maxExpand: 1000,
    });
  } catch {
    // Unfinished or unsupported TeX stays readable as its source.
    return tell({ status: 'error' });
  }
  let reported = '';
  const report = () => {
    const box = host.getBoundingClientRect();
    const next = `${round(box.width)}x${round(box.height)}`;
    if (!(box.width > 0) || !(box.height > 0) || next === reported) return;
    reported = next;
    tell({ status: 'rendered', width: round(box.width), height: round(box.height) });
  };
  new ResizeObserver(report).observe(host);
  await document.fonts.ready;
  report();
}

render();
