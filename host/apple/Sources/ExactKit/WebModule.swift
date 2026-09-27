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

private struct WebLoadError: Error { let message: String }
private struct WebRequestError: Error { let message: String }

private final class WebModule {
    typealias CreateFn = @convention(c) (UInt32, UnsafeMutableRawPointer?, WebEventFn?, WebReplyFn?) -> UnsafeMutableRawPointer?
    typealias PlatformViewFn = @convention(c) (UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer?
    typealias SetFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UInt32) -> Void
    typealias SnapshotFn = @convention(c) (UnsafeMutableRawPointer?, UInt32) -> Void
    typealias EvalFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UnsafePointer<UInt8>?, UInt32) -> Void
    typealias DestroyFn = @convention(c) (UnsafeMutableRawPointer?) -> Void

    let library: UnsafeMutableRawPointer
    let create: CreateFn
    let platformView: PlatformViewFn
    let setSrc: SetFn
    let setDocument: SetFn
    let setSandbox: SetFn
    let snapshot: SnapshotFn
    let evaluate: EvalFn
    let destroy: DestroyFn

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
              let setSandbox = symbol("exact_web_set_sandbox", SetFn.self),
              let snapshot = symbol("exact_web_snapshot", SnapshotFn.self),
              let evaluate = symbol("exact_web_agent_eval", EvalFn.self),
              let destroy = symbol("exact_web_destroy", DestroyFn.self)
        else {
            dlclose(library)
            return .failure(WebLoadError(message: "\(path) is not an exact web arm (missing exports)"))
        }
        return .success(WebModule(
            library: library, create: create, platformView: platformView,
            setSrc: setSrc, setDocument: setDocument, setSandbox: setSandbox, snapshot: snapshot,
            evaluate: evaluate, destroy: destroy))
    }

    private init(
        library: UnsafeMutableRawPointer, create: @escaping CreateFn,
        platformView: @escaping PlatformViewFn, setSrc: @escaping SetFn, setDocument: @escaping SetFn,
        setSandbox: @escaping SetFn, snapshot: @escaping SnapshotFn,
        evaluate: @escaping EvalFn, destroy: @escaping DestroyFn
    ) {
        self.library = library
        self.create = create
        self.platformView = platformView
        self.setSrc = setSrc
        self.setDocument = setDocument
        self.setSandbox = setSandbox
        self.snapshot = snapshot
        self.evaluate = evaluate
        self.destroy = destroy
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
    var initialized = false
    var loading = true
    var unavailable = false

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
    private var nextToken: UInt32 = 1
    private(set) var failure: String?

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
        switch WebModule.load(path: WebViews.modulePath()) {
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

    func update(_ owner: NodeView) {
        guard let entry = entries[owner.id] else { return }
        let src = Self.browserSource(owner.props["src"])
        let sandbox = owner.props["sandbox"]
        let changedSrc = !entry.initialized || entry.src != src
        let changedSandbox = !entry.initialized || entry.sandbox != sandbox
        entry.src = src
        entry.sandbox = sandbox
        entry.initialized = true
        guard let module, let handle = entry.handle else { return }
        if changedSrc || changedSandbox { entry.loading = true }
        if let src, URL(string: src)?.scheme == nil, !src.hasPrefix("//"), let resolver = session?.app.resolver {
            let path = src.components(separatedBy: "?")[0].components(separatedBy: "#")[0]
            let name = path.hasPrefix("/") ? String(path.dropFirst()) : path
            let document = resolver.bytes(name).map { String(decoding: $0, as: UTF8.self) } ?? ""
            send(document, to: handle, using: module.setDocument)
        } else { send(nil, to: handle, using: module.setDocument) }
        if changedSandbox { send(sandbox, to: handle, using: module.setSandbox) }
        if changedSrc { send(src, to: handle, using: module.setSrc) }
    }

    private static func browserSource(_ source: String?) -> String? {
        guard let source, source.hasPrefix("//") else { return source }
        let scheme = ExactApp.shared.connectedPage?.scheme?.lowercased()
        return (scheme == "http" || scheme == "https" ? scheme! : "https") + ":" + source
    }

    private func send(_ value: String?, to handle: UnsafeMutableRawPointer, using setter: WebModule.SetFn) {
        guard let value else { setter(handle, nil, 0, 0); return }
        let data = Data(value.utf8)
        data.withUnsafeBytes { bytes in
            setter(handle, bytes.bindMemory(to: UInt8.self).baseAddress, UInt32(data.count), 1)
        }
    }

    func destroy(id: UInt32) {
        guard let entry = entries[id] else { return }
        if let handle = entry.handle { module?.destroy(handle) }
        entries.removeValue(forKey: id)
    }

    func reset() {
        for id in Array(entries.keys) { destroy(id: id) }
    }

    fileprivate func receivedEvent(id: UInt32, kind: UInt32, data: Data) {
        guard let entry = entries[id], let owner = entry.owner else { return }
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
        default: break
        }
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
        if let script {
            let data = Data(script.utf8)
            data.withUnsafeBytes { bytes in
                module.evaluate(handle, token, bytes.bindMemory(to: UInt8.self).baseAddress, UInt32(data.count))
            }
        } else {
            module.snapshot(handle, token)
        }
        let deadline = Date(timeIntervalSinceNow: 5)
        while !wait.done && Date() < deadline {
            RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.01))
        }
        waits.removeValue(forKey: token)
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
        case .success:
            return ["tapped": Int(owner.id), "guest": true, "at": at]
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
          return JSON.stringify({ok:true, value:'value' in target ? target.value : target.textContent});
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
