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
// - `{"extension", "source"|"send", "args", "preferred"?, "origin"?,
//   "request"?}`: a call of one of its names, `origin` when a tool or a
//   wake made it, `request` a send's number, which `cancel` names;
// - `{"extension", "release", "args"?}`: a live source with no reader left,
//   or, with `args`, an argument group of one (LLP 1133.009 D10 item 12);
// - `{"extension", "preferred", "permissions"}`: the agent's preferred
//   values and the runner's permission states, for an implementation bound
//   under the agent;
// - `{"extension", "dispose"}`: handles a dropped reply carried (item 10);
// - `{"extension", "cancel"}`: a send let go before its reply (LLP
//   1133.016 D15 item 2);
// - `{"extension", "foreground"}`: the app went to the foreground or left
//   it (item 3);
// - `{"extension", "view", "displayed"|"bound", "origin"}`: a package view's
//   displayed state or binding prop changed (LLP 1133.008 D8 item 2);
// - `{"extension", "surface", "displayed", "origin"}`: the canvases of a
//   surface its input feeds were displayed or hidden (LLP 1133.013 D6);
// - `{"extension", "woke", "outcome"}`: a wake it raised ended (LLP
//   1133.017 D8), told to the `ended` its `context.wake` gave;
// - `{"extension", "sync", "args"}`, through `call`: data code's
//   synchronous form.
//
// A picture of a package's `img` scheme comes through the table's `image`
// entry instead (`ExactExtensions.image`), since its answer is an image.
//
// An error is answered as `{"error": code}`. At the session's end each
// adapter's `destroy` is called and waited for, a second at most (item 1).
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
import CoreGraphics
import Security
#if canImport(ExactPackage)
import ExactPackage
#endif

/// One extension's adapter, as the build writes it.
open class ExactExtensionAdapter {
    public init() {}
    open func later(_ name: String, _ args: [String: Any], preferred: Any?, origin: String, request: Int?, reply: ExactReply) {
        reply.send(["error": "NotSupportedError"])
    }
    /// The implementation's own answer to `name`, read by the host itself
    /// (the permission's status, before a call not the app's).
    open func ask(_ name: String, _ args: [String: Any], answer: @escaping ([String: Any]) -> Void) {
        answer(["error": "NotSupportedError"])
    }
    open func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        ["error": "NotSupportedError"]
    }
    open func release(_ name: String) {}
    open func release(_ name: String, args: [String: Any]) {}
    open func preferred(_ values: [String: Any]) {}
    open func dispose(_ ids: [String]) {}
    open func cancel(_ request: Int) {}
    open func foreground(_ shown: Bool) {}
    open func destroy(_ done: @escaping () -> Void) { done() }
    open func view(_ tag: String, id: String, props: [String: String], events: ExactNativeEvents) throws -> ExactNativeInstance {
        throw ExactNativeRefusal("no view \(tag)")
    }
    open func displayed(_ id: String, _ shown: Bool, origin: String) {}
    open func bound(_ id: String, _ prop: String, origin: String) {}
    open func image(_ source: String, width: Int, height: Int, fit: String, owned: Bool, reply: @escaping (CGImage?, String?) -> Void) {
        reply(nil, "NotSupportedError")
    }
    open func permissions(_ states: [String: String]) {}
    open func displayed(surface: String, _ shown: Bool, origin: String) {}
    open func orientation(_ angle: Double) {}
    /// The host's wake for this extension's due requests (`ExactWake`'s fields as a dictionary).
    open func due(_ wake: [String: Any], done: @escaping (Bool) -> Void) { done(true) }
    /// The authenticator entry (ExactPackage.swift's `ExactAuthenticator`),
    /// for the extension `exactAuthenticator` names.
    open func evaluate(_ accessControl: SecAccessControl, _ request: [String: Any], reply: @escaping (AnyObject?, [String: Any]?) -> Void) {
        reply(nil, ["error": "NotSupportedError", "message": "this extension serves no authenticator"])
    }
}

/// The adapters of each session's module, by extension; main thread only.
enum ExactExtensions {
    private static var made: [ObjectIdentifier: [String: ExactExtensionAdapter]] = [:]
    /// Each session's id, as its adapters' contexts say it.
    private static var sessions: [ObjectIdentifier: String] = [:]
    private static var nextSession = 0
    /// The `ended` each `context.wake` gave, by module, extension and id.
    static var ended: [String: (String) -> Void] = [:]
    /// The live modules, in the order they were made: the first is handed
    /// a due wake (LLP 1133.017 D7: the app's first session).
    private static var modules: [WeakModule] = []

    /// A module was made. The first one hears the host's due wakes
    /// (`exact.wakes.due`, ExactKit's Wakes.swift), each handed to its
    /// extension's adapter; then the host is told a module is ready, so it
    /// hands over a wake it holds.
    static func created(_ m: ExactModule) {
        modules.append(WeakModule(m))
        if modules.count == 1 {
            NotificationCenter.default.addObserver(forName: Notification.Name("exact.wakes.due"), object: nil, queue: .main) { note in
                guard let offer = note.object as? NSMutableDictionary, let due = offer as? [String: Any],
                      let name = due["package"] as? String, let token = due["token"] as? String,
                      let m = modules.lazy.compactMap({ $0.module }).first, let a = adapter(m, name) else { return }
                offer["taken"] = true
                a.due(due) { success in
                    let body: [String: Any] = ["token": token, "package": name, "success": success]
                    NotificationCenter.default.post(name: Notification.Name("exact.wakes.done"), object: body as NSDictionary)
                }
            }
        }
        DispatchQueue.main.async { NotificationCenter.default.post(name: Notification.Name("exact.wakes.ready"), object: nil) }
    }

    /// The extensions this artifact composes, for the host to bind.
    static var names: [String] { exactExtensions.keys.sorted() }

    /// The extensions a drive binds as in production (`--bind`, `EXACT_AGENT_BIND`).
    static let real = Set((ProcessInfo.processInfo.environment["EXACT_AGENT_BIND"] ?? "").split(separator: ",").map(String.init))

    /// Each session's track table, which its adapters share (LLP 1133.009 D10 item 6).
    private static var tables: [ObjectIdentifier: AnyObject] = [:]

    private static func adapter(_ m: ExactModule, _ name: String) -> ExactExtensionAdapter? {
        let key = ObjectIdentifier(m)
        if let a = made[key]?[name] { return a }
        guard let make = exactExtensions[name] else { return nil }
        let session = sessions[key] ?? {
            nextSession += 1
            sessions[key] = String(nextSession)
            return String(nextSession)
        }()
        let a = make(m, session)
        made[key, default: [:]][name] = a
        return a
    }

    /// An `app:/` path's file in the session's directories.
    static func file(_ path: String, _ module: ExactModuleContext) -> URL? {
        guard path.hasPrefix("app:/") else { return nil }
        let parts = path.dropFirst(5).split(separator: "/", omittingEmptySubsequences: false).map(String.init)
        guard parts.count >= 2, parts.allSatisfy({ !$0.isEmpty && $0 != "." && $0 != ".." }) else { return nil }
        let roots = ["data": module.data, "cache": module.cache, "tmp": module.temporary]
        guard let root = roots[parts[0]] else { return nil }
        return parts.dropFirst().reduce(root) { $0.appendingPathComponent($1) }
    }

    /// A request of the runner's; `false` for one that names no extension.
    static func later(_ m: ExactModule, _ request: [String: Any], reply: ExactReply) -> Bool {
        guard let extensionName = request["extension"] as? String else { return false }
        guard let a = adapter(m, extensionName) else {
            reply.send(["error": "NotSupportedError"])
            return true
        }
        let args = request["args"] as? [String: Any]
        if let view = request["view"] as? String {
            let origin = request["origin"] as? String ?? "app"
            if let shown = request["displayed"] as? Bool { a.displayed(view, shown, origin: origin) }
            if let prop = request["bound"] as? String { a.bound(view, prop, origin: origin) }
            reply.send([:])
        } else if let surface = request["surface"] as? String, let shown = request["displayed"] as? Bool {
            a.displayed(surface: surface, shown, origin: request["origin"] as? String ?? "app")
            reply.send([:])
        } else if let name = request["release"] as? String {
            if let args { a.release(name, args: args) } else { a.release(name) }
            reply.send([:])
        } else if let values = request["preferred"] as? [String: Any], request["source"] == nil, request["send"] == nil {
            if let states = request["permissions"] as? [String: String] { a.permissions(states) }
            a.preferred(values)
            reply.send([:])
        } else if let ids = request["dispose"] as? [String] {
            a.dispose(ids)
            reply.send([:])
        } else if let cancelled = request["cancel"] as? Int {
            a.cancel(cancelled)
            reply.send([:])
        } else if let shown = request["foreground"] as? Bool {
            a.foreground(shown)
            reply.send([:])
        } else if let id = request["woke"] as? String {
            ended.removeValue(forKey: "\(ObjectIdentifier(m).hashValue) \(extensionName) \(id)")?(request["outcome"] as? String ?? "completed")
            reply.send([:])
        } else if let name = (request["source"] ?? request["send"]) as? String, let admit = request["admit"] as? [String: Any],
                  let status = admit["status"] as? String, let field = admit["field"] as? String {
            // A call not the app's that would ask (LLP 1133.002 D12, LLP 1133.004 D6): refused while the
            // implementation's own status reads `prompt`, since only a person may answer the prompt.
            let origin = request["origin"] as? String ?? "device"
            a.ask(status, [:]) { answer in
                guard answer[field] as? String != "prompt" else {
                    return reply.send(["error": admit["denied"] as? String ?? "NotAllowedError", "message": "person-required"])
                }
                a.later(name, args ?? [:], preferred: request["preferred"], origin: origin, request: request["request"] as? Int, reply: reply)
            }
        } else if let name = (request["source"] ?? request["send"]) as? String {
            a.later(name, args ?? [:], preferred: request["preferred"], origin: request["origin"] as? String ?? "app", request: request["request"] as? Int, reply: reply)
        } else {
            reply.send(["error": "NotSupportedError"])
        }
        return true
    }

    /// Package view `tag` of node `id`, made by `extension`'s adapter in `m`'s
    /// session, which is made now if no call made it first.
    static func view(_ m: ExactModule, _ extensionName: String, _ tag: String, id: String, props: [String: String], events: ExactNativeEvents) throws -> ExactNativeInstance {
        guard let a = adapter(m, extensionName) else { throw ExactNativeRefusal("no extension \(extensionName) for \(tag)") }
        return try a.view(tag, id: id, props: props, events: events)
    }

    /// The session window's angle changed: each adapter made in `m`'s session is told.
    static func orientation(_ m: ExactModule, _ angle: Double) {
        for a in (made[ObjectIdentifier(m)] ?? [:]).values { a.orientation(angle) }
    }

    /// A synchronous form (`{"extension", "sync", "args"}`); `nil` for a request that names no extension.
    static func call(_ m: ExactModule, _ request: [String: Any]) throws -> [String: Any]? {
        guard let extensionName = request["extension"] as? String else { return nil }
        guard let a = adapter(m, extensionName), let name = request["sync"] as? String else {
            return ["error": "NotSupportedError"]
        }
        return try a.call(name, request["args"] as? [String: Any] ?? [:])
    }

    /// A picture of `extension`'s `img` scheme (LLP 1133.019 D11 item 1),
    /// which the runner admitted: its adapter, made now if no call made it
    /// first, draws it.
    static func image(_ m: ExactModule, _ request: [String: Any], reply: @escaping (CGImage?, String?) -> Void) {
        guard let name = request["extension"] as? String, let a = adapter(m, name),
              let source = request["source"] as? String else { return reply(nil, "NotSupportedError") }
        a.image(source, width: request["width"] as? Int ?? 0, height: request["height"] as? Int ?? 0,
                fit: request["fit"] as? String ?? "fill", owned: request["owned"] as? Bool ?? false, reply: reply)
    }

    /// A presence check (LLP 1133.010 D13), by the extension whose
    /// interface declares the authenticator entry, made now if no call
    /// made it first.
    static func authenticate(_ m: ExactModule, _ request: [String: Any], _ accessControl: SecAccessControl,
                             reply: @escaping (AnyObject?, [String: Any]?) -> Void) {
        guard let name = exactAuthenticator, let a = adapter(m, name) else {
            return reply(nil, ["error": "NotSupportedError", "message": "this build composes no authenticator"])
        }
        a.evaluate(accessControl, request, reply: reply)
    }

    /// The session's module is going: each adapter finishes what it must,
    /// waited for a second at most on the main run loop, and goes with it.
    static func destroy(_ m: ExactModule) {
        let key = ObjectIdentifier(m)
        modules.removeAll { $0.module == nil || $0.module === m }
        let adapters = made.removeValue(forKey: key) ?? [:]
        sessions[key] = nil
        tables[key] = nil
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

/// A capture's claims on the host's audio session (LLP 1133.009 D7), through
/// the host table's `audio_claim` and `audio_release` (NativeHatches.swift).
enum AudioClaims {
    typealias EventFn = @convention(c) (UnsafeMutableRawPointer?, UInt32) -> Void
    /// `audio_claim(host, mode, modeLen, options, background, event, ctx)` → claim (0: refused).
    typealias ClaimFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UInt64, UInt32, EventFn, UnsafeMutableRawPointer?) -> UInt64
    typealias ReleaseFn = @convention(c) (UnsafeMutableRawPointer?, UInt64) -> Void
    final class Box {
        let events: (String) -> Void
        init(_ events: @escaping (String) -> Void) { self.events = events }
    }
    /// `AVAudioSession.CategoryOptions` by name, as the host's bits read them.
    static let options = ["mixWithOthers", "duckOthers", "allowBluetoothHFP", "allowBluetoothA2DP", "defaultToSpeaker",
                          "bluetoothHighQualityRecording", "farFieldInput", "overrideMutedMicrophoneInterruption"]

    /// The release, or nil when the host refused the claim; a host with no
    /// audio session owner composes nothing, and the claim is empty.
    static func claim(_ h: ExactHatches, mode: String?, options: [String], background: Bool, events: @escaping (String) -> Void) -> (() -> Void)? {
        guard let claimFn = h.audioClaimFn, let releaseFn = h.audioReleaseFn else { return {} }
        let bits = options.reduce(UInt64(0)) { bits, name in Self.options.firstIndex(of: name).map { bits | 1 << UInt64($0) } ?? bits }
        let box = Unmanaged.passRetained(Box(events)), word = Array((mode ?? "").utf8)
        let id = word.withUnsafeBufferPointer { claimFn(h.host, $0.baseAddress, UInt32($0.count), bits, background ? 1 : 0, audioEvent, box.toOpaque()) }
        guard id != 0 else { box.release(); return nil }
        let host = h.host
        return { releaseFn(host, id); box.release() }
    }
}

/// A package's call of the app's handler (LLP 1133.016 D15 item 1), through
/// the host table's `invoke` (NativeHatches.swift): the runner admits and
/// runs it and answers JSON, given to `reply` on the main thread after the
/// call that asked has returned.
enum Invocations {
    /// `invoke(host, body, len, reply, ctx)`: `reply(ctx, answer, len)` once.
    typealias ReplyFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32) -> Void
    typealias InvokeFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, ReplyFn, UnsafeMutableRawPointer?) -> Void
    final class Box {
        let reply: ([String: Any]) -> Void
        init(_ reply: @escaping ([String: Any]) -> Void) { self.reply = reply }
    }

    static func invoke(_ h: ExactHatches?, _ body: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        guard let h, let fn = h.invokeFn, let data = try? JSONSerialization.data(withJSONObject: body) else {
            return reply(["error": "NotSupportedError", "message": "this host runs no handler for a package"])
        }
        let box = Unmanaged.passRetained(Box(reply)), bytes = [UInt8](data)
        bytes.withUnsafeBufferPointer { fn(h.host, $0.baseAddress, UInt32($0.count), invokeReply, box.toOpaque()) }
    }
}

private let invokeReply: Invocations.ReplyFn = { ctx, bytes, length in
    guard let ctx else { return }
    let box = Unmanaged<Invocations.Box>.fromOpaque(ctx).takeRetainedValue()
    let data = bytes.map { Data(bytes: $0, count: Int(length)) } ?? Data()
    let answer = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] ?? ["error": "OperationError", "message": "the runner's answer was not JSON"]
    // After the call that asked has returned, as a handler's answer always is on the web.
    DispatchQueue.main.async { box.reply(answer) }
}

/// The host's word for an audio session event, to the claim's `events`.
private let audioEvent: AudioClaims.EventFn = { ctx, code in
    guard let ctx else { return }
    let box = Unmanaged<AudioClaims.Box>.fromOpaque(ctx).takeUnretainedValue()
    box.events(["", "interrupted", "resumed", "reset", "route"][Int(min(code, 4))])
}

#if canImport(ExactPackage)
// Only an artifact that composes packages has their seam to compile against.
extension ExactExtensions {
    /// The context of extension `name`'s implementation in `m`'s session
    /// (ExactPackage.swift): the session's clock, journal, track table,
    /// files, the host's audio session, its canvases' inputs and the window's angle.
    static func context(_ m: ExactModule, _ name: String, session: String) -> ExactPackageContext {
        let module = m.context, tracks = tables[ObjectIdentifier(m)] as? ExactTracks ?? ExactTracks()
        tables[ObjectIdentifier(m)] = tracks
        return ExactPackageContext(
            agent: module.agent && !real.contains(name), session: session,
            changed: { [weak module] topic in module?.changed(topic) },
            log: { [weak module] line in module?.diagnostics.log(line) },
            now: { [weak module] in module?.now() ?? 0 },
            tracks: tracks,
            file: { [weak module] path in module.flatMap { file(path, $0) } },
            audioSession: ExactAudioSession { [weak m] mode, options, background, events in
                m?.hatches.flatMap { AudioClaims.claim($0, mode: mode, options: options, background: background, events: events) }
            },
            picture: { [weak m] kind, source in m?.hatches?.picture(kind, source) },
            surfaces: ExactSurfaces { [weak m] surface in
                guard m?.hatches?.hasSurface(surface) == true else { return nil }
                return ExactSurfaceInput(surface: surface) { [weak m] sensor, values, dropped, at, ended in
                    m?.hatches?.sensor(surface, sensor, values, dropped: dropped, at: at, ended: ended)
                }
            },
            orientation: { [weak m] in m?.hatches?.window?.angle ?? 0 },
            view: { [weak module] in module?.sessionView as? ExactPackagePlatformView },
            invoke: { [weak m] handle, handler, args, reply in
                let body: [String: Any] = ["extension": name, "handle": handle, "handler": handler, "args": args]
                Invocations.invoke(m?.hatches, body, reply: reply)
            },
            emit: { [weak module] event, args in module?.emit(name, event, args) },
            activation: { [weak module] event, args, navigate in
                module?.activation(name, event, args, navigate: navigate) ?? [:]
            },
            wake: { [weak module] event, args, id, done in
                ended["\(ObjectIdentifier(m).hashValue) \(name) \(id)"] = done
                module?.wake(name, event, args, id: id)
            },
            wakes: ExactWakes { [weak module] key, request in
                var request = request
                // The host's store keeps the wall clock; the session's may be the agent's.
                if let at = request?["at"] as? Double, let now = module?.now() { request?["at"] = at - now + Date().timeIntervalSince1970 * 1000 }
                let body: [String: Any] = ["package": name, "key": key, "request": request ?? NSNull()]
                NotificationCenter.default.post(name: Notification.Name("exact.wakes.request"), object: body as NSDictionary)
            },
            transfers: ExactTransfers { op, args in
                let box = NSMutableDictionary(dictionary: args)
                box["op"] = op
                box["package"] = name
                NotificationCenter.default.post(name: Notification.Name("exact.transfers"), object: box)
                return box["reply"]
            })
    }
}
#endif
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

    /// Raise `extensionName`'s wake event `name` for the platform object
    /// `id` (LLP 1133.017 D8), through `changed` as an emitted event goes;
    /// the runner tells the extension `{"woke": id, "outcome"}` at its end.
    public func wake(_ extensionName: String, _ name: String, _ args: [Any], id: String) {
        let body: [String: Any] = ["extension": extensionName, "event": name, "args": args, "id": id]
        guard JSONSerialization.isValidJSONObject(body), let json = try? JSONSerialization.data(withJSONObject: body) else {
            diagnostics.log("\(extensionName).\(name): its arguments are not JSON")
            return
        }
        changed("exact-wake:" + String(decoding: json, as: UTF8.self))
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

private final class WeakModule {
    weak var module: ExactModule?
    init(_ module: ExactModule) { self.module = module }
}

/// An emitted event's arguments on their way to the main thread.
private final class ExactEmitted: @unchecked Sendable {
    let args: [Any]
    init(_ args: [Any]) { self.args = args }
}
