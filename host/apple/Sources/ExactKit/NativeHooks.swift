// The host's side of the module hooks (@ref LLP 1075.003 §3.2): the app's
// one module receives Exact's own UIKit objects at defined moments — a
// navigation controller when Exact builds it, a route when its controller
// is built, changed and ended — and acts back by clicking an authored
// control. The module side, its handles and the table's entries are
// `host/apple/modules/ExactNativeModule.swift`; NativeModule.swift reads the
// table. Every call is on the main thread and named in the journal, so
// `logs` shows what app code ran. macOS projects no routes, so no hook runs
// there (§3.11).
//
// The host's table, handed to the module once (`module_connect`):
//
//    0  u32 size                40
//    8  resolve(host, routeKey, keyLen, id, idLen) → node (0: none)
//   16  act(host, node, action) → 0 done   action 0 click, 1 focus, 2 blur
//   24  log(host, text, len)
//   32  delegate(host, controller, object)  the app's delegate for a
//        controller whose own slot Exact keeps (nil clears it)
//
// `host` is the session's runtime handle, as for `changed` and `now`: a
// destroyed session's is answered with nothing.
import CExact
import Foundation
#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// What a native container covers of a box (LLP 1075.003 §3.5): bars over
/// its edges, added to its padding, or the whole box, which a native bar
/// replaces (`display: none`).
enum HostCover: Equatable {
    struct Edges: Equatable { var top: CGFloat, right: CGFloat, bottom: CGFloat, left: CGFloat }
    case edges(Edges)
    case whole
}

/// The module's hook entries (NativeModule.swift reads them from its table).
typealias HookConnectFn = @convention(c) (UnsafeMutableRawPointer?, UnsafeRawPointer?) -> Void
typealias HookNavigationFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UnsafeMutableRawPointer?, UInt32) -> UInt32
typealias HookRouteFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32) -> Void
typealias HookTabsFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UnsafeMutableRawPointer?, UInt32) -> Void
typealias HookTabContainerFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UnsafePointer<UnsafeMutableRawPointer?>?, UInt32) -> UnsafeMutableRawPointer?

private typealias HookResolveFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UnsafePointer<UInt8>?, UInt32) -> UInt32
private typealias HookActFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UInt32) -> Int32
private typealias HookLogFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32) -> Void
private typealias HookDelegateFn = @convention(c) (UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UnsafeMutableRawPointer?) -> Void

private func hookText(_ bytes: UnsafePointer<UInt8>?, _ length: UInt32) -> String {
    bytes.map { String(decoding: UnsafeBufferPointer(start: $0, count: Int(length)), as: UTF8.self) } ?? ""
}

private func hookSession(_ host: UnsafeMutableRawPointer?) -> ExactSession? {
    guard Thread.isMainThread else { return nil }
    return ExactSession.session(for: ExactRuntime(UInt(bitPattern: host)))
}

private let hookResolve: HookResolveFn = { host, key, keyLength, id, idLength in
    #if os(iOS)
    hookSession(host)?.presenter.navigation.resolve(route: hookText(key, keyLength), id: hookText(id, idLength))?.id ?? 0
    #else
    0
    #endif
}

private let hookAct: HookActFn = { host, node, action in
    #if os(iOS)
    hookSession(host)?.presenter.navigation.act(node, action) == true ? 0 : 1
    #else
    1
    #endif
}

private let hookLog: HookLogFn = { host, bytes, length in
    hookSession(host)?.log("hook \(hookText(bytes, length))")
}

private let hookDelegate: HookDelegateFn = { host, controller, object in
    #if os(iOS)
    guard let controller, let session = hookSession(host) else { return }
    let delegate = object.map { Unmanaged<AnyObject>.fromOpaque($0).takeUnretainedValue() }
    session.presenter.navigation.setAppDelegate(Unmanaged<AnyObject>.fromOpaque(controller).takeUnretainedValue(), delegate)
    #endif
}

/// The host's callbacks, one table for the process: each finds its session
/// by the handle it is called with.
private let hookHostTable: UnsafeRawPointer = {
    let size = 40
    let t = UnsafeMutableRawPointer.allocate(byteCount: size, alignment: 8)
    t.initializeMemory(as: UInt8.self, repeating: 0, count: size)
    t.storeBytes(of: UInt32(size), as: UInt32.self)
    t.storeBytes(of: unsafeBitCast(hookResolve, to: UnsafeRawPointer.self), toByteOffset: 8, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(hookAct, to: UnsafeRawPointer.self), toByteOffset: 16, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(hookLog, to: UnsafeRawPointer.self), toByteOffset: 24, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(hookDelegate, to: UnsafeRawPointer.self), toByteOffset: 32, as: UnsafeRawPointer.self)
    return UnsafeRawPointer(t)
}()

/// A route hook's moment.
enum RouteHookEvent: UInt32 {
    case built = 0, changed = 1, ended = 2
    var name: String { switch self { case .built: "built"; case .changed: "changed"; case .ended: "ended" } }
}

extension NativeViews {
    /// `tabs` for Exact's tab container (`built`), or its end.
    func tabsHook(_ controller: AnyObject?, event: UInt32, index: Int = 0) {
        guard hooksConnected, let instance, let (tabs, _) = tabCalls else { return }
        if event != 2 { session?.log("hook tabs: \(["built", "retired", "", "the app's container retired"][Int(min(event, 3))])") }
        tabs(instance, event, controller.map { Unmanaged.passUnretained($0).toOpaque() }, UInt32(index))
    }

    /// `tabContainer`: a container the app owns for these stacks, or nil.
    func tabContainerHook(names: [String], nodes: [UInt32], selected: Int, controllers: [AnyObject]) -> AnyObject? {
        guard hooksConnected, let instance, let (_, container) = tabCalls else { return nil }
        let json = (try? JSONSerialization.data(withJSONObject: ["names": names, "nodes": nodes, "selected": selected])) ?? Data()
        var pointers: [UnsafeMutableRawPointer?] = controllers.map { Unmanaged.passUnretained($0).toOpaque() }
        let made = json.withUnsafeBytes { j in
            pointers.withUnsafeMutableBufferPointer { c in
                container(instance, j.bindMemory(to: UInt8.self).baseAddress, UInt32(json.count), UnsafePointer(c.baseAddress), UInt32(c.count))
            }
        }
        let owned = made.map { Unmanaged<AnyObject>.fromOpaque($0).takeRetainedValue() }
        session?.log("hook tabContainer: \(owned.map { "the app's \(type(of: $0))" } ?? "Exact's")")
        return owned
    }

    /// After the session's module is made: hand it the host's callbacks
    /// when its table has hooks, then let the presenter replay the objects
    /// it built before (a cold launch's, LLP 1075.003 Q3 (c)).
    func connectHooks(_ connect: HookConnectFn, _ navigation: HookNavigationFn, _ route: HookRouteFn,
                      _ tabs: (HookTabsFn, HookTabContainerFn)?, _ module: UnsafeMutableRawPointer) {
        connect(module, hookHostTable)
        hookCalls = (navigation, route)
        tabCalls = tabs
        hooksConnected = true
        session?.log("hook: connected")
        DispatchQueue.main.async { [weak self] in self?.onHooksConnected?() }
    }

    private var hookTarget: ((navigation: HookNavigationFn, route: HookRouteFn), UnsafeMutableRawPointer)? {
        guard hooksConnected, let instance, let hookCalls else { return nil }
        return (hookCalls, instance)
    }

    /// `navigation` for a controller Exact built (`built`), or the handle's
    /// end (`!built`): returns the stack's `showsBar`, Exact's default when
    /// no hook ran.
    func navigationHook(_ controller: AnyObject, built: Bool, showsBar: Bool, label: String) -> Bool {
        guard let (calls, module) = hookTarget else { return showsBar }
        let hook = calls.navigation
        let flags = hook(module, built ? 0 : 1, Unmanaged.passUnretained(controller).toOpaque(), showsBar ? 1 : 0)
        guard built else { return showsBar }
        let shows = flags & 1 != 0
        session?.log("hook navigation \(label): built, showsBar \(shows)\(shows != showsBar ? " (the hook's)" : "")")
        return shows
    }

    /// `route` (built, changed) or `routeEnded`, with the route's `data-*`
    /// words (its `dataset` row, already a JSON object of strings).
    func routeHook(_ event: RouteHookEvent, controller: AnyObject, navigation: AnyObject?, scroll: AnyObject?,
                   key: String, dataset: String?) {
        guard let (calls, module) = hookTarget else { return }
        let hook = calls.route
        let keyJSON = (try? JSONSerialization.data(withJSONObject: [key])).map { String(decoding: $0, as: UTF8.self).dropFirst().dropLast() } ?? "\"\""
        let json = Data("{\"key\":\(keyJSON),\"data\":\(dataset ?? "{}")}".utf8)
        let raw = { (o: AnyObject?) in o.map { Unmanaged.passUnretained($0).toOpaque() } }
        session?.log("hook route \(key): \(event.name)")
        json.withUnsafeBytes { j in
            hook(module, event.rawValue, raw(controller), raw(navigation), raw(scroll),
                 j.bindMemory(to: UInt8.self).baseAddress, UInt32(json.count))
        }
    }
}
