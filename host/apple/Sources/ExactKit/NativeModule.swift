// Native modules on Apple hosts (@ref LLP 1024 D2–D5): one NativeView arm,
// one app module artifact behind `dlopen`, loaded after the first painted
// frame, and a versioned C function table whose tag → factory roster is
// looked up by name. Shared by the AppKit and UIKit presenters.
//
// The table (`exact_native_abi()`, 64-bit layout; the module side is
// `host/apple/modules/ExactNativeModule.swift`):
//
//   0  u32 major            1
//   4  u32 size             72 or more
//   8  const char *roster   JSON: {"tag": {"snapshot": bool}, …}
//  16  create(tag, tagLen, props, propsLen, event, reply, ctx, nonce, err, errCap) → handle
//  24  platform_view(handle) → NSView * / UIView *   (the module keeps ownership)
//  32  set_props(handle, json, len, err, errCap) → 0 accepted, else refused
//  40  snapshot(handle, token)          nullable; answered on `reply`
//  48  destroy(handle)
//  56  set_bounds                       reserved, NULL (LLP 1024 §5)
//  64  agent_input                      reserved, NULL
//
//   event(ctx, nonce, kind, bytes, len)          kind: EventKind 0–8 — press,
//     change, hover, focus, blur, key, submit, load, message; change, key and
//     message carry UTF-8, hover "true"/"false"; from any thread.
//   reply(ctx, nonce, token, kind, bytes, len)   kind 0 PNG bytes, 2 error text.
//
// Every entry is called on the main thread. Callbacks may come from any
// thread: the bytes are copied, the call hops to the main queue, and an
// invalidated nonce (a destroyed instance) is dropped and logged. The
// artifact is never closed (D5).
import Foundation
#if os(macOS)
import AppKit
typealias NativePlatformView = NSView
typealias NativeImage = NSImage
#else
import UIKit
typealias NativePlatformView = UIView
typealias NativeImage = UIImage
#endif

private typealias NativeEventFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UInt32, UnsafePointer<UInt8>?, UInt32) -> Void
private typealias NativeReplyFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UInt32, UInt32, UnsafePointer<UInt8>?, UInt32) -> Void

private struct NativeFailure: Error { let state: String; let message: String }

/// The loaded table: the roster and the entries, read once.
private final class NativeTable {
    static let major: UInt32 = 1
    typealias CreateFn = @convention(c) (UnsafePointer<UInt8>?, UInt32, UnsafePointer<UInt8>?, UInt32, NativeEventFn?, NativeReplyFn?, UnsafeMutableRawPointer?, UInt32, UnsafeMutablePointer<UInt8>?, UInt32) -> UnsafeMutableRawPointer?
    typealias ViewFn = @convention(c) (UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer?
    typealias SetFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UnsafeMutablePointer<UInt8>?, UInt32) -> Int32
    typealias SnapshotFn = @convention(c) (UnsafeMutableRawPointer?, UInt32) -> Void
    typealias DestroyFn = @convention(c) (UnsafeMutableRawPointer?) -> Void
    typealias AbiFn = @convention(c) () -> UnsafeRawPointer?

    let path: String
    let roster: [String: [String: Any]]
    let create: CreateFn
    let platformView: ViewFn
    let setProps: SetFn
    let snapshot: SnapshotFn?
    let destroy: DestroyFn

    private init(path: String, roster: [String: [String: Any]], create: @escaping CreateFn, platformView: @escaping ViewFn,
                 setProps: @escaping SetFn, snapshot: SnapshotFn?, destroy: @escaping DestroyFn) {
        self.path = path; self.roster = roster; self.create = create; self.platformView = platformView
        self.setProps = setProps; self.snapshot = snapshot; self.destroy = destroy
    }

    static func load(path: String) -> Result<NativeTable, NativeFailure> {
        guard FileManager.default.fileExists(atPath: path) else {
            return .failure(NativeFailure(state: "unavailable", message: "no module artifact at \(path)"))
        }
        guard let library = dlopen(path, RTLD_NOW | RTLD_LOCAL) else {
            return .failure(NativeFailure(state: "unavailable", message: "dlopen \(path): \(String(cString: dlerror()))"))
        }
        // Never dlclosed, even on refusal: nothing in v1 unloads (D5).
        guard let entry = dlsym(library, "exact_native_abi"),
              let table = unsafeBitCast(entry, to: AbiFn.self)()
        else { return .failure(NativeFailure(state: "unavailable", message: "\(path) exports no exact_native_abi table")) }
        let major = table.load(as: UInt32.self), size = table.load(fromByteOffset: 4, as: UInt32.self)
        guard major == NativeTable.major else {
            return .failure(NativeFailure(state: "unavailable", message: "module ABI \(major), host ABI \(NativeTable.major)"))
        }
        guard size >= 72 else { return .failure(NativeFailure(state: "unavailable", message: "module table of \(size) bytes, host needs 72")) }
        func pointer(_ offset: Int) -> UnsafeRawPointer? { table.load(fromByteOffset: offset, as: UnsafeRawPointer?.self) }
        guard let rosterText = pointer(8).map({ String(cString: $0.assumingMemoryBound(to: CChar.self)) }),
              let roster = try? JSONSerialization.jsonObject(with: Data(rosterText.utf8)) as? [String: [String: Any]]
        else { return .failure(NativeFailure(state: "unavailable", message: "\(path): unreadable roster")) }
        guard let create = pointer(16), let view = pointer(24), let set = pointer(32), let destroy = pointer(48) else {
            return .failure(NativeFailure(state: "unavailable", message: "\(path): the table lacks a required entry"))
        }
        return .success(NativeTable(
            path: path, roster: roster, create: unsafeBitCast(create, to: CreateFn.self),
            platformView: unsafeBitCast(view, to: ViewFn.self), setProps: unsafeBitCast(set, to: SetFn.self),
            snapshot: pointer(40).map { unsafeBitCast($0, to: SnapshotFn.self) },
            destroy: unsafeBitCast(destroy, to: DestroyFn.self)))
    }
}

private final class NativeEntry {
    weak var owner: NodeView?
    let id: UInt32
    var name = ""
    var state = "loading"
    var error: String?
    var nonce: UInt32 = 0
    var handle: UnsafeMutableRawPointer?
    var view: NativePlatformView?
    var props = "{}"
    var snapshotBit = false
    init(owner: NodeView) { self.owner = owner; self.id = owner.id }
    var status: [String: Any] {
        var s: [String: Any] = ["name": name, "state": state]
        if let error { s["error"] = error }
        return s
    }
}

private final class NativeWait { var data: Data?; var error: String?; var done = false }

/// Process-wide: the one artifact, and every live nonce → its manager.
private enum NativeProcess {
    nonisolated(unsafe) static var table: Result<NativeTable, NativeFailure>?
    nonisolated(unsafe) static var owners: [UInt32: WeakNatives] = [:]
    nonisolated(unsafe) static var retired: [UInt32: WeakNatives] = [:]
    nonisolated(unsafe) static var next: UInt32 = 1
}
private final class WeakNatives { weak var natives: NativeViews?; init(_ n: NativeViews) { natives = n } }

private let nativeEventCallback: NativeEventFn = { _, nonce, kind, bytes, length in
    let data = bytes.map { Data(bytes: $0, count: Int(length)) } ?? Data()
    // Never synchronously: the host enters the runner through the presenter's gate.
    DispatchQueue.main.async {
        if let natives = NativeProcess.owners[nonce]?.natives { natives.received(nonce: nonce, kind: kind, data: data) }
        else { NativeProcess.retired[nonce]?.natives?.dropped(nonce: nonce, kind: kind) }
    }
}

private let nativeReplyCallback: NativeReplyFn = { _, nonce, token, kind, bytes, length in
    let data = bytes.map { Data(bytes: $0, count: Int(length)) } ?? Data()
    let deliver: () -> Void = { NativeProcess.owners[nonce]?.natives?.replied(token: token, kind: kind, data: data) }
    if Thread.isMainThread { deliver() } else { DispatchQueue.main.async(execute: deliver) }
}

final class NativeViews {
    weak var session: ExactSession?
    private var entries: [UInt32: NativeEntry] = [:]
    private var gateOpen = false
    private var waits: [UInt32: NativeWait] = [:]
    private var nextToken: UInt32 = 1
    private static let kinds = ["press", "change", "hover", "focus", "blur", "key", "submit", "load", "message"]

    private func log(_ line: String) {
        session?.log("native \(line)")
        if ExactEnv.environment["EXACT_NATIVE_TRACE"] == "1" { fputs("exact native: \(line)\n", stderr) }
    }

    /// The artifact's path: beside the executable (macOS) or in Frameworks
    /// (iOS); a development build may name another file (`EXACT_MODULES`,
    /// a file, never a directory: D5). The module name never enters a path.
    static func modulePath(session: ExactSession?) -> String {
        #if os(macOS)
        let standard = Bundle.main.executableURL!.deletingLastPathComponent().appendingPathComponent("libexact_modules.dylib").path
        #else
        let standard = (Bundle.main.privateFrameworksPath ?? Bundle.main.bundlePath) + "/libexact_modules.dylib"
        #endif
        let trust = ((GpuModule.bakedCompatibility["inputs"] as? [String: Any])?["trust"] as? String) ?? "development"
        guard trust != "production", let override = ExactEnv.environment["EXACT_MODULES"], !override.isEmpty else { return standard }
        return override
    }

    private static func admitted(_ name: String) -> Bool {
        let words = name.split(separator: "-", omittingEmptySubsequences: false)
        let reserved: Set<String> = ["annotation-xml", "color-profile", "font-face", "font-face-src", "font-face-uri", "font-face-format", "font-face-name", "missing-glyph"]
        return words.count > 1 && !reserved.contains(name) && words.allSatisfy { w in
            guard let first = w.unicodeScalars.first, ("a"..."z").contains(first) else { return false }
            return w.unicodeScalars.allSatisfy { ("a"..."z").contains($0) || ("0"..."9").contains($0) || $0 == "_" }
        }
    }

    /// A NativeView's create commit: the box exists now; the module attaches
    /// at the paint gate, or at once when the gate has opened (D3).
    func create(owner: NodeView) {
        entries[owner.id] = NativeEntry(owner: owner)
    }

    /// The paint gate: the turn after the first drawn frame (the GPU
    /// module's), and every later batch. The first call loads the artifact.
    func loadIfNeeded() {
        guard !gateOpen, !entries.isEmpty else { return }
        gateOpen = true
        for entry in entries.values.sorted(by: { $0.id < $1.id }) where entry.state == "loading" && !entry.name.isEmpty { attach(entry) }
    }

    private func table() -> Result<NativeTable, NativeFailure> {
        if let loaded = NativeProcess.table { return loaded }
        let path = NativeViews.modulePath(session: session)
        let after = session?.firstDrawMs.map { String(format: "%.1f", ExactEnv.wall() - $0) } ?? "?"
        log("loading \(path) \(after) ms after first pixel")
        let loaded = NativeTable.load(path: path)
        NativeProcess.table = loaded
        switch loaded {
        case .success(let t): log("loaded \((t.path as NSString).lastPathComponent): \(t.roster.keys.sorted().joined(separator: ", "))")
        case .failure(let f): log("\(f.state): \(f.message)")
        }
        return loaded
    }

    private func fail(_ entry: NativeEntry, _ state: String, _ message: String) {
        entry.state = state
        entry.error = message
        log("\(entry.name) #\(entry.id): \(state): \(message)")
    }

    private func attach(_ entry: NativeEntry) {
        guard let owner = entry.owner, entries[entry.id] === entry else { return }
        // A key check on plan bytes: a doctored plan that skipped the bake dies here.
        guard NativeViews.admitted(entry.name) else { return fail(entry, "error", "refused module name \"\(entry.name)\"") }
        let table: NativeTable
        switch self.table() {
        case .failure(let f): return fail(entry, f.state, f.message)
        case .success(let t): table = t
        }
        guard let caps = table.roster[entry.name] else { return fail(entry, "error", "the module artifact has no factory for \(entry.name)") }
        entry.snapshotBit = caps["snapshot"] as? Bool == true && table.snapshot != nil
        let nonce = NativeProcess.next
        NativeProcess.next &+= 1
        NativeProcess.owners[nonce] = WeakNatives(self)
        entry.nonce = nonce
        let props = owner.props["nativeViewProps"] ?? "{}"
        var error = [UInt8](repeating: 0, count: 512)
        let tag = Data(entry.name.utf8), json = Data(props.utf8)
        let handle = tag.withUnsafeBytes { t in json.withUnsafeBytes { p in
            table.create(t.bindMemory(to: UInt8.self).baseAddress, UInt32(tag.count), p.bindMemory(to: UInt8.self).baseAddress, UInt32(json.count),
                         nativeEventCallback, nativeReplyCallback, nil, nonce, &error, UInt32(error.count))
        } }
        guard let handle else {
            NativeProcess.owners.removeValue(forKey: nonce)
            entry.nonce = 0
            return fail(entry, "error", "create refused: \(String(cString: error.map { CChar(bitPattern: $0) }))")
        }
        guard let raw = table.platformView(handle) else {
            table.destroy(handle)
            NativeProcess.owners.removeValue(forKey: nonce)
            entry.nonce = 0
            return fail(entry, "error", "\(entry.name) returned no platform view")
        }
        let view = Unmanaged<NativePlatformView>.fromOpaque(raw).takeUnretainedValue()
        // The host sizes the box; the platform view fills it (bounds are observed, D4).
        view.frame = owner.bounds
        #if os(macOS)
        view.autoresizingMask = [.width, .height]
        #else
        view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        #endif
        owner.addSubview(view)
        entry.handle = handle
        entry.view = view
        entry.props = props
        entry.state = "ready"
        entry.error = nil
        log("\(entry.name) #\(entry.id): ready")
    }

    /// A props commit: the first names the module (the create commit carries
    /// no props yet); later ones replace the whole aggregate.
    func update(_ owner: NodeView) {
        guard let entry = entries[owner.id] else { return }
        if entry.name.isEmpty, entry.state == "loading" {
            entry.name = owner.props["nativeViewModuleName"] ?? ""
            log("\(entry.name) #\(owner.id): loading")
            if gateOpen { attach(entry) }
            return
        }
        guard let handle = entry.handle, case .success(let table)? = NativeProcess.table else { return }
        let props = owner.props["nativeViewProps"] ?? "{}"
        guard props != entry.props else { return }
        var error = [UInt8](repeating: 0, count: 512)
        let json = Data(props.utf8)
        let status = json.withUnsafeBytes { p in table.setProps(handle, p.bindMemory(to: UInt8.self).baseAddress, UInt32(json.count), &error, UInt32(error.count)) }
        if status == 0 {
            entry.props = props
            if entry.state == "error" { entry.state = "ready"; entry.error = nil }
        } else {
            fail(entry, "error", "props refused: \(String(cString: error.map { CChar(bitPattern: $0) }))")
        }
    }

    /// The node is gone: the nonce dies first, then the instance (D4).
    func destroy(id: UInt32) {
        guard let entry = entries.removeValue(forKey: id) else { return }
        if entry.nonce != 0 {
            NativeProcess.owners.removeValue(forKey: entry.nonce)
            NativeProcess.retired[entry.nonce] = WeakNatives(self)
        }
        entry.view?.removeFromSuperview()
        if let handle = entry.handle, case .success(let table)? = NativeProcess.table {
            table.destroy(handle)
            log("\(entry.name) #\(id): destroyed")
        }
        entry.handle = nil
    }

    fileprivate func dropped(nonce: UInt32, kind: UInt32) {
        let name = kind < NativeViews.kinds.count ? NativeViews.kinds[Int(kind)] : "kind \(kind)"
        log("dropped \(name) from nonce \(nonce) after destroy")
    }

    fileprivate func received(nonce: UInt32, kind: UInt32, data: Data) {
        guard let entry = entries.values.first(where: { $0.nonce == nonce }), let owner = entry.owner,
              let presenter = owner.presenter, presenter.views[entry.id] === owner
        else { return dropped(nonce: nonce, kind: kind) }
        guard kind < NativeViews.kinds.count else { return log("\(entry.name) #\(entry.id): refused event kind \(kind)") }
        let name = NativeViews.kinds[Int(kind)], text = String(decoding: data, as: UTF8.self), id = entry.id
        guard owner.handlers.contains(name) else { return }
        switch kind {
        case 0: presenter.press(id)
        case 1: presenter.change(id, text)
        case 2: presenter.hover(owner, text == "true")
        case 3: presenter.focus(id)
        case 4: presenter.blur(id)
        case 5: presenter.key(id, text)
        case 6: presenter.submit(id)
        case 7: presenter.load(id)
        default: presenter.message(id, text)
        }
    }

    fileprivate func replied(token: UInt32, kind: UInt32, data: Data) {
        guard let wait = waits[token] else { return }
        if kind == 2 { wait.error = String(decoding: data, as: UTF8.self) } else { wait.data = data }
        wait.done = true
    }

    /// `tree`: each NativeView's status object (D2), `loading` until the gate.
    func decorate(_ tree: [String: Any]) -> [String: Any] {
        guard var nodes = tree["nodes"] as? [[String: Any]] else { return tree }
        for index in nodes.indices where nodes[index]["type"] as? String == "NativeView" {
            guard let id = (nodes[index]["id"] as? NSNumber)?.uint32Value else { continue }
            let name = (nodes[index]["props"] as? [String: Any])?["nativeViewModuleName"] as? String ?? ""
            nodes[index]["module"] = entries[id]?.status ?? ["name": name, "state": "loading"]
        }
        var out = tree
        out["nodes"] = nodes
        return out
    }

    /// The platform views whose tag answers snapshots: hidden while the
    /// capture draws their pictures instead (Metal- and remote-layer views).
    var snapshotViews: [NativePlatformView] { entries.values.filter(\.snapshotBit).compactMap(\.view) }

    /// One tokened snapshot per snapshot-bit instance on screen, for this capture.
    func snapshots() -> [UInt32: NativeImage] {
        guard case .success(let table)? = NativeProcess.table, let snapshot = table.snapshot else { return [:] }
        var out: [UInt32: NativeImage] = [:]
        for entry in entries.values.sorted(by: { $0.id < $1.id }) where entry.snapshotBit {
            guard let handle = entry.handle, entry.owner?.window != nil else { continue }
            let token = nextToken
            nextToken &+= 1
            let wait = NativeWait()
            waits[token] = wait
            snapshot(handle, token)
            let deadline = Date(timeIntervalSinceNow: 5)
            while !wait.done && Date() < deadline { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.01)) }
            waits.removeValue(forKey: token)
            if let data = wait.data, let image = NativeImage(data: data) {
                out[entry.id] = image
                log("\(entry.name) #\(entry.id): snapshot token \(token), \(data.count) bytes")
            } else {
                log("\(entry.name) #\(entry.id): snapshot token \(token) failed: \(wait.error ?? "timed out")")
            }
        }
        return out
    }
}

extension NodeView {
    /// The embedded platform content a node's kind brings: an iframe's web
    /// view (LLP 1020 D3) or a native module's box (LLP 1024 D2).
    func embedPlatformView(_ presenter: Presenter) {
        if kind == "native" { presenter.session?.natives.create(owner: self); return }
        guard kind == "iframe", let w = presenter.session?.webviews.create(owner: self) else { return }
        w.frame = bounds
        #if os(macOS)
        w.autoresizingMask = [.width, .height]
        w.wantsLayer = true
        #else
        w.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        #endif
        addSubview(w)
        web = w
    }

    func updateEmbedded() {
        if kind == "iframe" { presenter?.session?.webviews.update(self) }
        if kind == "native" { presenter?.session?.natives.update(self) }
    }

    func destroyEmbedded() {
        presenter?.session?.webviews.destroy(id: id)
        presenter?.session?.natives.destroy(id: id)
    }
}
