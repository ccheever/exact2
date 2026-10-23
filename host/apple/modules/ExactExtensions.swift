// A package's Swift implementation of a module, composed into the app's one
// module (LLP 1124 D6, LLP 1133.001 D1, D5). Each package compiles as its own
// Swift module against `ExactPackage` (ExactPackage.swift, its class's shape
// and the context it is given), so two packages' types cannot collide; the
// build writes an adapter for each (`exactExtensions`, beside this file,
// never committed) over the package's class.
//
// A request naming `extension` is the runner's, and goes to that
// extension's adapter, made once a session on its first request; the app's
// own module never sees it:
//
// - `{"extension", "source"|"send", "args", "preferred"?}`: a call of one
//   of its names;
// - `{"extension", "release", "args"?}`: a live source with no reader left,
//   or, with `args`, an argument group of one (LLP 1133.009 D10 item 12);
// - `{"extension", "preferred"}`: the agent's preferred values, for an
//   implementation bound under the agent;
// - `{"extension", "dispose"}`: handles a dropped reply carried (item 10);
// - `{"extension", "foreground"}`: the app went to the foreground or left
//   it (item 3);
// - `{"extension", "sync", "args"}`, through `call`: data code's
//   synchronous form.
//
// An error is answered as `{"error": code}`. At the session's end each
// adapter's `destroy` is called and waited for, a second at most (item 1).
import Foundation

/// One extension's adapter, as the build writes it.
open class ExactExtensionAdapter {
    public init() {}
    open func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: ExactReply) {
        reply.send(["error": "NotSupportedError"])
    }
    open func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        ["error": "NotSupportedError"]
    }
    open func release(_ name: String) {}
    open func release(_ name: String, args: [String: Any]) {}
    open func preferred(_ values: [String: Any]) {}
    open func dispose(_ ids: [String]) {}
    open func foreground(_ shown: Bool) {}
    open func destroy(_ done: @escaping () -> Void) { done() }
}

/// The adapters of each session's module, by extension; main thread only.
enum ExactExtensions {
    private static var made: [ObjectIdentifier: [String: ExactExtensionAdapter]] = [:]
    /// Each session's id, as its adapters' contexts say it.
    private static var sessions: [ObjectIdentifier: String] = [:]
    private static var nextSession = 0

    /// The extensions this artifact composes, for the host to bind.
    static var names: [String] { exactExtensions.keys.sorted() }

    /// The extensions a drive binds as in production (`--bind`, `EXACT_AGENT_BIND`).
    static let real = Set((ProcessInfo.processInfo.environment["EXACT_AGENT_BIND"] ?? "").split(separator: ",").map(String.init))

    private static func adapter(_ m: ExactModule, _ name: String) -> ExactExtensionAdapter? {
        let key = ObjectIdentifier(m)
        if let a = made[key]?[name] { return a }
        guard let make = exactExtensions[name] else { return nil }
        let session = sessions[key] ?? {
            nextSession += 1
            sessions[key] = String(nextSession)
            return String(nextSession)
        }()
        let a = make(m.context, session)
        made[key, default: [:]][name] = a
        return a
    }

    /// A request of the runner's; `false` for one that names no extension.
    static func later(_ m: ExactModule, _ request: [String: Any], reply: ExactReply) -> Bool {
        guard let extensionName = request["extension"] as? String else { return false }
        guard let a = adapter(m, extensionName) else {
            reply.send(["error": "NotSupportedError"])
            return true
        }
        let args = request["args"] as? [String: Any]
        if let name = request["release"] as? String {
            if let args { a.release(name, args: args) } else { a.release(name) }
            reply.send([:])
        } else if let values = request["preferred"] as? [String: Any], request["source"] == nil, request["send"] == nil {
            a.preferred(values)
            reply.send([:])
        } else if let ids = request["dispose"] as? [String] {
            a.dispose(ids)
            reply.send([:])
        } else if let shown = request["foreground"] as? Bool {
            a.foreground(shown)
            reply.send([:])
        } else if let name = (request["source"] ?? request["send"]) as? String {
            a.later(name, args ?? [:], preferred: request["preferred"], reply: reply)
        } else {
            reply.send(["error": "NotSupportedError"])
        }
        return true
    }

    /// A synchronous form (`{"extension", "sync", "args"}`); `nil` for a request that names no extension.
    static func call(_ m: ExactModule, _ request: [String: Any]) throws -> [String: Any]? {
        guard let extensionName = request["extension"] as? String else { return nil }
        guard let a = adapter(m, extensionName), let name = request["sync"] as? String else {
            return ["error": "NotSupportedError"]
        }
        return try a.call(name, request["args"] as? [String: Any] ?? [:])
    }

    /// The session's module is going: each adapter finishes what it must,
    /// waited for a second at most on the main run loop, and goes with it.
    static func destroy(_ m: ExactModule) {
        let key = ObjectIdentifier(m)
        let adapters = made.removeValue(forKey: key) ?? [:]
        sessions[key] = nil
        guard !adapters.isEmpty else { return }
        var waiting = Set(adapters.keys)
        for (name, a) in adapters { a.destroy { waiting.remove(name) } }
        let deadline = Date().addingTimeInterval(1)
        while !waiting.isEmpty, Thread.isMainThread, Date() < deadline {
            _ = RunLoop.main.run(mode: .default, before: deadline)
        }
        if !waiting.isEmpty {
            m.context.diagnostics.log("extensions \(waiting.sorted().joined(separator: ", ")): not finished a second after the session's end; let go as they stand")
        }
    }
}
