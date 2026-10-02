// Native modules on Apple hosts (@ref LLP 1024 D2–D5, LLP 1067.000): one
// NativeView arm, one app module artifact behind `dlopen`, loaded after the
// first painted frame or at the first long call, and a versioned C function
// table whose tag → factory roster is looked up by name. The artifact's
// module is instantiated once per session (Q6): its views receive it, and a
// source's `native.later` calls reach it through the runtime
// (`exact_set_app_module`). Shared by the AppKit and UIKit presenters.
//
// The table (`exact_native_abi()`, 64-bit layout; the module side is
// `host/apple/modules/ExactNativeModule.swift`):
//
//   0  u32 major            2
//   4  u32 size             104 or more
//   8  const char *roster   JSON: {"tag": {"snapshot": bool}, …}
//  16  create(module, tag, tagLen, props, propsLen, event, reply, ctx, nonce, err, errCap) → handle
//  24  platform_view(handle) → NSView * / UIView *   (the module keeps ownership)
//  32  set_props(handle, json, len, err, errCap) → 0 accepted, else refused
//  40  snapshot(handle, token)          nullable; answered on `reply`
//  48  destroy(handle)
//  56  set_bounds                       reserved, NULL (LLP 1024 §5)
//  64  agent_input                      reserved, NULL
//  72  module_create(json, len, host, changed, now, err, errCap) → module
//        json: {"agent", "data", "cache", "temporary"}; changed(host, topic,
//        len) from any thread; now(host) → the session clock, main thread
//  80  module_destroy(module)
//  88  module_later(module, body, len, reply, answer)
//        answer(reply, status, bytes, len) once, any thread: exact_app_reply
//  96  module_call(module, body, len, slot, answer)
//        answer(slot, status, bytes, len) before returning: exact_app_answer
// 104  prepare_for_reuse(handle) → 0 reset, else refused   size ≥ 112; nullable
//        (LLP 1068 §4.8): as if created with no props; the next set_props is
//        a first mount, and `load` follows once no pixel of the last row shows
// 112  module_connect(module, host_table)       size ≥ 136; the hooks (LLP
//        1075.003 §3.2), NativeHooks.swift: the host's callbacks, once
// 120  module_navigation(module, event, controller, flags) → flags
//        event 0 built (the hook runs), 1 retired; bit 0 showsBar
// 128  module_route(module, event, controller, navigation, scroll, json, len)
//        event 0 built, 1 changed, 2 ended; json {"key", "data": {…}}
// 136  module_tabs(module, event, controller, index)      size ≥ 160
//        event 0 built, 1 retired, 2 the router selected `index` in the
//        app's container, 3 the app's container retired
// 144  module_tab_container(module, json, len, controllers, count) → container
//        json {"names", "nodes", "selected"}; retained once, or nil (Exact's)
// 152  platform_controller(handle) → UIViewController *, a native screen's
// 160  module_element(module, event, view, platform, json, len)   size ≥ 168
//        (LLP 1075.003.000): a node marked `hook="word"`; event 0 built,
//        1 changed, 2 ended; json {"hook", "node", "id", "kind", "data"}
// 168  module_toolbar(module, toolbar, window)        size ≥ 176; macOS:
//        the window toolbar Exact installed (LLP 1075.003.000 §3.7)
//
//   event(ctx, nonce, kind, bytes, len)          kind: EventKind 0–8 — press,
//     change, hover, focus, blur, key, submit, load, message; change, key and
//     message carry UTF-8, hover "true"/"false"; from any thread.
//   reply(ctx, nonce, token, kind, bytes, len)   kind 0 PNG bytes, 2 error text.
//
// Every entry is called on the main thread (LLP 1067.000 Q5). Callbacks may come from any
// thread: the bytes are copied, the call hops to the main queue, and an
// invalidated nonce (a destroyed instance) is dropped and logged. The
// artifact is never closed (D5).
//
// Reuse (iOS, LLP 1068 §4.8, §4.9): a tag whose roster entry says `reuse`
// is pooled by tag (the app has one artifact). At a destroy the instance's
// incarnation ends first (a callback it issues from then on is dropped),
// then `prepare_for_reuse` resets it and its view leaves the window; a
// later node of the tag takes it under a never-used incarnation, its props
// applied as a first mount, the view transparent until the instance's
// `load`. A module's nonce is fixed for its instance's life; each callback
// captures the incarnation current for that nonce when it is issued, and is
// delivered only to that one.
import CExact
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
    static let major: UInt32 = 2
    static let size: UInt32 = 104
    typealias CreateFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UnsafePointer<UInt8>?, UInt32, NativeEventFn?, NativeReplyFn?, UnsafeMutableRawPointer?, UInt32, UnsafeMutablePointer<UInt8>?, UInt32) -> UnsafeMutableRawPointer?
    typealias ViewFn = @convention(c) (UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer?
    typealias SetFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UnsafeMutablePointer<UInt8>?, UInt32) -> Int32
    typealias SnapshotFn = @convention(c) (UnsafeMutableRawPointer?, UInt32) -> Void
    typealias DestroyFn = @convention(c) (UnsafeMutableRawPointer?) -> Void
    typealias ReuseFn = @convention(c) (UnsafeMutableRawPointer?) -> Int32
    typealias AbiFn = @convention(c) () -> UnsafeRawPointer?
    typealias ChangedFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32) -> Void
    typealias NowFn = @convention(c) (UnsafeMutableRawPointer?) -> Double
    typealias AnswerFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UnsafePointer<UInt8>?, Int) -> Void
    typealias ModuleCreateFn = @convention(c) (UnsafePointer<UInt8>?, UInt32, UnsafeMutableRawPointer?, ChangedFn?, NowFn?, UnsafeMutablePointer<UInt8>?, UInt32) -> UnsafeMutableRawPointer?
    typealias ModuleDestroyFn = @convention(c) (UnsafeMutableRawPointer?) -> Void
    typealias ModuleLaterFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, Int, UnsafeMutableRawPointer?, AnswerFn?) -> Void

    let path: String
    let roster: [String: [String: Any]]
    let create: CreateFn
    let platformView: ViewFn
    let setProps: SetFn
    let snapshot: SnapshotFn?
    let destroy: DestroyFn
    let moduleCreate: ModuleCreateFn
    let moduleDestroy: ModuleDestroyFn
    let moduleLater: ModuleLaterFn
    let moduleCall: ModuleLaterFn
    var prepareForReuse: ReuseFn?
    /// The hooks (LLP 1075.003 §3.2), in a table of 136 bytes or more.
    var connect: HookConnectFn?, navigationHook: HookNavigationFn?, routeHook: HookRouteFn?
    /// Tabs and native screens, in a table of 160 bytes or more.
    var tabsHook: HookTabsFn?, tabContainerHook: HookTabContainerFn?, platformController: ViewFn?
    /// Hooked nodes (LLP 1075.003.000), in a table of 168 bytes or more;
    /// the window toolbar's hook (macOS), in one of 176 or more.
    var elementHook: HookElementFn?, toolbarHook: HookToolbarFn?

    private init(path: String, roster: [String: [String: Any]], create: @escaping CreateFn, platformView: @escaping ViewFn,
                 setProps: @escaping SetFn, snapshot: SnapshotFn?, destroy: @escaping DestroyFn,
                 moduleCreate: @escaping ModuleCreateFn, moduleDestroy: @escaping ModuleDestroyFn, moduleLater: @escaping ModuleLaterFn, moduleCall: @escaping ModuleLaterFn) {
        self.path = path; self.roster = roster; self.create = create; self.platformView = platformView
        self.setProps = setProps; self.snapshot = snapshot; self.destroy = destroy
        self.moduleCreate = moduleCreate; self.moduleDestroy = moduleDestroy; self.moduleLater = moduleLater; self.moduleCall = moduleCall
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
        return read(table, path: path)
    }

    /// A table in memory: the artifact's, or a test's.
    static func read(_ table: UnsafeRawPointer, path: String) -> Result<NativeTable, NativeFailure> {
        let major = table.load(as: UInt32.self), size = table.load(fromByteOffset: 4, as: UInt32.self)
        guard major == NativeTable.major else {
            return .failure(NativeFailure(state: "unavailable", message: "module ABI \(major), host ABI \(NativeTable.major)"))
        }
        guard size >= NativeTable.size else { return .failure(NativeFailure(state: "unavailable", message: "module table of \(size) bytes, host needs \(NativeTable.size)")) }
        func pointer(_ offset: Int) -> UnsafeRawPointer? { table.load(fromByteOffset: offset, as: UnsafeRawPointer?.self) }
        guard let rosterText = pointer(8).map({ String(cString: $0.assumingMemoryBound(to: CChar.self)) }),
              let roster = try? JSONSerialization.jsonObject(with: Data(rosterText.utf8)) as? [String: [String: Any]]
        else { return .failure(NativeFailure(state: "unavailable", message: "\(path): unreadable roster")) }
        guard let create = pointer(16), let view = pointer(24), let set = pointer(32), let destroy = pointer(48),
              let moduleCreate = pointer(72), let moduleDestroy = pointer(80), let moduleLater = pointer(88),
              let moduleCall = pointer(96) else {
            return .failure(NativeFailure(state: "unavailable", message: "\(path): the table lacks a required entry"))
        }
        let loaded = NativeTable(
            path: path, roster: roster, create: unsafeBitCast(create, to: CreateFn.self),
            platformView: unsafeBitCast(view, to: ViewFn.self), setProps: unsafeBitCast(set, to: SetFn.self),
            snapshot: pointer(40).map { unsafeBitCast($0, to: SnapshotFn.self) },
            destroy: unsafeBitCast(destroy, to: DestroyFn.self),
            moduleCreate: unsafeBitCast(moduleCreate, to: ModuleCreateFn.self),
            moduleDestroy: unsafeBitCast(moduleDestroy, to: ModuleDestroyFn.self),
            moduleLater: unsafeBitCast(moduleLater, to: ModuleLaterFn.self),
            moduleCall: unsafeBitCast(moduleCall, to: ModuleLaterFn.self))
        loaded.prepareForReuse = size >= 112 ? pointer(104).map { unsafeBitCast($0, to: ReuseFn.self) } : nil
        if size >= 136 {
            loaded.connect = pointer(112).map { unsafeBitCast($0, to: HookConnectFn.self) }
            loaded.navigationHook = pointer(120).map { unsafeBitCast($0, to: HookNavigationFn.self) }
            loaded.routeHook = pointer(128).map { unsafeBitCast($0, to: HookRouteFn.self) }
        }
        if size >= 160 {
            loaded.tabsHook = pointer(136).map { unsafeBitCast($0, to: HookTabsFn.self) }
            loaded.tabContainerHook = pointer(144).map { unsafeBitCast($0, to: HookTabContainerFn.self) }
            loaded.platformController = pointer(152).map { unsafeBitCast($0, to: ViewFn.self) }
        }
        if size >= 168 { loaded.elementHook = pointer(160).map { unsafeBitCast($0, to: HookElementFn.self) } }
        if size >= 176 { loaded.toolbarHook = pointer(168).map { unsafeBitCast($0, to: HookToolbarFn.self) } }
        return .success(loaded)
    }
}

private final class NativeEntry {
    weak var owner: NodeView?
    let id: UInt32
    var name = ""
    var state = "loading"
    var error: String?
    /// The incarnation token callbacks are delivered under; `instance` is
    /// the nonce the module was created with (they differ after a reuse).
    var nonce: UInt32 = 0
    var instance: UInt32 = 0
    /// A reused view, transparent until the instance's `load`; how many
    /// rows it has served since it was made.
    var revealing = false
    var uses = 0
    /// Taken before its node had a box (`laidOut`).
    var sizing = false
    var handle: UnsafeMutableRawPointer?
    var view: NativePlatformView?
    var props = "{}"
    var snapshotBit = false
    #if os(iOS)
    /// A native screen's controller, a child of its route's while it lives.
    var screen: UIViewController?
    #endif
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
    /// Module nonce → its current incarnation (0 while parked), read on the
    /// calling thread when a callback is issued (LLP 1068 §4.9).
    nonisolated(unsafe) static var live: [UInt32: UInt32] = [:]
    static let lock = NSLock()
    static func incarnation(_ nonce: UInt32) -> UInt32 { lock.lock(); defer { lock.unlock() }; return live[nonce] ?? 0 }
    static func set(_ nonce: UInt32, _ token: UInt32?) { lock.lock(); live[nonce] = token; lock.unlock() }
}
private final class WeakNatives { weak var natives: NativeViews?; init(_ n: NativeViews) { natives = n } }

private let nativeEventCallback: NativeEventFn = { _, instance, kind, bytes, length in
    let data = bytes.map { Data(bytes: $0, count: Int(length)) } ?? Data()
    let nonce = NativeProcess.incarnation(instance)
    // Never synchronously: the host enters the runner through the presenter's gate.
    DispatchQueue.main.async {
        if let natives = NativeProcess.owners[nonce]?.natives { natives.received(nonce: nonce, kind: kind, data: data) }
        else { NativeProcess.retired[nonce]?.natives?.dropped(nonce: nonce, kind: kind) }
    }
}

private let nativeReplyCallback: NativeReplyFn = { _, instance, token, kind, bytes, length in
    let data = bytes.map { Data(bytes: $0, count: Int(length)) } ?? Data()
    let nonce = NativeProcess.incarnation(instance)
    let deliver: () -> Void = { NativeProcess.owners[nonce]?.natives?.replied(token: token, kind: kind, data: data) }
    if Thread.isMainThread { deliver() } else { DispatchQueue.main.async(execute: deliver) }
}

// The session's module reaches its session by runtime handle, as the wake
// does: a stranger's (a destroyed session's) is dropped or refused.
private let nativeLaterCallback: ExactAppLaterFn = { ctx, body, length, reply in
    let rt = ExactRuntime(UInt(bitPattern: ctx))
    let data = body.map { Data(bytes: $0, count: length) } ?? Data()
    DispatchQueue.main.async {
        guard let natives = ExactSession.session(for: rt)?.natives else {
            let bytes = Array("the session ended".utf8)
            return bytes.withUnsafeBufferPointer { exact_app_reply(reply, 503, $0.baseAddress, $0.count) }
        }
        natives.later(data, reply: reply)
    }
}

// A `native.call` arrives on the source's thread: the owner's for main
// placement, a worker's for worker placement. From the owner it goes through
// the one door to main (LLP 1072 T5), served while main waits on the owner;
// from a worker, a synchronous hop, since main never waits on a worker.
private let nativeCallCallback: ExactAppCallFn = { ctx, body, length, slot in
    let rt = ExactRuntime(UInt(bitPattern: ctx))
    let data = body.map { Data(bytes: $0, count: length) } ?? Data()
    let work = {
        guard let natives = ExactSession.session(for: rt)?.natives else {
            let bytes = Array("the session ended".utf8)
            return bytes.withUnsafeBufferPointer { exact_app_answer(slot, 503, $0.baseAddress, $0.count) }
        }
        natives.call(data, slot: slot)
    }
    Owner.shared.callMain(work)
}

private let nativeChangedCallback: NativeTable.ChangedFn = { host, topic, length in
    let rt = ExactRuntime(UInt(bitPattern: host))
    let text = topic.map { Data(bytes: $0, count: Int(length)) } ?? Data()
    DispatchQueue.main.async {
        guard let session = ExactSession.session(for: rt) else { return }
        session.runtime.appChanged(text)
    }
}

private let nativeNowCallback: NativeTable.NowFn = { host in
    ExactSession.session(for: ExactRuntime(UInt(bitPattern: host)))?.now() ?? ExactEnv.wall()
}

final class NativeViews {
    weak var session: ExactSession?
    private var entries: [UInt32: NativeEntry] = [:]
    private var gateOpen = false
    private var waits: [UInt32: NativeWait] = [:]
    private var nextToken: UInt32 = 1
    /// Since launch (`state.pool.native`): instances created, taken from the
    /// pool (and of those, by the row they last showed), parked, and parked
    /// ones destroyed.
    private(set) var made = 0, reused = 0, returned = 0, parks = 0, dropped = 0, hidden = 0, released = 0
    #if os(iOS)
    /// Parked instances by tag, oldest first.
    fileprivate var parked: [String: [NativeEntry]] = [:]
    fileprivate var observers: [NSObjectProtocol] = []
    deinit { for o in observers { NotificationCenter.default.removeObserver(o) } }
    #endif
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
        let started = CFAbsoluteTimeGetCurrent()
        let loaded = NativeTable.load(path: path)
        NativeProcess.table = loaded
        let took = String(format: "%.1f", (CFAbsoluteTimeGetCurrent() - started) * 1000)
        switch loaded {
        case .success(let t): log("loaded \((t.path as NSString).lastPathComponent) in \(took) ms: \(t.roster.keys.sorted().joined(separator: ", "))")
        case .failure(let f): log("\(f.state): \(f.message)")
        }
        return loaded
    }

    // MARK: The session's module (LLP 1067.000 Q5–Q7)

    private(set) var instance: UnsafeMutableRawPointer?
    private var instanceFailure: NativeFailure?
    /// Whether the module's hooks are connected, and who replays them for
    /// the objects built before (LLP 1075.003 §3.2; NativeHooks.swift).
    var hooksConnected = false
    var onHooksConnected: (() -> Void)?
    var hookCalls: (navigation: HookNavigationFn, route: HookRouteFn)?
    var tabCalls: (HookTabsFn, HookTabContainerFn)?
    var elementCall: HookElementFn?
    var toolbarCall: HookToolbarFn?

    /// The session's one module instance, made at the first view or long
    /// call that needs it. Main thread.
    private func module(_ table: NativeTable) -> Result<UnsafeMutableRawPointer, NativeFailure> {
        if let instance { return .success(instance) }
        if let instanceFailure { return .failure(instanceFailure) }
        guard let session else { return .failure(NativeFailure(state: "unavailable", message: "the session ended")) }
        let roots = NativeViews.roots(session: session)
        let context: [String: Any] = ["agent": ExactEnv.agentMode, "data": roots.data, "cache": roots.cache, "temporary": roots.temporary]
        let json = (try? JSONSerialization.data(withJSONObject: context)) ?? Data()
        var error = [UInt8](repeating: 0, count: 512)
        let host = UnsafeMutableRawPointer(bitPattern: UInt(session.runtime.rt))
        let made = json.withUnsafeBytes { j in
            table.moduleCreate(j.bindMemory(to: UInt8.self).baseAddress, UInt32(json.count), host, nativeChangedCallback, nativeNowCallback, &error, UInt32(error.count))
        }
        guard let made else {
            let failure = NativeFailure(state: "error", message: "module create refused: \(String(cString: error.map { CChar(bitPattern: $0) }))")
            instanceFailure = failure
            log(failure.message)
            return .failure(failure)
        }
        instance = made
        log("module instance made (agent \(ExactEnv.agentMode))")
        if let connect = table.connect, let navigation = table.navigationHook, let route = table.routeHook {
            let tabs = table.tabsHook.flatMap { tabs in table.tabContainerHook.map { (tabs, $0) } }
            connectHooks(connect, navigation, route, tabs, table.elementHook, table.toolbarHook, made)
        }
        return .success(made)
    }

    /// Where the module keeps its files: the app's roots, as the runtime
    /// configures storage; under the agent, a scratch tree of this process.
    private static func roots(session: ExactSession) -> (data: String, cache: String, temporary: String) {
        let id = Bundle.main.bundleIdentifier ?? "app"
        if ExactEnv.agentMode {
            let base = (NSTemporaryDirectory() as NSString).appendingPathComponent("exact-agent-\(getpid())-\(session.runtime.rt)")
            return (base + "/data", base + "/cache", base + "/temporary")
        }
        let home = NSHomeDirectory()
        let cache = home + "/Library/Caches/exact/" + id
        return (home + "/Library/Application Support/exact/" + id + "/data", cache + "/cache", cache + "/temporary")
    }

    /// Tell the runtime this session has an app module, when the app ships
    /// an artifact: `native` is then available to its sources, and their long
    /// calls come here. Nothing loads until one arrives or a view needs it.
    func installAppModule() {
        guard let session else { return }
        let path = NativeViews.modulePath(session: session)
        guard FileManager.default.fileExists(atPath: path) else { return }
        hasAppModule = true
        let rt = session.runtime.rt
        session.runtime.on { exact_set_app_module(rt, nativeLaterCallback, nativeCallCallback, UnsafeMutableRawPointer(bitPattern: UInt(rt))) }
    }

    private var hasAppModule = false

    /// After first pixel, before data activates: load the artifact and make
    /// the session's instance, so the first `native.call` pays for neither.
    /// A first load of a freshly built dylib took 164 ms inside a call's
    /// 100 ms budget (the sample host, 2026-09-27).
    func prepareAppModule() {
        guard hasAppModule, instance == nil, case .success(let table) = self.table() else { return }
        _ = module(table)
    }

    /// A long call, on the main thread: the artifact loads, the instance is
    /// made if need be, and the module takes the call (Q5).
    fileprivate func later(_ body: Data, reply: UnsafeMutableRawPointer?) {
        let refuse = { (message: String) in
            let bytes = Array(message.utf8)
            bytes.withUnsafeBufferPointer { exact_app_reply(reply, 503, $0.baseAddress, $0.count) }
        }
        let table: NativeTable
        switch self.table() {
        case .failure(let f): return refuse(f.message)
        case .success(let t): table = t
        }
        switch module(table) {
        case .failure(let f): refuse(f.message)
        case .success(let m):
            body.withUnsafeBytes { b in
                table.moduleLater(m, b.bindMemory(to: UInt8.self).baseAddress, body.count, reply, { reply, status, bytes, length in exact_app_reply(reply, status, bytes, length) })
            }
        }
    }

    /// A `native.call`, on the main thread: answered before this returns.
    fileprivate func call(_ body: Data, slot: UnsafeMutableRawPointer?) {
        let refuse = { (message: String) in
            let bytes = Array(message.utf8)
            bytes.withUnsafeBufferPointer { exact_app_answer(slot, 503, $0.baseAddress, $0.count) }
        }
        let table: NativeTable
        switch self.table() {
        case .failure(let f): return refuse(f.message)
        case .success(let t): table = t
        }
        switch module(table) {
        case .failure(let f): refuse(f.message)
        case .success(let m):
            body.withUnsafeBytes { b in
                table.moduleCall(m, b.bindMemory(to: UInt8.self).baseAddress, body.count, slot, { slot, status, bytes, length in exact_app_answer(slot, status, bytes, length) })
            }
        }
    }

    /// Session teardown, after the views and the runtime: the module goes last.
    func destroyModule() {
        #if os(iOS)
        drainParked()
        #endif
        guard let instance, case .success(let table)? = NativeProcess.table else { return }
        self.instance = nil
        hooksConnected = false
        table.moduleDestroy(instance)
        log("module instance destroyed")
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
        let module: UnsafeMutableRawPointer
        switch self.module(table) {
        case .failure(let f): return fail(entry, f.state, f.message)
        case .success(let m): module = m
        }
        entry.snapshotBit = caps["snapshot"] as? Bool == true && table.snapshot != nil
        let started = CFAbsoluteTimeGetCurrent()
        #if os(iOS)
        if reuse(entry, table: table, owner: owner) { return }
        #endif
        let nonce = NativeProcess.next
        NativeProcess.next &+= 1
        NativeProcess.owners[nonce] = WeakNatives(self)
        NativeProcess.set(nonce, nonce)
        entry.nonce = nonce
        entry.instance = nonce
        let props = owner.props["nativeViewProps"] ?? "{}"
        var error = [UInt8](repeating: 0, count: 512)
        let tag = Data(entry.name.utf8), json = Data(props.utf8)
        let handle = tag.withUnsafeBytes { t in json.withUnsafeBytes { p in
            table.create(module, t.bindMemory(to: UInt8.self).baseAddress, UInt32(tag.count), p.bindMemory(to: UInt8.self).baseAddress, UInt32(json.count),
                         nativeEventCallback, nativeReplyCallback, nil, nonce, &error, UInt32(error.count))
        } }
        guard let handle else {
            NativeProcess.owners.removeValue(forKey: nonce)
            NativeProcess.set(nonce, nil)
            entry.nonce = 0
            return fail(entry, "error", "create refused: \(String(cString: error.map { CChar(bitPattern: $0) }))")
        }
        guard let raw = table.platformView(handle) else {
            table.destroy(handle)
            NativeProcess.owners.removeValue(forKey: nonce)
            NativeProcess.set(nonce, nil)
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
        entry.handle = handle
        entry.view = view
        #if os(iOS)
        contain(entry, table: table) { owner.addSubview(view) }
        #else
        owner.addSubview(view)
        #endif
        entry.props = props
        entry.state = "ready"
        entry.error = nil
        made += 1
        measured?("native", CFAbsoluteTimeGetCurrent() - started)
        log("\(entry.name) #\(entry.id): ready")
    }

    /// A host that holds a costly view mid-fling (LLP 1068 §5.1): `holds`
    /// answers whether `owner`'s instance waits, `measured` hears each
    /// creation's cost, and `release` makes a held instance now, from the
    /// owner's latest props.
    #if os(macOS)
    func canReuse(_ name: String) -> Bool { false }
    #endif
    /// Tests: the process's table from memory, and this session's module
    /// instance, without an artifact or a runtime.
    static func install(table: UnsafeRawPointer) { NativeProcess.table = NativeTable.read(table, path: "test") }
    static func uninstallTable() { NativeProcess.table = nil }
    func install(module: UnsafeMutableRawPointer) { instance = module; gateOpen = true }
    /// Tests: the process's artifact from a file and this session's module
    /// made from it now, as the paint gate makes it (its hooks connect).
    func installArtifact(_ path: String) {
        NativeProcess.table = NativeTable.load(path: path)
        hasAppModule = true
        prepareAppModule()
    }
    var holds: ((NodeView) -> Bool)?
    var measured: ((String, TimeInterval) -> Void)?
    func release(_ owner: NodeView) {
        guard gateOpen, let entry = entries[owner.id], entry.owner === owner, entry.handle == nil,
              entry.state == "loading", !entry.name.isEmpty else { return }
        attach(entry)
    }

    /// Far module views, as UIKit's collection view treats the cells it
    /// keeps and recycles (the SwiftUI baseline's map rows; LLP 1068 §5.2;
    /// both hosts, the macOS hold passing `release: .infinity`):
    /// `distance` says how far a node's box is from what shows, in
    /// viewports (0 inside, nil when not in a list's row). A made view past
    /// `hide` is hidden and shown again inside it; past `release` its
    /// instance goes (parked for reuse or destroyed, as a destroyed node's)
    /// and the node waits to be made again from its latest props when it
    /// comes near. Returns the nodes whose instance went.
    func recycleFar(hide: CGFloat, release: CGFloat, _ distance: (NodeView) -> CGFloat?) -> [NodeView] {
        var gone: [NodeView] = []
        for entry in Array(entries.values) {
            guard let view = entry.view, let owner = entry.owner, let d = distance(owner) else { continue }
            let away = d > hide
            if view.isHidden != away { view.isHidden = away; hidden += away ? 1 : 0 }
            guard d > release, entry.state == "ready", entry.handle != nil else { continue }
            let name = entry.name
            destroy(id: entry.id)
            let fresh = NativeEntry(owner: owner)
            fresh.name = name
            entries[owner.id] = fresh
            released += 1
            gone.append(owner)
        }
        return gone
    }


    /// A props commit: the first names the module (the create commit carries
    /// no props yet); later ones replace the whole aggregate.
    func update(_ owner: NodeView) {
        guard let entry = entries[owner.id] else { return }
        if entry.name.isEmpty, entry.state == "loading" {
            entry.name = owner.props["nativeViewModuleName"] ?? ""
            log("\(entry.name) #\(owner.id): loading")
            if gateOpen, canReuse(entry.name) || holds?(owner) != true { attach(entry) }
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
        #if os(iOS)
        if park(entry) { return }
        #endif
        if entry.instance != 0 { NativeProcess.set(entry.instance, nil) }
        #if os(iOS)
        release(entry)
        #endif
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
        #if os(iOS)
        if kind == 7, entry.revealing { entry.revealing = false; entry.view?.alpha = 1 }
        #endif
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

    #if canImport(UIKit)
    /// Before a capture: `drawHierarchy` reuses a clean backing layer without
    /// calling `draw`, and a module view that draws from the session's clock
    /// (the agent's) may not have drawn since the clock moved.
    func redrawForCapture() {
        for view in entries.values.compactMap(\.view) { view.setNeedsDisplay(); view.layer.displayIfNeeded() }
    }
    #endif

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

#if os(iOS)
extension NativeViews {
    /// A native screen (LLP 1075.003 §3.6): its controller becomes a child
    /// of the controller its node shows in (a route's), so UIKit gives it
    /// appearance calls, the safe area and traits, in UIKit's order: added
    /// as a child, its view `insert`ed, then moved in. A node created in the
    /// batch that mounts its route is in no controller yet: it is contained
    /// once that batch is applied.
    fileprivate func contain(_ entry: NativeEntry, table: NativeTable, retry: Bool = true, insert: () -> Void = {}) {
        guard let handle = entry.handle, let made = table.platformController?(handle), let owner = entry.owner else { return insert() }
        let screen = Unmanaged<UIViewController>.fromOpaque(made).takeUnretainedValue()
        var responder: UIResponder? = owner
        while let next = responder, !(next is UIViewController) { responder = next.next }
        guard let parent = responder as? UIViewController else {
            insert()
            guard retry, let presenter = owner.presenter else { return log("\(entry.name) #\(entry.id): a screen with no controller to hold it") }
            presenter.afterBatch { [weak self, weak entry] in
                guard let self, let entry, self.entries[entry.id] === entry, entry.screen == nil,
                      case .success(let table)? = NativeProcess.table else { return }
                self.contain(entry, table: table, retry: false)
            }
            return
        }
        if screen.parent != nil, screen.parent !== parent {
            screen.willMove(toParent: nil)
            screen.removeFromParent()
        }
        let adds = screen.parent !== parent
        if adds { parent.addChild(screen) }
        insert()
        if adds { screen.didMove(toParent: parent) }
        entry.screen = screen
        if adds { log("\(entry.name) #\(entry.id): a screen in \(type(of: parent))") }
    }

    /// A screen's view leaves with its controller, in UIKit's order; a plain
    /// module view just leaves.
    fileprivate func release(_ entry: NativeEntry) {
        guard let screen = entry.screen else { return }
        screen.willMove(toParent: nil)
        entry.view?.removeFromSuperview()
        screen.removeFromParent()
        entry.screen = nil
    }

    /// Parked instances per tag, and the rows one instance serves before it
    /// is destroyed (LLP 1068 §6, measured on the iPad, §0.3): an instance
    /// keeps what it drew for every region it showed — a map served without
    /// a limit took the Extra Heavy fling's footprint from 257 MB to 502 —
    /// and four reuses kept the end footprint at a fresh map's while making
    /// a fifth as many. With far instances released (§5.2.1) two reuses keep
    /// the map feed's peak at the SwiftUI baseline's (889 against 886 MB on
    /// the iPad; four, 917) at a sixth more maps made.
    static let reuseCap = 2, reuseLimit = 2

    /// Whether a parked instance of `name` waits: taking one costs about a
    /// tenth of a creation, so it is never held mid-fling (LLP 1068 §5.1).
    func canReuse(_ name: String) -> Bool { !(parked[name]?.isEmpty ?? true) }

    /// A destroyed entry's instance parks, if its tag is reused, it was
    /// ready and in a window, and the module reset it. Its incarnation has
    /// ended before the reset runs.
    fileprivate func park(_ entry: NativeEntry) -> Bool {
        guard entry.uses < Self.reuseLimit, entry.state == "ready", let handle = entry.handle, let view = entry.view, view.window != nil,
              case .success(let table)? = NativeProcess.table, let prepare = table.prepareForReuse,
              table.roster[entry.name]?["reuse"] as? Bool == true else { return false }
        NativeProcess.set(entry.instance, 0)
        guard prepare(handle) == 0 else {
            log("\(entry.name) #\(entry.id): reuse refused")
            return false
        }
        var list = parked[entry.name] ?? []
        if list.count >= Self.reuseCap { discard(list.removeFirst()) }
        view.alpha = 1
        view.isHidden = false
        release(entry)
        view.removeFromSuperview()
        entry.nonce = 0; entry.revealing = false; entry.sizing = false; entry.owner = nil
        view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        list.append(entry)
        parked[entry.name] = list
        parks += 1
        observe()
        return true
    }

    /// `entry` takes a parked instance of its tag: a new incarnation, the
    /// props as a first mount, and the view transparent until `load`.
    fileprivate func reuse(_ entry: NativeEntry, table: NativeTable, owner: NodeView) -> Bool {
        guard var list = parked[entry.name], !list.isEmpty else { return false }
        let props = owner.props["nativeViewProps"] ?? "{}"
        // A row that comes back takes the instance that last showed it, as a
        // collection view's cell for the same item keeps its content: the
        // instance draws nothing new, so it is not a use (the limit counts
        // the rows an instance keeps drawings of).
        let same = list.lastIndex { $0.props == props }
        let from = list.remove(at: same ?? list.count - 1)
        parked[entry.name] = list.isEmpty ? nil : list
        guard let handle = from.handle, let view = from.view else { return false }
        let token = NativeProcess.next
        NativeProcess.next &+= 1
        NativeProcess.owners[token] = WeakNatives(self)
        entry.nonce = token
        entry.instance = from.instance
        entry.handle = handle
        entry.view = view
        entry.snapshotBit = from.snapshotBit
        entry.uses = from.uses + (same == nil ? 1 : 0)
        NativeProcess.set(from.instance, token)
        var error = [UInt8](repeating: 0, count: 512)
        let json = Data(props.utf8)
        view.alpha = 0
        entry.revealing = true
        // A new node has no box yet: the view keeps its size, which is most
        // often the next row's, until the node is laid out (`laidOut`) — a
        // map's resize to nothing and back costs as much as its reset.
        if owner.bounds.isEmpty { view.autoresizingMask = []; entry.sizing = true } else { view.frame = owner.bounds }
        contain(entry, table: table) { owner.addSubview(view) }
        let status = json.withUnsafeBytes { p in table.setProps(handle, p.bindMemory(to: UInt8.self).baseAddress, UInt32(json.count), &error, UInt32(error.count)) }
        entry.props = props
        entry.state = "ready"
        entry.error = nil
        reused += 1
        if same != nil { returned += 1 }
        if status != 0 { fail(entry, "error", "props refused: \(String(cString: error.map { CChar(bitPattern: $0) }))") }
        log("\(entry.name) #\(entry.id): ready (reused)")
        return true
    }

    /// `owner` was laid out: a taken view that kept its size takes the box.
    func laidOut(_ owner: NodeView) {
        guard let entry = entries[owner.id], entry.sizing, let view = entry.view, !owner.bounds.isEmpty else { return }
        entry.sizing = false
        view.frame = owner.bounds
        view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
    }

    /// A parked instance is destroyed: past the cap, on memory pressure, in
    /// the background.
    private func discard(_ entry: NativeEntry) {
        NativeProcess.set(entry.instance, nil)
        entry.view?.removeFromSuperview()
        if let handle = entry.handle, case .success(let table)? = NativeProcess.table { table.destroy(handle) }
        entry.handle = nil
        dropped += 1
    }
    func drainParked() {
        for entry in parked.values.joined() { discard(entry) }
        parked.removeAll()
    }
    private func observe() {
        guard observers.isEmpty else { return }
        for name in [UIApplication.didReceiveMemoryWarningNotification, UIApplication.didEnterBackgroundNotification] {
            observers.append(NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in self?.drainParked() })
        }
    }
    /// `state.pool.native`.
    var observation: [String: Any] {
        ["made": made, "reused": reused, "returned": returned, "parks": parks, "dropped": dropped, "hidden": hidden, "released": released,
         "parked": parked.mapValues(\.count), "cap": Self.reuseCap, "limit": Self.reuseLimit]
    }
}
#endif
