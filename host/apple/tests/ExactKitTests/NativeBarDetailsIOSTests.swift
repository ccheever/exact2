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

    /// The selected tab's own tint reaches the tab bar as written while UIKit dims tints: UIKit dims
    /// the bar itself, and keeps the tint once the dimming ends
    func testTheSelectedTabsOwnTintIsAsWrittenWhileDimmed() throws {
        let p = Presenter(), list = NodeView(id: 1, kind: "view", presenter: p)
        let tab = NodeView(id: 2, kind: "button", presenter: p)
        tab.applyStyle(["tint_color": [[255, 0, 0, 255], [0, 255, 0, 255]], "written": ["tint_color"]])
        list.addSubview(tab)
        let nav = p.navigation, controller = UITabBarController()
        nav.tabController = controller
        nav.tabItemsSource = (NavigationTabs(tablist: list, tabs: [tab], panels: []), [])
        nav.routerTab = 0
        list.tintAdjustmentMode = .dimmed
        nav.tint(list)
        XCTAssertEqual(controller.tabBar.tintColor.resolvedColor(with: UITraitCollection(userInterfaceStyle: .light)), .red)
        XCTAssertEqual(controller.tabBar.tintColor.resolvedColor(with: UITraitCollection(userInterfaceStyle: .dark)), .green)
        // A tint of its own that names the tint takes the tablist's accent at rest, each appearance its side
        list.applyStyle(["accent_color": [[0, 0, 255, 255], [255, 255, 0, 255]], "written": ["accent_color"]])
        tab.applyStyle(["tint_color": ["sys": "@tint", "c": [0, 0, 255, 255]], "written": ["tint_color"]])
        nav.tint(list)
        XCTAssertEqual(controller.tabBar.tintColor.resolvedColor(with: UITraitCollection(userInterfaceStyle: .light)), .blue, "the accent's light side")
        XCTAssertEqual(controller.tabBar.tintColor.resolvedColor(with: UITraitCollection(userInterfaceStyle: .dark)), .yellow, "the accent's dark side")
    }

    func testAnOwnTintEqualToTheAccentReplacesItsLiveResolver() throws {
        let p = Presenter(), list = NodeView(id: 1, kind: "view", presenter: p)
        let tab = NodeView(id: 2, kind: "button", presenter: p)
        list.applyStyle(["accent_color": [0, 0, 255, 255], "written": ["accent_color"]])
        list.addSubview(tab)
        let nav = p.navigation, controller = UITabBarController()
        nav.tabController = controller
        nav.tabItemsSource = (NavigationTabs(tablist: list, tabs: [tab], panels: []), [])
        nav.routerTab = 0
        nav.tint(list)
        tab.applyStyle(["tint_color": [0, 0, 255, 255], "written": ["tint_color"]])
        nav.tint(list)
        list.applyStyle(["accent_color": [255, 0, 0, 255], "written": ["accent_color"]])
        XCTAssertEqual(controller.tabBar.tintColor.resolvedColor(with: UITraitCollection(userInterfaceStyle: .dark)), .blue,
                       "the selected tab owns the colour even when its channels matched the old accent")
        list.applyStyle(["accent_color": [0, 0, 255, 255], "written": ["accent_color"]])
        tab.applyStyle([:])
        nav.tint(list)
        list.applyStyle(["accent_color": [255, 0, 0, 255], "written": ["accent_color"]])
        XCTAssertEqual(controller.tabBar.tintColor.resolvedColor(with: UITraitCollection(userInterfaceStyle: .dark)), .red,
                       "clearing an own tint reinstalls the live accent resolver even at equal channels")
    }

    /// A seal beside a pressable title is drawn in its colour as written while UIKit dims tints: the
    /// title's text keeps it once the dimming ends
    func testAHeadingRowsSealBesideAPressableTitleIsAsWrittenWhileDimmed() throws {
        let p = Presenter()
        let header = NodeView(id: 1, kind: "view", presenter: p)
        let row = NodeView(id: 2, kind: "view", presenter: p)
        let heading = NodeView(id: 3, kind: "text", presenter: p)
        let seal = NodeView(id: 4, kind: "image", presenter: p)
        header.handlers = ["press"]
        row.applyStyle(["display": "flex", "flex_direction": "row"])
        heading.props = ["text": "Verified"]
        seal.props = ["imageSource": "symbol:checkmark.seal.fill", "symbolName": "checkmark.seal.fill"]
        seal.applyStyle(["tint_color": [[255, 0, 0, 255], [0, 255, 0, 255]], "written": ["tint_color"]])
        header.addSubview(row); row.addSubview(heading); row.addSubview(seal)
        header.tintAdjustmentMode = .dimmed
        XCTAssertTrue(seal.dimsItsPaint, "the seal is on a dimmed control's face")
        let group = try XCTUnwrap(HeaderTitle(header: header, heading: heading, tap: nil, apart: []))
        let view = HeaderTitleView(host: p.navigation)
        view.title.textColor = .label
        view.update(group, title: "Verified")
        let title = try XCTUnwrap(view.title.attributedText)
        var count = 0
        title.enumerateAttribute(.attachment, in: NSRange(location: 0, length: title.length)) { value, range, _ in
            guard value is NSTextAttachment else { return }
            count += 1
            let ink = title.attribute(.foregroundColor, at: range.location, effectiveRange: nil) as? UIColor
            XCTAssertEqual(ink?.resolvedColor(with: UITraitCollection(userInterfaceStyle: .light)), .red)
            XCTAssertEqual(ink?.resolvedColor(with: UITraitCollection(userInterfaceStyle: .dark)), .green)
        }
        XCTAssertEqual(count, 1)
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
        view.title.textColor = .label
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
        let headingInk = title.attribute(.foregroundColor, at: 0, effectiveRange: nil) as? UIColor
        XCTAssertEqual(headingInk?.resolvedColor(with: UITraitCollection(userInterfaceStyle: .dark)),
                       UIColor.label.resolvedColor(with: UITraitCollection(userInterfaceStyle: .dark)))
        seal.applyStyle([:])
        let untinted = try XCTUnwrap(HeaderTitle(header: header, heading: heading, tap: nil, apart: []))
        view.update(untinted, title: "Verified")
        let inherited = try XCTUnwrap(view.title.attributedText)
        inherited.enumerateAttribute(.attachment, in: NSRange(location: 0, length: inherited.length)) { value, range, _ in
            guard value is NSTextAttachment else { return }
            let ink = inherited.attribute(.foregroundColor, at: range.location, effectiveRange: nil) as? UIColor
            XCTAssertEqual(ink?.resolvedColor(with: UITraitCollection(userInterfaceStyle: .dark)),
                           UIColor.label.resolvedColor(with: UITraitCollection(userInterfaceStyle: .dark)))
        }
        view.title.textColor = .red
        view.update(untinted, title: "Verified")
        let changed = try XCTUnwrap(view.title.attributedText)
        XCTAssertEqual(changed.attribute(.foregroundColor, at: 0, effectiveRange: nil) as? UIColor, .red,
                       "an unchanged title still follows a changed heading colour")
        XCTAssertTrue(title.string.hasPrefix("Verified"))
        XCTAssertEqual(view.accessibilityLabel, "Verified")
        XCTAssertTrue(view.subtitle.isHidden)
    }
}
#endif
