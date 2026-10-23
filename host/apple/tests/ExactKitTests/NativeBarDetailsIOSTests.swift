#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

final class NativeBarDetailsIOSTests: XCTestCase {
    func testTheSelectedTabsOwnTintOverridesTheTablistAccent() throws {
        let p = Presenter(), list = NodeView(id: 1, kind: "view", presenter: p)
        let first = NodeView(id: 2, kind: "button", presenter: p)
        let second = NodeView(id: 3, kind: "button", presenter: p)
        list.applyStyle(["accent_color": [0, 0, 255, 255], "written": ["accent_color"]])
        first.applyStyle(["tint_color": [[255, 0, 0, 255], [0, 255, 0, 255]], "written": ["tint_color"]])
        list.addSubview(first); list.addSubview(second)
        let nav = p.navigation, controller = UITabBarController()
        nav.tabController = controller
        nav.tabItemsSource = (NavigationTabs(tablist: list, tabs: [first, second], panels: []), [])
        nav.routerTab = 0
        nav.tint(list)
        XCTAssertEqual(controller.tabBar.tintColor.resolvedColor(with: UITraitCollection(userInterfaceStyle: .light)), .red)
        XCTAssertEqual(controller.tabBar.tintColor.resolvedColor(with: UITraitCollection(userInterfaceStyle: .dark)), .green)
        nav.routerTab = 1
        nav.tint(list)
        XCTAssertEqual(controller.tabBar.tintColor.resolvedColor(with: UITraitCollection(userInterfaceStyle: .light)), .blue)
        nav.routerTab = 0
        first.applyStyle(["tint_color": [255, 0, 0, 255]])
        nav.tint(list)
        XCTAssertEqual(controller.tabBar.tintColor.resolvedColor(with: UITraitCollection(userInterfaceStyle: .light)), .blue,
                       "an inherited tint leaves the tablist's accent")
    }

    func testAHeadingRowsSealIsDrawnBesideTheTitle() throws {
        let p = Presenter()
        let header = NodeView(id: 1, kind: "view", presenter: p)
        let row = NodeView(id: 2, kind: "view", presenter: p)
        let heading = NodeView(id: 3, kind: "text", presenter: p)
        let seal = NodeView(id: 4, kind: "image", presenter: p)
        row.applyStyle(["display": "flex", "flex_direction": "row"])
        heading.props = ["text": "Verified"]
        seal.props = ["imageSource": "symbol:checkmark.seal.fill", "symbolName": "checkmark.seal.fill"]
        seal.applyStyle(["tint_color": [[255, 0, 0, 255], [0, 255, 0, 255]], "written": ["tint_color"]])
        header.addSubview(row); row.addSubview(heading); row.addSubview(seal)
        let group = try XCTUnwrap(HeaderTitle(header: header, heading: heading, tap: nil, apart: []))
        XCTAssertTrue(group.drawn)
        XCTAssertTrue(group.placed.contains { $0 === seal })
        let view = HeaderTitleView(host: p.navigation)
        view.update(group, title: "Verified")
        let title = try XCTUnwrap(view.title.attributedText)
        var count = 0
        title.enumerateAttribute(.attachment, in: NSRange(location: 0, length: title.length)) { value, range, _ in
            if let attachment = value as? NSTextAttachment {
                count += 1
                XCTAssertEqual(attachment.image?.renderingMode, .alwaysTemplate)
                let ink = title.attribute(.foregroundColor, at: range.location, effectiveRange: nil) as? UIColor
                XCTAssertEqual(ink?.resolvedColor(with: UITraitCollection(userInterfaceStyle: .light)), .red)
                XCTAssertEqual(ink?.resolvedColor(with: UITraitCollection(userInterfaceStyle: .dark)), .green)
            }
        }
        XCTAssertEqual(count, 1)
        XCTAssertTrue(title.string.hasPrefix("Verified"))
        XCTAssertEqual(view.accessibilityLabel, "Verified")
        XCTAssertTrue(view.subtitle.isHidden)
    }
}
#endif
