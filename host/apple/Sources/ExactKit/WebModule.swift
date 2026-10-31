// The Apple iframe arm's opaque C ABI (@ref LLP 1020 D3/D4). This file is
// shared by both presenters and deliberately imports no WebKit.
import Foundation

#if os(macOS)
import AppKit
typealias ExactWebPlatformView = NSView
typealias ExactWebImage = NSImage
#else
import UIKit
typealias ExactWebPlatformView = UIView
typealias ExactWebImage = UIImage
#endif

private typealias WebEventFn = @convention(c) (
    UnsafeMutableRawPointer?, UInt32, UInt32, UnsafePointer<UInt8>?, UInt32
) -> Void
private typealias WebReplyFn = @convention(c) (
    UnsafeMutableRawPointer?, UInt32, UInt32, UInt32, UnsafePointer<UInt8>?, UInt32
) -> Void

private typealias WebConfiguringFn = @convention(c) (UnsafeMutableRawPointer?, UnsafeMutableRawPointer?) -> Void
private typealias WebAssetSinkFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32) -> Void
private typealias WebAssetReadFn = @convention(c) (UnsafePointer<UInt8>?, UInt32, UnsafeMutableRawPointer?, WebAssetSinkFn?) -> Void

/// An app asset for the arm's `exact-app` origin, through the resolver every
/// session reads: a delivered generation's or the dev URL's files reach a
/// bundled page's sub-resources as they reach its document (@ref LLP 1020.001 D3).
private let exactWebAssetRead: WebAssetReadFn = { name, length, context, sink in
    guard let sink else { return }
    let name = String(decoding: UnsafeBufferPointer(start: name, count: Int(length)), as: UTF8.self)
    guard let bytes = ExactApp.shared.resolver.bytes(name) else { return }
    bytes.withUnsafeBytes { sink(context, $0.bindMemory(to: UInt8.self).baseAddress, UInt32(bytes.count)) }
}

private struct WebLoadError: Error { let message: String }
private struct WebRequestError: Error { let message: String }

private final class WebModule {
    typealias CreateFn = @convention(c) (UInt32, UnsafeMutableRawPointer?, WebEventFn?, WebReplyFn?) -> UnsafeMutableRawPointer?
    typealias PlatformViewFn = @convention(c) (UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer?
    typealias SetFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UInt32) -> Void
    typealias PostFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32) -> Void
    typealias SnapshotFn = @convention(c) (UnsafeMutableRawPointer?, UInt32) -> Void
    typealias EvalFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UnsafePointer<UInt8>?, UInt32) -> Void
    typealias DestroyFn = @convention(c) (UnsafeMutableRawPointer?) -> Void
    typealias SetReaderFn = @convention(c) (WebAssetReadFn?) -> Void
    typealias ClearDoneFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32) -> Void
    typealias SetConfiguringFn = @convention(c) (UnsafeMutableRawPointer?, WebConfiguringFn?) -> Void
    typealias SetReaderlessFn = @convention(c) (UnsafePointer<UInt8>?, UInt32) -> Void
    typealias ClearFn = @convention(c) (UnsafePointer<UInt8>?, UInt32, UnsafeMutableRawPointer?, ClearDoneFn?) -> Void

    let library: UnsafeMutableRawPointer
    let create: CreateFn
    let platformView: PlatformViewFn
    let setSrc: SetFn
    let setDocument: SetFn
    /// The files a bundled page's query names, resolved (@ref LLP 1020.001 D7).
    let setFiles: SetFn
    let setSandbox: SetFn
    let postMessage: PostFn
    let snapshot: SnapshotFn
    let evaluate: EvalFn
    let destroy: DestroyFn
    let configure: PostFn
    let command: PostFn
    let clearSiteData: ClearFn
    let endSession: SetReaderlessFn
    /// The agent's answer to a request the arm held (@ref LLP 1020.002 §10).
    let answer: PostFn
    /// The app's hatch had the frame's view: the arm checks what it owns (§9).
    let checkOwned: DestroyFn
    /// Hands a frame's arm the host's moment before each web view it makes (§9).
    let setConfiguring: SetConfiguringFn

    static func load(path: String) -> Result<WebModule, WebLoadError> {
        guard let library = dlopen(path, RTLD_NOW | RTLD_LOCAL) else {
            return .failure(WebLoadError(message: "dlopen \(path): \(String(cString: dlerror()))"))
        }
        func symbol<T>(_ name: String, _: T.Type) -> T? {
            dlsym(library, name).map { unsafeBitCast($0, to: T.self) }
        }
        guard let create = symbol("exact_web_create", CreateFn.self),
              let platformView = symbol("exact_web_platform_view", PlatformViewFn.self),
              let setSrc = symbol("exact_web_set_src", SetFn.self),
              let setDocument = symbol("exact_web_set_document", SetFn.self),
              let setFiles = symbol("exact_web_set_files", SetFn.self),
              let setSandbox = symbol("exact_web_set_sandbox", SetFn.self),
              let postMessage = symbol("exact_web_post_message", PostFn.self),
              let snapshot = symbol("exact_web_snapshot", SnapshotFn.self),
              let evaluate = symbol("exact_web_agent_eval", EvalFn.self),
              let destroy = symbol("exact_web_destroy", DestroyFn.self),
              let setReader = symbol("exact_web_set_reader", SetReaderFn.self),
              let configure = symbol("exact_web_configure", PostFn.self),
              let command = symbol("exact_web_command", PostFn.self),
              let clearSiteData = symbol("exact_web_clear_site_data", ClearFn.self),
              let endSession = symbol("exact_web_end_session", SetReaderlessFn.self),
              let answer = symbol("exact_web_answer", PostFn.self),
              let checkOwned = symbol("exact_web_check_owned", DestroyFn.self),
              let setConfiguring = symbol("exact_web_set_configuring", SetConfiguringFn.self)
        else {
            dlclose(library)
            return .failure(WebLoadError(message: "\(path) is not an exact web arm (missing exports)"))
        }
        setReader(exactWebAssetRead)
        return .success(WebModule(
            library: library, create: create, platformView: platformView,
            setSrc: setSrc, setDocument: setDocument, setFiles: setFiles, setSandbox: setSandbox, postMessage: postMessage, snapshot: snapshot,
            evaluate: evaluate, destroy: destroy, configure: configure, command: command, clearSiteData: clearSiteData, endSession: endSession,
            answer: answer, checkOwned: checkOwned, setConfiguring: setConfiguring))
    }

    private init(
        library: UnsafeMutableRawPointer, create: @escaping CreateFn,
        platformView: @escaping PlatformViewFn, setSrc: @escaping SetFn, setDocument: @escaping SetFn, setFiles: @escaping SetFn,
        setSandbox: @escaping SetFn, postMessage: @escaping PostFn, snapshot: @escaping SnapshotFn,
        evaluate: @escaping EvalFn, destroy: @escaping DestroyFn,
        configure: @escaping PostFn, command: @escaping PostFn, clearSiteData: @escaping ClearFn, endSession: @escaping SetReaderlessFn,
        answer: @escaping PostFn, checkOwned: @escaping DestroyFn, setConfiguring: @escaping SetConfiguringFn
    ) {
        self.library = library
        self.create = create
        self.platformView = platformView
        self.setSrc = setSrc
        self.setDocument = setDocument
        self.setFiles = setFiles
        self.setSandbox = setSandbox
        self.postMessage = postMessage
        self.snapshot = snapshot
        self.evaluate = evaluate
        self.destroy = destroy
        self.configure = configure
        self.command = command
        self.clearSiteData = clearSiteData
        self.endSession = endSession
        self.answer = answer
        self.checkOwned = checkOwned
        self.setConfiguring = setConfiguring
    }
}

private final class WebCallbackBox {
    weak var manager: WebViews?
    let id: UInt32
    init(manager: WebViews, id: UInt32) { self.manager = manager; self.id = id }
}

private final class WebEntry {
    weak var owner: NodeView?
    let callback: WebCallbackBox
    var handle: UnsafeMutableRawPointer?
    var platformView: ExactWebPlatformView?
    var src: String?
    var sandbox: String?
    /// What `exact_web_configure` was last given (`partition`, `zoom`, the
    /// `apple.*` words), and the last `FrameNavigation` record the frame
    /// reported, which `tree` shows (@ref LLP 1020.002 §10).
    var configuration: Data?
    var partition: String?
    var navigation: [String: Any]?
    /// A local PDF's page count while the frame shows it in a PDF view (@ref LLP 1020.002 D10).
    var pdfPages: Int?
    var initialized = false
    var loading = true
    var unavailable = false
    var fileFailed = false
    var sourceRevision: UInt64 = 0
    /// A guest request was held for the agent since the last guest script
    /// began: a `tap` whose click opened a dialog stops waiting for it.
    var asked = false

    init(owner: NodeView, callback: WebCallbackBox) {
        self.owner = owner
        self.callback = callback
    }
}

private final class WebWait {
    var data: Data?
    var error: String?
    var done = false
}

private let exactWebEventCallback: WebEventFn = { context, _, kind, bytes, length in
    guard let context else { return }
    let box = Unmanaged<WebCallbackBox>.fromOpaque(context).takeUnretainedValue()
    let data = bytes.map { Data(bytes: $0, count: Int(length)) } ?? Data()
    let deliver = { if let manager = box.manager { manager.receivedEvent(id: box.id, kind: kind, data: data) } }
    if Thread.isMainThread { deliver() } else { DispatchQueue.main.async(execute: deliver) }
}

/// Called on the main thread inside the arm's `build()`, before the web
/// view exists: no queueing, or the configuration would be made already.
private let exactWebConfiguringCallback: WebConfiguringFn = { context, configuration in
    guard let context, let configuration else { return }
    let box = Unmanaged<WebCallbackBox>.fromOpaque(context).takeUnretainedValue()
    box.manager?.configuring(id: box.id, Unmanaged<AnyObject>.fromOpaque(configuration).takeUnretainedValue())
}

private let exactWebReplyCallback: WebReplyFn = { context, _, token, kind, bytes, length in
    guard let context else { return }
    let box = Unmanaged<WebCallbackBox>.fromOpaque(context).takeUnretainedValue()
    let data = bytes.map { Data(bytes: $0, count: Int(length)) } ?? Data()
    let deliver = { if let manager = box.manager { manager.receivedReply(token: token, kind: kind, data: data) } }
    if Thread.isMainThread { deliver() } else { DispatchQueue.main.async(execute: deliver) }
}

final class WebViews {
    /// The session these views belong to (LLP 1031 D12: the arm's library
    /// is loaded once per process; the views and their node ids are the
    /// session's).
    weak var session: ExactSession?
    nonisolated(unsafe) private static var sharedModule: WebModule?
    nonisolated(unsafe) private static var attempted = false
    private var module: WebModule? { WebViews.sharedModule }
    private var entries: [UInt32: WebEntry] = [:]
    private var waits: [UInt32: WebWait] = [:]
    /// Guest requests held for the agent, by ticket: the frame, the arm's
    /// token, the capability and what it asked (@ref LLP 1020.002 §10).
    private var held: [UInt64: (view: UInt32, token: Any, capability: String, args: [String: Any])] = [:]
    /// Names in `app:/tmp/downloads/` handed to downloads not yet in: two at
    /// once never get one name. A completed one's file then holds its name.
    private var reserved = Set<String>()
    private var nextToken: UInt32 = 1
    private(set) var failure: String?
    var moduleLocation: () -> String = WebViews.modulePath

    var status: String {
        if module != nil { return "module loaded; \(entries.count) iframes" }
        return "not loaded: \(failure ?? (entries.isEmpty ? "no iframes" : "not requested"))"
    }

    static func modulePath() -> String {
        #if os(macOS)
        return Bundle.main.executableURL!.deletingLastPathComponent().appendingPathComponent("libexact_web.dylib").path
        #else
        return embeddedModule(framework: "ExactWeb", dylib: "libexact_web.dylib")
        #endif
    }

    private func loadIfNeeded() -> WebModule? {
        if let module { return module }
        guard !WebViews.attempted else { return nil }
        WebViews.attempted = true
        switch WebModule.load(path: moduleLocation()) {
        case .success(let loaded):
            WebViews.sharedModule = loaded
            return loaded
        case .failure(let error):
            failure = error.message
            FileHandle.standardError.write(Data("exact web: \(error.message)\n".utf8))
            return nil
        }
    }

    /// The sole dlopen gate: called only from an `iframe` NodeView's create
    /// commit, so an iframe-free first screen never touches WebKit (@ref LLP 1020 D3).
    func create(owner: NodeView) -> ExactWebPlatformView? {
        let callback = WebCallbackBox(manager: self, id: owner.id)
        let entry = WebEntry(owner: owner, callback: callback)
        entries[owner.id] = entry
        guard let module = loadIfNeeded() else {
            entry.loading = false
            entry.unavailable = true
            return nil
        }
        let context = Unmanaged.passUnretained(callback).toOpaque()
        guard let handle = module.create(owner.id, context, exactWebEventCallback, exactWebReplyCallback) else {
            entry.loading = false
            entry.unavailable = true
            failure = "libexact_web.dylib refused iframe \(owner.id)"
            FileHandle.standardError.write(Data("exact web: \(failure!)\n".utf8))
            return nil
        }
        module.setConfiguring(handle, exactWebConfiguringCallback)
        configure(entry, owner, module: module, handle: handle)
        guard let rawView = module.platformView(handle) else {
            module.destroy(handle)
            entry.loading = false
            entry.unavailable = true
            failure = "libexact_web.dylib returned no platform view for \(owner.id)"
            FileHandle.standardError.write(Data("exact web: \(failure!)\n".utf8))
            return nil
        }
        let view = Unmanaged<ExactWebPlatformView>.fromOpaque(rawView).takeUnretainedValue()
        entry.handle = handle
        entry.platformView = view
        return view
    }

    /// `partition`, `zoom` and the `apple.*` words as the arm takes them
    /// (`exact_web_configure`), before its web view is made.
    private func configuration(_ owner: NodeView) -> Data {
        var values: [String: Any] = ["agent": ExactEnv.agentMode, "session": sessionKey,
                                     "app": WebViews.appKey(session?.appFileRoots["data"])]
        if let partition = owner.props["partition"] { values["partition"] = partition }
        // How `@exact/browser-automation` finds the frame: its id among the
        // frames of the folder its session's module resolves `app:/tmp/` in (@ref LLP 1020.002 D9).
        if let id = owner.props["id"], let session {
            values["element"] = id
            values["owner"] = URL(fileURLWithPath: NativeViews.roots(session: session).temporary, isDirectory: true).standardizedFileURL.path
        }
        // Only a frame that handles `newwindow=` asks for popups and tabs (@ref LLP 1020.002 D5).
        if owner.handlers.contains("newwindow") { values["newwindow"] = true }
        // A hatched frame's web view has its own preferences (§9).
        if owner.props["hatch"] != nil { values["hatched"] = true }
        if let zoom = owner.style["zoom"]?.number, zoom > 0 { values["zoom"] = zoom }
        // A local PDF, by its name: the arm shows a PDF's view from the start, and no web view (the file decides).
        // Not a sandboxed or partitioned frame, which stays WebKit's from its mount (D10).
        if owner.props["sandbox"] == nil, owner.props["partition"] == nil, Self.localPDF(owner.props["src"]) { values["pdf"] = true }
        for (prop, key) in WebViews.appleWords {
            guard let value = owner.props[prop] else { continue }
            values[key] = value == "true" ? true as Any : value == "false" ? false as Any : value as Any
        }
        return (try? JSONSerialization.data(withJSONObject: values, options: .sortedKeys)) ?? Data("{}".utf8)
    }

    /// The app a `persist:` partition belongs to: the runner's app id, which
    /// names the app's data root (`…/Library/Application Support/exact/<app id>/data`,
    /// `host/apple/src/picker.rs`), so two apps embedded in one process keep
    /// theirs apart. Never the root's whole path: iOS moves an app's data
    /// container (its HOME) at every update and reinstall, and a store keyed
    /// by it would be lost each time. Without a root, the bundle's id.
    static func appKey(_ dataRoot: String?, home: String = NSHomeDirectory()) -> String {
        let fallback = Bundle.main.bundleIdentifier ?? "exact"
        guard let dataRoot else { return fallback }
        let marker = "/Library/Application Support/exact/"
        if let range = dataRoot.range(of: marker),
           let id = dataRoot[range.upperBound...].split(separator: "/").first, !id.isEmpty {
            return String(id)
        }
        return dataRoot.hasPrefix(home) ? String(dataRoot.dropFirst(home.count)) : dataRoot
    }

    /// The tier 2 words (@ref LLP 1020.002 §9), by prop.
    static let appleWords = [
        ("appleAllowsBackForwardNavigationGestures", "allowsBackForwardNavigationGestures"),
        ("appleAllowsMagnification", "allowsMagnification"),
        ("appleInactiveSchedulingPolicy", "inactiveSchedulingPolicy"),
    ]

    private func configure(_ entry: WebEntry, _ owner: NodeView, module: WebModule, handle: UnsafeMutableRawPointer) {
        let data = configuration(owner)
        guard data != entry.configuration else { return }
        entry.configuration = data
        entry.partition = owner.props["partition"]
        data.withUnsafeBytes { module.configure(handle, $0.bindMemory(to: UInt8.self).baseAddress, UInt32(data.count)) }
    }

    func update(_ owner: NodeView) {
        guard let entry = entries[owner.id] else { return }
        let src = Self.browserSource(owner.props["src"])
        let sandbox = owner.props["sandbox"]
        let changedSrc = !entry.initialized || entry.src != src
        let changedSandbox = !entry.initialized || entry.sandbox != sandbox
        entry.src = src
        entry.sandbox = sandbox
        entry.initialized = true
        if let module, let handle = entry.handle {
            configure(entry, owner, module: module, handle: handle)
            replaceView(entry, owner, module: module, handle: handle)
        }
        guard let module, let handle = entry.handle, changedSrc || changedSandbox else { return }
        guard changedSrc else {
            if !entry.fileFailed { entry.loading = true }
            send(sandbox, to: handle, using: module.setSandbox)
            return
        }
        entry.loading = true
        entry.fileFailed = false
        entry.sourceRevision &+= 1
        let revision = entry.sourceRevision
        let resolver = session?.app.resolver
        guard let src, src.hasPrefix("app:/") || (URL(string: src)?.scheme == nil && !src.hasPrefix("//") && resolver != nil) else {
            send(nil as Data?, to: handle, using: module.setFiles)
            send(nil as Data?, to: handle, using: module.setDocument)
            if changedSandbox { send(sandbox, to: handle, using: module.setSandbox) }
            send(src, to: handle, using: module.setSrc)
            return
        }
        // A local document is read off the main thread, once per `src`
        // (a row mounting read its file on the main thread at every
        // update); `src` reaches the arm with it, and the frame shows
        // loading until then. Its bytes go as they are: the arm shows an
        // HTML file as a document and another (a PDF) by its type (#115).
        let path = src.components(separatedBy: "?")[0].components(separatedBy: "#")[0]
        let name = path.hasPrefix("/") ? String(path.dropFirst()) : path
        let id = owner.id
        let appFile = src.hasPrefix("app:/") ? AppFiles.url(path) : nil
        // The app files the page's query names, by their roots; the arm gives
        // those it can open and empties the rest ("" here: refused already).
        var given: [String: String] = [:]
        for value in src.hasPrefix("app:/") ? [] : WebViews.givenPaths(src) {
            let (root, why) = WebViews.givenRoot(value)
            given[value] = root ?? ""
            if root == nil { FileHandle.standardError.write(Data("exact: iframe \(src): \(value) is not given (\(why))\n".utf8)) }
        }
        WebViews.reads.async { [weak self] in
            let document = src.hasPrefix("app:/")
                ? appFile.flatMap { try? Data(contentsOf: $0) }
                : resolver?.bytes(name)
            DispatchQueue.main.async {
                guard let self, self.entries[id] === entry, entry.sourceRevision == revision, let handle = entry.handle, let module = self.module else { return }
                if src.hasPrefix("app:/"), document == nil {
                    entry.fileFailed = true
                    entry.loading = false
                    self.send(nil as Data?, to: handle, using: module.setFiles)
                    self.send(nil as Data?, to: handle, using: module.setDocument)
                    self.send(entry.sandbox, to: handle, using: module.setSandbox)
                    self.send(nil as String?, to: handle, using: module.setSrc)
                    if let session = self.session, entry.owner?.handlers.contains("error") == true {
                        session.apply(session.runtime.media(id, event: "error", payload: "iframe: \(src): file unavailable", now: session.now()))
                    }
                    return
                }
                self.send(given.isEmpty ? nil : (try? JSONSerialization.data(withJSONObject: given)), to: handle, using: module.setFiles)
                self.send(document ?? Data(), to: handle, using: module.setDocument)
                self.send(entry.sandbox, to: handle, using: module.setSandbox)
                self.send(src, to: handle, using: module.setSrc)
            }
        }
    }

    private static let reads = DispatchQueue(label: "exact.web.documents", qos: .userInitiated)

    /// The `app:/` paths a bundled page's `src` names in its query (@ref LLP
    /// 1020.001 D7): for a page under `assets/` or `deck/` (an `.html` or
    /// `.htm` file), each value that begins `app:/`, read as a form reads it.
    /// Empty for any other source. picker-glue.js `pageFiles` reads a source
    /// the same way.
    static func givenPaths(_ src: String) -> [String] {
        guard !src.hasPrefix("//"), !src.hasPrefix("app:"), URL(string: src)?.scheme == nil,
              let url = URL(string: src, relativeTo: URL(string: "exact-app://app/"))?.absoluteURL,
              ["assets/", "deck/"].contains(where: String(url.path.dropFirst()).hasPrefix),
              ["html", "htm"].contains(url.pathExtension.lowercased()),
              let query = URLComponents(url: url, resolvingAgainstBaseURL: false)?.percentEncodedQuery else { return [] }
        return query.split(separator: "&").compactMap { pair in
            guard let equals = pair.firstIndex(of: "="),
                  let value = pair[pair.index(after: equals)...].replacingOccurrences(of: "+", with: " ").removingPercentEncoding,
                  value.hasPrefix("app:/") else { return nil }
            return value
        }
    }

    /// The directory of the root below which `path` names a file a page may
    /// be given, or why it names none (@ref LLP 1020.001 D7): `app:/cache` or
    /// `app:/tmp`, with no `.`, `..` or empty part. The arm opens the file
    /// below it, following no link.
    static func givenRoot(_ path: String) -> (root: String?, why: String) {
        let parts = path.hasPrefix("app:/") ? path.dropFirst(5).split(separator: "/", omittingEmptySubsequences: false) : []
        guard parts.count >= 2, parts[0] == "cache" || parts[0] == "tmp",
              !parts.dropFirst().contains(where: { $0.isEmpty || $0 == "." || $0 == ".." || $0.contains("\0") }) else {
            return (nil, "a page is given app:/cache and app:/tmp files only")
        }
        guard let file = AppFiles.url(path) else { return (nil, "this app has no file roots here") }
        return (parts.dropFirst().reduce(file) { url, _ in url.deletingLastPathComponent() }.path, "")
    }

    /// A new partition made the arm a new web view, or it shows a local PDF's
    /// view or a web view again: the node shows it in place of the old one,
    /// and the app's hatch hears `changed` with it (@ref LLP 1020.002 D1, D10, §9).
    private func replaceView(_ entry: WebEntry, _ owner: NodeView, module: WebModule, handle: UnsafeMutableRawPointer) {
        guard let raw = module.platformView(handle) else { return }
        let view = Unmanaged<ExactWebPlatformView>.fromOpaque(raw).takeUnretainedValue()
        guard view !== entry.platformView else { return }
        let old = entry.platformView
        view.frame = old?.frame ?? owner.bounds
        view.autoresizingMask = old?.autoresizingMask ?? []
        view.isHidden = owner.cssVisibilityHidden
        #if os(macOS)
        view.wantsLayer = true
        #endif
        old?.removeFromSuperview()
        owner.addSubview(view)
        owner.web = view
        entry.platformView = view
        owner.presenter?.elements.realized(owner)
    }

    /// The arm is about to make a frame's web view: a hatched frame's hatch
    /// hears `configuring` with its configuration (@ref LLP 1020.002 §9).
    fileprivate func configuring(id: UInt32, _ configuration: AnyObject) {
        guard let owner = entries[id]?.owner else { return }
        owner.presenter?.elements.configuring(owner, configuration)
    }

    /// The app's hatch was called with the frame's view: a development arm
    /// says what it changed of what the arm owns.
    func hatched(_ id: UInt32) {
        guard let module, let handle = entries[id]?.handle else { return }
        module.checkOwned(handle)
    }

    /// Whether `src` names a local file (an `app:/` path, or the app's
    /// bundled one, as `update` reads them) ending in `.pdf`.
    static func localPDF(_ source: String?) -> Bool {
        guard let source, source.hasPrefix("app:/") || (URL(string: source)?.scheme == nil && !source.hasPrefix("//")) else { return false }
        let path = source.split(separator: "#", maxSplits: 1).first.map { $0.split(separator: "?", maxSplits: 1).first ?? $0 } ?? ""
        return path.lowercased().hasSuffix(".pdf")
    }
    private static func browserSource(_ source: String?) -> String? {
        guard let source, source.hasPrefix("//") else { return source }
        let scheme = ExactApp.shared.connectedPage?.scheme?.lowercased()
        return (scheme == "http" || scheme == "https" ? scheme! : "https") + ":" + source
    }

    private func send(_ value: String?, to handle: UnsafeMutableRawPointer, using setter: WebModule.SetFn) {
        send(value.map { Data($0.utf8) }, to: handle, using: setter)
    }

    private func send(_ data: Data?, to handle: UnsafeMutableRawPointer, using setter: WebModule.SetFn) {
        guard let data else { setter(handle, nil, 0, 0); return }
        data.withUnsafeBytes { bytes in
            setter(handle, bytes.bindMemory(to: UInt8.self).baseAddress, UInt32(data.count), 1)
        }
    }

    /// Replies are sent only to this mounted frame, never queued by its authored id.
    func postMessage(_ target: String, _ text: String) {
        let matches = entries.values.filter { $0.owner?.props["id"] == target }
        guard matches.count == 1, let entry = matches.first, !entry.fileFailed,
              let handle = entry.handle, let module else {
            fputs("exact: postMessage: no unique live iframe with id \(target)\n", stderr)
            return
        }
        guard entry.partition == nil else {
            fputs("exact: postMessage: iframe \(target) has a partition; a partitioned frame has no message bridge\n", stderr)
            return
        }
        Data(text.utf8).withUnsafeBytes { bytes in
            module.postMessage(handle, bytes.bindMemory(to: UInt8.self).baseAddress, UInt32(bytes.count))
        }
    }
    func hasFrame(_ target: String) -> Bool {
        entries.values.contains { $0.owner?.props["id"] == target }
    }

    func destroy(id: UInt32) {
        guard let entry = entries[id] else { return }
        // The runner retires their holds with the node; the arm ends them unanswered.
        held = held.filter { $0.value.view != id }
        if let handle = entry.handle { module?.destroy(handle) }
        entries.removeValue(forKey: id)
    }

    func reset() {
        for id in Array(entries.keys) { destroy(id: id) }
    }

    /// The session's own key for its in-memory partitions.
    private lazy var sessionKey = UUID().uuidString

    /// The session ended: its frames are gone, and its in-memory partitions go
    /// with them (an arm never loaded made none).
    func endSession() {
        reset()
        guard let module else { return }
        let data = Data("{\"session\":\"\(sessionKey)\"}".utf8)
        data.withUnsafeBytes { module.endSession($0.bindMemory(to: UInt8.self).baseAddress, UInt32(data.count)) }
    }

    fileprivate func receivedEvent(id: UInt32, kind: UInt32, data: Data) {
        // A failed file says nothing more, but the view the arm shows in its place is still shown (kind 7).
        guard let entry = entries[id], !entry.fileFailed || kind == 7, let owner = entry.owner else { return }
        switch kind {
        case 0:
            if owner.handlers.contains("load") { DispatchQueue.main.async { [weak owner] in owner?.presenter?.load(id) } }
        case 1:
            if owner.handlers.contains("message") {
                let text = String(decoding: data, as: UTF8.self)
                DispatchQueue.main.async { [weak owner] in owner?.presenter?.message(id, text) }
            }
        case 2: entry.loading = false
        case 3: entry.loading = true
        case 4:
            // A `FrameNavigation` record (@ref LLP 1020.002 D2).
            guard let value = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let name = value["event"] as? String, let record = value["record"] as? [String: Any] else { return }
            entry.navigation = record
            guard owner.handlers.contains(name), let session,
                  let json = try? JSONSerialization.data(withJSONObject: record, options: .sortedKeys) else { return }
            let text = String(decoding: json, as: UTF8.self)
            DispatchQueue.main.async { [weak session] in
                guard let session else { return }
                session.apply(session.runtime.frameNavigation(id, event: name, record: text, now: session.now()))
            }
        case 5:
            // A guest's `newwindow` or `download` (@ref LLP 1020.002 D5).
            guard let value = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let name = value["event"] as? String, let record = value["record"] as? [String: Any] else { return }
            if name == "download", let path = record["path"] as? String, path.hasPrefix("app:/tmp/downloads/"), record["state"] as? String == "completed" {
                reserved.remove(String(path.dropFirst("app:/tmp/downloads/".count)))
            }
            guard owner.handlers.contains(name), let session,
                  let json = try? JSONSerialization.data(withJSONObject: record, options: .sortedKeys) else { return }
            let text = String(decoding: json, as: UTF8.self)
            DispatchQueue.main.async { [weak session] in
                guard let session else { return }
                session.apply(session.runtime.frameNavigation(id, event: name, record: text, now: session.now()))
            }
        case 6:
            hold(entry, id: id, data)
        case 7:
            // The arm shows a local PDF's view (`{pdf: pages}`) or a web view again (`{}`).
            let value = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
            entry.pdfPages = (value?["pdf"] as? NSNumber)?.intValue
            if let module, let handle = entry.handle { replaceView(entry, owner, module: module, handle: handle) }
        default: break
        }
    }

    /// A guest's dialog, file input or download destination under the agent
    /// (@ref LLP 1020.002 §10): held in the runner as `dialog`, `open-file`
    /// or `save-file` on the frame's node, answered by `tap @t` or `type @t`.
    private func hold(_ entry: WebEntry, id: UInt32, _ data: Data) {
        guard let value = try? JSONSerialization.jsonObject(with: data) as? [String: Any], let token = value["token"],
              let capability = value["capability"] as? String, let args = value["args"] as? [String: Any], let session else { return }
        let request: [String: Any] = ["op": "frameRequest", "node": Int(id), "capability": capability, "args": args]
        let line = (try? JSONSerialization.data(withJSONObject: request)).map { String(decoding: $0, as: UTF8.self) } ?? "{}"
        let reply = (try? JSONSerialization.jsonObject(with: Data(session.agent(line).utf8)) as? [String: Any]) ?? [:]
        guard let ticket = (reply["ticket"] as? NSNumber)?.uint64Value else {
            fputs("exact: iframe \(id): \(capability) answered as Cancel: \(reply["error"] as? String ?? "not held")\n", stderr)
            return deliver(entry, ["token": token, "cancel": true])
        }
        held[ticket] = (id, token, capability, args)
        entry.asked = true
    }

    /// `tap @t` or `type @t` answered a guest request: the arm gets the
    /// person's answer as the platform's sheet or panel would give it. False
    /// when the ticket is not one of these.
    func answered(_ reply: [String: Any], request: [String: Any]) -> Bool {
        guard let ticket = (reply["ticket"] as? NSNumber)?.uint64Value, let asked = held.removeValue(forKey: ticket) else { return false }
        guard let entry = entries[asked.view] else { return true }
        var answer: [String: Any] = ["token": asked.token]
        let text = request["text"] as? String
        switch (reply["answered"] as? String, asked.capability) {
        case ("cancel", _), (nil, _): answer["cancel"] = true
        case ("value", "dialog"): answer["text"] = text ?? ""
        case (_, "dialog"): break
        case ("value", "open-file"):
            answer["paths"] = (text ?? "").split(whereSeparator: \.isNewline).map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
        case ("value", "save-file"):
            let path = (text ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
            answer["path"] = path
            answer["shown"] = path
        case (_, "save-file"):
            // The agent's default place (§10): the app's own temporary files,
            // under a name no earlier download there has.
            guard let folder = AppFiles.url("app:/tmp/downloads") else {
                answer["cancel"] = true
                break
            }
            let name = WebViews.freshName(asked.args["suggestedFilename"] as? String ?? "download", in: folder, taken: reserved)
            reserved.insert(name)
            let shown = "app:/tmp/downloads/\(name)", url = folder.appendingPathComponent(name)
            answer["path"] = url.path
            answer["shown"] = shown
        default: answer["cancel"] = true
        }
        deliver(entry, answer)
        return true
    }

    /// `name`, or Finder's next free form of it in `folder`: `report.txt`,
    /// then `report 2.txt`, `report 3.txt`; `taken` are names already given.
    static func freshName(_ name: String, in folder: URL, taken: Set<String> = []) -> String {
        let base = (name as NSString).deletingPathExtension, ext = (name as NSString).pathExtension
        var candidate = name, n = 1
        while taken.contains(candidate) || FileManager.default.fileExists(atPath: folder.appendingPathComponent(candidate).path) {
            n += 1
            candidate = ext.isEmpty ? "\(base) \(n)" : "\(base) \(n).\(ext)"
        }
        return candidate
    }

    private func deliver(_ entry: WebEntry, _ answer: [String: Any]) {
        guard let module, let handle = entry.handle, let data = try? JSONSerialization.data(withJSONObject: answer) else { return }
        data.withUnsafeBytes { module.answer(handle, $0.bindMemory(to: UInt8.self).baseAddress, UInt32(data.count)) }
    }

    /// `navigate(id, url)`, `traverseHistory(id, delta)`, `reload(id,
    /// ignoreCache=)`, `stopLoading(id)` (@ref LLP 1020.002 D3) and
    /// `captureScreenshot(id, path, clip)` (D7) and `openDevTools(id)` (D11),
    /// to the one mounted frame with that id. A capture's `app:/` path goes
    /// with the file it names.
    func command(_ name: String, _ args: [Any]) {
        let target = args.first as? String ?? ""
        let matches = entries.values.filter { $0.owner?.props["id"] == target }
        guard matches.count == 1, let entry = matches.first, let handle = entry.handle, let module else {
            fputs("exact: \(name): no unique live iframe with id \(target)\n", stderr)
            return
        }
        var op: [String: Any] = ["op": name]
        switch name {
        case "navigate": op["url"] = args.count > 1 ? Self.browserSource(args[1] as? String) ?? "" : ""
        case "traverseHistory": op["delta"] = args.count > 1 ? (args[1] as? NSNumber)?.intValue ?? 0 : 0
        case "reload": op["ignoreCache"] = args.count > 1 ? args[1] as? Bool ?? false : false
        case "captureScreenshot":
            let path = args.count > 1 ? args[1] as? String ?? "" : ""
            op["path"] = path
            op["file"] = AppFiles.url(path)?.path ?? ""
            if args.count > 2, let clip = args[2] as? [NSNumber] { op["clip"] = clip }
        default: break
        }
        guard let data = try? JSONSerialization.data(withJSONObject: op) else { return }
        data.withUnsafeBytes { module.command(handle, $0.bindMemory(to: UInt8.self).baseAddress, UInt32(data.count)) }
    }

    fileprivate func receivedReply(token: UInt32, kind: UInt32, data: Data) {
        guard let wait = waits[token] else { return }
        if kind == 2 { wait.error = String(decoding: data, as: UTF8.self) }
        else { wait.data = data }
        wait.done = true
    }

    private func request(_ entry: WebEntry, script: String? = nil) -> Result<Data, WebRequestError> {
        guard let module, let handle = entry.handle else { return .failure(WebRequestError(message: failure ?? "web arm unavailable")) }
        let token = nextToken
        nextToken &+= 1
        let wait = WebWait()
        waits[token] = wait
        entry.asked = false
        if let script {
            let data = Data(script.utf8)
            data.withUnsafeBytes { bytes in
                module.evaluate(handle, token, bytes.bindMemory(to: UInt8.self).baseAddress, UInt32(data.count))
            }
        } else {
            module.snapshot(handle, token)
        }
        let deadline = Date(timeIntervalSinceNow: 5)
        while !wait.done && !entry.asked && Date() < deadline {
            RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.01))
        }
        waits.removeValue(forKey: token)
        // A guest script that opened a dialog waits for its answer, which the
        // agent gives by its ticket: the script's own reply is not awaited.
        if !wait.done, entry.asked { return .success(Data("{\"ok\":true,\"held\":true}".utf8)) }
        if let error = wait.error { return .failure(WebRequestError(message: error)) }
        guard wait.done, let data = wait.data else { return .failure(WebRequestError(message: "web arm reply timed out")) }
        return .success(data)
    }

    func tree(_ line: String = "{\"op\":\"tree\"}") -> [String: Any] {
        guard let session, let data = session.agent(line).data(using: .utf8),
              var root = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else { return ["error": "unreadable tree"] }
        guard var nodes = root["nodes"] as? [[String: Any]] else { return root }
        for index in nodes.indices {
            guard let id = (nodes[index]["id"] as? NSNumber)?.uint32Value,
                  let entry = entries[id]
            else { continue }
            nodes[index]["url"] = entry.src ?? ""
            nodes[index]["loading"] = entry.loading
            if let partition = entry.partition { nodes[index]["partition"] = partition }
            if let pages = entry.pdfPages {
                // A PDF frame is no browsing context: no guest outline (@ref LLP 1020.002 D10).
                nodes[index]["pdf"] = true
                nodes[index]["pages"] = pages
                continue
            }
            if let navigation = entry.navigation {
                // What a person sees of the guest, never its cookies or storage (@ref LLP 1020.002 §10).
                nodes[index]["navigation"] = navigation.filter { ["url", "title", "loading", "controller", "canGoBack", "canGoForward"].contains($0.key) }
            }
            if entry.unavailable { nodes[index]["unavailable"] = true }
            guard !entry.loading,
                  case .success(let outline) = request(entry, script: WebViews.outlineScript),
                  let guest = try? JSONSerialization.jsonObject(with: outline) as? [[String: Any]]
            else { continue }
            nodes[index]["guest"] = guest
        }
        root["nodes"] = nodes
        return root
    }

    func tap(_ owner: NodeView, request values: [String: Any], at: [Double]) -> [String: Any] {
        guard let entry = entries[owner.id] else { return ["error": "iframe \(owner.id) is unavailable"] }
        let selector = javascriptString(values["selector"] as? String)
        let x = (values["x"] as? Double).map { String($0) } ?? "innerWidth / 2"
        let y = (values["y"] as? Double).map { String($0) } ?? "innerHeight / 2"
        let script = """
        (() => {
          const x = \(x), y = \(y);
          const selector = \(selector);
          const target = (selector ? document.querySelector(selector) : null) || document.elementFromPoint(x, y) || document.body;
          if (!target) return JSON.stringify({ok:false, error:'guest tap found no target'});
          // Script input is intentionally untrusted (@ref LLP 1020 D4;
          // exact1 20260806-webview-frame-guest-click-delivery).
          target.dispatchEvent(new PointerEvent('pointerdown', {bubbles:true, composed:true, clientX:x, clientY:y, button:0, buttons:1}));
          target.dispatchEvent(new PointerEvent('pointerup', {bubbles:true, composed:true, clientX:x, clientY:y, button:0, buttons:0}));
          target.dispatchEvent(new MouseEvent('click', {bubbles:true, composed:true, clientX:x, clientY:y, button:0}));
          return JSON.stringify({ok:true});
        })()
        """
        switch guestResponse(request(entry, script: script), operation: "tap") {
        case .failure(let error):
            return ["error": error.message]
        case .success(let object):
            var result: [String: Any] = ["tapped": Int(owner.id), "guest": true, "at": at]
            // The click opened a dialog, held as this ticket.
            if object["held"] as? Bool == true, let ticket = held.filter({ $0.value.view == owner.id }).keys.max() { result["held"] = Int(ticket) }
            return result
        }
    }

    func type(_ owner: NodeView, request values: [String: Any]) -> [String: Any] {
        guard let entry = entries[owner.id] else { return ["error": "iframe \(owner.id) is unavailable"] }
        let selector = javascriptString(values["selector"] as? String)
        let key = values["key"] as? String
        let text = values["text"] as? String ?? ""
        let action: String
        if let key {
            action = "target.dispatchEvent(new KeyboardEvent('keydown', {key:\(javascriptString(key)), bubbles:true, composed:true})); target.dispatchEvent(new KeyboardEvent('keyup', {key:\(javascriptString(key)), bubbles:true, composed:true}));"
        } else {
            action = "if ('value' in target) target.value = \(javascriptString(text)); else target.textContent = \(javascriptString(text)); target.dispatchEvent(new InputEvent('input', {data:\(javascriptString(text)), inputType:'insertText', bubbles:true, composed:true})); target.dispatchEvent(new Event('change', {bubbles:true, composed:true}));"
        }
        let script = """
        (() => {
          const selector = \(selector);
          const active = document.activeElement;
          const editable = active && active.matches?.('input,textarea,[contenteditable]') ? active : null;
          const target = (selector ? document.querySelector(selector) : null) || editable || document.querySelector('input,textarea,[contenteditable]');
          if (!target) return JSON.stringify({ok:false, error:'guest type found no target'});
          // Script input is intentionally isTrusted:false (@ref LLP 1020 D4).
          target.focus();
          \(action)
          // A password's value is never agent output (#134): a fixed mark.
          return JSON.stringify({ok:true, value:'value' in target ? (target.type === 'password' && target.value ? '•••' : target.value) : target.textContent});
        })()
        """
        let object: [String: Any]
        switch guestResponse(request(entry, script: script), operation: "type") {
        case .failure(let error):
            return ["error": error.message]
        case .success(let response):
            object = response
        }
        var result: [String: Any] = ["typed": Int(owner.id), "guest": true]
        if let value = object["value"] { result["value"] = value }
        if let key { result["key"] = key }
        return result
    }

    private func guestResponse(
        _ response: Result<Data, WebRequestError>, operation: String
    ) -> Result<[String: Any], WebRequestError> {
        switch response {
        case .failure(let error):
            return .failure(error)
        case .success(let data):
            guard let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
                return .failure(WebRequestError(message: "guest \(operation) returned an unreadable reply"))
            }
            guard object["ok"] as? Bool == true else {
                return .failure(WebRequestError(
                    message: object["error"] as? String ?? "guest \(operation) found no target"))
            }
            return .success(object)
        }
    }

    func snapshots() -> [UInt32: ExactWebImage] {
        var out: [UInt32: ExactWebImage] = [:]
        for (id, entry) in entries.sorted(by: { $0.key < $1.key }) {
            guard entry.owner?.window != nil else { continue }
            let data: Data
            switch request(entry) {
            case .success(let bytes): data = bytes
            case .failure(let error):
                FileHandle.standardError.write(Data("exact: iframe \(id) snapshot: \(error.message)\n".utf8))
                continue
            }
            #if os(macOS)
            if let image = NSImage(data: data) { out[id] = image }
            #else
            if let image = UIImage(data: data) { out[id] = image }
            #endif
        }
        return out
    }

    private static let outlineScript = """
    (() => {
      const out = [];
      const visit = (el, depth) => {
        if (depth > 4 || out.length >= 32) return;
        const id = el.id || undefined;
        const testId = el.getAttribute('data-testid') || el.getAttribute('testId') || undefined;
        const text = Array.from(el.childNodes).filter(n => n.nodeType === Node.TEXT_NODE).map(n => n.textContent.trim()).filter(Boolean).join(' ').replace(/\\s+/g, ' ').slice(0, 160) || undefined;
        if (id || testId || text) out.push({guest:true, depth, tag:el.localName, ...(id ? {id} : {}), ...(testId ? {testId} : {}), ...(text ? {text} : {})});
        for (const child of el.children) visit(child, depth + 1);
      };
      for (const child of document.body?.children || []) visit(child, 0);
      return JSON.stringify(out);
    })()
    """

    private func javascriptString(_ value: String?) -> String {
        guard let value,
              let data = try? JSONSerialization.data(withJSONObject: [value]),
              var text = String(data: data, encoding: .utf8)
        else { return "null" }
        text.removeFirst()
        text.removeLast()
        return text
    }
}

extension ExactSession {
    /// `postMessage(text, target)`: the compiler marks an iframe target; a name that is both a
    /// live frame's id and a canvas surface, or a canvas post to a frame's id, is refused.
    func post(_ text: String, to target: String, iframe: Bool) {
        if webviews.hasFrame(target) && canvases.hasSurface(target) {
            fputs("exact: postMessage: ambiguous iframe id and canvas surface \(target)\n", stderr)
        } else if iframe {
            webviews.postMessage(target, text)
        } else if webviews.hasFrame(target) {
            fputs("exact: postMessage: canvas name collides with an iframe id \(target)\n", stderr)
        } else {
            canvases.post(target, text)
        }
    }
}
