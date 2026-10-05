import XCTest
import CoreGraphics
@testable import ExactKit

/// LLP 1100 D2: a colour row in its own space becomes a `CGColor` in that
/// space, nothing clipped; the paths that still take channels get its sRGB
/// clip.
final class WideColorTests: XCTestCase {
    func testADisplayP3ColourIsDrawnInDisplayP3() throws {
        let v: BatchValue = ["cs": [["s": "display-p3", "v": [1, 0, 0, 0.5]]], "c": [255, 0, 0, 128]]
        let c = try XCTUnwrap(v.cgColor(dark: false))
        XCTAssertEqual(c.colorSpace?.name, CGColorSpace.extendedDisplayP3)
        XCTAssertEqual(c.components, [1, 0, 0, 0.5])
        // Converted for comparison only: P3 red is outside sRGB.
        let srgb = try XCTUnwrap(c.converted(to: CGColorSpace(name: CGColorSpace.extendedSRGB)!, intent: .defaultIntent, options: nil))
        XCTAssertGreaterThan(srgb.components![0], 1.0)
        XCTAssertLessThan(srgb.components![1], 0.0)
        XCTAssertEqual(v.channels(dark: false), [255, 0, 0, 128], "the clip, for channel paths")
        XCTAssertFalse(v.isSchemeColor)
        let plain: BatchValue = [255, 128, 0, 255]
        XCTAssertEqual(try XCTUnwrap(plain.cgColor(dark: false)).colorSpace?.name, CGColorSpace.sRGB, "an sRGB row is still sRGB")
    }

    func testAPairPicksItsHalfAndIsSchemeAware() throws {
        let v: BatchValue = ["cs": [["s": "srgb-linear", "v": [-0.2, 1.1, 0.1, 1]], ["s": "srgb", "v": [0, 0, 0, 1]]],
                             "c": [[0, 255, 0, 255], [0, 0, 0, 255]]]
        XCTAssertTrue(v.isSchemeColor)
        XCTAssertEqual(try XCTUnwrap(v.cgColor(dark: false)).colorSpace?.name, CGColorSpace.extendedLinearSRGB)
        XCTAssertEqual(try XCTUnwrap(v.cgColor(dark: false)).components?[0], -0.2)
        XCTAssertEqual(try XCTUnwrap(v.cgColor(dark: true)).colorSpace?.name, CGColorSpace.extendedSRGB)
        XCTAssertEqual(v.channels(dark: true), [0, 0, 0, 255])
    }

    func testTextCarriesTheSpaceThroughItsChannels() throws {
        let v: BatchValue = ["cs": [["s": "display-p3", "v": [0, 1, 0, 1]]], "c": [0, 255, 0, 255]]
        let c = try XCTUnwrap(v.textChannels(dark: false))
        XCTAssertEqual(c.count, 9)
        XCTAssertEqual(Array(c[0..<4]), [0, 255, 0, 255], "the first four are the sRGB clip, as every reader expects")
        XCTAssertEqual(TextEngine.color(c).cgColor.colorSpace?.name, CGColorSpace.extendedDisplayP3)
        let plain: BatchValue = [10, 20, 30, 255]
        XCTAssertEqual(plain.textChannels(dark: false), [10, 20, 30, 255])
    }

    func testABoxShadowIsCastInItsSpace() throws {
        let row: BatchValue = [["o": [0, 2], "b": 4, "c": ["cs": [["s": "display-p3", "v": [0, 1, 0, 0.5]]], "c": [0, 255, 0, 128]]]]
        let spec = try XCTUnwrap(BoxShadowSpec.list(row, dark: false).first)
        XCTAssertEqual(spec.color.colorSpace?.name, CGColorSpace.extendedDisplayP3)
        XCTAssertEqual(spec.color.alpha, 0.5)
    }

    func testMixedAppearanceGradientDenseSamplesOnlyItsLegacyScheme() throws {
        for legacyDark in [false, true] {
            let legacy: BatchValue = [0, 128, 0, 0, 128, 1, 0, 0, 64, 64]
            let wide: BatchValue = [0, 1.2, -0.04, -0.02, 1, 1, 0, 0, 1, 1]
            let row: BatchValue = ["linear": 180, "stops": legacyDark ? wide : legacy,
                "dark": legacyDark ? legacy : wide,
                "space": legacyDark ? "srgb-linear" : "srgb",
                "darkSpace": legacyDark ? "srgb" : "srgb-linear"]
            let gradient = try XCTUnwrap(Gradient(row))
            let (locations, colors) = gradient.stops(dark: legacyDark, dense: true)
            XCTAssertEqual(locations.count, 17)
            let midpoint = try XCTUnwrap(colors.first(where: { color in
                abs((color.components?[3] ?? 0) - 96.0 / 255) < 0.00001
            }))
            XCTAssertEqual(midpoint.colorSpace?.name, CGColorSpace.sRGB)
            XCTAssertEqual(try XCTUnwrap(midpoint.components)[0], 64.0 / 255, accuracy: 0.00001)
            XCTAssertEqual(try XCTUnwrap(midpoint.components)[2], 32.0 / 255, accuracy: 0.00001)
            let (wideLocations, wideColors) = gradient.stops(dark: !legacyDark, dense: true)
            XCTAssertEqual(wideLocations.count, 2)
            XCTAssertEqual(wideColors[0].colorSpace?.name, CGColorSpace.extendedLinearSRGB)
        }
    }

    func testAGradientInterpolatedElsewhereIsDrawnInExtendedLinearSRGB() throws {
        let row: BatchValue = ["linear": 180, "stops": [0, 1.2, -0.04, -0.02, 1, 1, 0, 0, 1, 1], "space": "srgb-linear"]
        let g = try XCTUnwrap(Gradient(row))
        let (locations, colors) = g.stops(dark: false, dense: true)
        XCTAssertEqual(locations, [0, 1], "already dense: not resampled in sRGB")
        XCTAssertEqual(colors[0].colorSpace?.name, CGColorSpace.extendedLinearSRGB)
        XCTAssertEqual(colors[0].components?[0], 1.2, "outside sRGB, unclipped")
    }
}
