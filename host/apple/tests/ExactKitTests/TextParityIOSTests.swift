#if os(iOS)
import XCTest
import UIKit
import CoreText
import CExact
@testable import ExactKit

/// iOS against Chrome on the Mac (TextParityCases.swift). iOS's SF has other
/// vertical metrics than the Mac's, so no rounding rule reaches Chrome's
/// numbers at every size: iOS keeps CoreText's sum, rounded up once per
/// paragraph, which is Chrome's height for a line at 12-15 px (the scroll
/// fixture's 14 px rows: 17, smoke.mjs's cross-host 652) and 1-2 px taller at
/// 11, 16 and 17 (14, 20, 21 against 13, 18, 20). Baselines are unrounded.
///   bun host/apple/build.mjs --test --ios
final class TextParityIOSTests: XCTestCase {
    func testRuntimeInstalledFamilyUsesItsOwnFace() throws {
        guard !TextEngine.installedFaces("Menlo").isEmpty else { throw XCTSkip("Menlo unavailable") }
        let engine = TextEngine(resolve: { _ in nil })
        let name = Array("Menlo".utf8)
        name.withUnsafeBufferPointer { n in
            var face = ExactFontFace()
            face.family = n.baseAddress; face.family_len = n.count; face.stack = 8
            withUnsafePointer(to: &face) { row in
                var catalog = ExactFontCatalog()
                catalog.faces = row; catalog.count = 1
                withUnsafePointer(to: &catalog) { engine.install($0) }
            }
        }
        let font = engine.font(size: 20, weight: 700, family: 8, italic: false) as CTFont
        XCTAssertEqual(CTFontCopyFamilyName(font) as String, "Menlo")
    }
    func testALinesHeightIsChromesAt12To15Px() {
        assertChromeNormal([(400, 12, 15, 12), (400, 13, 16, 13), (400, 14, 17, 14), (400, 15, 18, 15),
                            (600, 13, 16, 13), (600, 15, 18, 15)], baselines: false)
    }

    func testIntrinsicWidthsAreChromes() { assertChromeWidths() }
}
#endif
