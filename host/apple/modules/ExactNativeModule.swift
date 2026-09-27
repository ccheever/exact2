// The module side of the native-module table (@ref LLP 1024 D4), compiled
// into the app's one artifact, `libexact_modules.dylib`, with the app's own
// `modules/apple/*.swift` by `host/apple/build.mjs`. The host side, and the
// table's layout, is `Sources/ExactKit/NativeModule.swift`.
//
// An app's module source declares its roster as a table of factories:
//
//     let exactNativeModules: [String: ExactNativeFactory] = [
//         "photo-editor": ExactNativeFactory { props, events in try PhotoEditor(props: props, events: events) },
//     ]
//
// and each instance subclasses `ExactNativeInstance`: a platform view, a
// props replacement that may refuse (the last accepted stays), an optional
// PNG snapshot, and `destroy`. `events` may be called from any thread.
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

/// A refusal with a message the host logs and reports in `tree`.
public struct ExactNativeRefusal: Error, CustomStringConvertible {
    public let description: String
    public init(_ message: String) { description = message }
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

/// A roster entry: how to make an instance, and whether it answers snapshots.
public struct ExactNativeFactory {
    public let snapshot: Bool
    public let make: ([String: String], ExactNativeEvents) throws -> ExactNativeInstance
    public init(snapshot: Bool = false, make: @escaping ([String: String], ExactNativeEvents) throws -> ExactNativeInstance) {
        self.snapshot = snapshot
        self.make = make
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

private let create: @convention(c) (UnsafePointer<UInt8>?, UInt32, UnsafePointer<UInt8>?, UInt32, ExactNativeEventFn?, ExactNativeReplyFn?, UnsafeMutableRawPointer?, UInt32, UnsafeMutablePointer<UInt8>?, UInt32) -> UnsafeMutableRawPointer? = { tag, tagLength, json, jsonLength, event, reply, ctx, nonce, out, capacity in
    let name = tag.map { String(decoding: UnsafeBufferPointer(start: $0, count: Int(tagLength)), as: UTF8.self) } ?? ""
    guard let factory = exactNativeModules[name] else { write("no factory for \(name)", out, capacity); return nil }
    guard let event else { write("no event callback", out, capacity); return nil }
    do {
        let instance = try factory.make(try props(json, jsonLength), ExactNativeEvents(fn: event, ctx: ctx, nonce: nonce))
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

/// The ABI major this artifact was built against; the host refuses others.
private let major: UInt32 = 1

private let table: UnsafeMutableRawPointer = {
    let roster = "{" + exactNativeModules.keys.sorted().map { tag in
        "\"\(tag)\":{\"snapshot\":\(exactNativeModules[tag]!.snapshot)}"
    }.joined(separator: ",") + "}"
    let size = 72
    let t = UnsafeMutableRawPointer.allocate(byteCount: size, alignment: 8)
    t.initializeMemory(as: UInt8.self, repeating: 0, count: size)
    t.storeBytes(of: major, as: UInt32.self)
    t.storeBytes(of: UInt32(size), toByteOffset: 4, as: UInt32.self)
    t.storeBytes(of: UnsafeRawPointer(strdup(roster)), toByteOffset: 8, as: UnsafeRawPointer?.self)
    t.storeBytes(of: unsafeBitCast(create, to: UnsafeRawPointer.self), toByteOffset: 16, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(platformView, to: UnsafeRawPointer.self), toByteOffset: 24, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(setProps, to: UnsafeRawPointer.self), toByteOffset: 32, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(snapshot, to: UnsafeRawPointer.self), toByteOffset: 40, as: UnsafeRawPointer.self)
    t.storeBytes(of: unsafeBitCast(destroy, to: UnsafeRawPointer.self), toByteOffset: 48, as: UnsafeRawPointer.self)
    return t
}()

@_cdecl("exact_native_abi")
public func exactNativeAbi() -> UnsafeRawPointer { UnsafeRawPointer(table) }
