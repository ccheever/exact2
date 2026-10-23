import XCTest
import CoreGraphics
import ExactCore
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
    func testLayoutClipAndGradientMeetThePaintedBorder() throws {
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        for scale in [1, 2, 3] {
            let session = try HeadlessSession(plan: fixturePlan("FRACTIONAL_BORDERS"), size: CGSize(width: 200, height: 200),
                facts: SessionFacts(displayScale: Double(scale), preferences: []))
            defer { session.close() }
            let snapshot = try session.snapshot()
            for (name, width) in [("thin", 0.5), ("middle", 0.9), ("thick", 1.5)] {
                let box = try XCTUnwrap(snapshot.nodes.values.first { $0.props["testId"] == name })
                let child = try XCTUnwrap(snapshot.nodes.values.first { $0.props["testId"] == "child-" + name })
                let used = CGFloat(max(1, floor(width * Double(scale) + 0.001))) / CGFloat(scale)
                XCTAssertEqual(child.frame.minX, used, accuracy: 0.00001)
                XCTAssertEqual(box.contentBox.minX, used, accuracy: 0.00001)
                let presenter = Presenter(), node = NodeView(id: 1, kind: "view", presenter: presenter)
                presenter.views[1] = node
                node.frame = CGRect(origin: .zero, size: box.frame.size)
                #if os(macOS)
                node.wantsLayer = true; node.layer?.contentsScale = CGFloat(scale)
                #else
                node.contentScaleFactor = CGFloat(scale)
                #endif
                node.applyStyle(box.style)
                node.applyKernelContentBox(box.contentBox, padding: nil)
                let inner = node.bounds.insetBy(dx: used, dy: used)
                XCTAssertEqual(node.gradientBox.minX, inner.minX, accuracy: 0.00001)
                let outline = CGPath(rect: node.bounds, transform: nil)
                let clip = try XCTUnwrap(node.backgroundOutline(outline))
                XCTAssertEqual(clip.boundingBox.minX, inner.minX, accuracy: 0.00001)
                let w = Int((node.bounds.width * CGFloat(scale)).rounded()), h = Int((node.bounds.height * CGFloat(scale)).rounded())
                let ctx = try XCTUnwrap(CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                    space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
                ctx.scaleBy(x: CGFloat(scale), y: CGFloat(scale))
                ctx.setFillColor(CGColor(red: 1, green: 0, blue: 1, alpha: 1)); ctx.fill(node.bounds)
                // Paint the clipped background and border as a capture does.
                node.paintBackground(ctx, border: outline, color: true)
                #if os(macOS)
                NSGraphicsContext.saveGraphicsState()
                NSGraphicsContext.current = NSGraphicsContext(cgContext: ctx, flipped: true)
                node.drawBoxBorder()
                NSGraphicsContext.restoreGraphicsState()
                #else
                node.boxDrawn = true
                UIGraphicsPushContext(ctx); node.draw(node.bounds); UIGraphicsPopContext()
                #endif
                let pixels = BorderParity.pixels(try XCTUnwrap(ctx.makeImage())).2
                let greens = (0..<h).map { pixels[($0 * w + w / 2) * 4 + 1] }
                let edge = Int((used * CGFloat(scale)).rounded())
                XCTAssertEqual(greens, Array(repeating: 0, count: edge) + Array(repeating: 255, count: h - 2 * edge) + Array(repeating: 0, count: edge), "seam: \(width) at \(scale)x")
                #if os(macOS)
                node.field = NSTextField()
                #else
                node.field = UITextField()
                #endif
                XCTAssertTrue(node.isNativeTextControl)
                XCTAssertEqual(node.contentBox().minX, used, accuracy: 0.00001, "native field inset")
            }
        }
    }

}
