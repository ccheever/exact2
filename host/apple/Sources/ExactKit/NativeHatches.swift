// The host's side of the module hatches (@ref LLP 1075.003 §3.2): the app's
// one module receives Exact's own UIKit objects at defined moments — a
// navigation controller when Exact builds it, a route when its controller
// is built, changed and ended — and acts back by clicking an authored
// control. The module side, its handles and the table's entries are
// `host/apple/modules/ExactNativeModule.swift`; NativeModule.swift reads the
// table. Every call is on the main thread and named in the journal, so
// `logs` shows what app code ran. macOS projects no routes, so its route
// and tab hatches never run there (§3.11); `element` runs on both (LLP
// 1075.003.000, ElementHatches.swift).
//
// The host's table, handed to the module once (`module_connect`):
//
//    0  u32 size                96
//    8  resolve(host, routeKey, keyLen, id, idLen) → node (0: none)
//   16  act(host, node, action) → 0 done   action 0 click, 1 focus, 2 blur
//   24  log(host, text, len)
//   32  delegate(host, controller, object)  the app's delegate for a
//        controller whose own slot Exact keeps (nil clears it)
//   40  toolbar_item(host, toolbar, item)    an item the app adds after
//        Exact's to the window toolbar (macOS, LLP 1075.003.000 §3.7)
//   56  input(host, node, text, len) → 0 queued: an authored text field's
//        whole value, as a person's typing would leave it (LLP
//        1075.003.000.001 §2.5); queued like `act`
//   80  owns(…), 88 parts(…): regions and parts (§3.4, §3.5, HatchRegions.swift)
//   64  frames(host, token, on), 72 after(host, token, ms): the frame
//        clock (LLP 1075.003.000.001 §2.4, HatchClock.swift)
//   48  diagnostics → { u32 size 16; 8 record(…) }, or nil in a production
//        bake: what hatch code says of itself (LLP 1075.003.000.001 §3.2,
//        HatchDiagnostics.swift). `record` may be called on any thread.
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

/// The module's hatch entries (NativeModule.swift reads them from its table).
typealias HatchConnectFn = @convention(c) (UnsafeMutableRawPointer?, UnsafeRawPointer?) -> Void
typealias HatchNavigationFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UnsafeMutableRawPointer?, UInt32) -> UInt32
typealias HatchRouteFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32) -> Void
typealias HatchTabsFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UnsafeMutableRawPointer?, UInt32) -> Void
typealias HatchTabContainerFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UnsafePointer<UnsafeMutableRawPointer?>?, UInt32) -> UnsafeMutableRawPointer?
typealias HatchElementFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32) -> UInt32
typealias HatchToolbarFn = @convention(c) (UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UnsafeMutableRawPointer?) -> Void

private typealias HatchResolveFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UnsafePointer<UInt8>?, UInt32) -> UInt32
private typealias HatchActFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UInt32) -> Int32
private typealias HatchLogFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32) -> Void
private typealias HatchDelegateFn = @convention(c) (UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UnsafeMutableRawPointer?) -> Void

private func hatchText(_ bytes: UnsafePointer<UInt8>?, _ length: UInt32) -> String {
    bytes.map { String(decoding: UnsafeBufferPointer(start: $0, count: Int(length)), as: UTF8.self) } ?? ""
}

private func hatchSession(_ host: UnsafeMutableRawPointer?) -> ExactSession? {
    guard Thread.isMainThread else { return nil }
    return ExactSession.session(for: ExactRuntime(UInt(bitPattern: host)))
}

private let hatchResolve: HatchResolveFn = { host, key, keyLength, id, idLength in
    #if os(iOS) || os(tvOS)
    hatchSession(host)?.presenter.navigation.resolve(route: hatchText(key, keyLength), id: hatchText(id, idLength))?.id ?? 0
    #else
    0
    #endif
}

private let hatchAct: HatchActFn = { host, node, action in
    hatchSession(host)?.presenter.elements.act(node, action) == true ? 0 : 1
}

private typealias HatchInputFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UnsafePointer<UInt8>?, UInt32) -> Int32
private let hatchInputText: HatchInputFn = { host, node, bytes, length in
    hatchSession(host)?.presenter.elements.input(node, hatchText(bytes, length)) == true ? 0 : 1
}

private typealias HatchFramesFn = @convention(c) (UnsafeMutableRawPointer?, UInt64, Int32) -> Void
private typealias HatchAfterFn = @convention(c) (UnsafeMutableRawPointer?, UInt64, Double) -> Void
private let hatchFrames: HatchFramesFn = { host, token, on in hatchSession(host)?.natives.hatchClock.frames(token, on: on != 0) }
private let hatchAfter: HatchAfterFn = { host, token, ms in hatchSession(host)?.natives.hatchClock.after(token, ms: ms) }

private typealias HatchOwnsFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UInt32, UInt32, UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UInt32) -> Int32
private typealias HatchPartsFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UnsafePointer<UInt8>?, UInt32, UnsafePointer<UnsafeMutableRawPointer?>?, UInt32) -> Int32
private let hatchOwns: HatchOwnsFn = { host, scope, scopeLength, node, kind, object, what, whatLength, flags in
    guard let regions = hatchSession(host)?.presenter.elements.regions else { return 1 }
    let bound = object.map { Unmanaged<AnyObject>.fromOpaque($0).takeUnretainedValue() }
    return regions.owns(scope: hatchText(scope, scopeLength), node: node, kind: kind, object: bound, what: hatchText(what, whatLength), surface: flags & 1 != 0) ? 0 : 1
}
private let hatchParts: HatchPartsFn = { host, node, json, length, views, count in
    guard let elements = hatchSession(host)?.presenter.elements, let json,
          let rows = try? JSONSerialization.jsonObject(with: Data(bytes: json, count: Int(length))) as? [[String: String]], rows.count == Int(count) else { return 1 }
    let word = elements.presenter.views[node]?.props["hatch"] ?? ""
    let list = rows.enumerated().map { index, row in
        HatchRegions.Part(id: row["id"] ?? "", role: row["role"] ?? "", label: row["label"] ?? "",
                          view: views?[index].map { Unmanaged<AnyObject>.fromOpaque($0).takeUnretainedValue() } as? PlatformView)
    }
    return elements.regions.setParts(node: node, scope: "element \(word)", list) ? 0 : 1
}

private let hatchLog: HatchLogFn = { host, bytes, length in
    hatchSession(host)?.log("hatch \(hatchText(bytes, length))")
}

private let hatchDelegate: HatchDelegateFn = { host, controller, object in
    guard let controller, let session = hatchSession(host) else { return }
    let delegate = object.map { Unmanaged<AnyObject>.fromOpaque($0).takeUnretainedValue() }
    #if os(iOS) || os(tvOS)
    session.presenter.navigation.setAppDelegate(Unmanaged<AnyObject>.fromOpaque(controller).takeUnretainedValue(), delegate)
    #else
    guard Unmanaged<AnyObject>.fromOpaque(controller).takeUnretainedValue() === session.presenter.toolbar.toolbar else { return }
    session.presenter.toolbar.setAppDelegate(delegate as? NSToolbarDelegate)
    #endif
}

private let hatchToolbarItem: HatchToolbarFn = { host, toolbar, item in
    #if os(macOS)
    guard let toolbar, let item, let session = hatchSession(host),
          Unmanaged<AnyObject>.fromOpaque(toolbar).takeUnretainedValue() === session.presenter.toolbar.toolbar,
          let added = Unmanaged<AnyObject>.fromOpaque(item).takeUnretainedValue() as? NSToolbarItem else { return }
    session.presenter.toolbar.addAppItem(added)
    #endif
}

private typealias HatchRecordFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UInt32, UnsafePointer<UInt8>?, UInt32, UnsafePointer<UInt8>?, UInt32, Double, UnsafePointer<UInt8>?, UInt32) -> UInt64

private let hatchRecord: HatchRecordFn = { host, kind, node, scope, scopeLength, name, nameLength, value, text, textLength in
    guard let store = HatchDiagnostics.store(for: ExactRuntime(UInt(bitPattern: host))) else { return 0 }
    return store.record(kind: kind, node: node, scope: hatchText(scope, scopeLength), name: hatchText(name, nameLength), value: value,
                        text: text.map { Data(bytes: $0, count: Int(textLength)) } ?? Data())
}

/// The diagnostics' call table, which a development build's host table names.
private let hatchDiagnosticsTable: UnsafeRawPointer = {
    let t = UnsafeMutableRawPointer.allocate(byteCount: 16, alignment: 8)
    t.initializeMemory(as: UInt8.self, repeating: 0, count: 16)
    t.storeBytes(of: UInt32(16), as: UInt32.self)
    t.storeBytes(of: unsafeBitCast(hatchRecord, to: UnsafeRawPointer.self), toByteOffset: 8, as: UnsafeRawPointer.self)
    return UnsafeRawPointer(t)
}()

/// The host's callbacks, one table for the process: each finds its session
/// by the handle it is called with.
private let hatchHostTable: UnsafeRawPointer = {
    let size = 96
    let t = UnsafeMutableRawPointer.allocate(byteCount: size, alignment: 8)
    t.initializeMemory(as: UInt8.self, repeating: 0, count: size)
    t.storeBytes(of: UInt32(size), as: UInt32.self)
    t.storeBytes(of: unsafeBitCast(hatchResolve, to: UnsafeRawPointer.self), toByteOffset: 8, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(hatchAct, to: UnsafeRawPointer.self), toByteOffset: 16, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(hatchLog, to: UnsafeRawPointer.self), toByteOffset: 24, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(hatchDelegate, to: UnsafeRawPointer.self), toByteOffset: 32, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(hatchToolbarItem, to: UnsafeRawPointer.self), toByteOffset: 40, as: UnsafeRawPointer.self)
    if HatchDiagnostics.measuring { t.storeBytes(of: hatchDiagnosticsTable, toByteOffset: 48, as: UnsafeRawPointer.self) }
    t.storeBytes(of: unsafeBitCast(hatchInputText, to: UnsafeRawPointer.self), toByteOffset: 56, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(hatchFrames, to: UnsafeRawPointer.self), toByteOffset: 64, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(hatchAfter, to: UnsafeRawPointer.self), toByteOffset: 72, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(hatchOwns, to: UnsafeRawPointer.self), toByteOffset: 80, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(hatchParts, to: UnsafeRawPointer.self), toByteOffset: 88, as: UnsafeRawPointer.self)
    return UnsafeRawPointer(t)
}()

/// A route hatch's moment.
enum RouteHatchEvent: UInt32 {
    case built = 0, changed = 1, ended = 2
    var name: String { switch self { case .built: "built"; case .changed: "changed"; case .ended: "ended" } }
}

extension NativeViews {
    /// `tabs` for Exact's tab container (`built`), or its end.
    func tabsHatch(_ controller: AnyObject?, event: UInt32, index: Int = 0) {
        guard hatchesConnected, let instance, let (tabs, _) = tabCalls else { return }
        session?.log("hatch tabs: \(["built", "retired", "the router selected tab \(index) in the app's container", "the app's container retired"][Int(min(event, 3))])")
        timedHatch("tabs", event == 0 ? "built" : event == 2 ? "changed" : "ended") {
            tabs(instance, event, controller.map { Unmanaged.passUnretained($0).toOpaque() }, UInt32(index))
        }
    }

    /// One hatch call: under the crash breadcrumb, production included (§4.4),
    /// and timed by the session's store in a development build (§3.1).
    func timedHatch<T>(_ scope: String, _ moment: String, site: Int? = nil, counts: Bool = true, _ body: () -> T) -> T {
        if let crumbSlot { HatchBreadcrumb.shared?.push(crumbSlot, name: scope, moment: moment, incarnation: hatchIncarnation) }
        defer { if let crumbSlot { HatchBreadcrumb.shared?.pop(crumbSlot) } }
        guard let store = session?.hatchDiagnostics else { return body() }
        return store.timed(scope, moment, site: site, counts: counts, body)
    }

    /// This session's slot in the process's breadcrumb, taken as its hatches
    /// first connect; what earlier runs left is said by the first to connect.
    private func takeBreadcrumb() {
        hatchIncarnation &+= 1
        hatchSites = [:]
        guard crumbSlot == nil, let crumbs = HatchBreadcrumb.shared else { return }
        crumbSlot = crumbs.take(label: session?.label ?? "")
        if crumbSlot == nil { session?.log("hatch: 8 sessions hold the crash breadcrumb's slots; this one runs without one") }
        if !HatchBreadcrumb.reported {
            HatchBreadcrumb.reported = true
            for line in HatchBreadcrumb.lastEnds { session?.log(line) }
        }
    }

    /// `tabContainer`: a container the app owns for these stacks, or nil.
    func tabContainerHatch(names: [String], nodes: [UInt32], selected: Int, controllers: [AnyObject]) -> AnyObject? {
        guard hatchesConnected, let instance, let (_, container) = tabCalls else { return nil }
        let json = (try? JSONSerialization.data(withJSONObject: ["names": names, "nodes": nodes, "selected": selected])) ?? Data()
        var pointers: [UnsafeMutableRawPointer?] = controllers.map { Unmanaged.passUnretained($0).toOpaque() }
        let made = timedHatch("tabContainer", "built") {
            json.withUnsafeBytes { j in
                pointers.withUnsafeMutableBufferPointer { c in
                    container(instance, j.bindMemory(to: UInt8.self).baseAddress, UInt32(json.count), UnsafePointer(c.baseAddress), UInt32(c.count))
                }
            }
        }
        let owned = made.map { Unmanaged<AnyObject>.fromOpaque($0).takeRetainedValue() }
        session?.log("hatch tabContainer: \(owned.map { "the app's \(type(of: $0))" } ?? "Exact's")")
        return owned
    }

    /// `element` (built, changed) or `elementEnded` for a hatched node (LLP
    /// 1075.003.000): its view, its kind's platform object, and
    /// `{"hatch", "node", "id", "kind", "data"}`. `quiet` leaves the call out
    /// of the journal (a list row's after the first; `state` counts them).
    /// True when the hatch set `reusable` (LLP 1075.003.000.000 §8).
    /// A node's plan site, by which its hatch's calls are timed (§3.1): asked
    /// of the runner once a node, and only where calls are timed at all. A
    /// node's last call finds what its first kept, the runner's node being
    /// gone by then.
    private func planSite(_ id: UInt32, ended: Bool) -> Int? {
        guard HatchDiagnostics.measuring, let session else { return nil }
        if ended { return hatchSites.removeValue(forKey: id) ?? nil }
        if let known = hatchSites[id] { return known }
        let reply = try? JSONSerialization.jsonObject(with: Data(session.agent("{\"op\":\"node\",\"id\":\(id)}").utf8)) as? [String: Any]
        let site = (reply?["site"] as? NSNumber)?.intValue
        hatchSites[id] = .some(site)
        return site
    }

    @discardableResult
    func elementHatch(_ node: NodeView, event: UInt32, platform: AnyObject?, quiet: Bool = false) -> Bool {
        guard hatchesConnected, let instance, let call = elementCall else { return false }
        let fields: [String: Any] = ["hatch": node.props["hatch"] ?? "", "node": node.id, "id": node.props["id"] ?? "", "kind": node.kind]
        var json = (try? JSONSerialization.data(withJSONObject: fields)) ?? Data("{}".utf8)
        json.removeLast()
        json.append(Data(",\"data\":\(node.props["dataset"] ?? "{}")}".utf8))
        let word = node.props["hatch"] ?? ""
        if !quiet { session?.log("hatch element \(word) #\(node.id): \(["built", "changed", "ended"][Int(min(event, 2))])") }
        // ElementHatches counts a node's calls; the store times them.
        let flags = timedHatch("element \(word)", ["built", "changed", "ended"][Int(min(event, 2))], site: planSite(node.id, ended: event == 2), counts: false) {
            json.withUnsafeBytes { j in
                call(instance, event, Unmanaged.passUnretained(node).toOpaque(), platform.map { Unmanaged.passUnretained($0).toOpaque() },
                     j.bindMemory(to: UInt8.self).baseAddress, UInt32(json.count))
            }
        }
        // A span its hatch left open ends with the node, abandoned.
        if event == 2 { session?.hatchDiagnostics.ended(node: node.id) }
        return flags & 1 != 0
    }

    /// `toolbar` for the window toolbar Exact installed (macOS, LLP
    /// 1075.003.000 §3.7).
    func toolbarHatch(_ toolbar: AnyObject, window: AnyObject) {
        guard hatchesConnected, let instance, let call = toolbarCall else { return }
        session?.log("hatch toolbar: built")
        timedHatch("toolbar", "built") { call(instance, Unmanaged.passUnretained(toolbar).toOpaque(), Unmanaged.passUnretained(window).toOpaque()) }
    }

    /// After the session's module is made: hand it the host's callbacks
    /// when its table has hatches, then let the presenter replay the objects
    /// it built before (a cold launch's, LLP 1075.003 Q3 (c)).
    func connectHatches(_ connect: HatchConnectFn, _ navigation: HatchNavigationFn, _ route: HatchRouteFn,
                      _ tabs: (HatchTabsFn, HatchTabContainerFn)?, _ element: HatchElementFn?, _ toolbar: HatchToolbarFn?,
                      _ module: UnsafeMutableRawPointer) {
        // A new module is a new incarnation: its counts start at nothing.
        session?.hatchDiagnostics.reset()
        takeBreadcrumb()
        connect(module, hatchHostTable)
        hatchCalls = (navigation, route)
        tabCalls = tabs
        elementCall = element
        toolbarCall = toolbar
        hatchesConnected = true
        session?.log("hatch: connected")
        DispatchQueue.main.async { [weak self] in self?.onHatchesConnected?() }
    }

    private var hatchTarget: ((navigation: HatchNavigationFn, route: HatchRouteFn), UnsafeMutableRawPointer)? {
        guard hatchesConnected, let instance, let hatchCalls else { return nil }
        return (hatchCalls, instance)
    }

    /// `navigation` for a controller Exact built (`built`), or the handle's
    /// end (`!built`): returns the stack's `showsBar`, Exact's default when
    /// no hatch ran.
    func navigationHatch(_ controller: AnyObject, built: Bool, showsBar: Bool, label: String) -> Bool {
        guard let (calls, module) = hatchTarget else { return showsBar }
        let hatch = calls.navigation
        let flags = timedHatch("navigation", built ? "built" : "ended") { hatch(module, built ? 0 : 1, Unmanaged.passUnretained(controller).toOpaque(), showsBar ? 1 : 0) }
        guard built else { return showsBar }
        let shows = flags & 1 != 0
        session?.log("hatch navigation \(label): built, showsBar \(shows)\(shows != showsBar ? " (the hatch's)" : "")")
        return shows
    }

    /// `route` (built, changed) or `routeEnded`, with the route's `data-*`
    /// words (its `dataset` row, already a JSON object of strings).
    func routeHatch(_ event: RouteHatchEvent, controller: AnyObject, navigation: AnyObject?, scroll: AnyObject?,
                   key: String, dataset: String?) {
        guard let (calls, module) = hatchTarget else { return }
        let hatch = calls.route
        let keyJSON = (try? JSONSerialization.data(withJSONObject: [key])).map { String(decoding: $0, as: UTF8.self).dropFirst().dropLast() } ?? "\"\""
        let json = Data("{\"key\":\(keyJSON),\"data\":\(dataset ?? "{}")}".utf8)
        let raw = { (o: AnyObject?) in o.map { Unmanaged.passUnretained($0).toOpaque() } }
        session?.log("hatch route \(key): \(event.name)")
        timedHatch("route", event.name) {
            json.withUnsafeBytes { j in
                hatch(module, event.rawValue, raw(controller), raw(navigation), raw(scroll),
                      j.bindMemory(to: UInt8.self).baseAddress, UInt32(json.count))
            }
        }
        if event == .ended { session?.presenter.elements.regions.ended(scope: "route \(key)") }
    }
}
