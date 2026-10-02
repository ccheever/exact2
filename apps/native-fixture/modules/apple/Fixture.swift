// The native-module fixture (LLP 1024 D8) on Apple hosts: a coloured box the
// smoke can drive through every part of the seam. `exact-fixture` paints its
// colour with a layer background, which the ordinary `cacheDisplay` capture
// does not see, so its colour in a screenshot is the tokened snapshot's; it
// echoes each props object it accepts as a `message`, fires all nine events
// when `emit` changes, refuses `reject=true`, and after `destroy` calls back
// from a background thread (the host must drop it). `exact-plain` draws its
// colour in `draw(_:)`, which the ordinary capture does see. Neither takes
// hits: an agent `tap` lands on the node's own view.
import Foundation
#if os(macOS)
import AppKit
#else
import UIKit
#endif

final class FixtureModule: ExactModule {
    override class var views: [String: ExactNativeFactory] {
        ["exact-fixture": ExactNativeFactory(snapshot: true) { props, events in try FixtureBox(props: props, events: events) },
         "exact-plain": ExactNativeFactory { props, events in PlainBox(props: props, events: events) }]
    }
}
let exactModule: ExactModule.Type = FixtureModule.self

private func rgb(_ hex: String?) -> (CGFloat, CGFloat, CGFloat) {
    guard let hex, hex.hasPrefix("#"), hex.count == 7, let v = UInt32(hex.dropFirst(), radix: 16) else { return (0.5, 0.5, 0.5) }
    return (CGFloat((v >> 16) & 0xff) / 255, CGFloat((v >> 8) & 0xff) / 255, CGFloat(v & 0xff) / 255)
}

private func echo(_ props: [String: String]) -> String {
    let data = (try? JSONSerialization.data(withJSONObject: props, options: [.sortedKeys, .withoutEscapingSlashes])) ?? Data()
    return "props:" + String(decoding: data, as: UTF8.self)
}

#if os(macOS)
private final class Passive: NSView {
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
}
private final class Drawn: NSView {
    var tint = (CGFloat(0.5), CGFloat(0.5), CGFloat(0.5))
    override func hitTest(_ point: NSPoint) -> NSView? { subviews.isEmpty ? nil : super.hitTest(point) }
    override func draw(_ dirtyRect: NSRect) {
        NSColor(srgbRed: tint.0, green: tint.1, blue: tint.2, alpha: 1).setFill()
        bounds.fill()
    }
}
#else
private final class Passive: UIView {}
private final class Drawn: UIView {
    var tint = (CGFloat(0.5), CGFloat(0.5), CGFloat(0.5))
    override func draw(_ rect: CGRect) {
        UIColor(red: tint.0, green: tint.1, blue: tint.2, alpha: 1).setFill()
        UIRectFill(bounds)
    }
}
#endif

final class FixtureBox: ExactNativeInstance {
    private let box = Passive(frame: .zero)
    private var tint = (CGFloat(0.5), CGFloat(0.5), CGFloat(0.5))
    private var emit = "0"

    init(props: [String: String], events: ExactNativeEvents) throws {
        if props["reject"] == "true" { throw ExactNativeRefusal("reject=true") }
        super.init(events: events)
        #if os(macOS)
        box.wantsLayer = true
        #else
        box.isUserInteractionEnabled = false
        #endif
        apply(props)
        emit = props["emit"] ?? "0"
        events.message(echo(props))
        events.load()
    }

    override var view: ExactNativeView { box }

    private func apply(_ props: [String: String]) {
        events.intrinsicSize(CGSize(width: 200, height: 96))
        tint = rgb(props["tint"])
        #if os(macOS)
        box.layer?.backgroundColor = CGColor(srgbRed: tint.0, green: tint.1, blue: tint.2, alpha: 1)
        #else
        box.backgroundColor = UIColor(red: tint.0, green: tint.1, blue: tint.2, alpha: 1)
        #endif
    }

    override func setProps(_ props: [String: String]) throws {
        if props["reject"] == "true" { throw ExactNativeRefusal("reject=true") }
        apply(props)
        events.message(echo(props))
        let next = props["emit"] ?? "0"
        guard next != emit else { return }
        emit = next
        guard (Int(next) ?? 0) > 0 else { return }
        // Every event, from a background thread, in order: the host copies,
        // hops to its presenter and enters the runner after the batch.
        let events = self.events
        DispatchQueue.global().async {
            events.press(); events.change("changed"); events.hover(true); events.focus(); events.blur()
            events.key("Enter"); events.submit(); events.load(); events.message("hello")
        }
    }

    override func snapshot() throws -> Data {
        let size = box.bounds.size
        guard size.width > 0, size.height > 0 else { throw ExactNativeRefusal("no bounds yet") }
        let w = Int(size.width * 2), h = Int(size.height * 2)
        guard let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                                  space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
        else { throw ExactNativeRefusal("no context") }
        ctx.setFillColor(CGColor(srgbRed: tint.0, green: tint.1, blue: tint.2, alpha: 1))
        ctx.fill(CGRect(x: 0, y: 0, width: w, height: h))
        guard let image = ctx.makeImage() else { throw ExactNativeRefusal("no image") }
        #if os(macOS)
        let rep = NSBitmapImageRep(cgImage: image)
        guard let png = rep.representation(using: .png, properties: [:]) else { throw ExactNativeRefusal("no PNG") }
        return png
        #else
        guard let png = UIImage(cgImage: image).pngData() else { throw ExactNativeRefusal("no PNG") }
        return png
        #endif
    }

    override func destroy() {
        // A late callback, as a PTY's reader thread would make: the host has
        // already invalidated this instance's nonce and must drop it.
        let events = self.events
        DispatchQueue.global().asyncAfter(deadline: .now() + 0.1) { events.message("late") }
    }
}

#if os(macOS)
private final class FixtureEditor: NSTextView, NSTextViewDelegate {
    var events: ExactNativeEvents?
    override func becomeFirstResponder() -> Bool {
        let accepted = super.becomeFirstResponder()
        if accepted { events?.focus() }
        return accepted
    }
    override func resignFirstResponder() -> Bool {
        let accepted = super.resignFirstResponder()
        if accepted { events?.blur() }
        return accepted
    }
    func textDidChange(_ notification: Notification) { events?.change(string) }
    override func keyDown(with event: NSEvent) {
        events?.key(event.keyCode == 51 ? "Backspace" : event.characters ?? "")
        super.keyDown(with: event)
    }
}
#endif

final class PlainBox: ExactNativeInstance {
    private let box = Drawn(frame: .zero)
    #if os(macOS)
    private var editor: FixtureEditor?
    override var focusTarget: ExactNativeView? { editor }
    override func agentInput(_ input: ExactNativeInput) throws {
        guard let editor, editor.window?.firstResponder === editor else { throw ExactNativeRefusal("no focused fixture editor") }
        switch input {
        case .text(let text):
            editor.selectAll(nil)
            editor.insertText(text, replacementRange: editor.selectedRange())
        case .key(let key, let phase):
            // Exercise real AppKit commands; unsupported chords refuse before delivery.
            let chars: String, code: UInt16
            switch key {
            case "Backspace": chars = "\u{7f}"; code = 51
            case "ArrowLeft": chars = "\u{f702}"; code = 123
            case "ArrowRight": chars = "\u{f703}"; code = 124
            case "Enter": chars = "\r"; code = 36
            default: throw ExactNativeRefusal("fixture does not support key \(key)")
            }
            let phases: [NSEvent.EventType] = phase == "up" ? [.keyUp] : phase == "down" ? [.keyDown] : [.keyDown, .keyUp]
            for type in phases {
                guard let event = NSEvent.keyEvent(with: type,
                    location: .zero, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                    windowNumber: editor.window!.windowNumber, context: nil, characters: chars,
                    charactersIgnoringModifiers: chars, isARepeat: false, keyCode: code) else { throw ExactNativeRefusal("no key event") }
                if type == .keyUp { editor.keyUp(with: event) } else { editor.keyDown(with: event) }
            }
        }
    }
    #endif

    init(props: [String: String], events: ExactNativeEvents) {
        super.init(events: events)
        #if os(iOS)
        box.isUserInteractionEnabled = false
        box.contentMode = .redraw
        #endif
        #if os(macOS)
        if props["customInput"] == "true" {
            let editor = FixtureEditor(frame: NSRect(x: 0, y: 0, width: 200, height: 48))
            editor.events = events
            editor.delegate = editor
            editor.isRichText = false
            editor.autoresizingMask = [.width, .height]
            box.addSubview(editor)
            self.editor = editor
        }
        #endif
        apply(props)
    }

    override var view: ExactNativeView { box }

    private func apply(_ props: [String: String]) {
        events.intrinsicSize(props["natural"] == "false" ? nil : CGSize(width: 120, height: props["expanded"] == "true" ? 64 : 32))
        box.tint = rgb(props["tint"])
    }

    override func setProps(_ props: [String: String]) throws {
        apply(props)
        #if os(macOS)
        box.needsDisplay = true
        #else
        box.setNeedsDisplay()
        #endif
    }
}
