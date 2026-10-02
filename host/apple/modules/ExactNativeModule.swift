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
// Hooks (LLP 1075.003 §3.2, iOS): the module also receives Exact's own
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

    init(json: [String: Any], host: UnsafeMutableRawPointer?, changed: @escaping ExactModuleChangedFn, now: @escaping ExactModuleNowFn) {
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
    /// The host's side of the hooks, once it has connected (LLP 1075.003).
    var hooks: ExactHooks?
    /// A node the Contract marks `hook="word"` (LLP 1075.003.000), on every
    /// host with native objects: after the batch that mounts it
    /// (`element.isNew`), and again when its `data-*` words change. It runs
    /// on the main thread as each one mounts, and a hooked node leaves
    /// Exact's fast path: on iOS it is never drawn as a flat leaf into its
    /// parent's layer, and a list row that holds one is never reused.
    open func element(_ element: ExactElement) {}
    /// A hooked node is leaving; its view goes after this returns, and the
    /// handle does nothing from now on.
    open func elementEnded(_ element: ExactElement) {}
    #if os(iOS)
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

/// The host's callbacks for the hooks (LLP 1075.003 §3.2), one table per
/// session: `resolve(host, routeKey, keyLen, id, idLen)` → the node a route
/// holds under an HTML id (0: none); `act(host, node, action)` — 0 click,
/// 1 focus, 2 blur — queued past the batch being applied; `log(host, text,
/// len)` into the journal; `delegate(host, controller, object)`: the app's
/// delegate for a controller whose slot Exact keeps.
final class ExactHooks {
    typealias ResolveFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UnsafePointer<UInt8>?, UInt32) -> UInt32
    typealias ActFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UInt32) -> Int32
    typealias LogFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32) -> Void
    typealias DelegateFn = @convention(c) (UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UnsafeMutableRawPointer?) -> Void
    let host: UnsafeMutableRawPointer?
    let resolveFn: ResolveFn, actFn: ActFn, logFn: LogFn, delegateFn: DelegateFn
    #if os(iOS)
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

#if os(iOS)
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
            guard let hooks else { return }
            let object = delegate.map { Unmanaged.passUnretained($0 as AnyObject).toOpaque() }
            hooks.delegateFn(hooks.host, Unmanaged.passUnretained(controller).toOpaque(), object)
        }
    }
    weak var hooks: ExactHooks?
    init(controller: UINavigationController, showsBar: Bool, hooks: ExactHooks) {
        self.controller = controller; self.showsBar = showsBar; self.hooks = hooks
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
    weak var hooks: ExactHooks?
    init(key: String, controller: UIViewController, data: ExactData, hooks: ExactHooks) {
        self.key = key; self.controller = controller; self.data = data; isNew = true; self.hooks = hooks
    }

    /// The live node the route holds under this HTML id, resolved now, as
    /// Exact resolves a route's Back control (LLP 1035.001 D1).
    public func element(_ id: String) -> ExactElement? {
        guard let hooks else { return nil }
        guard isLive else {
            hooks.log("route \(key): element(\"\(id)\") on a route that has ended")
            return nil
        }
        let node = hooks.resolve(route: key, id: id)
        return node == 0 ? nil : ExactElement(id: id, node: node, route: self, hooks: hooks)
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
            guard let hooks else { return }
            let object = delegate.map { Unmanaged.passUnretained($0 as AnyObject).toOpaque() }
            hooks.delegateFn(hooks.host, Unmanaged.passUnretained(controller).toOpaque(), object)
        }
    }
    weak var hooks: ExactHooks?
    init(controller: UITabBarController, hooks: ExactHooks) { self.controller = controller; self.hooks = hooks }
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
    weak var hooks: ExactHooks?
    init(tabs: [ExactTab], selected: Int, tabNodes: [UInt32], hooks: ExactHooks) {
        self.tabs = tabs; self.selected = selected; self.tabNodes = tabNodes; self.hooks = hooks
    }
    /// The container selected a tab: Exact presses its authored tab, and the
    /// router decides (its history, its pop to root on a second press).
    public func select(_ index: Int) {
        guard let hooks else { return }
        guard isLive, tabNodes.indices.contains(index), hooks.act(tabNodes[index], 0) else { return hooks.log("tabs: select(\(index)) refused") }
    }
}

#endif

/// An authored element a hook acts on, as the DOM's: `click()` presses it as
/// a tap does, queued until the batch being applied is done. A route's
/// (`route.element(id)`, iOS) lives while its route does; a node the
/// Contract marks `hook="word"` (LLP 1075.003.000) while the node does, and
/// carries its view and the platform object of its kind.
public final class ExactElement {
    /// Its HTML id ("" when it has none).
    public let id: String
    let node: UInt32, key: String
    #if os(iOS)
    /// The route it was resolved in: once that ends (a pop, a reload), the
    /// element does nothing, so a saved one never reaches a later node.
    weak var route: ExactRoute?
    #endif
    weak var hooks: ExactHooks?
    /// A hooked node's word, or nil for a route's element.
    public internal(set) var hook: ExactHookKey?
    /// A hooked node's `data-*` words.
    public internal(set) var data = ExactData([:])
    /// A hooked node's view. Its frame, transform, alpha, hidden state, the
    /// paint Exact draws and Exact's own subviews are Exact's; add
    /// interactions, gestures, subviews and sublayers of your own (§3.6).
    public internal(set) weak var view: ExactNativeView?
    /// The platform object of a hooked node's kind, or nil: a text field or
    /// text view, a control, a web view, a scroll view. What an authored row
    /// or attribute writes on it is Exact's; the rest is yours.
    public internal(set) weak var platform: AnyObject?
    /// Whether this call is the node's first.
    public internal(set) var isNew = true
    var ended = false
    /// False once the element's route or node has ended: it then does nothing.
    public var isLive: Bool {
        guard !ended else { return false }
        #if os(iOS)
        if hook == nil { return route?.isLive == true }
        #endif
        return hook != nil
    }
    #if os(iOS)
    init(id: String, node: UInt32, route: ExactRoute, hooks: ExactHooks) {
        self.id = id; self.node = node; key = route.key; self.route = route; self.hooks = hooks
    }
    #endif
    init(hook: ExactHookKey, id: String, node: UInt32, hooks: ExactHooks) {
        self.id = id; self.node = node; key = hook.name; self.hook = hook; self.hooks = hooks
    }
    private func act(_ action: UInt32, _ name: String) {
        guard let hooks else { return }
        let what = hook == nil ? "route \(key)" : "element \(key)"
        guard isLive, hooks.act(node, action) else { return hooks.log("\(what): \(name)() on #\(id.isEmpty ? String(node) : id) refused") }
    }
    /// Press it, as HTMLElement.click() does: its `press` handler runs.
    public func click() { act(0, "click") }
    /// Focus it (LLP 1035.001's focus rules).
    public func focus() { act(1, "focus") }
    public func blur() { act(2, "blur") }
    #if os(iOS)
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

/// The nine events, as the kernel's `EventKind` ordinals.
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
}

/// One instance of a module tag.
open class ExactNativeInstance {
    public let events: ExactNativeEvents
    public init(events: ExactNativeEvents) { self.events = events }
    /// The view the host puts in the node's box; it fills the box, and
    /// observes its own bounds.
    open var view: ExactNativeView { fatalError("\(type(of: self)) has no view") }
    #if os(iOS)
    /// A native screen (LLP 1075.003 §3.6): a controller whose view is
    /// `view`, contained in the route's controller as a child while it shows.
    open var controller: UIViewController? { nil }
    #endif
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

#if os(iOS)
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

// The hooks (LLP 1075.003 §3.2): the host connects its callbacks once,
// after the session's module is made; then each moment is one call.
private let moduleConnect: @convention(c) (UnsafeMutableRawPointer?, UnsafeRawPointer?) -> Void = { raw, table in
    guard let m = module(raw), let table else { return }
    m.hooks = ExactHooks(host: m.context.host, table: table)
}

/// `navigation(module, event, controller, flags) → flags`: event 0 built (the
/// hook runs), 1 retired (the handle goes). Bit 0 of the flags is
/// `showsBar`, Exact's default in and the stack's choice out.
private let moduleNavigation: @convention(c) (UnsafeMutableRawPointer?, UInt32, UnsafeMutableRawPointer?, UInt32) -> UInt32 = { raw, event, controller, flags in
    #if os(iOS)
    guard let m = module(raw), let hooks = m.hooks, let controller else { return flags }
    let nav = Unmanaged<UINavigationController>.fromOpaque(controller).takeUnretainedValue()
    let id = ObjectIdentifier(nav)
    if event == 1 {
        hooks.navigations.removeValue(forKey: id)
        return flags
    }
    let handle = hooks.navigations[id] ?? ExactNavigation(controller: nav, showsBar: flags & 1 != 0, hooks: hooks)
    hooks.navigations[id] = handle
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
    #if os(iOS)
    guard let m = module(raw), let hooks = m.hooks, let controller, let json, length > 0,
          let object = try? JSONSerialization.jsonObject(with: Data(bytes: json, count: Int(length))) as? [String: Any],
          let key = object["key"] as? String else { return }
    let data = ExactData(object["data"] as? [String: String] ?? [:])
    let view = Unmanaged<UIViewController>.fromOpaque(controller).takeUnretainedValue()
    // A route node replaced under the same key is a new controller: the old
    // one's handle ends before the new one's first call, whichever arrives first.
    func end(_ route: ExactRoute) {
        hooks.routes.removeValue(forKey: route.key)
        route.isLive = false
        m.routeEnded(route)
    }
    if event == 2 {
        if let route = hooks.routes[key], route.controller === view { end(route) }
        return
    }
    let route: ExactRoute
    if let known = hooks.routes[key], known.controller === view {
        route = known
        route.isNew = false
        route.data = data
    } else {
        if let old = hooks.routes[key] { end(old) }
        route = ExactRoute(key: key, controller: view, data: data, hooks: hooks)
        hooks.routes[key] = route
    }
    route.navigation = navigation.flatMap { hooks.navigations[ObjectIdentifier(Unmanaged<UINavigationController>.fromOpaque($0).takeUnretainedValue())] }
    route.contentScrollView = scroll.map { Unmanaged<UIScrollView>.fromOpaque($0).takeUnretainedValue() }
    m.route(route)
    #endif
}

/// `tabs(module, event, controller, index)`: event 0 Exact built its tab
/// container (the hook runs), 1 it retired, 2 the router selected `index` in
/// a container the app owns, 3 that container retired.
private let moduleTabs: @convention(c) (UnsafeMutableRawPointer?, UInt32, UnsafeMutableRawPointer?, UInt32) -> Void = { raw, event, controller, index in
    #if os(iOS)
    guard let m = module(raw), let hooks = m.hooks else { return }
    switch event {
    case 0:
        guard let controller else { return }
        let tabs = ExactTabs(controller: Unmanaged<UITabBarController>.fromOpaque(controller).takeUnretainedValue(), hooks: hooks)
        hooks.tabs = tabs
        m.tabs(tabs)
    case 1:
        hooks.tabs = nil
    case 3:
        hooks.contents?.isLive = false
        hooks.contents = nil
    default:
        guard let contents = hooks.contents, contents.tabs.indices.contains(Int(index)) else { return }
        contents.selected = Int(index)
        contents.onSelect?(Int(index))
    }
    #endif
}

/// `tab_container(module, json, len, controllers, count) → controller`: the
/// tabs (`{"names": […], "nodes": […], "selected": i}`) and their navigation
/// controllers; a container the app owns, retained once for the host, or nil.
private let moduleTabContainer: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UnsafePointer<UnsafeMutableRawPointer?>?, UInt32) -> UnsafeMutableRawPointer? = { raw, json, length, controllers, count in
    #if os(iOS)
    guard let m = module(raw), let hooks = m.hooks, let json, let controllers,
          let object = try? JSONSerialization.jsonObject(with: Data(bytes: json, count: Int(length))) as? [String: Any],
          let names = object["names"] as? [String], let nodes = object["nodes"] as? [NSNumber], names.count == Int(count) else { return nil }
    let navs = (0..<Int(count)).compactMap { controllers[$0].map { Unmanaged<UINavigationController>.fromOpaque($0).takeUnretainedValue() } }
    guard navs.count == names.count else { return nil }
    let tabs = zip(names, navs).map { ExactTab(name: $0, controller: $1) }
    let contents = ExactTabContents(tabs: tabs, selected: object["selected"] as? Int ?? 0, tabNodes: nodes.map(\.uint32Value), hooks: hooks)
    guard let container = m.tabContainer(contents) else { return nil }
    hooks.contents = contents
    return Unmanaged.passRetained(container).toOpaque()
    #else
    return nil
    #endif
}

/// `element(module, event, view, platform, json, len)` (LLP 1075.003.000):
/// event 0 built, 1 changed, 2 ended; json {"hook", "node", "id", "kind",
/// "data"}.
private let moduleElement: @convention(c) (UnsafeMutableRawPointer?, UInt32, UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32) -> Void = { raw, event, view, platform, json, length in
    guard let m = module(raw), let hooks = m.hooks, let json, length > 0,
          let object = try? JSONSerialization.jsonObject(with: Data(bytes: json, count: Int(length))) as? [String: Any],
          let word = object["hook"] as? String, let node = (object["node"] as? NSNumber)?.uint32Value else { return }
    if event == 2 {
        guard let element = hooks.elements.removeValue(forKey: node) else { return }
        element.ended = true
        m.elementEnded(element)
        return
    }
    let element: ExactElement
    if let known = hooks.elements[node] {
        element = known
        element.isNew = false
    } else {
        element = ExactElement(hook: ExactHookKey(word), id: object["id"] as? String ?? "", node: node, hooks: hooks)
        hooks.elements[node] = element
    }
    element.data = ExactData(object["data"] as? [String: String] ?? [:])
    element.view = view.map { Unmanaged<ExactNativeView>.fromOpaque($0).takeUnretainedValue() }
    element.platform = platform.map { Unmanaged<AnyObject>.fromOpaque($0).takeUnretainedValue() }
    m.element(element)
}

/// `platform_controller(handle) → UIViewController?`: a native screen's
/// controller (the module keeps ownership), or nil for a plain view.
private let platformController: @convention(c) (UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer? = { raw in
    #if os(iOS)
    handle(raw)?.instance.controller.map { Unmanaged.passUnretained($0).toOpaque() }
    #else
    nil
    #endif
}

/// The ABI major this artifact was built against; the host refuses others.
private let major: UInt32 = 2

private let table: UnsafeMutableRawPointer = {
    let text = "{" + roster.keys.sorted().map { tag in
        "\"\(tag)\":{\"snapshot\":\(roster[tag]!.snapshot),\"reuse\":\(roster[tag]!.reuse)}"
    }.joined(separator: ",") + "}"
    let size = 168
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
    t.storeBytes(of: unsafeBitCast(moduleCreate, to: UnsafeRawPointer.self), toByteOffset: 72, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleDestroy, to: UnsafeRawPointer.self), toByteOffset: 80, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleLater, to: UnsafeRawPointer.self), toByteOffset: 88, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleCall, to: UnsafeRawPointer.self), toByteOffset: 96, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(prepareForReuse, to: UnsafeRawPointer.self), toByteOffset: 104, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleConnect, to: UnsafeRawPointer.self), toByteOffset: 112, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleNavigation, to: UnsafeRawPointer.self), toByteOffset: 120, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleRoute, to: UnsafeRawPointer.self), toByteOffset: 128, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleTabs, to: UnsafeRawPointer.self), toByteOffset: 136, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleTabContainer, to: UnsafeRawPointer.self), toByteOffset: 144, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(platformController, to: UnsafeRawPointer.self), toByteOffset: 152, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleElement, to: UnsafeRawPointer.self), toByteOffset: 160, as: UnsafeRawPointer.self)
    return t
}()

@_cdecl("exact_native_abi")
public func exactNativeAbi() -> UnsafeRawPointer { UnsafeRawPointer(table) }
