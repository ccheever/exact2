#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// The parity page's border cases as AppKit draws them (`BorderParity`).
final class BorderParityMacTests: XCTestCase {
    private func render(_ style: NodeStyle, dark: Bool, page: CGColor) -> [UInt8] {
        _ = NSApplication.shared
        let p = Presenter()
        let node = NodeView(id: 1, kind: "view", presenter: p)
        node.frame = NSRect(x: 0, y: 0, width: 100, height: 70)
        node.appearance = NSAppearance(named: dark ? .darkAqua : .aqua)
        p.root.addSubview(node); p.views[1] = node
        node.applyStyle(style)
        let ctx = BorderParity.canvas(page)
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = NSGraphicsContext(cgContext: ctx, flipped: true)
        // What the layer's clip-path mask does on screen; `draw` alone does not.
        if let path = node.clipPath { ctx.addPath(path); ctx.clip() }
        // A capture's paint: on screen the box is the layer's where Core
        // Animation can say it (`BoxLayerMac.swift`, the window-server
        // parity smokes); captures and what it cannot say draw here.
        Capture.capturing = true
        node.draw(node.bounds)
        Capture.capturing = false
        NSGraphicsContext.restoreGraphicsState()
        return BorderParity.bytes(ctx)
    }

    /// The window server's picture of each style's node, as the parity smokes
    /// see a Mac window (its layer properties and sublayers where Core
    /// Animation says the box, `BoxLayerMac.swift`). Every node is a
    /// presenter's root in a 100×70 cell that clips as a window of its own
    /// would, so one window, one wait and one capture serve a whole page;
    /// nil where no window server answers (a process may read its own
    /// windows without permission).
    private func onScreen(_ styles: [NodeStyle], dark: Bool, page: CGColor) -> [[UInt8]]? {
        _ = NSApplication.shared
        let columns = min(styles.count, 10), rows = (styles.count + columns - 1) / columns
        let size = NSSize(width: 100 * columns, height: 70 * rows)
        let window = NSWindow(contentRect: NSRect(origin: NSPoint(x: 40, y: 40), size: size), styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.backgroundColor = NSColor(cgColor: page)
        window.appearance = NSAppearance(named: dark ? .darkAqua : .aqua)
        let grid = NSView(frame: NSRect(origin: .zero, size: size))
        window.contentView = grid
        var nodes: [NodeView] = [], presenters: [Presenter] = []
        for (i, style) in styles.enumerated() {
            let p = Presenter()
            // Cells from the top left, in the grid's bottom-up coordinates.
            p.root.frame = NSRect(x: CGFloat(i % columns) * 100, y: size.height - CGFloat(i / columns + 1) * 70, width: 100, height: 70)
            p.root.wantsLayer = true
            p.root.layer?.masksToBounds = true
            grid.addSubview(p.root)
            let node = NodeView(id: 1, kind: "view", presenter: p)
            node.frame = NSRect(x: 0, y: 0, width: 100, height: 70)
            p.root.addSubview(node); p.views[1] = node
            node.applyStyle(style)
            nodes.append(node); presenters.append(p)
        }
        window.orderFrontRegardless()
        defer { window.orderOut(nil); withExtendedLifetime(presenters) {} }
        for node in nodes { node.display() }
        CATransaction.flush()
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.15))
        typealias Create = @convention(c) (CGRect, UInt32, UInt32, UInt32) -> Unmanaged<CGImage>?
        // A sleeping display or a locked screen answers with an empty picture
        // (`Agent.emptyPicture`): no picture, as with no window server.
        guard let sym = dlsym(UnsafeMutableRawPointer(bitPattern: -2), "CGWindowListCreateImage"),
              let image = unsafeBitCast(sym, to: Create.self)(.null, 1 << 3, UInt32(window.windowNumber), 1 << 0)?.takeRetainedValue(),
              !Agent.emptyPicture(image)
        else { return nil }
        let scale = CGFloat(image.width) / size.width
        return styles.indices.map { i in
            let cell = CGRect(x: CGFloat(i % columns) * 100 * scale, y: CGFloat(i / columns) * 70 * scale, width: 100 * scale, height: 70 * scale)
            let ctx = BorderParity.canvas(page)
            ctx.saveGState()
            ctx.translateBy(x: 0, y: 70); ctx.scaleBy(x: 1, y: -1)
            ctx.interpolationQuality = .high
            ctx.draw(image.cropping(to: cell)!, in: CGRect(x: 0, y: 0, width: 100, height: 70))
            ctx.restoreGState()
            return BorderParity.bytes(ctx)
        }
    }

    /// A page's cases on screen, in the order `check` asks for them: the
    /// first ask, which names the page's appearance, pictures them all.
    private func onScreen(_ cases: [BorderParity.Case]) -> (NodeStyle, Bool, CGColor) -> [UInt8] {
        var pictures: [[UInt8]] = [], next = 0
        return { [self] _, dark, page in
            if pictures.isEmpty { pictures = onScreen(cases.map(\.style), dark: dark, page: page)! }
            defer { next += 1 }
            return pictures[next]
        }
    }

    /// On screen, where the box is layers: the same Chrome pictures. A
    /// Retina window's picture is resampled to Chrome's 1×, which moves every
    /// antialiased edge a little (the drawn cases measure 1–2.2 mean there),
    /// so the band is wider than the drawing tests'.
    func testEveryCaseMatchesChromeOnScreen() throws {
        guard onScreen([[:]], dark: false, page: CGColor(gray: 1, alpha: 1)) != nil else { throw XCTSkip("no window server picture (no window server, or its display is asleep or the screen locked)") }
        // The window server blends a translucent layer in the display's
        // space. sRGB and Display P3 share sRGB's transfer curve, so their
        // blends are Chrome's; a display that does not (an HDMI dummy's EDID
        // profile, a pure 1.96 gamma) lightens every translucent blend, a
        // gradient's fade 3.5/255 off, and the picture is the display's,
        // not the host's paint.
        let srgb = CGColorSpace(name: CGColorSpace.sRGB)!
        if let space = NSScreen.main?.colorSpace?.cgColorSpace,
           let gray = CGColor(colorSpace: srgb, components: [0.5, 0.5, 0.5, 1])?.converted(to: space, intent: .relativeColorimetric, options: nil)?.components,
           abs(gray[0] - 0.5) > 0.01 {
            throw XCTSkip("the display's transfer curve is not sRGB's (\(NSScreen.main?.localizedName ?? "?"): sRGB 0.5 is \(gray[0])); translucent blends on it are not Chrome's")
        }
        let band = (mean: 3.0, over: 4.0)
        var failures = BorderParity.check(flipped: false, dark: false, band: band, render: onScreen(BorderParity.cases(flipped: false)))
        failures += GradientParity.check(dark: false, band: band, render: onScreen(GradientParity.cases()))
        failures += GradientParity.check(dark: true, band: band, render: onScreen(GradientParity.cases()))
        XCTAssert(failures.isEmpty, failures.joined(separator: "\n"))
    }

    func testEveryBorderCaseMatchesChrome() {
        var failures = BorderParity.check(flipped: false, dark: false) { render($0, dark: $1, page: $2) }
        failures += BorderParity.check(flipped: true, dark: true) { render($0, dark: $1, page: $2) }
        XCTAssert(failures.isEmpty, failures.joined(separator: "\n"))
    }

    /// LLP 1066: the gradient page's cases (`GradientParity`), light then dark.
    func testEveryGradientCaseMatchesChrome() {
        var failures = GradientParity.check(dark: false) { render($0, dark: $1, page: $2) }
        failures += GradientParity.check(dark: true) { render($0, dark: $1, page: $2) }
        XCTAssert(failures.isEmpty, failures.joined(separator: "\n"))
    }
}
#endif
