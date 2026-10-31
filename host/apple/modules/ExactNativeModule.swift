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
import Security
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
    // The frame clock (§2.4): the host's entries, there once the module
    // has the hatch table (before its hatches are called), and what each token calls.
    typealias FramesFn = @convention(c) (UnsafeMutableRawPointer?, UInt64, Int32) -> Void
    typealias AfterFn = @convention(c) (UnsafeMutableRawPointer?, UInt64, Double) -> Void
    var framesFn: FramesFn?, afterFn: AfterFn?
    var ticks: [UInt64: (ExactFrame) -> Void] = [:], afters: [UInt64: () -> Void] = [:]
    private var nextToken: UInt64 = 1

    /// The session's own root view (LLP 1133.027 D17 item 3), which the host
    /// hands over whenever the session's view changes; nil while none is mounted.
    public internal(set) weak var sessionView: AnyObject?

    /// Who posts what an activation is for (LLP 1133.004 D7): the session, its owner, the process.
    let poster: [String: String]
    var emitted: UInt64 = 0

    init(json: [String: Any], host: UnsafeMutableRawPointer?, changed: @escaping ExactModuleChangedFn, now: @escaping ExactModuleNowFn) {
        diagnostics = ExactDiagnostics(host: host, scope: "module", node: 0)
        let url = { (key: String) in URL(fileURLWithPath: json[key] as? String ?? NSTemporaryDirectory(), isDirectory: true) }
        agent = json["agent"] as? Bool ?? false
        data = url("data"); cache = url("cache"); temporary = url("temporary")
        poster = ["session", "owner", "process"].reduce(into: [:]) { $0[$1] = json[$1] as? String ?? "" }
        self.host = host; changedFn = changed; nowFn = now
    }

    /// The session's clock, in milliseconds: the agent's under the agent,
    /// so what a view draws from it repeats. Main thread.
    public func now() -> Double { nowFn(host) }

    /// A frame ticket (LLP 1075.003.000.001 §2.4): `body` at each frame, after
    /// that frame's tasks and timers, with the session clock's time. Logical
    /// time, for behaviour that keeps step with the app (an indicator that
    /// follows an animation): each presented frame on the wall, and under the
    /// agent the virtual 60 Hz display, where a seek stops at each tick, so
    /// the callback sees that instant's state and a drive repeats. Not for
    /// measuring the display: `perf frames` and `diagnostics.measure` are.
    /// Main thread. `stop()` ends it; a reload drops it.
    @discardableResult
    public func frames(_ body: @escaping (ExactFrame) -> Void) -> ExactTicket {
        let token = nextToken
        nextToken += 1
        guard let framesFn else { return ExactTicket {} }
        ticks[token] = body
        framesFn(host, token, 1)
        return ExactTicket { [weak self] in
            guard let self, self.ticks.removeValue(forKey: token) != nil else { return }
            framesFn(self.host, token, 0)
        }
    }

    /// `body` once, `ms` later on the session clock: the agent's under the
    /// agent, where a seek fires it at its own instant. `stop()` before then
    /// and it never runs. Main thread.
    @discardableResult
    public func after(_ ms: Double, _ body: @escaping () -> Void) -> ExactTicket {
        let token = nextToken
        nextToken += 1
        guard let afterFn else { return ExactTicket {} }
        afters[token] = body
        afterFn(host, token, max(0, ms))
        return ExactTicket { [weak self] in
            guard let self, self.afters.removeValue(forKey: token) != nil else { return }
            afterFn(self.host, token, -1)
        }
    }

    /// Say a device topic changed: every TypeScript answer that called
    /// `native.watch(topic)` is asked again (LLP 1016.002). Any thread.
    public func changed(_ topic: String) {
        let bytes = Array(topic.utf8)
        bytes.withUnsafeBufferPointer { changedFn(host, $0.baseAddress, UInt32($0.count)) }
    }

    /// Say the module wrote the app's files (`data`, `cache` or `temporary`;
    /// `path` is the `app:/` path it wrote, or the directory): what a
    /// TypeScript write under `app:/` does by itself, a widget's timeline
    /// is reloaded from the store as written, and every answer that
    /// `native.watch(path)` is asked again. Any thread.
    public func wrote(_ path: String = "app:/data") {
        changed(path.hasPrefix("app:/") ? path : "app:/" + path)
    }
}

/// One frame of a `frames` ticket: the session clock's time in milliseconds,
/// and the commit the frame's state ends at.
public struct ExactFrame: Sendable {
    public let now: Double, seq: UInt64
}

/// What `frames` and `after` return: `stop()` ends the ticket or cancels the wait.
public final class ExactTicket {
    private var end: (() -> Void)?
    init(_ end: @escaping () -> Void) { self.end = end }
    public func stop() { end?(); end = nil }
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

    /// Resolve the TypeScript promise with a JSON object, or with the value
    /// of its one key `exact:answer` (a package's list, a `bool`, or null for
    /// an absent `option`; ExactPackage.swift).
    public func send(_ value: [String: Any]) {
        let sent: Any = value.count == 1 ? value["exact:answer"] ?? value : value
        let scalar = sent is NSNull || sent is NSNumber || sent is String
        guard scalar || JSONSerialization.isValidJSONObject(sent),
              let data = try? JSONSerialization.data(withJSONObject: sent, options: .fragmentsAllowed) else {
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
    /// The Mac app is quitting (⌘Q, the app menu, an Apple Event, SIGTERM,
    /// the last window closing) and no `beforeunload` kept it: start the
    /// work the quit must wait for (stop a child process, flush a socket),
    /// off the main thread, and call `done` when it has finished, from any
    /// thread. The quit waits for every session's module and the storage
    /// in flight, five seconds at most in all (LLP 1097 D10, LLP 1069.010
    /// Q4); `destroy()` follows whether or not the work finished, and
    /// force-ends what is still running (SIGKILL a child) without waiting.
    /// Called once, on the main thread, never on iOS or tvOS. Dropping
    /// `done` uncalled counts as calling it. By default, done at once.
    open func willQuit(done: @escaping () -> Void) { done() }
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
    /// A frame marked `hatch="word"` is about to make a web view (LLP
    /// 1020.002 §9): `configuration` is its `WKWebViewConfiguration`, its
    /// data store and Exact's content worlds set, its preferences the frame's
    /// own. Set an `applicationNameForUserAgent`, content rule lists, URL
    /// scheme handlers for the app's own schemes, or those preferences; the
    /// data store and the content controller must stay the arm's. Change
    /// only the configuration: the node may not be in the tree yet. Called
    /// before each web view the arm makes, again for a new `partition` or a
    /// PDF back to HTML; a hatched frame's first web view waits for the
    /// module to connect, so none is made without it. `element(_:)` then
    /// hands the web view at `built`; if `built` has no `platform` (the view
    /// is made after the batch, as macOS and an iOS list row usually make
    /// it), the `changed` after this call hands it. `isNew` is true at
    /// `built`.
    open func elementConfiguration(_ element: ExactElement, _ configuration: AnyObject) {}
    #if os(macOS)
    /// The window toolbar Exact installed for the Contract's commands (LLP
    /// 1075.003.000 §3.7): once, when installed (at a cold launch, once the
    /// module loads). Its display mode and appearance are the app's; its
    /// command items and delegate slot Exact's.
    open func toolbar(_ toolbar: ExactToolbar) {}
    #endif
    /// The app (LLP 1075.003.000.001 §2.1): when the hatches connect, after
    /// first pixel (`app.isNew`), and again when one of its facts changes.
    /// Never inside a batch. Set what is app-wide here; `app.application` is
    /// nil unless the embedder gave this session the process (§2.1.1).
    open func app(_ app: ExactApp) {}
    /// The session is ending, or its plan reloading: undo what `app` set.
    /// After a reload `app` runs again, with a new handle.
    open func appEnded(_ app: ExactApp) {}
    /// The window the session presents into: when the hatches connect or the
    /// session moves to a window (`window.isNew`), and again on a size or
    /// safe-area change. `window.window` is nil unless the embedder said the
    /// window is this session's own (`window.exclusive`).
    open func window(_ window: ExactWindow) {}
    /// The session is leaving the window (for another, a reload, its end):
    /// take back the recognizers and views `window` added, while it is there.
    open func windowEnded(_ window: ExactWindow) {}
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
    /// A tab whose stack shows no bar and has pushed nothing is a plain view
    /// controller in it until the stack pushes or shows a bar.
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

#if os(macOS)
public typealias ExactPlatformView = NSView
public typealias ExactPlatformRecognizer = NSGestureRecognizer
#else
public typealias ExactPlatformView = UIView
public typealias ExactPlatformRecognizer = UIGestureRecognizer
#endif

/// A control a hatch drew, named for the agent (LLP 1075.003.000.001 §3.5):
/// `tree` lists it under its node with the frame the host observes, and
/// `tap <node>/<id>` reaches it as a real touch or pointer event at its
/// place, never by calling it. What it does when touched is the hatch's own
/// code, which acts on an authored node. `id` is unique within the node.
public struct ExactPart {
    public let id: String, role: String, label: String
    public weak var view: ExactPlatformView?
    public init(id: String, view: ExactPlatformView, role: String, label: String) {
        self.id = id; self.view = view; self.role = role; self.label = label
    }
}

/// The app, as its hatch hears of it (LLP 1075.003.000.001 §2.1): the facts
/// Contract sees, by the web's names, and the platform application object for
/// the one session the embedder gave the process to.
public final class ExactApp {
    #if os(macOS)
    /// `NSApp`, for process-wide state; nil unless `processOwner`.
    public internal(set) weak var application: NSApplication?
    #else
    /// The application, for process-wide state (appearance proxies); nil unless `processOwner`.
    public internal(set) weak var application: UIApplication?
    #endif
    /// Whether the embedder gave this session the process (§2.1.1).
    public internal(set) var processOwner = false
    /// `visible` or `hidden` (`document.visibilityState`).
    public internal(set) var visibilityState = "visible"
    /// `navigator.onLine`, asked of the host on main (else as last told).
    /// Connectivity is watched from the first read: read it once, even in
    /// the `isNew` call, to be told of its changes.
    public var onLine: Bool { hatches?.onLine() ?? toldOnLine }
    var toldOnLine = true
    /// `light` or `dark`.
    public internal(set) var prefersColorScheme = "light"
    /// `no-preference`, `more`, `less` or `custom`.
    public internal(set) var prefersContrast = "no-preference"
    public internal(set) var prefersReducedMotion = false
    public internal(set) var prefersReducedTransparency = false
    /// The root node's `data-*` words: what Contract projects for the app
    /// hatch to know. A change is a `changed` moment.
    public internal(set) var data = ExactData([:])
    /// True in the first `app` call of this handle; false when a fact changed.
    public internal(set) var isNew = true
    public internal(set) var isLive = true
    weak var hatches: ExactHatches?

    /// Say what this hatch set app-wide (§3.4): `owns(appearance: "tab bar
    /// tint: label")`. `state.hatches` lists it; saying it again replaces it.
    public func owns(appearance what: String, surface: Bool = false) {
        hatches?.owns(scope: "app", node: 0, kind: 2, object: nil, what, surface: surface)
    }

    func read(_ json: [String: Any]) {
        processOwner = json["processOwner"] as? Bool ?? false
        data = ExactData(json["data"] as? [String: String] ?? [:])
        let facts = json["facts"] as? [String: Any] ?? [:]
        visibilityState = facts["visibilityState"] as? String ?? visibilityState
        toldOnLine = facts["onLine"] as? Bool ?? toldOnLine
        prefersColorScheme = facts["prefersColorScheme"] as? String ?? prefersColorScheme
        prefersContrast = facts["prefersContrast"] as? String ?? prefersContrast
        prefersReducedMotion = facts["prefersReducedMotion"] as? Bool ?? prefersReducedMotion
        prefersReducedTransparency = facts["prefersReducedTransparency"] as? Bool ?? prefersReducedTransparency
    }
}

/// The window a session presents into, as its hatch hears of it (LLP
/// 1075.003.000.001 §2.1). A session embedded beside other UI shares its
/// window, so the window itself is handed over only when the embedder says
/// it is this session's own.
public final class ExactWindow {
    #if os(macOS)
    /// The window; nil unless `exclusive`.
    public internal(set) weak var window: NSWindow?
    #else
    /// The window and its scene; nil unless `exclusive`.
    public internal(set) weak var window: UIWindow?
    public internal(set) weak var scene: UIWindowScene?
    #endif
    /// Whether the embedder said this window is the session's own (§2.1.1).
    public internal(set) var exclusive = false
    /// The session's surface, in the window's coordinates, as Exact laid it out.
    public internal(set) var frame = CGRect.zero
    /// The surface's safe area: top, right, bottom, left.
    public internal(set) var safeArea: (top: CGFloat, right: CGFloat, bottom: CGFloat, left: CGFloat) = (0, 0, 0, 0)
    /// The window's angle as the web's `screen.orientation.angle`: 0, 90, 180 or 270.
    public internal(set) var angle: Double = 0
    /// True in the first `window` call of this handle; false on a size or safe-area change.
    public internal(set) var isNew = true
    public internal(set) var isLive = true
    weak var hatches: ExactHatches?

    /// Say what this hatch added to the window (§3.4), bound to the object:
    /// `owns(recognizer: threeFinger, "three fingers held 0.8 s: dev menu",
    /// surface: true)`. `surface` marks a hatch-only surface, one that
    /// changes no Contract state and has no authored stand-in.
    public func owns(view: ExactPlatformView, _ what: String, surface: Bool = false) {
        hatches?.owns(scope: "window", node: 0, kind: 0, object: view, what, surface: surface)
    }
    public func owns(recognizer: ExactPlatformRecognizer, _ what: String, surface: Bool = false) {
        hatches?.owns(scope: "window", node: 0, kind: 1, object: recognizer, what, surface: surface)
    }

    func read(_ json: [String: Any]) {
        exclusive = json["exclusive"] as? Bool ?? false
        if let f = json["frame"] as? [Double], f.count == 4 { frame = CGRect(x: f[0], y: f[1], width: f[2], height: f[3]) }
        if let s = json["safeArea"] as? [Double], s.count == 4 { safeArea = (CGFloat(s[0]), CGFloat(s[1]), CGFloat(s[2]), CGFloat(s[3])) }
        angle = json["angle"] as? Double ?? 0
    }
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
    /// One of 64 or more: `input(text)` on an authored field.
    typealias InputFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UnsafePointer<UInt8>?, UInt32) -> Int32
    let inputFn: InputFn?
    /// One of 80 or more: the frame clock's two entries.
    let framesFn: ExactModuleContext.FramesFn?, afterFn: ExactModuleContext.AfterFn?
    /// One of 96 or more: regions and parts (§3.4, §3.5).
    typealias OwnsFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UInt32, UInt32, UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UInt32) -> Int32
    typealias PartsFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UnsafePointer<UInt8>?, UInt32, UnsafePointer<UnsafeMutableRawPointer?>?, UInt32) -> Int32
    let ownsFn: OwnsFn?, partsFn: PartsFn?
    /// One of 104 or more: the bindings' answer (LLP 1121 D12; ExactTamber.swift).
    typealias TamberFn = @convention(c) (UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UnsafeMutablePointer<UInt8>?, UInt32) -> UInt32
    let tamberFn: TamberFn?
    /// One of 120 or more: `app.onLine`.
    typealias OnLineFn = @convention(c) (UnsafeMutableRawPointer?) -> UInt32
    let onLineFn: OnLineFn?
    /// One of 136 or more: a capture's claim on the audio session and its release (LLP 1133.009 D7; ExactExtensions.swift).
    let audioClaimFn: AudioClaims.ClaimFn?, audioReleaseFn: AudioClaims.ReleaseFn?
    /// One of 144 or more: a package picture changed (0) or revoked (1) (LLP 1133.019 D11 item 1).
    typealias PictureFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UnsafePointer<UInt8>?, UInt32) -> Void
    let pictureFn: PictureFn?
    /// One of 160 or more: a package's canvas input (LLP 1133.013 D6; ExactExtensions.swift).
    typealias SurfaceHasFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32) -> UInt32
    typealias SurfaceSensorFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UnsafePointer<UInt8>?, UInt32,
                                                UnsafePointer<Double>?, UInt32, UInt32, Double, UInt32) -> Void
    let surfaceHasFn: SurfaceHasFn?, surfaceSensorFn: SurfaceSensorFn?
    /// One of 168 or more: a package's call of the app's handler (LLP 1133.016 D15 item 1; ExactExtensions.swift).
    let invokeFn: Invocations.InvokeFn?
    /// One of 176 or more: an authored content arrangement.
    typealias ArrangementFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UnsafeMutableRawPointer?) -> Int32
    let arrangementFn: ArrangementFn?
    #if os(iOS) || os(tvOS)
    /// One of 112 or more: a held tab's navigation controller, built (`ExactTab.controller`).
    typealias StackFn = @convention(c) (UnsafeMutableRawPointer?, UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer?
    let stackFn: StackFn?
    var navigations: [ObjectIdentifier: ExactNavigation] = [:]
    var routes: [String: ExactRoute] = [:]
    var tabs: ExactTabs?
    var contents: ExactTabContents?
    #endif
    var elements: [UInt32: ExactElement] = [:]
    var app: ExactApp?, window: ExactWindow?

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
        inputFn = table.load(as: UInt32.self) >= 64
            ? table.load(fromByteOffset: 56, as: UnsafeRawPointer?.self).map { unsafeBitCast($0, to: InputFn.self) } : nil
        let clock = table.load(as: UInt32.self) >= 80
        framesFn = clock ? table.load(fromByteOffset: 64, as: UnsafeRawPointer?.self).map { unsafeBitCast($0, to: ExactModuleContext.FramesFn.self) } : nil
        afterFn = clock ? table.load(fromByteOffset: 72, as: UnsafeRawPointer?.self).map { unsafeBitCast($0, to: ExactModuleContext.AfterFn.self) } : nil
        let regions = table.load(as: UInt32.self) >= 96
        ownsFn = regions ? table.load(fromByteOffset: 80, as: UnsafeRawPointer?.self).map { unsafeBitCast($0, to: OwnsFn.self) } : nil
        partsFn = regions ? table.load(fromByteOffset: 88, as: UnsafeRawPointer?.self).map { unsafeBitCast($0, to: PartsFn.self) } : nil
        tamberFn = table.load(as: UInt32.self) >= 104
            ? table.load(fromByteOffset: 96, as: UnsafeRawPointer?.self).map { unsafeBitCast($0, to: TamberFn.self) } : nil
        #if os(iOS) || os(tvOS)
        stackFn = table.load(as: UInt32.self) >= 112
            ? table.load(fromByteOffset: 104, as: UnsafeRawPointer?.self).map { unsafeBitCast($0, to: StackFn.self) } : nil
        #endif
        onLineFn = table.load(as: UInt32.self) >= 120
            ? table.load(fromByteOffset: 112, as: UnsafeRawPointer?.self).map { unsafeBitCast($0, to: OnLineFn.self) } : nil
        let audio = table.load(as: UInt32.self) >= 136
        audioClaimFn = audio ? table.load(fromByteOffset: 120, as: UnsafeRawPointer?.self).map { unsafeBitCast($0, to: AudioClaims.ClaimFn.self) } : nil
        audioReleaseFn = audio ? table.load(fromByteOffset: 128, as: UnsafeRawPointer?.self).map { unsafeBitCast($0, to: AudioClaims.ReleaseFn.self) } : nil
        pictureFn = table.load(as: UInt32.self) >= 144
            ? table.load(fromByteOffset: 136, as: UnsafeRawPointer?.self).map { unsafeBitCast($0, to: PictureFn.self) } : nil
        let surfaces = table.load(as: UInt32.self) >= 160
        surfaceHasFn = surfaces ? table.load(fromByteOffset: 144, as: UnsafeRawPointer?.self).map { unsafeBitCast($0, to: SurfaceHasFn.self) } : nil
        surfaceSensorFn = surfaces ? table.load(fromByteOffset: 152, as: UnsafeRawPointer?.self).map { unsafeBitCast($0, to: SurfaceSensorFn.self) } : nil
        invokeFn = table.load(as: UInt32.self) >= 168
            ? table.load(fromByteOffset: 160, as: UnsafeRawPointer?.self).map { unsafeBitCast($0, to: Invocations.InvokeFn.self) } : nil
        arrangementFn = table.load(as: UInt32.self) >= 176
            ? table.load(fromByteOffset: 168, as: UnsafeRawPointer?.self).map { unsafeBitCast($0, to: ArrangementFn.self) } : nil
    }

    /// A package picture changed (0) or revoked (1): the host's raster loader acts on it.
    func picture(_ kind: UInt32, _ source: String) {
        let bytes = Array(source.utf8)
        bytes.withUnsafeBufferPointer { pictureFn?(host, kind, $0.baseAddress, UInt32($0.count)) }
    }

    /// Whether the session places a canvas of surface `name`; nil when the host feeds no canvas input.
    func hasSurface(_ name: String) -> Bool? {
        guard let surfaceHasFn else { return nil }
        let bytes = Array(name.utf8)
        return bytes.withUnsafeBufferPointer { surfaceHasFn(host, $0.baseAddress, UInt32($0.count)) } == 1
    }

    /// A sample written into surface `surface`'s canvas input, or its end.
    func sensor(_ surface: String, _ sensor: String, _ values: [Double], dropped: Int, at: Double, ended: Bool) {
        guard let surfaceSensorFn else { return }
        let s = Array(surface.utf8), n = Array(sensor.utf8)
        s.withUnsafeBufferPointer { s in
            n.withUnsafeBufferPointer { n in
                values.withUnsafeBufferPointer { v in
                    surfaceSensorFn(host, s.baseAddress, UInt32(s.count), n.baseAddress, UInt32(n.count), v.baseAddress, UInt32(v.count),
                                    UInt32(clamping: dropped), at, ended ? 1 : 0)
                }
            }
        }
    }

    func onLine() -> Bool? { onLineFn.flatMap { f in let answer = f(host); return answer > 1 ? nil : answer == 1 } }

    func log(_ line: String) {
        let bytes = Array(line.utf8)
        bytes.withUnsafeBufferPointer { logFn(host, $0.baseAddress, UInt32($0.count)) }
    }

    /// A region (§3.4): `what`, bound weakly to `object` (kind 0 a view, 1 a
    /// recognizer) or to nothing (2, an appearance), for the scope named.
    func owns(scope: String, node: UInt32, kind: UInt32, object: AnyObject?, _ what: String, surface: Bool) {
        guard let ownsFn else { return }
        let s = Array(scope.utf8), w = Array(what.utf8)
        _ = s.withUnsafeBufferPointer { s in
            w.withUnsafeBufferPointer { w in
                ownsFn(host, s.baseAddress, UInt32(s.count), node, kind, object.map { Unmanaged.passUnretained($0).toOpaque() }, w.baseAddress, UInt32(w.count), surface ? 1 : 0)
            }
        }
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
    /// Whether the stack's routes may show UIKit's bar. Exact's default is
    /// true, and each route then decides for itself (LLP 1115.000 D5.3: a
    /// route whose header is the bar shows it, whatever the stack's first
    /// route is); false, set in `navigation`, and no route of the stack
    /// shows one. Under the agent the authored header paints and the bar
    /// stays hidden whatever this says.
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
    /// The route's content scroll view: the one it names with
    /// `navigationScroll`, else its first that scrolls vertically (LLP
    /// 1115.000 D11.1). Its offset, insets, size and delegate are Exact's
    /// (LLP 1075.003 §3.5).
    public internal(set) weak var contentScrollView: UIScrollView?
    /// Whether this call is the controller's first.
    public internal(set) var isNew: Bool
    /// False once `routeEnded` has run: the handle then does nothing.
    public internal(set) var isLive = true
    weak var hatches: ExactHatches?
    init(key: String, controller: UIViewController, data: ExactData, hatches: ExactHatches) {
        self.key = key; self.controller = controller; self.data = data; isNew = true; self.hatches = hatches
    }

    /// Tell the agent what this hatch added to the route (§3.4): a title
    /// view, a recognizer. It ends when the object goes or the route does.
    public func owns(view: UIView, _ what: String, surface: Bool = false) {
        if isLive { hatches?.owns(scope: "route \(key)", node: 0, kind: 0, object: view, what, surface: surface) }
    }
    public func owns(recognizer: UIGestureRecognizer, _ what: String, surface: Bool = false) {
        if isLive { hatches?.owns(scope: "route \(key)", node: 0, kind: 1, object: recognizer, what, surface: surface) }
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
    /// The navigation controller Exact keeps the tab's stack in. A tab whose
    /// stack shows no bar and has pushed nothing has none until this is
    /// read: reading it builds it, and the tab keeps it.
    public var controller: UINavigationController {
        if let nav = shown as? UINavigationController { return nav }
        guard let hatches, let stack = hatches.stackFn, let built = stack(hatches.host, Unmanaged.passUnretained(shown).toOpaque()) else {
            preconditionFailure("tab \(name): the host built no navigation controller")
        }
        return Unmanaged<UINavigationController>.fromOpaque(built).takeUnretainedValue()
    }
    /// The tab's item, which Exact updates in place as the authored tab changes.
    public var item: UITabBarItem { shown.tabBarItem }
    /// What Exact shows the tab in: its navigation controller, or the plain
    /// container of a tab that has none yet.
    let shown: UIViewController
    weak var hatches: ExactHatches?
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
    /// rest is yours. A video, frame or native view in a list row, and on
    /// macOS any frame, may be made after `built`: the hatch hears `changed`
    /// once it is there.
    public internal(set) weak var platform: AnyObject?
    /// Whether this call is the node's first `element(_:)`.
    public internal(set) var isNew = true
    /// `element(_:)` has been called for it.
    var told = false
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

    private var scopeName: String { hatch == nil ? "route \(key)" : "element \(key)" }

    /// Tell the agent what this hatch added (LLP 1075.003.000.001 §3.4): a
    /// sentence, bound weakly to the view or recognizer. `tree` shows it
    /// under the node with what the host observes of the object (whether it
    /// is live, its frame, its class), and its interior is counted as this
    /// region's. It ends when the object goes or the node does.
    public func owns(view: ExactPlatformView, _ what: String, surface: Bool = false) {
        if isLive { hatches?.owns(scope: scopeName, node: node, kind: 0, object: view, what, surface: surface) }
    }
    public func owns(recognizer: ExactPlatformRecognizer, _ what: String, surface: Bool = false) {
        if isLive { hatches?.owns(scope: scopeName, node: node, kind: 1, object: recognizer, what, surface: surface) }
    }

    #if os(iOS)
    /// Lay out this element's two authored children with UIKit, preserving their content.
    /// Pass nil to restore their authored layout. Navigation stays outside the arrangement.
    @available(iOS 27.1, *)
    @discardableResult
    public func arrange(using controller: UIArrangementViewController?) -> Bool {
        guard isLive, let hatches, let arrange = hatches.arrangementFn else { return false }
        return arrange(hatches.host, node, controller.map { Unmanaged.passUnretained($0).toOpaque() }) == 0
    }
    #endif

    /// The controls this hatch drew in the node (§3.5), at most 32. Setting
    /// the list replaces it; one that breaks a bound is refused whole, by
    /// name, in the journal, and the list stays as it was.
    public var parts: [ExactPart] = [] {
        didSet {
            guard !settingParts, isLive, let hatches, let partsFn = hatches.partsFn else { return }
            let rows = parts.map { ["id": $0.id, "role": $0.role, "label": $0.label] }
            let json = (try? JSONSerialization.data(withJSONObject: rows)) ?? Data("[]".utf8)
            var views: [UnsafeMutableRawPointer?] = parts.map { part in part.view.map { Unmanaged.passUnretained($0).toOpaque() } }
            let refused = json.withUnsafeBytes { j in
                views.withUnsafeMutableBufferPointer { v in
                    partsFn(hatches.host, node, j.bindMemory(to: UInt8.self).baseAddress, UInt32(json.count), UnsafePointer(v.baseAddress), UInt32(v.count))
                }
            } != 0
            if refused { settingParts = true; parts = oldValue; settingParts = false }
        }
    }
    private var settingParts = false

    /// Replace an authored text field's whole value, as a person's typing
    /// would leave it (LLP 1075.003.000.001 §2.5): cut to the field's own
    /// limits, heard by its `input` and `change` handlers, without moving
    /// focus, so a native search controller can feed an authored field while
    /// it keeps the keyboard. Queued like `click()`. Refused by name, in the
    /// journal, for a node that is not an editable text field, a disabled or
    /// readonly one, one that is composing, and text over 64 KB. The journal
    /// records the length, never the text.
    public func input(_ text: String) {
        guard let hatches else { return }
        let what = hatch == nil ? "route \(key)" : "element \(key)"
        guard isLive, let inputFn = hatches.inputFn else { return hatches.log("\(what): input() on #\(id.isEmpty ? String(node) : id) refused") }
        let bytes = Array(text.utf8)
        _ = bytes.withUnsafeBufferPointer { inputFn(hatches.host, node, $0.baseAddress, UInt32($0.count)) }
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

private let moduleCreate: @convention(c) (UnsafePointer<UInt8>?, UInt32, UnsafeMutableRawPointer?, ExactModuleChangedFn?, ExactModuleNowFn?, UnsafeMutablePointer<UInt8>?, UInt32) -> UnsafeMutableRawPointer? = { json, length, host, changed, now, out, capacity in
    guard let changed, let now else { ExactNativeViewABI.write("no host callbacks", out, capacity); return nil }
    let data = json.map { Data(bytes: $0, count: Int(length)) } ?? Data()
    let object = (try? JSONSerialization.jsonObject(with: data) as? [String: Any]) ?? [:]
    let instance = exactModule.init(context: ExactModuleContext(json: object, host: host, changed: changed, now: now))
    ExactExtensions.created(instance)
    return Unmanaged.passRetained(instance).toOpaque()
}

private let moduleDestroy: @convention(c) (UnsafeMutableRawPointer?) -> Void = { raw in
    guard let raw else { return }
    let m = Unmanaged<ExactModule>.fromOpaque(raw)
    ExactExtensions.destroy(m.takeUnretainedValue())
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
    if !ExactExtensions.later(m, request, reply: reply) { m.later(request, reply: reply) }
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
        let value = try ExactExtensions.call(m, request) ?? m.call(request)
        guard JSONSerialization.isValidJSONObject(value) else { return answer(500, "the reply was not JSON") }
        answer(200, String(decoding: try JSONSerialization.data(withJSONObject: value), as: UTF8.self))
    } catch {
        answer(500, String(describing: error))
    }
}

private func module(_ raw: UnsafeMutableRawPointer?) -> ExactModule? {
    raw.map { Unmanaged<ExactModule>.fromOpaque($0).takeUnretainedValue() }
}

/// `done(ctx)` exactly once: when the module calls it, or when it lets the
/// closure go without calling it.
private final class QuitDone: @unchecked Sendable {
    private let lock = NSLock()
    private var called = false
    private let ctx: UnsafeMutableRawPointer?
    private let fn: @convention(c) (UnsafeMutableRawPointer?) -> Void
    init(ctx: UnsafeMutableRawPointer?, fn: @escaping @convention(c) (UnsafeMutableRawPointer?) -> Void) { self.ctx = ctx; self.fn = fn }
    func call() {
        let first = lock.withLock { () -> Bool in
            defer { called = true }
            return !called
        }
        if first { fn(ctx) }
    }
    deinit { call() }
}

/// `will_quit(module, ctx, done)`: the quit's work (macOS); `done(ctx)` once, any thread.
private let moduleWillQuit: @convention(c) (UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, (@convention(c) (UnsafeMutableRawPointer?) -> Void)?) -> Void = { raw, ctx, fn in
    guard let fn else { return }
    let done = QuitDone(ctx: ctx, fn: fn)
    guard let m = module(raw) else { return done.call() }
    m.willQuit { done.call() }
}

// The hatches (LLP 1075.003 §3.2): the host connects its callbacks once,
// after the session's module is made; then each moment is one call.
private let moduleConnect: @convention(c) (UnsafeMutableRawPointer?, UnsafeRawPointer?) -> Void = { raw, table in
    guard let m = module(raw), let table else { return }
    m.hatches = ExactHatches(host: m.context.host, table: table)
    m.context.diagnostics.recordFn = m.hatches?.recordFn
    m.context.framesFn = m.hatches?.framesFn
    m.context.afterFn = m.hatches?.afterFn
}

/// `tamber(module)`: a batch moved what a binding reads (LLP 1121 D12); each
/// place is asked again, and each that moved redrawn (ExactTamber.swift).
private let moduleTamber: @convention(c) (UnsafeMutableRawPointer?) -> Void = { raw in
    guard module(raw) != nil else { return }
    Tambers.moved()
}

/// `tick(module, kind, token, now, seq)`: a frame ticket's tick (0) or an
/// `after` that came due (1). A token the module no longer holds is one it
/// stopped inside an earlier callback of this instant: nothing runs.
private let moduleTick: @convention(c) (UnsafeMutableRawPointer?, UInt32, UInt64, Double, UInt64) -> Void = { raw, kind, token, now, seq in
    guard let m = module(raw) else { return }
    if kind == 0 { m.context.ticks[token]?(ExactFrame(now: now, seq: seq)) }
    else { m.context.afters.removeValue(forKey: token)?() }
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
/// tabs (`{"names": […], "nodes": […], "selected": i}`) and their controllers
/// (a navigation controller, or a held tab's container, `heldStacks`); a
/// container the app owns, retained once for the host, or nil.
private let moduleTabContainer: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UnsafePointer<UnsafeMutableRawPointer?>?, UInt32) -> UnsafeMutableRawPointer? = { raw, json, length, controllers, count in
    #if os(iOS) || os(tvOS)
    guard let m = module(raw), let hatches = m.hatches, let json, let controllers,
          let object = try? JSONSerialization.jsonObject(with: Data(bytes: json, count: Int(length))) as? [String: Any],
          let names = object["names"] as? [String], let nodes = object["nodes"] as? [NSNumber], names.count == Int(count) else { return nil }
    let shown = (0..<Int(count)).compactMap { controllers[$0].map { Unmanaged<UIViewController>.fromOpaque($0).takeUnretainedValue() } }
    guard shown.count == names.count else { return nil }
    let tabs = zip(names, shown).map { ExactTab(name: $0, shown: $1, hatches: hatches) }
    let contents = ExactTabContents(tabs: tabs, selected: object["selected"] as? Int ?? 0, tabNodes: nodes.map(\.uint32Value), hatches: hatches)
    guard let container = m.tabContainer(contents) else { return nil }
    hatches.contents = contents
    return Unmanaged.passRetained(container).toOpaque()
    #else
    return nil
    #endif
}

/// `element(module, event, view, platform, json, len) → flags`
/// (LLP 1075.003.000): event 0 built, 1 changed, 2 ended, 3 a frame's
/// configuration before its web view (the platform object, LLP 1020.002
/// §9); json {"hatch", "node", "id", "kind", "data"}; bit 0 of the flags:
/// `reusable`.
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
        element.isNew = !known.told
    } else {
        element = ExactElement(hatch: ExactHatchKey(word), id: object["id"] as? String ?? "", node: node, hatches: hatches)
        hatches.elements[node] = element
    }
    element.data = ExactData(object["data"] as? [String: String] ?? [:])
    element.view = view.map { Unmanaged<ExactNativeView>.fromOpaque($0).takeUnretainedValue() }
    // A hatched node's view is a place a binding finds, as a module view is (LLP 1121 D12).
    if let view = element.view { Tambers.made(view, by: m) }
    if event == 3 {
        if let platform { m.elementConfiguration(element, Unmanaged<AnyObject>.fromOpaque(platform).takeUnretainedValue()) }
        return 0
    }
    element.platform = platform.map { Unmanaged<AnyObject>.fromOpaque($0).takeUnretainedValue() }
    element.told = true
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

/// `app(module, event, application, json, len)`: event 0 built, 1 changed, 2
/// ended; the application only for the process's owner.
private let moduleApp: @convention(c) (UnsafeMutableRawPointer?, UInt32, UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32) -> Void = { raw, event, application, json, length in
    guard let m = module(raw), let hatches = m.hatches, let json,
          let fields = try? JSONSerialization.jsonObject(with: Data(bytes: json, count: Int(length))) as? [String: Any] else { return }
    let app = hatches.app ?? ExactApp()
    let isNew = hatches.app == nil
    app.hatches = hatches
    app.read(fields)
    #if os(macOS)
    app.application = application.map { Unmanaged<NSApplication>.fromOpaque($0).takeUnretainedValue() }
    #else
    app.application = application.map { Unmanaged<UIApplication>.fromOpaque($0).takeUnretainedValue() }
    #endif
    if event == 2 {
        hatches.app = nil
        guard !isNew else { return }
        m.appEnded(app)
        app.isLive = false
        app.application = nil
        return
    }
    hatches.app = app
    app.isNew = isNew
    m.app(app)
}

/// `window(module, event, window, scene, json, len)`: event 0 built, 1
/// changed, 2 ended; the window and scene only when it is the session's own.
private let moduleWindow: @convention(c) (UnsafeMutableRawPointer?, UInt32, UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32) -> Void = { raw, event, window, scene, json, length in
    guard let m = module(raw), let hatches = m.hatches, let json,
          let fields = try? JSONSerialization.jsonObject(with: Data(bytes: json, count: Int(length))) as? [String: Any] else { return }
    // A new surface is a new handle: one from before never reaches the next window.
    let handle = event == 0 ? ExactWindow() : (hatches.window ?? ExactWindow())
    handle.hatches = hatches
    let isNew = event == 0 || hatches.window == nil
    let angle = hatches.window?.angle
    handle.read(fields)
    #if os(macOS)
    handle.window = window.map { Unmanaged<NSWindow>.fromOpaque($0).takeUnretainedValue() }
    #else
    handle.window = window.map { Unmanaged<UIWindow>.fromOpaque($0).takeUnretainedValue() }
    handle.scene = scene.map { Unmanaged<UIWindowScene>.fromOpaque($0).takeUnretainedValue() }
    #endif
    if event == 2 {
        hatches.window = nil
        guard !isNew else { return }
        m.windowEnded(handle)
        handle.isLive = false
        handle.window = nil
        return
    }
    hatches.window = handle
    handle.isNew = isNew
    m.window(handle)
    if angle != handle.angle { ExactExtensions.orientation(m, handle.angle) }
}


/// `authenticate(module, json, len, accessControl, ctx, done)` (LLP 1133.010
/// D13): the person's check for a presence item, by the extension whose
/// interface declares the authenticator entry; json {"reason", "policy",
/// "origin"}, the access control borrowed. done(ctx, context, error, len)
/// once, from any thread: the evaluated context retained once for the host,
/// or nil and an error's JSON.
private let moduleAuthenticate: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, Int, UnsafeMutableRawPointer?, UnsafeMutableRawPointer?,
                                                (@convention(c) (UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, Int) -> Void)?) -> Void = { raw, json, length, control, ctx, done in
    guard let done else { return }
    let fail = { (error: [String: Any]) in
        let data = (try? JSONSerialization.data(withJSONObject: error)) ?? Data()
        data.withUnsafeBytes { done(ctx, nil, $0.bindMemory(to: UInt8.self).baseAddress, data.count) }
    }
    guard let m = module(raw), let json, let control,
          let request = try? JSONSerialization.jsonObject(with: Data(bytes: json, count: length)) as? [String: Any]
    else { return fail(["error": "NotSupportedError", "message": "an unreadable presence check"]) }
    let access = Unmanaged<SecAccessControl>.fromOpaque(control).takeUnretainedValue()
    ExactExtensions.authenticate(m, request, access) { context, error in
        guard let context else { return fail(error ?? ["error": "UnknownError"]) }
        done(ctx, Unmanaged.passRetained(context).toOpaque(), nil, 0)
    }
}

/// `image(module, json, len, ctx, done)` (LLP 1133.019 D11 item 1): a
/// picture of a package's `img` scheme, which the runner admitted; json
/// {"extension", "source", "width", "height", "fit", "owned"}. done(ctx,
/// image, error, len) once, from any thread: the `CGImage` retained once for
/// the host, or nil and an error's code.
private let moduleImage: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, Int, UnsafeMutableRawPointer?,
                                         (@convention(c) (UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, Int) -> Void)?) -> Void = { raw, json, length, ctx, done in
    guard let done else { return }
    let fail = { (code: String) in
        let bytes = Array(code.utf8)
        bytes.withUnsafeBufferPointer { done(ctx, nil, $0.baseAddress, $0.count) }
    }
    guard let m = module(raw), let json,
          let request = try? JSONSerialization.jsonObject(with: Data(bytes: json, count: length)) as? [String: Any]
    else { return fail("NotSupportedError") }
    ExactExtensions.image(m, request) { image, error in
        guard let image else { return fail(error ?? "UnknownError") }
        done(ctx, Unmanaged.passRetained(image).toOpaque(), nil, 0)
    }
}

/// `session_view(module, view)` (LLP 1133.027 D17 item 3): the session's
/// root view, borrowed, or nil when none is mounted; packages host views
/// under it and present in its window. Main thread.
private let moduleSessionView: @convention(c) (UnsafeMutableRawPointer?, UnsafeMutableRawPointer?) -> Void = { raw, view in
    module(raw)?.context.sessionView = view.map { Unmanaged<AnyObject>.fromOpaque($0).takeUnretainedValue() }
}

/// The ABI major this artifact was built against; the host refuses others.
private let major: UInt32 = 3

private let table: UnsafeMutableRawPointer = {
    // Beside the tags, under a key no tag can take (a tag has a hyphen): the
    // hatch words this module was built to handle (LLP 1075.003.000.001
    // §4.3), so the host calls it with no word its code never compiled.
    // `heldStacks`: `tabContainer` takes a tab Exact holds without a
    // navigation controller, whose `ExactTab.controller` builds it.
    let words = "\"hatches\":{\"heldStacks\":true,\"words\":[" + ExactHatchKey._words.map { "\"\($0)\"" }.joined(separator: ",") + "]}"
        + ",\"extensions\":{\"names\":[" + ExactExtensions.names.map { "\"\($0)\"" }.joined(separator: ",") + "]}"
        // `app.onLine` is asked of the host (`on_line`), not read from the facts.
        + ",\"facts\":{\"onLine\":\"asks\"}"
    let roster = ExactNativeViewABI.roster
    let text = "{" + (roster.keys.sorted().map { tag in
        "\"\(tag)\":{\"snapshot\":\(roster[tag]!.snapshot),\"reuse\":\(roster[tag]!.reuse),\"firstFrame\":\(roster[tag]!.firstFrame)}"
    } + [words]).joined(separator: ",") + "}"
    let size = 256
    let t = UnsafeMutableRawPointer.allocate(byteCount: size, alignment: 8)
    t.initializeMemory(as: UInt8.self, repeating: 0, count: size)
    t.storeBytes(of: major, as: UInt32.self)
    t.storeBytes(of: UInt32(size), toByteOffset: 4, as: UInt32.self)
    t.storeBytes(of: UnsafeRawPointer(strdup(text)), toByteOffset: 8, as: UnsafeRawPointer?.self)
    t.storeBytes(of: unsafeBitCast(ExactNativeViewABI.create, to: UnsafeRawPointer.self), toByteOffset: 16, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(ExactNativeViewABI.platformView, to: UnsafeRawPointer.self), toByteOffset: 24, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(ExactNativeViewABI.setProps, to: UnsafeRawPointer.self), toByteOffset: 32, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(ExactNativeViewABI.snapshot, to: UnsafeRawPointer.self), toByteOffset: 40, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(ExactNativeViewABI.destroy, to: UnsafeRawPointer.self), toByteOffset: 48, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(ExactNativeViewABI.agentInput, to: UnsafeRawPointer.self), toByteOffset: 64, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(ExactNativeViewABI.focusTarget, to: UnsafeRawPointer.self), toByteOffset: 112, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleCreate, to: UnsafeRawPointer.self), toByteOffset: 72, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleDestroy, to: UnsafeRawPointer.self), toByteOffset: 80, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleLater, to: UnsafeRawPointer.self), toByteOffset: 88, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleCall, to: UnsafeRawPointer.self), toByteOffset: 96, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(ExactNativeViewABI.prepareForReuse, to: UnsafeRawPointer.self), toByteOffset: 104, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleConnect, to: UnsafeRawPointer.self), toByteOffset: 120, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleNavigation, to: UnsafeRawPointer.self), toByteOffset: 128, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleRoute, to: UnsafeRawPointer.self), toByteOffset: 136, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleTabs, to: UnsafeRawPointer.self), toByteOffset: 144, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleTabContainer, to: UnsafeRawPointer.self), toByteOffset: 152, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(ExactNativeViewABI.platformController, to: UnsafeRawPointer.self), toByteOffset: 160, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleElement, to: UnsafeRawPointer.self), toByteOffset: 168, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleToolbar, to: UnsafeRawPointer.self), toByteOffset: 176, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleApp, to: UnsafeRawPointer.self), toByteOffset: 184, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleWindow, to: UnsafeRawPointer.self), toByteOffset: 192, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleTick, to: UnsafeRawPointer.self), toByteOffset: 200, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleWillQuit, to: UnsafeRawPointer.self), toByteOffset: 208, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleTamber, to: UnsafeRawPointer.self), toByteOffset: 216, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(ExactNativeViewABI.createView, to: UnsafeRawPointer.self), toByteOffset: 224, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleAuthenticate, to: UnsafeRawPointer.self), toByteOffset: 232, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleImage, to: UnsafeRawPointer.self), toByteOffset: 240, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(moduleSessionView, to: UnsafeRawPointer.self), toByteOffset: 248, as: UnsafeRawPointer.self)
    return t
}()

@_cdecl("exact_native_abi")
public func exactNativeAbi() -> UnsafeRawPointer { UnsafeRawPointer(table) }
