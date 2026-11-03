#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactGroupedLists

/// A row's switch whose control's `accent-color` is `AccentColor` draws its on
/// state in UIKit's tint: configured out of the window, as an unselected tab's
/// content is, the window's tint once it is in it, and the new one when that
/// tint changes.
final class GroupedSwitchAccentIOSTests: GroupedListFixture {
    func testASwitchConfiguredOutOfTheWindowTakesTheAccentInIt() throws {
        let p = presenter { self.model() }
        window.tintColor = .systemTeal
        p.viewport.removeFromSuperview()
        p.apply(wireBatch([["op": "style", "id": 13, "style": ["accent_color": ["sys": "@tint", "c": [[0, 136, 255, 255], [0, 145, 255, 255]]]]]]))
        let toggle = try XCTUnwrap(switches(try cell(p, 12)).first)
        window.addSubview(p.viewport)
        XCTAssertEqual(toggle.onTintColor, UIColor.tintColor, "UIKit's tint")
        XCTAssertEqual(rgba(toggle.onTintColor, in: toggle), rgba(.systemTeal), "in the window, the app's accent")
        window.tintColor = .systemPink
        XCTAssertEqual(rgba(toggle.onTintColor, in: toggle), rgba(.systemPink), "a changed accent")
        // A tint change outside a batch changes the colour UIKit draws, and nothing else
        p.views[13]?.props["checked"] = "false"
        window.tintColor = .systemTeal
        XCTAssertEqual(rgba(toggle.onTintColor, in: toggle), rgba(.systemTeal))
        XCTAssertTrue(toggle.isOn, "its state is the batch's to set")
    }

    private func switches(_ view: UIView) -> [UISwitch] {
        (view as? UISwitch).map { [$0] } ?? view.subviews.flatMap(switches)
    }

    /// `color` as it draws in `view` (light when there is none): UIKit's tint resolves by its traits
    private func rgba(_ color: UIColor?, in view: UIView? = nil) -> [Int] {
        var c: (CGFloat, CGFloat, CGFloat, CGFloat) = (0, 0, 0, 0)
        color?.resolvedColor(with: view?.traitCollection ?? UITraitCollection(userInterfaceStyle: .light)).getRed(&c.0, green: &c.1, blue: &c.2, alpha: &c.3)
        return [c.0, c.1, c.2, c.3].map { Int(($0 * 255).rounded()) }
    }
}
#endif
