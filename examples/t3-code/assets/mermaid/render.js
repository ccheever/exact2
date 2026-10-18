// One ```mermaid fence, rendered as T3 Code 1e2ecbd975 renders it (apps/web/src/components/chat/MermaidDiagram.tsx;
// MIT, see LICENSE-T3): the reference's Mermaid configuration, its DOMPurify pass over the SVG, its error and retry
// rules. The app frames this page per fence (markdown.contract MermaidBlock) with `theme`, `font` and `source` in the
// fragment, and the page tells it, as a JSON string posted to its parent, what came of it: `rendered` with the
// diagram's natural size, or `error` with Mermaid's message and whether Retry may help (timeline-mermaid.ts).

const SECURE = ['secure', 'securityLevel', 'startOnLoad', 'maxTextSize', 'suppressErrorRendering', 'maxEdges', 'htmlLabels', 'themeCSS'];
// Mermaid also lazy-loads diagram chunks inside render(); losing them there is worth a retry, unlike a syntax error.
const CHUNK_LOAD_ERROR = /dynamically imported module|importing a module script|failed to fetch/i;
const REMOTE_CSS_URL = /url\(\s*(?!['"]?#)[^)]*\)/gi;
const round = value => Math.round(value * 1000) / 1000;

const params = new URLSearchParams(location.hash.slice(1));
const theme = params.get('theme') === 'dark' ? 'dark' : 'light';
const font = params.get('font') || 'system-ui, sans-serif';
const source = (params.get('source') || '').trim();

function tell(result) {
  parent.postMessage(JSON.stringify({ ...result, theme, source }), '*');
}

// The reference's diagrams are laid out by Chromium. WebKit tracks the system font (its 'trak' table) and keeps a
// font's ascent and descent fractional, so its labels measure narrower and its text boxes taller, and Mermaid would
// size every node differently. Text boxes report Chromium's line metrics, and text is measured untracked.
function chromiumTextMetrics() {
  const measure = SVGGraphicsElement.prototype.getBBox;
  SVGTextElement.prototype.getBBox = function (...args) {
    const box = measure.apply(this, args);
    if (!box || !box.height) return box;
    const size = parseFloat(getComputedStyle(this).fontSize) || 16, ascent = size * 0.966875, descent = size * 0.210625;
    const shift = ascent - Math.round(ascent), shrink = ascent + descent - Math.round(ascent) - Math.round(descent);
    return new DOMRect(box.x, box.y + shift, box.width, Math.max(0, box.height - shrink));
  };
  document.body.style.textRendering = 'geometricPrecision';
}

// Diagrams can come from untrusted PR descriptions, so strip anything that can navigate, run script, or fetch remote
// content on top of Mermaid's own strict sanitization. CSS keeps only local url(#id) references.
function sanitizer(DOMPurify) {
  const purifier = DOMPurify(window);
  purifier.addHook('uponSanitizeElement', (node, data) => {
    if (data.tagName === 'style' && node.textContent) node.textContent = node.textContent.replace(REMOTE_CSS_URL, 'none');
  });
  purifier.addHook('uponSanitizeAttribute', (_node, data) => {
    if (data.attrName === 'style') data.attrValue = data.attrValue.replace(REMOTE_CSS_URL, 'none');
  });
  return svg => purifier.sanitize(svg, {
    ADD_TAGS: ['foreignObject'],
    HTML_INTEGRATION_POINTS: { foreignobject: true },
    FORBID_ATTR: ['href', 'xlink:href', 'src', 'srcset'],
    FORBID_TAGS: ['a', 'img', 'image', 'script'],
    USE_PROFILES: { svg: true, svgFilters: true, html: true },
  });
}

async function render() {
  let mermaid, sanitize;
  try {
    mermaid = (await import('./vendor/mermaid.esm.min.mjs')).default;
    sanitize = sanitizer((await import('./vendor/purify.es.mjs')).default);
  } catch {
    return tell({ status: 'error', message: 'Mermaid failed to load.', retryable: true });
  }
  const id = 'mermaid-diagram-0';
  let svg;
  try {
    document.body.style.fontFamily = font;
    chromiumTextMetrics();
    mermaid.initialize({
      startOnLoad: false,
      securityLevel: 'strict',
      suppressErrorRendering: true,
      // HTML labels and theme CSS are mounted while Mermaid lays the diagram out, before sanitizing, so diagram
      // directives must not set them.
      secure: SECURE,
      htmlLabels: false,
      flowchart: { htmlLabels: false },
      theme: theme === 'dark' ? 'dark' : 'default',
      fontFamily: font,
    });
    ({ svg } = await mermaid.render(id, source));
  } catch (error) {
    const message = error instanceof Error ? error.message : 'The diagram could not be rendered.';
    return tell({ status: 'error', message, retryable: CHUNK_LOAD_ERROR.test(message) });
  } finally {
    document.getElementById(`d${id}`)?.remove();
  }
  const host = document.getElementById('host');
  host.innerHTML = sanitize(svg);
  const root = host.querySelector('svg');
  if (!root) return tell({ status: 'error', message: 'The diagram could not be rendered.', retryable: false });
  const box = root.viewBox.baseVal;
  const width = box && box.width ? box.width : root.getBBox().width, height = box && box.height ? box.height : root.getBBox().height;
  tell({ status: 'rendered', width: round(width), height: round(height) });
}

render();
