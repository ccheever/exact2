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
// - `{"extension", "source"|"send", "args", "preferred"?, "origin"?}`: a
//   call of one of its names, `origin` when a tool or a wake made it;
// - `{"extension", "release", "args"?}`: a live source with no reader left,
//   or, with `args`, an argument group of one (LLP 1133.009 D10 item 12);
// - `{"extension", "preferred", "permissions"}`: the agent's preferred
//   values and the runner's permission states, for an implementation bound
//   under the agent;
// - `{"extension", "dispose"}`: handles a dropped reply carried (item 10);
// - `{"extension", "foreground"}`: the app went to the foreground or left
//   it (item 3);
// - `{"extension", "view", "displayed"|"bound", "origin"}`: a package view's
//   displayed state or binding prop changed (LLP 1133.008 D8 item 2);
// - `{"extension", "sync", "args"}`, through `call`: data code's
//   synchronous form.
//
// A picture of a package's `img` scheme comes through the table's `image`
// entry instead (`ExactExtensions.image`), since its answer is an image.
//
// An error is answered as `{"error": code}`. At the session's end each
// adapter's `destroy` is called and waited for, a second at most (item 1).
import Foundation
import CoreGraphics
import Security
#if canImport(ExactPackage)
import ExactPackage
#endif

/// One extension's adapter, as the build writes it.
open class ExactExtensionAdapter {
    public init() {}
    open func later(_ name: String, _ args: [String: Any], preferred: Any?, origin: String, reply: ExactReply) {
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
    open func view(_ tag: String, id: String, props: [String: String], events: ExactNativeEvents) throws -> ExactNativeInstance {
        throw ExactNativeRefusal("no view \(tag)")
    }
    open func displayed(_ id: String, _ shown: Bool, origin: String) {}
    open func bound(_ id: String, _ prop: String, origin: String) {}
    open func image(_ source: String, width: Int, height: Int, fit: String, owned: Bool, reply: @escaping (CGImage?, String?) -> Void) {
        reply(nil, "NotSupportedError")
    }
    open func permissions(_ states: [String: String]) {}
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
        } else if let shown = request["foreground"] as? Bool {
            a.foreground(shown)
            reply.send([:])
        } else if let name = (request["source"] ?? request["send"]) as? String {
            a.later(name, args ?? [:], preferred: request["preferred"], origin: request["origin"] as? String ?? "app", reply: reply)
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
    /// files and the host's audio session.
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
            picture: { [weak m] kind, source in m?.hatches?.picture(kind, source) })
    }
}
#endif
