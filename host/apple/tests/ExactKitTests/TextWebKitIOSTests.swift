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
}
#endif
