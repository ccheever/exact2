// The module side of the native-module table (@ref LLP 1024 D4, LLP 1067.000),
// compiled into the app's one artifact, `libexact_modules.dylib`, with the
// app's own `modules/apple/*.swift` by `host/apple/build.mjs`. The host side,
// and the table's layout, is `Sources/ExactKit/NativeModule.swift`.
//
// An app has one module: a subclass of `ExactModule`, named once.
//
//     final class Recorder: ExactModule {
//         override class var views: [String: ExactNativeFactory] {
//             ["waveform-view": ExactNativeFactory(for: Recorder.self) { recorder, props, events in
//                 WaveformView(recorder: recorder, events: events) }]
//         }
//         override func later(_ request: [String: Any], reply: ExactReply) { … }
//         override func call(_ request: [String: Any]) throws -> [String: Any] { … }
//     }
//     let exactModule: ExactModule.Type = Recorder.self
//
// The host makes one instance per session, at the first view or long call
// that needs it, and destroys it with the session, after its views. Its
// views receive it, so a view and a function share one object. Every entry
// (init, the views' create, props and destroy, `later`, `call`, `destroy`) is
// on the main thread (LLP 1067.000 D4); long work goes on the module's own
// queues, and a reply may be sent from any thread, once. `call` answers inside
// the TypeScript answer that asked, within its 100 ms budget, and holds the
// main thread while it runs: keep it to cheap queries. The web has no
// synchronous call, so a portable source falls back to `native.later`. `views` is the roster, read
// once per process. Each view instance subclasses `ExactNativeInstance`.
//
// A factory that sets `reuse` opts its tag into reuse in a list (LLP 1068
// §4.8): the instance's `prepareForReuse` makes it as if created with no
// props, the next `setProps` is a first mount, and `events.load()` follows
// once no pixel of the last row's shows (the host keeps the view
// transparent until then).
//
// Hatches (LLP 1075.003 §3.2, iOS): the module also receives Exact's own
// UIKit objects at defined moments — a navigation controller when Exact
// builds it, a route when its controller is built, changed and ended — and
// acts back by clicking an authored control (`route.element(id)?.click()`).
// What Exact owns on each object is LLP 1075.003 §3.5's table; the rest is
// the app's. A route's `data-*` words are read by the typed keys the build
// writes from app.json `data` (`route.data[.title]`).
//
//     #if os(iOS)
//     override func navigation(_ navigation: ExactNavigation) {
//         navigation.controller.navigationBar.prefersLargeTitles = true
//     }
//     override func route(_ route: ExactRoute) {
//         route.controller.navigationItem.rightBarButtonItem = route.data[.trailing].map { id in
//             UIBarButtonItem(image: UIImage(systemName: "square.and.pencil"),
//                             primaryAction: UIAction { _ in route.element(id)?.click() })
//         }
//     }
//     #endif
import Foundation
#if os(macOS)
import AppKit
public typealias ExactNativeView = NSView
#else
import UIKit
public typealias ExactNativeView = UIView
#endif

public typealias ExactNativeEventFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UInt32, UnsafePointer<UInt8>?, UInt32) -> Void
public typealias ExactNativeReplyFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UInt32, UInt32, UnsafePointer<UInt8>?, UInt32) -> Void
/// `changed(host, topic, len)`, from any thread.
public typealias ExactModuleChangedFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32) -> Void
/// `now(host)`: the session's clock in milliseconds, on the main thread.
public typealias ExactModuleNowFn = @convention(c) (UnsafeMutableRawPointer?) -> Double
/// `reply(ctx, status, bytes, len)`: a long call's one answer, from any thread.
public typealias ExactModuleReplyFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UnsafePointer<UInt8>?, Int) -> Void

/// A refusal with a message the host logs and reports in `tree`.
public struct ExactNativeRefusal: Error, CustomStringConvertible {
    public let description: String
    public init(_ message: String) { description = message }
}

/// What the session gives its module.
public final class ExactModuleContext: @unchecked Sendable {
    /// Under the agent: substitute device input before asking the OS
    /// (LLP 1067.000 Q7), so a drive is repeatable and never prompts.
    public let agent: Bool
    /// The app's own directories; under the agent, a scratch tree.
    public let data: URL, cache: URL, temporary: URL
    let host: UnsafeMutableRawPointer?
    private let changedFn: ExactModuleChangedFn
    private let nowFn: ExactModuleNowFn
    /// What the module's code says of itself (LLP 1075.003.000.001 §3.2),
    /// scoped `module`. It records once the hatches have connected, in a
    /// development build; before that, and in production, each call returns.
    public let diagnostics: ExactDiagnostics

    init(json: [String: Any], host: UnsafeMutableRawPointer?, changed: @escaping ExactModuleChangedFn, now: @escaping ExactModuleNowFn) {
        diagnostics = ExactDiagnostics(host: host, scope: "module", node: 0)
        let url = { (key: String) in URL(fileURLWithPath: json[key] as? String ?? NSTemporaryDirectory(), isDirectory: true) }
        agent = json["agent"] as? Bool ?? false
        data = url("data"); cache = url("cache"); temporary = url("temporary")
        self.host = host; changedFn = changed; nowFn = now
    }

    /// The session's clock, in milliseconds: the agent's under the agent,
    /// so what a view draws from it repeats. Main thread.
    public func now() -> Double { nowFn(host) }

    /// Say a device topic changed: every TypeScript answer that called
    /// `native.watch(topic)` is asked again (LLP 1016.002). Any thread.
    public func changed(_ topic: String) {
        let bytes = Array(topic.utf8)
        bytes.withUnsafeBufferPointer { changedFn(host, $0.baseAddress, UInt32($0.count)) }
    }
}

/// The answer to one `native.later` call. Only the first `send` or `fail`
/// answers; one dropped unanswered fails the call.
public final class ExactReply: @unchecked Sendable {
    private let lock = NSLock()
    private var ctx: UnsafeMutableRawPointer?
    private let fn: ExactModuleReplyFn
    init(ctx: UnsafeMutableRawPointer?, fn: @escaping ExactModuleReplyFn) { self.ctx = ctx; self.fn = fn }

    private func answer(_ status: UInt32, _ text: String) {
        lock.lock()
        let taken = ctx
        ctx = nil
        lock.unlock()
        guard let taken else { NSLog("exact module: ignored a second reply"); return }
        let bytes = Array(text.utf8)
        bytes.withUnsafeBufferPointer { fn(taken, status, $0.baseAddress, $0.count) }
    }

    /// Resolve the TypeScript promise with a JSON object.
    public func send(_ value: [String: Any]) {
        guard JSONSerialization.isValidJSONObject(value), let data = try? JSONSerialization.data(withJSONObject: value) else {
            return fail("the reply was not JSON")
        }
        answer(200, String(decoding: data, as: UTF8.self))
    }

    /// Reject the TypeScript promise with a message.
    public func fail(_ message: String) { answer(500, message) }

    deinit { if ctx != nil { fail("the module dropped the reply") } }
}

/// An app's one module: its views, its long calls, one instance a session.
open class ExactModule {
    /// The roster: each tag's factory, read once per process.
    open class var views: [String: ExactNativeFactory] { [:] }
    public let context: ExactModuleContext
    public required init(context: ExactModuleContext) { self.context = context }
    /// A long call (`native.later`): start the work and return; reply once.
    open func later(_ request: [String: Any], reply: ExactReply) {
        reply.fail("\(type(of: self)) answers no native.later")
    }
    /// A cheap query answered now (`native.call`), on the main thread and
    /// inside the asking answer's 100 ms budget. Throw to refuse.
    open func call(_ request: [String: Any]) throws -> [String: Any] {
        throw ExactNativeRefusal("\(type(of: self)) answers no native.call")
    }
    /// The session is ending; its views are already gone.
    open func destroy() {}
    /// The host's side of the hatches, once it has connected (LLP 1075.003).
    var hatches: ExactHatches?
    /// A node the Contract marks `hatch="word"` (LLP 1075.003.000), on every
    /// host with native objects: after the batch that mounts it
    /// (`element.isNew`), and again when its `data-*` words change. It runs
    /// on the main thread as each one mounts, and a hatched node leaves
    /// Exact's fast path: on iOS it is never drawn as a flat leaf into its
    /// parent's layer, and a list row that holds one is never reused.
    /// Measured (LLP 1075.003.000.000): every row's avatar hatched on the
    /// Extra Heavy feed, with this hatch empty, took a fling's CPU up about
    /// 16–20% and its frame rate 115.3 → 110.2 fps on an iPhone 13 Pro Max
    /// (117.6 → 115.1 on an M1 iPad Pro). All of it is the lost row reuse:
    /// set `element.reusable` (and undo in `elementEnded`) and the same feed
    /// ran as if nothing were hatched (115.0 fps, CPU +10 ms/s). The call
    /// itself, and a build with nothing hatched, cost nothing measurable.
    open func element(_ element: ExactElement) {}
    /// A hatched node is leaving; its view goes after this returns, and the
    /// handle does nothing from now on.
    open func elementEnded(_ element: ExactElement) {}
    #if os(macOS)
    /// The window toolbar Exact installed for the Contract's commands (LLP
    /// 1075.003.000 §3.7): once, when installed (at a cold launch, once the
    /// module loads). Its display mode and appearance are the app's; its
    /// command items and delegate slot Exact's.
    open func toolbar(_ toolbar: ExactToolbar) {}
    #endif
    #if os(iOS) || os(tvOS)
    /// A navigation controller Exact built: once, before any route in it is
    /// laid out (at a cold launch, once the module loads, for each one
    /// already built). Set `showsBar` here; the bar's look is the app's.
    open func navigation(_ navigation: ExactNavigation) {}
    /// A route: when its controller is built (`route.isNew`), and again
    /// whenever its `data-*` words or the header Exact projects change, each
    /// time before the frame that shows it. Its `navigationItem` is the
    /// app's, over the defaults Exact projects from an authored header.
    open func route(_ route: ExactRoute) {}
    /// A route's controller is leaving for good; its handle does nothing
    /// from now on.
    open func routeEnded(_ route: ExactRoute) {}
    /// Exact's tab container (LLP 1075.003 §3.7): once, before its tabs are
    /// laid out. Its appearance is the app's; its tabs and selection Exact's.
    open func tabs(_ tabs: ExactTabs) {}
    /// Own the tabs yourself: return a container holding each tab's
    /// controller (`contents.tabs`), which Exact keeps current, or nil to
    /// keep Exact's. Selecting is the app's container's: `contents.select`
    /// presses the authored tab; `contents.onSelect` hears the router's.
    open func tabContainer(_ contents: ExactTabContents) -> UIViewController? { nil }
    #endif
}

/// A node's `data-*` words, read by the typed keys the build writes from
/// app.json `data` (`ExactDataKey.title`; LLP 1075.003 Q2): a misspelled key
/// fails the Swift build, as an undeclared word fails the bake.
public struct ExactData: Sendable {
    let words: [String: String]
    init(_ words: [String: String]) { self.words = words }
    public subscript(_ key: ExactDataKey) -> String? { words[key.name] }
}

/// What hatch code says of itself, for the agent (LLP 1075.003.000.001 §3.2):
/// `logs` shows `log`'s lines, `state.hatches` the counters and snapshots,
/// `perf hatches` the timings. Development only, as `perf` is: in production
/// each call returns at once and keeps nothing. Every call is bounded (64
/// counters and 64 timings a module, 32 open spans, 16 snapshots of 4 KB, a
/// 256-byte line and 20 lines a second of session clock a scope); past a
/// bound it is refused and counted. Names are lowercase letters, digits, `-`
/// and `.`, at most 64 bytes. Any thread may call.
public final class ExactDiagnostics: @unchecked Sendable {
    typealias RecordFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UInt32, UnsafePointer<UInt8>?, UInt32, UnsafePointer<UInt8>?, UInt32, Double, UnsafePointer<UInt8>?, UInt32) -> UInt64
    let host: UnsafeMutableRawPointer?, scope: [UInt8], node: UInt32
    /// The host's entry: nil in a production build, on a host without it, and
    /// before the hatches connect.
    var recordFn: RecordFn?

    init(host: UnsafeMutableRawPointer?, scope: String, node: UInt32, recordFn: RecordFn? = nil) {
        self.host = host; self.scope = Array(scope.utf8); self.node = node; self.recordFn = recordFn
    }

    @discardableResult
    func record(_ kind: UInt32, _ name: String = "", _ value: Double = 0, _ text: [UInt8] = []) -> UInt64 {
        guard let recordFn else { return 0 }
        let name = Array(name.utf8)
        return scope.withUnsafeBufferPointer { s in
            name.withUnsafeBufferPointer { n in
                text.withUnsafeBufferPointer { t in
                    recordFn(host, kind, node, s.baseAddress, UInt32(s.count), n.baseAddress, UInt32(n.count), value, t.baseAddress, UInt32(t.count))
                }
            }
        }
    }

    /// A line in the journal, under this scope.
    public func log(_ text: @autoclosure () -> String) { if recordFn != nil { record(0, "", 0, Array(text().utf8)) } }
    /// A cumulative counter.
    public func count(_ name: String, by: Int = 1) { record(1, name, Double(by)) }
    /// One sample of a timing this code measured itself (wall time, perhaps).
    public func measure(_ name: String, ms: Double) { record(2, name, ms) }
    /// A span on the session clock, so two identical drives time it alike.
    public func begin(_ name: String) -> ExactSpan { ExactSpan(diagnostics: self, id: record(3, name)) }
    /// A snapshot, as JSON, the latest kept. One over 4 KB is refused whole.
    public func publish(_ name: String, _ value: Any) {
        guard recordFn != nil else { return }
        let json = JSONSerialization.isValidJSONObject([value]) ? (try? JSONSerialization.data(withJSONObject: value, options: .fragmentsAllowed)) : nil
        record(5, name, 0, json.map(Array.init) ?? [])
    }
    /// Ask for Save Trace (LLP 1079 D5): at most one a second.
    public func saveTrace() { record(6) }
}

/// A span `begin` opened; `end()` once closes it. One still open when its
/// node ends is counted as abandoned, not timed.
public struct ExactSpan: Sendable {
    let diagnostics: ExactDiagnostics, id: UInt64
    public func end() { if id != 0 { diagnostics.record(4, "", Double(id)) } }
}

/// The host's callbacks for the hatches (LLP 1075.003 §3.2), one table per
/// session: `resolve(host, routeKey, keyLen, id, idLen)` → the node a route
/// holds under an HTML id (0: none); `act(host, node, action)` — 0 click,
/// 1 focus, 2 blur — queued past the batch being applied; `log(host, text,
/// len)` into the journal; `delegate(host, controller, object)`: the app's
/// delegate for a controller whose slot Exact keeps.
final class ExactHatches {
    typealias ResolveFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UnsafePointer<UInt8>?, UInt32) -> UInt32
    typealias ActFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UInt32) -> Int32
    typealias LogFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32) -> Void
    typealias DelegateFn = @convention(c) (UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UnsafeMutableRawPointer?) -> Void
    typealias ToolbarItemFn = @convention(c) (UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UnsafeMutableRawPointer?) -> Void
    let host: UnsafeMutableRawPointer?
    let resolveFn: ResolveFn, actFn: ActFn, logFn: LogFn, delegateFn: DelegateFn
    /// A host table of 48 bytes or more: an item added to the window toolbar.
    let toolbarItemFn: ToolbarItemFn?
    /// One of 56 or more, in a development build: the diagnostics' entry.
    let recordFn: ExactDiagnostics.RecordFn?
    #if os(iOS) || os(tvOS)
    var navigations: [ObjectIdentifier: ExactNavigation] = [:]
    var routes: [String: ExactRoute] = [:]
    var tabs: ExactTabs?
    var contents: ExactTabContents?
    #endif
    var elements: [UInt32: ExactElement] = [:]

    init?(host: UnsafeMutableRawPointer?, table: UnsafeRawPointer) {
        guard table.load(as: UInt32.self) >= 40,
              let resolve = table.load(fromByteOffset: 8, as: UnsafeRawPointer?.self),
              let act = table.load(fromByteOffset: 16, as: UnsafeRawPointer?.self),
              let log = table.load(fromByteOffset: 24, as: UnsafeRawPointer?.self),
              let delegate = table.load(fromByteOffset: 32, as: UnsafeRawPointer?.self) else { return nil }
        self.host = host
        resolveFn = unsafeBitCast(resolve, to: ResolveFn.self)
        actFn = unsafeBitCast(act, to: ActFn.self)
        logFn = unsafeBitCast(log, to: LogFn.self)
        delegateFn = unsafeBitCast(delegate, to: DelegateFn.self)
        toolbarItemFn = table.load(as: UInt32.self) >= 48
            ? table.load(fromByteOffset: 40, as: UnsafeRawPointer?.self).map { unsafeBitCast($0, to: ToolbarItemFn.self) } : nil
        let calls = table.load(as: UInt32.self) >= 56 ? table.load(fromByteOffset: 48, as: UnsafeRawPointer?.self) : nil
        recordFn = calls.flatMap { $0.load(as: UInt32.self) >= 16 ? $0.load(fromByteOffset: 8, as: UnsafeRawPointer?.self) : nil }
            .map { unsafeBitCast($0, to: ExactDiagnostics.RecordFn.self) }
    }

    func log(_ line: String) {
        let bytes = Array(line.utf8)
        bytes.withUnsafeBufferPointer { logFn(host, $0.baseAddress, UInt32($0.count)) }
    }

    func resolve(route: String, id: String) -> UInt32 {
        let key = Array(route.utf8), name = Array(id.utf8)
        return key.withUnsafeBufferPointer { k in
            name.withUnsafeBufferPointer { n in resolveFn(host, k.baseAddress, UInt32(k.count), n.baseAddress, UInt32(n.count)) }
        }
    }

    func act(_ node: UInt32, _ action: UInt32) -> Bool { actFn(host, node, action) == 0 }
}

#if os(iOS) || os(tvOS)
/// A navigation controller Exact built (LLP 1075.003 §3.2, §3.7).
public final class ExactNavigation {
    public let controller: UINavigationController
    /// Whether the stack shows UIKit's bar: one bar per stack, for its life
    /// (LLP 1037 F1). Exact's default is whether the stack's first route is
    /// header-shaped; set it in `navigation`. Under the agent the authored
    /// header paints and the bar stays hidden whatever this says.
    public var showsBar: Bool
    /// The app's navigation delegate. Exact keeps the controller's own slot,
    /// where it reconciles pops, and forwards each call here after its own
    /// handling; what Exact does not implement (a custom transition's
    /// animator) comes straight here.
    public weak var delegate: UINavigationControllerDelegate? {
        didSet {
            guard let hatches else { return }
            let object = delegate.map { Unmanaged.passUnretained($0 as AnyObject).toOpaque() }
            hatches.delegateFn(hatches.host, Unmanaged.passUnretained(controller).toOpaque(), object)
        }
    }
    weak var hatches: ExactHatches?
    init(controller: UINavigationController, showsBar: Bool, hatches: ExactHatches) {
        self.controller = controller; self.showsBar = showsBar; self.hatches = hatches
    }
}

/// A route's controller and what the app said about it (LLP 1075.003 §3.2).
/// Its identity is its `navigationKey`, never a position.
public final class ExactRoute {
    /// The route's `navigationKey`: a router entry's id where the app uses
    /// the router (LLP 1038 D6).
    public let key: String
    public let controller: UIViewController
    /// The stack it is in, when Exact has built one.
    public internal(set) var navigation: ExactNavigation?
    /// The route node's `data-*` words.
    public internal(set) var data: ExactData
    /// The scroll view the route names with `navigationScroll`. Its offset,
    /// insets, size and delegate are Exact's (LLP 1075.003 §3.5).
    public internal(set) weak var contentScrollView: UIScrollView?
    /// Whether this call is the controller's first.
    public internal(set) var isNew: Bool
    /// False once `routeEnded` has run: the handle then does nothing.
    public internal(set) var isLive = true
    weak var hatches: ExactHatches?
    init(key: String, controller: UIViewController, data: ExactData, hatches: ExactHatches) {
        self.key = key; self.controller = controller; self.data = data; isNew = true; self.hatches = hatches
    }

    /// The live node the route holds under this HTML id, resolved now, as
    /// Exact resolves a route's Back control (LLP 1035.001 D1).
    public func element(_ id: String) -> ExactElement? {
        guard let hatches else { return nil }
        guard isLive else {
            hatches.log("route \(key): element(\"\(id)\") on a route that has ended")
            return nil
        }
        let node = hatches.resolve(route: key, id: id)
        return node == 0 ? nil : ExactElement(id: id, node: node, route: self, hatches: hatches)
    }
}

/// Exact's tab container (LLP 1075.003 §3.7).
public final class ExactTabs {
    public let controller: UITabBarController
    /// The app's tab delegate. Exact keeps the controller's own slot: a tab
    /// the bar would select presses its authored tab, after asking this
    /// delegate's `shouldSelect`; the rest is forwarded.
    public weak var delegate: UITabBarControllerDelegate? {
        didSet {
            guard let hatches else { return }
            let object = delegate.map { Unmanaged.passUnretained($0 as AnyObject).toOpaque() }
            hatches.delegateFn(hatches.host, Unmanaged.passUnretained(controller).toOpaque(), object)
        }
    }
    weak var hatches: ExactHatches?
    init(controller: UITabBarController, hatches: ExactHatches) { self.controller = controller; self.hatches = hatches }
}

/// One tab, for a container the app owns: its name, the navigation
/// controller Exact keeps its stack in, and the item its authored tab gives.
public struct ExactTab {
    public let name: String
    public let controller: UINavigationController
    /// The controller's item, which Exact updates in place as the authored tab changes.
    public var item: UITabBarItem { controller.tabBarItem }
}

/// What a container the app owns holds (LLP 1075.003 §3.6): each tab's
/// stack, which tab the router selects, and the way to select one.
public final class ExactTabContents {
    public let tabs: [ExactTab]
    /// The tab the router selects.
    public internal(set) var selected: Int
    /// The router selected another tab: show it.
    public var onSelect: ((Int) -> Void)?
    /// False once Exact retired the container: `select` then does nothing.
    public internal(set) var isLive = true
    let tabNodes: [UInt32]
    weak var hatches: ExactHatches?
    init(tabs: [ExactTab], selected: Int, tabNodes: [UInt32], hatches: ExactHatches) {
        self.tabs = tabs; self.selected = selected; self.tabNodes = tabNodes; self.hatches = hatches
    }
    /// The container selected a tab: Exact presses its authored tab, and the
    /// router decides (its history, its pop to root on a second press).
    public func select(_ index: Int) {
        guard let hatches else { return }
        guard isLive, tabNodes.indices.contains(index), hatches.act(tabNodes[index], 0) else { return hatches.log("tabs: select(\(index)) refused") }
    }
}

#endif

/// An authored element a hatch acts on, as the DOM's: `click()` presses it as
/// a tap does, queued until the batch being applied is done. A route's
/// (`route.element(id)`, iOS) lives while its route does; a node the
/// Contract marks `hatch="word"` (LLP 1075.003.000) while the node does, and
/// carries its view and the platform object of its kind.
public final class ExactElement {
    /// Its HTML id ("" when it has none).
    public let id: String
    let node: UInt32, key: String
    #if os(iOS) || os(tvOS)
    /// The route it was resolved in: once that ends (a pop, a reload), the
    /// element does nothing, so a saved one never reaches a later node.
    weak var route: ExactRoute?
    #endif
    weak var hatches: ExactHatches?
    /// What this node's hatch says of itself (LLP 1075.003.000.001 §3.2),
    /// scoped to its word: `element <word>`.
    public private(set) lazy var diagnostics = ExactDiagnostics(host: hatches?.host, scope: hatch == nil ? "route \(key)" : "element \(key)", node: node, recordFn: hatches?.recordFn)
    /// A hatched node's word, or nil for a route's element.
    public internal(set) var hatch: ExactHatchKey?
    /// A hatched node's `data-*` words.
    public internal(set) var data = ExactData([:])
    /// A hatched node's view. Its frame, transform, alpha, hidden state, the
    /// paint Exact draws and Exact's own subviews are Exact's; add
    /// interactions, gestures, subviews and sublayers of your own (§3.6).
    /// Nil once `elementEnded` has returned: the view may be another row's.
    public internal(set) weak var view: ExactNativeView?
    /// The platform object of a hatched node's kind, or nil: a text field or
    /// text view, a control (a segmented control too), a web view, a scroll
    /// view. What an authored row or attribute writes on it is Exact's; the
    /// rest is yours. A video, frame or native view in a list row may be made
    /// after `built`: the hatch hears `changed` once it is there.
    public internal(set) weak var platform: AnyObject?
    /// Whether this call is the node's first.
    public internal(set) var isNew = true
    /// Set in `element` (or in `elementEnded` itself) when `elementEnded`
    /// undoes everything this hatch adds to `view` (its interactions,
    /// gestures, subviews, sublayers, and any property it changed): a list
    /// row holding the node may then be reused for another row (iOS), as
    /// UIKit reuses a cell after `prepareForReuse`, and the next node there
    /// is a new element with `isNew`. A row whose view still has
    /// interactions or gesture recognizers is never reused, so one left
    /// behind costs the reuse, not another row. Read after each call; on
    /// other hosts it changes nothing (LLP 1075.003.000.000 §8).
    public var reusable = false
    var ended = false
    /// False once the element's route or node has ended: it then does nothing.
    public var isLive: Bool {
        guard !ended else { return false }
        #if os(iOS) || os(tvOS)
        if hatch == nil { return route?.isLive == true }
        #endif
        return hatch != nil
    }
    #if os(iOS) || os(tvOS)
    init(id: String, node: UInt32, route: ExactRoute, hatches: ExactHatches) {
        self.id = id; self.node = node; key = route.key; self.route = route; self.hatches = hatches
    }
    #endif
    init(hatch: ExactHatchKey, id: String, node: UInt32, hatches: ExactHatches) {
        self.id = id; self.node = node; key = hatch.name; self.hatch = hatch; self.hatches = hatches
    }
    private func act(_ action: UInt32, _ name: String) {
        guard let hatches else { return }
        let what = hatch == nil ? "route \(key)" : "element \(key)"
        guard isLive, hatches.act(node, action) else { return hatches.log("\(what): \(name)() on #\(id.isEmpty ? String(node) : id) refused") }
    }
    /// Press it, as HTMLElement.click() does: its `press` handler runs.
    public func click() { act(0, "click") }
    /// Focus it (LLP 1035.001's focus rules).
    public func focus() { act(1, "focus") }
    public func blur() { act(2, "blur") }
    #if os(iOS) || os(tvOS)
    public var scrollView: UIScrollView? { platform as? UIScrollView }
    public var textField: UITextField? { platform as? UITextField }
    public var textView: UITextView? { platform as? UITextView }
    public var control: UIControl? { platform as? UIControl }
    #else
    public var scrollView: NSScrollView? { platform as? NSScrollView }
    public var textField: NSTextField? { platform as? NSTextField }
    public var textView: NSTextView? { platform as? NSTextView }
    public var control: NSControl? { platform as? NSControl }
    #endif
}

#if os(macOS)
/// The window toolbar Exact installs for the Contract's commands (LLP
/// 1075.003.000 §3.7).
public final class ExactToolbar {
    public let toolbar: NSToolbar
    public private(set) weak var window: NSWindow?
    /// The app's toolbar delegate. Exact keeps the toolbar's own slot, where
    /// it supplies its command items, and forwards what it does not answer.
    public weak var delegate: NSToolbarDelegate? {
        didSet {
            guard let hatches else { return }
            let object = delegate.map { Unmanaged.passUnretained($0 as AnyObject).toOpaque() }
            hatches.delegateFn(hatches.host, Unmanaged.passUnretained(toolbar).toOpaque(), object)
        }
    }
    weak var hatches: ExactHatches?
    init(toolbar: NSToolbar, window: NSWindow?, hatches: ExactHatches) { self.toolbar = toolbar; self.window = window; self.hatches = hatches }
    /// Add an item after Exact's; Exact never removes it while the toolbar
    /// stays installed.
    public func add(_ item: NSToolbarItem) {
        guard let hatches else { return }
        guard let fn = hatches.toolbarItemFn else { return hatches.log("toolbar: this host takes no items") }
        fn(hatches.host, Unmanaged.passUnretained(toolbar).toOpaque(), Unmanaged.passUnretained(item).toOpaque())
    }
}
#endif

/// The nine events (kernel `EventKind` ordinals) and host-only size reports.
public final class ExactNativeEvents: @unchecked Sendable {
    let fn: ExactNativeEventFn
    let ctx: UnsafeMutableRawPointer?
    let nonce: UInt32
    init(fn: @escaping ExactNativeEventFn, ctx: UnsafeMutableRawPointer?, nonce: UInt32) { self.fn = fn; self.ctx = ctx; self.nonce = nonce }
    private func send(_ kind: UInt32, _ text: String = "") {
        let bytes = Array(text.utf8)
        bytes.withUnsafeBufferPointer { fn(ctx, nonce, kind, $0.baseAddress, UInt32($0.count)) }
    }
    public func press() { send(0) }
    public func change(_ value: String) { send(1, value) }
    public func hover(_ over: Bool) { send(2, over ? "true" : "false") }
    public func focus() { send(3) }
    public func blur() { send(4) }
    public func key(_ name: String) { send(5, name) }
    public func submit() { send(6) }
    public func load() { send(7) }
    public func message(_ text: String) { send(8, text) }
    /// Preferred content size in points, independent of the assigned frame.
    /// Report after creation and whenever content changes; nil forgets it.
    /// Both axes must be finite and positive. CSS still owns the final frame;
    /// no aspect ratio is inferred. This is not a constrained measure callback.
    /// Any thread; stale instances are ignored, and a turn's reports coalesce.
    public func intrinsicSize(_ size: CGSize?) {
        guard let size else { send(9); return }
        send(9, "\(size.width),\(size.height)")
    }
}

/// Agent-only input. Text replaces the widget's value, as standard `type`
/// does; keys use the agent's web-named chord and optional down/up phase.
/// A nil phase is one complete key press (down followed by up).
/// Throw before changing anything for an unsupported input. Human input and
/// IME continue through the platform responder, never through this hook.
public enum ExactNativeInput {
    case text(String)
    case key(String, phase: String?)
}

/// One instance of a module tag.
open class ExactNativeInstance {
    public let events: ExactNativeEvents
    public init(events: ExactNativeEvents) { self.events = events }
    /// The view the host puts in the node's box; it fills the box, and
    /// observes its own bounds.
    open var view: ExactNativeView { fatalError("\(type(of: self)) has no view") }
    #if os(iOS) || os(tvOS)
    /// A native screen (LLP 1075.003 §3.6): a controller whose view is
    /// `view`, contained in the route's controller as a child while it shows.
    open var controller: UIViewController? { nil }
    #endif
    /// Optional focus destination: the view itself or an attached descendant.
    /// The host owns first-responder changes; return nil to refuse focus.
    open var focusTarget: ExactNativeView? { nil }
    /// Synchronous, main-thread agent input, after the host focuses this view.
    /// Return only after delivery; do not retain a request for later delivery.
    open func agentInput(_ input: ExactNativeInput) throws {
        throw ExactNativeRefusal("native view does not support agent input")
    }
    /// The whole props object, replaced; throw to refuse it.
    open func setProps(_ props: [String: String]) throws {}
    /// PNG bytes of the view, for a tag whose factory sets `snapshot`.
    open func snapshot() throws -> Data { throw ExactNativeRefusal("no snapshot") }
    /// Last call. The events object is dead to the host after this.
    open func destroy() {}
    /// Reuse (a factory with `reuse`, LLP 1068 §4.8): the instance is now as
    /// if created with no props; throw to refuse, and the host destroys it.
    /// The next `setProps` is a first mount, and `events.load()` follows once
    /// nothing of the last row shows. Events sent before that mount are dropped.
    open func prepareForReuse() throws { throw ExactNativeRefusal("no reuse") }
}

#if os(iOS) || os(tvOS)
/// A module view that is a whole screen (LLP 1075.003 §3.6): `screen`'s view
/// fills the node's box, and `screen` is a child of the route's controller,
/// with UIKit's appearance, safe-area and trait propagation.
open class ExactNativeScreen: ExactNativeInstance {
    public let screen: UIViewController
    public init(screen: UIViewController, events: ExactNativeEvents) {
        self.screen = screen
        super.init(events: events)
    }
    open override var view: ExactNativeView { screen.view }
    open override var controller: UIViewController? { screen }
}
#endif

/// A roster entry: how to make an instance from the session's module,
/// whether it answers snapshots, and whether a list may reuse it (LLP 1068
/// §4.8).
public struct ExactNativeFactory {
    public let snapshot: Bool
    public let reuse: Bool
    public let make: (ExactModule, [String: String], ExactNativeEvents) throws -> ExactNativeInstance
    public init(snapshot: Bool = false, reuse: Bool = false, make: @escaping (ExactModule, [String: String], ExactNativeEvents) throws -> ExactNativeInstance) {
        self.snapshot = snapshot
        self.reuse = reuse
        self.make = make
    }
    /// A view that needs nothing from the module.
    public init(snapshot: Bool = false, reuse: Bool = false, make: @escaping ([String: String], ExactNativeEvents) throws -> ExactNativeInstance) {
        self.init(snapshot: snapshot, reuse: reuse) { _, props, events in try make(props, events) }
    }
    /// A view of the app's module, typed: `ExactNativeFactory(for: Recorder.self)
    /// { recorder, props, events in … }`. The session's module is always the
    /// app's `exactModule`; another type is refused by name.
    public init<M: ExactModule>(for module: M.Type, snapshot: Bool = false, reuse: Bool = false, make: @escaping (M, [String: String], ExactNativeEvents) throws -> ExactNativeInstance) {
        self.init(snapshot: snapshot, reuse: reuse) { owner, props, events in
            guard let typed = owner as? M else { throw ExactNativeRefusal("the session's module is \(type(of: owner)), not \(M.self)") }
            return try make(typed, props, events)
        }
    }
}

private func props(_ bytes: UnsafePointer<UInt8>?, _ length: UInt32) throws -> [String: String] {
    guard let bytes, length > 0 else { return [:] }
    let data = Data(bytes: bytes, count: Int(length))
    guard let object = try? JSONSerialization.jsonObject(with: data) as? [String: String] else {
        throw ExactNativeRefusal("props are not a JSON object of strings")
    }
    return object
}

private func write(_ message: String, _ out: UnsafeMutablePointer<UInt8>?, _ capacity: UInt32) {
    guard let out, capacity > 0 else { return }
    let bytes = Array(message.utf8.prefix(Int(capacity) - 1))
    for (i, b) in bytes.enumerated() { out[i] = b }
    out[bytes.count] = 0
}

private final class Handle {
    let instance: ExactNativeInstance
    let reply: ExactNativeReplyFn?
    let ctx: UnsafeMutableRawPointer?
    let nonce: UInt32
    init(_ instance: ExactNativeInstance, reply: ExactNativeReplyFn?, ctx: UnsafeMutableRawPointer?, nonce: UInt32) {
        self.instance = instance; self.reply = reply; self.ctx = ctx; self.nonce = nonce
    }
}

private func handle(_ raw: UnsafeMutableRawPointer?) -> Handle? {
    raw.map { Unmanaged<Handle>.fromOpaque($0).takeUnretainedValue() }
}

private func module(_ raw: UnsafeMutableRawPointer?) -> ExactModule? {
    raw.map { Unmanaged<ExactModule>.fromOpaque($0).takeUnretainedValue() }
}

private let roster: [String: ExactNativeFactory] = exactModule.views

private let create: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UnsafePointer<UInt8>?, UInt32, ExactNativeEventFn?, ExactNativeReplyFn?, UnsafeMutableRawPointer?, UInt32, UnsafeMutablePointer<UInt8>?, UInt32) -> UnsafeMutableRawPointer? = { owner, tag, tagLength, json, jsonLength, event, reply, ctx, nonce, out, capacity in
    let name = tag.map { String(decoding: UnsafeBufferPointer(start: $0, count: Int(tagLength)), as: UTF8.self) } ?? ""
    guard let factory = roster[name] else { write("no factory for \(name)", out, capacity); return nil }
    guard let owner = module(owner) else { write("no module instance", out, capacity); return nil }
    guard let event else { write("no event callback", out, capacity); return nil }
    do {
        let instance = try factory.make(owner, try props(json, jsonLength), ExactNativeEvents(fn: event, ctx: ctx, nonce: nonce))
        return Unmanaged.passRetained(Handle(instance, reply: reply, ctx: ctx, nonce: nonce)).toOpaque()
    } catch {
        write(String(describing: error), out, capacity)
        return nil
    }
}

private let platformView: @convention(c) (UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer? = { raw in
    handle(raw).map { Unmanaged.passUnretained($0.instance.view).toOpaque() }
}

private let setProps: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UnsafeMutablePointer<UInt8>?, UInt32) -> Int32 = { raw, json, length, out, capacity in
    guard let h = handle(raw) else { return 1 }
    do { try h.instance.setProps(try props(json, length)); return 0 } catch {
        write(String(describing: error), out, capacity)
        return 1
    }
}

private let focusTarget: @convention(c) (UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer? = { raw in
    handle(raw)?.instance.focusTarget.map { Unmanaged.passUnretained($0).toOpaque() }
}

private let agentInput: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UnsafeMutablePointer<UInt8>?, UInt32) -> Int32 = { raw, json, length, out, capacity in
    do {
        guard let h = handle(raw) else { throw ExactNativeRefusal("no native instance") }
        let request = try props(json, length)
        let input: ExactNativeInput
        if let key = request["key"], request["text"] == nil {
            let phase = request["phase"]
            guard phase == nil || phase == "down" || phase == "up" else { throw ExactNativeRefusal("invalid key phase") }
            input = .key(key, phase: phase)
        } else if let text = request["text"], request["key"] == nil, request["phase"] == nil {
            input = .text(text)
        } else { throw ExactNativeRefusal("expected text or key") }
        try h.instance.agentInput(input)
        return 0
    } catch { write(String(describing: error), out, capacity); return 1 }
}

private let snapshot: @convention(c) (UnsafeMutableRawPointer?, UInt32) -> Void = { raw, token in
    guard let h = handle(raw), let reply = h.reply else { return }
    do {
        let png = [UInt8](try h.instance.snapshot())
        png.withUnsafeBufferPointer { reply(h.ctx, h.nonce, token, 0, $0.baseAddress, UInt32($0.count)) }
    } catch {
        let text = Array(String(describing: error).utf8)
        text.withUnsafeBufferPointer { reply(h.ctx, h.nonce, token, 2, $0.baseAddress, UInt32($0.count)) }
    }
}

private let destroy: @convention(c) (UnsafeMutableRawPointer?) -> Void = { raw in
    guard let raw else { return }
    let h = Unmanaged<Handle>.fromOpaque(raw)
    h.takeUnretainedValue().instance.destroy()
    h.release()
}

private let moduleCreate: @convention(c) (UnsafePointer<UInt8>?, UInt32, UnsafeMutableRawPointer?, ExactModuleChangedFn?, ExactModuleNowFn?, UnsafeMutablePointer<UInt8>?, UInt32) -> UnsafeMutableRawPointer? = { json, length, host, changed, now, out, capacity in
    guard let changed, let now else { write("no host callbacks", out, capacity); return nil }
    let data = json.map { Data(bytes: $0, count: Int(length)) } ?? Data()
    let object = (try? JSONSerialization.jsonObject(with: data) as? [String: Any]) ?? [:]
    let instance = exactModule.init(context: ExactModuleContext(json: object, host: host, changed: changed, now: now))
    return Unmanaged.passRetained(instance).toOpaque()
}

private let moduleDestroy: @convention(c) (UnsafeMutableRawPointer?) -> Void = { raw in
    guard let raw else { return }
    let m = Unmanaged<ExactModule>.fromOpaque(raw)
    m.takeUnretainedValue().destroy()
    m.release()
}

private let moduleLater: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, Int, UnsafeMutableRawPointer?, ExactModuleReplyFn?) -> Void = { raw, body, length, ctx, fn in
    guard let fn else { return }
    let reply = ExactReply(ctx: ctx, fn: fn)
    guard let m = module(raw) else { return reply.fail("no module instance") }
    let data = body.map { Data(bytes: $0, count: length) } ?? Data()
    guard let request = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
        return reply.fail("the request was not a JSON object")
    }
    m.later(request, reply: reply)
}

private let moduleCall: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, Int, UnsafeMutableRawPointer?, ExactModuleReplyFn?) -> Void = { raw, body, length, slot, fn in
    guard let fn else { return }
    let answer = { (status: UInt32, text: String) in
        let bytes = Array(text.utf8)
        bytes.withUnsafeBufferPointer { fn(slot, status, $0.baseAddress, $0.count) }
    }
    guard let m = module(raw) else { return answer(500, "no module instance") }
    let data = body.map { Data(bytes: $0, count: length) } ?? Data()
    do {
        guard let request = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return answer(500, "the request was not a JSON object")
        }
        let value = try m.call(request)
        guard JSONSerialization.isValidJSONObject(value) else { return answer(500, "the reply was not JSON") }
        answer(200, String(decoding: try JSONSerialization.data(withJSONObject: value), as: UTF8.self))
    } catch {
        answer(500, String(describing: error))
    }
}

private let prepareForReuse: @convention(c) (UnsafeMutableRawPointer?) -> Int32 = { raw in
    guard let h = handle(raw) else { return 1 }
    do { try h.instance.prepareForReuse(); return 0 } catch { return 1 }
}

// The hatches (LLP 1075.003 §3.2): the host connects its callbacks once,
// after the session's module is made; then each moment is one call.
private let moduleConnect: @convention(c) (UnsafeMutableRawPointer?, UnsafeRawPointer?) -> Void = { raw, table in
    guard let m = module(raw), let table else { return }
    m.hatches = ExactHatches(host: m.context.host, table: table)
    m.context.diagnostics.recordFn = m.hatches?.recordFn
}

/// `navigation(module, event, controller, flags) → flags`: event 0 built (the
/// hatch runs), 1 retired (the handle goes). Bit 0 of the flags is
/// `showsBar`, Exact's default in and the stack's choice out.
private let moduleNavigation: @convention(c) (UnsafeMutableRawPointer?, UInt32, UnsafeMutableRawPointer?, UInt32) -> UInt32 = { raw, event, controller, flags in
    #if os(iOS) || os(tvOS)
    guard let m = module(raw), let hatches = m.hatches, let controller else { return flags }
    let nav = Unmanaged<UINavigationController>.fromOpaque(controller).takeUnretainedValue()
    let id = ObjectIdentifier(nav)
    if event == 1 {
        hatches.navigations.removeValue(forKey: id)
        return flags
    }
    let handle = hatches.navigations[id] ?? ExactNavigation(controller: nav, showsBar: flags & 1 != 0, hatches: hatches)
    hatches.navigations[id] = handle
    handle.showsBar = flags & 1 != 0
    m.navigation(handle)
    return handle.showsBar ? 1 : 0
    #else
    return flags
    #endif
}

/// `route(module, event, controller, navigation, scroll, json, len)`: event
/// 0 built, 1 changed, 2 ended; json `{"key": …, "data": {…}}`.
private let moduleRoute: @convention(c) (UnsafeMutableRawPointer?, UInt32, UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32) -> Void = { raw, event, controller, navigation, scroll, json, length in
    #if os(iOS) || os(tvOS)
    guard let m = module(raw), let hatches = m.hatches, let controller, let json, length > 0,
          let object = try? JSONSerialization.jsonObject(with: Data(bytes: json, count: Int(length))) as? [String: Any],
          let key = object["key"] as? String else { return }
    let data = ExactData(object["data"] as? [String: String] ?? [:])
    let view = Unmanaged<UIViewController>.fromOpaque(controller).takeUnretainedValue()
    // A route node replaced under the same key is a new controller: the old
    // one's handle ends before the new one's first call, whichever arrives first.
    func end(_ route: ExactRoute) {
        hatches.routes.removeValue(forKey: route.key)
        route.isLive = false
        m.routeEnded(route)
    }
    if event == 2 {
        if let route = hatches.routes[key], route.controller === view { end(route) }
        return
    }
    let route: ExactRoute
    if let known = hatches.routes[key], known.controller === view {
        route = known
        route.isNew = false
        route.data = data
    } else {
        if let old = hatches.routes[key] { end(old) }
        route = ExactRoute(key: key, controller: view, data: data, hatches: hatches)
        hatches.routes[key] = route
    }
    route.navigation = navigation.flatMap { hatches.navigations[ObjectIdentifier(Unmanaged<UINavigationController>.fromOpaque($0).takeUnretainedValue())] }
    route.contentScrollView = scroll.map { Unmanaged<UIScrollView>.fromOpaque($0).takeUnretainedValue() }
    m.route(route)
    #endif
}

/// `tabs(module, event, controller, index)`: event 0 Exact built its tab
/// container (the hatch runs), 1 it retired, 2 the router selected `index` in
/// a container the app owns, 3 that container retired.
private let moduleTabs: @convention(c) (UnsafeMutableRawPointer?, UInt32, UnsafeMutableRawPointer?, UInt32) -> Void = { raw, event, controller, index in
    #if os(iOS) || os(tvOS)
    guard let m = module(raw), let hatches = m.hatches else { return }
    switch event {
    case 0:
        guard let controller else { return }
        let tabs = ExactTabs(controller: Unmanaged<UITabBarController>.fromOpaque(controller).takeUnretainedValue(), hatches: hatches)
        hatches.tabs = tabs
        m.tabs(tabs)
    case 1:
        hatches.tabs = nil
    case 3:
        hatches.contents?.isLive = false
        hatches.contents = nil
    default:
        guard let contents = hatches.contents, contents.tabs.indices.contains(Int(index)) else { return }
        contents.selected = Int(index)
        contents.onSelect?(Int(index))
    }
    #endif
}

/// `tab_container(module, json, len, controllers, count) → controller`: the
/// tabs (`{"names": […], "nodes": […], "selected": i}`) and their navigation
/// controllers; a container the app owns, retained once for the host, or nil.
private let moduleTabContainer: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UnsafePointer<UnsafeMutableRawPointer?>?, UInt32) -> UnsafeMutableRawPointer? = { raw, json, length, controllers, count in
    #if os(iOS) || os(tvOS)
    guard let m = module(raw), let hatches = m.hatches, let json, let controllers,
          let object = try? JSONSerialization.jsonObject(with: Data(bytes: json, count: Int(length))) as? [String: Any],
          let names = object["names"] as? [String], let nodes = object["nodes"] as? [NSNumber], names.count == Int(count) else { return nil }
    let navs = (0..<Int(count)).compactMap { controllers[$0].map { Unmanaged<UINavigationController>.fromOpaque($0).takeUnretainedValue() } }
    guard navs.count == names.count else { return nil }
    let tabs = zip(names, navs).map { ExactTab(name: $0, controller: $1) }
    let contents = ExactTabContents(tabs: tabs, selected: object["selected"] as? Int ?? 0, tabNodes: nodes.map(\.uint32Value), hatches: hatches)
    guard let container = m.tabContainer(contents) else { return nil }
    hatches.contents = contents
    return Unmanaged.passRetained(container).toOpaque()
    #else
    return nil
    #endif
}

/// `element(module, event, view, platform, json, len) → flags`
/// (LLP 1075.003.000): event 0 built, 1 changed, 2 ended; json {"hatch",
/// "node", "id", "kind", "data"}; bit 0 of the flags: `reusable`.
private let moduleElement: @convention(c) (UnsafeMutableRawPointer?, UInt32, UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32) -> UInt32 = { raw, event, view, platform, json, length in
    guard let m = module(raw), let hatches = m.hatches, let json, length > 0,
          let object = try? JSONSerialization.jsonObject(with: Data(bytes: json, count: Int(length))) as? [String: Any],
          let word = object["hatch"] as? String, let node = (object["node"] as? NSNumber)?.uint32Value else { return 0 }
    if event == 2 {
        guard let element = hatches.elements.removeValue(forKey: node) else { return 0 }
        element.ended = true
        m.elementEnded(element)
        // The view goes to the node pool or away: a handle the app kept no
        // longer reaches it, or the row that takes it next. `reusable` may
        // be said here too, beside the undoing it promises.
        element.view = nil
        element.platform = nil
        return element.reusable ? 1 : 0
    }
    let element: ExactElement
    if let known = hatches.elements[node] {
        element = known
        element.isNew = false
    } else {
        element = ExactElement(hatch: ExactHatchKey(word), id: object["id"] as? String ?? "", node: node, hatches: hatches)
        hatches.elements[node] = element
    }
    element.data = ExactData(object["data"] as? [String: String] ?? [:])
    element.view = view.map { Unmanaged<ExactNativeView>.fromOpaque($0).takeUnretainedValue() }
    element.platform = platform.map { Unmanaged<AnyObject>.fromOpaque($0).takeUnretainedValue() }
    m.element(element)
    return element.reusable ? 1 : 0
}

/// `toolbar(module, toolbar, window)` (macOS, LLP 1075.003.000 §3.7).
private let moduleToolbar: @convention(c) (UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UnsafeMutableRawPointer?) -> Void = { raw, toolbar, window in
    #if os(macOS)
    guard let m = module(raw), let hatches = m.hatches, let toolbar else { return }
    m.toolbar(ExactToolbar(toolbar: Unmanaged<NSToolbar>.fromOpaque(toolbar).takeUnretainedValue(),
                           window: window.map { Unmanaged<NSWindow>.fromOpaque($0).takeUnretainedValue() }, hatches: hatches))
    #endif
}

/// `platform_controller(handle) → UIViewController?`: a native screen's
/// controller (the module keeps ownership), or nil for a plain view.
private let platformController: @convention(c) (UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer? = { raw in
    #if os(iOS) || os(tvOS)
    handle(raw)?.instance.controller.map { Unmanaged.passUnretained($0).toOpaque() }
    #else
    nil
    #endif
}

/// The ABI major this artifact was built against; the host refuses others.
private let major: UInt32 = 3

private let table: UnsafeMutableRawPointer = {
    // Beside the tags, under a key no tag can take (a tag has a hyphen): the
    // hatch words this module was built to handle (LLP 1075.003.000.001
    // §4.3), so the host calls it with no word its code never compiled.
    let words = "\"hatches\":{\"words\":[" + ExactHatchKey._words.map { "\"\($0)\"" }.joined(separator: ",") + "]}"
    let text = "{" + (roster.keys.sorted().map { tag in
        "\"\(tag)\":{\"snapshot\":\(roster[tag]!.snapshot),\"reuse\":\(roster[tag]!.reuse)}"
    } + [words]).joined(separator: ",") + "}"
    let size = 184
    let t = UnsafeMutableRawPointer.allocate(byteCount: size, alignment: 8)
    t.initializeMemory(as: UInt8.self, repeating: 0, count: size)
    t.storeBytes(of: major, as: UInt32.self)
    t.storeBytes(of: UInt32(size), toByteOffset: 4, as: UInt32.self)
    t.storeBytes(of: UnsafeRawPointer(strdup(text)), toByteOffset: 8, as: UnsafeRawPointer?.self)
    t.storeBytes(of: unsafeBitCast(create, to: UnsafeRawPointer.self), toByteOffset: 16, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(platformView, to: UnsafeRawPointer.self), toByteOffset: 24, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(setProps, to: UnsafeRawPointer.self), toByteOffset: 32, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(snapshot, to: UnsafeRawPointer.self), toByteOffset: 40, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(destroy, to: UnsafeRawPointer.self), toByteOffset: 48, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(agentInput, to: UnsafeRawPointer.self), toByteOffset: 64, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(focusTarget, to: UnsafeRawPointer.self), toByteOffset: 112, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleCreate, to: UnsafeRawPointer.self), toByteOffset: 72, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleDestroy, to: UnsafeRawPointer.self), toByteOffset: 80, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleLater, to: UnsafeRawPointer.self), toByteOffset: 88, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleCall, to: UnsafeRawPointer.self), toByteOffset: 96, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(prepareForReuse, to: UnsafeRawPointer.self), toByteOffset: 104, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleConnect, to: UnsafeRawPointer.self), toByteOffset: 120, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleNavigation, to: UnsafeRawPointer.self), toByteOffset: 128, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleRoute, to: UnsafeRawPointer.self), toByteOffset: 136, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleTabs, to: UnsafeRawPointer.self), toByteOffset: 144, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleTabContainer, to: UnsafeRawPointer.self), toByteOffset: 152, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(platformController, to: UnsafeRawPointer.self), toByteOffset: 160, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleElement, to: UnsafeRawPointer.self), toByteOffset: 168, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleToolbar, to: UnsafeRawPointer.self), toByteOffset: 176, as: UnsafeRawPointer.self)
    return t
}()

@_cdecl("exact_native_abi")
public func exactNativeAbi() -> UnsafeRawPointer { UnsafeRawPointer(table) }
