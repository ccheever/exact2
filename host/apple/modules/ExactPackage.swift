// What a package's Swift implementation of a module sees of exact2 (LLP
// 1133.001 D5; LLP 1133.009 D10 items 1-3; LLP 1133.008 D8 item 9): the
// `ExactPackage` module, compiled on its own before the packages, which
// import it, and linked into the app's one module, whose adapter for each
// package (`exactExtensions`, written by host/apple/build.mjs) holds one
// instance per session. Nothing of ExactKit or of the app's own module is
// reachable from here.
//
// Every entry is on the main thread (LLP 1067.000 D4).
import AVFoundation
import Foundation
import Security
#if os(macOS)
import AppKit
/// The platform's view a package's view puts in its node's box.
public typealias ExactPackagePlatformView = NSView
#else
import UIKit
/// The platform's view a package's view puts in its node's box.
public typealias ExactPackagePlatformView = UIView
#endif

/// What a package refuses, said by name (a view it does not serve, props it cannot take).
public struct ExactPackageRefusal: Error, CustomStringConvertible {
    public let description: String
    public init(_ description: String) { self.description = description }
}

/// The events a package's view sends its element, which Contract's handlers
/// take (`message=`, `load=`, …), and its natural size. Any thread.
public final class ExactPackageEvents {
    private let send: (UInt32, String) -> Void
    private let size: (CGSize?) -> Void
    public init(send: @escaping (UInt32, String) -> Void, size: @escaping (CGSize?) -> Void) { self.send = send; self.size = size }
    public func press() { send(0, "") }
    public func change(_ value: String) { send(1, value) }
    public func hover(_ over: Bool) { send(2, over ? "true" : "false") }
    public func focus() { send(3, "") }
    public func blur() { send(4, "") }
    public func key(_ name: String) { send(5, name) }
    public func submit() { send(6, "") }
    public func load() { send(7, "") }
    public func message(_ text: String) { send(8, text) }
    /// An event the view declares beyond the nine (LLP 1133.021 §9 item 2),
    /// `args` in the declared types' JSON: the handler named for it runs
    /// with them decoded. Sent as a message the runner recognises.
    public func emit(_ name: String, _ args: [Any]) {
        let body: [String: Any] = ["exact:event": name, "args": args]
        guard let data = try? JSONSerialization.data(withJSONObject: body), let text = String(data: data, encoding: .utf8) else { return }
        send(8, text)
    }
    /// Preferred content size in points; an authored size wins; nil forgets it.
    public func intrinsicSize(_ s: CGSize?) { size(s) }
}

/// One instance of a view a package serves (LLP 1133.001 D1), made by the
/// session's implementation (`ExactPackage.view`).
public protocol ExactPackageView: AnyObject {
    /// The view the host puts in the node's box; it fills the box.
    var view: ExactPackagePlatformView { get }
    /// Every prop, as strings, replaced on each change; throw to refuse them.
    func setProps(_ props: [String: String]) throws
    /// Last call: the node is gone.
    func destroy()
}

public extension ExactPackageView {
    func destroy() {}
}

/// The session an implementation instance belongs to.
public final class ExactPackageContext {
    /// The session's track table, shared by its implementations (LLP
    /// 1133.009 D10 item 6).
    public let tracks: ExactTracks
    /// The process's audio session, which the host owns (D7, D10 item 5).
    public let audioSession: ExactAudioSession
    private let fileFn: (String) -> URL?
    /// Under the agent, unless the drive bound the implementation as in
    /// production (`--bind`): an implementation that is its own substitute
    /// then opens no device and asks the OS for nothing (LLP 1133.006 D8).
    public let agent: Bool
    /// The session's id, unique in the process: what a process object
    /// shared by sessions (a capture, a photo output) keys its consumers by.
    public let session: String
    /// The host's own id for the session, which what the host keeps for it
    /// is named by: under the agent, the private pasteboard its `copyText`
    /// and @exact/clipboard write (LLP 1133.006 §7).
    public let hostSession: String
    /// Who made the call being answered now (`app`, `tool`, `wake`): set by
    /// the host before each `later`, read by an implementation at its entry,
    /// as a capture's first lease is refused to a tool's read (LLP 1133.009 D6).
    public var origin = "app"
    /// The runner's number for the send being answered now, set beside
    /// `origin`: the one `cancel(_:)` names when the call is let go (LLP
    /// 1133.016 D15 item 2). Nil for a read.
    public var request: Int?
    /// Whether the app is in the foreground, as the runner's admission reads
    /// it; the host sets it before `foreground(_:)` is told each change.
    public var foreground = true
    /// The session's canvases, whose input an implementation may feed (LLP
    /// 1133.013 D6).
    public let surfaces: ExactSurfaces
    private let changedFn: (String) -> Void
    private let logFn: (String) -> Void
    private let nowFn: () -> Double
    private let pictureFn: (UInt32, String) -> Void
    private let orientationFn: () -> Double
    private let viewFn: () -> ExactPackagePlatformView?
    private let invokeFn: (String, String, [String: Any], @escaping ([String: Any]) -> Void) -> Void
    private let emitFn: (String, [Any]) -> Void
    private let stateFn: ([String: Any]) -> Void
    private let activationFn: (String, [Any], String?) -> [AnyHashable: Any]
    private let wakeFn: (String, [Any], String, @escaping (String) -> Void) -> Void
    private let expireWakeFn: (String) -> Void
    /// The host's wake requests (LLP 1133.017 D8).
    public let wakes: ExactWakes
    /// The host's transfer carrier (LLP 1133.017 D8, D10).
    public let transfers: ExactTransfers
    /// The host's list of the app icon's shortcuts (LLP 1133.023 D15 item 2).
    public let shortcuts = ExactShortcutStore()
    /// The host's content index mirror (LLP 1133.023 D4, D15 item 2).
    public let contentIndex = ExactContentIndexStore()
    /// The process's pad slots (LLP 1133.022 D10 item 1); nil on a host
    /// older than them, where a pad module reads no pads (item 4).
    public let pads: ExactPads?
    private let afterFn: (Double, @escaping () -> Void) -> () -> Void
    /// The host's region carrier (LLP 1133.021 D14).
    public let regions: ExactRegions

    public init(agent: Bool, session: String, hostSession: String = "", changed: @escaping (String) -> Void,
                log: @escaping (String) -> Void, now: @escaping () -> Double,
                tracks: ExactTracks = ExactTracks(), file: @escaping (String) -> URL? = { _ in nil },
                audioSession: ExactAudioSession = ExactAudioSession { _, _, _, _ in nil },
                picture: @escaping (UInt32, String) -> Void = { _, _ in },
                surfaces: ExactSurfaces = ExactSurfaces { _ in nil }, orientation: @escaping () -> Double = { 0 },
                view: @escaping () -> ExactPackagePlatformView? = { nil },
                invoke: @escaping (String, String, [String: Any], @escaping ([String: Any]) -> Void) -> Void = { _, _, _, reply in reply(["error": "NotSupportedError"]) },
                emit: @escaping (String, [Any]) -> Void = { _, _ in },
                activation: @escaping (String, [Any], String?) -> [AnyHashable: Any] = { _, _, _ in [:] },
                wake: @escaping (String, [Any], String, @escaping (String) -> Void) -> Void = { _, _, _, _ in },
                wakes: ExactWakes = ExactWakes { _, _ in },
                transfers: ExactTransfers = ExactTransfers { _, _ in nil },
                pads: ExactPads? = nil,
                after: @escaping (Double, @escaping () -> Void) -> () -> Void = { ms, body in
                    let work = DispatchWorkItem(block: body)
                    DispatchQueue.main.asyncAfter(deadline: .now() + ms / 1000, execute: work)
                    return work.cancel
                },
                regions: ExactRegions = ExactRegions { _, _ in nil },
                expireWake: @escaping (String) -> Void = { _ in },
                state: @escaping ([String: Any]) -> Void = { _ in }) {
        self.agent = agent; self.session = session; self.hostSession = hostSession
        changedFn = changed; logFn = log; nowFn = now
        self.tracks = tracks; fileFn = file; self.audioSession = audioSession; pictureFn = picture
        self.surfaces = surfaces; orientationFn = orientation; viewFn = view; invokeFn = invoke
        emitFn = emit; activationFn = activation; wakeFn = wake; self.wakes = wakes; self.transfers = transfers
        self.pads = pads; afterFn = after; self.regions = regions; expireWakeFn = expireWake; stateFn = state
    }

    /// `body` once, `ms` later on the session's clock: the agent's under the
    /// agent, where a seek runs it at its own instant, so a drive repeats.
    /// Call what it returns to cancel it first. Main thread.
    @discardableResult
    public func after(_ ms: Double, _ body: @escaping () -> Void) -> () -> Void { afterFn(ms, body) }

    /// Raise the interface's wake event `name` with `args` for the platform
    /// object `id` (LLP 1133.017 D8): the events of one id are one wake, run
    /// with the wake's origin, and `ended` hears its outcome once:
    /// `completed`, `expired`, `superseded`, `refused`, or `unheard` when no
    /// handler hears the event. `ExactWake.raise` names the id itself.
    /// Main thread.
    public func wake(_ name: String, _ args: [Any], id: String, ended: @escaping (String) -> Void = { _ in }) {
        wakeFn(name, args, id, ended)
    }

    /// The platform expired the wake raised as `id` before its budget ran
    /// out: it expires now, or ends `expired` unrun if it had not begun, and
    /// its `ended` hears `expired`. `ExactWake` says this itself when the
    /// host's task expires. Main thread.
    public func expireWake(id: String) { expireWakeFn(id) }

    /// Raise the interface's event `name` with `args` (LLP 1133.004 D6):
    /// delivered in order, once, with the device's origin. Any thread.
    public func emit(_ name: String, _ args: [Any]) { emitFn(name, args) }

    /// The `userInfo` entry for a notification the implementation posts
    /// whose opening raises event `name` with `args`, or opens `navigate`
    /// (LLP 1133.004 D6, D7): the host's notification delegate reads it and
    /// raises the event, a person's activation, on the session that posted it.
    public func activation(_ name: String, _ args: [Any], navigate: String? = nil) -> [AnyHashable: Any] {
        activationFn(name, args, navigate)
    }

    /// The app's handler `handler` run for the work `handle` names (LLP
    /// 1133.016 D15 item 1): one a call that made the handle listed, run by
    /// the runner as a fresh answer whose origin is at most `tool`. `reply`
    /// gets `{"text": JSON}` or `{"error", "message"}`, on the main thread.
    public func invoke(_ handle: String, _ handler: String, _ args: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        invokeFn(handle, handler, args, reply)
    }

    /// A picture of the interface's `img` scheme changed (LLP 1133.019 D11
    /// item 1): the host loads `source` again wherever it is shown.
    public func pictureChanged(_ source: String) { pictureFn(0, source) }
    /// A picture the app may no longer read (D2): the host clears it from
    /// every `img` that shows it and from its cache, cancels its loads and
    /// drops a later reply to them; each such `img` fires `error`. A load
    /// after `pictureChanged(source)` asks again.
    public func revokePicture(_ source: String) { pictureFn(1, source) }


    /// The angle of the session's window, as the web's `screen.orientation.angle`
    /// (0, 90, 180, 270; LLP 1133.013 D12 item 4): the window scene's interface
    /// orientation on iOS, 0 on the Mac. `orientation(_:)` is told each change.
    public var orientation: Double { orientationFn() }

    /// The session's own root view (LLP 1133.027 D17 item 3), which a
    /// package's hosted views go under: in the sample host, its pane. Nil
    /// while the session's view is in no window. Main thread.
    public var view: ExactPackagePlatformView? {
        guard let view = viewFn(), view.window != nil else { return nil }
        return view
    }

    /// Where the session presents (LLP 1133.027 D17 item 3): its window, key
    /// or not, since a sheet for a window behind another belongs in that
    /// window; nil while the session's view is in no window. A package
    /// presents in its own session rather than the key window's.
    public var presentation: ExactPresentation? { view.flatMap(ExactPresentation.init) }

    /// The file an admitted `app:/` path names (LLP 1133.009 D10 item 4):
    /// the session's own directories, a scratch tree under the agent.
    /// Admission has checked the path against the calling source's grants.
    public func file(_ path: String) -> URL? { fileFn(path) }

    /// Under the agent, what the implementation discloses of its own objects
    /// for `state.extensions` (LLP 1133.008 D8 item 9): listed as its
    /// `implementation`, each field replacing the one of its name said
    /// before. Nothing outside the agent. Main thread.
    public func state(_ fields: [String: Any]) { stateFn(fields) }

    /// The readers of `topic` are asked again (LLP 1016.002).
    public func changed(_ topic: String) { changedFn(topic) }
    /// A journal line in a development build; nothing in a production one.
    public func log(_ line: String) { logFn(line) }
    /// The session's clock, ms: the agent's under the agent.
    public func now() -> Double { nowFn() }

    /// Whether the embedder lets the app leave for `url` (LLP 1133.024 D22
    /// item 7), `how` one of `browser` (a page shown over the app) and
    /// `external` (another app): asked before a page or a hand-off by URL
    /// opens, as `ExactSession.follow` hands a link to the delegate. The
    /// session's delegate decides (`ExactLeaveDelegate`); one that does not
    /// say lets it go. Main thread.
    public func leave(_ url: String, how: String) -> Bool {
        let ask = NSMutableDictionary(dictionary: ["url": url, "how": how])
        if let view = viewFn() { ask["view"] = view }
        NotificationCenter.default.post(name: Notification.Name("exact.leave"), object: ask)
        return ask["allow"] as? Bool ?? true
    }
}

/// The session's window (`ExactPackageContext.presentation`).
public struct ExactPresentation {
    #if os(macOS)
    /// The session's window. Session windows have no content view controller.
    public let window: NSWindow
    init?(_ view: NSView) {
        guard let window = view.window else { return nil }
        self.window = window
    }
    #else
    /// The session's window and its scene.
    public let window: UIWindow
    public let scene: UIWindowScene
    /// The window's root view controller, which a child view controller is added to.
    public let rootViewController: UIViewController
    init?(_ view: UIView) {
        guard let window = view.window, let scene = window.windowScene, let root = window.rootViewController else { return nil }
        self.window = window; self.scene = scene; rootViewController = root
    }
    #endif
}

/// A package's implementation class: what the adapter calls. Only `init`,
/// `later` and `call` are required; the rest do nothing unless the
/// implementation has something to do.
public protocol ExactPackage: AnyObject {
    init(context: ExactPackageContext)
    /// A source's read or a send, by its declared name, with the keyword
    /// arguments the call wrote; `reply` once. Under the agent, `preferred`
    /// is the agent's value for the name, which the runner also lays over
    /// the answer (LLP 1133.009 D10 item 11). An answer that is not an
    /// object (a list, a `bool`, or `NSNull()` for an absent `option`) is
    /// replied as `["exact:answer": value]`, a key no shape's field can take.
    func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void)
    /// A synchronous form, answered now (LLP 1133.002 D14).
    func call(_ name: String, _ args: [String: Any]) throws -> [String: Any]
    /// Live source `name` has no reader left: stop what it started (LLP 1133.002 D5).
    func release(_ name: String)
    /// No reader of live source `name` reads it with `args` any longer (LLP
    /// 1133.009 D10 item 12); a reader asked again has its new arguments
    /// first.
    func release(_ name: String, args: [String: Any])
    /// The agent's preferred values for this interface's names, by name,
    /// when bound under the agent and whenever one changes.
    func preferred(_ values: [String: Any])
    /// Handles a reply carried that the runner dropped: never the app's (LLP
    /// 1133.009 D10 item 10).
    func dispose(_ ids: [String])
    /// The send numbered `request` (`context.request` when it was asked) was
    /// let go: stop its work, whose reply no one reads (LLP 1133.016 D15 item 2).
    func cancel(_ request: Int)
    /// The app went to the foreground or left it.
    func foreground(_ shown: Bool)
    /// The session ends: finish what must be finished, then `done`; the
    /// host waits a second at most (LLP 1133.009 D10 item 1).
    func destroy(_ done: @escaping () -> Void)
    /// Make view `tag`, one of the package's `views`, for node `id` of this
    /// session: the id its `displayed` and `bound` reports name (LLP 1133.008
    /// D8 item 1). A report can come before the view is made, since the
    /// host attaches views after first pixel.
    func view(_ tag: String, id: String, props: [String: String], events: ExactPackageEvents) throws -> ExactPackageView
    /// View `id`'s displayed state changed, by the origin of the commit that
    /// changed it (`app`, `tool` or `wake`; LLP 1133.008 D8 item 2).
    func displayed(_ id: String, _ shown: Bool, origin: String)
    /// Prop `prop`, which the declaration marks as binding a device, changed
    /// on view `id`, by the origin of the commit that changed it; the value
    /// reaches the view as its props do.
    func bound(_ id: String, _ prop: String, origin: String)
    /// A picture of the interface's `img` scheme (LLP 1133.019 D2, D11 item
    /// 1), admitted by the runner: `source` drawn for a box of `width` by
    /// `height` device pixels at `fit` (`object-fit`'s word), upright.
    /// `owned`: let through as the app's own item only (D6). `reply` once,
    /// on any thread, with the image or an error's code.
    func image(_ source: String, width: Int, height: Int, fit: String, owned: Bool, reply: @escaping (CGImage?, String?) -> Void)
    /// The runner's permission states for this interface, by name, when
    /// bound under the agent and whenever one changes.
    func permissions(_ states: [String: String])
    /// The canvases of `surface`, which this implementation's input feeds,
    /// were displayed or hidden, by the origin of the commit that did it (LLP
    /// 1133.013 D6, D12 item 3). The lease: hidden, the input pauses; shown
    /// by an `app` commit, it resumes; shown by a `tool`'s or a `wake`'s, it
    /// stays paused. A start on a hidden canvas is told this before the start.
    func displayed(surface: String, _ shown: Bool, origin: String)
    /// The session window's angle changed (`ExactPackageContext.orientation`).
    func orientation(_ angle: Double)
    /// The host ran the app for this implementation's due wake requests
    /// (`context.wakes`, LLP 1133.017 D8): raise their events, then `done`.
    func woke(_ wake: ExactWake)
}

public extension ExactPackage {
    func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] { ["error": "NotSupportedError"] }
    func release(_ name: String) {}
    func release(_ name: String, args: [String: Any]) {}
    func preferred(_ values: [String: Any]) {}
    func dispose(_ ids: [String]) {}
    func cancel(_ request: Int) {}
    func foreground(_ shown: Bool) {}
    func destroy(_ done: @escaping () -> Void) { done() }
    func view(_ tag: String, id: String, props: [String: String], events: ExactPackageEvents) throws -> ExactPackageView {
        throw ExactPackageRefusal("\(type(of: self)) serves no view \(tag)")
    }
    func displayed(_ id: String, _ shown: Bool, origin: String) {}
    func bound(_ id: String, _ prop: String, origin: String) {}
    func image(_ source: String, width: Int, height: Int, fit: String, owned: Bool, reply: @escaping (CGImage?, String?) -> Void) {
        reply(nil, "NotSupportedError")
    }
    func permissions(_ states: [String: String]) {}
    func displayed(surface: String, _ shown: Bool, origin: String) {}
    func orientation(_ angle: Double) {}
    func woke(_ wake: ExactWake) { wake.done(success: true) }
}

// MARK: wakes (LLP 1133.017 D8)

/// The host's wake requests, kept in its defaults across launches. The
/// host keeps the platform's entry points, which must exist before any
/// package loads: on iOS one `BGAppRefreshTask` and the two
/// `BGProcessingTask` identifiers, each request submitted at the soonest
/// time asked of it; on the Mac a scheduler per request. A due request is
/// handed to the implementation that asked as `woke(_:)`, once.
public final class ExactWakes {
    private let requestFn: (String, [String: Any]?) -> Void
    public init(_ request: @escaping (String, [String: Any]?) -> Void) { requestFn = request }
    /// A wake for `key` at `at` or later (the session clock, ms): `refresh`
    /// (iOS's 30 s) or `processing` (minutes, while the device is idle),
    /// with a network connection and, if asked, external power. A request
    /// for a key replaces the last.
    public func request(kind: String, at: Double, key: String, network: Bool = true, power: Bool = false) {
        requestFn(key, ["kind": kind, "at": at, "network": network, "power": power])
    }
    /// No wake for `key`.
    public func cancel(key: String) { requestFn(key, nil) }
}

/// The app's own shortcuts on its icon's menu, after the manifest's (LLP
/// 1133.023 D3), which the host keeps and shows: iOS's quick actions, the
/// Mac's Dock menu. It survives a launch with no package loaded.
public final class ExactShortcutStore {
    /// The list replaced by `items`, each as the interface's `ShortcutItem`.
    public func set(_ items: [[String: Any]]) {
        NotificationCenter.default.post(name: Notification.Name("exact.shortcuts.set"), object: ["items": items] as NSDictionary)
    }
}

/// The content index's mirror, which the host keeps in the app's container
/// and tells Spotlight the difference of (LLP 1133.023 D4). Each entry is the
/// interface's `ContentDescription`. A change answers nil, or the web's
/// name for its refusal (`QuotaExceededError`, `TypeError`).
public final class ExactContentIndexStore {
    private func post(_ name: String, _ body: [String: Any]) -> NSMutableDictionary {
        let body = NSMutableDictionary(dictionary: body)
        NotificationCenter.default.post(name: Notification.Name("exact.contentIndex." + name), object: body)
        return body
    }
    /// The entries of `kind` replaced by `entries`, in their order.
    public func set(kind: String?, entries: [[String: Any]]) -> String? {
        post("set", ["kind": kind as Any, "entries": entries])["error"] as? String
    }
    public func add(_ entry: [String: Any]) -> String? { post("add", ["entry": entry])["error"] as? String }
    public func delete(id: String) { _ = post("delete", ["id": id]) }
    /// Every entry, in the order the app set them.
    public var entries: [[String: Any]] { post("entries", [:])["entries"] as? [[String: Any]] ?? [] }
}

/// The host's background `URLSession`s and their durable records (LLP
/// 1133.017 D10), which outlive the process: a registration's requests run
/// while the app is suspended or terminated, and when they have all ended
/// the host offers the registration to the implementation that started it
/// as `woke(_:)` of kind `transfer`, its key the registration's id, until
/// it is retired.
public final class ExactTransfers {
    private let callFn: (String, [String: Any]) -> Any?
    public init(_ call: @escaping (String, [String: Any]) -> Any?) { callFn = call }
    /// Start registration `id`: each request `{url, method, headers,
    /// bodyFrom, saveTo}`, `bodyFrom` a file path the host copies first,
    /// `saveTo` kept as given; past `downloadTotal` bytes received it fails.
    /// Nil, or the error's name (`TypeError` for an id in flight or a bad
    /// request). The host posts `exact.transfers.changed` `{package, id}` as
    /// its bytes move and when it ends.
    public func start(id: String, requests: [[String: Any]], discretionary: Bool = false, downloadTotal: Int64? = nil) -> String? {
        var args: [String: Any] = ["id": id, "requests": requests, "discretionary": discretionary]
        if let downloadTotal { args["downloadTotal"] = downloadTotal }
        return callFn("start", args) as? String
    }
    /// Cancel its requests in flight; false when none is.
    public func abort(id: String) -> Bool { callFn("abort", ["id": id]) as? Bool ?? false }
    /// Delete its record and the response files the host kept.
    public func retire(id: String) { _ = callFn("retire", ["id": id]) }
    /// `{id, uploadTotal, uploaded, downloadTotal, downloaded, result,
    /// failureReason, records: [{index, url, status, contentType, path?,
    /// location?, saveTo?, bodyLost}], apple?}`, never a header or a query.
    public func record(id: String) -> [String: Any]? { callFn("record", ["id": id]) as? [String: Any] }
    /// This implementation's registrations not retired.
    public var ids: [String] { callFn("ids", [:]) as? [String] ?? [] }
}

/// The host's region carrier (LLP 1133.021 D14): one `CLMonitor` the host
/// makes again at launch, before a package loads, whenever its store holds
/// a region, and the store, which is the truth. A crossing is written to
/// the store as the region's undelivered transition, then offered to the
/// implementation that added the region as `woke(_:)` of kind `region`, its
/// keys the regions' names; a transition stays until `delivered(name:)`,
/// and is offered again at each later launch. macOS hears crossings only
/// while the app runs.
public final class ExactRegions {
    private let callFn: (String, [String: Any]) -> Any?
    public init(_ call: @escaping (String, [String: Any]) -> Any?) { callFn = call }
    /// Watch a circle of `radius` metres around the coordinate as `name`,
    /// replacing a region of that name. Nil, or the error's name.
    public func add(name: String, latitude: Double, longitude: Double, radius: Double) -> String? {
        callFn("add", ["name": name, "latitude": latitude, "longitude": longitude, "radius": radius]) as? String
    }
    /// Stop watching `name`, and forget its undelivered transition.
    public func remove(name: String) { _ = callFn("remove", ["name": name]) }
    /// This implementation's regions: `{name, latitude, longitude, radius, inside?}`.
    public func list() -> [[String: Any]] { callFn("list", [:]) as? [[String: Any]] ?? [] }
    /// The undelivered transition of `name`: `{event: "enter"|"leave", timestamp}`, ms since the epoch.
    public func pending(name: String) -> [String: Any]? { callFn("pending", ["name": name]) as? [String: Any] }
    /// The transition of `name` was delivered: the store forgets it.
    public func delivered(name: String) { _ = callFn("delivered", ["name": name]) }
}

/// A wake the host was given for an implementation's due requests.
public final class ExactWake {
    /// `refresh`, `processing`, `transfer`, or `region`.
    public let kind: String
    /// The keys of the requests now due.
    public let keys: [String]
    /// When the platform ends the wake (ms since the epoch), when known.
    public let deadline: Double?
    /// The platform's own object: the `BGTask` on iOS, nil on the Mac.
    public let platform: AnyObject?
    private let raiseFn: (String, [Any], @escaping (String) -> Void) -> Void
    private var doneFn: ((Bool) -> Void)?
    public init(kind: String, keys: [String], deadline: Double?, platform: AnyObject?,
                raise: @escaping (String, [Any], @escaping (String) -> Void) -> Void, done: @escaping (Bool) -> Void) {
        self.kind = kind; self.keys = keys; self.deadline = deadline; self.platform = platform
        raiseFn = raise; doneFn = done
    }
    /// Raise one declared wake event; `ended` hears its outcome.
    public func raise(_ event: String, _ args: [Any], ended: @escaping (String) -> Void) { raiseFn(event, args, ended) }
    /// Every event this wake raised has ended: the host completes the
    /// platform's task once each implementation it woke is done. Once.
    public func done(success: Bool) { let d = doneFn; doneFn = nil; d?(success) }
    /// The host's due wake as the composed adapter hears it; each event
    /// raised is a wake of its own id. When the host says the platform
    /// expired its task (`exact.wakes.expired` with this wake's token), each
    /// event raised that has not ended expires.
    public convenience init(_ wake: [String: Any], context: ExactPackageContext, done: @escaping (Bool) -> Void) {
        var open: Set<String> = []
        var observer: NSObjectProtocol?
        self.init(kind: wake["kind"] as? String ?? "refresh", keys: wake["keys"] as? [String] ?? [], deadline: wake["deadline"] as? Double,
                  platform: wake["task"] as AnyObject?,
                  raise: { [context] event, args, ended in
                      let id = UUID().uuidString
                      open.insert(id)
                      context.wake(event, args, id: id) { outcome in open.remove(id); ended(outcome) }
                  },
                  done: { success in
                      if let o = observer { NotificationCenter.default.removeObserver(o); observer = nil }
                      done(success)
                  })
        guard let token = wake["token"] as? String else { return }
        observer = NotificationCenter.default.addObserver(forName: Notification.Name("exact.wakes.expired"), object: nil, queue: .main) { [context] note in
            guard (note.object as? [String: Any])?["token"] as? String == token else { return }
            for id in open { context.expireWake(id: id) }
        }
    }
}

// MARK: canvas inputs (LLP 1133.013 D6, D12 item 3)

/// The session's canvases, as an implementation feeds them.
public final class ExactSurfaces {
    public typealias Open = (_ surface: String) -> ExactSurfaceInput?
    private let open: Open
    public init(_ open: @escaping Open) { self.open = open }
    /// The input of surface `surface`'s canvas: the host drains what is
    /// written there into the live canvas of that surface once a display
    /// frame, before the frame's input reaches its module, every sample in
    /// order. Nil when the session places no canvas of that surface, or the
    /// host feeds none (an older host, one with no GPU module).
    public func input(surface: String) -> ExactSurfaceInput? { open(surface) }
}

/// A canvas's input that an implementation writes (`ExactSurfaces.input`).
/// Main thread. The host keeps about a second at the fastest rate a phone
/// samples; past it the oldest samples go, and the next sample of their
/// sensor delivered counts them in its `dropped`.
public final class ExactSurfaceInput {
    public typealias Write = (_ sensor: String, _ values: [Double], _ dropped: Int, _ at: Double, _ ended: Bool) -> Void
    public let surface: String
    private let write: Write
    public init(surface: String, write: @escaping Write) { self.surface = surface; self.write = write }
    /// One sample of `sensor` (a Generic Sensor name: `gravity`,
    /// `gyroscope`, …): x, y, z, and w for an orientation's quaternion, in
    /// the frame the input asked for; finite. `dropped` counts the samples
    /// lost before it (a pause); `at` is the session clock's time (ms) when
    /// it was taken, which also stamps the canvas's keys and touches.
    public func sample(_ sensor: String, _ values: [Double], dropped: Int = 0, at: Double) {
        write(sensor, values, dropped, at, false)
    }
    /// The input of `sensor` ended (`stopSensorInput`, the canvas gone): its
    /// last sample no longer stands.
    public func end(_ sensor: String, at: Double) { write(sensor, [], 0, at, true) }
}

// MARK: the authenticator entry (LLP 1133.010 D13)

/// What an implementation serves when its interface declares an
/// `authenticator`: the host's presence backend asks it for the person's
/// check before reading a locked keychain item, in this process. The sheet
/// has one owner, the implementation, with one permission history and one
/// origin rule; the item and its bytes stay the host's.
public protocol ExactAuthenticator: AnyObject {
    /// Evaluate `accessControl` for the use of its item, showing the
    /// platform's sheet with `reason`, already translated. `policy` is the
    /// grant line's words after the name (`user-presence optional`);
    /// `origin` who made the call (`app`, `tool`, `wake`). `reply` once, on
    /// the main thread: the evaluated context (an `LAContext`), which the
    /// host reads the item with and then invalidates, or a failure
    /// `{"error": code, "message": text}` in the interface's codes.
    func evaluate(accessControl: SecAccessControl, reason: String, policy: String, origin: String,
                  reply: @escaping (AnyObject?, [String: Any]?) -> Void)
}

// MARK: tracks (LLP 1133.009 D3, D6, D10 item 6)

/// Why a track call failed: a `DOMException` name, as answered.
public struct ExactTrackError: Error {
    public let code: String
    public init(_ code: String) { self.code = code }
}

/// What a subscription returns: `cancel()` leaves, once.
public final class ExactSubscription {
    private var leave: (() -> Void)?
    public init(_ leave: @escaping () -> Void) { self.leave = leave }
    public func cancel() { let l = leave; leave = nil; l?() }
}

/// A track or a stream in the table. A track's `owner` is the
/// implementation that opened it; a stream lists its tracks.
public final class ExactTrackEntry {
    public let id: String, kind: String, tracks: [String]
    public weak var owner: ExactTrackOwner?
    /// A track's state as `mediaStreamTrack` answers it (`muted`,
    /// `readyState`, `settings`, …), for a package that consumes it.
    public var state: (() -> [String: Any])?
    /// A video track's running capture, while its lease is held (LLP
    /// 1133.008 D3, D8 item 3); nil otherwise and under the agent.
    public var video: (() -> ExactVideoSource?)?
    /// Under the agent, where no capture exists: the picture a viewfinder of
    /// the track draws, upright and unmirrored, which an observer analyses as
    /// each frame (LLP 1133.027 D17 item 2). The camera package sets it.
    public var picture: CGImage?
    public init(id: String, kind: String, tracks: [String] = [], owner: ExactTrackOwner? = nil) {
        self.id = id; self.kind = kind; self.tracks = tracks; self.owner = owner
    }
}

/// The implementation that opened a track: it decides whether a consumer
/// joining takes the track's lease (D6), and tells it the track's changes.
public protocol ExactTrackOwner: AnyObject {
    /// `consumer` joins track `id`; `start` is an explicit start, which
    /// reopens an idle track; `origin` is `app`, `tool` or `wake`.
    func subscribe(_ id: String, _ consumer: ExactTrackConsumer, start: Bool, origin: String) throws -> ExactSubscription
    /// `observer` joins track `id` as an output that is not a consumer (LLP
    /// 1133.027 D17 item 1): told what a consumer is told, attaching its
    /// outputs through the same `configure`, but it never takes, holds or
    /// reopens the lease and never counts as a consumer.
    func observe(_ id: String, _ observer: ExactTrackConsumer) throws -> ExactSubscription
}

public extension ExactTrackOwner {
    func observe(_ id: String, _ observer: ExactTrackConsumer) throws -> ExactSubscription { throw ExactTrackError("NotSupportedError") }
}

/// A consumer of a track: a recorder, a level, a hatch's tap. Main thread.
public protocol ExactTrackConsumer: AnyObject {
    /// The capture runs: its source, or nil under the agent (no device).
    func live(_ source: ExactAudioSource?)
    /// A video track's capture runs: its source, or nil under the agent.
    func live(video source: ExactVideoSource?)
    /// The track is muted: `idle`, `interrupted`, `background`, `silenced`, …
    func muted(_ reason: String)
    func unmuted()
    /// The track ended; nothing more follows.
    func ended()
}

public extension ExactTrackConsumer {
    func live(_ source: ExactAudioSource?) {}
    func live(video source: ExactVideoSource?) {}
}

/// A running camera capture (LLP 1133.008 D3, D8 item 3): the platform's
/// capture graph, which a consumer attaches outputs to rather than reading
/// frames. A video track's consumer is told `live(video:)` instead of
/// `live(_:)`, with nil under the agent.
public protocol ExactVideoSource: AnyObject {
    /// The capture session. A preview layer is attached to it on the main
    /// thread; inputs and outputs only inside `configure`.
    var session: AVCaptureSession { get }
    /// The camera it captures from.
    var device: AVCaptureDevice { get }
    /// Run `change` on the session's queue between `beginConfiguration()`
    /// and `commitConfiguration()`: add or remove inputs and outputs only.
    /// Never waited for from the main thread.
    func configure(_ change: @escaping (AVCaptureSession) -> Void)
    /// Run `work` on the session's queue, after every configuration asked
    /// before it: a capture from an output it added.
    func run(_ work: @escaping (AVCaptureSession) -> Void)
    /// A package's process object for this capture, made by `make` on first
    /// ask and kept while the capture lives (the camera's photo output).
    func shared<T: AnyObject>(_ key: String, _ make: () -> T) -> T
    /// Constraints that keep the camera (`zoom`, `torch`), applied to the
    /// device and reported by the tracks on it, as `applyConstraints` would.
    func apply(_ constraints: [String: Any])
    /// Focus and expose at `point`, in the device's unit square.
    func focus(at point: CGPoint)
}

/// A running audio capture, which a consumer taps.
public protocol ExactAudioSource: AnyObject {
    var format: AVAudioFormat { get }
    /// `block` hears each buffer of the capture on the audio thread until
    /// the subscription is cancelled. It must not block.
    func tap(_ block: @escaping (AVAudioPCMBuffer, AVAudioTime) -> Void) -> ExactSubscription
}

/// The session's track table: tracks and streams by id.
public final class ExactTracks {
    private var entries: [String: ExactTrackEntry] = [:]
    public init() {}
    public func add(_ entry: ExactTrackEntry) { entries[entry.id] = entry }
    public func lookup(_ id: String) -> ExactTrackEntry? { entries[id] }
    public func remove(_ id: String) { entries[id] = nil }
    public var all: [ExactTrackEntry] { Array(entries.values) }
    /// `consumer` joins track `id`, through its owner.
    public func subscribe(_ id: String, _ consumer: ExactTrackConsumer, start: Bool, origin: String = "app") throws -> ExactSubscription {
        guard let entry = entries[id], let owner = entry.owner else { throw ExactTrackError("NotFoundError") }
        return try owner.subscribe(id, consumer, start: start, origin: origin)
    }
    /// `observer` watches track `id`, through its owner, taking no lease.
    public func observe(_ id: String, _ observer: ExactTrackConsumer) throws -> ExactSubscription {
        guard let entry = entries[id], let owner = entry.owner else { throw ExactTrackError("NotFoundError") }
        return try owner.observe(id, observer)
    }
}

// MARK: pads (LLP 1133.022 D4, D10 item 1)

/// The process's one pad slot table: by index, the pad's controller object
/// (`GCController`) or `NSNull`. A canvas's pads (ExactSurfaces'
/// `CanvasPads`, which cannot import this module) and a pad module both
/// number pads from it, so an index names one pad to each. It lives in the
/// main thread's dictionary, which the host and the app's module share; a
/// pad that connects takes the slot it holds, else the lowest free one, and
/// one that goes frees it. Main thread.
public final class ExactPads {
    /// Under the agent, bound or not: a pad module lists only `injected`, and
    /// never reads or moves a pad attached to the machine.
    public let driven: Bool
    /// The drive's pads (`EXACT_AGENT_PADS`, a JSON list), each `{index, id,
    /// category, vendor, pressed}`: empty outside the agent. Their motors
    /// and light are a recorder (LLP 1133.022 §10).
    public let injected: [[String: Any]]
    /// The slot table's key in `Thread.main.threadDictionary`.
    public static let key = "exact.pads"
    /// GameController's own limit is lower; this bounds a misbehaving source.
    public static let most = 16
    public init(driven: Bool = false, injected: [[String: Any]] = []) { self.driven = driven; self.injected = driven ? injected : [] }

    private static var table: NSMutableArray {
        let d = Thread.main.threadDictionary
        if let t = d[key] as? NSMutableArray { return t }
        let t = NSMutableArray()
        d[key] = t
        return t
    }

    /// `pad`'s slot, if it holds one.
    public func index(of pad: AnyObject) -> Int? {
        let t = Self.table
        return (0..<t.count).first { t[$0] as AnyObject === pad }
    }
    /// `pad`'s slot: the one it holds, else `at` when free, else the lowest
    /// free one; nil when all are taken.
    public func take(_ pad: AnyObject, at wanted: Int? = nil) -> Int? {
        if let held = index(of: pad) { return held }
        let t = Self.table
        let free = (0..<t.count).filter { t[$0] is NSNull }
        let index = wanted.flatMap { w in w >= t.count || t[w] is NSNull ? w : nil } ?? free.first ?? t.count
        guard index < Self.most else { return nil }
        while t.count <= index { t.add(NSNull()) }
        t[index] = pad
        return index
    }
    /// `pad` leaves its slot: the slot it held.
    @discardableResult
    public func free(_ pad: AnyObject) -> Int? {
        guard let index = index(of: pad) else { return nil }
        Self.table[index] = NSNull()
        return index
    }
}

// MARK: the audio session (LLP 1133.009 D7, D10 item 5)

/// The process's audio session, which the host owns: a capture claims it
/// and the host composes every claim with the app's own sound.
public final class ExactAudioSession {
    public typealias Claim = (_ mode: String?, _ options: [String], _ background: Bool, _ events: @escaping (String) -> Void) -> (() -> Void)?
    private let claimFn: Claim
    public init(_ claim: @escaping Claim) { claimFn = claim }
    /// A capture's claim: the session records (`.playAndRecord`) while any
    /// is held. `events` hears `interrupted`, `resumed` (an interruption
    /// ended, the session active again), `reset` (media services were
    /// reset) and `route`. Nil when the host refused it (the session would
    /// not activate). Release it with `cancel()`.
    ///
    /// `mode` `speech-transient`, `speech-transient-solo` or
    /// `speech-playback` is a queue's speech claim instead (LLP 1133.012 D4):
    /// the owner takes the strongest spoken type held whenever no other I/O
    /// runs, and gives the session up when the last speech claim goes. It
    /// hears the same events; `options` are not read.
    public func claim(mode: String?, options: [String], background: Bool, events: @escaping (String) -> Void) -> ExactSubscription? {
        claimFn(mode, options, background, events).map(ExactSubscription.init)
    }
    /// A call's claim (LLP 1133.033 D10): the session takes the call's
    /// configuration and is never activated by the app while any is held;
    /// the system activates it. Captures and the app's sound join it. Held
    /// for as long as any call lasts. Nil where the host has no audio
    /// session owner to compose.
    public func claimCall(video: Bool, events: @escaping (String) -> Void) -> ExactSubscription? {
        claimFn(video ? "call-video" : "call-voice", [], true, events).map(ExactSubscription.init)
    }
    /// CallKit activated (`provider(_:didActivate:)`) or deactivated the
    /// calls' session.
    public func callAudio(_ active: Bool) {
        _ = claimFn(active ? "call-activated" : "call-deactivated", [], false, { _ in })
    }
}
