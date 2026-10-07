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
//   0  u32 major            3
//   4  u32 size             104 or more
//   8  const char *roster   JSON: {"tag": {"snapshot": bool}, …}
//  16  create(module, tag, tagLen, props, propsLen, event, reply, ctx, nonce, err, errCap) → handle
//  24  platform_view(handle) → NSView * / UIView *   (the module keeps ownership)
//  32  set_props(handle, json, len, err, errCap) → 0 accepted, else refused
//  40  snapshot(handle, token)          nullable; answered on `reply`
//  48  destroy(handle)
//  56  set_bounds                       reserved, NULL (LLP 1024 §5)
//  64  agent_input(handle, json, len, err, errCap) → 0 delivered, else refused
//        nullable; JSON {text: string} or {key: chord, phase?: down|up}
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
// 112  focus_target(handle) → borrowed NSView * / UIView *; size ≥ 120; nullable
//        the platform view or an attached descendant; nil refuses focus
// 120  module_connect(module, host_table)       size ≥ 144; the hatches (LLP
//        1075.003 §3.2), NativeHatches.swift: the host's callbacks, once
// 128  module_navigation(module, event, controller, flags) → flags
//        event 0 built (the hatch runs), 1 retired; bit 0 showsBar
// 136  module_route(module, event, controller, navigation, scroll, json, len)
//        event 0 built, 1 changed, 2 ended; json {"key", "data": {…}}
// 144  module_tabs(module, event, controller, index)      size ≥ 168
//        event 0 built, 1 retired, 2 the router selected `index` in the
//        app's container, 3 the app's container retired
// 152  module_tab_container(module, json, len, controllers, count) → container
//        json {"names", "nodes", "selected"}; retained once, or nil (Exact's)
// 160  platform_controller(handle) → UIViewController *, a native screen's
// 168  module_element(module, event, view, platform, json, len) → flags
//        size ≥ 176 (LLP 1075.003.000): a node marked `hatch="word"`; event
//        0 built, 1 changed, 2 ended; json {"hatch", "node", "id", "kind",
//        "data"}; flags bit 0: the hatch made it reusable
// 176  module_toolbar(module, toolbar, window)        size ≥ 184; macOS:
//        the window toolbar Exact installed (LLP 1075.003.000 §3.7)
//
//   event(ctx, nonce, kind, bytes, len)          kind: EventKind 0–8 — press,
//     change, hover, focus, blur, key, submit, load, message; change, key and
//     message carry UTF-8, hover "true"/"false"; from any thread.
//   event kind 9 is host-only intrinsic content size: UTF-8 "width,height"
//     in points, or empty to clear. Never dispatched as a Contract event.
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
    static let major: UInt32 = 3
    /// The hatch words the module handles, or nil for one that does not say.
    var hatchWords: Set<String>?
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
    /// The hatches (LLP 1075.003 §3.2), in a table of 144 bytes or more.
    var connect: HatchConnectFn?, navigationHatch: HatchNavigationFn?, routeHatch: HatchRouteFn?
    /// Tabs and native screens, in a table of 168 bytes or more.
    var tabsHatch: HatchTabsFn?, tabContainerHatch: HatchTabContainerFn?, platformController: ViewFn?
    /// Hatched nodes (LLP 1075.003.000), in a table of 176 bytes or more;
    /// the window toolbar's hatch (macOS), in one of 184 or more.
    var elementHatch: HatchElementFn?, toolbarHatch: HatchToolbarFn?
    var agentInput: SetFn?
    var focusTarget: ViewFn?

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
              var roster = try? JSONSerialization.jsonObject(with: Data(rosterText.utf8)) as? [String: [String: Any]]
        else { return .failure(NativeFailure(state: "unavailable", message: "\(path): unreadable roster")) }
        // The hatch words the module was built to handle, under a key no tag
        // can take (LLP 1075.003.000.001 §4.3); an older module names none.
        let hatchWords = (roster.removeValue(forKey: "hatches")?["words"] as? [String]).map(Set.init)
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
        loaded.hatchWords = hatchWords
        loaded.agentInput = pointer(64).map { unsafeBitCast($0, to: SetFn.self) }
        loaded.focusTarget = size >= 120 ? pointer(112).map { unsafeBitCast($0, to: ViewFn.self) } : nil
        loaded.prepareForReuse = size >= 112 ? pointer(104).map { unsafeBitCast($0, to: ReuseFn.self) } : nil
        if size >= 144 {
            loaded.connect = pointer(120).map { unsafeBitCast($0, to: HatchConnectFn.self) }
            loaded.navigationHatch = pointer(128).map { unsafeBitCast($0, to: HatchNavigationFn.self) }
            loaded.routeHatch = pointer(136).map { unsafeBitCast($0, to: HatchRouteFn.self) }
        }
        if size >= 168 {
            loaded.tabsHatch = pointer(144).map { unsafeBitCast($0, to: HatchTabsFn.self) }
            loaded.tabContainerHatch = pointer(152).map { unsafeBitCast($0, to: HatchTabContainerFn.self) }
            loaded.platformController = pointer(160).map { unsafeBitCast($0, to: ViewFn.self) }
        }
        if size >= 176 { loaded.elementHatch = pointer(168).map { unsafeBitCast($0, to: HatchElementFn.self) } }
        if size >= 184 { loaded.toolbarHatch = pointer(176).map { unsafeBitCast($0, to: HatchToolbarFn.self) } }
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
    #if os(iOS) || os(tvOS)
    /// A native screen's controller, a child of its route's while it lives.
    var screen: UIViewController?
    #endif
    var intrinsicSize: CGSize?
    var hasIntrinsicReport = false
    #if os(iOS) || os(tvOS)
    var worldLayout = NativeWorldLayout()
    #endif
    init(owner: NodeView) { self.owner = owner; self.id = owner.id }
    var status: [String: Any] {
        var s: [String: Any] = ["name": name, "state": state]
        if let error { s["error"] = error }
        return s
    }
}

#if os(iOS) || os(tvOS)
/// A fixed-size native child need not get UIKit layout when only an ancestor
/// moves. Re-arm it after a geometry batch, once per changed window geometry.
struct NativeWorldLayout {
    private struct Geometry: Equatable {
        let frame: CGRect
        let scale: CGFloat
        let window: ObjectIdentifier
    }
    private var previous: Geometry?

    mutating func refresh(_ view: UIView) {
        guard let window = view.window else { previous = nil; return }
        let next = Geometry(frame: view.convert(view.bounds, to: window),
                            scale: window.screen.scale, window: ObjectIdentifier(window))
        guard next != previous else { return }
        previous = next
        view.setNeedsLayout()
    }
}
#endif

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
        else {
            // A callback issued after retirement captured 0. The module's
            // original nonce still identifies its weak diagnostic owner;
            // use it only for logging, never to deliver to a reused view.
            let retiredNonce = nonce == 0 ? instance : nonce
            NativeProcess.retired[retiredNonce]?.natives?.dropped(nonce: retiredNonce, kind: kind)
        }
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
    weak var session: ExactSession? { didSet { hatchDiagnostics.attach(session) } }
    /// What Exact times of each hatch call and what hatch code records (LLP 1075.003.000.001 §3.1–3.2).
    let hatchDiagnostics = HatchDiagnostics()
    /// This session's slot in the crash breadcrumb (§4.4), and the count of
    /// times its hatches have connected: a record's incarnation.
    var crumbSlot: Int?, hatchIncarnation: UInt32 = 0
    /// `EXACT_HATCHES=off` (a development build only, §2.6): the module loads
    /// and its views and calls work; no hatch is connected or called.
    static let hatchesOff = !ExactEnv.productionBake && ExactEnv.environment["EXACT_HATCHES"] == "off"
    private var entries: [UInt32: NativeEntry] = [:]
    private var gateOpen = false
    private var waits: [UInt32: NativeWait] = [:]
    private var nextToken: UInt32 = 1
    /// Since launch (`state.pool.native`): instances created, taken from the
    /// pool (and of those, by the row they last showed), parked, and parked
    /// ones destroyed.
    private(set) var made = 0, reused = 0, returned = 0, parks = 0, dropped = 0, hidden = 0, released = 0
    #if os(iOS) || os(tvOS)
    /// Parked instances by tag, oldest first.
    fileprivate var parked: [String: [NativeEntry]] = [:]
    fileprivate var observers: [NSObjectProtocol] = []
    deinit {
        for o in observers { NotificationCenter.default.removeObserver(o) }
        if let crumbSlot { HatchBreadcrumb.shared?.free(crumbSlot) }
    }
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
        let standard = embeddedModule(framework: "ExactModules", dylib: "libexact_modules.dylib")
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

    /// Opens the paint gate once activation has run, then on every batch.
    /// The first call that finds a view loads the artifact.
    func loadIfNeeded() {
        guard activationRan, !gateOpen, !entries.isEmpty else { return }
        gateOpen = true
        for entry in entries.values.sorted(by: { $0.id < $1.id }) where entry.state == "loading" && !entry.name.isEmpty { attach(entry) }
    }

    /// Views wait for activation's frame to commit, so a slow view (a map
    /// takes tens of ms) does not delay that frame or pending input.
    private var activationRan = false
    func activated() {
        activationRan = true
        loadIfNeeded()
    }

    /// True from activation until `activated` runs (or a stale session's turn
    /// passes): the agent's settle counts it as work in flight.
    var activationQueued: Bool { queuedActivations > 0 }
    private var queuedActivations = 0
    /// Calls `activated` after activation's Core Animation commit, if `live()`
    /// still holds. A plain main-queue hop can run before that commit: the run
    /// loop drains queued blocks before its before-waiting and exit observers,
    /// where Core Animation commits (order 2000000). Exit covers a turn that
    /// never waits.
    func activateAfterCommit(_ live: @escaping () -> Bool) {
        queuedActivations += 1
        let observer = CFRunLoopObserverCreateWithHandler(nil, CFRunLoopActivity.beforeWaiting.rawValue | CFRunLoopActivity.exit.rawValue, false, 2_000_001) { [weak self] _, _ in
            DispatchQueue.main.async { [weak self] in
                guard let self else { return }
                if live() { activated() }
                // A view's events are main-queue blocks its creation queued (`load`):
                // the count drops after them, so the agent's settle sees their handlers.
                DispatchQueue.main.async { [weak self] in self?.queuedActivations -= 1 }
            }
        }
        CFRunLoopAddObserver(CFRunLoopGetMain(), observer, .commonModes)
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
    /// Whether the module's hatches are connected, and who replays them for
    /// the objects built before (LLP 1075.003 §3.2; NativeHatches.swift).
    var hatchesConnected = false
    /// The hatch words the loaded module was built to handle (LLP
    /// 1075.003.000.001 §4.3), or nil when it does not say: then every word is called.
    var handledHatches: Set<String>? { if case .success(let t) = NativeProcess.table { t.hatchWords } else { nil } }
    var onHatchesConnected: (() -> Void)?
    var hatchCalls: (navigation: HatchNavigationFn, route: HatchRouteFn)?
    var tabCalls: (HatchTabsFn, HatchTabContainerFn)?
    var elementCall: HatchElementFn?
    var toolbarCall: HatchToolbarFn?

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
        if Self.hatchesOff { log("hatches: off (EXACT_HATCHES=off); no hatch is connected or called") }
        if !Self.hatchesOff, let connect = table.connect, let navigation = table.navigationHatch, let route = table.routeHatch {
            let tabs = table.tabsHatch.flatMap { tabs in table.tabContainerHatch.map { (tabs, $0) } }
            connectHatches(connect, navigation, route, tabs, table.elementHatch, table.toolbarHatch, made)
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
        #if os(iOS) || os(tvOS)
        drainParked()
        #endif
        guard let instance, case .success(let table)? = NativeProcess.table else { return }
        self.instance = nil
        hatchesConnected = false
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
        #if os(iOS) || os(tvOS)
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
        view.frame = owner.contentBox()
        #if os(macOS)
        view.autoresizingMask = [.width, .height]
        #else
        view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        #endif
        entry.handle = handle
        entry.view = view
        #if os(iOS) || os(tvOS)
        contain(entry, table: table) { owner.addSubview(view) }
        #else
        owner.addSubview(view)
        #endif
        entry.props = props
        entry.state = "ready"
        entry.error = nil
        #if os(macOS)
        owner.presenter?.keyViewLoopStale = true
        #endif
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
    func install(module: UnsafeMutableRawPointer, gateOpen open: Bool = true) { instance = module; gateOpen = open }
    /// Tests: the process's artifact from a file and this session's module
    /// made from it now, as the paint gate makes it (its hatches connect).
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
        #if os(macOS)
        owner.presenter?.keyViewLoopStale = true
        #endif
        if status == 0 {
            entry.props = props
            if entry.state == "error" { entry.state = "ready"; entry.error = nil }
        } else {
            fail(entry, "error", "props refused: \(String(cString: error.map { CChar(bitPattern: $0) }))")
        }
    }

    #if os(macOS)
    /// Include field editors, which AppKit keeps outside the widget subtree.
    func ownsFocus(_ owner: NodeView) -> Bool {
        guard let entry = entries[owner.id], entry.owner === owner else { return false }
        return ownsFocus(entry)
    }

    private func ownsFocus(_ entry: NativeEntry) -> Bool {
        guard let root = entry.view else { return false }
        return ownsFocus(in: root)
    }

    private func ownsFocus(in root: NSView) -> Bool {
        guard let responder = root.window?.firstResponder else { return false }
        if let view = responder as? NSView, view === root || view.isDescendant(of: root) { return true }
        func editing(_ view: NSView) -> Bool {
            if let field = view as? NSTextField, field.currentEditor() === responder { return true }
            return view.subviews.contains(where: editing)
        }
        return editing(root)
    }

    private func available(_ owner: NodeView) -> NativeEntry? {
        guard let entry = entries[owner.id], entry.owner === owner, entry.state == "ready",
              entry.handle != nil, let root = entry.view, root.superview === owner,
              root.window != nil, !owner.disabled, !owner.inert,
              owner.bounds.width > 0, owner.bounds.height > 0 else { return nil }
        var next: NSView? = root
        while let view = next {
            if view.isHidden { return nil }
            if (view as? NodeView)?.style["display"]?.string == "none" { return nil }
            next = view.superview
        }
        return entry
    }

    /// One responder for explicit focus, sequential focus and dialog entry.
    func focusTarget(_ owner: NodeView) -> NSView? {
        guard let entry = available(owner), let handle = entry.handle, let root = entry.view,
              let window = root.window, case .success(let table)? = NativeProcess.table,
              let raw = table.focusTarget?(handle) else { return nil }
        let target = Unmanaged<NSView>.fromOpaque(raw).takeUnretainedValue()
        guard target === root || target.isDescendant(of: root), target.window === window,
              !target.isHiddenOrHasHiddenAncestor, target.acceptsFirstResponder,
              (target as? NSControl)?.isEnabled != false else { return nil }
        return target
    }

    @discardableResult func focus(_ owner: NodeView) -> Bool {
        guard let target = focusTarget(owner), let window = target.window else { return false }
        if ownsFocus(in: target) { return true }
        return window.makeFirstResponder(target) && ownsFocus(in: target)
    }

    func inputToken(_ owner: NodeView) -> UInt32? {
        guard let entry = available(owner) else { return nil }
        return entry.nonce
    }

    func input(_ owner: NodeView, request: [String: Any], token: UInt32? = nil) -> [String: Any] {
        guard let entry = available(owner), token == nil || token == entry.nonce,
              let handle = entry.handle, case .success(let table)? = NativeProcess.table else {
            return ["error": "native view is unavailable, hidden, inert, disabled or replaced"]
        }
        guard let input = table.agentInput else { return ["error": "native view does not support agent input"] }
        let nonce = entry.nonce
        var payload: [String: String] = [:]
        if let key = request["key"] as? String {
            payload["key"] = key
            if let phase = request["phase"] as? String {
                guard phase == "down" || phase == "up" else { return ["error": "invalid key phase"] }
                payload["phase"] = phase
            }
        } else {
            guard request["phase"] == nil, let text = request["text"] as? String else { return ["error": "expected text or key"] }
            guard owner.props["editable"] != "false" else { return ["error": "native view is readonly"] }
            payload["text"] = text
        }
        // A held release must never steal focus back from a different widget.
        if token != nil {
            guard ownsFocus(owner) else { return ["error": "native view no longer owns focus"] }
        } else {
            guard focus(owner) else { return ["error": "native view refused focus"] }
        }
        // AppKit resigns the previous responder synchronously. Its blur handler
        // can render new restrictions or replace this instance before we return.
        guard available(owner) === entry, entry.nonce == nonce, entry.handle == handle else {
            return ["error": "native view is unavailable, hidden, inert, disabled or replaced"]
        }
        guard ownsFocus(owner) else { return ["error": "native view no longer owns focus"] }
        if payload["text"] != nil, owner.props["editable"] == "false" {
            return ["error": "native view is readonly"]
        }
        let bytes = Array((try! JSONSerialization.data(withJSONObject: payload)))
        var error = [UInt8](repeating: 0, count: 512)
        let status = bytes.withUnsafeBufferPointer { input(handle, $0.baseAddress, UInt32($0.count), &error, UInt32(error.count)) }
        guard status == 0 else {
            let message = String(decoding: error.prefix { $0 != 0 }, as: UTF8.self)
            return ["error": message.isEmpty ? "native view refused agent input" : message]
        }
        var reply: [String: Any] = ["typed": Int(owner.id), "delivery": "native-module"]
        if let key = payload["key"] { reply["key"] = key }
        if let phase = payload["phase"] { reply["phase"] = phase }
        return reply
    }
    #endif

    /// The node is gone: the nonce dies first, then the instance (D4).
    func destroy(id: UInt32) {
        guard let entry = entries.removeValue(forKey: id) else { return }
        if entry.nonce != 0 {
            NativeProcess.owners.removeValue(forKey: entry.nonce)
            NativeProcess.retired[entry.nonce] = WeakNatives(self)
        }
        #if os(iOS) || os(tvOS)
        if park(entry) { return }
        #endif
        if entry.instance != 0 { NativeProcess.set(entry.instance, nil) }
        #if os(iOS) || os(tvOS)
        release(entry)
        #else
        // Retire a descendant/field editor before destroying its module instance.
        entry.owner?.presenter?.keyViewLoopStale = true
        if ownsFocus(entry) { entry.view?.window?.makeFirstResponder(nil) }
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

    // Keep the incarnation through both asynchronous boundaries: the callback
    // hop and the session's in-flight collection fill. One turn, one layout.
    private var intrinsicSizes: [UInt32: CGSize?] = [:]
    private var intrinsicFlushPending = false
    private func intrinsic(_ entry: NativeEntry, data: Data) {
        let size: CGSize?
        if data.isEmpty { size = nil } else {
            let parts = String(decoding: data, as: UTF8.self).split(separator: ",", omittingEmptySubsequences: false)
            guard parts.count == 2, let w = Float(parts[0]), let h = Float(parts[1]),
                  w.isFinite, h.isFinite, w > 0, h > 0 else {
                return log("\(entry.name) #\(entry.id): refused intrinsic size")
            }
            size = CGSize(width: CGFloat(w), height: CGFloat(h))
        }
        guard !entry.hasIntrinsicReport || size != entry.intrinsicSize else { return }
        entry.hasIntrinsicReport = true
        entry.intrinsicSize = size
        intrinsicSizes.updateValue(size, forKey: entry.nonce)
        guard !intrinsicFlushPending else { return }
        intrinsicFlushPending = true
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            let flush = { [weak self] in self?.flushIntrinsicSizes() }
            if let session = self.session { session.whenIdle { flush() } } else { flush() }
        }
    }

    private func flushIntrinsicSizes() {
        let queued = intrinsicSizes
        intrinsicSizes.removeAll()
        intrinsicFlushPending = false
        var sizes: [(UInt32, CGSize?)] = []
        var presenter: Presenter?
        for (nonce, size) in queued {
            guard let entry = entries.values.first(where: { $0.nonce == nonce }),
                  NativeProcess.incarnation(entry.instance) == nonce,
                  let owner = entry.owner, let p = owner.presenter, p.views[entry.id] === owner else { continue }
            presenter = p
            sizes.append((entry.id, size))
        }
        if !sizes.isEmpty { presenter?.onIntrinsic?(sizes) }
    }

    /// The platform widget occupies CSS's content box, as the custom element's
    /// DOM content does. It never reports this assigned frame as a natural size.
    func laidOut(_ owner: NodeView) {
        guard let entry = entries[owner.id], let view = entry.view else { return }
        if entry.sizing && owner.bounds.isEmpty { return }
        entry.sizing = false
        view.frame = owner.contentBox()
        #if os(iOS) || os(tvOS)
        view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        #endif
    }

    #if os(iOS) || os(tvOS)
    func refreshWorldGeometry() {
        for entry in entries.values {
            guard let view = entry.view else { continue }
            entry.worldLayout.refresh(view)
        }
    }
    #endif

    fileprivate func received(nonce: UInt32, kind: UInt32, data: Data) {
        guard let entry = entries.values.first(where: { $0.nonce == nonce }), let owner = entry.owner,
              let presenter = owner.presenter, presenter.views[entry.id] === owner
        else { return dropped(nonce: nonce, kind: kind) }
        if kind == 9 { intrinsic(entry, data: data); return }
        guard kind < NativeViews.kinds.count else { return log("\(entry.name) #\(entry.id): refused event kind \(kind)") }
        #if os(iOS) || os(tvOS)
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
        case 5: presenter.key(id, KeyPress(text))
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

#if os(iOS) || os(tvOS)
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
        if owner.bounds.isEmpty { view.autoresizingMask = []; entry.sizing = true } else { view.frame = owner.contentBox() }
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
