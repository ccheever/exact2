// What a package's Swift implementation of a module sees of exact2 (LLP
// 1133.001 D5; LLP 1133.009 D10 items 1-3; LLP 1133.008 D8 item 9): the
// `ExactPackage` module, compiled on its own before the packages, which
// import it, and linked into the app's one module, whose adapter for each
// package (`exactExtensions`, written by host/apple/build.mjs) holds one
// instance per session. Nothing of ExactKit or of the app's own module is
// reachable from here.
//
// Every entry is on the main thread (LLP 1067.000 D4).
import Foundation
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
    /// Under the agent, unless the drive bound the implementation as in
    /// production (`--bind`): an implementation that is its own substitute
    /// then opens no device and asks the OS for nothing (LLP 1133.006 D8).
    public let agent: Bool
    /// The session's id, unique in the process: what a process object
    /// shared by sessions (a capture, a photo output) keys its consumers by.
    public let session: String
    /// Whether the app is in the foreground, as the runner's admission reads
    /// it; the host sets it before `foreground(_:)` is told each change.
    public var foreground = true
    private let changedFn: (String) -> Void
    private let logFn: (String) -> Void
    private let nowFn: () -> Double

    public init(agent: Bool, session: String, changed: @escaping (String) -> Void,
                log: @escaping (String) -> Void, now: @escaping () -> Double) {
        self.agent = agent; self.session = session
        changedFn = changed; logFn = log; nowFn = now
    }

    /// The readers of `topic` are asked again (LLP 1016.002).
    public func changed(_ topic: String) { changedFn(topic) }
    /// A journal line in a development build; nothing in a production one.
    public func log(_ line: String) { logFn(line) }
    /// The session's clock, ms: the agent's under the agent.
    public func now() -> Double { nowFn() }
}

/// A package's implementation class: what the adapter calls. Only `init`,
/// `later` and `call` are required; the rest do nothing unless the
/// implementation has something to do.
public protocol ExactPackage: AnyObject {
    init(context: ExactPackageContext)
    /// A source's read or a send, by its declared name, with the keyword
    /// arguments the call wrote; `reply` once. Under the agent, `preferred`
    /// is the agent's value for the name, which the runner also lays over
    /// the answer (LLP 1133.009 D10 item 11).
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
}

public extension ExactPackage {
    func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] { ["error": "NotSupportedError"] }
    func release(_ name: String) {}
    func release(_ name: String, args: [String: Any]) {}
    func preferred(_ values: [String: Any]) {}
    func dispose(_ ids: [String]) {}
    func foreground(_ shown: Bool) {}
    func destroy(_ done: @escaping () -> Void) { done() }
    func view(_ tag: String, id: String, props: [String: String], events: ExactPackageEvents) throws -> ExactPackageView {
        throw ExactPackageRefusal("\(type(of: self)) serves no view \(tag)")
    }
    func displayed(_ id: String, _ shown: Bool, origin: String) {}
    func bound(_ id: String, _ prop: String, origin: String) {}
}
