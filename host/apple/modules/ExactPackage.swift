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
}

public extension ExactPackage {
    func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] { ["error": "NotSupportedError"] }
    func release(_ name: String) {}
    func release(_ name: String, args: [String: Any]) {}
    func preferred(_ values: [String: Any]) {}
    func dispose(_ ids: [String]) {}
    func foreground(_ shown: Bool) {}
    func destroy(_ done: @escaping () -> Void) { done() }
}
