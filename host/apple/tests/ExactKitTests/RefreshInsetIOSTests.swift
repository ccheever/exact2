#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A scroller's pull-to-refresh control draws below its `padding-top`
/// (LLP 1010 §6.9, React Native's `progressViewOffset`): a padded list's
/// spinner shows between a header laid over the padding and the first row.
///   bun host/apple/build.mjs --test --ios
final class RefreshInsetIOSTests: XCTestCase {
    func testTheControlDrawsBelowTheScrollersPaddingTop() throws {
        let p = Presenter()
        let owner = NodeView(id: 1, kind: "list", presenter: p)
        owner.frame = CGRect(x: 0, y: 0, width: 390, height: 600)
        owner.style["padding_top"] = 92
        let scroll = ScrollView(frame: owner.bounds)
        scroll.contentSize = CGSize(width: 390, height: 3000)
        owner.addSubview(scroll)
        let control = PaddedRefreshControl()
        scroll.refreshControl = control
        control.setNeedsLayout()
        control.layoutIfNeeded()
        XCTAssertEqual(control.bounds.origin.y, -92, "the spinner draws 92 points down")
        // Laid out again (UIKit does so every frame of a pull), it stays.
        control.setNeedsLayout()
        control.layoutIfNeeded()
        XCTAssertEqual(control.bounds.origin.y, -92)
        // No padding: UIKit's own place.
        owner.style["padding_top"] = 0
        control.setNeedsLayout()
        control.layoutIfNeeded()
        XCTAssertEqual(control.bounds.origin.y, 0)
    }
}
#endif
