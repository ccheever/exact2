#if os(macOS)
import AppKit
import WebKit

/// A Browser tab's annotation pick (browser-surface part 3). MIT reference, see LICENSE-T3, T3 Code 1e2ecbd975:
/// apps/desktop/src/preview/Manager.ts (`pickElement`, `cancelPickElement`, `captureAnnotationScreenshot`,
/// `normalizeCaptureRect`), PickPreload.ts (the overlay, here assets/browser-annotate.js) and react-grab's
/// `getElementContext` (the component name, source and stack).
///
/// - The overlay runs in the app's own content world (`.defaultClient`, as T3BrowserFavicon's script): it shares
///   the page's DOM, not its JavaScript, so the page can neither see nor call it, nor post to its handler.
///   Electron runs the reference's preload in the page's world (`contextIsolation=false`) so react-grab can read
///   React; here the overlay marks each picked element `data-t3code-pick`, and one lookup evaluated in the page's
///   world (`.page`) reads React's fiber on those elements (`__reactFiber$…`: the nearest component's name, its
///   `_debugSource`, its `_debugOwner` chain), then removes the marks. Production React keeps the fiber but not
///   the debug fields, and minifies names, as in the reference.
/// - The crop is `takeSnapshot` of the union of the targets (the overlay's rect, clamped to the page), written as
///   a PNG draft image (`imageDirectory`, the composer's draft images) within five seconds; a timeout, a failed
///   write or an image over 10 MB keeps the annotation without it (`screenshotFailed`).
/// - Exactly one settlement per pick (Manager `claimSettle`): a result, Escape in the page, `cancel()`, a newer
///   pick or a main-frame navigation; a crop that finishes after its pick settled is dropped and its file removed.
/// - The data module reads the outcome with `take(serial:)` once `report.ready` (T3BrowserSessions' ops).
final class T3BrowserAnnotation: NSObject, WKScriptMessageHandler {
    static let handlerName = "t3BrowserAnnotate"
    static let scriptFile = "browser-annotate.js"
    static let screenshotTimeout: TimeInterval = 5
    static let maxImageBytes = 10 * 1024 * 1024
    static let maxFieldLength = 500
    static let maxStackFrames = 8

    /// The tab's own page: a pop-up shares its configuration (and so this handler), and its messages are not the tab's.
    weak var page: WKWebView?
    var changed: (() -> Void)?
    private(set) var active = false
    /// Bumps each time a pick settles (a result, a cancel, a navigation).
    private(set) var serial = 0
    /// The current pick's identity: a late callback of an older pick finds it changed.
    private var pick = 0
    /// The settled outcome waiting for the data module, by serial (only the newest is kept).
    private var outcome: (serial: Int, value: [String: Any])?
    private let imageDirectory: URL?
    private weak var controller: WKUserContentController?
    private var closed = false

    /// The script message handler holds this object weakly: WKUserContentController retains it.
    private final class Relay: NSObject, WKScriptMessageHandler {
        weak var owner: T3BrowserAnnotation?
        func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) { owner?.userContentController(controller, didReceive: message) }
    }

    init(configuration: WKWebViewConfiguration, imageDirectory: URL?) {
        self.imageDirectory = imageDirectory
        super.init()
        let relay = Relay(); relay.owner = self
        configuration.userContentController.add(relay, contentWorld: .defaultClient, name: Self.handlerName)
        controller = configuration.userContentController
    }

    /// The overlay's source (`EXACT_ASSETS` in a development run, else the bundle's `assets/`).
    static func script() -> String? {
        guard let url = T3TerminalAssets.fileURL(scriptFile), let data = try? Data(contentsOf: url) else { return nil }
        return String(data: data, encoding: .utf8)
    }

    var report: [String: Any] { ["active": active, "serial": serial, "ready": outcome != nil] }

    // MARK: Commands

    /// PreviewManager.pickElement: a new pick replaces the one in flight (settled as cancelled first).
    func start(theme: [String: String]) {
        guard !closed else { return }
        if active { cancel() }
        pick += 1
        let current = pick
        active = true
        note("start")
        changed?()
        guard let web = page, let source = Self.script(),
              let themeData = try? JSONSerialization.data(withJSONObject: theme), let themeJSON = String(data: themeData, encoding: .utf8) else {
            note("start failed: no page or overlay script")
            return settle(current, nil)
        }
        // The script installs itself once per document (it keeps `globalThis.__t3codeAnnotate`), then starts.
        let program = "if (!globalThis.__t3codeAnnotate) {\n\(source)\n}\nglobalThis.__t3codeAnnotate.start(\(themeJSON)) ? 'started' : 'failed';"
        web.evaluateJavaScript(program, in: nil, in: .defaultClient) { [weak self] result in
            guard let self, current == self.pick else { return }
            if case .failure(let error) = result {
                self.note("start failed: \((error as NSError).code)")
                self.settle(current, nil)
            }
        }
        // `if (!wc.isFocused()) wc.focus()`: the overlay's keys (V, R, D, E, Escape, the comment) reach the page.
        if let window = web.window, window.firstResponder !== web { window.makeFirstResponder(web) }
    }

    /// PreviewManager.cancelPickElement: the overlay comes down and the pick settles with nothing.
    func cancel() {
        guard active else { return }
        page?.evaluateJavaScript("globalThis.__t3codeAnnotate && globalThis.__t3codeAnnotate.cancel(); 'ok'", in: nil, in: .defaultClient) { _ in }
        note("cancel")
        settle(pick, nil)
    }

    /// A main-frame navigation started (Manager `onNavigated`): the overlay's document is going away.
    func navigated() {
        guard active else { return }
        note("navigated")
        settle(pick, nil)
    }

    /// The settled pick for `serial`, once: the result, or `["cancelled": true]`.
    func take(serial: Int) -> [String: Any]? {
        guard let outcome, outcome.serial == serial else { return nil }
        self.outcome = nil
        changed?()
        return outcome.value
    }

    func close() {
        guard !closed else { return }
        cancel()
        closed = true
        outcome = nil
        controller?.removeScriptMessageHandler(forName: Self.handlerName, contentWorld: .defaultClient)
    }

    // MARK: Messages

    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
        guard !closed, active, let web = page, message.webView === web, let body = message.body as? [String: Any] else { return }
        let current = pick
        switch body["type"] as? String {
        case "cancelled":
            note("cancelled in the page")
            settle(current, nil)
        case "picked":
            guard var annotation = body["annotation"] as? [String: Any], JSONSerialization.isValidJSONObject(annotation) else {
                note("picked: malformed")
                return settle(current, nil)
            }
            annotation["screenshot"] = NSNull() // the page never supplies the crop (isPreviewAnnotationPayload)
            let submission = body["submission"] as? String == "send" ? "send" : "attach"
            let rect = Self.normalizeCaptureRect(body["rect"])
            enrich(annotation, in: web) { [weak self] enriched in
                guard let self, current == self.pick, self.active else { return }
                self.crop(web, rect: rect, annotationId: enriched["id"] as? String ?? "annotation") { [weak self] screenshot in
                    guard let self, current == self.pick, self.active else {
                        // claimSettle: a crop outliving its pick touches nothing, and its file goes.
                        if let id = screenshot?["id"] as? String, let directory = self?.imageDirectory ?? nil { try? FileManager.default.removeItem(at: directory.appendingPathComponent("\(id).png")) }
                        return
                    }
                    var result: [String: Any] = ["annotation": enriched, "submission": submission, "screenshotFailed": screenshot == nil]
                    if let screenshot { result["screenshot"] = screenshot }
                    // ANNOTATION_CAPTURED_CHANNEL: the overlay tears down once its marks are in the crop.
                    web.evaluateJavaScript("globalThis.__t3codeAnnotate && globalThis.__t3codeAnnotate.captured(); 'ok'", in: nil, in: .defaultClient) { _ in }
                    let elements = (enriched["elements"] as? [Any])?.count ?? 0, regions = (enriched["regions"] as? [Any])?.count ?? 0, strokes = (enriched["strokes"] as? [Any])?.count ?? 0
                    self.note("picked \(submission): elements \(elements), regions \(regions), strokes \(strokes), screenshot \(screenshot == nil ? "failed" : "ok")")
                    self.settle(current, result)
                }
            }
        default:
            return
        }
    }

    /// Claims the pick's one settlement (Manager `claimSettle`).
    private func settle(_ id: Int, _ value: [String: Any]?) {
        guard id == pick, active else { return }
        active = false
        serial += 1
        outcome = (serial, value ?? ["cancelled": true])
        changed?()
    }

    // MARK: React (the page's world)

    /// One evaluation in the page's world: for each element the overlay marked, React's component name, the
    /// element's source and the component owner chain, by target id; then the marks are removed.
    static let reactLookup = """
    (() => {
      const clip = (value) => typeof value === "string" ? value.slice(0, \(maxFieldLength)) : null;
      const num = (value) => typeof value === "number" && Number.isFinite(value) ? value : null;
      const nameOf = (type) => {
        if (!type || (typeof type !== "function" && typeof type !== "object")) return null;
        const name = type.displayName || type.name || (type.render && (type.render.displayName || type.render.name)) || (type.type && (type.type.displayName || type.type.name));
        return typeof name === "string" && name ? name : null;
      };
      const isComponent = (fiber) => !!fiber && (typeof fiber.type === "function" || (!!fiber.type && typeof fiber.type === "object"));
      const frame = (fiber, source) => {
        const at = source || null;
        return { functionName: clip(nameOf(fiber && fiber.type)), fileName: clip(at && at.fileName), lineNumber: num(at && at.lineNumber), columnNumber: num(at && at.columnNumber) };
      };
      const out = {};
      for (const element of Array.from(document.querySelectorAll("[data-t3code-pick]"))) {
        const id = element.getAttribute("data-t3code-pick");
        element.removeAttribute("data-t3code-pick");
        const info = { componentName: null, source: null, stack: [] };
        try {
          const key = Object.keys(element).find((name) => name.startsWith("__reactFiber$") || name.startsWith("__reactInternalInstance$"));
          const fiber = key ? element[key] : null;
          if (fiber) {
            let component = fiber._debugOwner && isComponent(fiber._debugOwner) ? fiber._debugOwner : null;
            for (let node = fiber.return, steps = 0; !component && node && steps < 200; node = node.return, steps += 1) if (isComponent(node)) component = node;
            if (component) {
              info.componentName = clip(nameOf(component.type));
              for (let owner = component, steps = 0; owner && steps < \(maxStackFrames); owner = owner._debugOwner, steps += 1) info.stack.push(frame(owner, owner._debugSource));
              const source = fiber._debugSource || component._debugSource;
              info.source = source ? { ...frame(component, source) } : (info.stack[0] || null);
            }
          }
        } catch (_) {}
        if (id) out[id] = info;
      }
      return JSON.stringify(out);
    })()
    """

    private func enrich(_ annotation: [String: Any], in web: WKWebView, done: @escaping ([String: Any]) -> Void) {
        guard let elements = annotation["elements"] as? [[String: Any]], !elements.isEmpty else { return done(annotation) }
        web.evaluateJavaScript(Self.reactLookup, in: nil, in: .page) { result in
            var found: [String: [String: Any]] = [:]
            if case .success(let value) = result, let text = value as? String, let data = text.data(using: .utf8),
               let parsed = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
                for (id, entry) in parsed { if let entry = entry as? [String: Any] { found[id] = entry } }
            }
            done(Self.merge(annotation, react: found))
        }
    }

    /// The page world's findings merged into `elements[i].element` by target id, each field checked and bounded:
    /// a string or null name, a stack frame of strings or nulls and finite numbers or nulls.
    static func merge(_ annotation: [String: Any], react: [String: [String: Any]]) -> [String: Any] {
        guard let elements = annotation["elements"] as? [[String: Any]] else { return annotation }
        var next = annotation
        next["elements"] = elements.map { target -> [String: Any] in
            guard let id = target["id"] as? String, let info = react[id], var element = target["element"] as? [String: Any] else { return target }
            if let name = info["componentName"] as? String, !name.isEmpty { element["componentName"] = String(name.prefix(maxFieldLength)) }
            let stack = (info["stack"] as? [Any] ?? []).prefix(maxStackFrames).compactMap(stackFrame)
            element["stack"] = Array(stack)
            element["source"] = stackFrame(info["source"]) ?? (stack.first as Any? ?? NSNull())
            var copy = target; copy["element"] = element
            return copy
        }
        return next
    }

    static func stackFrame(_ value: Any?) -> [String: Any]? {
        guard let frame = value as? [String: Any] else { return nil }
        let text = { (key: String) -> Any in (frame[key] as? String).map { String($0.prefix(maxFieldLength)) } ?? NSNull() }
        let number = { (key: String) -> Any in
            guard let value = frame[key] as? NSNumber, CFGetTypeID(value) != CFBooleanGetTypeID(), value.doubleValue.isFinite else { return NSNull() }
            return value
        }
        return ["functionName": text("functionName"), "fileName": text("fileName"), "lineNumber": number("lineNumber"), "columnNumber": number("columnNumber")]
    }

    // MARK: The crop

    /// Manager `normalizeCaptureRect`: finite, positive, whole CSS pixels from the top-left.
    static func normalizeCaptureRect(_ value: Any?) -> CGRect? {
        guard let rect = value as? [String: Any] else { return nil }
        let read = { (key: String) -> Double? in (rect[key] as? NSNumber).map(\.doubleValue).flatMap { $0.isFinite ? $0 : nil } }
        guard let x = read("x"), let y = read("y"), let width = read("width"), let height = read("height"), width > 0, height > 0 else { return nil }
        return CGRect(x: max(0, x.rounded(.down)), y: max(0, y.rounded(.down)), width: max(1, width.rounded(.up)), height: max(1, height.rounded(.up)))
    }

    /// `captureAnnotationScreenshot`: the rect (or the whole page) as a PNG draft image, or nil within the timeout.
    private func crop(_ web: WKWebView, rect: CGRect?, annotationId: String, done: @escaping ([String: Any]?) -> Void) {
        guard let directory = imageDirectory else { return done(nil) }
        var finished = false
        let finish = { (value: [String: Any]?) in
            guard !finished else { return }
            finished = true
            done(value)
        }
        let configuration = WKSnapshotConfiguration()
        let bounds = web.bounds
        let target = (rect ?? bounds).intersection(CGRect(origin: .zero, size: bounds.size))
        if rect != nil, !target.isNull, target.width >= 1, target.height >= 1 { configuration.rect = target }
        let timeout = DispatchWorkItem { finish(nil) }
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.screenshotTimeout, execute: timeout)
        web.takeSnapshot(with: configuration) { image, _ in
            timeout.cancel()
            guard !finished, let image, let cgImage = image.cgImage(forProposedRect: nil, context: nil, hints: nil) else { return finish(nil) }
            let id = UUID().uuidString.lowercased()
            let url = directory.appendingPathComponent("\(id).png")
            guard (try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)) != nil,
                  let png = NSBitmapImageRep(cgImage: cgImage).representation(using: .png, properties: [:]),
                  png.count <= Self.maxImageBytes, (try? png.write(to: url, options: .atomic)) != nil else { return finish(nil) }
            let cropped = configuration.rect.isNull || configuration.rect.isEmpty ? CGRect(origin: .zero, size: bounds.size) : configuration.rect
            finish(["id": id, "name": "preview-annotation-\(annotationId).png", "sizeBytes": png.count, "mimeType": "image/png",
                    "width": cgImage.width, "height": cgImage.height,
                    "cropRect": ["x": cropped.minX, "y": cropped.minY, "width": cropped.width, "height": cropped.height]])
        }
    }

    /// A `t3.browser:` host line (the agent's `logs`); never the page's content.
    private func note(_ line: String) {
        FileHandle.standardError.write(Data("t3.browser: annotate \(line)\n".utf8))
    }
}
#endif
