#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1075.003 §9.11: `navigationDetent`'s words as UIKit detents, and
/// `fit-content` resolving to the route's content height within the sheet's
/// maximum.
final class ModalDetentIOSTests: XCTestCase {
    private final class Context: NSObject, UISheetPresentationControllerDetentResolutionContext {
        let containerTraitCollection = UITraitCollection()
        let maximumDetentValue: CGFloat
        init(_ maximum: CGFloat) { maximumDetentValue = maximum }
    }

    func testTheWordsBecomeDetentsInOrder() {
        let detents = ModalHost.detents("fit-content 300 large nonsense") { 120 }
        XCTAssertEqual(detents.map(\.identifier.rawValue), ["fit-content-0", "authored-1", UISheetPresentationController.Detent.Identifier.large.rawValue])
        XCTAssertEqual(detents[1].resolvedValue(in: Context(700)), 300)
        XCTAssertTrue(ModalHost.detents(nil) { nil }.isEmpty)
    }

    func testFitContentIsTheContentHeightWithinTheMaximum() {
        var content: CGFloat? = 180.4
        let detent = ModalHost.detents("fit-content") { content }[0]
        XCTAssertEqual(detent.resolvedValue(in: Context(700)), 181, "rounded up to a whole point")
        content = 900
        XCTAssertEqual(detent.resolvedValue(in: Context(700)), 700, "clamped to the sheet's maximum")
        // Read at each resolution: a grown route resolves anew.
        content = 260
        XCTAssertEqual(detent.resolvedValue(in: Context(700)), 260)
        // Not measured yet: the sheet's maximum, as the default large sheet.
        content = nil
        XCTAssertEqual(detent.resolvedValue(in: Context(700)), 700)
        content = 0
        XCTAssertEqual(detent.resolvedValue(in: Context(700)), 700)
    }
}
#endif
