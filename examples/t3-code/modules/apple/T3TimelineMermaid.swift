#if os(macOS)
import AppKit
import WebKit

/// Mermaid diagrams for the transcript (MermaidDiagram.tsx, T3 Code, MIT; see
/// LICENSE-T3). The connected T3 server serves the reference web client, and
/// with it the reference's own Mermaid build: an offscreen web view loads that
/// chunk from the server's origin, lays each diagram out with the reference's
/// configuration (`theme` default/dark, `htmlLabels: false`, strict security,
/// the body font) and flattens it into paths and text runs (`flattenScript`).
/// Renders run one at a time, as `initialize()` is global; each finished
/// render is cached by theme and source and announced on `t3.status`, so the
/// next snapshot draws it.
final class T3TimelineMermaid: NSObject, WKNavigationDelegate {
    private enum Loader { case idle, discovering, loading, ready, failed(String) }
    private struct Job { let key: String; let source: String; let theme: String }
    private let changed: (String) -> Void
    private var results: [String: String] = [:]
    private var order: [String] = []
    private var queue: [Job] = []
    private var busy = false
    private var loader = Loader.idle
    private var origin = ""
    private var chunk = ""
    private var font = "-apple-system, \"system-ui\", \"Segoe UI\", system-ui, sans-serif"
    private var webView: WKWebView?
    private static let limit = 128

    init(changed: @escaping (String) -> Void) { self.changed = changed }

    /// `{op: "mermaidRender", origin, font, diagrams: [{source, theme}], retry: [key]}` →
    /// `{items: [{key, json}]}`; `json` is empty while the diagram renders.
    func perform(_ request: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        let generation = request["generation"] as? Int ?? 0
        let nextOrigin = (request["origin"] as? String ?? "").trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        if let requested = request["font"] as? String, !requested.isEmpty { font = requested }
        if nextOrigin != origin {
            origin = nextOrigin; chunk = ""; loader = .idle
            webView?.navigationDelegate = nil; webView = nil
        }
        for key in request["retry"] as? [String] ?? [] {
            results.removeValue(forKey: key)
            if case .failed = loader { loader = .idle; webView = nil }
        }
        var items: [[String: Any]] = []
        for diagram in request["diagrams"] as? [[String: Any]] ?? [] {
            let source = diagram["source"] as? String ?? "", theme = diagram["theme"] as? String == "dark" ? "dark" : "light"
            let key = "\(theme)\n\(source)"
            if let json = results[key] { items.append(["key": key, "json": json]); continue }
            if !queue.contains(where: { $0.key == key }) { queue.append(Job(key: key, source: source, theme: theme)) }
            items.append(["key": key, "json": ""])
        }
        reply(["ok": true, "generation": generation, "value": ["items": items]])
        pump()
    }

    private func store(_ key: String, _ json: String) {
        results[key] = json
        order.removeAll { $0 == key }; order.append(key)
        while order.count > Self.limit { results.removeValue(forKey: order.removeFirst()) }
    }
    private static func failure(_ message: String, retryable: Bool) -> String {
        let body: [String: Any] = ["status": "error", "message": message, "retryable": retryable]
        return (try? JSONSerialization.data(withJSONObject: body)).flatMap { String(data: $0, encoding: .utf8) } ?? "{\"status\":\"error\",\"message\":\"\",\"retryable\":false}"
    }

    private func pump() {
        guard !busy, let job = queue.first else { return }
        switch loader {
        case .idle: discover()
        case .discovering, .loading: return
        case .failed(let message):
            for job in queue { store(job.key, Self.failure(message, retryable: true)) }
            queue.removeAll(); changed("t3.status")
        case .ready: render(job)
        }
    }

    /// The reference loads `mermaid.core-*.js` on demand from ChatMarkdown's
    /// chunk; find it by following the client's own asset references.
    private func discover() {
        guard let base = URL(string: origin + "/"), origin.hasPrefix("http") else { loader = .failed("Mermaid failed to load."); return pump() }
        loader = .discovering
        let wanted = origin
        Self.findChunk(base: base) { [weak self] found in
            DispatchQueue.main.async {
                guard let self, self.origin == wanted else { return }
                guard let found else { self.loader = .failed("Mermaid failed to load."); return self.pump() }
                self.chunk = found
                self.load(base: base)
            }
        }
    }
    private static func fetch(_ url: URL, _ done: @escaping (String?) -> Void) {
        let request = URLRequest(url: url, cachePolicy: .returnCacheDataElseLoad, timeoutInterval: 15)
        URLSession.shared.dataTask(with: request) { data, response, _ in
            guard let data, (response as? HTTPURLResponse)?.statusCode == 200 else { return done(nil) }
            done(String(data: data, encoding: .utf8))
        }.resume()
    }
    static func findChunk(base: URL, _ done: @escaping (String?) -> Void) {
        let reference = try! NSRegularExpression(pattern: #"(?:assets/|\./)([A-Za-z0-9_.-]+\.js)"#)
        let target = try! NSRegularExpression(pattern: #"mermaid\.core-[A-Za-z0-9_-]+\.js"#)
        func names(_ text: String) -> [String] {
            reference.matches(in: text, range: NSRange(text.startIndex..., in: text)).compactMap { Range($0.range(at: 1), in: text).map { "assets/" + text[$0] } }
        }
        func score(_ name: String) -> Int { name.contains("ChatMarkdown") ? 3 : (name.contains("main-") || name.contains("index-")) ? 2 : 0 }
        var queue: [String] = [], seen = Set<String>()
        func step() {
            queue.sort { score($0) > score($1) }
            guard seen.count < 80, let next = queue.first else { return done(nil) }
            queue.removeFirst()
            if !seen.insert(next).inserted { return step() }
            guard let url = URL(string: next, relativeTo: base) else { return step() }
            fetch(url) { text in
                guard let text else { return step() }
                if let match = target.firstMatch(in: text, range: NSRange(text.startIndex..., in: text)), let range = Range(match.range, in: text) {
                    return done("assets/" + text[range])
                }
                queue.append(contentsOf: names(text).filter { !seen.contains($0) })
                step()
            }
        }
        fetch(base) { index in
            guard let index else { return done(nil) }
            queue = names(index)
            step()
        }
    }

    private func load(base: URL) {
        loader = .loading
        let view = Self.renderer()
        view.navigationDelegate = self
        webView = view
        let page = "<!doctype html><html><head><meta charset=\"utf-8\"><script>\(Self.flattenScript)</script></head><body style=\"margin:0\"><div id=\"host\"></div></body></html>"
        view.loadHTMLString(page, baseURL: base)
    }
    /// The offscreen renderer: a non-persistent store, inspectable in a development build (EXACT2-GAPS X2).
    static func renderer() -> WKWebView {
        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = .nonPersistent()
        let view = WKWebView(frame: NSRect(x: 0, y: 0, width: 1600, height: 1200), configuration: configuration)
        T3WebInspection.mark(view, "mermaid")
        return view
    }
    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        guard webView === self.webView, case .loading = loader else { return }
        let url = origin + "/" + chunk
        webView.callAsyncJavaScript("window.__mermaid = (await import(url)).default; return true;", arguments: ["url": url], in: nil, in: .page) { [weak self] result in
            guard let self, webView === self.webView else { return }
            if case .success = result { self.loader = .ready } else { self.loader = .failed("Mermaid failed to load.") }
            self.pump()
        }
    }
    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) { loadFailed(webView) }
    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) { loadFailed(webView) }
    private func loadFailed(_ view: WKWebView) {
        guard view === webView else { return }
        loader = .failed("Mermaid failed to load."); pump()
    }

    private func render(_ job: Job) {
        guard let webView else { loader = .idle; return pump() }
        busy = true
        webView.callAsyncJavaScript("return await window.__t3render(source, theme, font);", arguments: ["source": job.source, "theme": job.theme, "font": font], in: nil, in: .page) { [weak self] result in
            guard let self else { return }
            self.busy = false
            let json: String
            switch result {
            case .success(let value): json = value as? String ?? Self.failure("The diagram could not be rendered.", retryable: false)
            case .failure(let error): json = Self.failure(error.localizedDescription, retryable: true)
            }
            self.queue.removeAll { $0.key == job.key }
            self.store(job.key, json)
            self.changed("t3.status")
            self.pump()
        }
    }

    /// Lays a diagram out and flattens it: every painted shape a path in the root's
    /// user space (transform, paint, stroke), every text run a positioned string on
    /// its baseline, every marker placed at its vertex and turned to the tangent.
    static let flattenScript = #"""
// MermaidDiagram.tsx's render, flattened for a native canvas (T3 Code, MIT; see
// LICENSE-T3). Mermaid lays the diagram out in this page with the reference's
// configuration; every painted shape then becomes a path in the root's user
// space (its transform, resolved paint and stroke) and every text run a
// positioned string, so the app can draw it without a browser. Markers
// (arrowheads) are placed the way SVG places them: at the end point, turned to
// the tangent, scaled from the marker's view box.
(() => {
  const SECURE = ['secure', 'securityLevel', 'startOnLoad', 'maxTextSize', 'suppressErrorRendering', 'maxEdges', 'htmlLabels', 'themeCSS'];
  const CHUNK_LOAD_ERROR = /dynamically imported module|importing a module script|failed to fetch/i;
  const round = value => Math.round(value * 1000) / 1000;
  const matrix = m => `matrix(${[m.a, m.b, m.c, m.d, m.e, m.f].map(round).join(' ')})`;
  const hex = n => Math.max(0, Math.min(255, Math.round(n))).toString(16).padStart(2, '0');
  /** A computed paint as #rrggbbaa, folding in an opacity; '' for none. */
  function paint(value, opacity) {
    if (!value || value === 'none' || value.startsWith('url(')) return '';
    const match = /rgba?\(([^)]+)\)/.exec(value);
    if (!match) return '';
    const parts = match[1].split(/[\s,/]+/).filter(Boolean).map(Number);
    const alpha = (parts.length > 3 ? parts[3] : 1) * opacity;
    if (alpha <= 0) return '';
    return `#${hex(parts[0])}${hex(parts[1])}${hex(parts[2])}${alpha >= 0.999 ? '' : hex(alpha * 255)}`;
  }
  function groupOpacity(element, root) {
    let opacity = 1;
    for (let node = element; node && node !== root; node = node.parentElement) opacity *= Number(getComputedStyle(node).opacity) || 0;
    return opacity;
  }
  const num = (element, name) => Number(element.getAttribute(name)) || 0;
  const screen = element => DOMMatrix.fromMatrix(element.getScreenCTM());
  function shapePath(element) {
    switch (element.tagName.toLowerCase()) {
      case 'path': return element.getAttribute('d') || '';
      case 'rect': {
        const x = num(element, 'x'), y = num(element, 'y'), w = num(element, 'width'), h = num(element, 'height');
        if (w <= 0 || h <= 0) return '';
        let rx = element.hasAttribute('rx') ? num(element, 'rx') : num(element, 'ry'), ry = element.hasAttribute('ry') ? num(element, 'ry') : rx;
        rx = Math.min(rx, w / 2); ry = Math.min(ry, h / 2);
        if (!rx || !ry) return `M${x} ${y}H${x + w}V${y + h}H${x}Z`;
        return `M${x + rx} ${y}H${x + w - rx}A${rx} ${ry} 0 0 1 ${x + w} ${y + ry}V${y + h - ry}A${rx} ${ry} 0 0 1 ${x + w - rx} ${y + h}H${x + rx}A${rx} ${ry} 0 0 1 ${x} ${y + h - ry}V${y + ry}A${rx} ${ry} 0 0 1 ${x + rx} ${y}Z`;
      }
      case 'circle': case 'ellipse': {
        const cx = num(element, 'cx'), cy = num(element, 'cy');
        const rx = element.tagName.toLowerCase() === 'circle' ? num(element, 'r') : num(element, 'rx'), ry = element.tagName.toLowerCase() === 'circle' ? rx : num(element, 'ry');
        if (rx <= 0 || ry <= 0) return '';
        return `M${cx - rx} ${cy}A${rx} ${ry} 0 1 0 ${cx + rx} ${cy}A${rx} ${ry} 0 1 0 ${cx - rx} ${cy}Z`;
      }
      case 'line': return `M${num(element, 'x1')} ${num(element, 'y1')}L${num(element, 'x2')} ${num(element, 'y2')}`;
      case 'polyline': case 'polygon': {
        const points = (element.getAttribute('points') || '').trim().split(/[\s,]+/).map(Number);
        if (points.length < 4) return '';
        let d = `M${points[0]} ${points[1]}`;
        for (let i = 2; i + 1 < points.length; i += 2) d += `L${points[i]} ${points[i + 1]}`;
        return element.tagName.toLowerCase() === 'polygon' ? d + 'Z' : d;
      }
    }
    return '';
  }
  function painted(element, rootInverse, root, transform) {
    const style = getComputedStyle(element), opacity = groupOpacity(element, root);
    const d = shapePath(element);
    if (!d) return null;
    const fill = paint(style.fill, (Number(style.fillOpacity) || 0) * opacity);
    const stroke = paint(style.stroke, (Number(style.strokeOpacity) || 0) * opacity);
    const width = parseFloat(style.strokeWidth) || 0;
    if (!fill && (!stroke || width <= 0)) return null;
    const dash = style.strokeDasharray && style.strokeDasharray !== 'none' ? style.strokeDasharray.replace(/px/g, '').replace(/,\s*/g, ' ') : '';
    return { kind: 'path', d, transform: matrix(transform ?? rootInverse.multiply(screen(element))), fill, stroke, width: round(width), dash,
      cap: style.strokeLinecap || 'butt', join: style.strokeLinejoin || 'miter' };
  }
  /** SVG marker placement: the content's viewBox scaled into markerWidth×markerHeight, ref point on the vertex, turned to the tangent. */
  function markers(element, rootInverse, root, items) {
    const style = getComputedStyle(element), width = parseFloat(style.strokeWidth) || 1;
    for (const [which, value] of [['start', style.markerStart], ['end', style.markerEnd]]) {
      const id = /url\(["']?#([^"')]+)["']?\)/.exec(value || '')?.[1];
      const marker = id ? root.querySelector(`marker[id="${CSS.escape(id)}"]`) : null;
      if (!marker || typeof element.getTotalLength !== 'function') continue;
      const length = element.getTotalLength();
      if (!(length > 0)) continue;
      const at = which === 'end' ? length : 0, epsilon = Math.min(0.5, length / 2);
      const point = element.getPointAtLength(at), near = element.getPointAtLength(which === 'end' ? at - epsilon : at + epsilon);
      let angle = which === 'end' ? Math.atan2(point.y - near.y, point.x - near.x) : Math.atan2(near.y - point.y, near.x - point.x);
      const orient = marker.getAttribute('orient') || '0';
      if (orient === 'auto-start-reverse' && which === 'start') angle += Math.PI;
      else if (orient !== 'auto' && orient !== 'auto-start-reverse') angle = (parseFloat(orient) || 0) * Math.PI / 180;
      const unitScale = (marker.getAttribute('markerUnits') || 'strokeWidth') === 'strokeWidth' ? width : 1;
      const mw = (marker.hasAttribute('markerWidth') ? num(marker, 'markerWidth') : 3) * unitScale, mh = (marker.hasAttribute('markerHeight') ? num(marker, 'markerHeight') : 3) * unitScale;
      const box = marker.viewBox?.baseVal;
      const sx = box && box.width ? mw / box.width : unitScale, sy = box && box.height ? mh / box.height : unitScale;
      const scale = Math.min(sx, sy);
      const refX = num(marker, 'refX'), refY = num(marker, 'refY');
      const place = new DOMMatrix().translate(point.x, point.y).rotate(angle * 180 / Math.PI).scale(box && box.width ? scale : unitScale).translate(-refX, -refY);
      const base = rootInverse.multiply(screen(element)).multiply(place);
      for (const child of marker.querySelectorAll('path,rect,circle,ellipse,line,polyline,polygon')) {
        const own = child.transform?.baseVal?.consolidate()?.matrix;
        const item = painted(child, rootInverse, marker, own ? base.multiply(DOMMatrix.fromMatrix(own)) : base);
        if (item) items.push(item);
      }
    }
  }
  /** The baseline a run's start position is reported on: its own alignment, else the dominant one it inherits. */
  function baseline(run) {
    for (let node = run; node && node.tagName && node.tagName.toLowerCase() !== 'svg'; node = node.parentElement) {
      const style = getComputedStyle(node), align = style.alignmentBaseline, dominant = style.dominantBaseline;
      if (align && !['auto', 'baseline'].includes(align)) return align === 'before-edge' || align === 'text-before-edge' ? 'text-before-edge' : align === 'after-edge' || align === 'text-after-edge' ? 'text-after-edge' : align;
      if (dominant && dominant !== 'auto') return dominant;
    }
    return 'alphabetic';
  }
  function texts(element, rootInverse, root, items) {
    const transform = matrix(rootInverse.multiply(screen(element)));
    const opacity = groupOpacity(element, root);
    const leaves = [...element.querySelectorAll('tspan')].filter(span => !span.querySelector('tspan'));
    const runs = leaves.length ? leaves : [element];
    for (const run of runs) {
      const text = (run.textContent || '').replace(/\s+/g, ' ');
      if (!text.trim() || run.getNumberOfChars() < 1) continue;
      const style = getComputedStyle(run), fill = paint(style.fill, (Number(style.fillOpacity) || 0) * opacity);
      if (!fill) continue;
      // A word's leading space belongs to the gap before it: start at its first glyph.
      const lead = text.startsWith(' ') && run.getNumberOfChars() > 1 ? 1 : 0;
      const start = run.getStartPositionOfChar(lead);
      items.push({ kind: 'text', text: lead ? text.slice(1) : text, transform, fill, x: round(start.x), y: round(start.y), size: round(parseFloat(style.fontSize) || 16),
        weight: Number(style.fontWeight) || (style.fontWeight === 'bold' ? 700 : 400), slant: style.fontStyle === 'italic' ? 'italic' : 'normal', baseline: baseline(run) });
    }
  }
  // Chrome rounds a font's ascent and descent to whole pixels for a text box; WebKit keeps
  // them fractional (SF Pro at 16px: 15.47 + 3.37), so each label box is ~0.8px taller here.
  // Text boxes report Chrome's line metrics, so Mermaid sizes nodes as the reference does.
  if (!window.__t3bbox) {
    window.__t3bbox = SVGGraphicsElement.prototype.getBBox;
    SVGTextElement.prototype.getBBox = function (...args) {
      const box = window.__t3bbox.apply(this, args);
      if (!box || !box.height) return box;
      const size = parseFloat(getComputedStyle(this).fontSize) || 16, ascent = size * 0.966875, descent = size * 0.210625;
      const shift = ascent - Math.round(ascent), shrink = ascent + descent - Math.round(ascent) - Math.round(descent);
      return new DOMRect(box.x, box.y + shift, box.width, Math.max(0, box.height - shrink));
    };
  }
  window.__t3render = async (source, theme, font) => {
    const mermaid = window.__mermaid;
    if (!mermaid) return JSON.stringify({ status: 'error', message: 'Mermaid failed to load.', retryable: true });
    const id = `mermaid-diagram-${(window.__t3next = (window.__t3next || 0) + 1)}`;
    let svg;
    try {
      document.body.style.fontFamily = font;
      // WebKit tracks the system font (its 'trak' table) where the reference's Chrome does not, so its
      // labels measure ~0.4px a glyph narrower at 16px and every box with them; geometric precision
      // measures untracked, as Chrome lays the reference's diagrams out (lane r4-timeline).
      document.body.style.textRendering = 'geometricPrecision';
      mermaid.initialize({ startOnLoad: false, securityLevel: 'strict', suppressErrorRendering: true, secure: SECURE, htmlLabels: false,
        flowchart: { htmlLabels: false }, theme: theme === 'dark' ? 'dark' : 'default', fontFamily: font });
      ({ svg } = await mermaid.render(id, source));
    } catch (error) {
      const message = error instanceof Error ? error.message : 'The diagram could not be rendered.';
      return JSON.stringify({ status: 'error', message, retryable: CHUNK_LOAD_ERROR.test(message) });
    } finally {
      document.getElementById(`d${id}`)?.remove();
    }
    const host = document.getElementById('host');
    host.innerHTML = svg;
    const root = host.querySelector('svg');
    if (!root) return JSON.stringify({ status: 'error', message: 'The diagram could not be rendered.', retryable: false });
    const box = root.viewBox.baseVal;
    const width = box && box.width ? box.width : root.getBBox().width, height = box && box.height ? box.height : root.getBBox().height;
    root.setAttribute('width', String(width)); root.setAttribute('height', String(height)); root.style.maxWidth = 'none';
    const rootInverse = screen(root).inverse();
    const items = [];
    for (const element of root.querySelectorAll('path,rect,circle,ellipse,line,polyline,polygon,text')) {
      if (element.closest('defs,marker,clipPath,mask,pattern,symbol,foreignObject')) continue;
      if (element.tagName.toLowerCase() !== 'text' && element.closest('text')) continue;
      const style = getComputedStyle(element);
      if (style.display === 'none' || style.visibility === 'hidden') continue;
      if (element.tagName.toLowerCase() === 'text') { texts(element, rootInverse, root, items); continue; }
      const item = painted(element, rootInverse, root);
      if (item) items.push(item);
      markers(element, rootInverse, root, items);
      if (items.length > 6000) break;
    }
    host.innerHTML = '';
    const viewBox = box && box.width ? [box.x, box.y, box.width, box.height].map(round).join(' ') : `0 0 ${round(width)} ${round(height)}`;
    return JSON.stringify({ status: 'rendered', width: round(width), height: round(height), viewBox, items });
  };
})();
"""#
}
#endif
