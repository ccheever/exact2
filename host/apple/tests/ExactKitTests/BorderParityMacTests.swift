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

    /// The window server's picture of the node (its layer properties and
    /// sublayers where Core Animation says the box, `BoxLayerMac.swift`),
    /// as the parity smokes see a Mac window; nil where no window server
    /// answers (a process may read its own windows without permission).
    private func onScreen(_ style: NodeStyle, dark: Bool, page: CGColor) -> [UInt8]? {
        _ = NSApplication.shared
        let p = Presenter()
        let window = NSWindow(contentRect: NSRect(x: 40, y: 40, width: 100, height: 70), styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.backgroundColor = NSColor(cgColor: page)
        window.appearance = NSAppearance(named: dark ? .darkAqua : .aqua)
        window.contentView = p.root
        let node = NodeView(id: 1, kind: "view", presenter: p)
        node.frame = NSRect(x: 0, y: 0, width: 100, height: 70)
        p.root.addSubview(node); p.views[1] = node
        node.applyStyle(style)
        window.orderFrontRegardless()
        defer { window.orderOut(nil) }
        node.display()
        CATransaction.flush()
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.15))
        typealias Create = @convention(c) (CGRect, UInt32, UInt32, UInt32) -> Unmanaged<CGImage>?
        // A sleeping display or a locked screen answers with an empty picture
        // (`Agent.emptyPicture`): no picture, as with no window server.
        guard let sym = dlsym(UnsafeMutableRawPointer(bitPattern: -2), "CGWindowListCreateImage"),
              let image = unsafeBitCast(sym, to: Create.self)(.null, 1 << 3, UInt32(window.windowNumber), 1 << 0)?.takeRetainedValue(),
              !Agent.emptyPicture(image)
        else { return nil }
        let ctx = BorderParity.canvas(page)
        ctx.saveGState()
        ctx.translateBy(x: 0, y: 70); ctx.scaleBy(x: 1, y: -1)
        ctx.interpolationQuality = .high
        ctx.draw(image, in: CGRect(x: 0, y: 0, width: 100, height: 70))
        ctx.restoreGState()
        return BorderParity.bytes(ctx)
    }

    /// On screen, where the box is layers: the same Chrome pictures. A
    /// Retina window's picture is resampled to Chrome's 1×, which moves every
    /// antialiased edge a little (the drawn cases measure 1–2.2 mean there),
    /// so the band is wider than the drawing tests'.
    func testEveryCaseMatchesChromeOnScreen() throws {
        guard onScreen([:], dark: false, page: CGColor(gray: 1, alpha: 1)) != nil else { throw XCTSkip("no window server picture (no window server, or its display is asleep or the screen locked)") }
        let band = (mean: 3.0, over: 4.0)
        var failures = BorderParity.check(flipped: false, dark: false, band: band) { onScreen($0, dark: $1, page: $2)! }
        failures += GradientParity.check(dark: false, band: band) { onScreen($0, dark: $1, page: $2)! }
        failures += GradientParity.check(dark: true, band: band) { onScreen($0, dark: $1, page: $2)! }
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
