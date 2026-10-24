// LLP 1019 §13 D11: CSS `font-weight` on a platform face is UIKit's (AppKit's)
// named instance at each hundred and the `wght` axis between them.
import XCTest
import CoreText
@testable import ExactKit
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

final class PlatformWeightsTests: XCTestCase {
    let engine = TextEngine(resolve: { _ in nil })

    private func wght(_ font: PlatformFont) -> Double {
        (CTFontCopyVariation(font as CTFont) as? [NSNumber: NSNumber])?[0x77676874 as NSNumber]?.doubleValue ?? 400
    }
    private func name(_ font: PlatformFont) -> String { CTFontCopyPostScriptName(font as CTFont) as String }

    func testEachHundredIsTheNamedInstance() {
        for (i, named) in TextEngine.namedWeights.enumerated() {
            let ours = engine.font(size: 17, weight: (i + 1) * 100, family: 0, italic: false)
            let theirs = PlatformFont.systemFont(ofSize: 17, weight: named)
            XCTAssertEqual(name(ours), name(theirs), "CSS \((i + 1) * 100)")
            XCTAssertEqual(wght(ours), wght(theirs), "CSS \((i + 1) * 100)")
            XCTAssertEqual(TextEngine.cssWeight(of: theirs), (i + 1) * 100)
        }
        XCTAssertEqual(wght(engine.font(size: 17, weight: 600, family: 0, italic: false)), 590, "San Francisco's semibold, not Chrome's 600")
    }

    func testBetweenHundredsTheAxisRunsFromOneNamedInstanceToTheNext() {
        func at(_ weight: Int) -> Double { wght(engine.font(size: 17, weight: weight, family: 0, italic: false)) }
        XCTAssertEqual(at(650), 645, accuracy: 0.01, "halfway from 590 to 700")
        XCTAssertEqual(at(550), 550, accuracy: 0.01, "halfway from 510 to 590")
        XCTAssertEqual(at(1000), 1000, accuracy: 0.01)
        XCTAssertEqual(at(1), 1, accuracy: 0.01, "the axis minimum")
        var last = 0.0
        for weight in stride(from: 1, through: 1000, by: 7) {
            XCTAssertGreaterThanOrEqual(at(weight), last, "continuous and rising at \(weight)")
            last = at(weight)
        }
        let between = engine.font(size: 17, weight: 650, family: 0, italic: false)
        XCTAssertEqual(TextEngine.cssWeight(of: between), 650)
    }

    func testItalicKeepsItsWeight() {
        let italic = engine.font(size: 17, weight: 700, family: 0, italic: true)
        XCTAssertTrue(name(italic).hasSuffix("BoldItalic"), name(italic))
        XCTAssertEqual(wght(italic), 700)
    }

    func testOtherDesignsUseTheirOwnNamedInstances() {
        // New York's semibold is `wght` 568, SF Mono's 571.
        let serif = engine.font(size: 17, weight: 600, family: 3, italic: false)
        XCTAssertTrue(name(serif).hasSuffix("Semibold"), name(serif))
        let mono = engine.font(size: 17, weight: 600, family: 5, italic: false)
        XCTAssertEqual(wght(mono), wght(PlatformFont.monospacedSystemFont(ofSize: 17, weight: .semibold)))
        // SF Mono's light is its axes' minimum, and its `YAXS` moves with `wght`.
        let light = PlatformFont.monospacedSystemFont(ofSize: 17, weight: .light)
        let regular = PlatformFont.monospacedSystemFont(ofSize: 17, weight: .regular)
        XCTAssertEqual(name(engine.font(size: 17, weight: 300, family: 5, italic: false)), name(light))
        func axis(_ font: PlatformFont, _ tag: UInt32) -> Double {
            let set = (CTFontCopyVariation(font as CTFont) as? [NSNumber: NSNumber])?[tag as NSNumber]?.doubleValue
            let axes = CTFontCopyVariationAxes(font as CTFont) as? [[CFString: Any]] ?? []
            let initial = axes.first { ($0[kCTFontVariationAxisIdentifierKey] as? NSNumber)?.uint32Value == tag }?[kCTFontVariationAxisDefaultValueKey]
            return set ?? (initial as? NSNumber)?.doubleValue ?? .nan
        }
        let between = engine.font(size: 17, weight: 350, family: 5, italic: false)
        for tag in [0x77676874, 0x59415853] as [UInt32] { // 'wght', 'YAXS'
            XCTAssertEqual(axis(between, tag), (axis(light, tag) + axis(regular, tag)) / 2, accuracy: 0.01, "halfway, axis \(tag)")
        }
    }
}
