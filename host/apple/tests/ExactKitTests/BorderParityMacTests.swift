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
    private func onScreen(_ styles: [NodeStyle], dark: Bool, page: CGColor, native: Bool = false) -> (scale: Int, pixels: [[UInt8]])? {
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
        let pictures = styles.indices.map { i in
            let cell = CGRect(x: CGFloat(i % columns) * 100 * scale, y: CGFloat(i / columns) * 70 * scale, width: 100 * scale, height: 70 * scale)
            let crop = image.cropping(to: cell)!
            if native { return BorderParity.pixels(crop).2 }
            let ctx = BorderParity.canvas(page)
            ctx.saveGState()
            ctx.translateBy(x: 0, y: 70); ctx.scaleBy(x: 1, y: -1)
            ctx.interpolationQuality = .high
            ctx.draw(crop, in: CGRect(x: 0, y: 0, width: 100, height: 70))
            ctx.restoreGState()
            return BorderParity.bytes(ctx)
        }
        return (native ? Int(scale) : 1, pictures)
    }

    /// A page's cases on screen, in the order `check` asks for them: the
    /// first ask, which names the page's appearance, pictures them all.
    private func onScreen(_ cases: [BorderParity.Case]) -> (NodeStyle, Bool, CGColor) -> [UInt8] {
        var pictures: [[UInt8]] = [], next = 0
        return { [self] _, dark, page in
            if pictures.isEmpty { pictures = onScreen(cases.map(\.style), dark: dark, page: page)!.pixels }
            defer { next += 1 }
            return pictures[next]
        }
    }

    /// On screen, where the box is layers: the same Chrome pictures. A
    /// Retina window's picture is resampled to Chrome's 1×, which moves every
    /// antialiased edge a little (the drawn cases measure 1–2.2 mean there),
    /// so the band is wider than the drawing tests'. Patterned borders stay
    /// at the capture's native scale: resampling fills the 1px dots' gaps.
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
        let patterns = onScreen(BorderParity.patterns().map(\.style), dark: false, page: CGColor(gray: 1, alpha: 1), native: true)!
        var nextPattern = 0
        failures += BorderParity.checkPatterns(scale: patterns.scale) { _, _, _ in
            defer { nextPattern += 1 }
            return patterns.pixels[nextPattern]
        }
        failures += GradientParity.check(dark: false, band: band, render: onScreen(GradientParity.cases()))
        failures += GradientParity.check(dark: true, band: band, render: onScreen(GradientParity.cases()))
        XCTAssert(failures.isEmpty, failures.joined(separator: "\n"))
    }

    func testEveryBorderCaseMatchesChrome() {
        var failures = BorderParity.check(flipped: false, dark: false) { render($0, dark: $1, page: $2) }
        failures += BorderParity.check(flipped: true, dark: true) { render($0, dark: $1, page: $2) }
        XCTAssert(failures.isEmpty, failures.joined(separator: "\n"))
    }

    func testPatternedBordersMatchChrome() {
        let failures = BorderParity.checkPatterns { render($0, dark: $1, page: $2) }
        XCTAssert(failures.isEmpty, failures.joined(separator: "\n"))
    }

    func testThinDottedUnderlineBandRejectsMissingInk() {
        for scale in [1, 2] {
            var blank = [UInt8](repeating: 255, count: 100 * 70 * scale * scale * 4)
            XCTAssertFalse(BorderParity.underlineHasInkAndGaps(blank, scale: scale), "blank passed at \(scale)×")
            for x in 10 * scale..<90 * scale {
                let i = ((70 * scale - 1) * 100 * scale + x) * 4
                blank[i] = 229; blank[i + 1] = 57; blank[i + 2] = 53
            }
            XCTAssertFalse(BorderParity.underlineHasInkAndGaps(blank, scale: scale), "solid line passed at \(scale)×")
        }
    }

    func testThinBoxGuardsRejectBlankAndSolidEdges() {
        for scale in [1, 2] {
            let chrome = BorderParity.chrome(scale == 1 ? "border-patterns.web.png" : "border-patterns.web-2x.png")
            for (x, y) in [(20, 20), (20, 115)] {
                var box: [UInt8] = []
                for py in 0..<70 * scale {
                    for px in 0..<100 * scale {
                        let i = ((y * scale + py) * chrome.0 + x * scale + px) * 4
                        box.append(contentsOf: chrome.2[i..<i + 4])
                    }
                }
                XCTAssertTrue(BorderParity.thinBoxEdgesHaveInkAndGaps(box, scale: scale))
                // Mutate one edge at a time: three good edges must not hide
                // either a missing edge or a solid fallback on the fourth.
                for edge in BorderParity.thinBoxEdges(scale: scale) {
                    for color: [UInt8] in [[255, 255, 255], [229, 57, 53]] {
                        var broken = box
                        for (px, py) in edge {
                            let i = (py * 100 * scale + px) * 4
                            broken.replaceSubrange(i..<i + 3, with: color)
                        }
                        XCTAssertFalse(BorderParity.thinBoxEdgesHaveInkAndGaps(broken, scale: scale), "bad edge passed at \(scale)×")
                    }
                    if y == 20 {
                        XCTAssertTrue(BorderParity.thinDotPhaseMatchesChrome(box, reference: chrome, at: x, y, scale: scale))
                        var shifted = box
                        for (offset, point) in edge.enumerated() {
                            let source = edge[(offset + scale) % edge.count]
                            let a = (point.1 * 100 * scale + point.0) * 4
                            let b = (source.1 * 100 * scale + source.0) * 4
                            shifted.replaceSubrange(a..<a + 4, with: box[b..<b + 4])
                        }
                        XCTAssertTrue(BorderParity.thinBoxEdgesHaveInkAndGaps(shifted, scale: scale))
                        XCTAssertFalse(BorderParity.thinDotPhaseMatchesChrome(shifted, reference: chrome, at: x, y, scale: scale), "wrong phase passed at \(scale)×")
                    }
                }
            }
        }
    }

    func testPatternedBorderLeavesGaps() {
        for pattern in ["dotted", "dashed"] {
            var style = BorderParity.cases(flipped: false)[0].style
            style["background_color"] = .array([.number(255), .number(255), .number(255), .number(255)])
            for side in ["top", "right", "bottom", "left"] {
                style["border_width_" + side] = .number(4)
                style["border_color_" + side] = .array([.number(255), .number(0), .number(0), .number(255)])
                style["border_style_" + side] = .string(pattern)
            }
            let pixels = render(style, dark: false, page: CGColor(gray: 1, alpha: 1))
            let rows = (0..<70).map { y in (10..<90).map { pixels[(y * 100 + $0) * 4 + 1] } }
            let borderRows = rows.filter { $0.filter { $0 < 100 }.count > 4 }
            XCTAssertFalse(borderRows.isEmpty, "\(pattern) has ink")
            XCTAssertTrue(borderRows.allSatisfy { $0.contains(where: { $0 > 245 }) }, "\(pattern) leaves gaps")
        }
    }

    /// LLP 1066: the gradient page's cases (`GradientParity`), light then dark.
    func testEveryGradientCaseMatchesChrome() {
        var failures = GradientParity.check(dark: false) { render($0, dark: $1, page: $2) }
        failures += GradientParity.check(dark: true) { render($0, dark: $1, page: $2) }
        XCTAssert(failures.isEmpty, failures.joined(separator: "\n"))
    }
}
#endif
