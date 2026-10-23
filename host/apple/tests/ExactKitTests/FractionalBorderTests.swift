import XCTest
import CoreGraphics
#if os(macOS)
import AppKit
#else
import UIKit
#endif
@testable import ExactKit

final class FractionalBorderTests: XCTestCase {
    func testLayerBordersSnapToChromesDevicePixels() throws {
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        // Chrome 156, --force-device-scale-factor=1/2/3: the black rows
        // on each edge of a 40x20 border-box for these authored widths.
        let widths = [0.333, 0.5, 0.75, 1.25, 1.5]
        let blackRows = [[1, 1, 1, 1, 1], [1, 1, 1, 2, 3], [1, 1, 2, 3, 4]]
        for scale in [1, 2, 3] {
            for (index, width) in widths.enumerated() {
                let presenter = Presenter()
                let node = NodeView(id: 1, kind: "view", presenter: presenter)
                presenter.views[1] = node
                node.frame = CGRect(x: 0, y: 0, width: 40, height: 20)
                #if os(macOS)
                node.wantsLayer = true
                node.layer?.contentsScale = CGFloat(scale)
                #else
                node.contentScaleFactor = CGFloat(scale)
                #endif
                var style: NodeStyle = ["background_color": .array([.number(255), .number(255), .number(255), .number(255)])]
                for side in ["top", "right", "bottom", "left"] {
                    style["border_width_" + side] = .number(width)
                    style["border_style_" + side] = .string("solid")
                    style["border_color_" + side] = .array([.number(0), .number(0), .number(0), .number(255)])
                }
                node.applyStyle(style)
                let ctx = try XCTUnwrap(CGContext(data: nil, width: 40 * scale, height: 20 * scale, bitsPerComponent: 8,
                    bytesPerRow: 40 * scale * 4, space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
                ctx.scaleBy(x: CGFloat(scale), y: CGFloat(scale))
                ctx.setFillColor(CGColor(gray: 1, alpha: 1)); ctx.fill(node.bounds)
                #if os(macOS)
                node.layer?.displayIfNeeded()
                node.layer?.render(in: ctx)
                #else
                node.layer.displayIfNeeded()
                node.layer.render(in: ctx)
                #endif
                let image = try XCTUnwrap(ctx.makeImage()), pixels = BorderParity.pixels(image).2
                let greens = (0..<20 * scale).map { pixels[($0 * 40 * scale + 20 * scale) * 4 + 1] }
                let black = Array(repeating: UInt8(0), count: blackRows[scale - 1][index])
                let white = Array(repeating: UInt8(255), count: 20 * scale - black.count * 2)
                XCTAssertEqual(greens, black + white + black, "width \(width) at \(scale)x")
            }
        }
    }
}
