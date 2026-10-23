// One ```mermaid fence, rendered as T3 Code 74541bab17 renders it (apps/web/src/components/chat/MermaidDiagram.tsx;
// MIT, see LICENSE-T3): pele, loaded per diagram type, mounted with the reference's options, its DOMPurify passes
// over the SVG, and KaTeX's MathML for `$$` labels. The app frames this page per fence (markdown.contract
// MermaidBlock) the lane wide, with `theme`, `font`, `source`, `view` and the preview's `width` in the fragment.
// Pele lays the diagram out for the page's width, as for the reference's lane, and lays it out again when that
// width changes. The page tells the app, as a JSON string posted to its parent, what came of it: `rendered` with the
// drawn diagram's size and the width it was drawn for, again whenever that size changes, or `error` with pele's
// message and whether Retry may help (timeline-mermaid.ts).

const params = new URLSearchParams(location.hash.slice(1));
const theme = params.get('theme') === 'dark' ? 'dark' : 'light';
const font = params.get('font') || 'system-ui, sans-serif';
const source = (params.get('source') || '').trim();
const view = params.get('view') || '';
// The expanded preview lays the diagram out as its opener drew it, at that width, and scales the SVG to its box.
const drawnWidth = Number(params.get('width')) || 0;
const round = value => Math.round(value * 1000) / 1000;

function tell(result) {
  parent.postMessage(JSON.stringify({ ...result, theme, source, view }), '*');
}

// MathML element types KaTeX emits, without annotation (the resolver removes it),
// semantics (only a wrapper, so its content is kept), and mglyph (an image).
const KATEX_MATHML_TAGS = new Set(['math', 'menclose', 'mfrac', 'mi', 'mlabeledtr', 'mn', 'mo', 'mover', 'mpadded', 'mphantom', 'mroot', 'mrow', 'mspace',
  'msqrt', 'mstyle', 'msub', 'msubsup', 'msup', 'mtable', 'mtd', 'mtext', 'mtr', 'munder', 'munderover']);
// Presentation attributes KaTeX sets on those elements. The profiles already
// cover class, style, width, height, display, and xmlns.
const KATEX_MATHML_ATTRIBUTES = new Set(['accent', 'accentunder', 'columnalign', 'columnlines', 'columnspacing', 'depth', 'displaystyle', 'fence', 'largeop',
  'linebreak', 'linethickness', 'lspace', 'mathbackground', 'mathcolor', 'mathsize', 'mathvariant', 'maxsize', 'minsize', 'notation', 'rowlines', 'rowspacing',
  'rspace', 'scriptlevel', 'separator', 'stretchy', 'voffset']);

// Diagrams can come from untrusted PR descriptions. Pele escapes labels and
// links and images are off, so this is defense in depth against anything that
// could navigate, run script, or fetch remote content.
function sanitizers(DOMPurify) {
  const purifier = DOMPurify(window);
  return {
    plain: svg => purifier.sanitize(svg, {
      FORBID_ATTR: ['href', 'xlink:href', 'src', 'srcset'],
      FORBID_TAGS: ['a', 'image', 'foreignObject', 'style'],
      USE_PROFILES: { svg: true, svgFilters: true },
      RETURN_DOM_FRAGMENT: true,
    }),
    // Pele puts MathML labels in a <foreignObject> with an HTML <div> around them.
    math: svg => purifier.sanitize(svg, {
      // DOMPurify still only accepts <math> under an HTML parent and checks every
      // MathML child's namespace.
      ADD_TAGS: ['foreignObject', ...KATEX_MATHML_TAGS],
      ADD_ATTR: (attribute, tagName) => KATEX_MATHML_TAGS.has(tagName) && KATEX_MATHML_ATTRIBUTES.has(attribute),
      HTML_INTEGRATION_POINTS: { foreignobject: true },
      FORBID_ATTR: ['href', 'xlink:href', 'src', 'srcset'],
      FORBID_TAGS: ['a', 'img', 'image', 'script', 'style'],
      USE_PROFILES: { svg: true, svgFilters: true, html: true },
      RETURN_DOM_FRAGMENT: true,
    }),
  };
}

// Pele has no typesetter, so $$...$$ labels go through the KaTeX the math page
// uses. It only loads for diagrams that contain math.
async function loadMath() {
  let katex;
  try { katex = (await import('../math/vendor/katex.mjs')).default; } catch { return null; }
  const typeset = new Map();
  let probe = null;
  // Pele calls this on every layout, including resizes.
  return (tex, size) => {
    const key = `${size}:${tex}`;
    if (typeset.has(key)) return typeset.get(key);
    let markup = null;
    try {
      if (!probe) {
        probe = document.createElement('div');
        probe.style.cssText = 'position:absolute;left:-9999px;visibility:hidden;display:inline-block;white-space:nowrap';
        document.body.append(probe);
      }
      probe.style.fontSize = `${size}px`;
      probe.innerHTML = katex.renderToString(tex, {
        displayMode: true,
        output: 'mathml',
        throwOnError: true,
        // No \href, \url, \htmlClass, or other commands that emit active HTML.
        trust: false,
        strict: 'ignore',
        maxSize: 20,
        maxExpand: 1000,
      });
      const expression = probe.querySelector('math');
      // The annotation repeats the TeX source; the sanitizer would keep its text.
      expression?.querySelector('annotation')?.remove();
      if (expression) {
        const { width, height } = expression.getBoundingClientRect();
        markup = { html: expression.outerHTML, width, height };
      }
    } catch {
      // Unsupported TeX stays readable as its source.
    }
    typeset.set(key, markup);
    return markup;
  };
}

async function render() {
  document.documentElement.dataset.theme = theme;
  document.body.style.fontFamily = font;
  let pele, sanitize, type = null;
  try {
    pele = await import('./vendor/pele/lazy.js');
    sanitize = sanitizers((await import('./vendor/purify.es.mjs')).default);
    try { type = pele.detectType(source); } catch {}
    // Each diagram type is its own chunk, so a diagram loads only its parser and layout.
    if (type) await pele.load(type);
  } catch {
    return tell({ status: 'error', message: 'The diagram renderer failed to load.', retryable: true });
  }
  const math = source.includes('$$') ? await loadMath() : null;
  const host = document.getElementById('host');
  // The drawn diagram's box: what pele drew, and its height again after CSS scales a diagram that cannot narrow.
  let reported = '';
  const report = () => {
    const svg = host.querySelector('svg');
    if (!svg) return;
    const box = svg.getBoundingClientRect();
    if (!(box.width > 0) || !(box.height > 0)) return;
    // The width pele laid the diagram out for: the frame's, which the app matches to its own.
    const drawnFor = Math.round(host.clientWidth);
    const next = `${drawnFor}:${round(box.width)}x${round(box.height)}`;
    if (next === reported) return;
    reported = next;
    tell({ status: 'rendered', width: round(box.width), height: round(box.height), drawnFor });
  };
  new ResizeObserver(report).observe(host);
  pele.mount(host, source, {
    links: false,
    images: false,
    zoom: false,
    // Keep the author's direction; a wide diagram scales down to fit instead of turning vertical.
    autoDirection: false,
    ...(drawnWidth > 0 ? { maxWidth: drawnWidth } : {}),
    ...(math ? { math, sanitize: sanitize.math } : { sanitize: sanitize.plain }),
    onRender: () => { reported = ''; report(); },
    onError: error => tell({ status: 'error', message: error instanceof Error ? error.message : 'The diagram could not be rendered.', retryable: false }),
  });
}

render();
