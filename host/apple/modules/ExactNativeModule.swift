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
    private let host: UnsafeMutableRawPointer?
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
    /// The whole props object, replaced; throw to refuse it.
    open func setProps(_ props: [String: String]) throws {}
    /// PNG bytes of the view, for a tag whose factory sets `snapshot`.
    open func snapshot() throws -> Data { throw ExactNativeRefusal("no snapshot") }
    /// Last call. The events object is dead to the host after this.
    open func destroy() {}
}

/// A roster entry: how to make an instance from the session's module, and
/// whether it answers snapshots.
public struct ExactNativeFactory {
    public let snapshot: Bool
    public let make: (ExactModule, [String: String], ExactNativeEvents) throws -> ExactNativeInstance
    public init(snapshot: Bool = false, make: @escaping (ExactModule, [String: String], ExactNativeEvents) throws -> ExactNativeInstance) {
        self.snapshot = snapshot
        self.make = make
    }
    /// A view that needs nothing from the module.
    public init(snapshot: Bool = false, make: @escaping ([String: String], ExactNativeEvents) throws -> ExactNativeInstance) {
        self.init(snapshot: snapshot) { _, props, events in try make(props, events) }
    }
    /// A view of the app's module, typed: `ExactNativeFactory(for: Recorder.self)
    /// { recorder, props, events in … }`. The session's module is always the
    /// app's `exactModule`; another type is refused by name.
    public init<M: ExactModule>(for module: M.Type, snapshot: Bool = false, make: @escaping (M, [String: String], ExactNativeEvents) throws -> ExactNativeInstance) {
        self.init(snapshot: snapshot) { owner, props, events in
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

/// The ABI major this artifact was built against; the host refuses others.
private let major: UInt32 = 2

private let table: UnsafeMutableRawPointer = {
    let text = "{" + roster.keys.sorted().map { tag in
        "\"\(tag)\":{\"snapshot\":\(roster[tag]!.snapshot)}"
    }.joined(separator: ",") + "}"
    let size = 104
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
    return t
}()

@_cdecl("exact_native_abi")
public func exactNativeAbi() -> UnsafeRawPointer { UnsafeRawPointer(table) }
