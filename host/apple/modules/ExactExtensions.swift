// A package's Swift implementation of a module, composed into the app's one
// module (LLP 1124 D6, LLP 1133.001 D1, D5). Each package compiles as its own
// Swift module, so two packages' types cannot collide; the build writes an
// adapter for each (`exactExtensions`, beside this file, never committed)
// over the package's class, whose shape is the whole contract:
//
//     public init(changed: @escaping (String) -> Void, agent: Bool, log: @escaping (String) -> Void)
//     public func later(_ name: String, _ args: [String: Any], reply: @escaping ([String: Any]) -> Void)
//     public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any]
//     public func release(_ name: String)
//
// A request naming `extension` is the runner's call of one of its names
// (`{"extension", "source"|"send", "args"}`), its release of a live source
// (`{"extension", "release"}`), the agent's preferred values for an
// implementation bound under the agent (`{"extension", "preferred"}`), or
// data code's synchronous form (`{"extension", "sync", "args"}`, through
// `call`), and goes to that extension's adapter, made once a session on its
// first request; the app's own module never sees it. An error is answered
// as `{"error": code}`. `changed(topic)` asks the topic's readers again;
// `log` is a journal line in a development build and nothing in a
// production one. `agent` is true under the agent unless the drive bound
// the implementation as in production (`--bind`).
import Foundation

/// One extension's adapter, as the build writes it.
open class ExactExtensionAdapter {
    public init() {}
    open func later(_ name: String, _ args: [String: Any], reply: ExactReply) {
        reply.send(["error": "NotSupportedError"])
    }
    open func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        ["error": "NotSupportedError"]
    }
    open func release(_ name: String) {}
    open func preferred(_ values: [String: Any]) {}
}

/// The adapters of each session's module, by extension; main thread only.
enum ExactExtensions {
    private static var made: [ObjectIdentifier: [String: ExactExtensionAdapter]] = [:]

    /// The extensions this artifact composes, for the host to bind.
    static var names: [String] { exactExtensions.keys.sorted() }

    /// The extensions a drive binds as in production (`--bind`, `EXACT_AGENT_BIND`).
    static let real = Set((ProcessInfo.processInfo.environment["EXACT_AGENT_BIND"] ?? "").split(separator: ",").map(String.init))

    private static func adapter(_ m: ExactModule, _ name: String) -> ExactExtensionAdapter? {
        if let a = made[ObjectIdentifier(m)]?[name] { return a }
        guard let make = exactExtensions[name] else { return nil }
        let a = make(m.context)
        made[ObjectIdentifier(m), default: [:]][name] = a
        return a
    }

    /// A long call or a release; `false` for a request that names no extension.
    static func later(_ m: ExactModule, _ request: [String: Any], reply: ExactReply) -> Bool {
        guard let extensionName = request["extension"] as? String else { return false }
        guard let a = adapter(m, extensionName) else {
            reply.send(["error": "NotSupportedError"])
            return true
        }
        let args = request["args"] as? [String: Any] ?? [:]
        if let name = request["release"] as? String {
            a.release(name)
            reply.send([:])
        } else if let values = request["preferred"] as? [String: Any] {
            a.preferred(values)
            reply.send([:])
        } else if let name = (request["source"] ?? request["send"]) as? String {
            a.later(name, args, reply: reply)
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

    /// The session's module is going: its adapters with it.
    static func destroy(_ m: ExactModule) {
        made[ObjectIdentifier(m)] = nil
    }
}
