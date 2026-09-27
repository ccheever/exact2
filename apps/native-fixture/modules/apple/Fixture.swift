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
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
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

final class PlainBox: ExactNativeInstance {
    private let box = Drawn(frame: .zero)

    init(props: [String: String], events: ExactNativeEvents) {
        super.init(events: events)
        #if os(iOS)
        box.isUserInteractionEnabled = false
        box.contentMode = .redraw
        #endif
        box.tint = rgb(props["tint"])
    }

    override var view: ExactNativeView { box }

    override func setProps(_ props: [String: String]) throws {
        box.tint = rgb(props["tint"])
        #if os(macOS)
        box.needsDisplay = true
        #else
        box.setNeedsDisplay()
        #endif
    }
}
