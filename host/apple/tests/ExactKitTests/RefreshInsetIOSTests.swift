#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A scroller's pull-to-refresh control sits below its `padding-top`
/// (LLP 1010 §6.9, React Native's `progressViewOffset`): a padded list's
/// spinner shows between a header laid over the padding and the first row.
///   bun host/apple/build.mjs --test --ios
final class RefreshInsetIOSTests: XCTestCase {
    func testTheControlSitsBelowTheScrollersPaddingTop() throws {
        let p = Presenter()
        let owner = NodeView(id: 1, kind: "list", presenter: p)
        owner.frame = CGRect(x: 0, y: 0, width: 390, height: 600)
        owner.style["padding_top"] = 92
        let scroll = ScrollView(frame: owner.bounds)
        scroll.contentSize = CGSize(width: 390, height: 3000)
        owner.addSubview(scroll)
        let control = PaddedRefreshControl()
        scroll.refreshControl = control
        let top = { control.convert(CGPoint.zero, to: owner).y }
        control.setNeedsLayout()
        control.layoutIfNeeded()
        XCTAssertEqual(top(), 92, "the control's top is 92 points down the port")
        // Laid out again (UIKit does so as a pull goes), it stays.
        control.setNeedsLayout()
        control.layoutIfNeeded()
        XCTAssertEqual(top(), 92)
        // A pull moves the content, not the control's place in the port.
        scroll.contentOffset = CGPoint(x: 0, y: -40)
        control.setNeedsLayout()
        control.layoutIfNeeded()
        XCTAssertEqual(top(), 92)
    }

}
#endif
