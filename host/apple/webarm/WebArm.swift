// The Apple iframe arm: WebKit stays in this dylib, loaded at the first
// iframe commit (@ref LLP 1020 D2/D3). The presenters see only its C ABI.
import Foundation
import UniformTypeIdentifiers
import WebKit

#if os(macOS)
import AppKit
typealias PlatformImage = NSImage
typealias PlatformView = NSView
/// WKWebView in a `fullSizeContentView` window otherwise inherits the
/// titlebar as a safe area and insets the guest by it — a black strip the
/// height of the titlebar over the deck. The kernel already framed this
/// box; the page fills it. @ref LLP 1020 D1
final class ExactWebView: WKWebView {
    override var safeAreaInsets: NSEdgeInsets { NSEdgeInsetsZero }
    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        pinInsets()
    }
    /// The keyboard input source the view's context last had connected.
    private var inputSource: String?
    private var inputSourceObserver: NSObjectProtocol?
    deinit {
        if let inputSourceObserver { NotificationCenter.default.removeObserver(inputSourceObserver) }
    }
    override func becomeFirstResponder() -> Bool {
        guard super.becomeFirstResponder() else { return false }
        if inputSourceObserver == nil {
            inputSourceObserver = NotificationCenter.default.addObserver(
                forName: NSTextInputContext.keyboardSelectionDidChangeNotification, object: nil, queue: .main
            ) { [weak self] _ in MainActor.assumeIsolated { self?.inputSourceChanged() } }
        }
        inputSource = inputContext?.selectedKeyboardInputSource
        return true
    }
    /// A source another process selects (an input source switcher, through
    /// `TISSelectInputSource`) while the view has the keys can leave WebKit's
    /// input context with no input method: keys then reach the page as the
    /// layout's characters and nothing composes until the view is focused
    /// again: after 6 of 8 switches to Korean 2-Set on macOS 27.2 (`gksskf`
    /// for 한날), none after the system's own input source shortcut (macOS 26).
    /// Deactivating and activating the context reconnects it: 26 of 26
    /// switches then composed. Only the current context is touched:
    /// activating another would take the input method from the view that has
    /// it; nor one holding marked text, which deactivating may commit or drop.
    /// A change skipped for either reason is reconnected at the next notice.
    private func inputSourceChanged() {
        guard window?.firstResponder === self else { return }
        // Without an editable element WebKit gives no context: the source is
        // unknown until the page focuses one again.
        guard let context = inputContext else { inputSource = nil; return }
        let source = context.selectedKeyboardInputSource
        guard source != inputSource, NSTextInputContext.current === context,
              (self as AnyObject as? NSTextInputClient)?.hasMarkedText() != true else { return }
        inputSource = source
        context.deactivate()
        context.activate()
    }
    override func layout() {
        super.layout()
        pinInsets()
    }
    func pinInsets() {
        setValue(false, forKey: "automaticallyAdjustsContentInsets")
        func walk(_ v: NSView) {
            if let s = v as? NSScrollView {
                if s.automaticallyAdjustsContentInsets { s.automaticallyAdjustsContentInsets = false }
                if s.contentInsets.top != 0 || s.contentInsets.left != 0 || s.contentInsets.bottom != 0 || s.contentInsets.right != 0 {
                    s.contentInsets = NSEdgeInsetsZero
                }
            }
            v.subviews.forEach(walk)
        }
        walk(self)
    }
}
#else
import UIKit
typealias PlatformImage = UIImage
typealias PlatformView = UIView
#endif

public typealias EventFn = @convention(c) (
    UnsafeMutableRawPointer?, UInt32, UInt32, UnsafePointer<UInt8>?, UInt32
) -> Void
public typealias ReplyFn = @convention(c) (
    UnsafeMutableRawPointer?, UInt32, UInt32, UInt32, UnsafePointer<UInt8>?, UInt32
) -> Void
/// The host's moment before a frame's web view is made (@ref LLP 1020.002
/// §9): the arm's context and the `WKWebViewConfiguration`, for the app's hatch.
public typealias ConfiguringFn = @convention(c) (UnsafeMutableRawPointer?, UnsafeMutableRawPointer?) -> Void
/// Receives an asset's bytes from `ReadFn`; not called for a missing asset.
public typealias SinkFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32) -> Void
/// The host's read of an app asset by name (`deck/app.js`), through the
/// session's resolver: the embedded bundle, a delivered generation, or the
/// dev URL's. Called off the main thread.
public typealias ReadFn = @convention(c) (UnsafePointer<UInt8>?, UInt32, UnsafeMutableRawPointer?, SinkFn?) -> Void
private var assetReader: ReadFn?

/// The app's assets as an origin, `exact-app://app/<name>`, so a bundled
/// page's relative URLs (scripts, styles, fonts, chunks, workers, wasm)
/// resolve to the files beside it, as they do beside the page on the web
/// (@ref LLP 1020.001 D1). WebKit refuses a handler for `http(s)`, so the
/// origin is a scheme of its own; WebKit treats it as a secure context.
/// One per web view: the configuration retains it, and it holds its arm
/// weakly.
final class AssetScheme: NSObject, WKURLSchemeHandler {
    static let scheme = "exact-app"
    static let origin = "exact-app://app"
    static let base = URL(string: "exact-app://app/")!
    static let reads = DispatchQueue(label: "exact.web.assets", qos: .userInitiated, attributes: .concurrent)
    /// What is served: the trees a web build publishes beside the app's
    /// page for a page to live in, never the rest of what the resolver can
    /// read (the bundle's resources, or the working directory of a bare
    /// development executable) (@ref LLP 1020.001 D2).
    static let trees = ["assets/", "deck/"]
    weak var arm: WebArm?
    /// Tasks WebKit stopped: a stopped task must not be answered.
    var stopped = Set<ObjectIdentifier>()

    func webView(_ webView: WKWebView, start task: WKURLSchemeTask) {
        guard let url = task.request.url else { return }
        // Only the arm's own web view showing a bundled page serves the
        // origin: a remote guest, or a popup WebKit built from a copy of this
        // configuration, cannot read the app's assets through it.
        guard let arm, arm.built === webView, arm.assetPage != nil else { return refuse(task, url, logging: false) }
        // The wrapper an iOS page needs is served at the page's own origin,
        // so a framed page keeps first-party storage, as a direct one does
        // (@ref LLP 1020.001 D4).
        if !arm.direct, url.absoluteString == arm.wrapperURL.absoluteString, let wrapper = arm.wrapperDocument {
            let body = Data(wrapper.utf8)
            task.didReceive(HTTPURLResponse(url: url, statusCode: 200, httpVersion: "HTTP/1.1", headerFields: [
                "Content-Type": "text/html; charset=utf-8", "Content-Length": String(body.count), "Cache-Control": "no-store"])!)
            task.didReceive(body)
            task.didFinish()
            return
        }
        // A direct page's own response carries its `sandbox`, as the
        // synthetic local document's does (@ref LLP 1020 §10).
        let sandbox = arm.direct && arm.assetPage?.absoluteString == url.absoluteString ? arm.sandbox : nil
        let range = task.request.value(forHTTPHeaderField: "Range")
        let id = ObjectIdentifier(task)
        let name = String(url.path.drop(while: { $0 == "/" }))
        // A file the app gave this page (@ref LLP 1020.001 D7).
        if url.host == "app", name.hasPrefix(FrameFiles.prefix) {
            guard let file = arm.files.file(name) else { return refuse(task, url, given: "is not a file this frame was given") }
            return serveGiven(task, url, file)
        }
        guard url.host == "app", AssetScheme.trees.contains(where: name.hasPrefix) else { return refuse(task, url, logging: true) }
        AssetScheme.reads.async {
            let bytes = AssetScheme.read(name)
            DispatchQueue.main.async { [weak self] in
                guard let self, self.stopped.remove(id) == nil else { return }
                guard let bytes else { return self.refuse(task, url, logging: true) }
                var headers = ["Content-Type": AssetScheme.type(url.pathExtension), "Accept-Ranges": "bytes", "Cache-Control": "no-cache"]
                if let sandbox { headers["Content-Security-Policy"] = "sandbox \(sandbox)" }
                var status = 200, body = bytes
                if let range, let (from, to) = AssetScheme.bytes(range, count: bytes.count) {
                    status = 206
                    body = bytes.subdata(in: from..<(to + 1))
                    headers["Content-Range"] = "bytes \(from)-\(to)/\(bytes.count)"
                }
                headers["Content-Length"] = String(body.count)
                task.didReceive(HTTPURLResponse(url: url, statusCode: status, httpVersion: "HTTP/1.1", headerFields: headers)!)
                task.didReceive(body)
                task.didFinish()
            }
        }
    }

    func webView(_ webView: WKWebView, stop task: WKURLSchemeTask) {
        stopped.insert(ObjectIdentifier(task))
    }

    func refuse(_ task: WKURLSchemeTask, _ url: URL, logging: Bool = false, given: String? = nil) {
        if logging {
            let trees = AssetScheme.trees.joined(separator: " or ")
            FileHandle.standardError.write(Data("exact: iframe \(arm?.src ?? ""): \(url.absoluteString) is not an app asset under \(trees)\n".utf8))
        }
        if let given { FileHandle.standardError.write(Data("exact: iframe \(arm?.src ?? ""): \(url.absoluteString) \(given)\n".utf8)) }
        // A given file's refusal carries its responses' sandbox, as they all do.
        let headers = given == nil ? ["Content-Length": "0"] : ["Content-Length": "0", "Content-Security-Policy": FrameFiles.sandbox]
        task.didReceive(HTTPURLResponse(url: url, statusCode: 404, httpVersion: "HTTP/1.1", headerFields: headers)!)
        task.didFinish()
    }

    static func read(_ name: String) -> Data? {
        guard let assetReader else { return nil }
        final class Box { var data: Data? }
        let box = Box()
        let sink: SinkFn = { context, bytes, length in
            Unmanaged<Box>.fromOpaque(context!).takeUnretainedValue().data = Data(UnsafeBufferPointer(start: bytes, count: Int(length)))
        }
        let utf8 = Array(name.utf8)
        withExtendedLifetime(box) {
            utf8.withUnsafeBufferPointer { assetReader($0.baseAddress, UInt32($0.count), Unmanaged.passUnretained(box).toOpaque(), sink) }
        }
        return box.data
    }

    /// The types a browser is strict about are named exactly: a module
    /// script refuses any other JavaScript type, and streaming wasm
    /// compilation requires `application/wasm`.
    static func type(_ ext: String) -> String {
        switch ext.lowercased() {
        case "js", "mjs": return "text/javascript; charset=utf-8"
        case "wasm": return "application/wasm"
        case "html", "htm": return "text/html; charset=utf-8"
        case "css": return "text/css; charset=utf-8"
        case "json", "map": return "application/json"
        default:
            let type = UTType(filenameExtension: ext)?.preferredMIMEType ?? "application/octet-stream"
            return type.hasPrefix("text/") ? "\(type); charset=utf-8" : type
        }
    }

    /// One `bytes=` range (RFC 9110 §14.1.2), as media elements ask for.
    static func bytes(_ header: String, count: Int) -> (Int, Int)? {
        guard count > 0, header.hasPrefix("bytes="), !header.contains(",") else { return nil }
        let parts = header.dropFirst(6).split(separator: "-", maxSplits: 1, omittingEmptySubsequences: false)
        guard parts.count == 2 else { return nil }
        if parts[0].isEmpty {
            guard let suffix = Int(parts[1]), suffix > 0 else { return nil }
            return (max(0, count - suffix), count - 1)
        }
        guard let from = Int(parts[0]), from < count else { return nil }
        let to = parts[1].isEmpty ? count - 1 : min(Int(parts[1]) ?? count - 1, count - 1)
        return to >= from ? (from, to) : nil
    }
}

final class WebArm: NSObject, WKScriptMessageHandler, WKNavigationDelegate {
    let id: UInt32
    let context: UnsafeMutableRawPointer?
    let event: EventFn
    let reply: ReplyFn
    var controller = WKUserContentController()
    let world = WKContentWorld.world(name: "exact.agent")
    /// The web view, made at the first use, once `partition` is known: a
    /// partition is fixed when a web view is made, so changing it makes
    /// another (@ref LLP 1020.002 D1).
    var built: WKWebView?
    var webView: WKWebView { built ?? build() }
    /// A local PDF's view, shown in place of the web view (@ref LLP 1020.002 D10).
    var pdf: FramePDF?
    /// What the frame shows: the PDF view, or the web view. A frame said to
    /// show a local PDF (`pdf` in its configuration) shows the PDF's view
    /// from the start, before its file comes: it makes no web view that
    /// `showPDF` would drop at once (@ref LLP 1020.002 D10). A file that is
    /// not one goes the way any does (`closePDF`, a new view at kind 7).
    var platformView: PlatformView { pdf?.view ?? (startsAsPDF ? waitingPDF().view : webView) }
    private var startsAsPDF: Bool { built == nil && options["pdf"] as? Bool == true && partition == nil && sandbox == nil }
    /// The frame's dialogs, popups and external-scheme confirmations (D5):
    /// a web view's delegate, and a PDF's links' confirmation without one.
    @discardableResult func makeUI() -> FrameUI {
        if let ui { return ui }
        #if os(iOS)
        let made = agent ? FrameUIAgent(arm: self) : FrameUI(arm: self)
        #else
        let made = FrameUI(arm: self)
        #endif
        ui = made
        return made
    }
    private func waitingPDF() -> FramePDF {
        makeUI() // a PDF's link to another app asks through it (D5), with no web view built
        let shown = FramePDF(arm: self)
        shown.view.zoom = CGFloat((options["zoom"] as? NSNumber)?.doubleValue ?? 1)
        pdf = shown
        return shown
    }
    #if EXACT_DEVELOPMENT
    /// What the arm owns that the app's hatch changed, each said once (§9).
    var ownedSaid = Set<String>()
    #endif
    /// How many user scripts the arm put in the web view's content controller.
    var ownScripts = 0
    /// The host's moment before each web view is made (`exact_web_set_configuring`).
    var configuringHook: ConfiguringFn?
    /// The data store the arm gave the web view's configuration.
    weak var ownStore: WKWebsiteDataStore?
    /// A partitioned frame's name; its guest is a top-level page in that
    /// partition's data store, with no message bridge, no `exact-app` scheme
    /// and no script in the page's world (@ref LLP 1020.002 D1).
    var partition: String?
    var agent = false
    /// The session's app and the session (`exact_web_configure`): a
    /// persistent partition is the app's, an in-memory one the session's.
    var app = Bundle.main.bundleIdentifier ?? "exact"
    var session = ""
    var options: [String: Any] = [:]
    /// The words `applyOptions` set on the current web view.
    var applied = Set<String>()
    var browser: FrameBrowser?
    /// The frame's element id and its session's module folder, by which
    /// `exact_web_automation` finds it, and its automation once driven (@ref LLP 1020.002 D9).
    var element = ""
    var owner = ""
    var automation: FrameAutomation?
    /// The web view's `WKUIDelegate`: its popups, dialogs, file inputs,
    /// downloads and permissions (@ref LLP 1020.002 D5).
    var ui: FrameUI?
    /// The frame handles `newwindow=`: only then does it ask for popups.
    var handlesNewWindow = false
    /// The frame is marked `hatch`: its hatch writes on its configuration.
    var hatched = false
    /// Requests held for the agent (`ask`), by token.
    var asked: [UInt32: (web: ObjectIdentifier?, done: ([String: Any]?) -> Void)] = [:]
    var nextAsk: UInt32 = 1
    /// The `about:blank` load shown in place of a refused source: its own
    /// navigation reports nothing, and one it cancels still ends as itself.
    var blank: WKNavigation?
    /// Whether `navigation` is that load: a nil one never is.
    func isBlank(_ navigation: WKNavigation?) -> Bool { blank != nil && navigation === blank }
    var src: String?
    /// A local `src`'s bytes, as the host read them (`exact_web_set_document`).
    var suppliedDocument: Data?
    var sandbox: String?
    var srcInitialized = false
    var sandboxInitialized = false
    var servePending = false
    var serving = false
    var generation = 0
    var guestFrame: WKFrameInfo?
    var suppressLoad = false
    var recovering = false
    var invalidated = false
    /// The guest is the web view's own document, not a frame in a wrapper
    /// (see `serve`).
    var direct = false
    /// A direct guest navigated itself: its later documents' messages are
    /// not the app's (the wrapper's `navigated`).
    var documentNonce: String?
    var revoked = false
    /// The origin a direct guest's messages must come from.
    var expectedOrigin: String?
    var pageBridge = false
    /// The guest is a bundled page: its failed sub-resources are logged.
    var localGuest = false
    /// A bundled HTML page's URL under `AssetScheme`, while one is served.
    var assetPage: URL?
    /// The app files that page was given (@ref LLP 1020.001 D7).
    let files = FrameFiles()
    /// The wrapper `AssetScheme` serves around such a page on iOS.
    var wrapperDocument: String?
    /// A bundled page's origin: loopback HTTP, a secure context (as the web
    /// dev loop's `http://127.0.0.1` page is) that WebKit holds to no
    /// mixed-content blocking. Under an `https:` origin WebKit refuses every
    /// `http:` sub-resource, loopback included, which Chrome loads (#135).
    static let localOrigin = "http://exact.localhost"
    static let template: WKWebViewConfiguration = {
        let template = WKWebViewConfiguration()
        template.preferences = preferences()
        _ = template.websiteDataStore
        _ = template.defaultWebpagePreferences
        return template
    }()
    /// Unsaid, as Safari has them (@ref LLP 1020.002 D1, D5): a scripted
    /// window needs a person's gesture, and a page's `requestFullscreen()`
    /// takes the screen.
    static func preferences() -> WKPreferences {
        let preferences = WKPreferences()
        preferences.javaScriptCanOpenWindowsAutomatically = false
        preferences.isElementFullscreenEnabled = true
        return preferences
    }
    var wrapperURL: URL

    init(id: UInt32, context: UnsafeMutableRawPointer?, event: @escaping EventFn, reply: @escaping ReplyFn) {
        self.id = id
        self.context = context
        self.event = event
        self.reply = reply
        self.wrapperURL = URL(string: "https://exact.invalid/frame/\(id)/index.html")!
        super.init()
    }

    func build() -> WKWebView {
        controller = WKUserContentController()
        let web = partition == nil ? buildFrame() : buildPartitioned()
        built = web
        web.navigationDelegate = self
        web.uiDelegate = makeUI()
        #if EXACT_DEVELOPMENT
        // Safari's platform inspector sees only this guest, not Exact's
        // native tree. Production arms never compile this opt-in (#101).
        web.isInspectable = true
        #endif
        #if os(macOS)
        (web as? ExactWebView)?.pinInsets()
        #else
        web.scrollView.contentInsetAdjustmentBehavior = .never
        web.scrollView.contentInset = .zero
        #endif
        applied = []
        applyOptions(to: web)
        // Preload scripts are a partitioned frame's: an unpartitioned one may be a wrapper's child.
        if partition != nil { automation?.attached(controller) }
        ownScripts = controller.userScripts.count
        return web
    }

    func buildFrame() -> WKWebView {
        // A copy of one shared configuration, not a fresh one: a fresh
        // configuration builds its preferences, visited-link store and
        // page defaults again for every row (`ensureLazyInitializedRefs`,
        // 3–4 ms a view). The copy shares only what every iframe on one web
        // page shares: the default website data store (same-origin storage
        // and cookies) and default preferences. The script bridge stays per
        // view (@ref LLP 1020 D2, LLP 1068 §0.1 Q3: views are not pooled).
        let configuration = WebArm.template.copy() as! WKWebViewConfiguration
        // A copy shares the template's preference objects: a frame whose
        // hatch may write on them, or that says its own scheduling policy,
        // has its own, so no other frame's change.
        if hatched || options["inactiveSchedulingPolicy"] != nil {
            configuration.preferences = WebArm.preferences()
            configuration.defaultWebpagePreferences = WKWebpagePreferences()
        }
        configuration.userContentController = controller
        let assets = AssetScheme()
        configuration.setURLSchemeHandler(assets, forURLScheme: AssetScheme.scheme)
        assets.arm = self
        controller.add(self, contentWorld: world, name: "exactAgent")
        controller.add(self, contentWorld: world, name: "exactFrame")
        // A direct guest's `parent` is its own window: what it posts to
        // `parent` arrives as a message from itself. The agent world takes
        // it before the page's listeners, as an iframe's post never reaches
        // its own window, and hands it to the app (@ref LLP 1020 D2).
        controller.addUserScript(WKUserScript(
            source: "(() => { const replies = new WeakSet(), nonce = Array.from(crypto.getRandomValues(new Uint32Array(4)), n => n.toString(16)).join('-'); globalThis.__exactHostReply = (text, expected) => { if (expected !== nonce) return; const e = new MessageEvent('message', {data:text, source:window}); replies.add(e); window.dispatchEvent(e); }; addEventListener('message', e => { if (replies.has(e) || e.source !== window) return; e.stopImmediatePropagation(); let p = e.data; if (typeof p !== 'string') { try { p = JSON.stringify(p) } catch { return } } if (typeof p === 'string') window.webkit.messageHandlers.exactFrame.postMessage(p) }, true); window.webkit.messageHandlers.exactAgent.postMessage({document:nonce}); })()",
            injectionTime: .atDocumentStart,
            forMainFrameOnly: true,
            in: world))
        controller.addUserScript(WKUserScript(
            source: "if (window.parent === window.top && window !== window.top) window.webkit.messageHandlers.exactAgent.postMessage('ready')",
            injectionTime: .atDocumentStart,
            forMainFrameOnly: false,
            in: world))
        // A sub-resource WebKit refuses or fails to load fires `error` on its
        // element and is otherwise silent; for a bundled page the host logs it.
        controller.addUserScript(WKUserScript(
            source: "addEventListener('error', e => { const t = e.target; if (!(t instanceof Element)) return; const u = t.currentSrc || t.src || t.href; if (typeof u === 'string' && u) window.webkit.messageHandlers.exactAgent.postMessage({ failed: u }) }, true)",
            injectionTime: .atDocumentStart,
            forMainFrameOnly: false,
            in: world))
        configuring(configuration)
        #if os(macOS)
        let webView: WKWebView = ExactWebView(frame: .zero, configuration: configuration)
        #else
        let webView = WKWebView(frame: .zero, configuration: configuration)
        #endif
        // The guest is a leaf the kernel already framed. WKWebView's default
        // is to inset itself for the titlebar / safe area, which leaves a
        // black strip of `underPageBackgroundColor` over the top of the
        // deck and over siblings (the account mark). Off: the iframe fills
        // the node's box (@ref LLP 1020 D1).
        #if os(macOS)
        webView.setValue(false, forKey: "drawsBackground")
        if #available(macOS 12.0, *) { webView.underPageBackgroundColor = .clear }
        #else
        webView.isOpaque = false
        webView.backgroundColor = .clear
        webView.scrollView.backgroundColor = .clear
        if #available(iOS 15.0, *) { webView.underPageBackgroundColor = .clear }
        #endif
        return webView
    }

    /// A partitioned frame's web view: its own preferences (the template's
    /// are shared by every frame), the partition's store, and only the icon
    /// reader. Unsaid, as Safari has them: the page draws its own
    /// background, a scripted `window.open` needs a person's gesture, and
    /// on iOS a `playsinline` video plays in the tab (@ref LLP 1020.002 D1).
    func buildPartitioned() -> WKWebView {
        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = Partitions.store(partition!, agent: agent, app: app, session: session)
        configuration.userContentController = controller
        let preferences = WebArm.preferences()
        if #available(macOS 14.0, iOS 17.0, *) { preferences.inactiveSchedulingPolicy = .throttle }
        configuration.preferences = preferences
        #if os(iOS)
        configuration.allowsInlineMediaPlayback = true
        #endif
        let browser = FrameBrowser(emit: { [weak self] name, record in self?.emitNavigation(name, record) },
                                   source: { [weak self] in self?.src })
        browser.external = { [weak self] url in
            guard let self, let web = self.built else { return }
            self.ui?.confirmExternal(url, from: web, byApp: true)
        }
        browser.install(on: controller)
        self.browser = browser
        configuring(configuration)
        #if os(macOS)
        let webView: WKWebView = ExactWebView(frame: .zero, configuration: configuration)
        #else
        let webView = WKWebView(frame: .zero, configuration: configuration)
        #endif
        browser.attach(webView)
        return webView
    }

    /// The app's hatch has the configuration before the web view is made
    /// (@ref LLP 1020.002 §9), its data store, preferences and content
    /// worlds set: what it adds (an `applicationNameForUserAgent`, content
    /// rule lists, its own URL schemes) is the new web view's.
    func configuring(_ configuration: WKWebViewConfiguration) {
        ownStore = configuration.websiteDataStore
        configuringHook?(context, Unmanaged.passUnretained(configuration).toOpaque())
    }

    /// The `apple.*` words and `zoom` (@ref LLP 1020.002 §9, D6). A word
    /// never said leaves WebKit's default; one said and then dropped goes
    /// back to it.
    func applyOptions(to web: WKWebView) {
        let said = { (key: String) in self.options[key] != nil || self.applied.contains(key) }
        if said("zoom") { web.pageZoom = CGFloat((options["zoom"] as? NSNumber)?.doubleValue ?? 1) }
        if said("allowsBackForwardNavigationGestures") { web.allowsBackForwardNavigationGestures = options["allowsBackForwardNavigationGestures"] as? Bool ?? false }
        #if os(macOS)
        if said("allowsMagnification") { web.allowsMagnification = options["allowsMagnification"] as? Bool ?? false }
        #endif
        // Never on the template's shared preferences: a policy said after the
        // web view was made with them waits for its next one.
        if #available(macOS 14.0, iOS 17.0, *), said("inactiveSchedulingPolicy"), web.configuration.preferences !== WebArm.template.preferences {
            switch options["inactiveSchedulingPolicy"] as? String {
            case "suspend": web.configuration.preferences.inactiveSchedulingPolicy = .suspend
            case "none": web.configuration.preferences.inactiveSchedulingPolicy = .none
            // `throttle` is a partitioned frame's unsaid policy (D1); WebKit's default, `suspend`, an unpartitioned one's.
            default: web.configuration.preferences.inactiveSchedulingPolicy = partition != nil || options["inactiveSchedulingPolicy"] != nil ? .throttle : .suspend
            }
        }
        applied = Set(options.keys).subtracting(["partition", "agent", "app", "session", "newwindow"])
    }

    /// The app's hatch has the web view or PDF view (@ref LLP 1020.002 §9):
    /// a development build says by name what it changed of what the arm owns,
    /// as LLP 1133 D3 detects and does not enforce. A script message handler
    /// a hatch adds is not detected: WebKit lists no handlers.
    func checkOwned() {
        #if EXACT_DEVELOPMENT
        var changed: [String] = []
        if let pdf {
            if pdf.view.delegate !== pdf { changed.append("the PDF view's delegate") }
        } else if let web = built {
            if web.navigationDelegate !== self { changed.append("navigationDelegate") }
            if web.uiDelegate !== ui { changed.append("uiDelegate") }
            if web.configuration.websiteDataStore !== ownStore { changed.append("websiteDataStore") }
            if web.configuration.userContentController !== controller { changed.append("userContentController") }
            if controller.userScripts.count != ownScripts { changed.append("the user scripts of the arm's content controller") }
            if abs(Double(web.pageZoom) - ((options["zoom"] as? NSNumber)?.doubleValue ?? 1)) > 0.001 { changed.append("pageZoom") }
            #if os(macOS)
            if web.appearance != nil { changed.append("appearance") }
            #else
            if web.overrideUserInterfaceStyle != .unspecified { changed.append("overrideUserInterfaceStyle") }
            #endif
        }
        for property in changed where ownedSaid.insert(property).inserted {
            FileHandle.standardError.write(Data("exact: iframe \(id): the app's hatch changed \(property), which the web arm owns (LLP 1020.002 §9)\n".utf8))
        }
        #endif
    }

    /// `partition`, `zoom` and the `apple.*` words, before the web view is
    /// first made and whenever they change. A new partition replaces the web
    /// view; the presenter takes the new one from `exact_web_platform_view`.
    func configure(_ values: [String: Any]) {
        agent = values["agent"] as? Bool ?? false
        if let app = values["app"] as? String, !app.isEmpty { self.app = app }
        session = values["session"] as? String ?? ""
        handlesNewWindow = values["newwindow"] as? Bool ?? false
        hatched = values["hatched"] as? Bool ?? false
        element = values["element"] as? String ?? ""
        owner = values["owner"] as? String ?? ""
        Automations.arms.add(self)
        let name = values["partition"] as? String
        options = values
        pdf?.view.zoom = CGFloat((options["zoom"] as? NSNumber)?.doubleValue ?? 1)
        guard name != partition || (built == nil && pdf == nil) else {
            if let built { applyOptions(to: built) }
            return
        }
        let replacing = built != nil || pdf != nil
        if replacing { teardown() }
        partition = name
        if replacing {
            // A local PDF's frame losing its partition shows the PDF's view, and makes no web view for `serve` to drop.
            if startsAsPDF { _ = waitingPDF() } else { _ = build(); closePDF() }
            markLoading()
            scheduleServe()
        }
    }

    /// Lets the current web view go: its navigation in flight ends first.
    func teardown() {
        guard let old = built else { return }
        if let browser, let flight = browser.flight {
            browser.flight = nil
            browser.emitEnd(flight, "navigateerror", error: "AbortError", message: "the frame's partition changed")
        }
        browser?.close(controller: controller)
        browser = nil
        automation?.end("iframe \(element)'s partition changed")
        ui?.close()
        ui?.forget(old)
        endAsked()
        old.stopLoading()
        old.navigationDelegate = nil
        old.uiDelegate = nil
        setPageBridge(false)
        controller.removeScriptMessageHandler(forName: "exactAgent", contentWorld: world)
        controller.removeScriptMessageHandler(forName: "exactFrame", contentWorld: world)
        built = nil
    }

    deinit {
        invalidate()
    }

    func invalidate() {
        guard !invalidated else { return }
        invalidated = true
        files.end()
        automation?.end("iframe \(element) unmounted")
        Automations.arms.remove(self)
        pdf?.close()
        browser?.close(controller: controller)
        ui?.close()
        ui?.forget(built)
        endAsked()
        built?.uiDelegate = nil
        built?.stopLoading()
        setPageBridge(false)
        controller.removeScriptMessageHandler(forName: "exactAgent", contentWorld: world)
        controller.removeScriptMessageHandler(forName: "exactFrame", contentWorld: world)
        built?.navigationDelegate = nil
    }

    func setSrc(_ value: String?) {
        guard !srcInitialized || src != value else { return }
        srcInitialized = true
        src = value
        // The app moved the frame: what a person decided about the old page no longer holds.
        ui?.forget(built)
        markLoading()
        scheduleServe()
    }

    func setSandbox(_ value: String?) {
        guard !sandboxInitialized || sandbox != value else { return }
        sandboxInitialized = true
        sandbox = value
        markLoading()
        // Sandbox is immutable per mount in v1 (@ref LLP 1020 D2).
        scheduleServe()
    }

    func markLoading() {
        guestFrame = nil
        documentNonce = nil
        emit(kind: 3)
    }

    /// A partitioned frame's source: a web page loaded as itself, or
    /// `about:blank` and `navigateerror` for one it refuses: a `sandbox`
    /// (WebKit cannot sandbox a top-level page) or a source that is not a web
    /// page (a bundled or `app:/` file would need the `exact-app` scheme a
    /// remote page must not reach) (@ref LLP 1020.002 D1).
    func servePartitioned() {
        generation += 1
        guestFrame = nil
        documentNonce = nil
        revoked = false
        direct = true
        assetPage = files.give(nil, source: nil)
        localGuest = false
        expectedOrigin = nil
        let refusal: String?
        if !Partitions.valid(partition!) {
            refusal = "partition \"\(partition!)\" is not a partition name: persist: and 1 to 64 of A-Z a-z 0-9 . _ -, or the same without persist:"
        } else if sandbox != nil {
            refusal = "an iframe with a partition takes no sandbox: WebKit cannot sandbox a top-level page"
        } else if let src, GuestURL.web(src) == nil {
            refusal = "a partitioned frame shows web pages; \(src) is not an http, https or about:blank URL"
        } else {
            refusal = nil
        }
        guard let refusal else {
            serving = true
            webView.load(URLRequest(url: src.flatMap(GuestURL.web) ?? URL(string: "about:blank")!))
            return
        }
        FileHandle.standardError.write(Data("exact: iframe \(id): \(refusal)\n".utf8))
        if let browser, let flight = browser.flight {
            browser.flight = nil
            browser.emitEnd(flight, "navigateerror", error: "AbortError", message: "superseded")
        }
        blank = webView.load(URLRequest(url: URL(string: "about:blank")!))
        emit(kind: 2)
        browser?.send("navigateerror", error: Partitions.valid(partition!) ? "NotSupportedError" : "SyntaxError", message: refusal)
    }

    /// A guest's `newwindow` or `download` record for the presenter: kind 5,
    /// `{event, record}` (@ref LLP 1020.002 D5).
    func emitRecord(_ name: String, _ record: [String: Any]) {
        guard let data = try? JSONSerialization.data(withJSONObject: ["event": name, "record": record]) else { return }
        emit(kind: 5, text: String(decoding: data, as: UTF8.self))
    }

    /// Under the agent, a guest's dialog, file input or download destination
    /// is a request the drive answers (@ref LLP 1020.002 §10): kind 6,
    /// `{token, capability, args}`; `exact_web_answer` delivers the answer,
    /// and nil is Cancel (also when the frame goes first).
    func ask(_ capability: String, _ args: [String: Any], from web: WKWebView?, done: @escaping ([String: Any]?) -> Void) {
        guard !invalidated else { return done(nil) }
        let token = nextAsk
        nextAsk &+= 1
        asked[token] = (web.map(ObjectIdentifier.init), done)
        guard let data = try? JSONSerialization.data(withJSONObject: ["token": token, "capability": capability, "args": args]) else {
            asked[token] = nil
            return done(nil)
        }
        emit(kind: 6, text: String(decoding: data, as: UTF8.self))
    }

    /// Held requests end unanswered with their web view (`web`, a closed
    /// popup's; nil, every one): a dialog as Cancel, a panel as nothing chosen.
    func endAsked(for web: WKWebView? = nil) {
        var ending: [([String: Any]?) -> Void] = []
        for (token, held) in asked where web == nil || held.web == web.map(ObjectIdentifier.init) {
            asked[token] = nil
            ending.append(held.done)
        }
        for done in ending { done(nil) }
    }

    func answer(_ value: [String: Any]) {
        guard let token = (value["token"] as? NSNumber)?.uint32Value, let held = asked.removeValue(forKey: token) else { return }
        held.done(value["cancel"] as? Bool == true ? nil : value)
    }

    /// A `FrameNavigation` record for the presenter: kind 4, `{event, record}`.
    func emitNavigation(_ name: String, _ record: [String: Any]) {
        guard let data = try? JSONSerialization.data(withJSONObject: ["event": name, "record": record]) else { return }
        emit(kind: 4, text: String(decoding: data, as: UTF8.self))
    }

    func command(_ op: [String: Any]) {
        guard !invalidated else { return }
        checkOwned()
        // Any frame's, a PDF one's too (@ref LLP 1020.002 D7); the inspector any web view's (D11).
        if op["op"] as? String == "captureScreenshot" { return capture(op) }
        if op["op"] as? String == "openDevTools" { return openDevTools() }
        guard let browser else {
            // An unpartitioned frame may be a child of a wrapper document
            // (a sandboxed page; a remote one on iOS): only a partitioned
            // frame is always a top-level browsing context to move.
            let record: [String: Any] = ["url": "", "destination": "", "navigationId": 0, "navigationType": "push", "title": "", "icons": [String](),
                                         "canGoBack": false, "canGoForward": false, "loading": false, "partitioned": true, "controller": "person",
                                         "error": "NotSupportedError", "message": "\(op["op"] as? String ?? "command"): iframe \(id) has no partition", "recovering": false]
            FileHandle.standardError.write(Data("exact: \(record["message"]!)\n".utf8))
            return emitNavigation("navigateerror", record)
        }
        if op["op"] as? String == "navigate" { ui?.forget(built) }
        browser.command(op)
    }

    func scheduleServe() {
        guard !invalidated, !servePending else { return }
        servePending = true
        DispatchQueue.main.async { [weak self] in
            guard let self, !self.invalidated, self.servePending else { return }
            self.servePending = false
            self.serve()
        }
    }

    /// One document per row, as a platform web view is used: a local
    /// document is the web view's own document, a bundled page at its
    /// `AssetScheme` URL and an app file at the synthetic origin (its
    /// `sandbox` as a CSP `sandbox` header, which WebKit enforces on
    /// a top-level document, opaque origin included), and an unsandboxed
    /// http(s) `src` is loaded as itself. Only a sandboxed remote `src`
    /// keeps the wrapper and its inner `<iframe>`: WebKit cannot sandbox a
    /// top-level network load, and approximating it is the fail-open
    /// class D2 removed (@ref LLP 1020 D2, §10).
    func serve(error: String? = nil) {
        guard !invalidated else { return }
        checkOwned()
        if partition == nil, error == nil, sandbox == nil, let file = src.flatMap(localFile), file.1 == "application/pdf", showPDF(file.0) { return }
        closePDF()
        if partition != nil { return servePartitioned() }
        generation += 1
        guestFrame = nil
        documentNonce = nil
        serving = true
        revoked = false
        if error == nil, let file = src.flatMap(localFile) {
            serveFile(file.0, type: file.1)
            return
        }
        let local = error.map(errorDocument) ?? src.flatMap(localDocument)
        let remote = local == nil ? src.flatMap(remoteSource) : nil
        let web = remote.flatMap(URL.init(string:)).flatMap { ["http", "https"].contains($0.scheme?.lowercased() ?? "") ? $0 : nil }
        // An HTTPS wrapper would block an HTTP guest as mixed content even
        // when the app explicitly allows it through ATS. Match the remote
        // HTTP guest's scheme; WebKit still enforces ATS and the iframe's
        // sandbox. A local document is served at `localOrigin`.
        let origin = local != nil ? WebArm.localOrigin
            : web?.scheme?.lowercased() == "http" ? "http://exact.invalid" : "https://exact.invalid"
        wrapperURL = URL(string: "\(origin)/frame/\(id)/index.html")!
        localGuest = local != nil && error == nil
        // Every serve gives the page's files anew, so the last serve's end.
        assetPage = files.give(localGuest ? src.flatMap(WebArm.assetURL) : nil, source: src)
        if assetPage != nil { wrapperURL = URL(string: "\(AssetScheme.origin)/.exact/frame/\(id)/index.html")! }
        #if os(iOS)
        // iOS lays a top-level document out by its viewport `<meta>` (980
        // CSS px without one); a frame ignores it and takes its box. Only a
        // document that asks for its box's width is laid out the same
        // either way, so only such a local document goes direct here.
        direct = local.map(WebArm.fitsItsBox) ?? (remote == nil)
        #else
        // macOS ignores the viewport `<meta>`, as a frame does.
        direct = local != nil || (web != nil && sandbox == nil) || (local == nil && remote == nil)
        #endif
        setPageBridge(!direct)
        guard direct else {
            let request = URLRequest(url: wrapperURL, cachePolicy: .reloadIgnoringLocalCacheData)
            if assetPage != nil {
                wrapperDocument = wrapper(error: nil)
                webView.load(request)
            } else {
                webView.loadSimulatedRequest(request, responseHTML: wrapper(error: error))
            }
            return
        }
        expectedOrigin = guestOrigin(remote: remote, local: local != nil)
        if let web, local == nil {
            webView.load(URLRequest(url: web))
            return
        }
        if let assetPage {
            webView.load(URLRequest(url: assetPage, cachePolicy: .reloadIgnoringLocalCacheData))
            return
        }
        var headers = ["Content-Type": "text/html; charset=utf-8"]
        if let sandbox { headers["Content-Security-Policy"] = "sandbox \(sandbox)" }
        let response = HTTPURLResponse(url: wrapperURL, statusCode: 200, httpVersion: "HTTP/1.1", headerFields: headers)!
        webView.loadSimulatedRequest(URLRequest(url: wrapperURL, cachePolicy: .reloadIgnoringLocalCacheData),
                                     response: response, responseData: Data((local ?? "").utf8))
    }

    /// A non-HTML local file (plain text, a PDF, an image) is the web view's
    /// own document under its content type, as Chrome shows such
    /// a frame by the type its server sent: WebKit's PDF view for a PDF, not
    /// its bytes as HTML text (#115, @ref LLP 1020 §10).
    func serveFile(_ bytes: Data, type: String) {
        let url = URL(string: "\(WebArm.localOrigin)/frame/\(id)/index.html")!
        wrapperURL = url
        localGuest = true
        assetPage = files.give(nil, source: nil)
        direct = true
        setPageBridge(false)
        expectedOrigin = guestOrigin(remote: nil, local: true)
        var headers = ["Content-Type": type.hasPrefix("text/") ? "\(type); charset=utf-8" : type]
        if let sandbox { headers["Content-Security-Policy"] = "sandbox \(sandbox)" }
        let response = HTTPURLResponse(url: url, statusCode: 200, httpVersion: "HTTP/1.1", headerFields: headers)!
        webView.loadSimulatedRequest(URLRequest(url: url, cachePolicy: .reloadIgnoringLocalCacheData), response: response, responseData: bytes)
    }

    /// Whether a document's viewport `<meta>` asks for the device width at
    /// scale 1: what a frame of its box gives it.
    static func fitsItsBox(_ html: String) -> Bool {
        guard let tag = html.range(of: #"<meta[^>]*name\s*=\s*["']?viewport["']?[^>]*>"#, options: [.regularExpression, .caseInsensitive]),
              let content = html[tag].range(of: #"content\s*=\s*("[^"]*"|'[^']*')"#, options: [.regularExpression, .caseInsensitive])
        else { return false }
        let pairs = html[tag][content].lowercased().filter { !$0.isWhitespace && $0 != "\"" && $0 != "'" }
            .dropFirst("content=".count).split(whereSeparator: { $0 == "," || $0 == ";" })
        var width: Substring?, scale: Substring?
        for pair in pairs {
            let kv = pair.split(separator: "=", maxSplits: 1)
            guard kv.count == 2 else { continue }
            if kv[0] == "width" { width = kv[1] } else if kv[0] == "initial-scale" { scale = kv[1] }
        }
        return width == "device-width" && (scale == nil || Double(scale!) == 1)
    }

    /// The wrapper's page-world bridge exists only while a wrapper is
    /// served: a direct guest is the main frame and must not reach it.
    func setPageBridge(_ on: Bool) {
        guard on != pageBridge else { return }
        pageBridge = on
        if on { controller.add(self, name: "exact") } else { controller.removeScriptMessageHandler(forName: "exact") }
    }

    /// An origin as `guestOrigin` spells it.
    static func spelled(_ o: WKSecurityOrigin) -> String {
        guard !o.protocol.isEmpty else { return "null" }
        let host = o.host.contains(":") ? "[\(o.host)]" : o.host
        let port = o.port == 0 || (o.protocol == "http" && o.port == 80) || (o.protocol == "https" && o.port == 443) ? "" : ":\(o.port)"
        return "\(o.protocol)://\(host)\(port)"
    }

    func wrapper(error: String?) -> String {
        let local = error.map(errorDocument) ?? src.flatMap(localDocument)
        let remote = local == nil ? src.flatMap(remoteSource) : nil
        let source = (error == nil ? assetPage : nil).map { " src=\"\(attribute($0.absoluteString))\"" }
            ?? local.map { " srcdoc=\"\(attribute($0))\"" }
            ?? remote.map { " src=\"\(attribute($0))\"" }
            ?? ""
        let restriction = sandbox.map { " sandbox=\"\(attribute($0))\"" } ?? ""
        let expectedOrigin = javascript(guestOrigin(remote: remote, local: local != nil))
        return """
        <!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
        <style>html,body,iframe{margin:0;width:100%;height:100%;border:0;display:block}body{overflow:hidden}</style>
        <iframe id="exact-frame"\(source)\(restriction)></iframe>
        <script>
        (() => {
          const inner = document.getElementById('exact-frame');
          const guestOrigin = \(expectedOrigin);
          let committed = false;
          let connected = false;
          let navigated = false;
          window.__exactRevokeGuest = () => { navigated = true; };
          window.__exactHostReply = (text, expectedGeneration) => {
            if (expectedGeneration !== \(generation) || navigated || (!committed && !connected) || guestOrigin === null) return;
            inner.contentWindow.postMessage(text, guestOrigin === 'null' ? '*' : guestOrigin);
          };
          addEventListener('message', event => {
            if (event.source !== inner.contentWindow) return;
            if (navigated || guestOrigin === null || event.origin !== guestOrigin) return;
            connected = true;
            let payload = event.data;
            if (typeof payload !== 'string') {
              try { payload = JSON.stringify(payload); } catch { return; }
            }
            if (typeof payload !== 'string') return;
            webkit.messageHandlers.exact.postMessage(JSON.stringify({kind:'message', generation:\(generation), payload}));
          });
          inner.addEventListener('load', () => {
            if (committed) navigated = true;
            committed = true;
            webkit.messageHandlers.exact.postMessage(JSON.stringify({kind:'load', generation:\(generation)}));
          });
        })();
        </script>
        """
    }

    func remoteSource(_ source: String) -> String? {
        guard let url = URL(string: source), let scheme = url.scheme?.lowercased() else { return nil }
        return ["http", "https", "data", "about", "blob"].contains(scheme) ? source : nil
    }

    func guestOrigin(remote: String?, local: Bool) -> String? {
        let tokens = Set((sandbox ?? "").split(whereSeparator: { $0.isWhitespace }).map(String.init))
        if sandbox != nil, !tokens.contains("allow-same-origin") { return "null" }
        if local { return assetPage == nil ? WebArm.localOrigin : AssetScheme.origin }
        guard let remote, let url = URL(string: remote),
              let scheme = url.scheme?.lowercased(),
              (scheme == "http" || scheme == "https"),
              let host = url.host?.lowercased() else { return nil }
        let shownHost = host.contains(":") ? "[\(host)]" : host
        let defaultPort = scheme == "http" ? 80 : 443
        let port = url.port.flatMap { $0 == defaultPort ? nil : ":\($0)" } ?? ""
        return "\(scheme)://\(shownHost)\(port)"
    }

    func javascript(_ value: String?) -> String {
        guard let data = try? JSONSerialization.data(withJSONObject: value as Any, options: .fragmentsAllowed),
              let text = String(data: data, encoding: .utf8) else { return "null" }
        return text.replacingOccurrences(of: "<", with: "\\u003c")
    }

    /// HTML keeps the box-width wrapper an iOS frame needs (`fitsItsBox`).
    /// Every other known type keeps its MIME; text must never become HTML.
    func localFile(_ source: String) -> (Data, String)? {
        let path = source.split(separator: "?", maxSplits: 1, omittingEmptySubsequences: false)[0]
            .split(separator: "#", maxSplits: 1, omittingEmptySubsequences: false)[0]
        let ext = (String(path) as NSString).pathExtension
        let type = UTType(filenameExtension: ext)?.preferredMIMEType?.lowercased()
            ?? (source.hasPrefix("app:/") ? "application/octet-stream" : nil)
        guard let type, type != "text/html", let bytes = localBytes(source) else { return nil }
        return (bytes, type)
    }

    /// A schemeless `src` (not `app:/`) under `AssetScheme`, its query and
    /// fragment kept: `/deck/index.html` is `exact-app://app/deck/index.html`.
    static func assetURL(_ source: String) -> URL? {
        guard !source.hasPrefix("//"), URL(string: source)?.scheme == nil, !source.hasPrefix("app:"),
              let url = URL(string: source, relativeTo: AssetScheme.base)?.absoluteURL,
              AssetScheme.trees.contains(where: String(url.path.dropFirst()).hasPrefix) else { return nil }
        return url
    }

    func localDocument(_ source: String) -> String? {
        localBytes(source).map { String(decoding: $0, as: UTF8.self) }
    }

    func localBytes(_ source: String) -> Data? {
        if let suppliedDocument { return suppliedDocument }
        // Hosted http(s) decks keep their URL. A scheme-less src — including
        // `URL(string:)` returning nil for a leading-dot relative path — is a
        // file under EXACT_ASSETS. An HTML one is then served by URL under
        // `AssetScheme`; these bytes decide only how it is placed on iOS
        // (`fitsItsBox`). Another is served under its type. Query and
        // fragment are URL metadata, not part of the filesystem path.
        if let scheme = URL(string: source)?.scheme?.lowercased(),
           scheme == "http" || scheme == "https" || scheme == "data" || scheme == "about" || scheme == "blob" {
            return nil
        }
        let path = source.split(separator: "?", maxSplits: 1)[0].split(separator: "#", maxSplits: 1)[0]
        let relative = path.drop(while: { $0 == "/" })
        let base = ProcessInfo.processInfo.environment["EXACT_ASSETS"]
            .map { URL(fileURLWithPath: $0) }
            ?? Bundle.main.resourceURL
            ?? URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
        let root = base.resolvingSymlinksInPath().standardizedFileURL.path
        let file = base.appendingPathComponent(String(relative))
            .resolvingSymlinksInPath().standardizedFileURL
        guard file.path == root || file.path.hasPrefix(root.hasSuffix("/") ? root : root + "/") else { return nil }
        return try? Data(contentsOf: file)
    }

    /// What most likely refused a bundled page's sub-resource. The page sees
    /// only that it failed; ATS applies in an app bundle (not the bare
    /// executable the agent runs) to `http:` on a named, non-local host.
    static func whyFailed(_ source: String) -> String {
        guard let url = URL(string: source), url.scheme?.lowercased() == "http",
              let host = url.host?.lowercased(), host.contains("."), !host.hasSuffix(".local"), !host.hasSuffix(".localhost"),
              host.rangeOfCharacter(from: CharacterSet(charactersIn: "0123456789.:").inverted) != nil,
              Bundle.main.bundleURL.pathExtension == "app"
        else { return "the request failed; is its server running?" }
        let ats = Bundle.main.object(forInfoDictionaryKey: "NSAppTransportSecurity") as? [String: Any]
        if ats?["NSAllowsArbitraryLoadsInWebContent"] as? Bool == true || ats?["NSAllowsArbitraryLoads"] as? Bool == true {
            return "the request failed; is its server running?"
        }
        #if os(macOS)
        let os = "macos"
        #else
        let os = "ios"
        #endif
        return "App Transport Security refuses http: to a named host; app.json's host.\(os).appTransportSecurity can allow it"
    }

    func errorDocument(_ message: String) -> String {
        "<!doctype html><meta charset=utf-8><style>body{font:14px system-ui;padding:16px;color:#6b1d1d;background:#fff3f3}</style><p>\(attribute(message))</p>"
    }

    func attribute(_ value: String) -> String {
        value.replacingOccurrences(of: "&", with: "&amp;")
            .replacingOccurrences(of: "\"", with: "&quot;")
            .replacingOccurrences(of: "<", with: "&lt;")
            .replacingOccurrences(of: ">", with: "&gt;")
    }

    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
        // Every handler serves this arm's own web view, never a popup WebKit
        // made from a copy of its configuration (@ref LLP 1020.002 D1).
        guard !invalidated, message.webView === built else { return }
        if message.name == "exactFrame" {
            // A direct guest's post to `parent`, from the document `src`
            // committed and not one it navigated to.
            guard direct, !revoked, message.frameInfo.isMainFrame, let payload = message.body as? String,
                  let expectedOrigin, WebArm.spelled(message.frameInfo.securityOrigin) == expectedOrigin else { return }
            emit(kind: 1, text: payload)
            return
        }
        if message.name == "exactAgent" {
            if let body = message.body as? [String: Any] {
                if let nonce = body["document"] as? String, direct, !revoked, message.frameInfo.isMainFrame,
                   let expectedOrigin, WebArm.spelled(message.frameInfo.securityOrigin) == expectedOrigin {
                    documentNonce = nonce
                }
                // `AssetScheme` logs its own misses.
                if localGuest, let failed = body["failed"] as? String, !failed.hasPrefix("\(AssetScheme.scheme):") {
                    FileHandle.standardError.write(Data("exact: iframe \(src ?? ""): \(failed) did not load (\(WebArm.whyFailed(failed)))\n".utf8))
                }
                return
            }
            if !message.frameInfo.isMainFrame, guestFrame == nil { guestFrame = message.frameInfo }
            return
        }
        guard message.name == "exact", message.frameInfo.isMainFrame,
              let text = message.body as? String,
              let data = text.data(using: .utf8),
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              object["generation"] as? Int == generation,
              let kind = object["kind"] as? String
        else { return }
        if kind == "message", let payload = object["payload"] as? String {
            emit(kind: 1, text: payload)
        } else if kind == "load" {
            emit(kind: 2)
            if suppressLoad { suppressLoad = false } else { emit(kind: 0) }
        }
    }

    func webViewWebContentProcessDidTerminate(_ webView: WKWebView) {
        guard !invalidated, webView === built else { return }
        if let browser {
            emit(kind: 2)
            return browser.crashed()
        }
        // Process death is a silent wrapper re-serve (@ref LLP 1020 §5).
        suppressLoad = true
        markLoading()
        serve()
    }

    func webView(
        _ webView: WKWebView,
        decidePolicyFor navigationAction: WKNavigationAction,
        decisionHandler: @escaping (WKNavigationActionPolicy) -> Void
    ) {
        guard !invalidated else { decisionHandler(.cancel); return }
        switch ui?.intercept(navigationAction, in: webView) ?? .proceed {
        case .cancel: return decisionHandler(.cancel)
        case .download: return decisionHandler(.download)
        case .proceed: break
        }
        if let browser {
            if blank == nil || navigationAction.request.url?.absoluteString != "about:blank" { browser.decided(navigationAction) }
            decisionHandler(.allow)
            return
        }
        if direct {
            // The guest navigates itself, as a frame may; its later
            // documents are not the committed `src` (@ref LLP 1020 D2).
            if navigationAction.targetFrame?.isMainFrame == true, !serving {
                revoked = true
                guestFrame = nil
            }
            decisionHandler(.allow)
            return
        }
        guard navigationAction.targetFrame?.isMainFrame == true else {
            // Once the injected guest agent identified the committed child,
            // revoke its message channel at the start of any subsequent
            // child navigation, before the replacement document can run.
            if guestFrame != nil { webView.evaluateJavaScript("window.__exactRevokeGuest?.()") }
            decisionHandler(.allow)
            return
        }
        if serving, navigationAction.request.url == wrapperURL {
            decisionHandler(.allow)
            return
        }
        // An allow-top-navigation escape or wrapper self-reload damages the
        // topology. Only the navigation begun by `serve` may reach the main
        // frame; every other one restores the wrapper from its source string.
        decisionHandler(.cancel)
        markLoading()
        scheduleServe()
    }

    func webView(_ webView: WKWebView, decidePolicyFor navigationResponse: WKNavigationResponse,
                 decisionHandler: @escaping (WKNavigationResponsePolicy) -> Void) {
        decisionHandler(!invalidated && FrameUI.downloads(navigationResponse) ? .download : .allow)
    }

    func webView(_ webView: WKWebView, navigationAction: WKNavigationAction, didBecome download: WKDownload) {
        ui?.downloads.adopt(download, from: webView)
    }

    func webView(_ webView: WKWebView, navigationResponse: WKNavigationResponse, didBecome download: WKDownload) {
        ui?.downloads.adopt(download, from: webView)
    }

    func webView(_ webView: WKWebView, didStartProvisionalNavigation navigation: WKNavigation!) {
        guard !invalidated, !isBlank(navigation), let browser else { return }
        if browser.flight == nil { emit(kind: 3) }
        browser.started(navigation)
    }

    func webView(_ webView: WKWebView, didReceiveServerRedirectForProvisionalNavigation navigation: WKNavigation!) {
        guard !invalidated, !isBlank(navigation) else { return }
        browser?.redirected(navigation)
    }

    func webView(_ webView: WKWebView, didCommit navigation: WKNavigation!) {
        ui?.committed(webView)
        guard !invalidated, !isBlank(navigation) else { return }
        automation?.committed()
        browser?.committed(navigation)
    }

    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
        guard !invalidated, let browser else { return }
        serving = false
        if isBlank(navigation) { blank = nil; return }
        emit(kind: 2)
        browser.failed(navigation, error)
    }

    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
        serving = false
        guard !invalidated else { return }
        if let browser {
            if isBlank(navigation) { blank = nil; return }
            emit(kind: 2)
            return browser.failed(navigation, error)
        }
        guard !recovering else {
            recovering = false
            emit(kind: 2)
            emit(kind: 0)
            return
        }
        recovering = true
        markLoading()
        serve(error: error.localizedDescription)
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        serving = false
        recovering = false
        guard direct, !invalidated else { return }
        if isBlank(navigation) { blank = nil; return }
        // A direct guest's document loaded: the iframe's `load`.
        emit(kind: 2)
        if suppressLoad { suppressLoad = false } else { emit(kind: 0) }
        browser?.finished(navigation)
    }

    func postMessage(_ text: String) {
        guard pdf == nil else { return }
        guard partition == nil else {
            FileHandle.standardError.write(Data("exact: postMessage: iframe \(id) has a partition; a partitioned frame has no message bridge\n".utf8))
            return
        }
        guard !invalidated, !servePending, !revoked else { return }
        if direct {
            guard let documentNonce else { return }
            webView.evaluateJavaScript("globalThis.__exactHostReply?.(\(javascript(text)), \(javascript(documentNonce)))", in: nil, in: world) { _ in }
        } else {
            webView.evaluateJavaScript("window.__exactHostReply?.(\(javascript(text)), \(generation))")
        }
    }

    func snapshot(token: UInt32) {
        if let pdf {
            guard let image = pdf.picture(pdf.view.bounds), let data = FrameCaptures.png(image) else { return sendReply(token: token, kind: 2, text: "the PDF view drew nothing") }
            return sendReply(token: token, kind: 0, data: data)
        }
        guard webView.bounds.width > 0, webView.bounds.height > 0 else {
            sendReply(token: token, kind: 2, text: "iframe has no snapshot box")
            return
        }
        let configuration = WKSnapshotConfiguration()
        configuration.rect = webView.bounds
        configuration.afterScreenUpdates = true
        webView.takeSnapshot(with: configuration) { [weak self] image, error in
            guard let self else { return }
            guard let image, let data = self.png(image) else {
                self.sendReply(token: token, kind: 2, text: error?.localizedDescription ?? "WebKit returned no snapshot")
                return
            }
            self.sendReply(token: token, kind: 0, data: data)
        }
    }

    func evaluate(token: UInt32, script: String) {
        guard pdf == nil else { return sendReply(token: token, kind: 2, text: "a PDF frame has no document to script") }
        if direct {
            guard !serving else { sendReply(token: token, kind: 2, text: "guest frame is not ready"); return }
            webView.evaluateJavaScript(script, in: nil, in: world) { [weak self] result in self?.evaluated(token: token, result) }
            return
        }
        guard let guestFrame else {
            sendReply(token: token, kind: 2, text: "guest frame is not ready")
            return
        }
        webView.evaluateJavaScript(script, in: guestFrame, in: world) { [weak self] result in self?.evaluated(token: token, result) }
    }

    func evaluated(token: UInt32, _ result: Result<Any, Error>) {
        switch result {
        case .success(let value):
            if let text = value as? String { sendReply(token: token, kind: 1, text: text) }
            else if value is NSNull { sendReply(token: token, kind: 1, text: "null") }
            else { sendReply(token: token, kind: 1, text: String(describing: value)) }
        case .failure(let error):
            sendReply(token: token, kind: 2, text: error.localizedDescription)
        }
    }

    func png(_ image: PlatformImage) -> Data? {
        #if os(macOS)
        guard let cg = image.cgImage(forProposedRect: nil, context: nil, hints: nil) else { return nil }
        return NSBitmapImageRep(cgImage: cg).representation(using: .png, properties: [:])
        #else
        return image.pngData()
        #endif
    }

    func emit(kind: UInt32, text: String = "") {
        guard !invalidated else { return }
        let data = Data(text.utf8)
        data.withUnsafeBytes { bytes in
            event(context, id, kind, bytes.bindMemory(to: UInt8.self).baseAddress, UInt32(data.count))
        }
    }

    func sendReply(token: UInt32, kind: UInt32, text: String) {
        sendReply(token: token, kind: kind, data: Data(text.utf8))
    }

    func sendReply(token: UInt32, kind: UInt32, data: Data) {
        guard !invalidated else { return }
        data.withUnsafeBytes { bytes in
            reply(context, id, token, kind, bytes.bindMemory(to: UInt8.self).baseAddress, UInt32(data.count))
        }
    }
}

private func arm(_ handle: UnsafeMutableRawPointer?) -> WebArm? {
    handle.map { Unmanaged<WebArm>.fromOpaque($0).takeUnretainedValue() }
}

@_cdecl("exact_web_create")
public func exactWebCreate(_ id: UInt32, _ context: UnsafeMutableRawPointer?, _ event: EventFn?, _ reply: ReplyFn?) -> UnsafeMutableRawPointer? {
    guard let event, let reply else { return nil }
    return Unmanaged.passRetained(WebArm(id: id, context: context, event: event, reply: reply)).toOpaque()
}

@_cdecl("exact_web_platform_view")
public func exactWebPlatformView(_ handle: UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer? {
    arm(handle).map { Unmanaged.passUnretained($0.platformView).toOpaque() }
}

@_cdecl("exact_web_set_src")
public func exactWebSetSrc(_ handle: UnsafeMutableRawPointer?, _ bytes: UnsafePointer<UInt8>?, _ length: UInt32, _ present: UInt32) {
    arm(handle)?.setSrc(present == 0 ? nil : String(decoding: UnsafeBufferPointer(start: bytes, count: Int(length)), as: UTF8.self))
}

@_cdecl("exact_web_set_reader")
public func exactWebSetReader(_ read: ReadFn?) {
    assetReader = read
}

/// Set by the host before the frame's first `exact_web_platform_view`.
@_cdecl("exact_web_set_configuring")
public func exactWebSetConfiguring(_ handle: UnsafeMutableRawPointer?, _ hook: ConfiguringFn?) {
    arm(handle)?.configuringHook = hook
}

@_cdecl("exact_web_set_document")
public func exactWebSetDocument(_ handle: UnsafeMutableRawPointer?, _ bytes: UnsafePointer<UInt8>?, _ length: UInt32, _ present: UInt32) {
    arm(handle)?.suppliedDocument = present == 0 ? nil : Data(UnsafeBufferPointer(start: bytes, count: Int(length)))
}

/// Before `exact_web_set_src`: `{"app:/cache/a.pdf": "/…/cache"}`, each
/// `app:/cache` or `app:/tmp` path the page's query names, to its root's
/// directory; absent for none (@ref LLP 1020.001 D7).
@_cdecl("exact_web_set_files")
public func exactWebSetFiles(_ handle: UnsafeMutableRawPointer?, _ bytes: UnsafePointer<UInt8>?, _ length: UInt32, _ present: UInt32) {
    arm(handle)?.files.supplied = present == 0 ? [:] : json(bytes, length).compactMapValues { $0 as? String }
}

@_cdecl("exact_web_set_sandbox")
public func exactWebSetSandbox(_ handle: UnsafeMutableRawPointer?, _ bytes: UnsafePointer<UInt8>?, _ length: UInt32, _ present: UInt32) {
    arm(handle)?.setSandbox(present == 0 ? nil : String(decoding: UnsafeBufferPointer(start: bytes, count: Int(length)), as: UTF8.self))
}

@_cdecl("exact_web_post_message")
public func exactWebPostMessage(_ handle: UnsafeMutableRawPointer?, _ bytes: UnsafePointer<UInt8>?, _ length: UInt32) {
    arm(handle)?.postMessage(String(decoding: UnsafeBufferPointer(start: bytes, count: Int(length)), as: UTF8.self))
}

@_cdecl("exact_web_snapshot")
public func exactWebSnapshot(_ handle: UnsafeMutableRawPointer?, _ token: UInt32) {
    arm(handle)?.snapshot(token: token)
}

@_cdecl("exact_web_agent_eval")
public func exactWebAgentEval(_ handle: UnsafeMutableRawPointer?, _ token: UInt32, _ bytes: UnsafePointer<UInt8>?, _ length: UInt32) {
    let script = String(decoding: UnsafeBufferPointer(start: bytes, count: Int(length)), as: UTF8.self)
    arm(handle)?.evaluate(token: token, script: script)
}

@_cdecl("exact_web_destroy")
public func exactWebDestroy(_ handle: UnsafeMutableRawPointer?) {
    guard let handle else { return }
    let retained = Unmanaged<WebArm>.fromOpaque(handle)
    let arm = retained.takeUnretainedValue()
    arm.invalidate()
    arm.built?.removeFromSuperview()
    arm.pdf?.view.removeFromSuperview()
    retained.release()
}

private func json(_ bytes: UnsafePointer<UInt8>?, _ length: UInt32) -> [String: Any] {
    guard let bytes else { return [:] }
    let data = Data(UnsafeBufferPointer(start: bytes, count: Int(length)))
    return (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] ?? [:]
}

/// `{partition, agent, zoom, allowsBackForwardNavigationGestures,
/// allowsMagnification, inactiveSchedulingPolicy}`: before the first
/// `exact_web_platform_view`, and again when any changes (a new partition
/// makes a new web view, which that call then returns).
@_cdecl("exact_web_configure")
public func exactWebConfigure(_ handle: UnsafeMutableRawPointer?, _ bytes: UnsafePointer<UInt8>?, _ length: UInt32) {
    arm(handle)?.configure(json(bytes, length))
}

/// A session ended (`{session}`): its in-memory partitions are let go.
@_cdecl("exact_web_end_session")
public func exactWebEndSession(_ bytes: UnsafePointer<UInt8>?, _ length: UInt32) {
    let session = json(bytes, length)["session"] as? String ?? ""
    if Thread.isMainThread { Partitions.end(session: session) } else { DispatchQueue.main.async { Partitions.end(session: session) } }
}

/// A frame command (@ref LLP 1020.002 D3): `{op: "navigate", url}`,
/// `{op: "traverseHistory", delta}`, `{op: "reload", ignoreCache}`,
/// `{op: "stopLoading"}`; or a picture of the guest (D7), `{op:
/// "captureScreenshot", path, file, clip}`; or its inspector (D11), `{op:
/// "openDevTools"}`.
@_cdecl("exact_web_command")
public func exactWebCommand(_ handle: UnsafeMutableRawPointer?, _ bytes: UnsafePointer<UInt8>?, _ length: UInt32) {
    arm(handle)?.command(json(bytes, length))
}

/// The app's hatch was called with this frame's view (@ref LLP 1020.002 §9):
/// a development build says by name what it changed of what the arm owns.
@_cdecl("exact_web_check_owned")
public func exactWebCheckOwned(_ handle: UnsafeMutableRawPointer?) {
    arm(handle)?.checkOwned()
}

/// The agent's answer to a request the arm held (`ask`): `{token, cancel}`,
/// `{token, text}` (a dialog), `{token, paths}` (a file input) or `{token,
/// path, shown}` (a download's destination and how its record names it).
@_cdecl("exact_web_answer")
public func exactWebAnswer(_ handle: UnsafeMutableRawPointer?, _ bytes: UnsafePointer<UInt8>?, _ length: UInt32) {
    arm(handle)?.answer(json(bytes, length))
}

public typealias ClearFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32) -> Void

/// A call of `@exact/browser-automation` (@ref LLP 1020.002 D9), its options
/// with `op`, `id` and `owner` (the session's module folder): `done` receives
/// the answer's JSON, `{error, message}` when it failed.
@_cdecl("exact_web_automation")
public func exactWebAutomation(_ bytes: UnsafePointer<UInt8>?, _ length: UInt32, _ context: UnsafeMutableRawPointer?, _ done: ClearFn?) {
    let request = json(bytes, length)
    let finish: ([String: Any]) -> Void = { answer in
        let data = (try? JSONSerialization.data(withJSONObject: answer)) ?? Data("{}".utf8)
        data.withUnsafeBytes { done?(context, $0.bindMemory(to: UInt8.self).baseAddress, UInt32(data.count)) }
    }
    let run = {
        let id = request["id"] as? String ?? ""
        let found = Automations.find(owner: request["owner"] as? String ?? "", id: id)
        guard found.count == 1, let arm = found.first else { return finish(["error": "NotFoundError", "message": "no unique live iframe with id \(id)"]) }
        let automation = arm.automation ?? FrameAutomation(arm: arm)
        arm.automation = automation
        automation.run(request, reply: finish)
    }
    if Thread.isMainThread { run() } else { DispatchQueue.main.async(execute: run) }
}

/// `clearSiteData(partition, types)` (@ref LLP 1020.002 D4), `{partition,
/// types}`: `done` receives "" when the platform says the clear is done, or
/// `<DOMException name>: <message>`.
@_cdecl("exact_web_clear_site_data")
public func exactWebClearSiteData(_ bytes: UnsafePointer<UInt8>?, _ length: UInt32, _ context: UnsafeMutableRawPointer?, _ done: ClearFn?) {
    let request = json(bytes, length)
    let finish: ((String, String)?) -> Void = { failure in
        let text = failure.map { "\($0.0): \($0.1)" } ?? ""
        Array(text.utf8).withUnsafeBufferPointer { done?(context, $0.baseAddress, UInt32($0.count)) }
    }
    let run = { Partitions.clear(request["partition"] as? String ?? "", types: request["types"] as? [String] ?? [], done: finish) }
    if Thread.isMainThread { run() } else { DispatchQueue.main.async(execute: run) }
}
