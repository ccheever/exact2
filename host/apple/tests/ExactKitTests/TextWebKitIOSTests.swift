#if os(iOS)
import XCTest
import UIKit
@testable import ExactKit

/// Safari on the iOS 27 simulator (TextWebKitCases.swift): iOS WebKit rounds
/// a face's ascent, descent and line gap up to whole pixels. CoreText's own
/// sum (SwiftUI's line height) is 19.09 at 16 px; Safari's is 20.
///   bun host/apple/build.mjs --test --ios
final class TextWebKitIOSTests: XCTestCase {
    func testNormalLineHeightIsSafarisForTheSystemFaces() {
        assertWebKitNormal([
            (0, 400, 11, 14, 11), (0, 400, 12, 15, 12), (0, 400, 13, 17, 13), (0, 400, 15, 19, 15), (0, 400, 16, 20, 16),
            (0, 400, 17, 22, 17), (0, 400, 20, 25, 20), (0, 400, 24, 29, 23), (0, 400, 28, 34, 27), (0, 400, 34, 42, 33),
            (0, 600, 13, 17, 13), (0, 600, 15, 19, 15), (0, 600, 17, 22, 17),
            (5, 400, 13, 17, 13), (5, 400, 15, 19, 15), (5, 400, 16, 20, 16), (5, 600, 15, 19, 15),
        ])
    }

    func testAFallbackFacesLineBoxIsSafaris() {
        assertWebKitFallback([
            ("مرحبا", 13, 18, 14), ("สวัสดี", 13, 19.5, 15.5), ("明天", 13, 17, 13), ("مرحبا", 15, 20, 16),
            ("สวัสดี", 15, 23, 18), ("明天", 15, 19, 15), ("مرحبا", 16, 21, 17), ("สวัสดี", 16, 24, 19),
            ("明天", 16, 20, 16), ("مرحبا", 21, 28, 22), ("สวัสดี", 21, 30, 24), ("明天", 21, 26, 20),
        ])
    }

    func testASetLineHeightCentresSafarisContentArea() {
        assertWebKitExplicit([
            ("20px", 13, 20, 14.5), ("20px", 16, 20, 16), ("20px", 17, 20, 16), ("20px", 20, 20, 17.5),
            ("24px", 13, 24, 16.5), ("24px", 16, 24, 18), ("17px", 16, 17, 14.5), ("17px", 20, 17, 16),
            ("1.2", 13, 15.5938, 12.2969), ("1.2", 16, 19.1875, 15.5938), ("1.2", 17, 20.3906, 16.1875),
            ("1.5", 13, 19.5, 14.25), ("1.5", 17, 25.5, 18.75), ("25.3px", 13, 25.2969, 17.1406), ("25.3px", 20, 25.2969, 20.1406),
        ])
    }

    func testIntrinsicWidthsAreSafarisAdvances() { assertWebKitWidths() }

    /// Safari's underline at 3× (the iPhone 17 Pro): its top one point below
    /// the baseline (two at 34 px), `size / 16` thick rounded up to pixels.
    func testAnUnderlineIsWhereSafariPaintsIt() throws {
        let engine = TextEngine(resolve: { _ in nil })
        let context = try XCTUnwrap(CGContext(data: nil, width: 30, height: 30, bitsPerComponent: 8, bytesPerRow: 120,
                                              space: CGColorSpaceCreateDeviceRGB(),
                                              bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        context.scaleBy(x: 3, y: 3)
        for (size, below, pixels): (CGFloat, CGFloat, CGFloat) in [(12, 1, 3), (13, 1, 3), (15, 1, 3), (16, 1, 3),
                                                                  (17, 1, 4), (20, 1, 4), (24, 1, 5), (34, 2, 7)] {
            let run = Run(text: "xxxx", size: size, weight: 400, family: 0, italic: false, lineHeight: nil,
                          letterSpacing: 0, decoration: "underline")
            let p = engine.paragraph(Spec(runs: [run], align: 0, lineClamp: 0, color: [0, 0, 0, 255]), width: .infinity)
            let marks = TextLinePaint.underlines(p.lines[0], at: CGPoint(x: 0, y: 5), in: context)
            let rect = try XCTUnwrap(marks.first?.0, "\(size)px")
            XCTAssertEqual(rect.minY, 5 + below, accuracy: 0.001, "\(size)px")
            XCTAssertEqual(rect.height * 3, pixels, accuracy: 0.001, "\(size)px")
        }
    }
}
#endif
