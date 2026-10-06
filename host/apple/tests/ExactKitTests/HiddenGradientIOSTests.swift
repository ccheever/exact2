import XCTest
import CoreGraphics
@testable import ExactKit
#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// A hidden box paints no `background-image`. The gradient is drawn from
/// `paintGradient`, which iOS `draw` calls even when the colour is skipped.
final class HiddenGradientIOSTests: XCTestCase {
    private func reds(_ node: NodeView, _ paint: (CGContext) -> Void) -> Int {
        let w = max(1, Int(node.bounds.width)), h = max(1, Int(node.bounds.height))
        guard let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                                  space: CGColorSpaceCreateDeviceRGB(),
                                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return 0 }
        #if os(macOS)
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = NSGraphicsContext(cgContext: ctx, flipped: true)
        paint(ctx)
        NSGraphicsContext.restoreGraphicsState()
        #else
        UIGraphicsPushContext(ctx)
        paint(ctx)
        UIGraphicsPopContext()
        #endif
        guard let raw = ctx.data else { return 0 }
        let bytes = raw.bindMemory(to: UInt8.self, capacity: w * h * 4)
        var count = 0
        for i in stride(from: 0, to: w * h * 4, by: 4) where bytes[i] > 180 && bytes[i + 1] < 60 && bytes[i + 2] < 60 && bytes[i + 3] > 180 {
            count += 1
        }
        return count
    }

    func testAHiddenBoxPaintsNoGradient() throws {
        let p = Presenter()
        let gradient: [String: Any] = ["linear": 180, "stops": [0, 255, 0, 0, 255, 1, 255, 0, 0, 255]]
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "style": ["background_image": gradient, "visibility": "hidden"]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 80.0, "h": 40.0],
        ]))
        let node = try XCTUnwrap(p.views[1])
        XCTAssertTrue(node.cssVisibilityHidden)
        let painted = reds(node) { node.paintGradient($0, clip: CGPath(rect: node.bounds, transform: nil)) }
        XCTAssertEqual(painted, 0, "paintGradient still fills a hidden box")
        let drawn = reds(node) { _ in node.draw(node.bounds) }
        XCTAssertEqual(drawn, 0, "draw still paints the hidden gradient")
    }
}
