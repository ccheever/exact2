// A module's views on the module side of the native-module table (@ref LLP
// 1024 D4; LLP 1133.001 D1): what a view instance is, the roster's factories,
// and the table's view entries (`ExactNativeViewABI`), which
// ExactNativeModule.swift lays out. The roster is the app's own module's
// `views` and the views its packages serve (`exactPackageViews`, written by
// host/apple/compose.mjs), each made by the package's implementation in the
// session.
import Foundation
#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// The nine events (kernel `EventKind` ordinals) and host-only size reports.
public final class ExactNativeEvents: @unchecked Sendable {
    let fn: ExactNativeEventFn
    let ctx: UnsafeMutableRawPointer?
    let nonce: UInt32
    init(fn: @escaping ExactNativeEventFn, ctx: UnsafeMutableRawPointer?, nonce: UInt32) { self.fn = fn; self.ctx = ctx; self.nonce = nonce }
    /// Event `kind` with its text; a package's view sends through this (compose.mjs).
    func send(_ kind: UInt32, _ text: String = "") {
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
    /// Preferred content size in points, independent of the assigned frame.
    /// Report after creation and whenever content changes; nil forgets it.
    /// Both axes must be finite and positive. CSS still owns the final frame;
    /// no aspect ratio is inferred. This is not a constrained measure callback.
    /// Any thread; stale instances are ignored, and a turn's reports coalesce.
    public func intrinsicSize(_ size: CGSize?) {
        guard let size else { send(9); return }
        send(9, "\(size.width),\(size.height)")
    }
}

/// Agent-only input. Text replaces the widget's value, as standard `type`
/// does; keys use the agent's web-named chord and optional down/up phase.
/// A nil phase is one complete key press (down followed by up).
/// Throw before changing anything for an unsupported input. Human input and
/// IME continue through the platform responder, never through this hook.
public enum ExactNativeInput {
    case text(String)
    case key(String, phase: String?)
}

/// One instance of a module tag.
open class ExactNativeInstance {
    public let events: ExactNativeEvents
    public init(events: ExactNativeEvents) { self.events = events }
    /// The view the host puts in the node's box; it fills the box, and
    /// observes its own bounds.
    open var view: ExactNativeView { fatalError("\(type(of: self)) has no view") }
    #if os(iOS) || os(tvOS)
    /// A native screen (LLP 1075.003 §3.6): a controller whose view is
    /// `view`, contained in the route's controller as a child while it shows.
    open var controller: UIViewController? { nil }
    #endif
    /// Optional focus destination: the view itself or an attached descendant.
    /// The host owns first-responder changes; return nil to refuse focus.
    open var focusTarget: ExactNativeView? { nil }
    /// Synchronous, main-thread agent input, after the host focuses this view.
    /// Return only after delivery; do not retain a request for later delivery.
    open func agentInput(_ input: ExactNativeInput) throws {
        throw ExactNativeRefusal("native view does not support agent input")
    }
    /// The whole props object, replaced; throw to refuse it.
    open func setProps(_ props: [String: String]) throws {}
    /// PNG bytes of the view, for a tag whose factory sets `snapshot`.
    open func snapshot() throws -> Data { throw ExactNativeRefusal("no snapshot") }
    /// Last call. The events object is dead to the host after this.
    open func destroy() {}
    /// Reuse (a factory with `reuse`, LLP 1068 §4.8): the instance is now as
    /// if created with no props; throw to refuse, and the host destroys it.
    /// The next `setProps` is a first mount, and `events.load()` follows once
    /// nothing of the last row shows. Events sent before that mount are dropped.
    open func prepareForReuse() throws { throw ExactNativeRefusal("no reuse") }
}

#if os(iOS) || os(tvOS)
/// A module view that is a whole screen (LLP 1075.003 §3.6): `screen`'s view
/// fills the node's box, and `screen` is a child of the route's controller,
/// with UIKit's appearance, safe-area and trait propagation.
open class ExactNativeScreen: ExactNativeInstance {
    public let screen: UIViewController
    public init(screen: UIViewController, events: ExactNativeEvents) {
        self.screen = screen
        super.init(events: events)
    }
    open override var view: ExactNativeView { screen.view }
    open override var controller: UIViewController? { screen }
}
#endif

/// A roster entry: how to make an instance from the session's module,
/// whether it answers snapshots, whether a list may reuse it (LLP 1068
/// §4.8), and whether it joins the first frame its node shows in.
///
/// Every other tag's view attaches after the first frame has painted (LLP
/// 1024 D3), so a slow view (a map) never delays it. `firstFrame` is for a
/// cheap view that must be there from the first frame, such as a boot
/// splash: the artifact loads, and its view and the session's module are
/// made, before that frame commits, so their time is first pixel's. Its
/// `context.frames` and `after` work from then; the module's hatches are
/// called from first pixel on. The build reads the flag from this roster.
public struct ExactNativeFactory {
    public let snapshot: Bool
    public let reuse: Bool
    public let firstFrame: Bool
    public let make: (ExactModule, [String: String], ExactNativeEvents) throws -> ExactNativeInstance
    /// A package's view (LLP 1133.001 D1), made with its node's id in the
    /// session, by which its implementation hears whether it is displayed.
    let made: ((ExactModule, String, [String: String], ExactNativeEvents) throws -> ExactNativeInstance)?
    public init(snapshot: Bool = false, reuse: Bool = false, firstFrame: Bool = false, make: @escaping (ExactModule, [String: String], ExactNativeEvents) throws -> ExactNativeInstance) {
        self.snapshot = snapshot
        self.reuse = reuse
        self.firstFrame = firstFrame
        self.make = make
        made = nil
    }
    /// A view that needs nothing from the module.
    public init(snapshot: Bool = false, reuse: Bool = false, firstFrame: Bool = false, make: @escaping ([String: String], ExactNativeEvents) throws -> ExactNativeInstance) {
        self.init(snapshot: snapshot, reuse: reuse, firstFrame: firstFrame) { _, props, events in try make(props, events) }
    }
    /// A view of the app's module, typed: `ExactNativeFactory(for: Recorder.self)
    /// { recorder, props, events in … }`. The session's module is always the
    /// app's `exactModule`; another type is refused by name.
    public init<M: ExactModule>(for module: M.Type, snapshot: Bool = false, reuse: Bool = false, firstFrame: Bool = false, make: @escaping (M, [String: String], ExactNativeEvents) throws -> ExactNativeInstance) {
        self.init(snapshot: snapshot, reuse: reuse, firstFrame: firstFrame) { owner, props, events in
            guard let typed = owner as? M else { throw ExactNativeRefusal("the session's module is \(type(of: owner)), not \(M.self)") }
            return try make(typed, props, events)
        }
    }
    /// A package's view, by the session's module, the node's id, its props and events.
    init(package make: @escaping (ExactModule, String, [String: String], ExactNativeEvents) throws -> ExactNativeInstance) {
        snapshot = false; reuse = false; firstFrame = false
        self.make = { owner, props, events in try make(owner, "", props, events) }
        made = make
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

/// A view's instance, made by its factory with the node's id where the host gave one.
private func made(_ owner: UnsafeMutableRawPointer?, _ view: String?, _ tag: UnsafePointer<UInt8>?, _ tagLength: UInt32, _ json: UnsafePointer<UInt8>?, _ jsonLength: UInt32, _ event: ExactNativeEventFn?, _ reply: ExactNativeReplyFn?, _ ctx: UnsafeMutableRawPointer?, _ nonce: UInt32, _ out: UnsafeMutablePointer<UInt8>?, _ capacity: UInt32) -> UnsafeMutableRawPointer? {
    let write = ExactNativeViewABI.write
    let name = tag.map { String(decoding: UnsafeBufferPointer(start: $0, count: Int(tagLength)), as: UTF8.self) } ?? ""
    guard let factory = ExactNativeViewABI.roster[name] else { write("no factory for \(name)", out, capacity); return nil }
    guard let owner = owner.map({ Unmanaged<ExactModule>.fromOpaque($0).takeUnretainedValue() }) else { write("no module instance", out, capacity); return nil }
    guard let event else { write("no event callback", out, capacity); return nil }
    do {
        let events = ExactNativeEvents(fn: event, ctx: ctx, nonce: nonce), given = try props(json, jsonLength)
        let instance = try factory.made.map { try $0(owner, view ?? "", given, events) } ?? factory.make(owner, given, events)
        Tambers.made(instance.view, by: owner)
        return Unmanaged.passRetained(Handle(instance, reply: reply, ctx: ctx, nonce: nonce)).toOpaque()
    } catch {
        write(String(describing: error), out, capacity)
        return nil
    }
}

/// The table's view entries, by its offsets (ExactNativeModule.swift).
enum ExactNativeViewABI {
    /// The app's own views, then its packages' (the bake refuses a tag in both).
    static let roster: [String: ExactNativeFactory] = exactModule.views.merging(exactPackageViews) { own, _ in own }

    static func write(_ message: String, _ out: UnsafeMutablePointer<UInt8>?, _ capacity: UInt32) {
        guard let out, capacity > 0 else { return }
        let bytes = Array(message.utf8.prefix(Int(capacity) - 1))
        for (i, b) in bytes.enumerated() { out[i] = b }
        out[bytes.count] = 0
    }

    // 16 create(module, tag, tagLen, props, propsLen, event, reply, ctx, nonce, err, errCap) → handle
    static let create: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UnsafePointer<UInt8>?, UInt32, ExactNativeEventFn?, ExactNativeReplyFn?, UnsafeMutableRawPointer?, UInt32, UnsafeMutablePointer<UInt8>?, UInt32) -> UnsafeMutableRawPointer? = { owner, tag, tagLength, json, jsonLength, event, reply, ctx, nonce, out, capacity in
        made(owner, nil, tag, tagLength, json, jsonLength, event, reply, ctx, nonce, out, capacity)
    }

    // 224 create_view(module, view, tag, …): `create` with the node's id, which a package's view is made with.
    static let createView: @convention(c) (UnsafeMutableRawPointer?, UInt64, UnsafePointer<UInt8>?, UInt32, UnsafePointer<UInt8>?, UInt32, ExactNativeEventFn?, ExactNativeReplyFn?, UnsafeMutableRawPointer?, UInt32, UnsafeMutablePointer<UInt8>?, UInt32) -> UnsafeMutableRawPointer? = { owner, view, tag, tagLength, json, jsonLength, event, reply, ctx, nonce, out, capacity in
        made(owner, String(view), tag, tagLength, json, jsonLength, event, reply, ctx, nonce, out, capacity)
    }

    static let platformView: @convention(c) (UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer? = { raw in
        handle(raw).map { Unmanaged.passUnretained($0.instance.view).toOpaque() }
    }

    static let setProps: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UnsafeMutablePointer<UInt8>?, UInt32) -> Int32 = { raw, json, length, out, capacity in
        guard let h = handle(raw) else { return 1 }
        do { try h.instance.setProps(try props(json, length)); return 0 } catch {
            write(String(describing: error), out, capacity)
            return 1
        }
    }

    static let focusTarget: @convention(c) (UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer? = { raw in
        handle(raw)?.instance.focusTarget.map { Unmanaged.passUnretained($0).toOpaque() }
    }

    static let agentInput: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UnsafeMutablePointer<UInt8>?, UInt32) -> Int32 = { raw, json, length, out, capacity in
        do {
            guard let h = handle(raw) else { throw ExactNativeRefusal("no native instance") }
            let request = try props(json, length)
            let input: ExactNativeInput
            if let key = request["key"], request["text"] == nil {
                let phase = request["phase"]
                guard phase == nil || phase == "down" || phase == "up" else { throw ExactNativeRefusal("invalid key phase") }
                input = .key(key, phase: phase)
            } else if let text = request["text"], request["key"] == nil, request["phase"] == nil {
                input = .text(text)
            } else { throw ExactNativeRefusal("expected text or key") }
            try h.instance.agentInput(input)
            return 0
        } catch { write(String(describing: error), out, capacity); return 1 }
    }

    static let snapshot: @convention(c) (UnsafeMutableRawPointer?, UInt32) -> Void = { raw, token in
        guard let h = handle(raw), let reply = h.reply else { return }
        do {
            let png = [UInt8](try h.instance.snapshot())
            png.withUnsafeBufferPointer { reply(h.ctx, h.nonce, token, 0, $0.baseAddress, UInt32($0.count)) }
        } catch {
            let text = Array(String(describing: error).utf8)
            text.withUnsafeBufferPointer { reply(h.ctx, h.nonce, token, 2, $0.baseAddress, UInt32($0.count)) }
        }
    }

    static let destroy: @convention(c) (UnsafeMutableRawPointer?) -> Void = { raw in
        guard let raw else { return }
        let h = Unmanaged<Handle>.fromOpaque(raw)
        h.takeUnretainedValue().instance.destroy()
        h.release()
    }

    static let prepareForReuse: @convention(c) (UnsafeMutableRawPointer?) -> Int32 = { raw in
        guard let h = handle(raw) else { return 1 }
        do { try h.instance.prepareForReuse(); return 0 } catch { return 1 }
    }

    /// `platform_controller(handle) → UIViewController?`: a native screen's
    /// controller (the module keeps ownership), or nil for a plain view.
    static let platformController: @convention(c) (UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer? = { raw in
        #if os(iOS) || os(tvOS)
        handle(raw)?.instance.controller.map { Unmanaged.passUnretained($0).toOpaque() }
        #else
        nil
        #endif
    }
}
