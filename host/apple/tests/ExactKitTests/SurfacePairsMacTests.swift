#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// The Mac's surface pairs (LLP 1095 D2's roles; theme2's Mac surface-roles ruling): a card on a plain page and a card
/// on a grouped page each draw apart from their page, in light, dark and both increased-contrast appearances, in
/// AppKit's own colours (on current macOS the window's and a control's backgrounds are one colour, so a card names the
/// under-page grey).
final class SurfacePairsMacTests: XCTestCase {
    /// AppKit's colour, resolved in `appearance`, as 0...255 sRGB channels
    private func appKit(_ color: NSColor, _ appearance: NSAppearance.Name) -> [Double] {
        var out: [Double] = []
        NSAppearance(named: appearance)!.performAsCurrentDrawingAppearance {
            let c = color.usingColorSpace(.sRGB)!
            out = [c.redComponent, c.greenComponent, c.blueComponent, c.alphaComponent].map { ($0 * 255).rounded() }
        }
        return out
    }

    func testEachCardDrawsApartFromItsPageInEveryAppearance() throws {
        _ = NSApplication.shared
        let path = try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_SURFACE_PAIRS_PLAN"], "build.mjs --test compiles the surface-pairs fixture")
        let session = ExactApp.shared.makeSession(label: "surface-pairs-mac")
        defer { session.destroy() }
        XCTAssertNil(session.boot(plan: try Data(contentsOf: URL(fileURLWithPath: path)), size: CGSize(width: 402, height: 874)).error)
        func row(_ testId: String) throws -> BatchValue {
            let node = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == testId }, "no \(testId)")
            return try XCTUnwrap(node.style["background_color"], "\(testId) has its role")
        }
        let appearances: [(NSAppearance.Name, dark: Bool, contrast: Bool)] = [
            (.aqua, false, false), (.darkAqua, true, false),
            (.accessibilityHighContrastAqua, false, true), (.accessibilityHighContrastDarkAqua, true, true)]
        for (page, card) in [("plain", "plain-card"), ("grouped", "grouped-card")] {
            let (pageRow, cardRow) = (try row(page), try row(card))
            for (appearance, dark, contrast) in appearances {
                let drawn = { (r: BatchValue) in r.channels(dark: dark, contrast: contrast, elevated: false, tint: nil)?.map { $0.rounded() } }
                let (onPage, onCard) = (try XCTUnwrap(drawn(pageRow)), try XCTUnwrap(drawn(cardRow)))
                XCTAssertNotEqual(onPage, onCard, "\(card) on \(page), \(appearance.rawValue)")
                XCTAssertEqual(onPage, appKit(.windowBackgroundColor, appearance), "\(page), \(appearance.rawValue): the window's")
                XCTAssertEqual(onCard, appKit(.underPageBackgroundColor, appearance), "\(card), \(appearance.rawValue): the under-page grey")
            }
        }
    }
}
#endif
