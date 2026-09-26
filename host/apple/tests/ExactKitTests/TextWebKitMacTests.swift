#if os(macOS)
import XCTest
import AppKit
@testable import ExactKit

/// WKWebView on macOS 27 (TextWebKitCases.swift): macOS WebKit rounds a
/// face's ascent, descent and line gap to the nearest whole pixel. It also
/// floors a set line height to a whole pixel, which is not followed (Chrome
/// keeps the fraction; LLP 1001 §6), so no set-height case is pinned here.
final class TextWebKitMacTests: XCTestCase {
    func testNormalLineHeightIsWebKitsForTheSystemFaces() {
        assertWebKitNormal([
            (0, 400, 11, 13, 11), (0, 400, 12, 15, 12), (0, 400, 13, 16, 13), (0, 400, 15, 18, 15), (0, 400, 16, 18, 15),
            (0, 400, 17, 20, 16), (0, 400, 20, 23, 19), (0, 400, 24, 28, 23), (0, 400, 28, 33, 27), (0, 400, 34, 40, 33),
            (0, 600, 13, 16, 13), (0, 600, 16, 18, 15), (5, 400, 13, 16, 13), (5, 400, 16, 18, 15), (5, 600, 17, 20, 16),
        ])
    }

    func testAFallbackFacesLineBoxIsWebKits() {
        assertWebKitFallback([
            ("مرحبا", 13, 16, 13), ("สวัสดี", 13, 18, 14), ("明天", 13, 16, 13), ("مرحبا", 15, 19, 15),
            ("สวัสดี", 15, 20, 16), ("明天", 15, 18, 15), ("مرحبا", 16, 20, 16), ("สวัสดี", 16, 22, 17),
            ("明天", 16, 18, 15), ("مرحبا", 21, 26, 21), ("สวัสดี", 21, 29, 23), ("明天", 21, 24, 20),
        ])
    }

    func testIntrinsicWidthsAreWebKitsAdvances() { assertWebKitWidths() }
}
#endif
