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
// An interface that declares events (LLP 1133.004 D6) gives its init two
// more: `emit: @escaping (String, [Any]) -> Void`, which raises one with the
// device's origin, and `activation: @escaping (String, [Any], String?) ->
// [AnyHashable: Any]`, the `userInfo` entry for content whose opening
// raises one (or opens the location given) through the host's delegate.
//
// A request naming `extension` is the runner's call of one of its names
// (`{"extension", "source"|"send", "args"}`), its release of a live source
// (`{"extension", "release"}`), or data code's synchronous form (`{"extension",
// "sync", "args"}`, through `call`), and goes to that extension's adapter,
// made once a session on its first request; the app's own module never sees
// it. An error is answered as `{"error": code}`. `changed(topic)` asks the
// topic's readers again; `log` is a journal line in a development build and
// nothing in a production one.
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
    /// A long call whose answer the host reads itself (its admission, below).
    open func ask(_ name: String, _ args: [String: Any], answer: @escaping ([String: Any]) -> Void) {
        answer(["error": "NotSupportedError"])
    }
}

/// The adapters of each session's module, by extension; main thread only.
enum ExactExtensions {
    private static var made: [ObjectIdentifier: [String: ExactExtensionAdapter]] = [:]

    /// The extensions this artifact composes, for the host to bind.
    static var names: [String] { exactExtensions.keys.sorted() }

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
        } else if let name = (request["source"] ?? request["send"]) as? String, let admit = request["admit"] as? [String: Any],
                  let status = admit["status"] as? String, let field = admit["field"] as? String {
            // A device-origin call that would ask (LLP 1133.002 D12, LLP 1133.004 D6): refused while the
            // implementation's own status reads `prompt`, since only a person may answer the prompt.
            a.ask(status, [:]) { answer in
                guard answer[field] as? String != "prompt" else {
                    return reply.send(["error": admit["denied"] as? String ?? "NotAllowedError", "message": "\(name): person-required"])
                }
                a.later(name, args, reply: reply)
            }
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

extension ExactModuleContext {
    /// Raise `extensionName`'s event `name` with `args` (LLP 1133.004 D6):
    /// delivered in order, once, with the device's origin. Its one channel
    /// to the runner is `changed`; the sequence number keeps two alike from
    /// coalescing as two announcements of one topic do. Any thread: the
    /// count is the main thread's, so a call from elsewhere hops there.
    public func emit(_ extensionName: String, _ name: String, _ args: [Any]) {
        guard Thread.isMainThread else {
            let boxed = ExactEmitted(args)
            return DispatchQueue.main.async { [weak self] in self?.emit(extensionName, name, boxed.args) }
        }
        emitted &+= 1
        let body: [String: Any] = ["extension": extensionName, "event": name, "args": args, "seq": emitted]
        guard JSONSerialization.isValidJSONObject(body), let json = try? JSONSerialization.data(withJSONObject: body) else {
            diagnostics.log("\(extensionName).\(name): its arguments are not JSON")
            return
        }
        changed("exact-event:" + String(decoding: json, as: UTF8.self))
    }

    /// The `userInfo` entry for content the module posts whose activation
    /// raises `name` (LLP 1133.004 D6, D7): the host's notification delegate
    /// reads it when the person opens the notification, opens `navigate` if
    /// given, else raises the event on the session that posted it.
    public func activation(_ extensionName: String, _ name: String, _ args: [Any], navigate: String? = nil) -> [AnyHashable: Any] {
        let json = (try? JSONSerialization.data(withJSONObject: args)).map { String(decoding: $0, as: UTF8.self) } ?? "[]"
        var entry: [String: Any] = poster
        entry["extension"] = extensionName; entry["event"] = name; entry["args"] = json
        if let navigate { entry["navigate"] = navigate }
        return ["exact": entry]
    }
}

/// An emitted event's arguments on their way to the main thread.
private final class ExactEmitted: @unchecked Sendable {
    let args: [Any]
    init(_ args: [Any]) { self.args = args }
}
